use bevy::prelude::*;
use bevy_hsm::{prelude::*, system_registry};

#[derive(Resource, Deref)]
struct StateIds(Vec<Entity>);

fn tautology(_: In<GuardContext>) -> bool {
    true
}

fn contradiction(_: In<GuardContext>) -> bool {
    false
}

fn spawn_state_ids(mut entity_commands: EntityCommands, ids: &[Entity]) {
    entity_commands
        .commands_mut()
        .insert_resource(StateIds(ids.to_vec()));
}

fn setup() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StateMachinePlugin::default());

    let world = app.world_mut();

    world.resource_scope(
        |world: &mut World, mut guard_registry: Mut<GuardRegistry>| {
            system_registry!(<world,guard_registry>[
                "tautology"=>tautology,
                "contradiction"=>contradiction
            ]);
        },
    );

    app
}

#[test]
fn test_fsm_event() {
    let mut app = setup();
    let world = app.world_mut();

    let state_machine = world
        .spawn(fsm!(
            states:{
                #[state]: A,
                #[state]: B,
                #[state]: C,
                #[state]: D,
            },
            transitions:{
                A => B,
                B => C : event(true),
                C => D : guard("tautology"),
                C => B : guard("contradiction"),
            }
            :spawn_state_ids,
        ))
        .id();

    let ids = world.remove_resource::<StateIds>().unwrap();
    fn get_curr_state(world: &World, state_machine: Entity) -> Entity {
        world
            .get::<FsmStateMachine>(state_machine)
            .unwrap()
            .curr_state_id()
    }

    // A -> B true
    world.trigger(FsmTrigger::with_next(state_machine, ids[1]));
    assert_eq!(get_curr_state(world, state_machine), ids[1]);

    // A -> D false
    world.trigger(FsmTrigger::with_next(state_machine, ids[3]));
    assert_eq!(get_curr_state(world, state_machine), ids[1]);

    // B -> ? event=false false
    world.trigger(FsmTrigger::with_event(state_machine, EventData::new(false)));
    assert_eq!(get_curr_state(world, state_machine), ids[1]);

    // B -event(true)-> C true
    world.trigger(FsmTrigger::with_event(state_machine, EventData::new(true)));
    assert_eq!(get_curr_state(world, state_machine), ids[2]);

    // C -guard(false)-> B false
    world.trigger(FsmTrigger::with_guard(state_machine, ids[1]));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[2]);

    // C -guard(true)-> D true
    world.trigger(FsmTrigger::with_guard(state_machine, ids[3]));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[3]);
}

#[test]
fn test_hsm_event() {
    let mut app = setup();

    let world = app.world_mut();

    let state_machine = world
        .spawn(hsm!(
            #[state]:A(
                #[state]:B(
                    #[state]:D(
                        #[state]:F,
                        #[state]:G,
                    ),
                    #[state]:E,
                ),
                #[state]:C,
            )
            StateLifecycle::default(),
            :spawn_state_ids,
        ))
        .id();

    let ids = world.remove_resource::<StateIds>().unwrap();

    fn get_curr_state(world: &World, state_machine: Entity) -> Entity {
        world
            .get::<HsmStateMachine>(state_machine)
            .unwrap()
            .curr_state_id()
    }

    let tautology = GuardCondition::from("tautology");
    let contradiction = GuardCondition::from("contradiction");

    // A -> D false
    world.trigger(HsmTrigger::to_sub(state_machine, ids[3]));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[0]);

    // A -> B true
    world.trigger(HsmTrigger::to_sub(state_machine, ids[1]));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[1]);

    // B -> D guard=true true
    world.trigger(HsmTrigger::guard_sub(
        state_machine,
        tautology.clone(),
        ids[3],
    ));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[3]);

    // D -> F guard=false false
    world.trigger(HsmTrigger::guard_sub(
        state_machine,
        contradiction.clone(),
        ids[5],
    ));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[3]);

    // D -> B true
    world.trigger(HsmTrigger::to_super(state_machine));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[1]);

    // B -> A guard=false false
    world.trigger(HsmTrigger::guard_super(state_machine, contradiction));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[1]);

    // B -> A guard=true true
    world.trigger(HsmTrigger::guard_super(state_machine, tautology));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[0]);

    // A -> B -> D -> F true
    world.trigger(HsmTrigger::chain(state_machine, ids[5]));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[5]);

    // F -> D -> B -> C true
    world.trigger(HsmTrigger::chain(state_machine, ids[2]));
    world.flush();
    assert_eq!(get_curr_state(world, state_machine), ids[2]);
}
