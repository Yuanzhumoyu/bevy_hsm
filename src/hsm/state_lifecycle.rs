use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

#[cfg(feature = "history")]
use crate::hsm::history::HistoricalNode;
#[cfg(feature = "state_data")]
use crate::prelude::StateScenePatch;
use crate::{
    context::{ActionContext, TransitionContext},
    error::{StateMachineError, StateMachineErrorEvent, warn_event_world},
    hsm::{state_machine::*, state_tree::StateTree, transition::Transition},
    labels::SystemLabel,
    markers::Terminated,
    prelude::{
        ActionRegistry, AfterEnterSystem, AfterExitSystem, BeforeEnterSystem, BeforeExitSystem,
        CheckOnTransitionStates, GuardEnter, GuardExit, OnUpdateSystem, ServiceTarget,
        StateActionBuffer, TransitionRegistry,
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
    /// 运行与特定状态关联的动作系统
    ///
    /// Runs the action system associated with a specific state
    fn run_state_action_system<T: Component + std::ops::Deref<Target = SystemLabel>>(
        world: &mut DeferredWorld,
        state_id: Entity,
        state_context: ActionContext,
    ) {
        let Some(action_system_id) = ActionRegistry::get_action_id::<T>(world, state_id) else {
            return;
        };

        state_context.run_system(world, action_system_id);
    }

    /// 运行与状态转换关联的转换系统
    ///
    /// Runs the transition system associated with a state transition
    fn run_transition_action_system<T: Component + std::ops::Deref<Target = SystemLabel>>(
        world: &mut DeferredWorld,
        state_id: Entity,
        state_context: TransitionContext,
    ) {
        let Some(action_system_id) = TransitionRegistry::get_transition_id::<T>(world, state_id)
        else {
            return;
        };
        state_context.run_system(world, action_system_id);
    }

    #[cfg(feature = "hybrid")]
    fn handle_hybrid_entry(
        world: &mut DeferredWorld,
        state_machine_id: Entity,
        state_id: Entity,
    ) -> Result<(), StateMachineError> {
        use crate::{fsm::hybrid::NestedFsm, hsm::HsmState, prelude::FsmGraph};
        let Some(state) = world.get::<HsmState>(state_id) else {
            return Err(StateMachineError::HsmStateMissing(state_id));
        };

        let Some(fsm_config) = state.fsm_config else {
            return Ok(());
        };

        let Some(init_state) = world
            .get::<FsmGraph>(fsm_config.graph_id)
            .map(|graph| graph.init_state())
        else {
            return Err(StateMachineError::GraphMissing(fsm_config.graph_id));
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

        Ok(())
    }

    #[cfg(feature = "hybrid")]
    fn handle_hybrid_exit(world: &mut DeferredWorld, state_machine_id: Entity, state_id: Entity) {
        use crate::{fsm::hybrid::HsmOwnedFsms, prelude::FsmStateMachine};

        // 当没有[`HsmOwnedFsms`]组件时直接退出, 直接退出
        let Some(mut mapping) = world.get_mut::<HsmOwnedFsms>(state_machine_id) else {
            return;
        };

        // 当[`HsmOwnedFsms`]组件存储[`FsmStateMachine`]组件时, 才会执行退出操作
        // 当没有[`FsmStateMachine`]组件时, 直接退出
        let Some(fsm_state_machine) = mapping.0.remove(&state_id) else {
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
            world.get_entity_mut([state_machine_id, fsm_state_machine])
            && let Some(mut hsm) = state_machine_mut.get_mut::<HsmStateMachine>()
            && let Some(mut fsm) = fsm_state_machine_mut.get_mut::<FsmStateMachine>()
        {
            hsm.history
                .set_last_state_fsm_history(state_id, fsm.history.take());
        }

        world.commands().entity(fsm_state_machine).despawn();
    }

    fn prepare_transition(
        world: &mut DeferredWorld,
        hook_context: HookContext,
    ) -> Result<TransitionInfo, StateMachineError> {
        let state_machine_id = hook_context.entity;

        let Ok(mut entity_mut) = world.get_entity_mut(state_machine_id) else {
            return Err(StateMachineError::HsmStateMachineMissing(state_machine_id));
        };

        let Some(lifecycle) = entity_mut.get::<StateLifecycle>().copied() else {
            return Err(StateMachineError::StateLifecycleMissing(state_machine_id));
        };

        let service_target = match entity_mut.get::<ServiceTarget>() {
            Some(service_target) => service_target.0,
            None => state_machine_id,
        };

        let Some(mut state_machine) = entity_mut.get_mut::<HsmStateMachine>() else {
            return Err(StateMachineError::HsmStateMachineMissing(state_machine_id));
        };

        let curr_state_id = state_machine.curr_state_id();
        let curr = Transition::with_lifecycle(curr_state_id, lifecycle);
        let prev = state_machine.replace_prev_state(curr);
        #[cfg(feature = "history")]
        {
            let depth = state_machine.interrupt_depth();
            let tree_id = state_machine.state_tree();
            state_machine.push_history(HistoricalNode::new(
                curr_state_id,
                lifecycle.into(),
                tree_id,
                depth,
            ));
        }

        let state_context = ActionContext::new(service_target, state_machine_id, curr_state_id);

        Ok(TransitionInfo {
            state_machine_id,
            prev_transition: prev,
            curr_transition: curr,
            curr_state_id,
            state_context,
            hsm_state: lifecycle,
        })
    }

    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let transition_info = match Self::prepare_transition(&mut world, hook_context) {
            Ok(info) => info,
            Err(e) => {
                warn!("{}", e);
                return;
            }
        };

        let TransitionInfo {
            state_machine_id,
            prev_transition,
            curr_transition,
            curr_state_id,
            state_context,
            hsm_state,
        } = transition_info;

        match hsm_state {
            StateLifecycle::Enter => {
                Self::handle_enter(
                    &mut world,
                    state_machine_id,
                    curr_state_id,
                    prev_transition,
                    curr_transition,
                    state_context,
                );
            }
            StateLifecycle::Update => {
                Self::handle_update(&mut world, state_machine_id, curr_state_id, state_context);
            }
            StateLifecycle::Exit => {
                Self::handle_exit(
                    &mut world,
                    state_machine_id,
                    curr_state_id,
                    prev_transition,
                    curr_transition,
                    state_context,
                );
            }
        };

        Self::process_transition_queue(&mut world, state_machine_id);
    }

    fn handle_enter(
        world: &mut DeferredWorld,
        state_machine_id: Entity,
        curr_state_id: Entity,
        prev_transition: Transition,
        curr_transition: Transition,
        state_context: ActionContext,
    ) {
        // When transitioning Update(parent) → Enter(child) via Nested
        // strategy, the parent's OnUpdateSystem must be filtered so only
        // the leaf state receives Update events. Remove from curr/next
        // immediately (the filter alone only applies at the next swap).
        if let (Transition::Update(prev_state_id), Transition::Enter(_)) =
            (prev_transition, curr_transition)
        {
            StateActionBuffer::buffer_scope(
                world.as_unsafe_world_cell(),
                prev_state_id,
                move |buff| {
                    let parent_ctx = ActionContext::new(
                        state_context.service_target,
                        state_machine_id,
                        prev_state_id,
                    );
                    buff.curr.remove(&parent_ctx);
                    buff.next.remove(&parent_ctx);
                    buff.add_filter(parent_ctx);
                },
            );
        }

        let Some(relationship) = prev_transition.to_transition(curr_transition) else {
            return;
        };

        Self::run_transition_action_system::<BeforeEnterSystem>(
            world,
            curr_state_id,
            TransitionContext::with(state_context.service_target, state_machine_id, relationship),
        );

        #[cfg(feature = "hybrid")]
        if let Err(e) = Self::handle_hybrid_entry(world, state_machine_id, curr_state_id) {
            error!("{}", e);
            world
                .commands()
                .trigger(StateMachineErrorEvent::new(state_machine_id, e));
        }

        #[cfg(feature = "state_data")]
        StateScenePatch::spawn_state_scene(
            world,
            curr_state_id,
            state_machine_id,
            state_context.service_target,
        );

        Self::run_state_action_system::<AfterEnterSystem>(world, curr_state_id, state_context);

        world
            .commands()
            .entity(state_machine_id)
            .insert(StateLifecycle::Update);
    }

    fn handle_update(
        world: &mut DeferredWorld,
        state_machine_id: Entity,
        curr_state_id: Entity,
        state_context: ActionContext,
    ) {
        // Only track for guard checking when guards actually exist
        let has_guards = world
            .get::<HsmStateMachine>(state_machine_id)
            .is_some_and(|sm| {
                let tree_id = sm.state_tree();
                world.get::<StateTree>(tree_id).is_some_and(|tree| {
                    world.entity(curr_state_id).contains::<GuardExit>()
                        || tree
                            .get_sub_states(curr_state_id)
                            .map(|subs| {
                                subs.iter()
                                    .any(|&sub| world.entity(sub).contains::<GuardEnter>())
                            })
                            .unwrap_or(false)
                })
            });

        if has_guards {
            let mut check_on_transition_states = world.resource_mut::<CheckOnTransitionStates>();
            check_on_transition_states.insert(state_machine_id);
        }

        if world.entity(curr_state_id).contains::<OnUpdateSystem>() {
            StateActionBuffer::buffer_scope(
                world.as_unsafe_world_cell(),
                curr_state_id,
                move |buff| {
                    buff.remove_filter(state_context);
                    buff.add(state_context);
                },
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_exit(
        world: &mut DeferredWorld,
        state_machine_id: Entity,
        curr_state_id: Entity,
        _prev: Transition,
        curr_transition: Transition,
        state_context: ActionContext,
    ) {
        StateActionBuffer::buffer_scope(world.as_unsafe_world_cell(), curr_state_id, move |buff| {
            buff.remove_interceptor(state_context);
            buff.add_filter(state_context);
        });

        Self::run_state_action_system::<BeforeExitSystem>(world, curr_state_id, state_context);

        #[cfg(feature = "hybrid")]
        Self::handle_hybrid_exit(world, state_machine_id, curr_state_id);

        #[cfg(feature = "state_data")]
        StateScenePatch::reclaim_state_scene(
            world,
            curr_state_id,
            state_machine_id,
            state_context.service_target,
        );

        world.commands().queue(move |world: &mut World| {
            let Some(mut state_machine) = world.get_mut::<HsmStateMachine>(state_machine_id) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::HsmStateMachineMissing(state_machine_id),
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
                    Self::run_transition_action_system::<AfterExitSystem>(
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
                    Self::run_transition_action_system::<AfterExitSystem>(
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

    fn process_transition_queue(world: &mut DeferredWorld, state_machine_id: Entity) {
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
