use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{
    context::*,
    error::StateMachineError,
    fsm::{FsmState, event::FsmTrigger, graph::FsmGraph},
    markers::Paused,
    prelude::{ActionDispatch, FsmTriggerType, GetBufferId, GuardRegistry, StateActionBuffer},
    state_actions::*,
};

#[cfg(feature = "state_data")]
use crate::state_data::StateData;

#[cfg(feature = "history")]
use crate::fsm::history::*;

///# 有限状态机\Finite state machine
#[derive(Component)]
#[component(on_insert = Self::on_insert,on_remove = Self::on_remove)]
pub struct FsmStateMachine {
    pub graph_id: Entity,
    pub(super) init_state: Entity,
    pub(super) curr_state: Entity,
    #[cfg(feature = "history")]
    pub history: FsmStateHistory,
}

impl FsmStateMachine {
    #[cfg(feature = "history")]
    pub fn new(graph_id: Entity, init_state: Entity, history_size: usize) -> Self {
        Self {
            graph_id,
            init_state,
            curr_state: init_state,
            history: FsmStateHistory::new(history_size),
        }
    }

    #[cfg(not(feature = "history"))]
    pub fn new(graph_id: Entity, init_state: Entity) -> Self {
        Self {
            graph_id,
            init_state,
            curr_state: init_state,
        }
    }

    pub const fn curr_state(&self) -> Entity {
        self.curr_state
    }

    pub fn init_state(&self) -> Entity {
        self.init_state
    }

    /// 设置当前状态, 并记录历史
    ///
    /// Set current state and record history
    pub fn set_curr_state(&mut self, state: Entity) {
        #[cfg(feature = "history")]
        self.history.push(self.curr_state);
        self.curr_state = state;
    }

    #[cfg(feature = "history")]
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(fsm_state_machine) = world.get::<FsmStateMachine>(entity) else {
            error!("{}", StateMachineError::FsmStateMachineMissing(entity));
            return;
        };
        let curr_state = fsm_state_machine.curr_state;
        let service_target = match world.get::<ServiceTarget>(entity) {
            Some(service_target) => service_target.0,
            None => entity,
        };

        #[cfg(feature = "state_data")]
        StateData::clone_components(&mut world, curr_state, service_target);

        let context = StateActionContext::new(service_target, entity, curr_state);

        'on_enter: {
            let Some(on_enter) = world.get::<OnEnterSystem>(curr_state) else {
                break 'on_enter;
            };

            let Some(id) = world
                .resource::<StateActionRegistry>()
                .get(on_enter.as_str())
                .cloned()
            else {
                warn!(
                    "{}",
                    StateMachineError::SystemNotFound {
                        system_name: on_enter.to_string(),
                        state: curr_state
                    }
                );
                break 'on_enter;
            };

            unsafe {
                let _ = world
                    .as_unsafe_world_cell()
                    .world_mut()
                    .run_system_with(id, context);
            };
        };

        StateActionBuffer::buffer_scope(
            world.as_unsafe_world_cell(),
            curr_state,
            move |_world: &mut World, buffer: &mut StateActionBuffer| {
                buffer.add(context);
            },
        );
    }

    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(fsm_state_machine) = world.get::<FsmStateMachine>(entity) else {
            error!("{}", StateMachineError::FsmStateMachineMissing(entity));
            return;
        };

        let curr_state = fsm_state_machine.curr_state;
        let service_target = match world.get::<ServiceTarget>(entity) {
            Some(service_target) => service_target.0,
            None => entity,
        };

        #[cfg(feature = "state_data")]
        StateData::remove_components(&mut world, entity, service_target);

        let context = StateActionContext::new(service_target, entity, curr_state);

        'on_exit: {
            let Some(on_exit) = world.get::<OnExitSystem>(curr_state) else {
                break 'on_exit;
            };

            let Some(id) = world
                .resource::<StateActionRegistry>()
                .get(on_exit.as_str())
                .cloned()
            else {
                warn!(
                    "{}",
                    StateMachineError::SystemNotFound {
                        system_name: on_exit.to_string(),
                        state: curr_state
                    }
                );
                break 'on_exit;
            };
            unsafe {
                let _ = world
                    .as_unsafe_world_cell()
                    .world_mut()
                    .run_system_with(id, context);
            }
        };

        StateActionBuffer::buffer_scope(
            world.as_unsafe_world_cell(),
            curr_state,
            move |_world: &mut World, buffer: &mut StateActionBuffer| {
                buffer.remove_interceptor(context);
                buffer.add_filter(context);
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_fsm_trigger(
        on: On<FsmTrigger>,
        mut commands: Commands,
        guard_registry: Res<GuardRegistry>,
        action_dispatch: Res<ActionDispatch>,
        action_registry: Res<StateActionRegistry>,
        mut query: Query<&mut FsmStateMachine, Without<Paused>>,
        query_service_target: Query<&ServiceTarget, With<FsmStateMachine>>,
        query_on_update_system: Query<&OnUpdateSystem, With<FsmState>>,
        query_on_enter_system: Query<&OnEnterSystem, With<FsmState>>,
        query_on_exit_system: Query<&OnExitSystem, With<FsmState>>,
        #[cfg(feature = "state_data")] query_state_data: Query<&StateData, With<FsmState>>,
        fsm_graph: Query<&FsmGraph>,
    ) {
        let FsmTrigger {
            state_machine: state_machine_id,
            typed,
        } = on.event().clone();

        let Ok(mut state_machine) = query.get_mut(state_machine_id) else {
            error!(
                "{}",
                StateMachineError::FsmStateMachineMissing(state_machine_id)
            );
            return;
        };
        let Ok(fsm_graph) = fsm_graph.get(state_machine.graph_id) else {
            error!(
                "{}",
                StateMachineError::GraphNotFound(state_machine.graph_id)
            );
            return;
        };
        let Some(state_transitions) = fsm_graph.get(state_machine.curr_state) else {
            error!(
                "{}",
                StateMachineError::StateNotInGraph {
                    graph: state_machine.graph_id,
                    state: state_machine.curr_state
                }
            );
            return;
        };

        let on_update_system = |state: Entity| -> Option<GetBufferId> {
            let Ok(on_update) = query_on_update_system.get(state) else {
                return None;
            };
            let Some(get_buffer_id) = action_dispatch.get(on_update.as_str()) else {
                warn!(
                    "{}",
                    StateMachineError::SystemNotFound {
                        system_name: on_update.to_string(),
                        state
                    }
                );
                return None;
            };
            Some(get_buffer_id)
        };

        let mut run_life_cycle_system = |to: Entity| {
            let from = state_machine.curr_state;
            let service_target = match query_service_target.get(state_machine_id) {
                Ok(service_target) => service_target.0,
                Err(_) => state_machine_id,
            };

            #[cfg(feature = "state_data")]
            if let Ok(state_data) = query_state_data.get(from).cloned() {
                commands.queue(state_data.remove_state_data_command(service_target));
            }

            let context = StateActionContext::new(service_target, state_machine_id, from);

            'on_remove_update: {
                if !query_on_update_system.contains(from) {
                    break 'on_remove_update;
                }
                if let Some(get_buff_id) = on_update_system(from) {
                    commands.queue(move |world: &mut World| {
                        (get_buff_id)(
                            world,
                            Box::new(move |_world, buffer| {
                                buffer.remove_interceptor(context);
                                buffer.add_filter(context);
                            }),
                        )
                    });
                }
            };

            'on_exit: {
                let Ok(on_exit) = query_on_exit_system.get(from) else {
                    break 'on_exit;
                };
                let Some(id) = action_registry.get(on_exit.as_str()).cloned() else {
                    warn!(
                        "{}",
                        StateMachineError::SystemNotFound {
                            system_name: on_exit.to_string(),
                            state: from
                        }
                    );
                    break 'on_exit;
                };

                commands.run_system_with(id, context);
            }

            state_machine.set_curr_state(to);

            #[cfg(feature = "state_data")]
            if let Ok(state_data) = query_state_data.get(to).cloned() {
                commands.queue(state_data.clone_state_data_command(to, service_target))
            }

            let context = StateActionContext::new(service_target, state_machine_id, to);

            'on_enter: {
                let Ok(on_enter) = query_on_enter_system.get(to) else {
                    break 'on_enter;
                };
                let Some(id) = action_registry.get(on_enter.as_str()).cloned() else {
                    warn!(
                        "{}",
                        StateMachineError::SystemNotFound {
                            system_name: on_enter.to_string(),
                            state: to
                        }
                    );
                    break 'on_enter;
                };

                commands.run_system_with(id, context);
            }

            'on_add_update: {
                if !query_on_update_system.contains(to) {
                    break 'on_add_update;
                }
                if let Some(get_buff_id) = on_update_system(to) {
                    commands.queue(move |world: &mut World| {
                        (get_buff_id)(
                            world,
                            Box::new(move |_world, buffer| {
                                buffer.add(context);
                            }),
                        )
                    });
                }
            };
        };
        match typed {
            FsmTriggerType::Transition(target) => {
                let Some(condition) = state_transitions.get_by_condition(target) else {
                    return;
                };
                let Some(id) = guard_registry.to_combinator_condition_id(condition) else {
                    warn!(
                        "[GuardRegistry] This condition<{:?}> does not exist for state {:?}",
                        condition, target
                    );
                    return;
                };
                let service_target = match query_service_target.get(state_machine_id) {
                    Ok(service_target) => service_target.0,
                    Err(_) => state_machine_id,
                };
                let context = GuardContext::new(
                    service_target,
                    state_machine_id,
                    state_machine.curr_state,
                    target,
                );
                let remove_buffer_id = on_update_system(state_machine.curr_state).map(|id| {
                    (
                        id,
                        StateActionContext::new(
                            service_target,
                            state_machine_id,
                            state_machine.curr_state,
                        ),
                    )
                });

                let on_exit_system_id = 'on_exit: {
                    let Ok(on_exit) = query_on_exit_system.get(state_machine.curr_state) else {
                        break 'on_exit None;
                    };

                    match action_registry.get(on_exit.as_str()) {
                        Some(id) => Some((
                            *id,
                            StateActionContext::new(
                                service_target,
                                state_machine_id,
                                state_machine.curr_state,
                            ),
                        )),
                        None => {
                            warn!(
                                "{}",
                                StateMachineError::SystemNotFound {
                                    system_name: on_exit.to_string(),
                                    state: state_machine.curr_state
                                }
                            );
                            None
                        }
                    }
                };

                let on_enter_system_id = 'on_enter: {
                    let Ok(on_enter) = query_on_enter_system.get(target) else {
                        break 'on_enter None;
                    };

                    match action_registry.get(on_enter.as_str()) {
                        Some(id) => Some((
                            *id,
                            StateActionContext::new(service_target, state_machine_id, target),
                        )),
                        None => {
                            warn!(
                                "{}",
                                StateMachineError::SystemNotFound {
                                    system_name: on_enter.to_string(),
                                    state: target
                                }
                            );
                            None
                        }
                    }
                };

                let add_buffer_id = on_update_system(target).map(|id| {
                    (
                        id,
                        StateActionContext::new(service_target, state_machine_id, target),
                    )
                });

                commands.queue(move |world: &mut World| {
                    match id.run(world, context) {
                        Ok(true) => {
                            if let Some((system, state_context)) = remove_buffer_id {
                                (system)(
                                    world,
                                    Box::new(move |_world, buffer| {
                                        buffer.remove_interceptor(state_context);
                                        buffer.add_filter(state_context);
                                    }),
                                );
                            }

                            if let Some((id, context)) = on_exit_system_id {
                                #[cfg(feature = "state_data")]
                                if let Some(state_data) =
                                    world.get::<StateData>(context.state()).cloned()
                                {
                                    state_data
                                        .remove_state_data_command(context.service_target)
                                        .apply(world);
                                }

                                let _ = world.run_system_with(id, context);
                            }

                            if let Some(mut state_machine) =
                                world.get_mut::<FsmStateMachine>(state_machine_id)
                            {
                                state_machine.set_curr_state(target);
                            }

                            if let Some((id, context)) = on_enter_system_id {
                                #[cfg(feature = "state_data")]
                                if let Some(state_date) =
                                    world.get::<StateData>(context.state()).cloned()
                                {
                                    state_date
                                        .clone_state_data_command(
                                            context.state(),
                                            context.service_target,
                                        )
                                        .apply(world);
                                }

                                let _ = world.run_system_with(id, context);
                            }

                            if let Some((system, state_context)) = add_buffer_id {
                                (system)(
                                    world,
                                    Box::new(move |_world, buffer| {
                                        buffer.add(state_context);
                                    }),
                                )
                            }
                        }
                        Ok(false) => {}
                        Err(e) => error!("{}", e),
                    };
                });
            }
            FsmTriggerType::Next(target) => {
                if state_transitions.contains(target) {
                    run_life_cycle_system(target);
                } else {
                    trace!(
                        "{}",
                        StateMachineError::InvalidTransitionTarget {
                            graph: state_machine.graph_id,
                            from_state: state_machine.curr_state,
                            to_state: target
                        }
                    );
                }
            }
            FsmTriggerType::Event(fsm_on_event) => {
                if let Some(target) = state_transitions.get_by_event(fsm_on_event.as_ref()) {
                    run_life_cycle_system(target);
                }
            }
        };
    }
}

///# 有限状态机初始化配置/Finite state machine initial configuration
#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsmBlueprint {
    pub graph_id: Entity,
    pub init_state: Entity,
    pub curr_state: Option<Entity>,
    #[cfg(feature = "history")]
    pub history_size: usize,
}

impl FsmBlueprint {
    #[cfg(feature = "history")]
    pub fn new(graph_id: Entity, init_state: Entity, history_size: usize) -> Self {
        Self {
            graph_id,
            init_state,
            curr_state: None,
            history_size,
        }
    }

    #[cfg(not(feature = "history"))]
    pub fn new(graph_id: Entity, init_state: Entity) -> Self {
        Self {
            graph_id,
            init_state,
            curr_state: None,
        }
    }

    pub fn with_curr_state(mut self, curr_state: Entity) -> Self {
        self.curr_state = Some(curr_state);
        self
    }
}
