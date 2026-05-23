#[cfg(any(feature = "hsm", feature = "fsm"))]
use bevy::prelude::Entity;

/// A frame saved on the interrupt stack, capturing both which state graph/tree
/// the state machine was running and which specific state it was in.
///
/// When an interrupt occurs, a frame is pushed. When resuming, the frame is
/// popped and the state machine restores both the graph and the state position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InterruptFrame {
    /// The entity holding the state graph/tree (`StateTree` for HSM,
    /// `FsmGraph` for FSM).
    pub graph_id: Entity,
    /// The specific state entity that was active when the interrupt fired.
    pub state_id: Entity,
}

impl InterruptFrame {
    pub const fn new(graph_id: Entity, state_id: Entity) -> Self {
        Self { graph_id, state_id }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct InterruptStack(Vec<InterruptFrame>);

impl InterruptStack {
    /// 检查是否处于中断状态
    ///
    /// Check if currently in an interrupted state
    pub fn is_interrupted(&self) -> bool {
        !self.0.is_empty()
    }

    /// 获取当前中断嵌套深度
    ///
    /// Get the current interrupt nesting depth
    pub fn interrupt_depth(&self) -> usize {
        self.0.len()
    }

    /// 将被中断的状态图和状态压入中断栈
    ///
    /// Push an interrupted state graph and state onto the interrupt stack
    pub fn push_interrupt(&mut self, graph_id: Entity, saved_state: Entity) {
        self.0.push(InterruptFrame::new(graph_id, saved_state));
    }

    /// 从中断栈弹出最近被中断的状态帧
    ///
    /// Pop the most recently interrupted frame from the interrupt stack
    pub fn pop_interrupt(&mut self) -> Option<InterruptFrame> {
        self.0.pop()
    }

    /// 清空中断栈，放弃所有未恢复的中断
    ///
    /// Clear the interrupt stack, abandoning all pending interrupts
    pub fn clear_interrupt_stack(&mut self) {
        self.0.clear();
    }
}
