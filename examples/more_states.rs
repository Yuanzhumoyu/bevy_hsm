use bevy::prelude::*;
use bevy_hsm::prelude::*;

fn debug_on_state(info: &str) -> impl Fn(In<HsmStateContext>, Query<&Name, With<HsmState>>) {
    move |context: In<HsmStateContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state).unwrap();
        println!("[{}]{}: {}", context.state, state_name, info);
    }
}

fn is_up(_entity: In<HsmStateContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowUp)
}

fn is_down(_entity: In<HsmStateContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowDown)
}

fn register_condition(
    mut commands: Commands,
    mut action_systems: ResMut<StateConditions>,
    mut on_enter_disposable_systems: ResMut<HsmOnEnterDisposableSystems>,
    mut on_exit_disposable_systems: ResMut<HsmOnExitDisposableSystems>,
) {
    let id = commands.register_system(is_up);
    action_systems.insert("is_up", id);
    let id = commands.register_system(is_down);
    action_systems.insert("is_down", id);

    let id = commands.register_system(debug_on_state("进入状态"));
    on_enter_disposable_systems.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("退出状态"));
    on_exit_disposable_systems.insert("debug_on_exit", id);
}

fn setup(mut commands: Commands) {
    let start_state_id = commands.spawn_empty().id();
    let state_machines = commands.spawn(StateMachines::new(10, start_state_id)).id();

    commands.entity(start_state_id).insert((
        Name::new("OFF"),
        HsmState::new(state_machines),
        HsmOnEnterSystem::new("debug_on_enter"),
        HsmOnExitSystem::new("debug_on_exit"),
    ));

    let id = commands
        .spawn((
            SuperState(start_state_id),
            Name::new("ON1"),
            HsmState::new(state_machines),
            HsmOnEnterCondition::new("is_up"),
            HsmOnExitCondition::new("is_down"),
            HsmOnEnterSystem::new("debug_on_enter"),
            HsmOnExitSystem::new("debug_on_exit"),
        ))
        .id();

    let id = commands
        .spawn((
            SuperState(id),
            Name::new("ON2"),
            HsmState::new(state_machines),
            HsmOnEnterCondition::new("is_up"),
            HsmOnExitCondition::new("is_down"),
            HsmOnEnterSystem::new("debug_on_enter"),
            HsmOnExitSystem::new("debug_on_exit"),
        ))
        .id();

    commands.spawn((
        SuperState(id),
        Name::new("ON3"),
        HsmState::new(state_machines),
        HsmOnEnterCondition::new("is_up"),
        HsmOnExitCondition::new("is_down"),
        HsmOnEnterSystem::new("debug_on_enter"),
        HsmOnExitSystem::new("debug_on_exit"),
    ));

    commands
        .entity(state_machines)
        .insert((Name::new("More States"), HsmOnState::default()));
}

/// # 流程图
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
