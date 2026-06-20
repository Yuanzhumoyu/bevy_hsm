//! # 守卫计数器 / Guarded Counter
//!
//! 本示例演示状态转换守卫与并行策略的组合使用：
//! - **GuardEnter/GuardExit**: 通过守卫系统控制状态间的自动转换
//! - **Parallel 策略**: 父状态在子状态激活时保持存活
//! - **OnUpdate 动作**: 在状态更新时执行计数逻辑
//!
//! This example demonstrates guards combined with the Parallel strategy:
//! - **GuardEnter/GuardExit**: control auto-transitions via guard systems
//! - **Parallel strategy**: parent state stays alive when child activates
//! - **OnUpdate action**: execute counting logic on state update
//!
//! ## 状态结构 / State Structure
//! ```text
//! Start (Parallel + Rebirth)
//!  └── Counter (Nested + Rebirth, GuardEnter="is_open", GuardExit="is_close")
//! ```
//!
//! ## 操作 / Controls
//! - **空格 / Space**: 切换锁状态 (Open ↔ Close)
//!
//! ## 工作流程 / Workflow
//! 1. 初始: Switch=Close, GuardEnter 阻止进入 Counter
//! 2. 按空格 → Switch=Open, GuardEnter 放行, 自动进入 Counter, 开始计数
//! 3. 再按空格 → Switch=Close, GuardExit 放行, 自动退出 Counter

use bevy::prelude::*;
use bevy_hsm::prelude::*;

// ── 组件 / Components ─────────────────────────────────────────────

/// 锁开关: 控制守卫是否放行
/// Lock switch: controls whether guards allow transitions
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
enum Switch {
    Open,
    #[default]
    Close,
}

/// 计数器: 在 Counter 状态中持续累加
/// Counter: increments while in the Counter state
#[derive(Component, Default)]
struct Count(usize);

// ── 守卫系统 / Guard Systems ──────────────────────────────────────

impl Switch {
    fn guard_is_open(In(ctx): In<GuardContext>, query: Query<&Switch>) -> bool {
        query
            .get(ctx.state_machine)
            .map(|s| matches!(s, Switch::Open))
            .unwrap_or(false)
    }

    fn guard_is_close(In(ctx): In<GuardContext>, query: Query<&Switch>) -> bool {
        query
            .get(ctx.state_machine)
            .map(|s| matches!(s, Switch::Close))
            .unwrap_or(false)
    }
}

// ── OnUpdate 动作: 计数 / Counting action ─────────────────────────

impl Count {
    fn tick(
        states: In<Vec<ActionContext>>,
        mut query: Query<(&Name, &mut Count)>,
    ) -> Option<Vec<ActionContext>> {
        for ctx in states.0.iter() {
            if let Ok((name, mut count)) = query.get_mut(ctx.service_target) {
                count.0 += 1;
                info!("{} count: {}", name, count.0);
            }
        }
        Some(states.0)
    }
}

// ── 动作系统 / Action Systems ─────────────────────────────────────

fn log_enter(In(ctx): In<ActionContext>, query: Query<&Name>) {
    if let Ok(name) = query.get(ctx.state()) {
        info!(">>> Enter  [{}]", name);
    }
}

fn log_exit(In(ctx): In<ActionContext>, query: Query<&Name>) {
    if let Ok(name) = query.get(ctx.state()) {
        info!("<<< Exit   [{}]", name);
    }
}

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    mut guard_registry: ResMut<GuardRegistry>,
    mut action_registry: ResMut<ActionRegistry>,
) {
    guard_registry.extend([
        ("is_open", commands.register_system(Switch::guard_is_open)),
        ("is_close", commands.register_system(Switch::guard_is_close)),
    ]);
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);

    // Start: 使用 Parallel 策略, 子状态激活时父状态保持存活
    // Start: uses Parallel strategy, parent stays alive when child activates
    let start = commands
        .spawn((
            Name::new("Start"),
            HsmState::with(
                StateTransitionStrategy::Parallel,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    // Counter: 受 GuardEnter/GuardExit 控制的子状态
    // Counter: child state controlled by GuardEnter/GuardExit
    let counter = commands
        .spawn((
            Name::new("Counter"),
            HsmState::default(),
            GuardEnter::new("is_open"),
            GuardExit::new("is_close"),
            AfterEnterSystem::new("log_enter"),
            OnUpdateSystem::new("Update:count_tick"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let traversal = TraversalStrategy::default();
    let mut state_tree = StateTree::new(start);
    state_tree
        .with_traversal(start, traversal)
        .with_child(start, counter);

    let sm = commands.spawn_empty().id();
    commands.entity(sm).insert((
        state_tree,
        Name::new("GuardedCounter"),
        HsmStateMachine::with(
            sm,
            start,
            #[cfg(feature = "history")]
            10,
        ),
        StateLifecycle::default(),
        Switch::Close,
        Count(0),
    ));
}

// ── 输入处理 / Input Handling ─────────────────────────────────────

fn handle_input(input: Res<ButtonInput<KeyCode>>, mut query: Query<&mut Switch>) {
    if input.just_pressed(KeyCode::Space) {
        let mut switch = query.single_mut().unwrap();
        let old = *switch;
        *switch = match old {
            Switch::Open => Switch::Close,
            Switch::Close => Switch::Open,
        };
        info!("Switch: {:?} → {:?}", old, *switch);
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_action_system(Update, "count_tick", Count::tick);

    app.add_systems(Startup, setup);
    app.add_systems(Update, handle_input);

    app.run();
}
