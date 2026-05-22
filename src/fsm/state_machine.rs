#[cfg(feature = "hybrid")]
use bevy::platform::collections::HashMap;
use bevy::{
    ecs::{
        lifecycle::HookContext, relationship::Relationship, system::SystemParam,
        world::DeferredWorld,
    },
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
use crate::state_data::StateScenePatch;

use crate::fsm::history::FsmStateHistory;

/// Holds pre-resolved system IDs and context for executing a state transition.
struct ResolvedTransition {
    remove_buffer: Option<(GetBufferId, ActionContext)>,
    exit_action: Option<(ActionId, ActionContext)>,
    after_exit: Option<(TransitionId, TransitionContext)>,
    before_enter: Option<(TransitionId, TransitionContext)>,
    enter_action: Option<(ActionId, ActionContext)>,
    add_buffer: Option<(GetBufferId, ActionContext)>,
    to: Entity,
    state_machine_id: Entity,
    service_target: Entity,
}

/// Executes all transition lifecycle steps on a [`World`].
///
/// The steps are:
/// 1. Remove from update buffer (from_state)
/// 2. Run exit action (from_state)
/// 3. Reclaim state scene (from_state, if `state_data` feature)
/// 4. Run after_exit transition (from_state)
/// 5. Set current state to `to`
/// 6. Run before_enter transition (to_state)
/// 7. Apply state scene (to_state, if `state_data` feature)
/// 8. Run enter action (to_state)
/// 9. Add to update buffer (to_state)
fn execute_transition_steps(
    world: &mut World,
    resolved: ResolvedTransition,
) -> bevy::prelude::Result<()> {
    let ResolvedTransition {
        remove_buffer,
        exit_action,
        after_exit,
        before_enter,
        enter_action,
        add_buffer,
        to,
        state_machine_id,
        service_target,
    } = resolved;
    if let Some((get_buff_id, ctx)) = remove_buffer {
        (get_buff_id)(
            world,
            Box::new({
                move |buffer: &mut StateActionBuffer| {
                    buffer.remove_interceptor(ctx);
                    buffer.add_filter(ctx);
                }
            }),
        );
    }

    if let Some((id, ref ctx)) = exit_action {
        ctx.queue_system_command(id).apply(world)?;

        #[cfg(feature = "state_data")]
        StateScenePatch::reclaim_state_scene_command(ctx.state(), state_machine_id, service_target)
            .apply(world);
    }

    if let Some((id, ctx)) = after_exit {
        ctx.queue_system_command(id).apply(world)?;
    }

    if let Some(mut sm) = world.get_mut::<FsmStateMachine>(state_machine_id) {
        sm.set_curr_state(to);
    }

    if let Some((id, ctx)) = before_enter {
        ctx.queue_system_command(id).apply(world)?;
    }

    if let Some((id, ref ctx)) = enter_action {
        #[cfg(feature = "state_data")]
        if let Some(patch) = world.get::<StateScenePatch>(ctx.state()).cloned() {
            patch
                .apply_state_scene_command(ctx.state(), state_machine_id, service_target)
                .apply(world);
        }
        ctx.queue_system_command(id).apply(world)?;
    }

    if let Some((get_buff_id, ctx)) = add_buffer {
        (get_buff_id)(
            world,
            Box::new({
                move |buffer: &mut StateActionBuffer| {
                    buffer.add(ctx);
                }
            }),
        );
    }

    Ok(())
}

/// # FSM 状态机
/// * 一个有限状态机（FSM）的运行时实例。
///
/// 该组件负责跟踪一个具体状态机的当前状态 (`curr_state`)。每个 [`FsmStateMachine`] 都必须关联到一个
/// 定义了其拓扑结构的 [`FsmGraph`]。
///
/// 多个 [`FsmStateMachine`] 实例可以共享同一个 [`FsmGraph`]，从而允许创建多个行为相同但状态独立的"智能体"。
///
/// 它的 `on_insert` 和 `on_remove` 钩子负责处理进入初始状态和在状态机被销毁时进行清理的逻辑。
///
/// # FSM State Machine
/// * A runtime instance of a Finite State Machine (FSM).
///
/// This component is responsible for tracking the current state (`curr_state`) of a specific state machine.
/// Each [`FsmStateMachine`] must be associated with a [`FsmGraph`] that defines its topology.
///
/// Multiple [`FsmStateMachine`] instances can share the same [`FsmGraph`], allowing for the creation of
/// multiple "agents" that have the same behavior but independent states.
///
/// Its `on_insert` and `on_remove` hooks handle the logic for entering the initial state and
/// cleaning up when the state machine is destroyed.
#[derive(Component)]
#[component(on_insert = Self::on_insert,on_remove = Self::on_remove)]
pub struct FsmStateMachine {
    /// 包含状态机拓扑 ([`FsmGraph`]) 的实体。
    /// The entity that holds the state machine's topology ([`FsmGraph`]).
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
        let service_target = get_service_target(&world, entity);

        if let Some(id) =
            TransitionRegistry::get_transition_id::<BeforeEnterSystem>(&world, curr_state)
        {
            let context = TransitionContext::with_initial(service_target, entity, curr_state);
            context.run_system(&mut world, id);
        }

        #[cfg(feature = "state_data")]
        StateScenePatch::spawn_state_scene(&mut world, curr_state, entity, service_target);

        let context = ActionContext::new(service_target, entity, curr_state);

        if let Some(id) = ActionRegistry::get_action_id::<AfterEnterSystem>(&world, curr_state) {
            context.run_system(&mut world, id);
        }

        StateActionBuffer::buffer_scope(
            world.as_unsafe_world_cell(),
            curr_state,
            move |buffer: &mut StateActionBuffer| {
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
        let service_target = get_service_target(&world, entity);

        let context = ActionContext::new(service_target, entity, curr_state);

        if let Some(id) = ActionRegistry::get_action_id::<BeforeExitSystem>(&world, curr_state) {
            context.run_system(&mut world, id);
        }

        #[cfg(feature = "state_data")]
        StateScenePatch::reclaim_state_scene(&mut world, curr_state, entity, service_target);

        if let Some(id) =
            TransitionRegistry::get_transition_id::<AfterExitSystem>(&world, curr_state)
        {
            let context = TransitionContext::with_final(service_target, entity, curr_state);
            context.run_system(&mut world, id);
        }

        StateActionBuffer::buffer_scope(
            world.as_unsafe_world_cell(),
            curr_state,
            move |buffer: &mut StateActionBuffer| {
                buffer.remove_interceptor(context);
                buffer.add_filter(context);
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_fsm_trigger(
        mut on: On<FsmTrigger>,
        mut commands: Commands,
        action_systems: ActionSystems,
        guard_registry: Res<GuardRegistry>,
        fsm_graph: Query<&FsmGraph>,
        query: Query<&FsmStateMachine, Without<Paused>>,
    ) {
        let FsmTrigger {
            state_machine,
            typed,
        } = on.event_mut();
        let state_machine_id = *state_machine;

        let Ok(state_machine) = query.get(state_machine_id) else {
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
                if let Some(guard) = state_transitions.get_by_guard(*target) {
                    Self::handle_guard_transition(
                        &mut commands,
                        &action_systems,
                        &guard_registry,
                        state_machine,
                        state_machine_id,
                        guard,
                        *target,
                    );
                } else {
                    trace!(
                        "{}",
                        StateMachineError::InvalidTransitionTarget {
                            graph: state_machine.graph_id,
                            from_state: state_machine.curr_state_id(),
                            to_state: *target
                        }
                    );
                }
            }
            FsmTriggerType::Next(target) => {
                if state_transitions.contains(*target) {
                    state_machine.handle_direct_transition(
                        &mut commands,
                        &action_systems,
                        state_machine_id,
                        *target,
                    );
                } else {
                    trace!(
                        "{}",
                        StateMachineError::InvalidTransitionTarget {
                            graph: state_machine.graph_id,
                            from_state: state_machine.curr_state_id(),
                            to_state: *target
                        }
                    );
                }
            }
            FsmTriggerType::Event(fsm_on_event) => {
                if let Some(target) = fsm_on_event.get_target(state_transitions) {
                    state_machine.handle_direct_transition(
                        &mut commands,
                        &action_systems,
                        state_machine_id,
                        target,
                    );
                }
            }
        };
    }

    fn handle_direct_transition(
        &self,
        commands: &mut Commands,
        action_systems: &ActionSystems,
        state_machine_id: Entity,
        to: Entity,
    ) {
        let from = self.curr_state_id();
        let resolved = action_systems.resolve_transition(from, to, state_machine_id);

        commands.queue(move |world: &mut World| -> bevy::prelude::Result<()> {
            execute_transition_steps(world, resolved)
        });
    }

    fn handle_guard_transition(
        commands: &mut Commands,
        action_systems: &ActionSystems,
        guard_registry: &GuardRegistry,
        state_machine: &FsmStateMachine,
        state_machine_id: Entity,
        guard: &GuardCondition,
        target: Entity,
    ) {
        let id = match guard_registry.to_combinator_condition_id(guard) {
            Ok(id) => id,
            Err(e) => {
                warn!(
                    "[GuardRegistry] This guard<{:?}> does not exist for state {:?}: {}",
                    guard, target, e
                );
                return;
            }
        };

        let from = state_machine.curr_state_id();
        let service_target = action_systems.service_target(state_machine_id);
        let context = GuardContext::new(service_target, state_machine_id, from, target);
        let resolved = action_systems.resolve_transition(from, target, state_machine_id);

        commands.queue(move |world: &mut World| -> bevy::prelude::Result<()> {
            if !id.run(world, context)? {
                return Ok(());
            }
            execute_transition_steps(world, resolved)
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

    #[inline]
    fn ok_and_then<T, E, R, F>(&self, res: Result<&T, E>, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> Option<R>,
    {
        res.ok().and_then(f)
    }

    pub fn get_buffer_id(&self, state: Entity) -> Option<GetBufferId> {
        self.ok_and_then(self.query_on_update_system.get(state), |update| {
            self.action_dispatch.get(update)
        })
    }

    pub fn get_enter_action_id(&self, state: Entity) -> Option<ActionId> {
        self.ok_and_then(self.query_on_enter_system.get(state), |enter| {
            self.action_registry.get(enter)
        })
    }

    pub fn get_exit_action_id(&self, state: Entity) -> Option<ActionId> {
        self.ok_and_then(self.query_on_exit_system.get(state), |exit| {
            self.action_registry.get(exit)
        })
    }

    pub fn get_enter_transition_id(&self, state: Entity) -> Option<TransitionId> {
        self.ok_and_then(self.query_before_enter_system.get(state), |enter| {
            self.transition_registry.get(enter)
        })
    }

    pub fn get_exit_transition_id(&self, state: Entity) -> Option<TransitionId> {
        self.ok_and_then(self.query_after_exit_system.get(state), |exit| {
            self.transition_registry.get(exit)
        })
    }

    /// Pre-resolves all system IDs needed for a transition from `from` to `to`.
    fn resolve_transition(
        &self,
        from: Entity,
        to: Entity,
        state_machine_id: Entity,
    ) -> ResolvedTransition {
        let service_target = self.service_target(state_machine_id);

        ResolvedTransition {
            remove_buffer: self.get_buffer_id(from).map(|id| {
                (
                    id,
                    ActionContext::new(service_target, state_machine_id, from),
                )
            }),
            exit_action: self.get_exit_action_id(from).map(|id| {
                (
                    id,
                    ActionContext::new(service_target, state_machine_id, from),
                )
            }),
            after_exit: self.get_exit_transition_id(from).map(|id| {
                (
                    id,
                    TransitionContext::with_transition(service_target, state_machine_id, from, to),
                )
            }),
            before_enter: self.get_enter_transition_id(to).map(|id| {
                (
                    id,
                    TransitionContext::with_transition(service_target, state_machine_id, from, to),
                )
            }),
            enter_action: self
                .get_enter_action_id(to)
                .map(|id| (id, ActionContext::new(service_target, state_machine_id, to))),
            add_buffer: self
                .get_buffer_id(to)
                .map(|id| (id, ActionContext::new(service_target, state_machine_id, to))),
            to,
            state_machine_id,
            service_target,
        }
    }
}

/// # FSM 蓝图
/// * 一个用于配置和创建 [`FsmStateMachine`] 实例的数据结构。
///
/// 这不是一个组件，而是一个普通的结构体，用作数据传输对象（DTO）。
/// 它的主要用途是在更复杂的结构中（例如 `HsmState`）定义一个嵌套的 FSM，
/// 允许在创建时精确控制 FSM 的初始状态和配置。
///
/// # FSM Blueprint
/// * A data structure for configuring and creating an [`FsmStateMachine`] instance.
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
