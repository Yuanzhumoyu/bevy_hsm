use crate::prelude::*;

use super::common::*;

// ── HSM: Cross-tree interrupt and resume ──────────────────────────

#[test]
fn hsm_cross_tree_interrupt_and_resume() {
    let mut app = create_app();
    let (sm, tree_a_id, root_a, _child_a, tree_b_id, root_b, _child_b) =
        create_two_tree_hsm(&mut app);

    // Boot in TreeA/RootA
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), root_a, "Should boot in RootA");
    assert_eq!(sm_comp.state_tree(), tree_a_id);

    // Interrupt to TreeB/RootB
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, tree_b_id, root_b));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.state_tree(),
        tree_b_id,
        "Tree should switch to B after cross-tree interrupt"
    );
    assert_eq!(
        sm_comp.curr_state_id(),
        root_b,
        "State should be RootB after cross-tree interrupt"
    );
    assert_eq!(
        sm_comp.interrupt_depth(),
        1,
        "Interrupt stack should save old context"
    );
    assert!(sm_comp.is_interrupted());

    // Resume back to TreeA
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.state_tree(),
        tree_a_id,
        "Tree should switch back to A after resume"
    );
    assert_eq!(
        sm_comp.curr_state_id(),
        root_a,
        "State should be RootA after resume"
    );
    assert_eq!(
        sm_comp.interrupt_depth(),
        0,
        "Interrupt stack should be empty after resume"
    );
    assert!(!sm_comp.is_interrupted());
}

#[test]
fn hsm_cross_tree_interrupt_to_child_and_resume() {
    let mut app = create_app();
    let (sm, tree_a_id, _root_a, _child_a, tree_b_id, _root_b, child_b) =
        create_two_tree_hsm(&mut app);

    // Boot in TreeA/RootA, then navigate to ChildA
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, _child_a));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), _child_a, "Should be in ChildA");

    // Interrupt from ChildA to TreeB/ChildB
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, tree_b_id, child_b));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.state_tree(), tree_b_id, "Tree should switch to B");
    assert_eq!(sm_comp.curr_state_id(), child_b, "State should be ChildB");
    assert_eq!(
        sm_comp.interrupt_depth(),
        1,
        "Should save one interrupt frame"
    );
    assert!(sm_comp.is_interrupted());

    // Resume back to TreeA/ChildA
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.state_tree(),
        tree_a_id,
        "Tree should switch back to A"
    );
    assert_eq!(
        sm_comp.curr_state_id(),
        _child_a,
        "State should be ChildA after resume"
    );
    assert_eq!(
        sm_comp.interrupt_depth(),
        0,
        "Stack should be empty after resume"
    );
}

// ── Regression: cross-tree interrupt exit must not re-enter old tree ─

/// When interrupting from Tree A's child (Idle under Work) to Tree B,
/// the exit path must fully exit the old tree with Exit transitions
/// — never re-enter old-tree ancestors via Rebirth/Resurrection.
#[test]
fn hsm_cross_tree_interrupt_exit_path_only_exits_old_tree() {
    let mut app = create_app();
    let (sm, _tree_a_id, _root_a, child_a, tree_b_id, root_b, _child_b) =
        create_two_tree_hsm(&mut app);

    // Boot in TreeA/RootA, navigate to ChildA, then clear log
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_a));
    app.update();
    clear_log(&mut app);

    // Interrupt from ChildA → TreeB/RootB (cross-tree)
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, tree_b_id, root_b));
    app.update();

    // Verify final state
    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.state_tree(), tree_b_id);
    assert_eq!(sm_comp.curr_state_id(), root_b);

    // Lifecycle log must contain Exit for both ChildA AND RootA,
    // and must NOT contain Enter for RootA (the old-tree parent).
    let log = get_log(&app);
    let root_a_exits: Vec<_> = log.iter().filter(|e| *e == "RootA:Exit").collect();
    let root_a_enters: Vec<_> = log.iter().filter(|e| *e == "RootA:Enter").collect();
    let child_a_exits: Vec<_> = log.iter().filter(|e| *e == "ChildA:Exit").collect();
    let root_b_enters: Vec<_> = log.iter().filter(|e| *e == "RootB:Enter").collect();

    assert_eq!(child_a_exits.len(), 1, "ChildA should be exited once");
    assert_eq!(
        root_a_exits.len(),
        1,
        "RootA (old-tree parent) must be exited — not silently abandoned"
    );
    assert_eq!(
        root_a_enters.len(),
        0,
        "RootA (old-tree parent) must NOT be re-entered via Rebirth during cross-tree exit"
    );
    assert_eq!(root_b_enters.len(), 1, "RootB should be entered");
}

/// After a cross-tree interrupt, only the target leaf state in the new
/// tree should receive Update events. Ancestors must NOT be alive.
#[test]
fn hsm_cross_tree_interrupt_only_target_state_receives_update() {
    let mut app = create_app();
    let (sm, _tree_a_id, _root_a, _child_a, tree_b_id, _root_b, child_b) =
        create_two_tree_hsm(&mut app);

    // Boot in TreeA/RootA, then interrupt directly to TreeB/ChildB
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, tree_b_id, child_b));
    app.update();

    // Verify the interrupt landed on ChildB
    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        child_b,
        "After interrupt, SM should be in ChildB"
    );
    assert_eq!(sm_comp.state_tree(), tree_b_id);

    clear_log(&mut app);

    // Run update frames — only the leaf (ChildB) should receive
    // Update events; the ancestor (RootB) must not.
    app.update();
    app.update();
    let log = get_log(&app);

    let root_b_updates: Vec<_> = log.iter().filter(|e| *e == "RootB:Update").collect();
    let child_b_updates: Vec<_> = log.iter().filter(|e| *e == "ChildB:Update").collect();

    assert_eq!(
        root_b_updates.len(),
        0,
        "RootB (ancestor) must NOT receive Update — only the leaf state should. Log: {log:?}"
    );
    assert!(
        !child_b_updates.is_empty(),
        "ChildB (target leaf) should receive Update events. Log: {log:?}"
    );
}

// ── Regression: cross-tree resume enter must not activate new-tree root ─

/// When resuming from a cross-tree interrupt back to a child state,
/// only the target leaf should be entered — the root must not be
/// entered as a full lifecycle state.
#[test]
fn hsm_cross_tree_resume_only_enters_target_leaf() {
    let mut app = create_app();
    let (sm, tree_a_id, _root_a, child_a, tree_b_id, root_b, _child_b) =
        create_two_tree_hsm(&mut app);

    // Boot → ChildA → interrupt to TreeB/RootB
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_a));
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, tree_b_id, root_b));
    app.update();
    clear_log(&mut app);

    // Resume back to TreeA/ChildA
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    // Verify final state
    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.state_tree(), tree_a_id);
    assert_eq!(sm_comp.curr_state_id(), child_a);

    // The enter path must only contain Enter for the target leaf (ChildA),
    // NOT Enter for RootA (the root is an implicit ancestor).
    let log = get_log(&app);
    let root_a_enters: Vec<_> = log.iter().filter(|e| *e == "RootA:Enter").collect();
    let child_a_enters: Vec<_> = log.iter().filter(|e| *e == "ChildA:Enter").collect();

    assert_eq!(
        root_a_enters.len(),
        0,
        "RootA must NOT be entered — only the target leaf state should be"
    );
    assert_eq!(
        child_a_enters.len(),
        1,
        "ChildA (target leaf) should be entered"
    );
}

/// After cross-tree resume, only the target leaf should receive Update
/// events. Ancestors must not be simultaneously alive.
#[test]
fn hsm_cross_tree_resume_only_target_leaf_receives_update() {
    let mut app = create_app();
    let (sm, _tree_a_id, _root_a, child_a, tree_b_id, root_b, _child_b) =
        create_two_tree_hsm(&mut app);

    // Boot → ChildA → interrupt to TreeB/RootB → resume
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child_a));
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, tree_b_id, root_b));
    app.update();
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();
    clear_log(&mut app);

    // Run update frames — only the leaf (ChildA) should receive
    // Update events; the ancestor (RootA) must not.
    app.update();
    app.update();
    let log = get_log(&app);

    let root_a_updates: Vec<_> = log.iter().filter(|e| *e == "RootA:Update").collect();
    let child_a_updates: Vec<_> = log.iter().filter(|e| *e == "ChildA:Update").collect();

    assert_eq!(
        root_a_updates.len(),
        0,
        "RootA must NOT receive Update — only the leaf (ChildA) should be alive. Log: {log:?}"
    );
    assert!(
        !child_a_updates.is_empty(),
        "ChildA should receive Update events. Log: {log:?}"
    );
}

// ── HSM: Same-tree interrupt and resume ───────────────────────────

#[test]
fn hsm_same_tree_interrupt_and_resume() {
    let mut app = create_app();
    let (sm, _root, a, a1, b) = create_deep_chain_hsm(&mut app);

    // Boot in Root → navigate to A → A1
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

    // Same-tree interrupt: A1 → B (under same root)
    let state_tree = app.world().get::<HsmStateMachine>(sm).unwrap().state_tree();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::interrupt(id, state_tree, b));
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(
        sm_comp.curr_state_id(),
        b,
        "Should be in B after same-tree interrupt"
    );
    assert_eq!(sm_comp.interrupt_depth(), 1);
    assert!(sm_comp.is_interrupted());

    // Resume back to A1
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
    app.update();

    let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
    assert_eq!(sm_comp.curr_state_id(), a1, "Should resume to A1");
    assert_eq!(sm_comp.interrupt_depth(), 0);
    assert!(!sm_comp.is_interrupted());
}
