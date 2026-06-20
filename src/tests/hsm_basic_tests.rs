use bevy::prelude::*;

use crate::prelude::*;

use super::common::*;

// ── HSM: Basic boot and navigation ────────────────────────────────

#[test]
fn hsm_boots_into_root_state() {
    let mut app = create_app();
    let (sm, root_state, _child_state) = create_two_level_hsm(&mut app);

    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), root_state);

    let log = get_log(&app);
    assert_eq!(log, vec!["Root:Enter", "Root:Update"], "boot log");
}

#[test]
fn hsm_to_sub_navigates_to_child() {
    let mut app = create_app();
    let (sm, _root_state, child_state) = create_two_level_hsm(&mut app);

    // Boot
    app.update();

    // Trigger ToSub → Child
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_state));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        child_state,
        "Should be in Child after ToSub"
    );
}

#[test]
fn hsm_to_super_returns_to_parent() {
    let mut app = create_app();
    let (sm, root_state, child_state) = create_two_level_hsm(&mut app);

    // Boot
    app.update();

    // Navigate to Child
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_state));
    app.update();

    // Navigate back to Root via ToSuper
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::to_super);
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        root_state,
        "Should be back in Root after ToSuper"
    );
}

#[test]
fn hsm_to_sub_and_back_lifecycle_events() {
    let mut app = create_app();
    let (sm, _root_state, child_state) = create_two_level_hsm(&mut app);

    // Boot
    app.update();
    clear_log(&mut app);

    // ToSub → Child
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_state));
    app.update();

    let log = get_log(&app);
    assert_eq!(log, vec!["Child:Enter", "Child:Update"], "ToSub log");

    // ToSuper → Root
    clear_log(&mut app);
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::to_super);
    app.update();

    let log = get_log(&app);
    assert_eq!(
        log,
        vec!["Child:Exit", "Root:Enter", "Root:Update"],
        "ToSuper log"
    );
}

// ── Regression: Nested ToSub parent OnUpdate leak ──────────────────

/// After ToSub to a child via Nested strategy, only the leaf (child)
/// should receive Update events. The parent's OnUpdateSystem must be
/// suppressed — otherwise both parent and child fire simultaneously.
#[test]
fn hsm_nested_to_sub_only_child_receives_update() {
    let mut app = create_app();
    let (sm, _root_state, child_state) = create_two_level_hsm(&mut app);

    // Boot — Root is active, receives Update
    app.update();

    let log = get_log(&app);
    assert!(
        log.contains(&"Root:Update".to_string()),
        "Root should receive Update after boot. Log: {log:?}"
    );

    // ToSub → Child (Nested: parent should be suppressed)
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_state));
    app.update();

    // Verify the SM is in Child
    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), child_state);

    clear_log(&mut app);

    // Run update frames — only the leaf (Child) should receive Update;
    // the ancestor (Root) must not.
    app.update();
    app.update();
    let log = get_log(&app);

    let root_updates: Vec<_> = log.iter().filter(|e| *e == "Root:Update").collect();
    let child_updates: Vec<_> = log.iter().filter(|e| *e == "Child:Update").collect();

    assert_eq!(
        root_updates.len(),
        0,
        "Root (parent, Nested strategy) must NOT receive Update after ToSub. Log: {log:?}"
    );
    assert!(
        !child_updates.is_empty(),
        "Child (leaf) should receive Update events. Log: {log:?}"
    );
}

// ── HSM: Lifecycle event ordering ─────────────────────────────────

#[test]
fn hsm_lifecycle_enter_before_update() {
    let mut app = create_app();
    let (_sm, _root_state, _child_state) = create_two_level_hsm(&mut app);

    app.update();

    let log = get_log(&app);

    let enter_pos = log.iter().position(|e| e == "Root:Enter");
    let update_pos = log.iter().position(|e| e == "Root:Update");

    assert!(enter_pos.is_some(), "Expected Root:Enter, got {log:?}");
    assert!(update_pos.is_some(), "Expected Root:Update, got {log:?}");
    assert!(
        enter_pos.unwrap() < update_pos.unwrap(),
        "Enter must happen before Update, log: {log:?}"
    );
}

#[test]
fn hsm_lifecycle_exit_before_enter_during_transition() {
    let mut app = create_app();
    let (sm, _root_state, child_state) = create_two_level_hsm(&mut app);

    app.update();

    // Navigate to Child
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_state));
    app.update();

    clear_log(&mut app);

    // Navigate back to Root
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::to_super);
    app.update();

    let log = get_log(&app);

    let exit_pos = log.iter().position(|e| e == "Child:Exit");
    let enter_pos = log.iter().position(|e| e == "Root:Enter");

    assert!(exit_pos.is_some(), "Expected Child:Exit, got {log:?}");
    assert!(enter_pos.is_some(), "Expected Root:Enter, got {log:?}");
    assert!(
        exit_pos.unwrap() < enter_pos.unwrap(),
        "Exit must happen before Enter during transition, log: {log:?}"
    );
}

// ── HSM: State machine termination ────────────────────────────────

#[test]
fn hsm_terminates_when_exit_cascade_reaches_end() {
    let mut app = create_app();
    register_log_systems(&mut app);

    // Root with Death behavior — exiting from root with Death terminates
    let world = app.world_mut();

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Death,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let child = world
        .spawn((
            Name::new("Child"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Death,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("TermHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    // Boot → Root enters
    app.update();

    // Navigate to Child
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child));
    app.update();

    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        child
    );

    clear_log(&mut app);

    // Exit from Child with Death → cascades to Root with Death → terminates
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::to_super);
    app.update();

    // After termination, Terminated marker should be present
    assert!(
        app.world().entity(sm).contains::<Terminated>(),
        "State machine should be terminated"
    );
}
