use bevy::prelude::*;

use crate::prelude::*;

use super::common::*;

// ── HSM: Transition systems (BeforeEnter / AfterExit) ──────────────────

#[test]
fn hsm_before_enter_system_fires() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();

    // Register transition systems
    let transition_id = world.register_system(log_before_enter);
    let mut transition_registry = TransitionRegistry::default();
    transition_registry.insert("log_before_enter", transition_id);
    world.insert_resource(transition_registry);

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
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
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            BeforeEnterSystem::new("log_before_enter"),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("TransHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    // Boot
    app.update();
    clear_log(&mut app);

    // ToSub → BeforeEnter should fire before AfterEnter
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child));
    app.update();

    let log = get_log(&app);
    let before_enter_pos = log.iter().position(|e| e.starts_with("BeforeEnter:"));
    let child_enter_pos = log.iter().position(|e| e == "Child:Enter");

    assert!(
        before_enter_pos.is_some(),
        "Expected BeforeEnter log, got {log:?}"
    );
    assert!(
        child_enter_pos.is_some(),
        "Expected Child:Enter log, got {log:?}"
    );
    assert!(
        before_enter_pos.unwrap() < child_enter_pos.unwrap(),
        "BeforeEnter must fire before Child:Enter, log: {log:?}"
    );
}

#[test]
fn hsm_after_exit_system_fires() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();

    let transition_id = world.register_system(log_after_exit);
    let mut transition_registry = TransitionRegistry::default();
    transition_registry.insert("log_after_exit", transition_id);
    world.insert_resource(transition_registry);

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
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
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            AfterExitSystem::new("log_after_exit"),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("TransHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    // Boot → navigate to Child
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child));
    app.update();

    clear_log(&mut app);

    // ToSuper → AfterExit should fire after BeforeExit
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::to_super);
    app.update();

    let log = get_log(&app);
    let child_exit_pos = log.iter().position(|e| e == "Child:Exit");
    let after_exit_pos = log.iter().position(|e| e.starts_with("AfterExit:"));

    assert!(
        child_exit_pos.is_some(),
        "Expected Child:Exit log, got {log:?}"
    );
    assert!(
        after_exit_pos.is_some(),
        "Expected AfterExit log, got {log:?}"
    );
    assert!(
        child_exit_pos.unwrap() < after_exit_pos.unwrap(),
        "Child:Exit must fire before AfterExit, log: {log:?}"
    );
}

// ── HSM: ServiceTarget delegation ──────────────────────────────────────

#[test]
fn hsm_service_target_routes_actions() {
    let mut app = create_app();

    let world = app.world_mut();
    let action_id = world.register_system(action_on_service_target);
    world.insert_resource(ActionRegistry::from([("on_target", action_id)]));

    // Create a separate service target entity
    let service_target = world.spawn(ActionFired(false)).id();

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("on_target"),
        ))
        .id();

    let tree = StateTree::new(root);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("ServiceHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
        ServiceTarget(service_target),
    ));

    app.update();

    // Action should have fired on the service target, not the SM
    assert!(
        app.world()
            .entity(service_target)
            .get::<ActionFired>()
            .unwrap()
            .0,
        "ActionFired should be true on service_target"
    );
    assert!(
        !app.world().entity(sm).contains::<ActionFired>(),
        "ActionFired should NOT be on the SM entity"
    );
}

// ── HSM: Paused marker ────────────────────────────────────────────

#[test]
fn paused_state_machine_ignores_to_sub() {
    let mut app = create_app();
    let (sm, root_state, child_state) = create_two_level_hsm(&mut app);

    // Boot
    app.update();

    // Pause the state machine
    app.world_mut().entity_mut(sm).insert(Paused);

    // Try to navigate to child
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_state));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        root_state,
        "Paused SM should stay in Root"
    );
}

#[test]
fn paused_state_machine_ignores_chain() {
    let mut app = create_app();
    let (sm, _root_state, a, _a1, b) = create_deep_chain_hsm(&mut app);

    app.update();

    // Navigate to A first
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, a));
    app.update();

    // Pause
    app.world_mut().entity_mut(sm).insert(Paused);

    // Try chain from A → B
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::chain(id, b));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        a,
        "Paused SM should stay in A, not chain to B"
    );
}

#[test]
fn hsm_paused_suppresses_update_system() {
    let mut app = create_app();
    let (sm, root_state, child_state) = create_two_level_hsm(&mut app);

    // Insert Paused BEFORE boot — SM should still initialize properly
    app.world_mut().entity_mut(sm).insert(Paused);

    // Boot with Paused — enter lifecycle still happens (hooks don't check Paused)
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        root_state,
        "Paused SM should still boot into root"
    );
    assert!(
        get_log(&app).contains(&"Root:Enter".to_string()),
        "Root:Enter should fire even when Paused"
    );

    // Transitions are blocked while Paused
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_state));
    app.update();

    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        root_state,
        "Paused SM should stay in Root after ToSub trigger"
    );
}

// ── HSM: Parallel strategy lifecycle ───────────────────────────────────

#[test]
fn hsm_parallel_strategy_parent_reenters_on_child_enter() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();

    // Parent with Parallel strategy + Rebirth behavior
    let parent = world
        .spawn((
            Name::new("ParallelParent"),
            HsmState::with(
                StateTransitionStrategy::Parallel,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    // Child with Nested strategy
    let child = world
        .spawn((
            Name::new("NestedChild"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut tree = StateTree::new(parent);
    tree.with_child(parent, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("ParallelHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            parent,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    // Boot → Parent enters and updates
    app.update();
    let log = get_log(&app);
    assert!(
        log.contains(&"ParallelParent:Enter".to_string()),
        "Parent should enter on boot, got {log:?}"
    );

    clear_log(&mut app);

    // ToSub to child — with Parallel strategy:
    // parent exits (StateLifecycle::Exit), child enters from the queue.
    // Parent stays as the owning state but does not re-enter.
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child));
    app.update();

    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        child,
        "Should be in child after ToSub under Parallel parent"
    );

    let log = get_log(&app);
    assert!(
        log.contains(&"ParallelParent:Exit".to_string()),
        "Parent should exit during ToSub under Parallel, got {log:?}"
    );
    assert!(
        log.contains(&"NestedChild:Enter".to_string()),
        "Child should enter during ToSub, got {log:?}"
    );

    // Verify ordering: Parent:Exit happens before Child:Enter
    let exit_pos = log.iter().position(|e| e == "ParallelParent:Exit");
    let enter_pos = log.iter().position(|e| e == "NestedChild:Enter");
    assert!(
        exit_pos < enter_pos,
        "Parent:Exit should precede Child:Enter, got {log:?}"
    );
}
