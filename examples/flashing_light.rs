//! # 闪烁灯 / Flashing Light
//!
//! 本示例演示基于计时器的守卫自动状态切换：
//! - **定时守卫 (Timer Guard)**: 守卫系统内部使用计时器, 时间到返回 true 触发自动转换
//! - **双向循环**: Red ↔ Yellow 通过计时器守卫形成自动循环
//! - **暂停/恢复**: 使用 Paused 标记暂停和恢复状态机
//!
//! This example demonstrates timer-based automatic state switching:
//! - **Timer guard**: guard system uses an internal timer, returns true when elapsed
//! - **Bidirectional loop**: Red ↔ Yellow auto-cycle via timer guards
//! - **Pause/Resume**: use Paused marker to pause and resume the state machine
//!
//! ## 状态结构 / State Structure
//! ```text
//! Red (Parallel + Rebirth)
//!  └── Yellow (Nested, GuardEnter="light_timer", GuardExit="light_timer")
//! ```
//!
//! ## 操作 / Controls
//! - **空格 / Space**: 暂停/恢复闪烁
//!
//! ## 工作流程 / Workflow
//! 1. 启动在 Red, GuardEnter 条件未满足, 停留在 Red
//! 2. 1 秒后计时器触发 → GuardEnter 放行 → 自动进入 Yellow
//! 3. Yellow 的 GuardExit 在 1 秒后触发 → 自动退出回 Red
//! 4. 循环往复, 形成红-黄交替闪烁

use bevy::prelude::*;
use bevy_hsm::prelude::*;

// ── 计时器守卫 / Timer Guard ──────────────────────────────────────

#[derive(Component, Default)]
struct LightTimer(Timer);

impl LightTimer {
    fn guard_on_timer(
        In(ctx): In<GuardContext>,
        time: Res<Time<Fixed>>,
        mut query: Query<&mut LightTimer>,
    ) -> bool {
        let mut timer = query.get_mut(ctx.service_target).unwrap();
        timer.0.tick(time.delta());
        timer.0.just_finished()
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

fn log_light(states: In<Vec<ActionContext>>, query: Query<&Name>) -> Option<Vec<ActionContext>> {
    for name in query.iter_many(states.iter().map(|c| c.state())) {
        info!("~~~ Light: {}", name);
    }
    // Some(states.0)
    None
}

// ── 初始化 / Startup ──────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    mut guard_registry: ResMut<GuardRegistry>,
    mut action_registry: ResMut<ActionRegistry>,
) {
    guard_registry.extend([(
        "light_timer",
        commands.register_system(LightTimer::guard_on_timer),
    )]);
    action_registry.extend([
        ("log_enter", commands.register_system(log_enter)),
        ("log_exit", commands.register_system(log_exit)),
    ]);

    let red = commands
        .spawn((
            Name::new("Red"),
            HsmState::with(
                StateTransitionStrategy::Parallel,
                ExitTransitionBehavior::Rebirth,
            ),
            OnUpdateSystem::new("Update:log_light"),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    // Yellow 同时有 GuardEnter 和 GuardExit 使用同一个计时器守卫
    // Yellow has both GuardEnter and GuardExit using the same timer guard
    let yellow = commands
        .spawn((
            Name::new("Yellow"),
            HsmState::default(),
            OnUpdateSystem::new("Update:log_light"),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            GuardEnter::new("light_timer"),
            GuardExit::new("light_timer"),
        ))
        .id();

    let traversal = TraversalStrategy::default();
    let mut state_tree = StateTree::new(red);
    state_tree
        .with_traversal(red, traversal)
        .with_child(red, yellow);

    let sm = commands.spawn_empty().id();
    commands.entity(sm).insert((
        state_tree,
        Name::new("FlashingLight"),
        HsmStateMachine::with(
            sm,
            red,
            #[cfg(feature = "history")]
            10,
        ),
        StateLifecycle::default(),
        LightTimer(Timer::from_seconds(1.0, TimerMode::Repeating)),
    ));
}

// ── 输入处理: 暂停/恢复 / Input: Pause/Resume ────────────────────

fn handle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    sm: Single<Entity, With<HsmStateMachine>>,
) {
    if !keyboard.just_pressed(KeyCode::Space) {
        return;
    }

    let sm_entity = *sm;
    commands.queue(move |world: &mut World| {
        let mut entity = world.entity_mut(sm_entity);
        if entity.contains::<Paused>() {
            info!("Resuming flashing light");
            entity.remove::<Paused>();
        } else {
            info!("Pausing flashing light");
            entity.insert(Paused);
        }
    });
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_action_system(Update, "log_light", log_light);

    app.add_systems(Startup, setup);
    app.add_systems(Update, handle_pause);

    app.run();
}
