use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn debug_on_state(info: &str) -> impl Fn(In<HsmStateContext>, Query<&Name, With<HsmState>>) {
    move |context: In<HsmStateContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state).unwrap();
        println!("[{}]{}: {}", context.state, state_name, info);
    }
}

fn debug_light(
    states: In<Vec<HsmStateContext>>,
    query: Query<&Name>,
) -> Option<Vec<HsmStateContext>> {
    for light in query.iter_many(states.0.iter().map(|a| a.state)) {
        println!("Current light: {}", light);
    }
    None
}

#[derive(Component, Default)]
struct LightTimer(Timer);

impl LightTimer {
    fn light_timer(
        entity: In<HsmStateContext>,
        time: Res<Time<Fixed>>,
        mut query: Query<&mut LightTimer>,
    ) -> bool {
        let mut timer = query.get_mut(entity.service_target).unwrap();
        timer.0.tick(time.delta());
        timer.0.is_finished()
    }
}

fn register_condition(
    mut commands: Commands,
    mut action_systems: ResMut<StateConditions>,
    mut on_enter_disposable_systems: ResMut<HsmOnEnterDisposableSystems>,
    mut on_exit_disposable_systems: ResMut<HsmOnExitDisposableSystems>,
) {
    let id = commands.register_system(LightTimer::light_timer);
    action_systems.insert("light_timer", id);

    let id = commands.register_system(debug_on_state("Entering state."));
    on_enter_disposable_systems.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("Exiting state."));
    on_exit_disposable_systems.insert("debug_on_exit", id);
}

fn setup(mut commands: Commands) {
    let start_state_id = commands.spawn_empty().id();
    let state_machines = commands.spawn(StateMachines::new(10, start_state_id)).id();

    commands.entity(start_state_id).insert((
        Name::new("red"),
        HsmState::new(state_machines),
        StateTransitionStrategy::Nested(false),
        HsmOnUpdateSystem::new("Update:debug_light"),
        HsmOnEnterSystem::new("debug_on_enter"),
        HsmOnExitSystem::new("debug_on_exit"),
    ));

    commands.spawn((
        SuperState(start_state_id),
        Name::new("yellow"),
        HsmState::new(state_machines),
        HsmOnUpdateSystem::new("Update:debug_light"),
        HsmOnEnterSystem::new("debug_on_enter"),
        HsmOnExitSystem::new("debug_on_exit"),
        HsmOnEnterCondition::new("light_timer"),
        HsmOnExitCondition::new("light_timer"),
    ));

    println!("State Machines: {:?}", state_machines);

    commands.entity(state_machines).insert((
        Name::new("Blinking Light Paused"),
        HsmOnState::default(),
        LightTimer(Timer::from_seconds(1.0, TimerMode::Repeating)),
    ));
}

fn blinking_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    state_machines: Single<Entity, With<StateMachines>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        let mut entity = commands.entity(state_machines.entity());
        entity.queue(|mut entity: EntityWorldMut<'_>| {
            match entity.get::<StationaryStateMachines>().is_some() {
                true => {
                    info!("Resuming blinking light");
                    entity.remove::<StationaryStateMachines>();
                }
                false => {
                    info!("Pausing blinking light");
                    entity.insert(StationaryStateMachines);
                }
            };
        });
    }
}

///
/// # 状态机示例\State Machine Example
///
/// 本示例演示了如何使用 bevy_hsm 库创建一个具有暂停功能的状态机
///
/// This example demonstrates how to use the bevy_hsm library to create a state machine with a pause feature.
///
/// ## 实体说明
/// * [LightTimer] - 计时器组件，用于控制灯的闪烁
/// - [LightTimer] - Timer component used to control the blinking
/// * [StateMachines] - 状态机组件，管理当前状态和状态转换
/// - [StateMachines] - State machine component, managing the current state and state transitions
/// * [StationaryStateMachines] - 状态机组件，用于暂停状态机
/// - [StationaryStateMachines] - State machine component used to pause the state machine
///
/// ## 状态转换\State Transitions
/// [red] <-> [yellow] - 通过计时器来在两个状态间转换
/// [red] <-> [yellow] - Transition between states through the timer
///
/// ## 状态机暂停\State Machine Pause
/// 通过切换按键空格来暂停和恢复状态机
///
/// Toggle the space key to pause and resume the state machine
///
fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(HsmPlugin::default());

    app.add_action_system(Update, "debug_light", debug_light);

    app.add_systems(Startup, (register_condition, setup).chain());
    app.add_systems(Update, blinking_pause);

    app.run();
}
