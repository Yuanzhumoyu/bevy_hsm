//! # 中断与恢复 / Interrupt & Resume
//!
//! 本示例演示 HSM 的中断 (Interrupt) 和恢复 (Resume) 机制：
//! - **中断 (Interrupt)**: 保存当前状态上下文，跳转到另一个状态树中的目标状态
//! - **恢复 (Resume)**: 弹出中断栈，恢复到中断前的状态
//! - **中断栈 (Interrupt Stack)**: 支持嵌套中断，后进先出
//!
//! This example demonstrates HSM interrupt and resume:
//! - **Interrupt**: save current context, jump to a target state in another tree
//! - **Resume**: pop the interrupt stack, restore to the pre-interrupt state
//! - **Interrupt Stack**: supports nested interrupts (LIFO)
//!
//! ## 状态结构 / State Structure
//! ```text
//! Tree A:  Work → Idle (默认)
//! Tree B:  Alert (独立的中断目标 / independent interrupt target)
//! ```
//!
//! ## 操作 / Controls
//! - **I**: 中断到 TreeB/Alert / Interrupt to TreeB/Alert
//! - **R**: 恢复到 TreeA / Resume back to TreeA
//! - **Esc**: 退出 / Exit
//!
//! ## 工作流程 / Workflow
//! 1. SM 启动在 TreeA/Idle
//! 2. 按 I → 保存 Idle 上下文，中断到 TreeB/Alert
//! 3. 按 R → 弹出中断栈，恢复到 TreeA/Idle

use bevy::prelude::*;
use bevy_hsm::prelude::*;

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
    for ctx in contexts.0.iter() {
        if let Ok(name) = query.get(ctx.state()) {
            info!("--- Update [{}]", name);
        }
    }
    Some(contexts.0)
}

// ── 资源 / Resource ───────────────────────────────────────────────

/// 存储树 B 的引用, 供输入系统使用
/// Holds references to tree B for the input system
#[derive(Resource)]
struct TreeB {
    tree: Entity,
    alert: Entity,
}

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);

    // ── Tree A: 主状态树 / Primary state tree ─────────────────────
    let work = commands
        .spawn((
            Name::new("Work"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let idle = commands
        .spawn((
            Name::new("Idle"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut tree_a = StateTree::new(work);
    tree_a.with_child(work, idle);
    let tree_a_id = commands.spawn(tree_a).id();

    // ── Tree B: 中断目标 / Interrupt target ───────────────────────
    let alert = commands
        .spawn((
            Name::new("Alert"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let tree_b = StateTree::new(alert);
    let tree_b_id = commands.spawn(tree_b).id();

    // ── 状态机: 初始绑定 Tree A, 启动在 Idle ────────────────────
    // ── State machine: initially bound to Tree A, starts at Idle ──
    let sm = commands.spawn_empty().id();
    commands.entity(sm).insert((
        Name::new("InterruptHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            tree_a_id,
            idle,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    commands.insert_resource(TreeB {
        tree: tree_b_id,
        alert,
    });
}

// ── 输入处理 / Input Handling ─────────────────────────────────────

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    tree_b: Res<TreeB>,
    sm: Single<Entity, With<HsmStateMachine>>,
    mut commands: Commands,
) {
    if keyboard.just_pressed(KeyCode::KeyI) {
        let sm_entity = *sm;
        let tree_b_id = tree_b.tree;
        let alert = tree_b.alert;
        commands.queue(move |world: &mut World| {
            let Some(sm_comp) = world.get::<HsmStateMachine>(sm_entity) else {
                return;
            };
            info!(
                "Interrupt! Saving context (tree={:?}, state={:?}), jumping to Alert",
                sm_comp.state_graph_id(),
                sm_comp.curr_state_id()
            );
            info!(
                "  Interrupt depth before: {} (+1)",
                sm_comp.interrupt_stack().interrupt_depth()
            );
            world
                .entity_mut(sm_entity)
                .trigger(|id| HsmTrigger::interrupt(id, tree_b_id, alert));
        });
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        let sm_entity = *sm;
        commands.queue(move |world: &mut World| {
            let Some(sm_comp) = world.get::<HsmStateMachine>(sm_entity) else {
                return;
            };
            if sm_comp.interrupt_stack().is_interrupted() {
                info!(
                    "Resume! Restoring context, depth before: {} (-1)",
                    sm_comp.interrupt_stack().interrupt_depth()
                );
                world.entity_mut(sm_entity).trigger(HsmTrigger::resume);
            } else {
                info!("Not interrupted — nothing to resume");
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
