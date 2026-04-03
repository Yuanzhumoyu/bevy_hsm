use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_hsm::prelude::*;

fn debug_on_state(info: &str) -> impl Fn(In<ActionContext>, Query<&Name, With<HsmState>>) {
    move |context: In<ActionContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state()).unwrap();
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

fn is_up(_: In<GuardContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowUp)
}

fn is_down(_: In<GuardContext>, input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::ArrowDown)
}

#[derive(Component, Clone)]
#[component(on_insert = Self::on_insert, on_remove = Self::on_remove)]
struct StateAData;

impl StateAData {
    fn on_insert(_world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        println!("StateAData inserted for state {:?}", entity);
    }

    fn on_remove(_world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        println!("StateAData removed for state {:?}", entity);
    }
}

fn check_state_a_data(query: Query<(), (With<StateAData>, With<HsmStateMachine>)>) {
    if !query.is_empty() {
        println!("--> Found StateAData component!");
    }
}

fn setup(
    mut commands: Commands,
    mut guard_registry: ResMut<GuardRegistry>,
    mut action_registry: ResMut<ActionRegistry>,
) {
    let id = commands.register_system(is_up);
    guard_registry.insert("is_up", id);
    let id = commands.register_system(is_down);
    guard_registry.insert("is_down", id);

    let id = commands.register_system(debug_on_state("Enter"));
    action_registry.insert("on_enter_name", id);
    let id = commands.register_system(debug_on_state("Exit"));
    action_registry.insert("on_exit_name", id);

    commands.spawn(hsm!(
        StateLifecycle::default(),
        Name::new("MyHSM"),
        #[state(after_enter="on_enter_name", before_exit="on_exit_name",behavior=Rebirth)]: Root(
            #[state_data(StateAData)]
            #[state(guard_enter="is_up", guard_exit="is_down", after_enter="on_enter_name", before_exit="on_exit_name")]: StateA(
                #[state(guard_enter="is_up", guard_exit="is_down", after_enter="on_enter_name", before_exit="on_exit_name")]: StateB
            )
        )
    ));
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_systems(Startup, setup);
    app.add_systems(Update, check_state_a_data);

    app.run();
}
