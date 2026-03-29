use std::collections::VecDeque;

#[cfg(all(feature = "history", feature = "hybrid"))]
use bevy::ecs::entity::Entity;

use crate::hsm::state_lifecycle::StateLifecycle;

/// # 状态历史
/// * 表示一个层级状态机（HSM）访问过的状态的历史记录。
///
/// 它作为一个有容量上限的双端队列（`VecDeque`），存储了 HSM 进入的状态序列。
/// 当一个新的状态被推入时，如果历史记录超出了容量，最旧的状态就会被移除。
///
/// 当 `history` 特性启用时，这个结构体通常是 `HsmStateMachine` 的一部分。
///
/// # State History
/// * Represents the history of visited states for a Hierarchical State Machine (HSM).
///
/// It functions as a capped-size, double-ended queue (`VecDeque`) that stores the sequence
/// of states an HSM has entered. When a new state is pushed, if the history exceeds its
/// capacity, the oldest state is removed.
///
/// It is typically part of an `HsmStateMachine` when the `history` feature is enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateHistory {
    history: VecDeque<HistoricalNode>,
    /// 最大历史记录长度
    ///
    /// Max history size
    max_size: usize,
}

impl StateHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// 设置当前状态的FSM历史记录
    #[cfg(all(feature = "history", feature = "hybrid"))]
    pub fn set_last_state_fsm_history(
        &mut self,
        state: Entity,
        fsm_history: crate::fsm::history::FsmStateHistory,
    ) {
        for HistoricalNode { left_cycle, id } in self.history.iter_mut().rev() {
            if state == *id
                && let HsmStateLifecycleRecord::Update(history) = left_cycle
            {
                *history = Some(fsm_history);
                break;
            }
        }
    }

    /// 推送一个状态到历史记录中
    ///
    /// Push a state into the history
    pub fn push(&mut self, node: HistoricalNode) {
        if self.history.len() >= self.max_size {
            self.history.pop_front();
        }
        self.history.push_back(node);
    }

    /// 获取状态历史记录
    ///
    /// Get the history
    pub fn iter(&self) -> StateHistoryIterator<'_> {
        StateHistoryIterator {
            history: self,
            down: 0,
            up: self.history.len(),
        }
    }

    /// 获取当前最新记录的历史
    ///
    /// Retrieve the latest historical records   
    pub fn get_current(&self) -> Option<&HistoricalNode> {
        self.history.back()
    }

    /// 获取指定索引的历史状态
    ///
    /// Get the history state at the specified index
    pub fn get_at(&self, index: usize) -> Option<&HistoricalNode> {
        self.history.get(self.history.len().checked_sub(index + 1)?)
    }

    /// 清除历史记录
    ///
    /// Clear the history
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// 获取历史记录长度
    ///
    /// Get the history length
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// 检查历史记录是否为空
    ///
    /// Check if the history is empty
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

impl Default for StateHistory {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(10),
            max_size: 10,
        }
    }
}

pub struct StateHistoryIterator<'a> {
    history: &'a StateHistory,
    down: usize,
    up: usize,
}

impl<'a> Iterator for StateHistoryIterator<'a> {
    type Item = &'a HistoricalNode;

    fn next(&mut self) -> Option<Self::Item> {
        if self.down >= self.up {
            return None;
        }
        let node = &self.history.history[self.down];
        self.down += 1;
        Some(node)
    }
}

impl<'a> DoubleEndedIterator for StateHistoryIterator<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.down >= self.up {
            return None;
        }
        self.up -= 1;
        Some(&self.history.history[self.up])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalNode {
    id: Entity,
    left_cycle: HsmStateLifecycleRecord,
}

impl HistoricalNode {
    pub fn new(id: Entity, left_cycle: HsmStateLifecycleRecord) -> Self {
        Self { id, left_cycle }
    }

    pub fn left_cycle(&self) -> &HsmStateLifecycleRecord {
        &self.left_cycle
    }

    pub fn id(&self) -> Entity {
        self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HsmStateLifecycleRecord {
    Enter,
    #[cfg(feature = "fsm")]
    Update(Option<crate::fsm::history::FsmStateHistory>),
    #[cfg(not(feature = "fsm"))]
    Update,
    Exit,
}

impl From<HsmStateLifecycleRecord> for StateLifecycle {
    fn from(value: HsmStateLifecycleRecord) -> Self {
        match value {
            HsmStateLifecycleRecord::Enter => StateLifecycle::Enter,
            #[cfg(feature = "fsm")]
            HsmStateLifecycleRecord::Update(_) => StateLifecycle::Update,
            #[cfg(not(feature = "fsm"))]
            HsmStateLifecycleRecord::Update => StateLifecycle::Update,
            HsmStateLifecycleRecord::Exit => StateLifecycle::Exit,
        }
    }
}

impl From<StateLifecycle> for HsmStateLifecycleRecord {
    fn from(value: StateLifecycle) -> Self {
        match value {
            StateLifecycle::Enter => HsmStateLifecycleRecord::Enter,
            #[cfg(feature = "fsm")]
            StateLifecycle::Update => HsmStateLifecycleRecord::Update(None),
            #[cfg(not(feature = "fsm"))]
            StateLifecycle::Update => HsmStateLifecycleRecord::Update,
            StateLifecycle::Exit => HsmStateLifecycleRecord::Exit,
        }
    }
}
