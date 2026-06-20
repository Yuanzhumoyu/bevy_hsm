//! # 事件驱动有限状态机 / Event-Driven FSM
//!
//! 本示例演示事件驱动的有限状态机：
//! - **事件转换 (Event Transition)**: 当特定事件触发时切换到目标状态
//! - **动作系统 (Action Systems)**: 进入/退出/更新时执行自定义逻辑
//! - **暂停/恢复 (Pause/Resume)**: 使用 Paused 标记暂停状态机
//!
//! This example demonstrates an event-driven finite state machine:
//! - **Event transitions**: switch to target state when a specific event fires
//! - **Action systems**: custom logic at enter/exit/update
//! - **Pause/Resume**: pause the state machine with the Paused marker
//!
//! ## 状态结构 / State Structure
//! ```text
//! Red ──(ToggleEvent)──> Green
//!  ^                      │
//!  └──────(ToggleEvent)───┘
//! ```
//!
//! ## 操作 / Controls
//! - **空格 / Space**: 发送 ToggleEvent, 在 Red ↔ Green 之间切换
//! - **P**: 暂停/恢复状态机 (暂停后不再响应事件)
//!
//! ## 核心概念 / Core Concepts
//! - `StateEvent`: 实现此 trait 的类型可作为事件触发转换
//! - `FsmGraph::with_event(from, event, to)`: 注册事件转换
//! - `FsmTrigger::with_event(sm, event)`: 向状态机发送事件

use bevy::prelude::*;
use bevy_hsm::prelude::*;

// ── 自定义事件类型 / Custom Event Type ────────────────────────────

/// 状态事件: 实现 StateEvent trait
/// (需要 Clone + Eq + Hash + Send + Sync + Debug + 'static)
///
/// StateEvent: types implementing this trait can trigger transitions
/// (requires Clone + Eq + Hash + Send + Sync + Debug + 'static)
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

fn log_update(contexts: In<Vec<ActionContext>>, query: Query<&Name>) -> Option<Vec<ActionContext>> {
    for ctx in contexts.iter() {
        if let Ok(name) = query.get(ctx.state()) {
            info!("--- Update [{}]", name);
        }
    }
    Some(contexts.0)
}

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);

    // 创建状态实体 / Create state entities
    let red = commands
        .spawn((
            Name::new("Red"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let green = commands
        .spawn((
            Name::new("Green"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    // 构建 FSM 图: Red ←→ Green (通过 ToggleEvent)
    // Build FSM graph: Red ←→ Green (via ToggleEvent)
    let mut graph = FsmGraph::new(red);
    graph
        .with_event(red, ToggleEvent, green)
        .with_event(green, ToggleEvent, red);

    let graph_id = commands.spawn(graph).id();

    // 生成 FSM 状态机 / Spawn FSM state machine
    commands.spawn((
        FsmStateMachine::with(
            graph_id,
            red, // 初始状态 / initial state
            #[cfg(feature = "history")]
            10,
        ),
        Name::new("EventFSM"),
    ));
}

// ── 输入处理 / Input Handling ─────────────────────────────────────

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    sm: Single<Entity, With<FsmStateMachine>>,
    mut commands: Commands,
) {
    let sm_entity = *sm;

    if keyboard.just_pressed(KeyCode::Space) {
        // 发送 ToggleEvent — FsmTrigger 是 EntityEvent, 使用 World::trigger
        // Send ToggleEvent — FsmTrigger is an EntityEvent, use World::trigger
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
