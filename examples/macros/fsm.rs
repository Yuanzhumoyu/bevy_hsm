use bevy::prelude::*;
use bevy_hsm::prelude::*;

use bevy_hsm_macros::fsm;

#[derive(Component)]
pub struct ComponentA;

#[derive(Component)]
pub struct ComponentB;

#[derive(Component)]
pub struct ComponentC;

#[derive(Component)]
pub struct ComponentD;

#[derive(PartialEq, Clone, Eq, Hash, Debug)]
pub enum MyEvent {
    Go,
    Back,
}

fn debug_on_state(info: &str) -> impl Fn(In<StateActionContext>, Query<&Name, With<FsmState>>) {
    move |context: In<StateActionContext>, query: Query<&Name, With<FsmState>>| {
        let binding = Name::new("state");
        let state_name = query.get(context.state()).unwrap_or(&binding);
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

fn setup(mut commands: Commands, mut action_registry: ResMut<StateActionRegistry>) {
    let id = commands.register_system(debug_on_state("Enter"));
    action_registry.insert("on_enter_name", id);
    let id = commands.register_system(debug_on_state("Exit"));
    action_registry.insert("on_exit_name", id);

    commands.spawn(fsm!(
        states:{
            #[state_data(ComponentA,ComponentB)]
            #[state(on_enter="on_enter_name", on_exit="on_exit_name")]:state(
                ComponentC,
                ComponentD,
            ),
            #[state(on_enter="on_enter_name", on_exit="on_exit_name")]ComponentA,
            #[state(on_enter="on_enter_name", on_exit="on_exit_name")]
            #[state_data(ComponentC)]
            (ComponentA,ComponentB),
            #[state(minimal)]:MinimalState
        },
        transitions:{
            state => MyEvent::Go => 1,
            1 => MyEvent::Go => 2,
            2 => MyEvent::Go => MinimalState,
            MinimalState => MyEvent::Back => 2,
            2 => MyEvent::Back => 1,
            1 => MyEvent::Back => 0,
        },
        components:{
            ComponentA,
            ComponentB,
        }
    ));
}

fn handle_input(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    state_machine: Single<Entity, With<FsmStateMachine>>,
) {
    if keyboard_input.just_pressed(KeyCode::ArrowUp) {
        print!("[Input]Go");
        commands.trigger(FsmTrigger::with_event(state_machine.entity(), MyEvent::Go));
    }
    if keyboard_input.just_pressed(KeyCode::ArrowDown) {
        print!("[Input]Back");
        commands.trigger(FsmTrigger::with_event(
            state_machine.entity(),
            MyEvent::Back,
        ));
    }
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::<Last>::default());

    app.add_systems(Startup, setup);

    app.add_systems(Update, handle_input);

    app.run();
}
