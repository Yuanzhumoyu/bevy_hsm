use bevy::prelude::*;

use crate::{
    context::GuardContext,
    error::{StateMachineError, error_event, error_event_world, warn_event, warn_event_world},
    guards::registry::CompiledGuard,
    guards::{GuardCondition, GuardRegistry},
    hsm::{
        HsmState,
        event::{HsmTrigger, HsmTriggerType},
        state_machine::HsmStateMachine,
        transition::Transition,
        transition_strategy::{
            build_enter_transition_plan, build_exit_transition_plan, handle_enter_transition,
            handle_exit_transition,
        },
    },
    markers::Paused,
    prelude::StateTree,
    state_machine::StateMachineState,
};

/// Observer handler for all HSM trigger events.
///
/// Dispatches the incoming [`HsmTrigger`] to the appropriate transition handler
/// based on its [`HsmTriggerType`].
pub(crate) fn handle_hsm_trigger(
    on: On<HsmTrigger>,
    mut commands: Commands,
    query_state_tree: Query<&StateTree>,
    query_state_machine: Populated<&HsmStateMachine, Without<Paused>>,
    guard_registry: Res<GuardRegistry>,
) {
    let HsmTrigger {
        state_machine,
        typed,
    } = on.event();
    let state_machine_id = *state_machine;

    let Ok(state_machine) = query_state_machine.get(state_machine_id) else {
        return;
    };

    let state_tree_id = state_machine.state_graph_id();

    match typed {
        HsmTriggerType::ToSuper => {
            handle_to_super(&mut commands, state_machine_id, state_tree_id);
        }
        HsmTriggerType::ToSub(enter_state_id) => {
            handle_to_sub(
                &mut commands,
                state_machine_id,
                state_tree_id,
                *enter_state_id,
            );
        }
        HsmTriggerType::Chain(next_state_id) => {
            handle_chain_transition(
                &mut commands,
                state_machine_id,
                state_tree_id,
                *next_state_id,
            );
        }
        HsmTriggerType::Interrupt(target_tree_id, interrupt_state_id) => {
            let target_tree_id = *target_tree_id;
            let interrupt_state_id = *interrupt_state_id;
            if target_tree_id == state_tree_id {
                // Same tree: validate LCA inside queue so curr_state is fresh.
                commands.queue(move |world: &mut World| {
                    let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
                        Some(sm) => sm.curr_state_id(),
                        None => return,
                    };
                    if curr_state_id == interrupt_state_id {
                        return;
                    }
                    let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
                        error_event_world(
                            world,
                            state_machine_id,
                            StateMachineError::StateTreeNotFound(state_tree_id),
                        );
                        return;
                    };
                    let Some((exit_path, enter_path)) =
                        state_tree.find_lca_and_paths(curr_state_id, interrupt_state_id)
                    else {
                        error_event_world(
                            world,
                            state_machine_id,
                            StateMachineError::LcaNotFound {
                                state_machine: state_machine_id,
                                from: curr_state_id,
                                to: interrupt_state_id,
                            },
                        );
                        return;
                    };
                    let Some(&lca) = exit_path.last() else {
                        return;
                    };
                    if let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) {
                        sm.interrupt_stack
                            .push_interrupt(state_tree_id, curr_state_id);
                    }
                    apply_chain_transitions(
                        world,
                        state_machine_id,
                        state_tree_id,
                        curr_state_id,
                        exit_path,
                        enter_path,
                        lca,
                    );
                });
            } else {
                // Cross-tree: validate target tree first, then queue push +
                // transition atomically.
                let Ok(target_tree) = query_state_tree.get(target_tree_id) else {
                    warn_event(
                        &mut commands,
                        state_machine_id,
                        StateMachineError::StateTreeNotFound(target_tree_id),
                    );
                    return;
                };
                let enter_path: Vec<Entity> = std::iter::once(interrupt_state_id)
                    .chain(target_tree.path_iter(interrupt_state_id))
                    .collect();
                commands.queue(move |world: &mut World| {
                    let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
                        Some(sm) => sm.curr_state_id(),
                        None => return,
                    };
                    if let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) {
                        sm.interrupt_stack
                            .push_interrupt(state_tree_id, curr_state_id);
                    }
                    apply_cross_tree_transition(
                        world,
                        state_machine_id,
                        curr_state_id,
                        state_tree_id,
                        target_tree_id,
                        enter_path,
                    );
                });
            }
        }
        HsmTriggerType::Resume => {
            commands.queue(move |world: &mut World| {
                // Peek first — validate before consuming the interrupt frame
                let frame = {
                    let Some(sm) = world.get::<HsmStateMachine>(state_machine_id) else {
                        return;
                    };
                    match sm.interrupt_stack.peek_interrupt() {
                        Some(frame) => frame,
                        None => {
                            error_event_world(
                                world,
                                state_machine_id,
                                StateMachineError::InterruptStackEmpty(state_machine_id),
                            );
                            return;
                        }
                    }
                };

                let (from_state, curr_tree) = match world.get::<HsmStateMachine>(state_machine_id) {
                    Some(sm) => (sm.curr_state_id(), sm.state_graph_id()),
                    None => return,
                };

                if from_state == frame.state_id && curr_tree == frame.graph_id {
                    // Already at the target — pop and discard the frame
                    if let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) {
                        sm.interrupt_stack.pop_interrupt();
                    }
                    return;
                }

                // Validate preconditions before popping
                if frame.graph_id == curr_tree {
                    // Same tree: validate LCA paths
                    let state_tree_id = curr_tree;
                    let paths = {
                        let Some(state_tree) = world.get::<StateTree>(curr_tree) else {
                            error_event_world(
                                world,
                                state_machine_id,
                                StateMachineError::StateTreeNotFound(curr_tree),
                            );
                            return;
                        };
                        let Some(paths) = state_tree.find_lca_and_paths(from_state, frame.state_id)
                        else {
                            error_event_world(
                                world,
                                state_machine_id,
                                StateMachineError::LcaNotFound {
                                    state_machine: state_machine_id,
                                    from: from_state,
                                    to: frame.state_id,
                                },
                            );
                            return;
                        };
                        paths
                    };

                    // All valid — now safe to pop
                    {
                        let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id)
                        else {
                            return;
                        };
                        sm.interrupt_stack.pop_interrupt();
                    }

                    let (exit_path, enter_path) = paths;
                    let Some(lca) = exit_path.last().copied() else {
                        error_event_world(
                            world,
                            state_machine_id,
                            StateMachineError::LcaNotFound {
                                state_machine: state_machine_id,
                                from: from_state,
                                to: frame.state_id,
                            },
                        );
                        return;
                    };
                    apply_chain_transitions(
                        world,
                        state_machine_id,
                        state_tree_id,
                        from_state,
                        exit_path,
                        enter_path,
                        lca,
                    );
                } else {
                    // Cross-tree resume: validate saved tree exists
                    let enter_path: Vec<Entity> = {
                        let Some(saved_tree) = world.get::<StateTree>(frame.graph_id) else {
                            error_event_world(
                                world,
                                state_machine_id,
                                StateMachineError::StateTreeNotFound(frame.graph_id),
                            );
                            return;
                        };
                        std::iter::once(frame.state_id)
                            .chain(saved_tree.path_iter(frame.state_id))
                            .collect()
                    };

                    // All valid — now safe to pop
                    {
                        let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id)
                        else {
                            return;
                        };
                        sm.interrupt_stack.pop_interrupt();
                    }

                    apply_cross_tree_transition(
                        world,
                        state_machine_id,
                        from_state,
                        curr_tree,
                        frame.graph_id,
                        enter_path,
                    );
                }
            });
        }
        _ => match typed {
            HsmTriggerType::GuardSub(guard, to_state_id) => {
                handle_guard_sub(
                    &mut commands,
                    state_machine_id,
                    state_tree_id,
                    *to_state_id,
                    guard,
                    &guard_registry,
                );
            }
            HsmTriggerType::GuardSuper(guard) => {
                handle_guard_super(
                    &mut commands,
                    state_machine_id,
                    state_tree_id,
                    guard,
                    &guard_registry,
                );
            }
            _ => unreachable!("Unexpected HsmTriggerType: {:?}", typed),
        },
    };
}

fn handle_to_super(commands: &mut Commands, state_machine_id: Entity, state_tree_id: Entity) {
    commands.queue(move |world: &mut World| {
        let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
            Some(sm) => sm.curr_state_id(),
            None => return,
        };
        let _ = handle_exit_transition(state_machine_id, state_tree_id, curr_state_id).apply(world);
    });
}

fn handle_to_sub(
    commands: &mut Commands,
    state_machine_id: Entity,
    state_tree_id: Entity,
    enter_state_id: Entity,
) {
    commands.queue(move |world: &mut World| {
        let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
            Some(sm) => sm.curr_state_id(),
            None => return,
        };
        let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::StateTreeNotFound(state_tree_id),
            );
            return;
        };
        if state_tree
            .get_sub_states(curr_state_id)
            .is_none_or(|sub_states| !sub_states.contains(&enter_state_id))
        {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::SubStateNotFound {
                    state_tree: state_tree_id,
                    state: curr_state_id,
                },
            );
            return;
        }
        let Some(strategy) = world.get::<HsmState>(curr_state_id).map(|s| s.strategy) else {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::HsmStateMissing(curr_state_id),
            );
            return;
        };
        let _ = handle_enter_transition(state_machine_id, curr_state_id, enter_state_id, strategy)
            .apply(world);
    });
}

/// Shared helper: run a compiled guard and delegate transition execution.
/// Handles GuardContext creation, guard evaluation, and error reporting.
/// The `execute` closure is called only when the guard returns `Ok(true)`.
fn run_guard_on_world(
    world: &mut World,
    state_tree_id: Entity,
    context: GuardContext,
    guard: &CompiledGuard,
    execute: impl FnOnce(&mut World, Entity, &GuardContext),
) {
    match guard.run(world, context) {
        Ok(true) => {
            execute(world, state_tree_id, &context);
        }
        Ok(false) => {}
        Err(e) => {
            error_event_world(
                world,
                context.state_machine,
                StateMachineError::GuardRunFailed {
                    state_machine: context.state_machine,
                    from_state: context.from_state(),
                    to_state: Some(context.to_state()),
                    source: e.to_string(),
                },
            );
        }
    }
}

fn handle_guard_super(
    commands: &mut Commands,
    state_machine_id: Entity,
    state_tree_id: Entity,
    guard: &GuardCondition,
    guard_registry: &GuardRegistry,
) {
    let guard = match guard_registry.to_combinator_condition_id(guard) {
        Ok(guard) => guard,
        Err(err) => {
            warn!("{}", err);
            error_event(
                commands,
                state_machine_id,
                StateMachineError::GuardNotFound {
                    condition: format!("{:?}", guard),
                    target: Entity::PLACEHOLDER,
                },
            );
            return;
        }
    };
    commands.queue(move |world: &mut World| {
        let from_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
            Some(sm) => sm.curr_state_id(),
            None => return,
        };
        let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::StateTreeNotFound(state_tree_id),
            );
            return;
        };
        let Some(to_state_id) = state_tree.get_super_state(from_state_id) else {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::SuperStateNotFound {
                    state_tree: state_tree_id,
                    state: from_state_id,
                },
            );
            return;
        };
        let service_target = crate::state_actions::get_service_target(world, state_machine_id);
        let context =
            GuardContext::new(service_target, state_machine_id, from_state_id, to_state_id);
        run_guard_on_world(
            world,
            state_tree_id,
            context,
            &guard,
            |world, tree_id, ctx| {
                let _ = handle_exit_transition(ctx.state_machine, tree_id, ctx.from_state())
                    .apply(world);
            },
        );
    });
}

fn handle_guard_sub(
    commands: &mut Commands,
    state_machine_id: Entity,
    state_tree_id: Entity,
    to_state_id: Entity,
    guard: &GuardCondition,
    guard_registry: &GuardRegistry,
) {
    let guard = match guard_registry.to_combinator_condition_id(guard) {
        Ok(guard) => guard,
        Err(err) => {
            warn!("{}", err);
            error_event(
                commands,
                state_machine_id,
                StateMachineError::GuardNotFound {
                    condition: format!("{:?}", guard),
                    target: to_state_id,
                },
            );
            return;
        }
    };
    commands.queue(move |world: &mut World| {
        let from_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
            Some(sm) => sm.curr_state_id(),
            None => return,
        };
        let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::StateTreeNotFound(state_tree_id),
            );
            return;
        };
        if state_tree
            .get_sub_states(from_state_id)
            .is_none_or(|sub_states| !sub_states.contains(&to_state_id))
        {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::SubStateNotFound {
                    state_tree: state_tree_id,
                    state: from_state_id,
                },
            );
            return;
        }
        let Some(strategy) = world.get::<HsmState>(from_state_id).map(|s| s.strategy) else {
            warn_event_world(
                world,
                state_machine_id,
                StateMachineError::HsmStateMissing(from_state_id),
            );
            return;
        };
        let service_target = crate::state_actions::get_service_target(world, state_machine_id);
        let context =
            GuardContext::new(service_target, state_machine_id, from_state_id, to_state_id);
        run_guard_on_world(
            world,
            state_tree_id,
            context,
            &guard,
            |world, _tree_id, ctx| {
                let _ = handle_enter_transition(
                    ctx.state_machine,
                    ctx.from_state(),
                    ctx.to_state(),
                    strategy,
                )
                .apply(world);
            },
        );
    });
}

/// Performs a cross-tree transition: exits the current state from the old tree,
/// switches the state machine to the new tree, and enters the target state.
/// Used by the Interrupt and Resume handlers when source and target are in
/// different state trees.
pub(crate) fn apply_cross_tree_transition(
    world: &mut World,
    state_machine_id: Entity,
    curr_state_id: Entity,
    old_tree_id: Entity,
    new_tree_id: Entity,
    enter_path: Vec<Entity>,
) {
    let mut transition_table = Vec::new();

    // ── Exit from old tree ────────────────────────────────────
    // Exit the current state, then cascade up to the root
    // (no LCA bound since we're leaving the tree entirely).

    // Force Death behavior: cross-tree exits must fully leave
    // the old tree, never re-enter or update via Rebirth/Resurrection.
    match build_exit_transition_plan(
        world,
        old_tree_id,
        curr_state_id,
        None, // exit all the way to root
        true,
    ) {
        Ok(ts) => transition_table.extend(ts),
        Err(e) => {
            error_event_world(world, state_machine_id, e);
            return;
        }
    }

    // ── Enter path in new tree ────────────────────────────────
    match build_enter_transition_plan(world, &enter_path) {
        Ok(enter_plan) => {
            transition_table.extend(enter_plan);
        }
        Err(err) => {
            error_event_world(world, state_machine_id, err);
            return;
        }
    };

    // ── Switch to new tree ────────────────────────────────────
    let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) else {
        error_event_world(
            world,
            state_machine_id,
            StateMachineError::HsmStateMachineMissing(state_machine_id),
        );
        return;
    };
    sm.state_tree = new_tree_id;

    // ── Apply first transition, queue the rest ───────────────
    let first = transition_table.first().copied().unwrap_or(Transition::End);
    let rest = &transition_table[1..];

    let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) else {
        error!(
            "{}",
            StateMachineError::HsmStateMachineMissing(state_machine_id)
        );
        return;
    };

    sm.push_next_states(rest.iter().copied());

    if let Some((state_id, lifecycle)) = first.to() {
        sm.set_curr_state(state_id);
        world.entity_mut(state_machine_id).insert(lifecycle);
    }
}

/// Builds and applies chain transition plans directly on the world.
/// Used by [`handle_chain_transition`] and the Resume interrupt handler.
pub(crate) fn apply_chain_transitions(
    world: &mut World,
    state_machine_id: Entity,
    state_tree_id: Entity,
    curr_state_id: Entity,
    exit_path: Vec<Entity>,
    enter_path: Vec<Entity>,
    lca: Entity,
) {
    let mut transition_table = Vec::new();

    // ── Exit path ────────────────────────────────────────────
    // exit_path = [curr_state, …, LCA]
    if exit_path.len() > 1 {
        match build_exit_transition_plan(world, state_tree_id, curr_state_id, Some(lca), true) {
            Ok(ts) => transition_table.extend(ts),
            Err(e) => {
                error_event_world(world, state_machine_id, e);
                return;
            }
        }
    }

    // ── Enter path ───────────────────────────────────────────
    match build_enter_transition_plan(world, &enter_path) {
        Ok(enter_plan) => {
            transition_table.extend(enter_plan);
        }
        Err(err) => {
            error_event_world(world, state_machine_id, err);
            return;
        }
    };

    // ── Apply first transition, queue the rest ───────────────
    let first = transition_table.first().copied().unwrap_or(Transition::End);
    let rest = &transition_table[1..];

    let Some(mut state_machine) = world.get_mut::<HsmStateMachine>(state_machine_id) else {
        error_event_world(
            world,
            state_machine_id,
            StateMachineError::HsmStateMachineMissing(state_machine_id),
        );
        return;
    };

    state_machine.push_next_states(rest.iter().copied());

    if let Some((state_id, lifecycle)) = first.to() {
        state_machine.set_curr_state(state_id);
        world.entity_mut(state_machine_id).insert(lifecycle);
    }
}

/// Queues a command that builds the chain transition plan using the unified
/// exit/enter helpers in [`transition_strategy`], then applies the first
/// transition immediately and queues the rest.
pub(crate) fn handle_chain_transition(
    commands: &mut Commands,
    state_machine_id: Entity,
    state_tree_id: Entity,
    next_state_id: Entity,
) {
    commands.queue(move |world: &mut World| {
        let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
            Some(sm) => sm.curr_state_id(),
            None => return,
        };
        if curr_state_id == next_state_id {
            return;
        }
        let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
            error_event_world(
                world,
                state_machine_id,
                StateMachineError::StateTreeNotFound(state_tree_id),
            );
            return;
        };
        let Some((exit_path, enter_path)) =
            state_tree.find_lca_and_paths(curr_state_id, next_state_id)
        else {
            error_event_world(
                world,
                state_machine_id,
                StateMachineError::LcaNotFound {
                    state_machine: state_machine_id,
                    from: curr_state_id,
                    to: next_state_id,
                },
            );
            return;
        };
        let Some(&lca) = exit_path.last() else {
            return;
        };
        apply_chain_transitions(
            world,
            state_machine_id,
            state_tree_id,
            curr_state_id,
            exit_path,
            enter_path,
            lca,
        );
    });
}
