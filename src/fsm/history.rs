use std::collections::VecDeque;

use bevy::prelude::*;

/// FSM 历史记录节点，包含状态 ID、所属图 ID 和记录时的中断嵌套深度。
///
/// FSM history node, containing the state ID, the graph ID it belongs to,
/// and the interrupt nesting depth at the time of recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsmHistoricalNode {
    state_id: Entity,
    graph_id: Entity,
    /// 记录时刻的中断嵌套深度。0 表示正常转换，>0 表示处于中断状态。
    ///
    /// Interrupt nesting depth at the time of recording.
    /// 0 = normal transition, >0 = during interrupt.
    interrupt_depth: usize,
}

impl FsmHistoricalNode {
    pub const fn new(state_id: Entity, graph_id: Entity, interrupt_depth: usize) -> Self {
        Self {
            state_id,
            graph_id,
            interrupt_depth,
        }
    }

    pub const fn state_id(&self) -> Entity {
        self.state_id
    }

    pub const fn graph_id(&self) -> Entity {
        self.graph_id
    }

    pub const fn interrupt_depth(&self) -> usize {
        self.interrupt_depth
    }
}

/// 有限状态机状态历史记录\FSM state history
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsmStateHistory {
    history: VecDeque<FsmHistoricalNode>,
    max_size: usize,
}

impl FsmStateHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn push(&mut self, node: FsmHistoricalNode) {
        if self.history.len() >= self.max_size {
            self.history.pop_front();
        }
        self.history.push_back(node);
    }

    pub fn get_at(&self, index: usize) -> Option<&FsmHistoricalNode> {
        self.history.get(self.history.len().checked_sub(index + 1)?)
    }

    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, FsmHistoricalNode> {
        self.history.iter()
    }

    pub fn take(&mut self) -> Self {
        Self {
            history: std::mem::take(&mut self.history),
            max_size: self.max_size,
        }
    }

    pub fn len(&self) -> usize {
        self.history.len()
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
