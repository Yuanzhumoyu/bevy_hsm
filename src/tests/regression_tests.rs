use super::common::*;
use crate::prelude::*;
use bevy::prelude::*;

fn spawn_sm(world: &mut World, tid: Entity, init: Entity) -> Entity {
    world
        .spawn((
            Name::new("SM"),
            StateLifecycle::default(),
            HsmStateMachine::with(tid, init, 10),
        ))
        .id()
}

// Bug #1: Chain intermediate state exits, not re-enters via Rebirth
#[test]
fn chain_exits_intermediate_not_reenter() {
    let mut app = create_app();
    let (sm, _r, _a, a1, b) = create_deep_chain_hsm(&mut app);
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, _a));
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, a1));
    app.update();
    clear_log(&mut app);
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::chain(id, b));
    app.update();
    let log = get_log(&app);
    assert_eq!(
        log,
        vec!["A1:Exit", "A:Exit", "B:Enter", "B:Update"],
        "chain A1->B log"
    );
}

// Bug #2: Cross-tree force_death exits Parallel parent
#[test]
fn cross_tree_exits_parallel_parent() {
    let mut app = create_app();
    register_log_systems(&mut app);
    let (ra, ca, ta, tb, rb, sm);
    {
        let w = app.world_mut();
        ra = w
            .spawn((
                Name::new("RootA"),
                HsmState::with(
                    StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Death,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
            ))
            .id();
        ca = w
            .spawn((
                Name::new("ChildA"),
                HsmState::with(
                    StateTransitionStrategy::Nested,
                    ExitTransitionBehavior::Death,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
                OnUpdateSystem::new("Update:log_update"),
            ))
            .id();
        let mut t = StateTree::new(ra);
        t.with_child(ra, ca);
        ta = w.spawn(t).id();
        rb = w
            .spawn((
                Name::new("RootB"),
                HsmState::with(
                    StateTransitionStrategy::Nested,
                    ExitTransitionBehavior::Death,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
            ))
            .id();
        tb = w.spawn(StateTree::new(rb)).id();
        sm = spawn_sm(w, ta, ra);
    }
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, ca));
    app.update();
    clear_log(&mut app);
    app.world_mut().trigger(HsmTrigger::interrupt(sm, tb, rb));
    app.update();
    let log = get_log(&app);
    assert_eq!(
        log,
        vec!["ChildA:Exit", "RootA:Exit", "RootB:Enter"],
        "cross-tree interrupt log"
    );
}

// Bug #3: Rebirth resumes Update after ToSuper
#[test]
fn rebirth_resumes_update() {
    let mut app = create_app();
    let (sm, _root, child) = create_two_level_hsm(&mut app);
    app.update();
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::to_sub(id, child));
    app.update();
    clear_log(&mut app);
    app.world_mut().entity_mut(sm).trigger(HsmTrigger::to_super);
    app.update();
    assert_eq!(
        get_log(&app),
        vec!["Child:Exit", "Root:Enter", "Root:Update"],
        "to_super log"
    );
    clear_log(&mut app);
    app.update();
    assert_eq!(get_log(&app), vec!["Root:Update"], "update after rebirth");
}

// Bug #4: Paused resume restores Update
#[test]
fn pause_resume_restores_update() {
    let mut app = create_app();
    let (sm, _root, _) = create_two_level_hsm(&mut app);
    app.update();
    clear_log(&mut app);
    app.world_mut().entity_mut(sm).insert(Paused);
    app.world_mut().entity_mut(sm).remove::<Paused>();
    app.update();
    assert_eq!(get_log(&app), vec!["Root:Update"], "update after resume");
}

// Bug #5: Parallel LCA exits on chain to child
#[test]
fn parallel_lca_exits_on_chain_to_child() {
    let mut app = create_app();
    register_log_systems(&mut app);
    let (red, yel, tid, sm);
    {
        let w = app.world_mut();
        red = w
            .spawn((
                Name::new("Red"),
                HsmState::with(
                    StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Rebirth,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
                OnUpdateSystem::new("Update:log_update"),
            ))
            .id();
        yel = w
            .spawn((
                Name::new("Yellow"),
                HsmState::default(),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
                OnUpdateSystem::new("Update:log_update"),
            ))
            .id();
        let mut t = StateTree::new(red);
        t.with_child(red, yel);
        tid = w.spawn(t).id();
        sm = spawn_sm(w, tid, red);
    }
    app.update();
    clear_log(&mut app);
    app.world_mut()
        .entity_mut(sm)
        .trigger(|id| HsmTrigger::chain(id, yel));
    app.update();
    let log = get_log(&app);
    assert_eq!(
        log,
        vec!["Red:Exit", "Yellow:Enter", "Yellow:Update"],
        "chain Red->Yellow log"
    );
}
