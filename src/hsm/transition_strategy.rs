use bevy::{ecs::schedule::ScheduleLabel, platform::collections::HashSet, prelude::*};

use crate::{
    context::GuardContext,
    error::{StateMachineError, error_event_world, warn_event, warn_event_world},
    guards::registry::CompiledGuardRegistry,
    hsm::{
        HsmState,
        state_lifecycle::StateLifecycle,
        state_machine::*,
        state_tree::StateTree,
        strategy::{get_hsm_state, get_state_tree},
        transition::Transition,
    },
    markers::*,
    prelude::{GuardEnter, GuardEnterCache, GuardExit, GuardExitCache},
    state_machine::StateMachineState,
};

pub use super::strategy::{
    ExitTransitionBehavior, ReverseTraversal, SequentialTraversal, StateTransitionStrategy,
    StateTraversalStrategy, TraversalStrategy,
};

/// Builds the exit transition plan for a state given its strategy and behavior.
///
/// The optional `stop_at` parameter limits how far up the tree the exit cascade
/// can propagate. When `Some(lca)`, the recursion stops before exiting `lca`.
/// When `None` (used by `handle_exit_transition`), the cascade runs to the root.
pub(crate) fn build_exit_transition_plan(
    world: &World,
    state_tree_id: Entity,
    mut state_id: Entity,
    stop_at: Option<Entity>,
    force_death: bool,
) -> Result<Vec<Transition>, StateMachineError> {
    // Helper: check if we've hit the boundary.
    let at_boundary = |id: Entity| stop_at.is_some_and(|b| b == id);
    let state_tree = get_state_tree(world, state_tree_id)?;
    let mut transitions = Vec::new();
    transitions.push(Transition::Exit(state_id));

    if force_death {
        while let Some(super_state) = state_tree.get_super_state(state_id)
            && !at_boundary(super_state)
        {
            let hsm_state = get_hsm_state(world, super_state)?;

            match hsm_state.strategy {
                StateTransitionStrategy::Nested | StateTransitionStrategy::Parallel => {
                    transitions.push(Transition::Exit(super_state));
                }
            }

            state_id = super_state;
        }
    } else {
        while let Some(super_state) = state_tree.get_super_state(state_id)
            && !at_boundary(super_state)
        {
            let hsm_state = get_hsm_state(world, super_state)?;
            match (hsm_state.strategy, hsm_state.behavior) {
                (
                    StateTransitionStrategy::Nested | StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Resurrection,
                ) => {
                    transitions.push(Transition::Update(super_state));
                    break;
                }
                (
                    StateTransitionStrategy::Nested | StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Rebirth,
                ) => {
                    transitions.push(Transition::Enter(super_state));
                    break;
                }
                (StateTransitionStrategy::Nested, ExitTransitionBehavior::Death) => {
                    transitions.push(Transition::Exit(super_state));
                }
                (StateTransitionStrategy::Parallel, ExitTransitionBehavior::Death) => {}
            }

            state_id = super_state;
        }
    }

    Ok(transitions)
}

/// Builds the enter transition plan from an LCA-to-target path.
///
/// `enter_path` is `[target, parent_of_target, ..., LCA]` as returned by
/// [`StateTree::find_lca_and_paths`]. Returns transitions in execution order:
/// first the LCA is entered, then each intermediate state, ending with the target.
pub(crate) fn build_enter_transition_plan(
    world: &World,
    enter_path: &[Entity],
) -> Result<Vec<Transition>, StateMachineError> {
    // Single-element path: used by cross-tree transitions when the target
    // is the root of the destination tree. Just enter it directly.
    if enter_path.len() == 1 {
        return Ok(vec![Transition::Enter(enter_path[0])]);
    }

    let mut transitions = Vec::with_capacity(enter_path.len() * 2);

    // enter_path = [target, ..., child_of_lca, LCA]
    // Reverse → [LCA, child_of_lca, ..., target], then slide windows
    for (i, [sub_state_id, curr_state_id]) in
        enter_path.array_windows::<2>().rev().copied().enumerate()
    {
        let hsm = get_hsm_state(world, curr_state_id)?;

        if hsm.strategy == StateTransitionStrategy::Parallel && (i != 0 || enter_path.len() == 2) {
            transitions.push(Transition::Exit(curr_state_id));
        }
        transitions.push(Transition::Enter(sub_state_id));
    }

    Ok(transitions)
}

/// 检查能否过渡状态的实体
///
/// Check whether the entity can transition
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq, Deref, DerefMut)]
pub(crate) struct CheckOnTransitionStates(HashSet<Entity>);

/// 在指定的调度中安装状态转换系统。
///
/// # Arguments
///
/// * `app` - Bevy 应用实例。
/// * `schedule` - 要安装系统的调度标签。
pub(crate) fn install_transition_systems<T: ScheduleLabel>(app: &mut App, schedule: T) {
    app.add_systems(
        schedule,
        (handle_enter_transitions, handle_exit_transitions)
            .chain()
            .run_if(|check_on_transition_states: Res<CheckOnTransitionStates>| {
                !check_on_transition_states.is_empty()
            }),
    );
}

fn handle_enter_transitions(
    mut commands: Commands,
    check_on_transition_states: Res<CheckOnTransitionStates>,
    query_state_machines: Populated<(Entity, &HsmStateMachine), Without<Paused>>,
    query_states: Query<&HsmState, With<HsmState>>,
) {
    for (state_machine_id, state_machine) in
        query_state_machines.iter_many(check_on_transition_states.iter())
    {
        let curr_state_id = state_machine.curr_state_id();
        let state_tree_id = state_machine.state_graph_id();
        let Ok(strategy) = query_states
            .get(curr_state_id)
            .map(|hsm_state| hsm_state.strategy)
        else {
            warn_event(
                &mut commands,
                state_machine_id,
                StateMachineError::HsmStateMissing(curr_state_id),
            );
            continue;
        };
        commands.queue(move |world: &mut World| {
            let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::StateTreeNotFound(state_tree_id),
                );
                return;
            };
            let sub_state_iter = state_tree.traversal_iter_with(world, curr_state_id, |e| {
                if !e.contains::<HsmState>() {
                    warn!("{}", StateMachineError::HsmStateMissing(e.id()));
                    return false;
                }
                e.contains::<GuardEnter>()
            });
            let Some(enter_state_id) = world.resource_scope(
                |world: &mut World, compiled_guard_registry: Mut<CompiledGuardRegistry>| {
                    world.resource_scope(
                        |world: &mut World, condition_buffer: Mut<GuardEnterCache>| {
                            find_enter_state_by_guards(
                                world,
                                &compiled_guard_registry,
                                &condition_buffer,
                                sub_state_iter,
                                state_machine_id,
                                curr_state_id,
                            )
                        },
                    )
                },
            ) else {
                return;
            };

            let _ =
                handle_enter_transition(state_machine_id, curr_state_id, enter_state_id, strategy)
                    .apply(world);
        });
    }
}

/// Searches sub-states ordered by traversal strategy, running guard conditions
/// via `GuardEnterCache`. Returns the first sub-state whose guard passes.
fn find_enter_state_by_guards(
    world: &mut World,
    compiled_guard_registry: &CompiledGuardRegistry,
    condition_buffer: &GuardEnterCache,
    sub_state_iter: Vec<Entity>,
    state_machine_id: Entity,
    curr_state_id: Entity,
) -> Option<Entity> {
    for sub_state_id in sub_state_iter {
        let Some(condition_id) = condition_buffer.get(&sub_state_id) else {
            continue;
        };
        let Some(guard_id) = compiled_guard_registry.get_by_id(*condition_id) else {
            continue;
        };
        let service_target = crate::state_actions::get_service_target(world, state_machine_id);
        match guard_id.run(
            world,
            GuardContext::new(
                service_target,
                state_machine_id,
                curr_state_id,
                sub_state_id,
            ),
        ) {
            Ok(true) => return Some(sub_state_id),
            Ok(false) => continue,
            Err(e) => {
                error_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::GuardRunFailed {
                        state_machine: state_machine_id,
                        from_state: curr_state_id,
                        to_state: Some(sub_state_id),
                        source: e.to_string(),
                    },
                );
                continue;
            }
        }
    }
    None
}

/// 处理进入转换：将状态机切换到子状态，根据策略决定是嵌套还是平级。
///
/// Handles enter transitions: switches the state machine to a child state,
/// deciding between nested or parallel based on the strategy.
///
/// - `Nested`: 当前状态直接切换到子状态，触发 `Enter` 生命周期。
/// - `Parallel`: 当前状态保持，子状态进入队列，触发 `Exit` 生命周期。
pub(super) fn handle_enter_transition(
    state_machine_id: Entity,
    curr_state_id: Entity,
    enter_state_id: Entity,
    strategy: StateTransitionStrategy,
) -> impl Command<Out = Result<()>> {
    move |world: &mut World| {
        world
            .resource_mut::<CheckOnTransitionStates>()
            .remove(&state_machine_id);

        let mut service_target = world.entity_mut(state_machine_id);
        let Some(mut state_machine) = service_target.get_mut::<HsmStateMachine>() else {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::HsmStateMachineMissing(state_machine_id),
            );
            return Ok(());
        };

        let next_on_state: StateLifecycle = match strategy {
            StateTransitionStrategy::Nested => {
                state_machine.set_curr_state(enter_state_id);
                StateLifecycle::Enter
            }
            StateTransitionStrategy::Parallel => {
                state_machine.set_curr_state(curr_state_id);
                state_machine.push_next_state(Transition::Enter(enter_state_id));
                StateLifecycle::Exit
            }
        };

        service_target.insert(next_on_state);
        Ok(())
    }
}

fn handle_exit_transitions(
    mut commands: Commands,
    check_on_transition_states: Res<CheckOnTransitionStates>,
    query_state_machines: Populated<(Entity, &HsmStateMachine), Without<Paused>>,
    query_on_exit_conditions: Query<Has<GuardExit>, With<HsmState>>,
    query_state_trees: Query<&StateTree>,
) {
    // 条件为空的状态
    for (state_machine_id, state_machine) in
        query_state_machines.iter_many(check_on_transition_states.iter())
    {
        let curr_state_id = state_machine.curr_state_id();
        let state_tree_id = state_machine.state_graph_id();
        let Ok(true) = query_on_exit_conditions.get(curr_state_id) else {
            continue;
        };
        let Ok(state_tree) = query_state_trees.get(state_tree_id) else {
            warn_event(
                &mut commands,
                state_machine_id,
                StateMachineError::StateTreeNotFound(state_tree_id),
            );
            continue;
        };
        let Some(super_state_id) = state_tree.get_super_state(curr_state_id) else {
            warn_event(
                &mut commands,
                state_machine_id,
                StateMachineError::SuperStateNotFound {
                    state_tree: state_tree_id,
                    state: curr_state_id,
                },
            );
            continue;
        };
        commands.queue(move |world: &mut World| -> Result<()> {
            match world.resource_scope(
                |world: &mut World, compiled_guard_registry: Mut<CompiledGuardRegistry>| {
                    world.resource_scope(
                        |world: &mut World, exit_guard_cache: Mut<GuardExitCache>| {
                            match exit_guard_cache
                                .get(&curr_state_id)
                                .and_then(|guard_id| compiled_guard_registry.get_by_id(*guard_id))
                            {
                                Some(guard_id) => {
                                    let service_target = crate::state_actions::get_service_target(
                                        world,
                                        state_machine_id,
                                    );
                                    guard_id.run(
                                        world,
                                        GuardContext::new(
                                            service_target,
                                            state_machine_id,
                                            curr_state_id,
                                            super_state_id,
                                        ),
                                    )
                                }
                                None => Ok(false),
                            }
                        },
                    )
                },
            ) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(e) => {
                    error_event_world(
                        world,
                        state_machine_id,
                        StateMachineError::GuardRunFailed {
                            state_machine: state_machine_id,
                            from_state: curr_state_id,
                            to_state: None,
                            source: e.to_string(),
                        },
                    );
                    return Ok(());
                }
            };

            handle_exit_transition(state_machine_id, state_tree_id, curr_state_id).apply(world)
        });
    }
}

/// 处理退出转换：从当前状态退出到父状态，构建退出计划并推送到状态机队列。
///
/// Handles exit transitions: exits from the current state to its parent state,
/// builds an exit plan and pushes it to the state machine's transition queue.
#[inline]
pub(super) fn handle_exit_transition(
    state_machine_id: Entity,
    state_tree_id: Entity,
    curr_state_id: Entity,
) -> impl Command<Out = Result<()>> {
    move |world: &mut World| -> Result<()> {
        world
            .resource_mut::<CheckOnTransitionStates>()
            .remove(&state_machine_id);

        let transition_queue =
            build_exit_transition_plan(world, state_tree_id, curr_state_id, None, false)?;

        let mut service_target = world.entity_mut(state_machine_id);
        let Some(mut state_machine) = service_target.get_mut::<HsmStateMachine>() else {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::HsmStateMachineMissing(state_machine_id),
            );
            return Ok(());
        };

        state_machine.push_next_states(transition_queue[1..].iter().copied());
        state_machine.set_curr_state(curr_state_id);
        service_target.insert(StateLifecycle::Exit);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/hsm_transition_strategy_tests.rs"]
mod tests;
