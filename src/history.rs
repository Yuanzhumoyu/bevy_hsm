use std::collections::VecDeque;

use crate::{state::HsmOnState, state_tree::TreeStateId};

/// 状态历史记录
///
/// State history record
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
        self.history.get(self.history.len() - index)
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
    id: TreeStateId,
    on_state: HsmOnState,
}

impl HistoricalNode {
    pub fn new(id: TreeStateId, on_state: HsmOnState) -> Self {
        Self { id, on_state }
    }

    pub fn id(&self) -> TreeStateId {
        self.id
    }

    pub fn on_state(&self) -> HsmOnState {
        self.on_state
    }
}
