use bevy::prelude::*;
use bevy_hsm::{hsm::event::HsmTrigger, prelude::*};

fn debug_on_state(info: &str) -> impl Fn(In<StateActionContext>, Query<&Name, With<HsmState>>) {
    move |context: In<StateActionContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state()).unwrap();
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

//
// Root (根状态)
// ├── Movement (移动分支)
// │   ├── Idle (待机)
// │   └── Walking (行走)
// └── Combat (战斗分支)
//     ├── Aiming (瞄准)
//     └── Attacking (攻击)
//
fn setup(mut commands: Commands, mut action_registry: ResMut<StateActionRegistry>) {
    let id = commands.register_system(debug_on_state("enter"));
    action_registry.insert("debug_on_enter", id);
    let id = commands.register_system(debug_on_state("exit"));
    action_registry.insert("debug_on_exit", id);

    fn trigger_event(mut entity_commands: EntityCommands, states: &[Entity]) {
        let id = entity_commands.id();
        entity_commands
            .commands()
            .trigger(HsmTrigger::with_next(id, states[3]));
    }

    commands.spawn(hsm! {
       #[state(on_enter = "debug_on_enter",on_exit = "debug_on_exit")]:Root(
            #[state(on_enter = "debug_on_enter",on_exit = "debug_on_exit",behavior = Death)]:Movement(
                #[state(on_enter = "debug_on_enter",on_exit = "debug_on_exit")]:Idle,
                #[state(on_enter = "debug_on_enter",on_exit = "debug_on_exit")]:Walking,
            ),
            #[state(on_enter = "debug_on_enter",on_exit = "debug_on_exit",behavior = Death)]:Combat(
                #[state(on_enter = "debug_on_enter",on_exit = "debug_on_exit")]:Aiming,
                #[state(on_enter = "debug_on_enter",on_exit = "debug_on_exit")]:Attacking,
            )
       )
       StateLifecycle::default()
       :trigger_event,
    });
}

fn player_input_system(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    hsm: Single<Entity, With<HsmStateMachine>>,
    query_state: Query<(Entity, &Name), With<HsmState>>,
) {
    let get_state_id = |name: &str| -> Option<Entity> {
        query_state
            .iter()
            .find_map(|(id, named)| (named.as_str() == name).then(|| id))
    };

    [
        (KeyCode::Numpad0, "Idle"),
        (KeyCode::Numpad1, "Walking"),
        (KeyCode::Numpad2, "Aiming"),
        (KeyCode::Numpad3, "Attacking"),
    ]
    .iter()
    .for_each(|(key, state_name)| {
        if input.just_pressed(*key) {
            println!("Switching to {}", state_name);
            commands.trigger(HsmTrigger::with_next(
                hsm.entity(),
                get_state_id(*state_name).unwrap(),
            ));
        }
    });
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_systems(Startup, setup);

    app.add_systems(Update, player_input_system);

    app.run();
}
