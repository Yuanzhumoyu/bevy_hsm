use std::{collections::VecDeque, fmt::Debug};

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{
    context::{ActionContext, GuardContext},
    error::StateMachineError,
    guards::CompiledGuard,
    hsm::{
        HsmState,
        event::HsmTrigger,
        state_tree::HsmStateId,
        transition_strategy::{
            CheckOnTransitionStates, handle_enter_transition, handle_exit_transition,
        },
    },
    markers::{Paused, Terminated},
    prelude::{EnterGuardCache, ExitGuardCache, ServiceTarget, StateActionBuffer, StateTree},
    state_actions::*,
};

#[cfg(feature = "state_data")]
use crate::state_data::StateData;

#[cfg(feature = "history")]
use crate::hsm::history::*;

/// 分层状态机\Hierarchical state machines
/// # 作用\Effect
/// * 管理实体的状态转换，包括当前状态、下一状态
/// - Manages entity state transitions, including current state, next state
/// # 示例\Example
///
/// ```rust
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
///
/// # fn  foo(mut commands: Commands) {
/// let id = commands.spawn_empty().id();
/// let tree_id = commands.spawn(StateTree::new(id)).id();
/// let state_machine = HsmStateMachine::with(HsmStateId::new(tree_id, id), 10);
/// # }
/// ```
#[derive(Component, Clone, PartialEq, Eq)]
pub struct HsmStateMachine {
    /// 历史记录
    ///
    /// History
    ///
    /// 记录实体的状态转换历史，用于回溯状态
    /// 最后一个状态始终为最新的状态
    ///
    /// Records entity's state transition history, used for state backtracking
    /// The last state is always the most recent state
    #[cfg(feature = "history")]
    pub history: StateHistory,
    /// 下一个状态
    ///
    /// Next state
    ///
    /// 实体下一个要转换到的状态
    ///
    /// Next state to transition to for the entity
    transition_queue: VecDeque<Transition>,
    curr_state: HsmStateId,
    /// 初始状态
    ///
    /// Initial state
    init_state: HsmStateId,
}

impl HsmStateMachine {
    pub fn new(
        init_state: HsmStateId,
        curr_state: HsmStateId,
        #[cfg(feature = "history")] history_len: usize,
    ) -> Self {
        Self {
            init_state,
            curr_state,
            transition_queue: VecDeque::new(),
            #[cfg(feature = "history")]
            history: StateHistory::new(history_len),
        }
    }

    pub fn with(init_state: HsmStateId, #[cfg(feature = "history")] history_len: usize) -> Self {
        Self {
            init_state,
            curr_state: init_state,
            transition_queue: VecDeque::new(),
            #[cfg(feature = "history")]
            history: StateHistory::new(history_len),
        }
    }

    pub const fn init_state(&self) -> HsmStateId {
        self.init_state
    }

    /// 获取当前状态的ID
    ///
    /// Get the ID of the current state
    pub const fn curr_state_id(&self) -> HsmStateId {
        self.curr_state
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn next_transition(&self) -> Option<&Transition> {
        self.transition_queue.front()
    }

    /// 设置初始状态
    ///
    /// Set the initial state
    pub fn set_init_state(&mut self, state: HsmStateId) {
        self.init_state = state;
    }

    /// 设置当前状态
    ///
    /// Set the current state
    pub fn set_curr_state(&mut self, state: HsmStateId) {
        self.curr_state = state;
    }

    /// 添加历史记录
    ///
    /// Add history record
    #[cfg(feature = "history")]
    fn push_history(&mut self, node: HistoricalNode) {
        self.history.push(node);
    }

    /// 添加下一个状态
    ///
    /// Add next state
    pub fn push_next_state(&mut self, next_state: Transition) {
        self.transition_queue.push_front(next_state);
    }

    /// 批量添加下一个状态
    ///
    /// Add multiple next states
    pub fn push_next_states<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = Transition>,
    {
        self.transition_queue.extend(iter);
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn next_state_id(&self) -> Option<HsmStateId> {
        self.transition_queue.front().and_then(|next| match next {
            Transition::Next((state_id, _)) => Some(*state_id),
            Transition::None => None,
        })
    }

    /// 获取下一个状态的OnState
    ///
    /// Get the OnState of the next state
    pub fn next_state_lifecycle(&self) -> Option<StateLifecycle> {
        self.transition_queue.front().and_then(|next| match next {
            Transition::Next((_, on_state)) => Some(*on_state),
            Transition::None => None,
        })
    }

    /// 弹出下一个状态
    ///
    /// Pop next state
    pub fn pop_next_state(&mut self) -> Transition {
        self.transition_queue
            .pop_front()
            .unwrap_or(Transition::None)
    }

    /// 获取状态历史记录
    ///
    /// Get state history
    #[cfg(feature = "history")]
    pub fn history_iter(&self) -> StateHistoryIterator<'_> {
        self.history.iter()
    }

    /// 获取历史记录长度
    ///
    /// Obtain the length of historical records
    #[cfg(feature = "history")]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// 检查是否处于指定状态
    ///
    /// Check if in specified state
    pub fn is_in_state(&self, state: HsmStateId) -> bool {
        self.curr_state_id() == state
    }

    /// 清空下一个状态队列
    ///
    /// Clear the next state queue
    pub fn clear_next_states(&mut self) {
        self.transition_queue.clear();
    }

    /// 清空状态历史队列
    ///
    /// Clear the state history queue
    #[cfg(feature = "history")]
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 检查是否正在转换状态
    ///
    /// Check if the state is transitioning
    pub fn is_transitioning(&self) -> bool {
        self.transition_queue
            .front()
            .is_some_and(|n| *n != Transition::None)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_hsm_trigger(
        on: On<HsmTrigger>,
        mut commands: Commands,
        query_state: Query<&HsmState>,
        query_state_tree: Query<&StateTree>,
        mut query: Query<&mut HsmStateMachine, Without<Paused>>,
        query_service_target: Query<&ServiceTarget, With<HsmStateMachine>>,
        enter_guard_cache: Res<EnterGuardCache>,
        exit_guard_cache: Res<ExitGuardCache>,
    ) {
        let HsmTrigger {
            state_machine,
            typed,
        } = on.event();
        let state_machine_id = *state_machine;

        let Ok(mut state_machine) = query.get_mut(state_machine_id) else {
            error!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return;
        };

        let curr_state_id = state_machine.curr_state_id();

        let Ok(state_tree) = query_state_tree.get(curr_state_id.tree()) else {
            warn!(
                "{}",
                StateMachineError::StateTreeNotFound(curr_state_id.tree())
            );
            return;
        };

        match typed {
            super::event::HsmTriggerType::ToSuper => {
                if let Some(super_state_id) = state_tree.get_super_state(curr_state_id.state()) {
                    commands.queue(handle_exit_transition(
                        state_machine_id,
                        curr_state_id,
                        super_state_id,
                    ));
                }
            }
            super::event::HsmTriggerType::ToSub(enter_state_id) => {
                if state_tree
                    .get_sub_states(curr_state_id.state())
                    .is_none_or(|sub_states| !sub_states.contains(enter_state_id))
                {
                    warn!(
                        "{}",
                        StateMachineError::SubStateNotFound {
                            state_tree: curr_state_id.tree(),
                            state: curr_state_id.state()
                        }
                    );
                    return;
                }

                let Ok(strategy) = query_state
                    .get(curr_state_id.state())
                    .map(|state| state.strategy)
                else {
                    warn!(
                        "{}",
                        StateMachineError::HsmStateMissing(curr_state_id.state())
                    );
                    return;
                };

                commands.queue(handle_enter_transition(
                    state_machine_id,
                    curr_state_id,
                    *enter_state_id,
                    strategy,
                ));
            }
            super::event::HsmTriggerType::Chain(next_state_id) => {
                state_machine.handle_state_transition(
                    &mut commands,
                    state_machine_id,
                    *next_state_id,
                    state_tree,
                    &query_state,
                );
            }
            _ => {
                let service_target = match query_service_target.get(state_machine_id) {
                    Ok(target) => target.0,
                    Err(_) => state_machine_id,
                };
                match typed {
                    super::event::HsmTriggerType::GuardedSub(enter_state_id) => {
                        if state_tree
                            .get_sub_states(curr_state_id.state())
                            .is_none_or(|sub_states| !sub_states.contains(enter_state_id))
                        {
                            warn!(
                                "{}",
                                StateMachineError::SubStateNotFound {
                                    state_tree: curr_state_id.tree(),
                                    state: curr_state_id.state()
                                }
                            );
                            return;
                        }

                        let Ok(strategy) = query_state
                            .get(curr_state_id.state())
                            .map(|state| state.strategy)
                        else {
                            warn!(
                                "{}",
                                StateMachineError::HsmStateMissing(curr_state_id.state())
                            );
                            return;
                        };

                        let Some(guard) = enter_guard_cache.get(enter_state_id).cloned() else {
                            return;
                        };
                        let context = GuardContext::new(
                            service_target,
                            state_machine_id,
                            curr_state_id.state(),
                            *enter_state_id,
                        );
                        let enter_state_id = *enter_state_id;
                        commands.queue(Self::handle_guarded_transition(
                            guard,
                            context,
                            move || {
                                handle_enter_transition(
                                    state_machine_id,
                                    curr_state_id,
                                    enter_state_id,
                                    strategy,
                                )
                            },
                        ));
                    }
                    crate::prelude::HsmTriggerType::GuardedSuper => {
                        let Some(exit_state_id) = state_tree.get_super_state(curr_state_id.state())
                        else {
                            warn!(
                                "{}",
                                StateMachineError::SuperStateNotFound {
                                    state_tree: curr_state_id.tree(),
                                    state: curr_state_id.state()
                                }
                            );
                            return;
                        };

                        let Some(guard) = exit_guard_cache.get(&exit_state_id).cloned() else {
                            return;
                        };
                        let context = GuardContext::new(
                            service_target,
                            state_machine_id,
                            curr_state_id.state(),
                            exit_state_id,
                        );
                        commands.queue(Self::handle_guarded_transition(
                            guard,
                            context,
                            move || {
                                handle_exit_transition(
                                    state_machine_id,
                                    curr_state_id,
                                    exit_state_id,
                                )
                            },
                        ));
                    }
                    _ => unreachable!(),
                }
            }
        };
    }

    fn handle_guarded_transition<F, C>(
        guard: CompiledGuard,
        context: GuardContext,
        handle_transition: F,
    ) -> impl Command<Result<()>>
    where
        F: FnOnce() -> C + Send + Sync + 'static,
        C: Command<Result<()>> + 'static,
    {
        move |world: &mut World| -> Result<()> {
            if guard.run(world, context)? {
                return handle_transition().apply(world);
            }
            Ok(())
        }
    }

    fn handle_state_transition(
        &mut self,
        commands: &mut Commands,
        state_machine_id: Entity,
        next_state_id: Entity,
        state_tree: &StateTree,
        query_state: &Query<&HsmState>,
    ) {
        let curr_state_id = self.curr_state_id().state();
        if curr_state_id == next_state_id {
            return;
        }

        let Some((exit_path, enter_path)) =
            state_tree.find_lca_and_paths(curr_state_id, next_state_id)
        else {
            error!("[HSM] Cannot find LCA for state transition");
            return;
        };

        let next_state_table = Self::build_transition_plan(
            self.curr_state_id(),
            exit_path,
            enter_path,
            state_tree,
            query_state,
        );

        self.push_next_states(next_state_table);

        if let Transition::Next((state_id, lifecycle)) = self.pop_next_state() {
            self.set_curr_state(state_id);
            commands.entity(state_machine_id).insert(lifecycle);
        }
    }

    fn build_transition_plan(
        curr_state_id: HsmStateId,
        mut exit_path: Vec<Entity>,
        enter_path: Vec<Entity>,
        state_tree: &StateTree,
        query_state: &Query<&HsmState>,
    ) -> Vec<Transition> {
        let mut next_state_table = Vec::new();

        Self::process_exit_path(
            &mut next_state_table,
            curr_state_id,
            &mut exit_path,
            state_tree,
            query_state,
        );

        Self::process_enter_path(
            &mut next_state_table,
            curr_state_id.tree(),
            &enter_path,
            query_state,
        );

        next_state_table
    }

    fn process_exit_path(
        next_state_table: &mut Vec<Transition>,
        curr_state_id: HsmStateId,
        exit_path: &mut Vec<Entity>,
        state_tree: &StateTree,
        query_state: &Query<&HsmState>,
    ) {
        use crate::prelude::{ExitTransitionBehavior::*, StateTransitionStrategy::*};

        exit_path.pop(); // remove LCA
        if !exit_path.is_empty() {
            next_state_table.push(Transition::Next((curr_state_id, StateLifecycle::Exit)));
        }

        let mut exit_iter = exit_path.iter().skip(1).copied().peekable();
        while let Some(super_state_id) = exit_iter.peek() {
            let Ok(HsmState {
                strategy, behavior, ..
            }) = query_state.get(*super_state_id)
            else {
                error!("{}", StateMachineError::HsmStateMissing(*super_state_id));
                return;
            };
            let state_id = HsmStateId::new(curr_state_id.tree(), *super_state_id);
            match (strategy, behavior) {
                (Nested | Parallel, Resurrection) => {
                    next_state_table.push(Transition::Next((state_id, StateLifecycle::Update)));
                    exit_iter.next();
                }
                (Nested | Parallel, Rebirth) => {
                    next_state_table.push(Transition::Next((state_id, StateLifecycle::Enter)));
                    exit_iter.next();
                }
                (Nested, Death) => 'nd: {
                    if state_tree.get_root() == state_id.state() {
                        next_state_table.push(Transition::Next((state_id, StateLifecycle::Exit)));
                        exit_iter.next();
                        break 'nd;
                    }
                    while let Some(super_state_id) = exit_iter.peek().copied() {
                        let Ok((strategy, behavior)) = query_state
                            .get(super_state_id)
                            .map(|state| (state.strategy, state.behavior))
                        else {
                            error!("{}", StateMachineError::HsmStateMissing(super_state_id));
                            return;
                        };
                        let state_id = HsmStateId::new(state_id.tree(), super_state_id);
                        if state_tree.get_root() == super_state_id {
                            next_state_table.push(Transition::Next((state_id, behavior.into())));
                            exit_iter.next();
                            break;
                        }

                        if strategy == Nested && behavior == Death {
                            next_state_table
                                .push(Transition::Next((state_id, StateLifecycle::Exit)));
                            exit_iter.next();
                        } else {
                            break;
                        }
                    }
                }
                (Parallel, Death) => 'bd: {
                    if exit_iter.peek().is_none() {
                        break 'bd;
                    }
                    let mut new_behavior = *behavior;
                    let mut new_state_id = state_id;
                    while let Some(super_state_id) = exit_iter.peek().copied() {
                        let Ok((strategy, behavior)) = query_state
                            .get(super_state_id)
                            .map(|state| (state.strategy, state.behavior))
                        else {
                            error!("{}", StateMachineError::HsmStateMissing(super_state_id));
                            return;
                        };
                        if !(strategy == Parallel && behavior == Death) {
                            break 'bd;
                        }
                        new_state_id = HsmStateId::new(new_state_id.tree(), super_state_id);
                        new_behavior = behavior;
                        exit_iter.next();
                    }
                    next_state_table.push(match new_behavior {
                        Rebirth => Transition::Next((state_id, StateLifecycle::Enter)),
                        Resurrection => Transition::Next((state_id, StateLifecycle::Update)),
                        Death => Transition::None,
                    });
                }
            }
        }
    }

    fn process_enter_path(
        next_state_table: &mut Vec<Transition>,
        state_tree_id: Entity,
        enter_path: &[Entity],
        query_state: &Query<&HsmState>,
    ) {
        use crate::prelude::StateTransitionStrategy::*;
        for (i, [sub_state_id, curr_state_id]) in
            enter_path.array_windows::<2>().copied().rev().enumerate()
        {
            let Ok(curr_state) = query_state.get(curr_state_id) else {
                error!("{}", StateMachineError::HsmStateMissing(curr_state_id));
                return;
            };
            let next_state_id = HsmStateId::new(state_tree_id, sub_state_id);
            if curr_state.strategy == Parallel && i != 0 {
                let curr_state_id = HsmStateId::new(state_tree_id, curr_state_id);
                next_state_table.push(Transition::Next((curr_state_id, StateLifecycle::Exit)));
            }
            next_state_table.push(Transition::Next((next_state_id, StateLifecycle::Enter)));
        }
    }
}

impl Debug for HsmStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(feature = "history")]
        {
            f.debug_struct("HsmStateMachine")
                .field("history", &self.history.iter().collect::<Vec<_>>())
                .field("transition_queue", &self.transition_queue)
                .field("curr_state", &self.curr_state)
                .field("init_state", &self.init_state)
                .finish()
        }
        #[cfg(not(feature = "history"))]
        {
            f.debug_struct("HsmStateMachine")
                .field("transition_queue", &self.transition_queue)
                .field("curr_state", &self.curr_state)
                .field("init_state", &self.init_state)
                .finish()
        }
    }
}

/// # 状态转换\State Transition
/// * 状态转换的枚举，包含下一个状态的ID和OnState
/// - The enum of state transitions, including the ID of the next state and OnState
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// 下一个状态的ID和OnState
    ///
    /// The ID of the next state and OnState
    Next((HsmStateId, StateLifecycle)),
    /// 无下一个状态
    ///
    /// No next state
    None,
}

struct TransitionContext {
    state_context: ActionContext,
    state_machine_id: Entity,
    curr_state_id: HsmStateId,
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
    fn run_lifecycle_system<T: Component + std::ops::Deref<Target = String>>(
        world: &mut DeferredWorld,
        state_id: Entity,
        state_context: ActionContext,
    ) {
        let Some(action_system_id) = ActionRegistry::get_action_id::<T>(world, state_id) else {
            return;
        };
        if let Err(e) = unsafe { world.as_unsafe_world_cell().world_mut() }
            .run_system_with(action_system_id, state_context)
        {
            let Some(system_name) = world.get::<T>(state_id) else {
                return;
            };
            error!(
                "{}",
                StateMachineError::SystemRunFailed {
                    system_name: system_name.to_string(),
                    state: state_id,
                    source: e.into()
                }
            );
        }
    }

    #[cfg(feature = "hybrid")]
    fn handle_hybrid_entry(
        world: &mut DeferredWorld,
        state_machine_id: Entity,
        curr_state_id: HsmStateId,
    ) {
        use crate::prelude::FsmGraph;

        let Some(state) = world.get::<HsmState>(curr_state_id.state()) else {
            error!(
                "{}",
                StateMachineError::HsmStateMissing(curr_state_id.state())
            );
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

        world.commands().entity(state_machine_id).insert(
            crate::fsm::state_machine::FsmStateMachine::with(
                fsm_config.graph_id,
                init_state,
                #[cfg(feature = "history")]
                fsm_config.history_size,
            ),
        );
    }

    fn prepare_transition(
        world: &mut DeferredWorld,
        hook_context: HookContext,
    ) -> Option<TransitionContext> {
        let state_machine_id = hook_context.entity;
        let (mut entitys, mut commands) = world.entities_and_commands();
        let mut entity_mut = match entitys.get_mut(state_machine_id) {
            Ok(entity_mut) => entity_mut,
            Err(e) => {
                warn!("{}", e);
                return None;
            }
        };

        let Some(hsm_state) = entity_mut.get::<StateLifecycle>().copied() else {
            warn!(
                "{}",
                StateMachineError::StateLifecycleMissing(state_machine_id)
            );
            return None;
        };
        #[cfg(feature = "hybrid")]
        if let Ok((mut fsm, mut hsm)) = entity_mut.get_components_mut::<(
            &mut crate::fsm::state_machine::FsmStateMachine,
            &mut HsmStateMachine,
        )>() {
            #[cfg(feature = "history")]
            {
                let fsm_history = fsm.history.take();
                hsm.history.set_last_state_fsm_history(fsm_history);
            }
            commands
                .entity(state_machine_id)
                .remove::<crate::fsm::state_machine::FsmStateMachine>();
        }

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

        #[cfg(feature = "history")]
        state_machine.push_history(HistoricalNode::new(curr_state_id, hsm_state.into()));

        let state_context =
            ActionContext::new(service_target, state_machine_id, curr_state_id.state());

        Some(TransitionContext {
            state_machine_id,
            curr_state_id,
            state_context,
            hsm_state,
        })
    }

    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let Some(TransitionContext {
            state_machine_id,
            curr_state_id,
            state_context,
            hsm_state,
        }) = Self::prepare_transition(&mut world, hook_context)
        else {
            return;
        };

        match hsm_state {
            StateLifecycle::Enter => {
                #[cfg(feature = "state_data")]
                StateData::clone_components(
                    &mut world,
                    curr_state_id.state(),
                    state_context.service_target,
                );

                // 运行进入系统
                Self::run_lifecycle_system::<OnEnterSystem>(
                    &mut world,
                    curr_state_id.state(),
                    state_context,
                );

                world
                    .commands()
                    .entity(state_machine_id)
                    .insert(StateLifecycle::Update);
            }
            StateLifecycle::Update => {
                // 添加过渡条件检查系统
                #[cfg(feature = "hybrid")]
                Self::handle_hybrid_entry(&mut world, state_machine_id, curr_state_id);

                let curr_state = world.entity(curr_state_id.state());
                if curr_state.contains::<OnEnterSystem>() || curr_state.contains::<OnExitSystem>() {
                    let mut check_on_transition_states =
                        world.resource_mut::<CheckOnTransitionStates>();
                    check_on_transition_states.insert(state_machine_id);
                }

                // 运行更新系统
                if world
                    .entity(curr_state_id.state())
                    .contains::<OnUpdateSystem>()
                {
                    StateActionBuffer::buffer_scope(
                        world.as_unsafe_world_cell(),
                        curr_state_id.state(),
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
                    curr_state_id.state(),
                    move |_world, buff| {
                        buff.remove_interceptor(state_context);
                        buff.add_filter(state_context);
                    },
                );

                // 运行退出系统
                Self::run_lifecycle_system::<OnExitSystem>(
                    &mut world,
                    curr_state_id.state(),
                    state_context,
                );

                #[cfg(feature = "state_data")]
                StateData::remove_components(
                    &mut world,
                    curr_state_id.state(),
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
                    let Transition::Next((curr_state, on_state)) = state_machine.pop_next_state()
                    else {
                        world.entity_mut(state_machine_id).insert(Terminated);
                        return;
                    };
                    state_machine.set_curr_state(curr_state);
                    world.entity_mut(state_machine_id).insert(on_state);
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
            let mut entity_commands = commands.entity(state_machine_id);
            while let Some(Transition::Next((curr_state, on_state))) =
                state_machine.transition_queue.pop_front()
            {
                entity_commands.queue(move |mut entity_mut: EntityWorldMut<'_>| {
                    let Some(mut state_machine) = entity_mut.get_mut::<HsmStateMachine>() else {
                        return;
                    };
                    state_machine.set_curr_state(curr_state);
                    entity_mut.insert(on_state);
                });
            }
        });
    }
}
