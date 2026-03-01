use bevy::prelude::*;

use crate::hsm::transition_strategy::{ExitTransitionBehavior, StateTransitionStrategy};

pub mod event;
pub mod guards;
#[cfg(feature = "history")]
pub mod history;
pub mod state_machine;
pub mod state_tree;
pub mod transition_strategy;

/// # 状态组件\State Component
/// * 标记状态的组件，需要绑定[`HsmStateMachine`]所在实体的id
/// - Used to mark a state component, which requires the id of the entity that has the [`HsmStateMachine`] component
#[derive(Component, Hash, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HsmState {
    pub strategy: StateTransitionStrategy,
    pub behavior: ExitTransitionBehavior,
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
