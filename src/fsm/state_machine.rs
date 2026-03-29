use bevy::{
    ecs::{
        lifecycle::HookContext, relationship::Relationship, system::SystemParam,
        world::DeferredWorld,
    },
    platform::collections::HashMap,
    prelude::*,
};

use crate::{
    context::*,
    error::StateMachineError,
    fsm::{FsmState, event::FsmTrigger, graph::FsmGraph},
    guards::GuardCondition,
    markers::Paused,
    prelude::{ActionDispatch, FsmTriggerType, GetBufferId, GuardRegistry, StateActionBuffer},
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
    graph_id: Entity,
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
        curr_state: Entity,
        #[cfg(feature = "history")] history_size: usize,
    ) -> Self {
        Self {
            graph_id,
            init_state,
            curr_state,
            #[cfg(feature = "history")]
            history: FsmStateHistory::new(history_size),
        }
    }

    pub fn with(
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

    pub const fn graph_id(&self) -> Entity {
        self.graph_id
    }

    pub const fn curr_state_id(&self) -> Entity {
        self.curr_state
    }

    pub const fn init_state_id(&self) -> Entity {
        self.init_state
    }

    /// 设置当前状态, 并记录历史
    ///
    /// Set current state and record history
    pub fn set_curr_state(&mut self, state: Entity) {
        #[cfg(feature = "history")]
        self.history.push(state);
        self.curr_state = state;
    }

    #[cfg(feature = "history")]
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    fn get_service_target(world: &DeferredWorld, entity: Entity) -> Entity {
        let entity_ref = world.entity(entity);

        #[cfg(feature = "hybrid")]
        if let Some(s) = entity_ref.get::<NestedFsm>() {
            return s.state_machine;
        }

        match entity_ref.get::<ServiceTarget>() {
            Some(service_target) => service_target.0,
            None => entity,
        }
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        #[cfg(feature = "history")]
        let Some(mut fsm_state_machine) = world.get_mut::<FsmStateMachine>(entity) else {
            error!("{}", StateMachineError::FsmStateMachineMissing(entity));
            return;
        };
        #[cfg(not(feature = "history"))]
        let Some(fsm_state_machine) = world.get::<FsmStateMachine>(entity) else {
            error!("{}", StateMachineError::FsmStateMachineMissing(entity));
            return;
        };
        let curr_state = fsm_state_machine.curr_state_id();
        #[cfg(feature = "history")]
        fsm_state_machine.history.push(curr_state);
        let service_target = Self::get_service_target(&world, entity);

        if let Some(id) =
            TransitionRegistry::get_transition_id::<BeforeEnterSystem>(&world, curr_state)
        {
            let context = TransitionContext::with_initial(service_target, entity, curr_state);
            let _ = context.run_system(&mut world, id);
        }

        #[cfg(feature = "state_data")]
        StateData::clone_components(&mut world, curr_state, service_target);

        let context = ActionContext::new(service_target, entity, curr_state);

        if let Some(id) = ActionRegistry::get_action_id::<AfterEnterSystem>(&world, curr_state) {
            let _ = context.run_system(&mut world, id);
        }

        info!("after enter");
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

        let curr_state = fsm_state_machine.curr_state_id();
        let service_target = Self::get_service_target(&world, entity);

        let context = ActionContext::new(service_target, entity, curr_state);

        if let Some(id) = ActionRegistry::get_action_id::<BeforeExitSystem>(&world, curr_state) {
            let _ = context.run_system(&mut world, id);
        }

        #[cfg(feature = "state_data")]
        StateData::remove_components(&mut world, curr_state, service_target);

        if let Some(id) =
            TransitionRegistry::get_transition_id::<AfterExitSystem>(&world, curr_state)
        {
            let context = TransitionContext::with_final(service_target, entity, curr_state);
            let _ = context.run_system(&mut world, id);
        }

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
        action_systems: ActionSystems,
        guard_registry: Res<GuardRegistry>,
        fsm_graph: Query<&FsmGraph>,
        mut query: Query<&mut FsmStateMachine, Without<Paused>>,
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
                StateMachineError::GraphMissing(state_machine.graph_id)
            );
            return;
        };
        let Some(state_transitions) = fsm_graph.get(state_machine.curr_state_id()) else {
            error!(
                "{}",
                StateMachineError::StateNotInGraph {
                    graph: state_machine.graph_id,
                    state: state_machine.curr_state_id()
                }
            );
            return;
        };

        match typed {
            FsmTriggerType::Guard(target) => {
                if let Some(guard) = state_transitions.get_by_guard(target) {
                    Self::handle_guard_transition(
                        &mut commands,
                        &action_systems,
                        &guard_registry,
                        &state_machine,
                        state_machine_id,
                        guard,
                        target,
                    );
                } else {
                    trace!(
                        "{}",
                        StateMachineError::InvalidTransitionTarget {
                            graph: state_machine.graph_id,
                            from_state: state_machine.curr_state_id(),
                            to_state: target
                        }
                    );
                }
            }
            FsmTriggerType::Next(target) => {
                if state_transitions.contains(target) {
                    state_machine.handle_direct_transition(
                        &mut commands,
                        &action_systems,
                        state_machine_id,
                        target,
                        #[cfg(feature = "state_data")]
                        &query_state_data,
                    );
                } else {
                    trace!(
                        "{}",
                        StateMachineError::InvalidTransitionTarget {
                            graph: state_machine.graph_id,
                            from_state: state_machine.curr_state_id(),
                            to_state: target
                        }
                    );
                }
            }
            FsmTriggerType::Event(mut fsm_on_event) => {
                if let Some(target) = fsm_on_event.get_target(state_transitions) {
                    state_machine.handle_direct_transition(
                        &mut commands,
                        &action_systems,
                        state_machine_id,
                        target,
                        #[cfg(feature = "state_data")]
                        &query_state_data,
                    );
                }
            }
        };
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_direct_transition(
        &mut self,
        commands: &mut Commands,
        action_systems: &ActionSystems,
        state_machine_id: Entity,
        to: Entity,
        #[cfg(feature = "state_data")] query_state_data: &Query<&StateData, With<FsmState>>,
    ) {
        let from = self.curr_state_id();
        let service_target = action_systems.service_target(state_machine_id);

        let context = ActionContext::new(service_target, state_machine_id, from);

        if let Some(get_buff_id) = action_systems.get_buffer_id(from) {
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

        action_systems.run_exit_action(from, context, commands);

        #[cfg(feature = "state_data")]
        if let Ok(state_data) = query_state_data.get(from).cloned() {
            commands.queue(state_data.remove_state_data_command(service_target));
        }

        let transition_context =
            TransitionContext::with_transition(service_target, state_machine_id, from, to);

        action_systems.run_after_exit(from, transition_context, commands);

        self.set_curr_state(to);

        action_systems.run_before_enter(to, transition_context, commands);

        #[cfg(feature = "state_data")]
        if let Ok(state_data) = query_state_data.get(to).cloned() {
            commands.queue(state_data.clone_state_data_command(to, service_target))
        }

        let context = ActionContext::new(service_target, state_machine_id, to);

        action_systems.run_enter_action(to, context, commands);

        if let Some(get_buff_id) = action_systems.get_buffer_id(to) {
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
    fn handle_guard_transition(
        commands: &mut Commands,
        action_systems: &ActionSystems,
        guard_registry: &GuardRegistry,
        state_machine: &FsmStateMachine,
        state_machine_id: Entity,
        guard: &GuardCondition,
        target: Entity,
    ) {
        let Some(id) = guard_registry.to_combinator_condition_id(guard) else {
            warn!(
                "[GuardRegistry] This guard<{:?}> does not exist for state {:?}",
                guard, target
            );
            return;
        };

        let service_target = action_systems.service_target(state_machine_id);
        let from = state_machine.curr_state_id();
        let context = GuardContext::new(service_target, state_machine_id, from, target);

        let remove_buffer_id = action_systems.get_buffer_id(from).map(|id| {
            (
                id,
                ActionContext::new(service_target, state_machine_id, from),
            )
        });

        let on_exit_system_id = action_systems.get_exit_action_id(from).map(|id| {
            (
                id,
                ActionContext::new(service_target, state_machine_id, from),
            )
        });

        let after_exit_system_id = action_systems.get_after_exit_transition_id(from);

        let before_enter_system_id = action_systems.get_before_enter_transition_id(target);

        let on_enter_system_id = action_systems.get_enter_action_id(target).map(|id| {
            (
                id,
                ActionContext::new(service_target, state_machine_id, target),
            )
        });

        let add_buffer_id = action_systems.get_buffer_id(target).map(|id| {
            (
                id,
                ActionContext::new(service_target, state_machine_id, target),
            )
        });

        commands.queue(move |world: &mut World| {
            match id.run(world, context) {
                Ok(true) => {
                    if let Some((system, action_context)) = remove_buffer_id {
                        (system)(
                            world,
                            Box::new(move |_world, buffer| {
                                buffer.remove_interceptor(action_context);
                                buffer.add_filter(action_context);
                            }),
                        );
                    }
                    if let Some((id, context)) = on_exit_system_id {
                        world.flush();
                        let _ = world.run_system_with(id, context);

                        #[cfg(feature = "state_data")]
                        if let Some(state_data) = world.get::<StateData>(context.state()).cloned() {
                            state_data
                                .remove_state_data_command(context.service_target)
                                .apply(world);
                        }
                    }

                    if let Some(id) = after_exit_system_id {
                        let context = TransitionContext::with_transition(
                            service_target,
                            state_machine_id,
                            from,
                            target,
                        );
                        world.flush();
                        let _ = world.run_system_with(id, context);
                    }

                    if let Some(mut state_machine) =
                        world.get_mut::<FsmStateMachine>(state_machine_id)
                    {
                        state_machine.set_curr_state(target);
                    }

                    if let Some(id) = before_enter_system_id {
                        let context = TransitionContext::with_transition(
                            service_target,
                            state_machine_id,
                            from,
                            target,
                        );
                        world.flush();
                        let _ = world.run_system_with(id, context);
                    }

                    if let Some((id, context)) = on_enter_system_id {
                        #[cfg(feature = "state_data")]
                        if let Some(state_data) = world.get::<StateData>(context.state()).cloned() {
                            state_data
                                .clone_state_data_command(context.state(), context.service_target)
                                .apply(world);
                        }
                        world.flush();
                        let _ = world.run_system_with(id, context);
                    }

                    if let Some((system, action_context)) = add_buffer_id {
                        (system)(
                            world,
                            Box::new(move |_world, buffer| {
                                buffer.add(action_context);
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

#[derive(SystemParam)]
pub(crate) struct ActionSystems<'w, 's> {
    action_dispatch: Res<'w, ActionDispatch>,
    action_registry: Res<'w, ActionRegistry>,
    transition_registry: Res<'w, TransitionRegistry>,
    query_on_exit_system: Query<'w, 's, &'static BeforeExitSystem, With<FsmState>>,
    query_on_enter_system: Query<'w, 's, &'static AfterEnterSystem, With<FsmState>>,
    query_on_update_system: Query<'w, 's, &'static OnUpdateSystem, With<FsmState>>,
    query_after_exit_system: Query<'w, 's, &'static AfterExitSystem, With<FsmState>>,
    query_before_enter_system: Query<'w, 's, &'static BeforeEnterSystem, With<FsmState>>,
    query_service_target: Query<'w, 's, &'static ServiceTarget, With<FsmStateMachine>>,
    #[cfg(feature = "hybrid")]
    query_hsm_child_of: Query<'w, 's, &'static NestedFsm, With<FsmStateMachine>>,
}

impl<'w, 's> ActionSystems<'w, 's> {
    #[inline]
    pub fn service_target(&self, state_machine: Entity) -> Entity {
        #[cfg(feature = "hybrid")]
        if let Ok(child_of) = self.query_hsm_child_of.get(state_machine) {
            return child_of.state_machine;
        }

        self.query_service_target
            .get(state_machine)
            .map_or(state_machine, ServiceTarget::get)
    }

    pub fn get_buffer_id(&self, state: Entity) -> Option<GetBufferId> {
        self.query_on_update_system
            .get(state)
            .ok()
            .and_then(|update| self.action_dispatch.get(update))
    }

    pub fn get_enter_action_id(&self, state: Entity) -> Option<ActionId> {
        self.query_on_enter_system
            .get(state)
            .ok()
            .and_then(|enter| self.action_registry.get(enter))
    }

    pub fn get_exit_action_id(&self, state: Entity) -> Option<ActionId> {
        self.query_on_exit_system
            .get(state)
            .ok()
            .and_then(|exit| self.action_registry.get(exit))
    }

    pub fn get_before_enter_transition_id(&self, state: Entity) -> Option<TransitionId> {
        self.query_before_enter_system
            .get(state)
            .ok()
            .and_then(|enter| self.transition_registry.get(enter))
    }

    pub fn get_after_exit_transition_id(&self, state: Entity) -> Option<TransitionId> {
        self.query_after_exit_system
            .get(state)
            .ok()
            .and_then(|exit| self.transition_registry.get(exit))
    }

    pub fn run_before_enter(
        &self,
        state: Entity,
        context: TransitionContext,
        commands: &mut Commands,
    ) {
        let Ok(enter) = self.query_before_enter_system.get(state) else {
            return;
        };
        if let Some(id) = self.transition_registry.get(enter) {
            commands.run_system_with(id, context);
            return;
        }
        warn!("{}", enter.not_found_error(state))
    }

    #[inline]
    pub fn run_enter_action(&self, state: Entity, context: ActionContext, commands: &mut Commands) {
        let Ok(enter) = self.query_on_enter_system.get(state) else {
            return;
        };
        if let Some(id) = self.action_registry.get(enter) {
            commands.run_system_with(id, context);
            return;
        };
        warn!("{}", enter.not_found_error(state))
    }

    #[inline]
    pub fn run_exit_action(&self, state: Entity, context: ActionContext, commands: &mut Commands) {
        let Ok(exit) = self.query_on_exit_system.get(state) else {
            return;
        };

        if let Some(id) = self.action_registry.get(exit) {
            commands.run_system_with(id, context);
            return;
        }
        warn!("{}", exit.not_found_error(state))
    }

    pub fn run_after_exit(
        &self,
        state: Entity,
        context: TransitionContext,
        commands: &mut Commands,
    ) {
        let Ok(exit) = self.query_after_exit_system.get(state) else {
            return;
        };
        if let Some(id) = self.transition_registry.get(exit) {
            commands.run_system_with(id, context);
            return;
        }
        warn!("{}", exit.not_found_error(state))
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

#[cfg(feature = "hybrid")]
#[derive(Component, PartialEq, Eq, Clone, Debug, Default, Deref)]
#[component(on_remove=Self::on_remove)]
pub struct HsmOwnedFsms(pub(crate) HashMap<Entity, Entity>);

#[cfg(feature = "hybrid")]
impl HsmOwnedFsms {
    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let (entitys, mut commands) = world.entities_and_commands();
        let Ok(state_machine) = entitys.get(entity) else {
            return;
        };
        let Some(mapping) = state_machine.get::<Self>() else {
            return;
        };

        mapping.values().copied().for_each(|fsm_id| {
            commands.entity(fsm_id).despawn();
        });
    }
}

#[cfg(feature = "hybrid")]
impl From<(Entity, Entity)> for HsmOwnedFsms {
    fn from(value: (Entity, Entity)) -> Self {
        Self(HashMap::from([value]))
    }
}

#[cfg(feature = "hybrid")]
#[derive(Component, PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[component(on_insert=Self::on_insert)]
pub struct NestedFsm {
    state_machine: Entity,
    state: Entity,
}

#[cfg(feature = "hybrid")]
impl NestedFsm {
    pub(crate) const fn new(state_machine: Entity, state: Entity) -> Self {
        Self {
            state,
            state_machine,
        }
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(child_of) = world.get::<NestedFsm>(entity).copied() else {
            return;
        };

        match world.get_mut::<HsmOwnedFsms>(child_of.state_machine) {
            Some(mut mapping) => {
                mapping.0.insert(child_of.state, entity);
            }
            None => {
                world
                    .commands()
                    .entity(child_of.state_machine)
                    .insert(HsmOwnedFsms::from((child_of.state, entity)));
            }
        }
    }
}
