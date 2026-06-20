use bevy::prelude::*;

use crate::interrupt::InterruptStack;

/// Trait providing unified access to the core state shared by
/// both HSM and FSM state machines.
///
/// Allows writing generic code that operates on either state machine type.
///
/// # Example
///
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_hsm::prelude::*;
///
/// fn debug_state_machine<S: StateMachineState + Component>(
///     query: Query<(Entity, &S)>,
/// ) {
///     for (entity, sm) in &query {
///         info!(
///             "State machine {:?} is in state {:?} (interrupted: {})",
///             entity,
///             sm.curr_state(),
///             sm.interrupt_stack().is_interrupted(),
///         );
///     }
/// }
/// ```
pub trait StateMachineState {
    /// Returns the currently active state entity.
    fn curr_state_id(&self) -> Entity;

    /// Returns the initial state entity.
    fn init_state_id(&self) -> Entity;

    fn state_graph_id(&self) -> Entity;

    /// Returns a shared reference to the interrupt stack.
    fn interrupt_stack(&self) -> &InterruptStack;

    /// Returns `true` if the state machine is currently in an interrupted state.
    fn is_interrupted(&self) -> bool {
        self.interrupt_stack().is_interrupted()
    }

    /// Returns the current interrupt nesting depth (0 = normal operation).
    fn interrupt_depth(&self) -> usize {
        self.interrupt_stack().interrupt_depth()
    }

    /// 获取历史记录长度
    ///
    /// Obtain the length of historical records
    #[cfg(feature = "history")]
    fn history_len(&self) -> usize;

    /// 清空状态历史队列
    ///
    /// Clear the state history queue
    #[cfg(feature = "history")]
    fn clear_history(&mut self);
}
