use bevy::prelude::*;

use crate::prelude::*;

use super::common::*;

// ── HSM: GuardEnter (entry guards) ──────────────────────────────────

#[test]
fn hsm_guard_sub_blocks_when_false() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();

    // Register guard
    let guard_id = world.register_system(guard_allow_enter);
    let guard_registry = GuardRegistry::from([("allow_enter", guard_id)]);
    world.insert_resource(guard_registry);

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
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
            GuardEnter::new("allow_enter"),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("GuardHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
        AllowEnter(false), // Guard will block
    ));

    // Boot → wait for GuardEnter to be checked (Nested strategy)
    // Frame 1: Root Enter → Root Update → GuardEnter checked on Child
    // Guard returns false → Child should NOT be entered
    app.update();
    app.update(); // Extra frame to let transition systems try

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        root,
        "Guard should have blocked entry to Child"
    );

    let log = get_log(&app);
    assert!(
        !log.contains(&"Child:Enter".to_string()),
        "Child should not have been entered, log: {log:?}"
    );
}

#[test]
fn hsm_guard_sub_allows_when_true() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();

    let guard_id = world.register_system(guard_allow_enter);
    let guard_registry = GuardRegistry::from([("allow_enter", guard_id)]);
    world.insert_resource(guard_registry);

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
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
            GuardEnter::new("allow_enter"),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("GuardHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
        AllowEnter(true), // Guard will allow
    ));

    // Frame 1: Root Enter → Root Update → GuardEnter checked on Child
    // Guard returns true → Child entered
    app.update();
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        child,
        "Guard should have allowed entry to Child"
    );

    let log = get_log(&app);
    assert!(
        log.contains(&"Child:Enter".to_string()),
        "Expected Child:Enter, got {log:?}"
    );
}

// ── HSM: GuardExit (exit guards) ────────────────────────────────────

#[test]
fn hsm_guard_exit_blocks_when_false() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let guard_id = world.register_system(guard_allow_exit);
    let guard_registry = GuardRegistry::from([("allow_exit", guard_id)]);
    world.insert_resource(guard_registry);

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
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
            GuardExit::new("allow_exit"),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("GuardExitHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
        AllowExit(false),
    ));

    // Boot → navigate to Child
    app.update();
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
        "Should be in Child before testing guard exit"
    );

    // GuardExit is checked by the automatic guard-checking system during
    // subsequent updates. Since AllowExit is false, the child state should
    // persist even after multiple frames.
    app.update();
    app.update();

    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        child,
        "GuardExit should have blocked auto-exit to Root"
    );
}

#[test]
fn hsm_guard_exit_allows_when_true() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let guard_id = world.register_system(guard_allow_exit);
    let guard_registry = GuardRegistry::from([("allow_exit", guard_id)]);
    world.insert_resource(guard_registry);

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    // GuardExit on child from the start — when AllowExit is true,
    // the auto-exit will fire immediately after entering child,
    // transitioning back to Root.
    let child = world
        .spawn((
            Name::new("Child"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            GuardExit::new("allow_exit"),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("GuardExitHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
        AllowExit(true),
    ));

    // Boot → navigate to Child
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child));
    app.update();

    // GuardExit allows → auto-exit fires immediately after entering Child,
    // so we end up back at Root (not Child).
    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        root,
        "GuardExit allowed auto-exit, should be back at Root"
    );
}

// ── HSM: Compound guard conditions ──────────────────────────────────

#[test]
fn hsm_compound_guard_and_allows_when_both_true() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let guard_a_id = world.register_system(guard_check_a);
    let guard_b_id = world.register_system(guard_check_b);
    let guard_registry = GuardRegistry::from([("guard_a", guard_a_id), ("guard_b", guard_b_id)]);
    world.insert_resource(guard_registry);

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    // Compound guard: and(guard_a, guard_b)
    let compound = GuardCondition::parse("and(guard_a,guard_b)").unwrap();
    let child = world
        .spawn((
            Name::new("Child"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            GuardEnter(compound),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("CompoundHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
        GuardA(true),
        GuardB(true),
    ));

    app.update();
    app.update();

    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        child,
        "Compound guard should allow entry when both are true"
    );
}

#[test]
fn hsm_compound_guard_and_blocks_when_one_false() {
    let mut app = create_app();
    register_log_systems(&mut app);

    let world = app.world_mut();
    let guard_a_id = world.register_system(guard_check_a);
    let guard_b_id = world.register_system(guard_check_b);
    let guard_registry = GuardRegistry::from([("guard_a", guard_a_id), ("guard_b", guard_b_id)]);
    world.insert_resource(guard_registry);

    let root = world
        .spawn((
            Name::new("Root"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let compound = GuardCondition::parse("and(guard_a,guard_b)").unwrap();
    let child = world
        .spawn((
            Name::new("Child"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            GuardEnter(compound),
        ))
        .id();

    let mut tree = StateTree::new(root);
    tree.with_child(root, child);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("CompoundHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
        GuardA(false), // A blocks
        GuardB(true),
    ));

    app.update();
    app.update();

    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        root,
        "Compound guard should block entry when one guard returns false"
    );
}
