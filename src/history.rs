use std::collections::VecDeque;

use bevy::ecs::entity::Entity;

/// 状态历史记录
///
/// State history record
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateHistory {
    history: VecDeque<Entity>,
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
    pub fn push(&mut self, state: Entity) {
        if self.history.len() >= self.max_size {
            self.history.pop_front();
        }
        self.history.push_back(state);
    }

    /// 获取状态历史记录
    ///
    /// Get the history
    pub fn get_history(&self) -> Vec<Entity> {
        self.history.iter().copied().collect()
    }

    /// 获取当前状态
    ///
    /// Get the current state
    pub fn get_current(&self) -> Option<Entity> {
        self.history.back().copied()
    }

    /// 获取上一个状态
    ///
    /// Get the previous state
    pub fn get_previous(&self) -> Option<Entity> {
        if self.history.len() < 2 {
            None
        } else {
            self.history.iter().rev().nth(1).copied()
        }
    }

    /// 获取指定索引的历史状态 (0是当前状态，1是上一个状态，等等)
    ///
    /// Get the history state at the specified index (0 is the current state, 1 is the previous state, etc.)
    pub fn get_at(&self, index: usize) -> Option<Entity> {
        if index < self.history.len() {
            self.history.iter().rev().nth(index).copied()
        } else {
            None
        }
    }

    /// 清除历史记录（保留当前状态）
    ///
    /// Clear the history (keep the current state)
    pub fn clear_history_except_current(&mut self) {
        if let Some(current) = self.history.back().cloned() {
            self.history.clear();
            self.history.push_back(current);
        }
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
