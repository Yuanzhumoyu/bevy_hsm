//! # 守卫状态机 / Guarded HSM
//!
//! 本示例演示状态转换守卫 (Transition Guard)：
//! - **GuardEnter**: 满足条件时自动进入子状态，不满足时阻止进入
//!
//! This example demonstrates transition guards:
//! - **GuardEnter**: auto-enter child state when condition is met, block otherwise
//!
//! ## 状态结构 / State Structure
//! ```text
//! Root (Nested)
//!  └── Counter (Nested, GuardEnter="is_unlocked")
//! ```
//!
//! ## 操作 / Controls
//! - **空格 / Space**: 切换锁状态 / Toggle lock
//! - **Enter**: 从 Counter 返回 Root / Return to Root from Counter
//!
//! ## 工作流程 / Workflow
//! 1. 锁定状态: GuardEnter 阻止进入 Counter, 状态停留在 Root
//! 2. 按空格解锁: GuardEnter 放行, 自动进入 Counter
//! 3. 按 Enter: 手动返回 Root

use bevy::prelude::*;
use bevy_hsm::prelude::*;

// ── 锁状态 / Lock State ───────────────────────────────────────────

#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
enum Lock {
    #[default]
    Locked,
    Unlocked,
}

// ── 守卫系统 / Guard Systems ──────────────────────────────────────

/// 守卫: 检查是否已解锁
/// Guard: check if unlocked
fn guard_is_unlocked(In(ctx): In<GuardContext>, query: Query<&Lock>) -> bool {
    query
        .get(ctx.state_machine)
        .map(|s| *s == Lock::Unlocked)
        .unwrap_or(false)
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

fn log_update(contexts: In<Vec<ActionContext>>, query: Query<&Name>) -> Option<Vec<ActionContext>> {
    for ctx in contexts.iter() {
        if let Ok(name) = query.get(ctx.state()) {
            info!("--- Update [{}]", name);
        }
    }
    Some(contexts.0)
}

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    mut action_registry: ResMut<ActionRegistry>,
    mut guard_registry: ResMut<GuardRegistry>,
) {
    // 注册守卫系统和动作系统
    // Register guard systems and action systems
    guard_registry.extend([("is_unlocked", commands.register_system(guard_is_unlocked))]);
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);

    let root = commands
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    // Counter 的 GuardEnter 引用 "is_unlocked" 守卫系统
    // 守卫系统返回 true 时自动进入, 返回 false 时停留在 Root
    // Counter's GuardEnter references the "is_unlocked" guard system
    // When guard returns true: auto-enter; when false: stay at Root
    let counter = commands
        .spawn((
            Name::new("Counter"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
            GuardEnter::new("is_unlocked"),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, counter);

    let sm = commands.spawn_empty().id();
    commands.entity(sm).insert((
        tree,
        Name::new("GuardedHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
        Lock::Locked,
    ));
}

// ── 输入处理 / Input Handling ─────────────────────────────────────

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    sm: Single<Entity, With<HsmStateMachine>>,
    mut commands: Commands,
) {
    let sm_entity = *sm;

    if keyboard.just_pressed(KeyCode::Space) {
        commands.queue(move |world: &mut World| {
            let mut lock = world.get_mut::<Lock>(sm_entity).unwrap();
            *lock = match *lock {
                Lock::Locked => {
                    info!("Unlocked! GuardEnter will now allow entry to Counter");
                    Lock::Unlocked
                }
                Lock::Unlocked => {
                    info!("Locked! GuardEnter will block entry to Counter");
                    Lock::Locked
                }
            };
        });
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        commands.queue(move |world: &mut World| {
            let Some(sm_comp) = world.get::<HsmStateMachine>(sm_entity) else {
                return;
            };
            let tree_id = sm_comp.state_graph_id();
            let Some(tree) = world.get::<StateTree>(tree_id) else {
                return;
            };

            if sm_comp.curr_state_id() != tree.get_root() {
                info!("Manual ToSuper → returning to Root");
                world.entity_mut(sm_entity).trigger(HsmTrigger::to_super);
            } else {
                info!("Already at Root");
            }
        });
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_action_system(Update, "log_update", log_update);

    app.add_systems(Startup, setup);
    app.add_systems(Update, handle_input);

    app.run();
}
