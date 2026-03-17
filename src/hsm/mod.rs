use bevy::prelude::*;

use crate::hsm::transition_strategy::{ExitTransitionBehavior, StateTransitionStrategy};

pub mod event;
pub mod guards;
#[cfg(feature = "history")]
pub mod history;
pub mod state_machine;
pub mod state_tree;
pub mod transition_strategy;

/// # HSM 状态
/// * 一个组件，用于将一个实体标识为层级状态机（HSM）中的一个状态，并配置其行为。
///
/// 与简单的 `FsmState` 不同，`HsmState` 包含了定义其在层级结构中如何交互的关键配置。
///
/// # HSM State
/// * A component that identifies an entity as a state within a Hierarchical State Machine (HSM) and configures its behavior.
///
/// Unlike the simple `FsmState`, `HsmState` contains key configurations that define how it interacts within the hierarchy.
#[derive(Component, Hash, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HsmState {
    /// 定义了当转换到这个状态时所采用的策略（例如，是浅进入、深进入还是恢复历史状态）。
    /// Defines the strategy to be used when transitioning *into* this state (e.g., shallow, deep, or history).
    pub strategy: StateTransitionStrategy,
    /// 定义了当从这个状态转换出去时的行为。
    /// Defines the behavior when transitioning *out of* this state.
    pub behavior: ExitTransitionBehavior,
    /// (当 `fsm` 特性启用时) 允许在这个 HSM 状态内部嵌套一个完整的 FSM，从而实现“状态机中的状态机”的复杂模式。
    /// (When the `fsm` feature is enabled) Allows nesting a complete FSM within this HSM state, enabling complex "state machine within a state machine" patterns.
    #[cfg(feature = "fsm")]
    pub fsm_config: Option<crate::prelude::FsmBlueprint>,
}

impl HsmState {
    pub fn with(strategy: StateTransitionStrategy, behavior: ExitTransitionBehavior) -> Self {
        Self {
            strategy,
            behavior,
            #[cfg(feature = "fsm")]
            fsm_config: None,
        }
    }

    #[inline]
    pub fn set_strategy(mut self, strategy: StateTransitionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    #[inline]
    pub fn set_behavior(mut self, behavior: ExitTransitionBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    #[inline]
    #[cfg(feature = "fsm")]
    pub fn set_fsm_config(mut self, fsm_config: Option<crate::prelude::FsmBlueprint>) -> Self {
        self.fsm_config = fsm_config;
        self
    }
}
