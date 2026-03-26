use bevy::prelude::*;
use bevy_hsm::prelude::*;

use bevy_hsm_macros::hsm;

#[derive(Component)]
pub struct ComponentA;

#[derive(Component)]
pub struct ComponentB;

#[derive(Component)]
pub struct ComponentC;

#[derive(Component)]
pub struct ComponentD;

fn debug_on_state(info: &str) -> impl Fn(In<ActionContext>, Query<&Name, With<HsmState>>) {
    move |context: In<ActionContext>, query: Query<&Name, With<HsmState>>| {
        let state_name = query.get(context.state()).unwrap();
        println!("[{}]{}: {}", context.state(), state_name, info);
    }
}

fn a(_context: In<GuardContext>) -> bool {
    false
}

fn b(_context: In<GuardContext>) -> bool {
    false
}

fn setup(
    mut commands: Commands,
    mut guard_registry: ResMut<GuardRegistry>,
    mut action_registry: ResMut<ActionRegistry>,
) {
    let id = commands.register_system(a);
    guard_registry.insert("a", id);
    let id = commands.register_system(b);
    guard_registry.insert("b", id);

    let id = commands.register_system(debug_on_state("Enter"));
    action_registry.insert("on_enter_name", id);
    let id = commands.register_system(debug_on_state("Exit"));
    action_registry.insert("on_exit_name", id);

    commands.spawn(hsm!(
        ComponentA,
        ComponentB,
        StateLifecycle::default(),
        #[state_data(ComponentA,ComponentB)]
        #[state(after_enter="on_enter_name", before_exit="on_exit_name")]:state
        (
            ComponentC,
            #[state] ComponentA,
            #[state(strategy=Nested)]
            (ComponentA,ComponentB),
            #[state]:A
            ComponentB,
            #[state_data(ComponentA,ComponentB)]
            #[state]:C
            (
                #[state]
                ComponentC,
                ComponentD,
            ),
            #[state(minimal)]:MinimalState,
            ComponentD,
        ),
    ));
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(StateMachinePlugin::default());

    app.add_systems(Startup, setup);

    app.run();
}
