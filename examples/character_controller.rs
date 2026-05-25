//! # 角色状态机 / Character Controller
//!
//! 本示例演示使用 `hsm!` 宏构建角色状态机：
//! - **hsm! 宏**: 声明式定义状态树和转换
//! - **Chain 转换**: 在树中任意两个状态间切换（通过 LCA 计算路径）
//! - **Death 行为**: 退出后不自动恢复, 适用于互斥的分支状态
//!
//! This example demonstrates building a character state machine with the `hsm!` macro:
//! - **hsm! macro**: declarative state tree and transition definitions
//! - **Chain transition**: switch between any two states in the tree (via LCA path)
//! - **Death behavior**: no auto-recovery after exit, suitable for exclusive branches
//!
//! ## 状态结构 / State Structure
//! ```text
//! Root
//! ├── Movement (behavior=Death)
//! │   ├── Idle
//! │   └── Walking
//! └── Combat (behavior=Death)
//!     ├── Aiming
//!     └── Attacking
//! ```
//!
//! ## 操作 / Controls
//! - **Num 0**: 切换到 Idle
//! - **Num 1**: 切换到 Walking
//! - **Num 2**: 切换到 Aiming
//! - **Num 3**: 切换到 Attacking
//!
//! ## 核心概念 / Core Concepts
//! - `hsm!` 宏中的 `behavior=Death`: 同层兄弟状态互斥, 离开 Movement 时整个分支退出
//! - Chain 转换自动计算 LCA 路径, 执行 exit → enter 序列

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

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);

    // hsm! 宏声明式构建状态机:
    // hsm! macro builds the state machine declaratively:
    //
    // #[state(...)]: 配置状态的动作系统和行为
    //   - after_enter: 进入后执行的系统
    //   - before_exit: 退出前执行的系统
    //   - behavior=Death: 退出后不自动恢复 (兄弟状态互斥)
    //
    // :Root( ... ): 定义状态树结构
    // StateLifecycle::default(): 插入 Enter 以启动
    // :trigger_fn: 可选, 在状态实体生成后调用
    commands.spawn(hsm! {
       #[state(after_enter="log_enter", before_exit="log_exit")]:Root(
            #[state(after_enter="log_enter", before_exit="log_exit", behavior=Death)]:Movement(
                #[state(after_enter="log_enter", before_exit="log_exit")]:Idle,
                #[state(after_enter="log_enter", before_exit="log_exit")]:Walking,
            ),
            #[state(after_enter="log_enter", before_exit="log_exit", behavior=Death)]:Combat(
                #[state(after_enter="log_enter", before_exit="log_exit")]:Aiming,
                #[state(after_enter="log_enter", before_exit="log_exit")]:Attacking,
            )
       )
       StateLifecycle::default()
    });
}

// ── 输入处理 / Input Handling ─────────────────────────────────────

fn handle_input(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    hsm: Single<Entity, With<HsmStateMachine>>,
    state_query: Query<(Entity, &Name), With<HsmState>>,
) {
    // 按键到状态名的映射 / Key-to-state mapping
    let bindings = [
        (KeyCode::Numpad0, "Idle"),
        (KeyCode::Numpad1, "Walking"),
        (KeyCode::Numpad2, "Aiming"),
        (KeyCode::Numpad3, "Attacking"),
    ];

    for (key, state_name) in bindings {
        if input.just_pressed(key) {
            // 按名称查找目标状态实体
            // Find target state entity by name
            let Some((target, _)) = state_query
                .iter()
                .find(|(_, name)| name.as_str() == state_name)
            else {
                continue;
            };

            info!("Chain transition to {}", state_name);
            // Chain 自动计算 LCA 路径并执行转换
            // Chain automatically computes LCA path and executes transition
            commands.trigger(HsmTrigger::chain(hsm.entity(), target));
        }
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
