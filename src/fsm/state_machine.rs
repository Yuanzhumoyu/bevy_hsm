use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{
    context::*,
    error::StateMachineError,
    fsm::{FsmState, event::FsmTrigger, graph::FsmGraph},
    markers::Paused,
    prelude::{
        ActionDispatch, FsmTriggerType, GetBufferId, GuardRegistry, OutgoingTransitions,
        StateActionBuffer,
    },
    state_actions::*,
};

#[cfg(feature = "state_data")]
use crate::state_data::StateData;

#[cfg(feature = "history")]
use crate::fsm::history::*;

/// # FSM 状态机
/// * 一个有限状态机（FSM）的运行时实例。
///
/// 该组件负责跟踪一个具体状态机的当前状态 (`curr_state`)。每个 `FsmStateMachine` 都必须关联到一个
/// 定义了其拓扑结构的 `FsmGraph`。
///
/// 多个 `FsmStateMachine` 实例可以共享同一个 `FsmGraph`，从而允许创建多个行为相同但状态独立的“智能体”。
///
/// 它的 `on_insert` 和 `on_remove` 钩子负责处理进入初始状态和在状态机被销毁时进行清理的逻辑。
///
/// # FSM State Machine
/// * A runtime instance of a Finite State Machine (FSM).
///
/// This component is responsible for tracking the current state (`curr_state`) of a specific state machine.
/// Each `FsmStateMachine` must be associated with an `FsmGraph` that defines its topology.
///
/// Multiple `FsmStateMachine` instances can share the same `FsmGraph`, allowing for the creation of
/// multiple "agents" that have the same behavior but independent states.
///
/// Its `on_insert` and `on_remove` hooks handle the logic for entering the initial state and
/// cleaning up when the state machine is destroyed.
#[derive(Component)]
#[component(on_insert = Self::on_insert,on_remove = Self::on_remove)]
pub struct FsmStateMachine {
    /// 包含状态机拓扑 (`FsmGraph`) 的实体。
    /// The entity that holds the state machine's topology (`FsmGraph`).
    pub graph_id: Entity,
    /// 状态机的初始状态，在创建时从图中复制。
    /// The initial state of the state machine, copied from the graph upon creation.
    pub(super) init_state: Entity,
    /// 此状态机实例当前所处的活动状态。
    /// The currently active state for this state machine instance.
    pub(super) curr_state: Entity,
    /// (当 `history` 特性启用时) 跟踪此状态机访问过的状态历史。
    /// (When the `history` feature is enabled) Tracks the history of visited states for this state machine.
    #[cfg(feature = "history")]
    pub history: FsmStateHistory,
}

impl FsmStateMachine {
    pub fn new(
        graph_id: Entity,
        init_state: Entity,
        #[cfg(feature = "history")] history_size: usize,
    ) -> Self {
        Self {
            graph_id,
            init_state,
            curr_state: init_state,
            #[cfg(feature = "history")]
            history: FsmStateHistory::new(history_size),
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
            let Some(id) = StateActionRegistry::get_system_id::<OnEnterSystem>(&world, curr_state)
            else {
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

        let context = StateActionContext::new(service_target, entity, curr_state);

        'on_exit: {
            let Some(id) = StateActionRegistry::get_system_id::<OnExitSystem>(&world, curr_state)
            else {
                break 'on_exit;
            };

            unsafe {
                let _ = world
                    .as_unsafe_world_cell()
                    .world_mut()
                    .run_system_with(id, context);
            }
        };

        #[cfg(feature = "state_data")]
        StateData::remove_components(&mut world, entity, service_target);

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
        fsm_graph: Query<&FsmGraph>,
        mut query: Query<&mut FsmStateMachine, Without<Paused>>,
        query_on_exit_system: Query<&OnExitSystem, With<FsmState>>,
        query_on_enter_system: Query<&OnEnterSystem, With<FsmState>>,
        query_on_update_system: Query<&OnUpdateSystem, With<FsmState>>,
        query_service_target: Query<&ServiceTarget, With<FsmStateMachine>>,
        #[cfg(feature = "state_data")] query_state_data: Query<&StateData, With<FsmState>>,
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

        match typed {
            FsmTriggerType::Transition(target) => {
                Self::handle_guarded_transition(
                    &mut commands,
                    &guard_registry,
                    &action_dispatch,
                    &action_registry,
                    &state_machine,
                    state_machine_id,
                    target,
                    state_transitions,
                    &query_service_target,
                    &query_on_update_system,
                    &query_on_enter_system,
                    &query_on_exit_system,
                );
            }
            FsmTriggerType::Next(target) => {
                if state_transitions.contains(target) {
                    state_machine.execute_state_transition(
                        &mut commands,
                        state_machine_id,
                        target,
                        &query_service_target,
                        &action_dispatch,
                        &action_registry,
                        &query_on_update_system,
                        &query_on_enter_system,
                        &query_on_exit_system,
                        #[cfg(feature = "state_data")]
                        &query_state_data,
                    );
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
                    state_machine.execute_state_transition(
                        &mut commands,
                        state_machine_id,
                        target,
                        &query_service_target,
                        &action_dispatch,
                        &action_registry,
                        &query_on_update_system,
                        &query_on_enter_system,
                        &query_on_exit_system,
                        #[cfg(feature = "state_data")]
                        &query_state_data,
                    );
                }
            }
        };
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_state_transition(
        &mut self,
        commands: &mut Commands,
        state_machine_id: Entity,
        to: Entity,
        query_service_target: &Query<&ServiceTarget, With<FsmStateMachine>>,
        action_dispatch: &ActionDispatch,
        action_registry: &StateActionRegistry,
        query_on_update_system: &Query<&OnUpdateSystem, With<FsmState>>,
        query_on_enter_system: &Query<&OnEnterSystem, With<FsmState>>,
        query_on_exit_system: &Query<&OnExitSystem, With<FsmState>>,
        #[cfg(feature = "state_data")] query_state_data: &Query<&StateData, With<FsmState>>,
    ) {
        let from = self.curr_state;
        let service_target = query_service_target
            .get(state_machine_id)
            .map_or(state_machine_id, |st| st.0);

        let on_update_system = |state: Entity| -> Option<GetBufferId> {
            let on_update = query_on_update_system.get(state).ok()?;
            let get_buffer_id = action_dispatch.get(on_update.as_str())?;
            Some(get_buffer_id)
        };

        let context = StateActionContext::new(service_target, state_machine_id, from);

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

        if let Ok(on_exit) = query_on_exit_system.get(from) {
            match action_registry.get(on_exit.as_str()).cloned() {
                Some(id) => {
                    commands.run_system_with(id, context);
                }
                None => {
                    warn!(
                        "{}",
                        StateMachineError::SystemNotFound {
                            system_name: on_exit.to_string(),
                            state: from
                        }
                    )
                }
            }
        }

        #[cfg(feature = "state_data")]
        if let Ok(state_data) = query_state_data.get(from).cloned() {
            commands.queue(state_data.remove_state_data_command(service_target));
        }

        self.set_curr_state(to);

        #[cfg(feature = "state_data")]
        if let Ok(state_data) = query_state_data.get(to).cloned() {
            commands.queue(state_data.clone_state_data_command(to, service_target))
        }

        let context = StateActionContext::new(service_target, state_machine_id, to);

        if let Ok(on_enter) = query_on_enter_system.get(to) {
            match action_registry.get(on_enter.as_str()).cloned() {
                Some(id) => {
                    commands.run_system_with(id, context);
                }
                None => {
                    warn!(
                        "{}",
                        StateMachineError::SystemNotFound {
                            system_name: on_enter.to_string(),
                            state: to
                        }
                    )
                }
            }
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
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_guarded_transition(
        commands: &mut Commands,
        guard_registry: &GuardRegistry,
        action_dispatch: &ActionDispatch,
        action_registry: &StateActionRegistry,
        state_machine: &FsmStateMachine,
        state_machine_id: Entity,
        target: Entity,
        state_transitions: &OutgoingTransitions,
        query_service_target: &Query<&ServiceTarget, With<FsmStateMachine>>,
        query_on_update_system: &Query<&OnUpdateSystem, With<FsmState>>,
        query_on_enter_system: &Query<&OnEnterSystem, With<FsmState>>,
        query_on_exit_system: &Query<&OnExitSystem, With<FsmState>>,
    ) {
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

        let service_target = query_service_target
            .get(state_machine_id)
            .map_or(state_machine_id, |st| st.0);
        let from = state_machine.curr_state;
        let context = GuardContext::new(service_target, state_machine_id, from, target);
        let on_update_system = |state: Entity| -> Option<GetBufferId> {
            let on_update = query_on_update_system.get(state).ok()?;
            let get_buffer_id = action_dispatch.get(on_update.as_str())?;
            Some(get_buffer_id)
        };

        let remove_buffer_id = on_update_system(from).map(|id| {
            (
                id,
                StateActionContext::new(service_target, state_machine_id, from),
            )
        });

        let on_exit_system_id = query_on_exit_system
            .get(from)
            .ok()
            .and_then(|on_exit| action_registry.get(on_exit.as_str()))
            .map(|id| {
                (
                    *id,
                    StateActionContext::new(service_target, state_machine_id, from),
                )
            });

        let on_enter_system_id = query_on_enter_system
            .get(target)
            .ok()
            .and_then(|on_enter| action_registry.get(on_enter.as_str()))
            .map(|id| {
                (
                    *id,
                    StateActionContext::new(service_target, state_machine_id, target),
                )
            });

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
                        if let Some(state_data) = world.get::<StateData>(context.state()).cloned() {
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
                        if let Some(state_date) = world.get::<StateData>(context.state()).cloned() {
                            state_date
                                .clone_state_data_command(context.state(), context.service_target)
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
                Ok(false) => {} // Guard failed, do nothing
                Err(e) => error!("{}", e),
            };
        });
    }
}

/// # FSM 蓝图
/// * 一个用于配置和创建 `FsmStateMachine` 实例的数据结构。
///
/// 这不是一个组件，而是一个普通的结构体，用作数据传输对象（DTO）。
/// 它的主要用途是在更复杂的结构中（例如 `HsmState`）定义一个嵌套的 FSM，
/// 允许在创建时精确控制 FSM 的初始状态和配置。
///
/// # FSM Blueprint
/// * A data structure for configuring and creating an `FsmStateMachine` instance.
///
/// This is not a component but a plain struct that acts as a Data Transfer Object (DTO).
/// Its primary use is to define a nested FSM within more complex structures (e.g., an `HsmState`),
/// allowing for precise control over the FSM's initial state and configuration upon creation.
#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsmBlueprint {
    /// FSM 将要使用的图实体。
    /// The graph entity that the FSM will use.
    pub graph_id: Entity,
    /// 可选的当前状态。如果设置了此值，状态机将从这个状态开始，而不是 `init_state`。
    /// Optional current state. If this is set, the state machine will start in this state instead of `init_state`.
    pub curr_state: Option<Entity>,
    #[cfg(feature = "history")]
    /// 状态历史记录大小（当 `history` 特性启用时）。
    /// The size of the state history (when the `history` feature is enabled).
    pub history_size: usize,
}

impl FsmBlueprint {
    pub fn new(graph_id: Entity, #[cfg(feature = "history")] history_size: usize) -> Self {
        Self {
            graph_id,
            curr_state: None,
            #[cfg(feature = "history")]
            history_size,
        }
    }

    pub fn with_curr_state(mut self, curr_state: Entity) -> Self {
        self.curr_state = Some(curr_state);
        self
    }
}
