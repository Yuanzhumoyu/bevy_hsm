use std::fmt::Debug;

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{
    hsm::{
        state_lifecycle::StateLifecycle,
        transition::{Transition, TransitionQueue},
    },
    interrupt::InterruptStack,
    state_machine::StateMachineState,
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
#[component(on_remove = Self::on_remove)]
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
    pub(crate) state_tree: Entity,
    curr_state: Entity,
    /// 初始状态
    ///
    /// Initial state
    init_state: Entity,
    /// 中断状态栈，保存被中断的状态图和状态位置
    /// 支持嵌套中断，最后被中断的状态最先恢复
    ///
    /// Interrupt state stack, stores interrupted state graph and position.
    /// Supports nested interrupts; most recently interrupted state resumes first.
    pub(crate) interrupt_stack: InterruptStack,
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
            interrupt_stack: InterruptStack::default(),
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
    pub(crate) fn set_curr_state(&mut self, state: Entity) {
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

    /// 替换前一个状态转换并返回旧值
    ///
    /// Replaces the previous state transition and returns the old value
    pub fn replace_prev_state(&mut self, prev_state: Transition) -> Transition {
        self.transition_queue.replace_prev(prev_state)
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

    /// 清空下一个状态队列
    ///
    /// Clear the next state queue
    pub fn clear_next_states(&mut self) {
        self.transition_queue.clear();
    }

    /// 检查是否正在转换状态
    ///
    /// Check if the state is transitioning
    pub fn is_transitioning(&self) -> bool {
        self.transition_queue.next() != Transition::End
    }

    /// Resets the state machine to its initial state, mirroring
    /// [`FsmStateMachine::reset_to_init_state`].
    ///
    /// Clears the transition queue and interrupt stack, then forces
    /// entry into the initial state. Used by [`Terminated::on_remove`](crate::markers::Terminated).
    pub(crate) fn reset_to_init_state(world: &mut DeferredWorld, entity: Entity) {
        let Some(mut sm) = world.get_mut::<HsmStateMachine>(entity) else {
            return;
        };
        sm.clear_next_states();
        sm.interrupt_stack.clear_interrupt_stack();
        #[cfg(feature = "history")]
        sm.clear_history();

        let init = sm.init_state_id();
        sm.set_curr_state(init);
        world
            .commands()
            .entity(entity)
            .insert(StateLifecycle::Enter);
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let entity = hook_context.entity;
        world
            .resource_mut::<crate::hsm::transition_strategy::CheckOnTransitionStates>()
            .remove(&entity);

        #[cfg(feature = "hybrid")]
        if world
            .entity_mut(entity)
            .contains::<crate::fsm::hybrid::HsmOwnedFsms>()
        {
            world
                .commands()
                .entity(entity)
                .remove::<crate::fsm::hybrid::HsmOwnedFsms>();
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
                .field("interrupt_stack", &self.interrupt_stack)
                .finish()
        }
        #[cfg(not(feature = "history"))]
        {
            f.debug_struct("HsmStateMachine")
                .field("transition_queue", &self.transition_queue)
                .field("curr_state", &self.curr_state)
                .field("init_state", &self.init_state)
                .field("interrupt_stack", &self.interrupt_stack)
                .finish()
        }
    }
}

impl crate::state_machine::StateMachineState for HsmStateMachine {
    #[inline]
    fn curr_state_id(&self) -> Entity {
        self.curr_state
    }

    #[inline]
    fn init_state_id(&self) -> Entity {
        self.init_state
    }

    #[inline]
    fn state_graph_id(&self) -> Entity {
        self.state_tree
    }

    #[inline]
    fn interrupt_stack(&self) -> &InterruptStack {
        &self.interrupt_stack
    }

    #[cfg(feature = "history")]
    fn history_len(&self) -> usize {
        self.history.len()
    }

    #[cfg(feature = "history")]
    fn clear_history(&mut self) {
        self.history.clear();
    }
}

#[cfg(test)]
#[path = "../tests/hsm_state_machine_interrupt_tests.rs"]
mod interrupt_tests;
