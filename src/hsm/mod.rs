use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use crate::{
    fsm::state_machine::FsmInitialConfiguration,
    hsm::on_transition::{ExitTransitionBehavior, StateTransitionStrategy},
};

pub mod history;
pub mod on_transition;
pub mod state_condition;
pub mod state_machine;
pub mod state_traversal;
pub mod state_tree;

/// # 状态组件\State Component
/// * 标记状态的组件，需要绑定[`HsmStateMachine`]所在实体的id
/// - Used to mark a state component, which requires the id of the entity that has the [`HsmStateMachine`] component
#[derive(Component, ScheduleLabel, Hash, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HsmState {
    pub strategy: StateTransitionStrategy,
    pub behavior: ExitTransitionBehavior,
    pub fsm_config: Option<FsmInitialConfiguration>,
}

impl HsmState {
    pub fn with(strategy: StateTransitionStrategy, behavior: ExitTransitionBehavior) -> Self {
        Self {
            strategy,
            behavior,
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
    pub fn set_fsm_group(mut self, fsm_config: Option<FsmInitialConfiguration>) -> Self {
        self.fsm_config = fsm_config;
        self
    }
}
