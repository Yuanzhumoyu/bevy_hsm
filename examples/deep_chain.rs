//! # 深层状态链 / Deep State Chain
//!
//! 本示例演示深层嵌套状态链中的守卫自动转换：
//! - **深层链 (Deep Chain)**: OFF → ON1 → ON2 → ON3 四级嵌套状态
//! - **守卫导航**: ↑ 键触发 GuardEnter, ↓ 键触发 GuardExit
//! - **BeforeEnter/AfterExit**: 转换系统在状态切换前后执行
//!
//! This example demonstrates guard-driven traversal in a deep state chain:
//! - **Deep chain**: OFF → ON1 → ON2 → ON3 (four nested levels)
//! - **Guard navigation**: ↑ triggers GuardEnter, ↓ triggers GuardExit
//! - **BeforeEnter/AfterExit**: transition systems fire around state changes
//!
//! ## 状态结构 / State Structure
//! ```text
//! OFF (Nested)
//!  └── ON1 (Nested, GuardEnter="is_up", GuardExit="is_down")
//!       └── ON2 (Nested, GuardEnter="is_up", GuardExit="is_down")
//!            └── ON3 (Nested, GuardEnter="is_up", GuardExit="is_down")
//! ```
//!
//! ## 操作 / Controls
//! - **↑ / 上箭头**: 向下进入更深的状态 (触发 GuardEnter)
//! - **↓ / 下箭头**: 向上退出到父状态 (触发 GuardExit)
//!
//! ## 工作流程 / Workflow
//! 1. 初始在 OFF, 按 ↑ → GuardEnter 放行, 自动进入 ON1
//! 2. 再按 ↑ → 进入 ON2
//! 3. 按 ↓ → GuardExit 放行, 自动退出到 ON1

use bevy::prelude::*;
use bevy_hsm::prelude::*;

// ── 守卫系统: 键盘驱动 / Guard Systems: keyboard-driven ──────────

fn guard_on_up(In(_): In<GuardContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowUp)
}

fn guard_on_down(In(_): In<GuardContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowDown)
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

fn log_before_enter(In(ctx): In<TransitionContext>) {
    info!(
        "  → BeforeEnter: from={:?} to={:?}",
        ctx.from_state(),
        ctx.to_state()
    );
}

fn log_after_exit(In(ctx): In<TransitionContext>) {
    info!(
        "  → AfterExit:  from={:?} to={:?}",
        ctx.from_state(),
        ctx.to_state()
    );
}

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    mut guard_registry: ResMut<GuardRegistry>,
    mut action_registry: ResMut<ActionRegistry>,
    mut transition_registry: ResMut<TransitionRegistry>,
) {
    guard_registry.extend([
        ("is_up", commands.register_system(guard_on_up)),
        ("is_down", commands.register_system(guard_on_down)),
    ]);
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);
    transition_registry.extend([
        ("before_enter", commands.register_system(log_before_enter)),
        ("after_exit", commands.register_system(log_after_exit)),
    ]);

    // 构建四级深层链 / Build four-level deep chain
    let off = commands
        .spawn((
            Name::new("OFF"),
            HsmState::default(),
            BeforeEnterSystem::new("before_enter"),
            AfterExitSystem::new("after_exit"),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let on1 = commands
        .spawn((
            Name::new("ON1"),
            HsmState::default(),
            GuardEnter::new("is_up"),
            GuardExit::new("is_down"),
            BeforeEnterSystem::new("before_enter"),
            AfterExitSystem::new("after_exit"),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let on2 = commands
        .spawn((
            Name::new("ON2"),
            HsmState::default(),
            GuardEnter::new("is_up"),
            GuardExit::new("is_down"),
            BeforeEnterSystem::new("before_enter"),
            AfterExitSystem::new("after_exit"),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let on3 = commands
        .spawn((
            Name::new("ON3"),
            HsmState::default(),
            GuardEnter::new("is_up"),
            GuardExit::new("is_down"),
            BeforeEnterSystem::new("before_enter"),
            AfterExitSystem::new("after_exit"),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let traversal = TraversalStrategy::default();
    let mut state_tree = StateTree::new(off);
    state_tree
        .with_traversal(off, traversal.clone())
        .with_child(off, on1)
        .with_traversal(on1, traversal.clone())
        .with_child(on1, on2)
        .with_traversal(on2, traversal)
        .with_child(on2, on3);

    let sm = commands.spawn_empty().id();
    commands.entity(sm).insert((
        state_tree,
        Name::new("DeepChain"),
        HsmStateMachine::with(
            sm,
            off,
            #[cfg(feature = "history")]
            10,
        ),
        StateLifecycle::default(),
    ));
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_systems(Startup, setup);

    app.run();
}
