use bevy::ecs::system::RunSystemError;
use bevy::prelude::*;

use crate::prelude::*;

use super::*;

// ── pure unit tests ──────────────────────────────────────────────

fn make_entity(id: u32) -> Entity {
    Entity::from_raw_u32(id).expect("invalid raw entity id")
}

#[test]
fn interrupt_stack_push_pop() {
    let e = make_entity;
    let mut sm = FsmStateMachine::with(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        #[cfg(feature = "history")]
        0,
    );

    assert!(!sm.interrupt_stack.is_interrupted());
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 0);
    assert!(sm.interrupt_stack.pop_interrupt().is_none());

    sm.interrupt_stack.push_interrupt(Entity::PLACEHOLDER, e(1));
    assert!(sm.interrupt_stack.is_interrupted());
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 1);

    sm.interrupt_stack.push_interrupt(Entity::PLACEHOLDER, e(2));
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 2);

    // LIFO order
    assert_eq!(
        sm.interrupt_stack.pop_interrupt(),
        Some(InterruptFrame::new(Entity::PLACEHOLDER, e(2)))
    );
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 1);
    assert!(sm.interrupt_stack.is_interrupted());

    assert_eq!(
        sm.interrupt_stack.pop_interrupt(),
        Some(InterruptFrame::new(Entity::PLACEHOLDER, e(1)))
    );
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 0);
    assert!(!sm.interrupt_stack.is_interrupted());

    assert!(sm.interrupt_stack.pop_interrupt().is_none());
}

#[test]
fn clear_interrupt_stack() {
    let e = make_entity;
    let mut sm = FsmStateMachine::with(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        #[cfg(feature = "history")]
        0,
    );

    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, e(10));
    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, e(20));
    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, e(30));
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 3);

    sm.interrupt_stack.clear_interrupt_stack();
    assert!(!sm.interrupt_stack.is_interrupted());
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 0);
    assert!(sm.interrupt_stack.pop_interrupt().is_none());
}

#[test]
fn set_curr_state_preserves_interrupt_stack() {
    let e = make_entity;
    let mut sm = FsmStateMachine::with(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        #[cfg(feature = "history")]
        0,
    );

    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, e(42));
    sm.set_curr_state(e(99));

    // interrupt stack must survive normal state changes
    assert_eq!(
        sm.interrupt_stack.pop_interrupt(),
        Some(InterruptFrame::new(Entity::PLACEHOLDER, e(42)))
    );
    assert_eq!(sm.curr_state_id(), e(99));
}

// ── integration-test helpers ────────────────────────────────────

#[derive(Resource, Default)]
struct EventLog(Vec<String>);

fn log_enter(ctx: In<ActionContext>, query: Query<&Name>, mut log: ResMut<EventLog>) {
    if let Ok(name) = query.get(ctx.state()) {
        log.0.push(format!("{}:Enter", name));
    }
}

fn log_exit(ctx: In<ActionContext>, query: Query<&Name>, mut log: ResMut<EventLog>) {
    if let Ok(name) = query.get(ctx.state()) {
        log.0.push(format!("{}:Exit", name));
    }
}

fn log_update(
    contexts: In<Vec<ActionContext>>,
    query: Query<&Name>,
    mut log: ResMut<EventLog>,
) -> Option<Vec<ActionContext>> {
    for ctx in &contexts.0 {
        if let Ok(name) = query.get(ctx.state()) {
            log.0.push(format!("{}:Update", name));
        }
    }
    None
}

fn create_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StateMachinePlugin::default());
    app
}

fn register_log_systems(app: &mut App) {
    app.add_action_system(Update, "log_update", log_update);
    let world = app.world_mut();
    let registry = ActionRegistry::from([
        ("log_enter", world.register_system(log_enter)),
        ("log_exit", world.register_system(log_exit)),
    ]);
    world.insert_resource(registry);
    world.insert_resource(EventLog::default());
}

fn get_event_log(app: &App) -> Vec<String> {
    app.world().get_resource::<EventLog>().unwrap().0.clone()
}

/// Creates a two-state FSM: A <-> B, starting in A.
/// Returns (state_machine_id, state_a, state_b).
fn create_two_state_fsm(app: &mut App) -> (Entity, Entity, Entity) {
    register_log_systems(app);

    let world = app.world_mut();
    let state_a = world
        .spawn((
            Name::new("A"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let state_b = world
        .spawn((
            Name::new("B"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut graph = FsmGraph::new(state_a);
    graph.with_add(state_a, state_b);
    graph.with_add(state_b, state_a);
    let graph_id = world.spawn(graph).id();

    let sm_id = world
        .spawn(FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ))
        .id();

    (sm_id, state_a, state_b)
}

/// Creates a three-state FSM: A <-> B <-> C, starting in A.
/// Returns (state_machine_id, state_a, state_b, state_c).
fn create_three_state_fsm(app: &mut App) -> (Entity, Entity, Entity, Entity) {
    register_log_systems(app);

    let world = app.world_mut();
    let state_a = world
        .spawn((
            Name::new("A"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let state_b = world
        .spawn((
            Name::new("B"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let state_c = world
        .spawn((
            Name::new("C"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut graph = FsmGraph::new(state_a);
    graph.with_add(state_a, state_b);
    graph.with_add(state_b, state_a);
    graph.with_add(state_b, state_c);
    graph.with_add(state_c, state_b);
    let graph_id = world.spawn(graph).id();

    let sm_id = world
        .spawn(FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ))
        .id();

    (sm_id, state_a, state_b, state_c)
}

fn interrupt(app: &mut App, sm: Entity, target_state: Entity) {
    let graph_id = {
        let world = app.world();
        world.get::<FsmStateMachine>(sm).unwrap().state_graph_id()
    };
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_interrupt(id, graph_id, target_state));
    app.update();
}

fn resume(app: &mut App, sm: Entity) -> Result<(), RunSystemError> {
    app.world_mut()
        .entity_mut(sm)
        .trigger(FsmTrigger::with_resume);
    app.update();
    Ok(())
}

fn next(app: &mut App, sm: Entity, target: Entity) {
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_next(id, target));
    app.update();
}

fn get_sm(app: &App, sm: Entity) -> &FsmStateMachine {
    app.world()
        .get::<FsmStateMachine>(sm)
        .expect("FsmStateMachine missing")
}

// ── integration tests ───────────────────────────────────────────

/// Boot: app starts in A, enter action fires, initial log has A:Enter.
#[test]
fn boot_enters_initial_state() {
    let mut app = create_app();
    let (sm_id, state_a, _state_b) = create_two_state_fsm(&mut app);

    // Hooks haven't fired yet (on_insert is pending)
    app.update();

    let sm = get_sm(&app, sm_id);
    assert_eq!(sm.curr_state_id(), state_a);

    let log = get_event_log(&app);
    assert!(
        log.iter().any(|e| e == "A:Enter"),
        "expected A:Enter, got {log:?}"
    );
}

/// Interrupt A → B, verify B is active and A is saved.
#[test]
fn basic_interrupt_and_resume() {
    let mut app = create_app();
    let (sm_id, state_a, state_b) = create_two_state_fsm(&mut app);
    app.update();

    // Clear the boot-up log so we only see interrupt-related events.
    app.world_mut()
        .get_resource_mut::<EventLog>()
        .unwrap()
        .0
        .clear();

    // Interrupt
    interrupt(&mut app, sm_id, state_b);

    let sm = get_sm(&app, sm_id);
    assert_eq!(
        sm.curr_state_id(),
        state_b,
        "should be in B after interrupt"
    );
    assert!(sm.interrupt_stack.is_interrupted());
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 1);

    // Resume
    resume(&mut app, sm_id).unwrap();

    let sm = get_sm(&app, sm_id);
    assert_eq!(
        sm.curr_state_id(),
        state_a,
        "should return to A after resume"
    );
    assert!(!sm.interrupt_stack.is_interrupted());
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 0);
}

/// Interrupting to the current state is a no-op.
#[test]
fn self_interrupt_is_noop() {
    let mut app = create_app();
    let (sm_id, state_a, _state_b) = create_two_state_fsm(&mut app);
    app.update();

    let before = get_sm(&app, sm_id);
    assert_eq!(before.curr_state_id(), state_a);
    assert!(!before.interrupt_stack.is_interrupted());

    // Interrupt to self
    interrupt(&mut app, sm_id, state_a);

    let after = get_sm(&app, sm_id);
    assert_eq!(after.curr_state_id(), state_a, "state should not change");
    assert!(
        !after.interrupt_stack.is_interrupted(),
        "stack should be empty"
    );
    assert_eq!(after.interrupt_stack.interrupt_depth(), 0);
}

/// Resume with an empty interrupt stack is a no-op.
#[test]
fn resume_empty_stack_is_noop() {
    let mut app = create_app();
    let (sm_id, state_a, _state_b) = create_two_state_fsm(&mut app);
    app.update();

    resume(&mut app, sm_id).unwrap();

    let sm = get_sm(&app, sm_id);
    assert_eq!(sm.curr_state_id(), state_a);
    assert!(!sm.interrupt_stack.is_interrupted());
}

/// Nested interrupts: A → B → C, then resume C → B → A.
#[test]
fn nested_interrupt() {
    let mut app = create_app();
    let (sm_id, state_a, state_b, state_c) = create_three_state_fsm(&mut app);
    app.update();

    // A → B
    interrupt(&mut app, sm_id, state_b);
    let sm = get_sm(&app, sm_id);
    assert_eq!(sm.curr_state_id(), state_b);
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 1);

    // B → C (nested)
    interrupt(&mut app, sm_id, state_c);
    let sm = get_sm(&app, sm_id);
    assert_eq!(sm.curr_state_id(), state_c);
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 2);

    // Resume C → B
    resume(&mut app, sm_id).unwrap();
    let sm = get_sm(&app, sm_id);
    assert_eq!(sm.curr_state_id(), state_b);
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 1);

    // Resume B → A
    resume(&mut app, sm_id).unwrap();
    let sm = get_sm(&app, sm_id);
    assert_eq!(sm.curr_state_id(), state_a);
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 0);
}

/// Interrupt preserves the state that was running at interrupt time
/// (not the initial state). A →(next) B →(interrupt) C →(resume) B.
#[test]
fn interrupt_saves_current_not_initial_state() {
    let mut app = create_app();
    let (sm_id, _state_a, state_b, state_c) = create_three_state_fsm(&mut app);
    app.update();

    // Normal transition A → B
    next(&mut app, sm_id, state_b);
    assert_eq!(get_sm(&app, sm_id).curr_state_id(), state_b);

    // Interrupt B → C
    interrupt(&mut app, sm_id, state_c);
    let sm = get_sm(&app, sm_id);
    assert_eq!(sm.curr_state_id(), state_c);
    assert!(sm.interrupt_stack.is_interrupted());

    // Resume should go back to B, not A
    resume(&mut app, sm_id).unwrap();
    assert_eq!(get_sm(&app, sm_id).curr_state_id(), state_b);
    assert!(!get_sm(&app, sm_id).interrupt_stack.is_interrupted());
}

/// Verify lifecycle event ordering during interrupt + resume.
/// Events expected (log cleared between transitions):
///   Interrupt B→C:  B:Exit, C:Enter
///   Resume  C→B:   C:Exit, B:Enter
#[test]
fn interrupt_and_resume_lifecycle_order() {
    let mut app = create_app();
    let (sm_id, _state_a, state_b, state_c) = create_three_state_fsm(&mut app);
    app.update();

    // Normal A → B
    next(&mut app, sm_id, state_b);

    // Clear: only look at interrupt/resume events
    app.world_mut()
        .get_resource_mut::<EventLog>()
        .unwrap()
        .0
        .clear();

    // Interrupt B → C
    interrupt(&mut app, sm_id, state_c);

    // Resume C → B
    resume(&mut app, sm_id).unwrap();

    let log = get_event_log(&app);

    let indices = |s: &str| -> Vec<usize> {
        log.iter()
            .enumerate()
            .filter(|(_, e)| *e == s)
            .map(|(i, _)| i)
            .collect()
    };

    let b_exit = indices("B:Exit");
    let c_enter = indices("C:Enter");
    let c_exit = indices("C:Exit");
    let b_enter = indices("B:Enter");

    // Interrupt: B:Exit happens, then C:Enter
    assert!(!b_exit.is_empty(), "expected B:Exit");
    assert!(!c_enter.is_empty(), "expected C:Enter");
    assert!(b_exit[0] < c_enter[0], "B:Exit must precede C:Enter");

    // Resume: C:Exit happens, then B:Enter (one new B:Enter from resume)
    assert!(!c_exit.is_empty(), "expected C:Exit");
    assert!(!b_enter.is_empty(), "expected B:Enter from resume");
    assert!(c_exit[0] < b_enter[0], "C:Exit must precede B:Enter");
}

/// OnUpdate systems fire when the state is entered (during interrupt/resume transitions).
/// Verify that both the interrupt target and the resumed state run their update systems.
#[test]
fn interrupt_and_resume_fire_on_update() {
    let mut app = create_app();
    let (sm_id, _state_a, state_b) = create_two_state_fsm(&mut app);
    app.update();

    // Interrupt A → B: B:Enter and B:Update fire during this update
    interrupt(&mut app, sm_id, state_b);
    let log = get_event_log(&app);
    assert!(log.iter().any(|e| e == "B:Enter"), "expected B:Enter");
    assert!(
        log.iter().any(|e| e == "B:Update"),
        "B should update after interrupt"
    );

    // Clear log, resume B → A
    app.world_mut()
        .get_resource_mut::<EventLog>()
        .unwrap()
        .0
        .clear();
    resume(&mut app, sm_id).unwrap();
    let log = get_event_log(&app);
    assert!(log.iter().any(|e| e == "A:Enter"), "expected A:Enter");
    assert!(
        log.iter().any(|e| e == "A:Update"),
        "A should update after resume, got {log:?}"
    );
}

/// Multiple independent interrupt-resume cycles, each returning correctly.
#[test]
fn multiple_interrupt_resume_cycles() {
    let mut app = create_app();
    let (sm_id, state_a, state_b, state_c) = create_three_state_fsm(&mut app);
    app.update();

    // Cycle 1: A →(interrupt) B →(resume) A
    interrupt(&mut app, sm_id, state_b);
    assert_eq!(get_sm(&app, sm_id).curr_state_id(), state_b);
    resume(&mut app, sm_id).unwrap();
    assert_eq!(get_sm(&app, sm_id).curr_state_id(), state_a);

    // Cycle 2: A →(interrupt) C →(resume) A
    interrupt(&mut app, sm_id, state_c);
    assert_eq!(get_sm(&app, sm_id).curr_state_id(), state_c);
    resume(&mut app, sm_id).unwrap();
    assert_eq!(get_sm(&app, sm_id).curr_state_id(), state_a);

    // Verify stack is clean
    assert!(!get_sm(&app, sm_id).interrupt_stack.is_interrupted());
}

/// Clear the interrupt stack mid-flight and verify resume becomes a no-op.
#[test]
fn clear_interrupt_stack_midflight() {
    let mut app = create_app();
    let (sm_id, _state_a, state_b, state_c) = create_three_state_fsm(&mut app);
    app.update();

    // Nested interrupts: A → B → C
    interrupt(&mut app, sm_id, state_b);
    interrupt(&mut app, sm_id, state_c);
    assert_eq!(get_sm(&app, sm_id).interrupt_stack.interrupt_depth(), 2);

    // Clear the stack while still in C
    app.world_mut()
        .entity_mut(sm_id)
        .entry::<FsmStateMachine>()
        .and_modify(|mut sm| sm.interrupt_stack.clear_interrupt_stack());
    app.update();

    assert!(!get_sm(&app, sm_id).interrupt_stack.is_interrupted());

    // Resume should be a no-op now
    resume(&mut app, sm_id).unwrap();
    assert_eq!(
        get_sm(&app, sm_id).curr_state_id(),
        state_c,
        "should remain in C after clearing stack"
    );
}
