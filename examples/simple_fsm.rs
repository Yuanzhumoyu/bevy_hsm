use bevy::prelude::*;
use bevy_hsm::{StateMachinePlugin, prelude::*};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct ToggleEvent;

fn log_on_enter(In(context): In<ActionContext>, query: Query<&Name>) {
    if let Ok(name) = query.get(context.state()) {
        info!("Entering state: {}", name);
    }
}

fn log_on_exit(In(context): In<ActionContext>, query: Query<&Name>) {
    if let Ok(name) = query.get(context.state()) {
        info!("Exiting state: {}", name);
    }
}

fn log_on_update(
    In(contexts): In<Vec<ActionContext>>,
    query: Query<&Name>,
) -> Option<Vec<ActionContext>> {
    let iter = query.iter_many(contexts.iter().map(|c| c.state()));
    for name in iter {
        info!("Updating state: {}", name);
    }
    Some(contexts)
}

fn setup_fsm(mut commands: Commands, mut action_registry: ResMut<ActionRegistry>) {
    let system_id = commands.register_system(log_on_enter);
    action_registry.insert("log_on_enter", system_id);
    let system_id = commands.register_system(log_on_exit);
    action_registry.insert("log_on_exit", system_id);

    let state_a = commands
        .spawn((
            FsmState,
            Name::new("State A"),
            OnEnterSystem::new("log_on_enter"),
            OnUpdateSystem::new("Update:log_on_update"),
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
        .with_event(state_a, ToggleEvent, state_b)
        .with_event(state_b, ToggleEvent, state_a);

    let graph_id = commands.spawn(graph).id();

    commands.spawn((
        FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ),
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

    if keyboard_input.just_pressed(KeyCode::KeyP) {
        let mut entity = commands.entity(state_machine.entity());
        entity.queue(|mut entity: EntityWorldMut<'_>| {
            match entity.get::<Paused>().is_some() {
                true => {
                    info!("Resuming blinking light");
                    entity.remove::<Paused>();
                }
                false => {
                    info!("Pausing blinking light");
                    entity.insert(Paused);
                }
            };
        });
    }
}

/// 状态机示例\State Machine Example
/// 本示例演示了如何使用状态机插件来创建一个简单的状态机，该状态机在两个状态之间切换。
/// 当按下空格键时，状态机将发送ToggleEvent事件，导致当前状态切换到另一个状态。
/// 通过切换按键空格来切换状态\
/// 通过切换按键P来暂停和恢复状态机
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_action_system(Update, "log_on_update", log_on_update);

    app.add_systems(Startup, setup_fsm)
        .add_systems(Update, handle_input);

    app.run();
}
