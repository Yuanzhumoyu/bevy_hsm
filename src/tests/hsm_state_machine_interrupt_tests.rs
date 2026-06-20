use bevy::prelude::*;

use crate::{StateMachinePlugin, context::*, prelude::*, state_actions::*};

use super::*;

#[derive(Resource, Default)]
struct EventLog(Vec<String>);

fn log_enter(ctx: In<ActionContext>, query: Query<&Name>, mut log: ResMut<EventLog>) {
    let name = query.get(ctx.state()).unwrap();
    log.0.push(format!("{}:Enter", name));
}

fn log_exit(ctx: In<ActionContext>, query: Query<&Name>, mut log: ResMut<EventLog>) {
    let name = query.get(ctx.state()).unwrap();
    log.0.push(format!("{}:Exit", name));
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

/// Creates two states A (root) and B (child of A), both Parallel+Rebirth.
/// Returns (state_machine, state_a, state_b).
fn create_two_state_setup(app: &mut App) -> (Entity, Entity, Entity) {
    register_log_systems(app);

    let world = app.world_mut();
    let state_a = world
        .spawn((
            Name::new("A"),
            HsmState::with(
                StateTransitionStrategy::Parallel,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let state_b = world
        .spawn((
            Name::new("B"),
            HsmState::with(
                StateTransitionStrategy::Parallel,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut state_tree = StateTree::new(state_a);
    state_tree.with_child(state_a, state_b);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        state_tree,
        Name::new("TestSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            state_a,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    (sm, state_a, state_b)
}

/// Creates three states A (root), B (child of A), C (child of B).
fn create_three_state_setup(app: &mut App) -> (Entity, Entity, Entity, Entity) {
    register_log_systems(app);

    let world = app.world_mut();
    let state_a = world
        .spawn((
            Name::new("A"),
            HsmState::with(
                StateTransitionStrategy::Parallel,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let state_b = world
        .spawn((
            Name::new("B"),
            HsmState::with(
                StateTransitionStrategy::Parallel,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let state_c = world
        .spawn((
            Name::new("C"),
            HsmState::with(
                StateTransitionStrategy::Parallel,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut state_tree = StateTree::new(state_a);
    state_tree.with_child(state_a, state_b);
    state_tree.with_child(state_b, state_c);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        state_tree,
        Name::new("TestSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            state_a,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    (sm, state_a, state_b, state_c)
}

fn get_event_log(app: &App) -> Vec<String> {
    app.world().get_resource::<EventLog>().unwrap().0.clone()
}

// ── Unit tests ────────────────────────────────────────────────

#[test]
fn interrupt_stack_push_pop() {
    let e = |i| Entity::from_raw_u32(i).unwrap();
    let mut sm = HsmStateMachine::with(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        #[cfg(feature = "history")]
        0,
    );

    assert!(!sm.interrupt_stack.is_interrupted());
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 0);
    assert_eq!(sm.interrupt_stack.pop_interrupt(), None);

    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, e(10));
    assert!(sm.interrupt_stack.is_interrupted());
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 1);

    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, e(20));
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 2);

    assert_eq!(
        sm.interrupt_stack.pop_interrupt(),
        Some(InterruptFrame::new(Entity::PLACEHOLDER, e(20)))
    );
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 1);
    assert!(sm.interrupt_stack.is_interrupted());

    assert_eq!(
        sm.interrupt_stack.pop_interrupt(),
        Some(InterruptFrame::new(Entity::PLACEHOLDER, e(10)))
    );
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 0);
    assert!(!sm.interrupt_stack.is_interrupted());

    assert_eq!(sm.interrupt_stack.pop_interrupt(), None);
}

#[test]
fn clear_interrupt_stack() {
    let mut sm = HsmStateMachine::with(
        Entity::PLACEHOLDER,
        Entity::PLACEHOLDER,
        #[cfg(feature = "history")]
        0,
    );

    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, Entity::from_raw_u32(1).unwrap());
    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, Entity::from_raw_u32(2).unwrap());
    sm.interrupt_stack
        .push_interrupt(Entity::PLACEHOLDER, Entity::from_raw_u32(3).unwrap());
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 3);

    sm.interrupt_stack.clear_interrupt_stack();
    assert_eq!(sm.interrupt_stack.interrupt_depth(), 0);
    assert!(!sm.interrupt_stack.is_interrupted());
}

// ── Integration tests ─────────────────────────────────────────

#[test]
fn basic_interrupt_and_resume() {
    let mut app = create_app();
    let (sm, _state_a, state_b) = create_two_state_setup(&mut app);

    // Boot: A enters.
    app.update();
    let initial_log = get_event_log(&app);
    assert!(
        initial_log.iter().any(|e| e == "A:Enter"),
        "Expected A:Enter, got {initial_log:?}"
    );

    // Interrupt A → B (Parallel strategy: A stays active, B enters)
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, id, state_b));
    app.update();

    let after_interrupt_log = get_event_log(&app);
    assert!(
        after_interrupt_log.iter().any(|e| e == "B:Enter"),
        "Expected B:Enter in {after_interrupt_log:?}"
    );

    // Verify state machine is in state B and interrupted
    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_b);
    assert!(sm_comp.interrupt_stack.is_interrupted());
    assert_eq!(sm_comp.interrupt_stack.interrupt_depth(), 1);

    // Resume B → A
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    let after_resume_log = get_event_log(&app);
    assert!(
        after_resume_log.iter().any(|e| e == "B:Exit"),
        "Expected B:Exit in {after_resume_log:?}"
    );
    assert!(
        after_resume_log.iter().filter(|e| *e == "A:Enter").count() >= 1,
        "Expected A re-enter in {after_resume_log:?}"
    );

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), _state_a);
    assert!(!sm_comp.interrupt_stack.is_interrupted());
}

#[test]
fn nested_interrupt() {
    let mut app = create_app();
    let (sm, _state_a, state_b, state_c) = create_three_state_setup(&mut app);

    // Boot: A enters
    app.update();

    // Interrupt A → B
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, id, state_b));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_b);
    assert_eq!(sm_comp.interrupt_stack.interrupt_depth(), 1);

    // Nested interrupt B → C
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, id, state_c));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_c);
    assert_eq!(sm_comp.interrupt_stack.interrupt_depth(), 2);

    // Resume C → B
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_b);
    assert_eq!(sm_comp.interrupt_stack.interrupt_depth(), 1);

    // Resume B → A
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), _state_a);
    assert_eq!(sm_comp.interrupt_stack.interrupt_depth(), 0);
    assert!(!sm_comp.interrupt_stack.is_interrupted());
}

#[test]
fn resume_with_empty_stack_is_noop() {
    let mut app = create_app();
    let (sm, state_a, _state_b) = create_two_state_setup(&mut app);

    // Boot: A enters
    app.update();
    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_a);

    // Resume with empty stack — should be a no-op
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    // State should still be A
    assert_eq!(sm_comp.curr_state_id(), state_a);
    assert!(!sm_comp.interrupt_stack.is_interrupted());
}

#[test]
fn interrupt_to_self_is_noop() {
    let mut app = create_app();
    let (sm, state_a, _state_b) = create_two_state_setup(&mut app);

    // Boot: A enters
    app.update();

    // Interrupt to self
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, id, state_a));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_a);
    // Stack should be empty since self-interrupt is skipped
    assert!(!sm_comp.interrupt_stack.is_interrupted());
}

#[test]
fn multiple_interrupts_and_resumes_event_order() {
    let mut app = create_app();
    let (sm, _state_a, state_b, state_c) = create_three_state_setup(&mut app);

    // Boot: A enters
    app.update();

    // Clear the boot log so we only see interrupt-related events
    app.world_mut()
        .get_resource_mut::<EventLog>()
        .unwrap()
        .0
        .clear();

    // Interrupt A → B (Parallel: A stays active, B enters)
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, id, state_b));
    app.update();

    // Interrupt B → C (Parallel: B stays active, C enters)
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, id, state_c));
    app.update();

    // Resume C → B
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    // Resume B → A
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    let log = get_event_log(&app);

    // With Parallel+Rebirth, the parent does not exit when a child enters.
    // Expected sequence: B:Enter, B:Update, C:Enter, C:Update,
    //                    C:Exit, B:Enter, B:Update, B:Exit, A:Enter
    assert!(
        log.contains(&"B:Enter".to_string()),
        "Expected B:Enter in {log:?}"
    );
    assert!(
        log.contains(&"C:Enter".to_string()),
        "Expected C:Enter in {log:?}"
    );
    assert!(
        log.contains(&"C:Exit".to_string()),
        "Expected C:Exit in {log:?}"
    );
    assert!(
        log.contains(&"B:Exit".to_string()),
        "Expected B:Exit in {log:?}"
    );

    // Verify correct causal ordering
    let idx = |s: &str| {
        log.iter()
            .enumerate()
            .filter(|(_, e)| *e == s)
            .map(|(i, _)| i)
            .collect::<Vec<_>>()
    };

    let b_enter = idx("B:Enter");
    let c_enter = idx("C:Enter");
    let c_exit = idx("C:Exit");
    let b_exit = idx("B:Exit");
    let a_enter = idx("A:Enter");

    // B first enters before C enters
    assert!(
        b_enter[0] < c_enter[0],
        "B:Enter({}) before C:Enter({}), log: {log:?}",
        b_enter[0],
        c_enter[0]
    );
    // C exits before B re-enters
    assert!(
        c_exit[0] < b_enter[1],
        "C:Exit({}) before B:Enter({}), log: {log:?}",
        c_exit[0],
        b_enter[1]
    );
    // B exits before A re-enters
    assert!(
        b_exit[0] < a_enter[0],
        "B:Exit({}) before A:Enter({}), log: {log:?}",
        b_exit[0],
        a_enter[0]
    );
}
