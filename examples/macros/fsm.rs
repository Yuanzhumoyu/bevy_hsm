//! # fsm! 宏示例 / fsm! Macro Demo
//!
//! 本示例演示 `fsm!` 宏的声明式有限状态机定义：
//! - **`fsm!` 宏**: 在一段 DSL 中定义状态、转换和组件
//! - **`states`**: 定义状态实体, 支持动作系统、状态数据
//! - **`transitions`**: 定义事件驱动的转换
//! - **`components`**: 在状态机实体上附加组件
//!
//! This example demonstrates the `fsm!` macro for declarative FSM definition:
//! - **`fsm!` macro**: define states, transitions, and components in a DSL
//! - **`states`**: define state entities with action systems and state data
//! - **`transitions`**: define event-driven transitions
//! - **`components`**: attach components to the state machine entity
//!
//! ## 状态结构 / State Structure
//! ```text
//! state ──(MyEvent::Go)──> State1 ──(MyEvent::Go)──> State2 ──(MyEvent::Go)──> Minimal
//!   ^                                                                              │
//!   └────────────────────(MyEvent::Back)────────────────────────────────────────────┘
//! ```
//!
//! ## 操作 / Controls
//! - **↑ / 上箭头**: 发送 MyEvent::Go (前进)
//! - **↓ / 下箭头**: 发送 MyEvent::Back (后退)

use bevy::prelude::*;
use bevy_hsm::prelude::*;

// ── 状态数据组件 / State Data Components ──────────────────────────

#[derive(Component, Default, Clone)]
struct ComponentA;

#[derive(Component, Default, Clone)]
struct ComponentB;

#[derive(Component, Default, Clone)]
struct ComponentC;

#[derive(Component)]
struct ComponentD;

// ── 事件类型 / Event Type ─────────────────────────────────────────

#[derive(PartialEq, Clone, Eq, Hash, Debug)]
enum MyEvent {
    Go,
    Back,
}

// ── 动作系统 / Action Systems ─────────────────────────────────────

fn log_enter(In(ctx): In<ActionContext>, query: Query<&Name, With<FsmState>>) {
    let fallback = Name::new("unnamed");
    let name = query.get(ctx.state()).unwrap_or(&fallback);
    info!(">>> Enter  [{}]", name);
}

fn log_exit(In(ctx): In<ActionContext>, query: Query<&Name, With<FsmState>>) {
    let fallback = Name::new("unnamed");
    let name = query.get(ctx.state()).unwrap_or(&fallback);
    info!("<<< Exit   [{}]", name);
}

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    action_registry.extend([
        ("on_enter_name", commands.register_system(log_enter)),
        ("on_exit_name", commands.register_system(log_exit)),
    ]);

    // fsm! 宏语法说明 / fsm! macro syntax:
    //
    // states: { ... }     — 定义状态实体 / define state entities
    //   #[state(...)]: name — 状态属性 / state attributes:
    //     after_enter="..." — 进入后动作 / after-enter action
    //     before_exit="..." — 退出前动作 / before-exit action
    //     state_scene={...} — 进入时自动插入的组件 / auto-inserted components
    //   (ComponentA, ...) — 状态上的附加组件 / extra components on state
    //   #[state(minimal)] — 最简状态 (无额外配置)
    //
    // transitions: { ... } — 定义事件转换 / define event transitions
    //   from => to :event(MyEvent::Go) — 收到事件时从 from 转到 to
    //
    // components: { ... } — 附加到状态机实体的组件 / components on SM entity
    commands.spawn(fsm!(
        states: {
            // state 0: 带 state_scene 和附加组件
            #[state(after_enter="on_enter_name", before_exit="on_exit_name", state_scene={
                ComponentA
                ComponentB
            })]: state(
                ComponentC,
                ComponentD,
            ),
            // state 1: 引用组件类型作为状态名
            #[state(after_enter="on_enter_name", before_exit="on_exit_name")] ComponentA,
            // state 2: state_scene 和附加组件 (无组件名字段)
            #[state(after_enter="on_enter_name", before_exit="on_exit_name", state_scene={ComponentC})]
            (ComponentA, ComponentB),
            // state 3: 最简状态 / minimal state
            #[state(minimal)]: MinimalState,
        },
        transitions: {
            state => 1 :event(MyEvent::Go),
            1 => 2 :event(MyEvent::Go),
            2 => MinimalState :event(MyEvent::Go),
            MinimalState => 2 :event(MyEvent::Back),
            2 => 1 :event(MyEvent::Back),
            1 => 0 :event(MyEvent::Back),
        },
        components: {
            ComponentA,
            ComponentB,
        },
    ));
}

// ── 输入处理 / Input Handling ─────────────────────────────────────

fn handle_input(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    sm: Single<Entity, With<FsmStateMachine>>,
) {
    let sm_entity = sm.entity();

    if keyboard.just_pressed(KeyCode::ArrowUp) {
        info!("↑ Go");
        commands.trigger(FsmTrigger::with_event(
            sm_entity,
            EventData::new(MyEvent::Go),
        ));
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        info!("↓ Back");
        commands.trigger(FsmTrigger::with_event(
            sm_entity,
            EventData::new(MyEvent::Back),
        ));
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_systems(Startup, setup);
    app.add_systems(Update, handle_input);

    app.run();
}
