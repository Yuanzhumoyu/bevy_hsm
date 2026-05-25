//! # hsm! 宏示例 / hsm! Macro Demo
//!
//! 本示例演示 `hsm!` 宏的声明式状态机定义：
//! - **`hsm!` 宏**: 在一段 DSL 中定义状态树、守卫和动作
//! - **`guard_enter`/`guard_exit`**: 守卫属性的宏语法
//! - **`behavior=Rebirth`**: 退出行为属性
//! - **`state_scene`**: 进入状态时自动插入的数据组件
//!
//! This example demonstrates the `hsm!` macro for declarative state machine definition:
//! - **`hsm!` macro**: define state tree, guards, and actions in a DSL
//! - **`guard_enter`/`guard_exit`**: guard attribute macro syntax
//! - **`behavior=Rebirth`**: exit behavior attribute
//! - **`state_scene`**: auto-inserted data component on state entry
//!
//! ## 状态结构 / State Structure
//! ```text
//! MyHSM
//!  └── Root (behavior=Rebirth)
//!       └── StateA (guard_enter="is_up", guard_exit="is_down", state_scene={StateAData})
//!            └── StateB (guard_enter="is_up", guard_exit="is_down")
//! ```
//!
//! ## 操作 / Controls
//! - **↑ / 上箭头**: 进入子状态 (触发 GuardEnter)
//! - **↓ / 下箭头**: 退出到父状态 (触发 GuardExit)

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_hsm::prelude::*;

// ── 守卫系统 / Guard Systems ──────────────────────────────────────

fn guard_on_up(In(_): In<GuardContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowUp)
}

fn guard_on_down(In(_): In<GuardContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowDown)
}

// ── 状态数据: 进入 StateA 时自动插入 / State Data: auto-inserted on StateA entry ──

#[derive(Component, Default, Clone)]
#[component(on_insert = Self::on_insert, on_remove = Self::on_remove)]
struct StateAData;

impl StateAData {
    fn on_insert(_world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        info!("StateAData inserted for state {:?}", entity);
    }

    fn on_remove(_world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        info!("StateAData removed for state {:?}", entity);
    }
}

/// 验证 StateAData 在进入 StateA 时被添加, 退出时被移除
/// Verifies StateAData is added on entering StateA, removed on exit
fn check_state_a_data(query: Query<(), (With<StateAData>, With<HsmStateMachine>)>) {
    if !query.is_empty() {
        info!("--> Found StateAData component on the state machine!");
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
        ("is_up", commands.register_system(guard_on_up)),
        ("is_down", commands.register_system(guard_on_down)),
    ]);
    action_registry.extend([
        ("on_enter_name", commands.register_system(log_enter)),
        ("on_exit_name", commands.register_system(log_exit)),
    ]);

    // hsm! 宏语法说明 / hsm! macro syntax:
    //
    // hsm!(
    //     StateLifecycle::default(),       // 插入 Enter 以启动 / insert Enter to boot
    //     Name::new("MyHSM"),              // 状态机名称 / SM name
    //     #[state(...)]: Root(             // 根状态 / root state
    //         #[state(                     // 子状态属性 / child state attributes:
    //             guard_enter="is_up",     //   - 进入守卫名 / enter guard name
    //             guard_exit="is_down",    //   - 退出守卫名 / exit guard name
    //             after_enter="...",       //   - 进入后动作 / after-enter action
    //             before_exit="...",       //   - 退出前动作 / before-exit action
    //             state_scene={StateAData} //   - 状态数据组件 / state data component
    //         )]: StateA(
    //             #[state(...)]: StateB
    //         )
    //     )
    // )
    commands.spawn(hsm!(
        StateLifecycle::default(),
        Name::new("MyHSM"),
        #[state(after_enter="on_enter_name", before_exit="on_exit_name", behavior=Rebirth)]: Root(
            #[state(
                guard_enter="is_up",
                guard_exit="is_down",
                after_enter="on_enter_name",
                before_exit="on_exit_name",
                state_scene={StateAData}
            )]: StateA(
                #[state(
                    guard_enter="is_up",
                    guard_exit="is_down",
                    after_enter="on_enter_name",
                    before_exit="on_exit_name"
                )]: StateB
            )
        )
    ));
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_systems(Startup, setup);
    app.add_systems(Update, check_state_a_data);

    app.run();
}
