use bevy::prelude::*;

use crate::{context::*, prelude::*};

// ── Shared test helpers ───────────────────────────────────────────

#[derive(Resource, Default)]
pub(crate) struct EventLog(pub Vec<String>);

pub(crate) fn log_enter(ctx: In<ActionContext>, query: Query<&Name>, mut log: ResMut<EventLog>) {
    if let Ok(name) = query.get(ctx.state()) {
        log.0.push(format!("{}:Enter", name));
    }
}

pub(crate) fn log_exit(ctx: In<ActionContext>, query: Query<&Name>, mut log: ResMut<EventLog>) {
    if let Ok(name) = query.get(ctx.state()) {
        log.0.push(format!("{}:Exit", name));
    }
}

pub(crate) fn log_update(
    contexts: In<Vec<ActionContext>>,
    query: Query<&Name>,
    mut log: ResMut<EventLog>,
) -> Option<Vec<ActionContext>> {
    for ctx in &contexts.0 {
        if let Ok(name) = query.get(ctx.state()) {
            log.0.push(format!("{}:Update", name));
        }
    }
    Some(contexts.0)
}

pub(crate) fn create_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StateMachinePlugin::default());
    app
}

pub(crate) fn register_log_systems(app: &mut App) {
    app.add_action_system(Update, "log_update", log_update);
    let world = app.world_mut();
    let registry = ActionRegistry::from([
        ("log_enter", world.register_system(log_enter)),
        ("log_exit", world.register_system(log_exit)),
    ]);
    world.insert_resource(registry);
    world.insert_resource(EventLog::default());
}

pub(crate) fn get_log(app: &App) -> Vec<String> {
    app.world().get_resource::<EventLog>().unwrap().0.clone()
}

pub(crate) fn clear_log(app: &mut App) {
    app.world_mut()
        .get_resource_mut::<EventLog>()
        .unwrap()
        .0
        .clear();
}

// ── HSM: ToSuper / ToSub basic navigation ─────────────────────────

/// Creates a two-level HSM tree: Root → Child (Nested + Rebirth).
/// Returns (state_machine_id, root_state, child_state).
pub(crate) fn create_two_level_hsm(app: &mut App) -> (Entity, Entity, Entity) {
    register_log_systems(app);

    let world = app.world_mut();
    let root_state = world
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

    let child_state = world
        .spawn((
            Name::new("Child"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut tree = StateTree::new(root_state);
    tree.with_child(root_state, child_state);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("HSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root_state,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    (sm, root_state, child_state)
}

// ── HSM: Chain transitions (LCA-based) ────────────────────────────

/// Creates a tree: Root → A → A1, Root → B.
/// Returns (sm, root, a, a1, b).
pub(crate) fn create_deep_chain_hsm(app: &mut App) -> (Entity, Entity, Entity, Entity, Entity) {
    register_log_systems(app);

    let world = app.world_mut();
    let mut mk_state = |name: &str| {
        world
            .spawn((
                Name::new(name.to_string()),
                HsmState::with(
                    StateTransitionStrategy::Nested,
                    ExitTransitionBehavior::Rebirth,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
                OnUpdateSystem::new("Update:log_update"),
            ))
            .id()
    };

    let root = mk_state("Root");
    let a = mk_state("A");
    let a1 = mk_state("A1");
    let b = mk_state("B");

    let mut tree = StateTree::new(root);
    tree.with_child(root, a);
    tree.with_child(a, a1);
    tree.with_child(root, b);

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        tree,
        Name::new("ChainHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            sm,
            root,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    (sm, root, a, a1, b)
}

// ── HSM: Cross-tree interrupt ─────────────────────────────────────

/// Creates two independent HSM trees.
/// Returns (sm, tree_a_id, root_a, child_a, tree_b_id, root_b, child_b).
pub(crate) fn create_two_tree_hsm(
    app: &mut App,
) -> (Entity, Entity, Entity, Entity, Entity, Entity, Entity) {
    register_log_systems(app);

    let world = app.world_mut();

    let root_a = world
        .spawn((
            Name::new("RootA"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();
    let child_a = world
        .spawn((
            Name::new("ChildA"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut tree_a = StateTree::new(root_a);
    tree_a.with_child(root_a, child_a);
    let tree_a_id = world.spawn(tree_a).id();

    let root_b = world
        .spawn((
            Name::new("RootB"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();
    let child_b = world
        .spawn((
            Name::new("ChildB"),
            HsmState::with(
                StateTransitionStrategy::Nested,
                ExitTransitionBehavior::Rebirth,
            ),
            AfterEnterSystem::new("log_enter"),
            BeforeExitSystem::new("log_exit"),
            OnUpdateSystem::new("Update:log_update"),
        ))
        .id();

    let mut tree_b = StateTree::new(root_b);
    tree_b.with_child(root_b, child_b);
    let tree_b_id = world.spawn(tree_b).id();

    let sm = world.spawn_empty().id();
    world.entity_mut(sm).insert((
        Name::new("CrossTreeHSM"),
        StateLifecycle::default(),
        HsmStateMachine::with(
            tree_a_id,
            root_a,
            #[cfg(feature = "history")]
            10,
        ),
    ));

    (sm, tree_a_id, root_a, child_a, tree_b_id, root_b, child_b)
}

// ── FSM: Linear graph ─────────────────────────────────────────────

/// Creates a linear FSM: A → B → C, starting in A.
pub(crate) fn create_linear_fsm(app: &mut App) -> (Entity, Entity, Entity, Entity) {
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
    graph.with_add(state_b, state_c);

    let graph_id = world.spawn(graph).id();
    let sm = world
        .spawn(FsmStateMachine::with(
            graph_id,
            state_a,
            #[cfg(feature = "history")]
            10,
        ))
        .id();

    (sm, state_a, state_b, state_c)
}

// ── HSM: Guard helpers ────────────────────────────────────────────

#[derive(Component)]
pub(crate) struct AllowEnter(pub bool);

pub(crate) fn guard_allow_enter(ctx: In<GuardContext>, query: Query<&AllowEnter>) -> bool {
    query.get(ctx.state_machine).map(|a| a.0).unwrap_or(false)
}

#[derive(Component)]
pub(crate) struct AllowExit(pub bool);

pub(crate) fn guard_allow_exit(ctx: In<GuardContext>, query: Query<&AllowExit>) -> bool {
    query.get(ctx.state_machine).map(|a| a.0).unwrap_or(false)
}

#[derive(Component)]
pub(crate) struct GuardA(pub bool);

#[derive(Component)]
pub(crate) struct GuardB(pub bool);

pub(crate) fn guard_check_a(ctx: In<GuardContext>, query: Query<&GuardA>) -> bool {
    query.get(ctx.state_machine).map(|a| a.0).unwrap_or(false)
}

pub(crate) fn guard_check_b(ctx: In<GuardContext>, query: Query<&GuardB>) -> bool {
    query.get(ctx.state_machine).map(|b| b.0).unwrap_or(false)
}

// ── HSM: Transition system helpers ────────────────────────────────

pub(crate) fn log_before_enter(ctx: In<TransitionContext>, mut log: ResMut<EventLog>) {
    log.0.push(format!(
        "BeforeEnter:from={:?}:to={:?}",
        ctx.from_state(),
        ctx.to_state()
    ));
}

pub(crate) fn log_after_exit(ctx: In<TransitionContext>, mut log: ResMut<EventLog>) {
    log.0.push(format!(
        "AfterExit:from={:?}:to={:?}",
        ctx.from_state(),
        ctx.to_state()
    ));
}

// ── HSM: ServiceTarget helpers ────────────────────────────────────

#[derive(Component, Default)]
pub(crate) struct ActionFired(pub bool);

pub(crate) fn action_on_service_target(ctx: In<ActionContext>, mut commands: Commands) {
    commands
        .entity(ctx.service_target)
        .insert(ActionFired(true));
}

// ── FSM: Event/guard helpers ──────────────────────────────────────

pub(crate) const EVENT_GO_TO_B: i32 = 1;
pub(crate) const EVENT_GO_TO_C: i32 = 2;

#[derive(Component)]
pub(crate) struct FsmAllowGuard(pub bool);

pub(crate) fn fsm_guard(ctx: In<GuardContext>, query: Query<&FsmAllowGuard>) -> bool {
    query.get(ctx.state_machine).map(|g| g.0).unwrap_or(false)
}
