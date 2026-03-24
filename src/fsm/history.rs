use std::collections::VecDeque;

use bevy::prelude::*;

/// 有限状态机状态历史记录\FSM state history
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsmStateHistory {
    history: VecDeque<Entity>,
    max_size: usize,
}

impl FsmStateHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn push(&mut self, state: Entity) {
        if self.history.len() >= self.max_size {
            self.history.pop_front();
        }
        self.history.push_back(state);
    }

    pub fn get_at(&self, index: usize) -> Option<Entity> {
        self.history
            .get(self.history.len().checked_sub(index + 1)?)
            .copied()
    }

    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, Entity> {
        self.history.iter()
    }

    pub fn take(&mut self) -> Self {
        Self {
            history: std::mem::take(&mut self.history),
            max_size: self.max_size,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}

impl Default for FsmStateHistory {
    fn default() -> Self {
        Self::new(10)
    }
}
