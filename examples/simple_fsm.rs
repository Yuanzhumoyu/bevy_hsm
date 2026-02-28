use bevy::prelude::*;
use bevy_hsm::{StateMachinePlugin, prelude::*};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct ToggleEvent;

fn log_on_enter(In(context): In<StateActionContext>, query: Query<&Name>) {
    if let Ok(name) = query.get(context.state()) {
        info!("Entering state: {}", name);
    }
}

fn log_on_exit(In(context): In<StateActionContext>, query: Query<&Name>) {
    if let Ok(name) = query.get(context.state()) {
        info!("Exiting state: {}", name);
    }
}

fn setup_fsm(mut commands: Commands, mut action_registry: ResMut<StateActionRegistry>) {
    let system_id = commands.register_system(log_on_enter);
    action_registry.insert("log_on_enter", system_id);
    let system_id = commands.register_system(log_on_exit);
    action_registry.insert("log_on_exit", system_id);

    let state_a = commands
        .spawn((
            FsmState,
            Name::new("State A"),
            OnEnterSystem::new("log_on_enter"),
            OnExitSystem::new("log_on_exit"),
        ))
        .id();

    let state_b = commands
        .spawn((
            FsmState,
            Name::new("State B"),
            OnEnterSystem::new("log_on_enter"),
            OnExitSystem::new("log_on_exit"),
        ))
        .id();

    let mut graph = FsmGraph::new(state_a);
    graph
        .add_event(state_a, ToggleEvent, state_b)
        .add_event(state_b, ToggleEvent, state_a);

    let graph_id = commands.spawn(graph).id();

    commands.spawn((
        FsmStateMachine::new(graph_id, state_a, 10),
        Name::new("MySimpleFsm"),
    ));
}

fn handle_input(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    state_machine: Single<Entity, With<FsmStateMachine>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        info!("Spacebar pressed, sending ToggleEvent.");
        let state_machine = state_machine.entity();
        commands.trigger(FsmTrigger::with_event(state_machine, ToggleEvent));
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::<Last>::default())
        .add_systems(Startup, setup_fsm)
        .add_systems(Update, handle_input)
        .run();
}
