use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

#[cfg(feature = "history")]
use crate::hsm::history::HistoricalNode;
#[cfg(feature = "state_data")]
use crate::prelude::StateData;
use crate::{
    context::{ActionContext, TransitionContext},
    error::StateMachineError,
    hsm::state_machine::*,
    labels::SystemLabel,
    markers::Terminated,
    prelude::{
        ActionRegistry, AfterEnterSystem, AfterExitSystem, BeforeEnterSystem, BeforeExitSystem,
        CheckOnTransitionStates, OnUpdateSystem, ServiceTarget, StateActionBuffer,
        TransitionRegistry,
    },
};

struct TransitionInfo {
    state_context: ActionContext,
    state_machine_id: Entity,
    prev_transition: Transition,
    curr_transition: Transition,
    curr_state_id: Entity,
    hsm_state: StateLifecycle,
}

/// # 状态变化检测组件\State Change Detection Component
/// * 用于检测状态变化，实时更新状态机的状态
/// - Used for detecting state changes and updating the state machine's state in real time
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(immutable, storage = "SparseSet", on_insert = Self::on_insert)]
pub enum StateLifecycle {
    /// 进入状态\Enter State
    #[default]
    Enter,
    /// 更新状态\Update State
    Update,
    /// 退出状态\Exit State
    Exit,
}

impl StateLifecycle {
    fn run_lifecycle_system<T: Component + std::ops::Deref<Target = SystemLabel>>(
        world: &mut DeferredWorld,
        state_id: Entity,
        state_context: ActionContext,
    ) {
        let Some(action_system_id) = ActionRegistry::get_action_id::<T>(world, state_id) else {
            return;
        };

        if let Err(e) = state_context.run_system(world, action_system_id) {
            let Some(system_name) = world.get::<T>(state_id) else {
                return;
            };
            error!("{}", system_name.run_failed_error(state_id, e.into()));
        }
    }

    fn run_transition_lifecycle_system<T: Component + std::ops::Deref<Target = SystemLabel>>(
        world: &mut DeferredWorld,
        state_id: Entity,
        state_context: TransitionContext,
    ) {
        let Some(action_system_id) = TransitionRegistry::get_transition_id::<T>(world, state_id)
        else {
            return;
        };
        if let Err(e) = state_context.run_system(world, action_system_id) {
            let Some(system_name) = world.get::<T>(state_id) else {
                return;
            };
            error!("{}", system_name.run_failed_error(state_id, e.into()));
        }
    }

    #[cfg(feature = "hybrid")]
    fn handle_hybrid_entry(world: &mut DeferredWorld, state_machine_id: Entity, state_id: Entity) {
        use crate::{fsm::state_machine::NestedFsm, hsm::HsmState, prelude::FsmGraph};

        let Some(state) = world.get::<HsmState>(state_id) else {
            error!("{}", StateMachineError::HsmStateMissing(state_id));
            return;
        };
        let Some(fsm_config) = state.fsm_config else {
            return;
        };

        let Some(init_state) = world
            .get::<FsmGraph>(fsm_config.graph_id)
            .map(|graph| graph.init_state())
        else {
            error!("{}", StateMachineError::GraphMissing(fsm_config.graph_id));
            return;
        };

        let curr_state = match fsm_config.curr_state {
            Some(state) => state,
            None => init_state,
        };

        world.commands().spawn((
            NestedFsm::new(state_machine_id, state_id),
            crate::fsm::state_machine::FsmStateMachine::new(
                fsm_config.graph_id,
                init_state,
                curr_state,
                #[cfg(feature = "history")]
                fsm_config.history_size,
            ),
        ));
    }

    #[cfg(feature = "hybrid")]
    fn handle_hybrid_exit(world: &mut DeferredWorld, state_machine_id: Entity, state_id: Entity) {
        use crate::{fsm::state_machine::HsmOwnedFsms, prelude::FsmStateMachine};

        let Some(mut mapping) = world.get_mut::<HsmOwnedFsms>(state_machine_id) else {
            return;
        };

        let Some(_fsm_state_machine) = mapping.0.remove(&state_id) else {
            return;
        };

        if mapping.is_empty() {
            world
                .commands()
                .entity(state_machine_id)
                .remove::<HsmOwnedFsms>();
        }

        #[cfg(feature = "history")]
        if let Ok([mut state_machine_mut, mut fsm_state_machine_mut]) =
            world.get_entity_mut([state_machine_id, _fsm_state_machine])
            && let Some(mut hsm) = state_machine_mut.get_mut::<HsmStateMachine>()
            && let Some(mut fsm) = fsm_state_machine_mut.get_mut::<FsmStateMachine>()
        {
            hsm.history
                .set_last_state_fsm_history(state_id, fsm.history.take());
        }

        world.commands().entity(_fsm_state_machine).despawn();
    }

    fn prepare_transition(
        world: &mut DeferredWorld,
        hook_context: HookContext,
    ) -> Option<TransitionInfo> {
        let state_machine_id = hook_context.entity;

        let Ok(mut entity_mut) = world.get_entity_mut(state_machine_id) else {
            error!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return None;
        };

        let Some(lifecycle) = entity_mut.get::<StateLifecycle>().copied() else {
            warn!(
                "{}",
                StateMachineError::StateLifecycleMissing(state_machine_id)
            );
            return None;
        };

        let service_target = match entity_mut.get::<ServiceTarget>() {
            Some(service_target) => service_target.0,
            None => state_machine_id,
        };

        let Some(mut state_machine) = entity_mut.get_mut::<HsmStateMachine>() else {
            warn!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return None;
        };

        let curr_state_id = state_machine.curr_state_id();
        let curr = Transition::with_lifecycle(curr_state_id, lifecycle);
        let prev = state_machine.push_prev_state(curr);
        #[cfg(feature = "history")]
        state_machine.push_history(HistoricalNode::new(curr_state_id, lifecycle.into()));

        let state_context = ActionContext::new(service_target, state_machine_id, curr_state_id);

        Some(TransitionInfo {
            state_machine_id,
            prev_transition: prev,
            curr_transition: curr,
            curr_state_id,
            state_context,
            hsm_state: lifecycle,
        })
    }

    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let Some(TransitionInfo {
            state_machine_id,
            prev_transition,
            curr_transition,
            curr_state_id,
            state_context,
            hsm_state,
        }) = Self::prepare_transition(&mut world, hook_context)
        else {
            return;
        };

        match hsm_state {
            StateLifecycle::Enter => {
                let Some(relationship) = prev_transition.to_transition(curr_transition) else {
                    return;
                };

                // 运行进入之前的系统
                Self::run_transition_lifecycle_system::<BeforeEnterSystem>(
                    &mut world,
                    curr_state_id,
                    TransitionContext::with(
                        state_context.service_target,
                        state_machine_id,
                        relationship,
                    ),
                );

                #[cfg(feature = "hybrid")]
                Self::handle_hybrid_entry(&mut world, state_machine_id, curr_state_id);

                #[cfg(feature = "state_data")]
                StateData::clone_components(
                    &mut world,
                    curr_state_id,
                    state_context.service_target,
                );

                // 运行进入后的系统
                Self::run_lifecycle_system::<AfterEnterSystem>(
                    &mut world,
                    curr_state_id,
                    state_context,
                );

                world
                    .commands()
                    .entity(state_machine_id)
                    .insert(StateLifecycle::Update);
            }
            StateLifecycle::Update => {
                // 添加过渡条件检查系统
                let mut check_on_transition_states =
                    world.resource_mut::<CheckOnTransitionStates>();
                check_on_transition_states.insert(state_machine_id);

                // 运行更新系统
                if world.entity(curr_state_id).contains::<OnUpdateSystem>() {
                    StateActionBuffer::buffer_scope(
                        world.as_unsafe_world_cell(),
                        curr_state_id,
                        move |_world, buff| {
                            buff.remove_filter(state_context);
                            buff.add(state_context);
                        },
                    );
                }
            }
            StateLifecycle::Exit => {
                // 过滤条件
                StateActionBuffer::buffer_scope(
                    world.as_unsafe_world_cell(),
                    curr_state_id,
                    move |_world, buff| {
                        buff.remove_interceptor(state_context);
                        buff.add_filter(state_context);
                    },
                );

                // 运行退出之前的系统
                Self::run_lifecycle_system::<BeforeExitSystem>(
                    &mut world,
                    curr_state_id,
                    state_context,
                );

                #[cfg(feature = "hybrid")]
                Self::handle_hybrid_exit(&mut world, state_machine_id, curr_state_id);

                #[cfg(feature = "state_data")]
                StateData::remove_components(
                    &mut world,
                    curr_state_id,
                    state_context.service_target,
                );

                world.commands().queue(move |world: &mut World| {
                    let Some(mut state_machine) =
                        world.get_mut::<HsmStateMachine>(state_machine_id)
                    else {
                        warn!(
                            "{}",
                            StateMachineError::HsmStateMachineMissing(state_machine_id)
                        );
                        return;
                    };
                    let next_transition = state_machine.pop_next_state();

                    let Some(relationship) = curr_transition.to_transition(next_transition) else {
                        return;
                    };

                    match next_transition.to() {
                        Some((curr_state, on_state)) => {
                            state_machine.set_curr_state(curr_state);
                            Self::run_transition_lifecycle_system::<AfterExitSystem>(
                                &mut world.into(),
                                curr_state_id,
                                TransitionContext::with(
                                    state_context.service_target,
                                    state_machine_id,
                                    relationship,
                                ),
                            );
                            world.entity_mut(state_machine_id).insert(on_state);
                        }
                        None => {
                            Self::run_transition_lifecycle_system::<AfterExitSystem>(
                                &mut world.into(),
                                curr_state_id,
                                TransitionContext::with(
                                    state_context.service_target,
                                    state_machine_id,
                                    relationship,
                                ),
                            );
                            world.entity_mut(state_machine_id).insert(Terminated);
                        }
                    };
                });
            }
        };

        world.commands().queue(move |world: &mut World| {
            let (mut entities, mut commands) = world.entities_and_commands();
            let Ok(mut state_machine_ref) = entities.get_mut(state_machine_id) else {
                return;
            };
            let Some(mut state_machine) = state_machine_ref.get_mut::<HsmStateMachine>() else {
                return;
            };

            if let Some((curr_state, on_state)) = state_machine.pop_next_state().to() {
                let mut entity_commands = commands.entity(state_machine_id);
                entity_commands.queue(move |mut entity_mut: EntityWorldMut<'_>| {
                    let Some(mut state_machine) = entity_mut.get_mut::<HsmStateMachine>() else {
                        return;
                    };
                    state_machine.set_curr_state(curr_state);
                    entity_mut.insert(on_state);
                });
                world.flush();
            }
        });
    }
}
