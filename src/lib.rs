//! # Bevy HSM (Hierarchical State Machine)
//!
//! 一个基于 Bevy 引擎的状态机系统，实现了分层状态机的功能。
//!
//! ## 功能特性
//!
//! - 支持状态的进入、更新和退出三个生命周期阶段
//! - 支持层次化状态（父状态和子状态）
//! - 支持状态转换条件
//! - 支持状态机系统和条件系统注册
//! -------------------------------------------------------
//! # Bevy HSM (Hierarchical State Machine)
//!
//! A hierarchical state machine system for the Bevy engine that implements hierarchical state machine functionality.
//!
//! ## Features
//!
//! - Supports state lifecycle phases: enter, update, and exit
//! - Supports hierarchical states (parent and child states)
//! - Supports state transition conditions
//! - Supports state machine system and condition system registration

pub extern crate bevy_hsm_macros;

pub mod action_dispatcher;
pub mod context;
mod error;
#[cfg(feature = "fsm")]
pub mod fsm;
pub mod guards;
#[cfg(feature = "hsm")]
pub mod hsm;
pub mod markers;
pub mod state_actions;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use crate::state_actions::StateActionRegistry;

#[derive(Debug, Default)]
#[cfg(feature = "hsm")]
pub struct StateMachinePlugin<T: ScheduleLabel = Last> {
    /// 状态转换的调度器
    transition_schedule: T,
}

#[cfg(feature = "hsm")]
impl<T: ScheduleLabel + Clone> Plugin for StateMachinePlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateActionRegistry>();

        use crate::hsm::{
            guards::{EnterGuardCache, ExitGuardCache, GuardRegistry},
            transition_strategy::{CheckOnTransitionStates, add_handle_on_state},
        };

        app.init_resource::<GuardRegistry>();
        app.init_resource::<CheckOnTransitionStates>();
        app.init_resource::<EnterGuardCache>();
        app.init_resource::<ExitGuardCache>();

        add_handle_on_state::<T>(app, self.transition_schedule.clone());

        #[cfg(feature = "fsm")]
        app.add_observer(fsm::state_machine::FsmStateMachine::handle_fsm_trigger);
    }
}

#[derive(Debug, Default)]
#[cfg(not(feature = "hsm"))]
pub struct StateMachinePlugin;

#[cfg(not(feature = "hsm"))]
impl Plugin for StateMachinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateActionRegistry>();

        #[cfg(feature = "fsm")]
        app.add_observer(fsm::state_machine::FsmStateMachine::handle_fsm_trigger);
    }
}

pub mod prelude {
    pub use crate::{
        StateMachinePlugin, action_dispatcher::*, context::*, guards::*, markers::*,
        state_actions::*,
    };

    #[cfg(feature = "hsm")]
    pub use crate::hsm::{
        HsmState, guards::*, state_machine::*, state_tree::*, transition_strategy::*,
    };

    #[cfg(feature = "fsm")]
    pub use crate::fsm::{FsmState, event::*, graph::*, state_machine::*};

    pub use crate::bevy_hsm_macros::combination_condition;
}
