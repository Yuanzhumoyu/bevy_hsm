use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn debug_on_state(info: &str) -> impl Fn(In<ActionContext>, Query<&Name, With<HsmState>>) {
    move |context: In<ActionContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state()).unwrap();
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

fn is_up(_entity: In<GuardContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowUp)
}

fn is_down(_entity: In<GuardContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowDown)
}

fn register_condition(
    mut commands: Commands,
    mut guard_registry: ResMut<GuardRegistry>,
    mut action_registry: ResMut<ActionRegistry>,
) {
    let id = commands.register_system(is_up);
    guard_registry.insert("is_up", id);
    let id = commands.register_system(is_down);
    guard_registry.insert("is_down", id);

    let id = commands.register_system(debug_on_state("进入状态"));
    action_registry.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("退出状态"));
    action_registry.insert("debug_on_exit", id);
}

fn setup(mut commands: Commands) {
    let start_id = commands
        .spawn((
            Name::new("OFF"),
            HsmState::default(),
            OnEnterSystem::new("debug_on_enter"),
            OnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id1 = commands
        .spawn((
            Name::new("ON1"),
            HsmState::default(),
            EnterGuard::new("is_up"),
            ExitGuard::new("is_down"),
            OnEnterSystem::new("debug_on_enter"),
            OnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id2 = commands
        .spawn((
            Name::new("ON2"),
            HsmState::default(),
            EnterGuard::new("is_up"),
            ExitGuard::new("is_down"),
            OnEnterSystem::new("debug_on_enter"),
            OnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id3 = commands
        .spawn((
            Name::new("ON3"),
            HsmState::default(),
            EnterGuard::new("is_up"),
            ExitGuard::new("is_down"),
            OnEnterSystem::new("debug_on_enter"),
            OnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let traversal = TraversalStrategy::default();
    let mut state_tree = StateTree::new(start_id);
    state_tree
        .with_traversal(start_id, traversal.clone())
        .with_child(start_id, id1)
        .with_traversal(id1, traversal.clone())
        .with_child(id1, id2)
        .with_traversal(id2, traversal)
        .with_child(id2, id3);

    let state_machine = commands.spawn_empty().id();
    commands.entity(state_machine).insert((
        state_tree,
        HsmStateMachine::with(
            HsmStateId::new(state_machine, start_id),
            #[cfg(feature = "history")]
            10,
        ),
        Name::new("More States"),
        StateLifecycle::default(),
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
        .add_plugins(StateMachinePlugin::default());

    app.add_systems(Startup, (register_condition, setup).chain());

    app.run();
}
