use bevy::prelude::*;

use crate::prelude::*;

use super::common::*;

// ── FSM: Basic transitions ────────────────────────────────────────

#[test]
fn fsm_boots_into_initial_state() {
    let mut app = create_app();
    let (sm, state_a, _state_b, _state_c) = create_linear_fsm(&mut app);

    app.update();

    let sm_comp = app.world().get::<FsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_a);

    let log = get_log(&app);
    assert_eq!(log, vec!["A:Enter", "A:Update"], "FSM boot log");
}

#[test]
fn fsm_next_transitions_forward() {
    let mut app = create_app();
    let (sm, _state_a, state_b, _state_c) = create_linear_fsm(&mut app);

    app.update();
    clear_log(&mut app);

    // A → B
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_next(id, state_b));
    app.update();

    let sm_comp = app.world().get::<FsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_b);

    let log = get_log(&app);
    assert_eq!(log, vec!["A:Exit", "B:Enter", "B:Update"], "FSM A->B log");
}

#[test]
fn fsm_next_to_self_is_noop() {
    let mut app = create_app();
    let (sm, state_a, _state_b, _state_c) = create_linear_fsm(&mut app);

    app.update();

    // Next to self should be no-op
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_next(id, state_a));
    app.update();

    let sm_comp = app.world().get::<FsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        state_a,
        "Self-transition should not change state"
    );
}

#[test]
fn fsm_multiple_transitions() {
    let mut app = create_app();
    let (sm, _state_a, state_b, state_c) = create_linear_fsm(&mut app);

    app.update();
    clear_log(&mut app);

    // A → B
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_next(id, state_b));
    app.update();

    assert_eq!(
        app.world()
            .get::<FsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        state_b
    );

    // B → C
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_next(id, state_c));
    app.update();

    assert_eq!(
        app.world()
            .get::<FsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        state_c
    );

    let log = get_log(&app);
    assert_eq!(
        log,
        vec![
            "A:Exit", "B:Enter", "B:Update", "B:Exit", "C:Enter", "C:Update"
        ],
        "FSM A->B->C log"
    );
}

#[test]
fn fsm_invalid_next_does_not_change_state() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let state_a = world
        .spawn((
            Name::new("A"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    // State B with no outgoing transitions to any other state
    let state_b = world
        .spawn((
            Name::new("B"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    // Only A → B, no B → anything
    let mut graph = FsmGraph::new(state_a);
    graph.with_add(state_a, state_b);

    let graph_id = world.spawn(graph).id();
    let sm = world
        .spawn(FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ))
        .id();

    app.update();

    // Transition A → B
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_next(id, state_b));
    app.update();

    assert_eq!(
        app.world()
            .get::<FsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        state_b
    );

    // Next to a state not in B's outgoing transitions → stays in B
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_next(id, state_a));
    app.update();

    assert_eq!(
        app.world()
            .get::<FsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        state_b,
        "Should stay in B when next() targets invalid transition"
    );
}

// ── FSM: Event-driven transitions ────────────────────────────────────

#[test]
fn fsm_event_transition_moves_to_correct_state() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let state_a = world
        .spawn((
            Name::new("A"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();
    let state_b = world
        .spawn((
            Name::new("B"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let mut graph = FsmGraph::new(state_a);
    graph.with_event(state_a, EVENT_GO_TO_B, state_b);

    let graph_id = world.spawn(graph).id();
    let sm = world
        .spawn(FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ))
        .id();

    app.update();

    app.world_mut()
        .trigger(FsmTrigger::with_event(sm, EventData::new(EVENT_GO_TO_B)));
    app.update();

    assert_eq!(
        app.world()
            .get::<FsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        state_b,
        "Event should transition from A to B"
    );
}

#[test]
fn fsm_unmatched_event_does_not_change_state() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let state_a = world
        .spawn((
            Name::new("A"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();
    let state_b = world
        .spawn((
            Name::new("B"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let mut graph = FsmGraph::new(state_a);
    graph.with_event(state_a, EVENT_GO_TO_B, state_b);

    let graph_id = world.spawn(graph).id();
    let sm = world
        .spawn(FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ))
        .id();

    app.update();

    app.world_mut()
        .trigger(FsmTrigger::with_event(sm, EventData::new(EVENT_GO_TO_C)));
    app.update();

    assert_eq!(
        app.world()
            .get::<FsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        state_a,
        "Unmatched event should not change state"
    );
}

// ── FSM: Guard transitions ──────────────────────────────────────────

#[test]
fn fsm_guard_transition_blocks_when_false() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let guard_id = world.register_system(fsm_guard);
    world.insert_resource(GuardRegistry::from([("fsm_guard", guard_id)]));

    let state_a = world
        .spawn((
            Name::new("A"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();
    let state_b = world
        .spawn((
            Name::new("B"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let mut graph = FsmGraph::new(state_a);
    graph.with_condition(state_a, GuardCondition::Id("fsm_guard".into()), state_b);

    let graph_id = world.spawn(graph).id();
    let sm = world
        .spawn((
            FsmStateMachine::with(
                graph_id,
                state_a,
                #[cfg(feature = "history")]
                10,
            ),
            FsmAllowGuard(false),
        ))
        .id();

    app.update();

    app.world_mut().trigger(FsmTrigger::with_guard(sm, state_b));
    app.update();

    assert_eq!(
        app.world()
            .get::<FsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        state_a,
        "Guard should block transition when false"
    );
}

#[test]
fn fsm_guard_transition_allows_when_true() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let guard_id = world.register_system(fsm_guard);
    world.insert_resource(GuardRegistry::from([("fsm_guard", guard_id)]));

    let state_a = world
        .spawn((
            Name::new("A"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();
    let state_b = world
        .spawn((
            Name::new("B"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let mut graph = FsmGraph::new(state_a);
    graph.with_condition(state_a, GuardCondition::Id("fsm_guard".into()), state_b);

    let graph_id = world.spawn(graph).id();
    let sm = world
        .spawn((
            FsmStateMachine::with(
                graph_id,
                state_a,
                #[cfg(feature = "history")]
                10,
            ),
            FsmAllowGuard(true),
        ))
        .id();

    app.update();

    app.world_mut().trigger(FsmTrigger::with_guard(sm, state_b));
    app.update();

    assert_eq!(
        app.world()
            .get::<FsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        state_b,
        "Guard should allow transition when true"
    );
}

// ── FSM: Interrupt within a graph ───────────────────────────────────

#[test]
fn fsm_interrupt_within_graph_and_resume() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();

    // Graph: A → B → C (linear, all two-way)
    let state_a = world
        .spawn((
            Name::new("A"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();
    let state_b = world
        .spawn((
            Name::new("B"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();
    let state_c = world
        .spawn((
            Name::new("C"),
            FsmState,
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let mut graph = FsmGraph::new(state_a);
    graph.with_add(state_a, state_b);
    graph.with_add(state_b, state_a);
    graph.with_add(state_b, state_c);
    graph.with_add(state_c, state_b);
    let graph_id = world.spawn(graph).id();

    let sm = world
        .spawn(FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ))
        .id();

    // Boot in A → navigate to B via next()
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_next(id, state_b));
    app.update();

    let sm_comp = app.world().get::<FsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_b);

    // Interrupt B → C (same graph)
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| FsmTrigger::with_interrupt(id, graph_id, state_c));
    app.update();

    let sm_comp = app.world().get::<FsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        state_c,
        "Should be in C after interrupt"
    );
    assert!(sm_comp.interrupt_stack.is_interrupted());

    // Resume C → B (restores saved state)
    app.world_mut()
        .entity_mut(sm)
        .trigger(FsmTrigger::with_resume);
    app.update();

    let sm_comp = app.world().get::<FsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), state_b, "Should resume to B");
    assert!(!sm_comp.interrupt_stack.is_interrupted());
}
