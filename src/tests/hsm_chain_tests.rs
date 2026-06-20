use bevy::prelude::*;

use crate::prelude::*;

use super::common::*;

// ── HSM: Chain transitions (LCA-based) ────────────────────────────

#[test]
fn hsm_chain_to_sibling_state() {
    let mut app = create_app();
    let (sm, _root, a, _a1, b) = create_deep_chain_hsm(&mut app);

    // Boot in Root
    app.update();

    // Navigate to A
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, a));
    app.update();

    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        a
    );

    clear_log(&mut app);

    // Chain from A → B (siblings under Root)
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::chain(id, b));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), b, "Should chain from A to B");

    let log = get_log(&app);
    assert_eq!(log, vec!["A:Exit", "B:Enter", "B:Update"], "chain A->B log");
}

#[test]
fn hsm_chain_deep_lca_transition() {
    let mut app = create_app();
    let (sm, _root, a, a1, b) = create_deep_chain_hsm(&mut app);

    // Boot → A → A1
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, a));
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, a1));
    app.update();

    assert_eq!(
        app.world()
            .get::<HsmStateMachine>(sm)
            .unwrap()
            .curr_state_id(),
        a1
    );

    clear_log(&mut app);

    // Chain from A1 → B (LCA is Root)
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::chain(id, b));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), b);

    let log = get_log(&app);
    assert_eq!(
        log,
        vec!["A1:Exit", "A:Exit", "B:Enter", "B:Update"],
        "chain A1->B log"
    );
}

#[test]
fn hsm_chain_to_self_is_noop() {
    let mut app = create_app();
    let (sm, root_state, _child_state) = create_two_level_hsm(&mut app);

    app.update();

    let before = app
        .world()
        .get::<HsmStateMachine>(sm)
        .unwrap()
        .curr_state_id();

    // Chain to self should be ignored
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::chain(id, root_state));
    app.update();

    let after = app
        .world()
        .get::<HsmStateMachine>(sm)
        .unwrap()
        .curr_state_id();
    assert_eq!(before, after, "Chain to self should not change state");
}
