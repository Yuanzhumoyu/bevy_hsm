//! # Hello HSM — 分层状态机入门 / Getting Started with Hierarchical State Machines
//!
//! 本示例演示 bevy_hsm 最核心的概念：
//! - **状态树 (StateTree)**：定义状态的父子层级关系
//! - **状态生命周期 (StateLifecycle)**：Enter → Update → Exit
//! - **状态转换 (Transition)**：ToSub / ToSuper 在父子状态间导航
//! - **动作系统 (Action System)**：在生命周期各阶段执行自定义逻辑
//!
//! This example demonstrates the core concepts of bevy_hsm:
//! - **State tree**: defines parent-child hierarchy of states
//! - **State lifecycle**: Enter → Update → Exit
//! - **Transitions**: ToSub / ToSuper for navigating between parent and child states
//! - **Action systems**: custom logic executed at each lifecycle stage
//!
//! ## 状态结构 / State Structure
//! ```text
//! Root (Nested + Rebirth)
//!  └── Child (Nested + Rebirth)
//! ```
//!
//! ## 操作 / Controls
//! - **空格 / Space**: 在 Root ↔ Child 之间切换 / Toggle between Root and Child
//! - **Esc**: 退出 / Exit

use bevy::prelude::*;
use bevy_hsm::prelude::*;

// ── 动作系统 / Action Systems ─────────────────────────────────────

/// 进入状态时打印日志 / Log when entering a state
fn log_enter(In(ctx): In<ActionContext>, query: Query<&Name>) {
    if let Ok(name) = query.get(ctx.state()) {
        info!(">>> Enter  [{}]", name);
    }
}

/// 退出状态时打印日志 / Log when exiting a state
fn log_exit(In(ctx): In<ActionContext>, query: Query<&Name>) {
    if let Ok(name) = query.get(ctx.state()) {
        info!("<<< Exit   [{}]", name);
    }
}

/// 更新状态时打印日志 / Log when updating a state
fn log_update(contexts: In<Vec<ActionContext>>, query: Query<&Name>) -> Option<Vec<ActionContext>> {
    for ctx in contexts.iter() {
        if let Ok(name) = query.get(ctx.state()) {
            info!("--- Update [{}]", name);
        }
    }
    Some(contexts.0)
}

// ── 资源 / Resource ───────────────────────────────────────────────

/// 存储子状态实体 ID, 供输入系统使用
/// Stores the child state Entity for use in the input system
#[derive(Resource)]
struct ChildState(Entity);

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    // 1. 注册动作系统 — 给每个系统一个名字, 状态通过名字引用它们
    //    Register action systems — each gets a name, states reference them by name
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);

    // 2. 创建状态实体
    //    Create state entities
    //
    //    HsmState::with(strategy, behavior):
    //    - Nested:  进入子状态时, 父状态退出
    //               Parent exits when entering child state
    //    - Rebirth: 重新进入该状态时重置
    //               Reset when re-entering this state
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

    let child = commands
        .spawn((
            Name::new("Child"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    // 3. 构建状态树: Root → Child
    //    Build state tree: Root → Child
    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    // 4. 生成状态机 — 将树和初始状态绑定到状态机实体上
    //    Spawn the state machine — bind tree and initial state to SM entity
    //
    //    HsmStateMachine::with(holder, init_state, history_len):
    //    - holder:     持有 StateTree 组件的实体 (SM 自身)
    //                  Entity that holds the StateTree component (the SM itself)
    //    - init_state: 启动时进入的初始状态
    //                  The initial state to enter on boot
    //
    //    StateLifecycle::default() (= Enter) 触发 on_insert 钩子,
    //    启动生命周期: Enter → Update → ...
    //    StateLifecycle::default() (= Enter) triggers the on_insert hook,
    //    starting the lifecycle: Enter → Update → ...
    let sm = commands.spawn_empty().id();
    commands.entity(sm).insert((
        tree,
        Name::new("HelloHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    commands.insert_resource(ChildState(child));
}

// ── 输入处理 / Input Handling ─────────────────────────────────────

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    child_state: Res<ChildState>,
    sm: Single<Entity, With<HsmStateMachine>>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::Space) {
        return;
    }

    let sm_entity = *sm;
    let child = child_state.0; // 提前取出 Entity, 避免闭包捕获 Res
    commands.queue(move |world: &mut World| {
        let Some(sm_comp) = world.get::<HsmStateMachine>(sm_entity) else {
            return;
        };

        let tree_id = sm_comp.state_graph_id();
        let curr = sm_comp.curr_state_id();
        let Some(tree) = world.get::<StateTree>(tree_id) else {
            return;
        };

        if curr == tree.get_root() {
            info!("Space: Root → Child (ToSub)");
            world
                .entity_mut(sm_entity)
                .trigger(|id| HsmTrigger::to_sub(id, child));
        } else {
            info!("Space: Child → Root (ToSuper)");
            world.entity_mut(sm_entity).trigger(HsmTrigger::to_super);
        }
    });
}

// ── 程序入口 / Entry Point ────────────────────────────────────────

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    // 注册 Update 动作系统 — OnUpdateSystem("Update:log_update") 引用它
    // Register the update action system — referenced by OnUpdateSystem("Update:log_update")
    app.add_action_system(Update, "log_update", log_update);

    app.add_systems(Startup, setup);
    app.add_systems(Update, handle_input);

    app.run();
}
