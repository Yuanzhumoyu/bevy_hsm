use std::{collections::VecDeque, fmt::Debug};

use bevy::prelude::*;

use crate::{
    context::{GuardContext, TransitionRelationship},
    error::StateMachineError,
    guards::{CompiledGuard, GuardRegistry},
    hsm::{
        HsmState,
        event::HsmTrigger,
        state_lifecycle::StateLifecycle,
        transition_strategy::{handle_enter_transition, handle_exit_transition},
    },
    markers::Paused,
    prelude::{ServiceTarget, StateTree},
};

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
/// let state_machine = HsmStateMachine::with(tree_id, id,#[cfg(feature = "history")] 10);
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
    pub(crate) transition_queue: TransitionQueue,
    state_tree: Entity,
    curr_state: Entity,
    /// 初始状态
    ///
    /// Initial state
    init_state: Entity,
}

impl HsmStateMachine {
    /// 创建一个新的状态机
    ///
    /// Create a new state machine
    pub fn new(
        state_tree: Entity,
        init_state: Entity,
        curr_state: Entity,
        #[cfg(feature = "history")] history_len: usize,
    ) -> Self {
        Self {
            state_tree,
            init_state,
            curr_state,
            transition_queue: TransitionQueue::default(),
            #[cfg(feature = "history")]
            history: StateHistory::new(history_len),
        }
    }

    /// 使用初始状态创建一个新的状态机，当前状态也为初始状态
    ///
    /// Create a new state machine with an initial state, the current state is also the initial state
    pub fn with(
        state_tree: Entity,
        init_state: Entity,
        #[cfg(feature = "history")] history_len: usize,
    ) -> Self {
        Self::new(
            state_tree,
            init_state,
            init_state,
            #[cfg(feature = "history")]
            history_len,
        )
    }

    /// 获取状态树
    /// Get the state tree
    pub const fn state_tree(&self) -> Entity {
        self.state_tree
    }

    /// 获取初始状态
    ///
    /// Get the initial state
    pub const fn init_state(&self) -> Entity {
        self.init_state
    }

    /// 获取当前状态的ID
    ///
    /// Get the ID of the current state
    pub const fn curr_state_id(&self) -> Entity {
        self.curr_state
    }

    /// 获取下一个状态转换
    ///
    /// Get the next state transition
    pub fn next_transition(&self) -> Transition {
        self.transition_queue.next()
    }

    /// 设置初始状态
    ///
    /// Set the initial state
    pub fn set_init_state(&mut self, state: Entity) {
        self.init_state = state;
    }

    /// 设置当前状态
    ///
    /// Set the current state
    pub fn set_curr_state(&mut self, state: Entity) {
        self.curr_state = state;
    }

    /// 添加历史记录
    ///
    /// Add history record
    #[cfg(feature = "history")]
    pub(crate) fn push_history(&mut self, node: HistoricalNode) {
        self.history.push(node);
    }

    /// 添加下一个状态
    ///
    /// Add next state
    pub fn push_next_state(&mut self, next_state: Transition) {
        self.transition_queue.push(next_state);
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

    /// 将一个状态转换插入到队列的前面
    ///
    /// Insert a state transition at the front of the queue
    pub fn push_prev_state(&mut self, prev_state: Transition) -> Transition {
        self.transition_queue.push_prev(prev_state)
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn next_state_id(&self) -> Option<Entity> {
        self.transition_queue.next().get_state_id()
    }

    /// 获取下一个状态的OnState
    ///
    /// Get the OnState of the next state
    pub fn next_state_lifecycle(&self) -> Option<StateLifecycle> {
        self.transition_queue.next().get_lifecyle()
    }

    /// 弹出下一个状态
    ///
    /// Pop next state
    pub fn pop_next_state(&mut self) -> Transition {
        self.transition_queue.pop()
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

    /// 获取状态转换队列长度
    ///
    /// Obtain the length of the state transition queue
    pub fn transition_queue_len(&self) -> usize {
        self.transition_queue.len()
    }

    /// 状态转换队列是否为空
    ///
    /// Is the state transition queue empty?
    pub fn transition_queue_is_empty(&self) -> bool {
        self.transition_queue.is_empty()
    }

    /// 检查是否处于指定状态
    ///
    /// Check if in specified state
    pub fn is_in_state(&self, state: Entity) -> bool {
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
        self.transition_queue.prev() != Transition::Start
            && self.transition_queue.next() != Transition::End
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_hsm_trigger(
        on: On<HsmTrigger>,
        mut commands: Commands,
        query_state: Query<&HsmState>,
        query_state_tree: Query<&StateTree>,
        mut query: Query<&mut HsmStateMachine, Without<Paused>>,
        query_service_target: Query<&ServiceTarget, With<HsmStateMachine>>,
        guard_registry: Res<GuardRegistry>,
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

        let state_tree_id = state_machine.state_tree();
        let curr_state_id = state_machine.curr_state_id();

        let Ok(state_tree) = query_state_tree.get(state_tree_id) else {
            warn!("{}", StateMachineError::StateTreeNotFound(state_tree_id));
            return;
        };

        match typed {
            super::event::HsmTriggerType::ToSuper => {
                if let Some(super_state_id) = state_tree.get_super_state(curr_state_id) {
                    commands.queue(handle_exit_transition(
                        state_machine_id,
                        state_tree_id,
                        curr_state_id,
                        super_state_id,
                    ));
                }
            }
            super::event::HsmTriggerType::ToSub(enter_state_id) => {
                if state_tree
                    .get_sub_states(curr_state_id)
                    .is_none_or(|sub_states| !sub_states.contains(enter_state_id))
                {
                    warn!(
                        "{}",
                        StateMachineError::SubStateNotFound {
                            state_tree: state_tree_id,
                            state: curr_state_id
                        }
                    );
                    return;
                }

                let Ok(strategy) = query_state.get(curr_state_id).map(|state| state.strategy)
                else {
                    warn!("{}", StateMachineError::HsmStateMissing(curr_state_id));
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
                    super::event::HsmTriggerType::GuardSub(guard, enter_state_id) => {
                        if state_tree
                            .get_sub_states(curr_state_id)
                            .is_none_or(|sub_states| !sub_states.contains(enter_state_id))
                        {
                            warn!(
                                "{}",
                                StateMachineError::SubStateNotFound {
                                    state_tree: state_tree_id,
                                    state: curr_state_id
                                }
                            );
                            return;
                        }

                        let Ok(strategy) =
                            query_state.get(curr_state_id).map(|state| state.strategy)
                        else {
                            warn!("{}", StateMachineError::HsmStateMissing(curr_state_id));
                            return;
                        };

                        let Some(guard) = guard_registry.to_combinator_condition_id(guard) else {
                            return;
                        };
                        let context = GuardContext::new(
                            service_target,
                            state_machine_id,
                            curr_state_id,
                            *enter_state_id,
                        );
                        let enter_state_id = *enter_state_id;
                        commands.queue(Self::handle_guard_transition(guard, context, move || {
                            handle_enter_transition(
                                state_machine_id,
                                curr_state_id,
                                enter_state_id,
                                strategy,
                            )
                        }));
                    }
                    crate::prelude::HsmTriggerType::GuardSuper(guard) => {
                        let Some(exit_state_id) = state_tree.get_super_state(curr_state_id) else {
                            warn!(
                                "{}",
                                StateMachineError::SuperStateNotFound {
                                    state_tree: state_tree_id,
                                    state: curr_state_id
                                }
                            );
                            return;
                        };

                        let Some(guard) = guard_registry.to_combinator_condition_id(guard) else {
                            return;
                        };
                        let context = GuardContext::new(
                            service_target,
                            state_machine_id,
                            curr_state_id,
                            exit_state_id,
                        );
                        commands.queue(Self::handle_guard_transition(guard, context, move || {
                            handle_exit_transition(
                                state_machine_id,
                                state_tree_id,
                                curr_state_id,
                                exit_state_id,
                            )
                        }));
                    }
                    _ => unreachable!("Unexpected HsmTriggerType: {:?}", typed),
                }
            }
        };
    }

    fn handle_guard_transition<F, C>(
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
        let curr_state_id = self.curr_state_id();
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

        if let Some((state_id, lifecycle)) = self.pop_next_state().to() {
            self.set_curr_state(state_id);
            commands.entity(state_machine_id).insert(lifecycle);
        }
    }

    fn build_transition_plan(
        curr_state_id: Entity,
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

        Self::process_enter_path(&mut next_state_table, &enter_path, query_state);

        next_state_table
    }

    fn process_exit_path(
        next_state_table: &mut Vec<Transition>,
        curr_state_id: Entity,
        exit_path: &mut Vec<Entity>,
        state_tree: &StateTree,
        query_state: &Query<&HsmState>,
    ) {
        use crate::prelude::{ExitTransitionBehavior::*, StateTransitionStrategy::*};

        exit_path.pop(); // remove LCA
        if !exit_path.is_empty() {
            next_state_table.push(Transition::Exit(curr_state_id));
        }

        let mut exit_iter = exit_path.iter().skip(1).copied().peekable();
        while let Some(super_state_id) = exit_iter.peek().copied() {
            let Ok(HsmState {
                strategy, behavior, ..
            }) = query_state.get(super_state_id)
            else {
                error!("{}", StateMachineError::HsmStateMissing(super_state_id));
                return;
            };
            match (strategy, behavior) {
                (Nested | Parallel, Resurrection) => {
                    next_state_table.push(Transition::Update(super_state_id));
                    exit_iter.next();
                }
                (Nested | Parallel, Rebirth) => {
                    next_state_table.push(Transition::Enter(super_state_id));
                    exit_iter.next();
                }
                (Nested, Death) => 'nd: {
                    if state_tree.get_root() == super_state_id {
                        next_state_table.push(Transition::Exit(super_state_id));
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
                        if state_tree.get_root() == super_state_id {
                            next_state_table
                                .push(Transition::with_behavior(super_state_id, behavior));
                            exit_iter.next();
                            break;
                        }

                        if strategy == Nested && behavior == Death {
                            next_state_table.push(Transition::Exit(super_state_id));
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
                        new_behavior = behavior;
                        exit_iter.next();
                    }
                    next_state_table.push(match new_behavior {
                        Rebirth => Transition::Enter(super_state_id),
                        Resurrection => Transition::Update(super_state_id),
                        Death => Transition::End,
                    });
                }
            }
        }
    }

    fn process_enter_path(
        next_state_table: &mut Vec<Transition>,
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

            if curr_state.strategy == Parallel && i != 0 {
                next_state_table.push(Transition::Exit(curr_state_id));
            }
            next_state_table.push(Transition::Enter(sub_state_id));
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
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Enter(Entity),
    Update(Entity),
    Exit(Entity),
    Start,
    End,
}

impl Transition {
    pub const fn to(self) -> Option<(Entity, StateLifecycle)> {
        match self {
            Transition::Enter(id) => Some((id, StateLifecycle::Enter)),
            Transition::Update(id) => Some((id, StateLifecycle::Update)),
            Transition::Exit(id) => Some((id, StateLifecycle::Exit)),
            Transition::Start | Transition::End => None,
        }
    }

    pub fn to_transition(self, next: Self) -> Option<TransitionRelationship> {
        use Transition::*;
        match (self, next) {
            // Represents the initial entry into the state machine.
            (Start, Enter(to)) | (Start, Update(to)) | (Start, Exit(to)) => {
                Some(TransitionRelationship::Final(to))
            }

            // Represents the final exit from the state machine.
            (Enter(from), End) | (Update(from), End) | (Exit(from), End) => {
                Some(TransitionRelationship::Initial(from))
            }

            // Represents a standard transition between two different states.
            (Enter(from), Enter(to))
            | (Enter(from), Update(to))
            | (Enter(from), Exit(to))
            | (Update(from), Enter(to))
            | (Update(from), Update(to))
            | (Update(from), Exit(to))
            | (Exit(from), Enter(to))
            | (Exit(from), Update(to))
            | (Exit(from), Exit(to)) => Some(TransitionRelationship::Transition(from, to)),
            // All other combinations are considered invalid transitions.
            _ => {
                error!("Invalid state transition pair: {:?} -> {:?}", self, next);
                None
            }
        }
    }

    pub const fn with_behavior(
        state_id: Entity,
        behavior: crate::prelude::ExitTransitionBehavior,
    ) -> Self {
        use crate::prelude::ExitTransitionBehavior;
        match behavior {
            ExitTransitionBehavior::Rebirth => Self::Enter(state_id),
            ExitTransitionBehavior::Resurrection => Self::Update(state_id),
            ExitTransitionBehavior::Death => Self::Exit(state_id),
        }
    }

    pub const fn with_lifecycle(state_id: Entity, lifecycle: StateLifecycle) -> Self {
        match lifecycle {
            StateLifecycle::Enter => Self::Enter(state_id),
            StateLifecycle::Update => Self::Update(state_id),
            StateLifecycle::Exit => Self::Exit(state_id),
        }
    }

    pub const fn get_state_id(&self) -> Option<Entity> {
        match self {
            Self::Enter(id) | Self::Update(id) | Self::Exit(id) => Some(*id),
            Self::Start | Self::End => None,
        }
    }

    pub const fn get_lifecyle(&self) -> Option<StateLifecycle> {
        match self {
            Transition::Enter(_) => Some(StateLifecycle::Enter),
            Transition::Update(_) => Some(StateLifecycle::Update),
            Transition::Exit(_) => Some(StateLifecycle::Exit),
            Transition::Start | Transition::End => None,
        }
    }
}

impl Debug for Transition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enter(id) => write!(f, "Enter({})", id),
            Self::Update(id) => write!(f, "Update({})", id),
            Self::Exit(id) => write!(f, "Exit({})", id),
            Self::Start => write!(f, "Start"),
            Self::End => write!(f, "End"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransitionQueue {
    prev_transition: Transition,
    next_transitions: VecDeque<Transition>,
}

impl TransitionQueue {
    pub fn push(&mut self, transition: Transition) {
        self.next_transitions.push_back(transition);
    }

    pub fn pop(&mut self) -> Transition {
        self.next_transitions.pop_front().unwrap_or(Transition::End)
    }

    pub fn next(&self) -> Transition {
        self.next_transitions
            .front()
            .copied()
            .unwrap_or(Transition::End)
    }

    pub fn push_prev(&mut self, transition: Transition) -> Transition {
        std::mem::replace(&mut self.prev_transition, transition)
    }

    pub fn prev(&self) -> Transition {
        self.prev_transition
    }

    pub fn clear(&mut self) {
        self.next_transitions.clear();
    }

    pub fn len(&self) -> usize {
        self.next_transitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.next_transitions.is_empty()
    }
}

impl Default for TransitionQueue {
    fn default() -> Self {
        Self {
            prev_transition: Transition::Start,
            next_transitions: VecDeque::new(),
        }
    }
}

impl Extend<Transition> for TransitionQueue {
    fn extend<T: IntoIterator<Item = Transition>>(&mut self, iter: T) {
        self.next_transitions.extend(iter);
    }
}
