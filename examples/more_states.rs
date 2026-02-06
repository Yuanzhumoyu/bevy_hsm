use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn debug_on_state(info: &str) -> impl Fn(In<HsmStateContext>, Query<&Name, With<HsmState>>) {
    move |context: In<HsmStateContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state()).unwrap();
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

fn is_up(_entity: In<HsmStateConditionContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowUp)
}

fn is_down(_entity: In<HsmStateConditionContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowDown)
}

fn register_condition(
    mut commands: Commands,
    mut action_systems: ResMut<StateConditions>,
    mut disposable_systems: ResMut<HsmOnStateDisposableSystems>,
) {
    let id = commands.register_system(is_up);
    action_systems.insert("is_up", id);
    let id = commands.register_system(is_down);
    action_systems.insert("is_down", id);

    let id = commands.register_system(debug_on_state("进入状态"));
    disposable_systems.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("退出状态"));
    disposable_systems.insert("debug_on_exit", id);
}

fn setup(mut commands: Commands) {
    let start_id = commands
        .spawn((
            Name::new("OFF"),
            HsmState::default(),
            HsmOnEnterSystem::new("debug_on_enter"),
            HsmOnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id1 = commands
        .spawn((
            Name::new("ON1"),
            HsmState::default(),
            HsmOnEnterCondition::new("is_up"),
            HsmOnExitCondition::new("is_down"),
            HsmOnEnterSystem::new("debug_on_enter"),
            HsmOnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id2 = commands
        .spawn((
            Name::new("ON2"),
            HsmState::default(),
            HsmOnEnterCondition::new("is_up"),
            HsmOnExitCondition::new("is_down"),
            HsmOnEnterSystem::new("debug_on_enter"),
            HsmOnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id3 = commands
        .spawn((
            Name::new("ON3"),
            HsmState::default(),
            HsmOnEnterCondition::new("is_up"),
            HsmOnExitCondition::new("is_down"),
            HsmOnEnterSystem::new("debug_on_enter"),
            HsmOnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let traversal = TraversalStrategy::default();
    let state_tree = StateTree::new(start_id, traversal.clone())
        .with_add(start_id, id1, traversal.clone())
        .with_add(id1, id2, traversal.clone())
        .with_add(id2, id3, traversal.clone());

    let state_machine = commands.spawn_empty().id();
    commands.entity(state_machine).insert((
        state_tree,
        StateMachine::new(10, TreeStateId::new(state_machine, start_id)),
        Name::new("More States"),
        HsmOnState::default(),
    ));
}

/// # 流程图\Flowchart
///    [`OFF`]
///
///  is_up↓↑is_dowm
///
///    [`ON1`]
///
///  is_up↓↑is_down
///
///    [`ON2`]
///
///  is_up↓↑is_down
///
///    [`ON3`]
///    
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(HsmPlugin::default());

    app.add_systems(Startup, (register_condition, setup).chain());

    app.run();
}
