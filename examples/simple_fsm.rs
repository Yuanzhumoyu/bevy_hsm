//! # 简单 FSM / Simple FSM
//!
//! 本示例演示事件驱动的有限状态机基本用法：
//! - **事件转换 (Event Transition)**: 使用自定义事件在状态间切换
//! - **BeforeEnter/AfterExit**: 转换系统在状态切换前后执行
//! - **暂停/恢复**: Paused 标记暂停状态机 (暂停后事件被忽略)
//!
//! This example demonstrates basic event-driven FSM usage:
//! - **Event transitions**: use custom events to switch between states
//! - **BeforeEnter/AfterExit**: transition systems fire around state changes
//! - **Pause/Resume**: Paused marker pauses the state machine (events ignored)
//!
//! ## 状态结构 / State Structure
//! ```text
//! State A ──(ToggleEvent)──> State B
//!    ^                        │
//!    └───────(ToggleEvent)────┘
//! ```
//!
//! ## 操作 / Controls
//! - **空格 / Space**: 发送 ToggleEvent, 在 A ↔ B 之间切换
//! - **P**: 暂停/恢复状态机

use bevy::prelude::*;
use bevy_hsm::prelude::*;

// ── 自定义事件类型 / Custom Event Type ────────────────────────────

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct ToggleEvent;

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

fn log_on_transition(label: &str) -> impl Fn(In<TransitionContext>) {
    let label = label.to_string();
    move |ctx: In<TransitionContext>| {
        info!("  → {:?} {}", ctx.relationship(), label);
    }
}

fn log_update(contexts: In<Vec<ActionContext>>, query: Query<&Name>) -> Option<Vec<ActionContext>> {
    for name in query.iter_many(contexts.iter().map(|c| c.state())) {
        info!("--- Update [{}]", name);
    }
    Some(contexts.0)
}

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    mut action_registry: ResMut<ActionRegistry>,
    mut transition_registry: ResMut<TransitionRegistry>,
) {
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);
    transition_registry.extend([
        (
            "before_enter",
            commands.register_system(log_on_transition("before enter")),
        ),
        (
            "after_exit",
            commands.register_system(log_on_transition("after exit")),
        ),
    ]);

    let state_a = commands
        .spawn((
            FsmState,
            Name::new("State A"),
            BeforeEnterSystem::new("before_enter"),
            AfterExitSystem::new("after_exit"),
            AfterEnterSystem::new("log_enter"),
            OnUpdateSystem::new("Update:log_update"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let state_b = commands
        .spawn((
            FsmState,
            Name::new("State B"),
            BeforeEnterSystem::new("before_enter"),
            AfterExitSystem::new("after_exit"),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    // 双向事件图 / Bidirectional event graph
    let mut graph = FsmGraph::new(state_a);
    graph
        .with_event(state_a, ToggleEvent, state_b)
        .with_event(state_b, ToggleEvent, state_a);

    let graph_id = commands.spawn(graph).id();

    commands.spawn((
        FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ),
        Name::new("SimpleFSM"),
    ));
}

// ── 输入处理 / Input Handling ─────────────────────────────────────

fn handle_input(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    sm: Single<Entity, With<FsmStateMachine>>,
) {
    let sm_entity = sm.entity();

    if keyboard.just_pressed(KeyCode::Space) {
        commands.queue(move |world: &mut World| {
            info!("Space: sending ToggleEvent");
            world.trigger(FsmTrigger::with_event(
                sm_entity,
                EventData::new(ToggleEvent),
            ));
        });
    }

    if keyboard.just_pressed(KeyCode::KeyP) {
        commands.queue(move |world: &mut World| {
            let mut entity = world.entity_mut(sm_entity);
            if entity.contains::<Paused>() {
                info!("Resuming FSM");
                entity.remove::<Paused>();
            } else {
                info!("Pausing FSM — events will be ignored");
                entity.insert(Paused);
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
