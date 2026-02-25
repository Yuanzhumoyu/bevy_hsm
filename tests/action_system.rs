use bevy::prelude::*;
use bevy_hsm::prelude::*;

#[derive(Component, Debug, Clone, Copy)]
enum Switch {
    Open,
    Close,
}

fn debug_on_state(info: &str) -> impl Fn(In<OnStateContext>, Query<&Name, With<HsmState>>) {
    move |context: In<OnStateContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state()).unwrap();
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

fn debug_hello_world(
    contexts: In<Vec<OnStateContext>>,
    mut query_switch: Query<&mut Switch>,
) -> Option<Vec<OnStateContext>> {
    let mut switch = query_switch.get_mut(contexts.0[0].service_target).unwrap();
    println!("Hello World {:?}", switch.as_ref());
    *switch = match *switch {
        Switch::Open => Switch::Close,
        Switch::Close => Switch::Open,
    };
    Some(contexts.0)
}

fn is_open(entity: In<OnStateConditionContext>, query: Query<&Switch>) -> bool {
    let switch = query.get(entity.service_target).unwrap();
    matches!(switch, Switch::Open)
}

fn is_close(entity: In<OnStateConditionContext>, query: Query<&Switch>) -> bool {
    let switch = query.get(entity.service_target).unwrap();
    matches!(switch, Switch::Close)
}

fn register_condition(
    mut commands: Commands,
    mut action_systems: ResMut<StateConditions>,
    mut named_state_systems: ResMut<NamedStateSystems>,
) {
    let id = commands.register_system(is_open);
    action_systems.insert("is_open", id);
    let id = commands.register_system(is_close);
    action_systems.insert("is_close", id);

    let id = commands.register_system(debug_on_state("Entering state."));
    named_state_systems.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("Exiting state."));
    named_state_systems.insert("debug_on_exit", id);
}

fn setup(mut commands: Commands) {
    let start_id = commands
        .spawn((
            Name::new("Start"),
            HsmState::default(),
            OnUpdateSystem::new("Update:debug_hello_world"),
            OnEnterSystem::new("debug_on_enter"),
            OnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id = commands
        .spawn((
            Name::new("Counter"),
            HsmState::default(),
            OnUpdateSystem::new("Update:debug_hello_world"),
            OnEnterSystem::new("debug_on_enter"),
            OnExitSystem::new("debug_on_exit"),
            HsmOnEnterCondition::new("is_open"),
            HsmOnExitCondition::new("is_close"),
        ))
        .id();

    let mut state_tree = StateTree::new(start_id);
    state_tree.with_add(start_id, id);

    let state_machine = commands.spawn_empty().id();
    commands.entity(state_machine).insert((
        HsmStateMachine::new(10, TreeStateId::new(state_machine, start_id)),
        Name::new("More States"),
        HsmOnState::default(),
        state_tree,
        Switch::Open,
    ));
}

#[test]
fn remove_action_system() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(HsmPlugin::default());

    app.add_action_system(Update, "debug_hello_world", debug_hello_world);

    app.add_systems(Startup, (register_condition, setup).chain());

    app.update();
    app.update();
    app.update();
    app.update();
    app.update();
    app.update();
}
