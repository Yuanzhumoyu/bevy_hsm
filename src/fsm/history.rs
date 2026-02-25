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
            .get(self.history.len().saturating_sub(index))
            .copied()
    }

    pub fn iter(&self) -> FsmStateHistoryIterator<'_> {
        FsmStateHistoryIterator {
            history: self,
            down: 0,
            up: self.history.len(),
        }
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

pub struct FsmStateHistoryIterator<'a> {
    history: &'a FsmStateHistory,
    down: usize,
    up: usize,
}

impl<'a> Iterator for FsmStateHistoryIterator<'a> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        if self.down >= self.up {
            return None;
        }
        let node = &self.history.history[self.down];
        self.down += 1;
        Some(*node)
    }
}

impl<'a> DoubleEndedIterator for FsmStateHistoryIterator<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.down >= self.up {
            return None;
        }
        self.up -= 1;
        Some(self.history.history[self.up])
    }
}
