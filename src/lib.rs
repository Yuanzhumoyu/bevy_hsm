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

extern crate bevy_hsm_macros;

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
#[cfg(feature = "state_data")]
pub mod state_data;

#[cfg(feature = "hsm")]
use std::sync::Arc;

#[cfg(feature = "hsm")]
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

use crate::action_dispatcher::ActionDispatch;
use crate::guards::GuardRegistry;
use crate::state_actions::StateActionRegistry;

pub struct StateMachinePlugin {
    #[cfg(feature = "hsm")]
    transition_system: Arc<dyn for<'a> Fn(&'a mut App) + Send + Sync>,
}

#[cfg(feature = "hsm")]
impl StateMachinePlugin {
    pub fn with_transition_system<T: ScheduleLabel + Clone>(schedule: T) -> Self {
        let f = move |app: &mut App| {
            crate::hsm::transition_strategy::add_handle_on_state(app, schedule.clone());
        };
        StateMachinePlugin {
            transition_system: Arc::new(f),
        }
    }
}

impl Plugin for StateMachinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateActionRegistry>();
        app.init_resource::<ActionDispatch>();
        app.init_resource::<GuardRegistry>();

        #[cfg(feature = "hsm")]
        {
            use crate::hsm::{
                guards::{EnterGuardCache, ExitGuardCache},
                transition_strategy::CheckOnTransitionStates,
            };

            app.init_resource::<CheckOnTransitionStates>();
            app.init_resource::<EnterGuardCache>();
            app.init_resource::<ExitGuardCache>();

            (self.transition_system)(app);

            app.add_observer(hsm::state_machine::HsmStateMachine::handle_hsm_trigger);
        }

        #[cfg(feature = "fsm")]
        app.add_observer(fsm::state_machine::FsmStateMachine::handle_fsm_trigger);
    }
}

impl Default for StateMachinePlugin {
    fn default() -> Self {
        Self {
            #[cfg(feature = "hsm")]
            transition_system: Arc::new(|app: &mut App| {
                crate::hsm::transition_strategy::add_handle_on_state(app, Last);
            }),
        }
    }
}

pub mod prelude {
    pub use crate::{
        StateMachinePlugin, action_dispatcher::*, context::*, guards::*, markers::*,
        state_actions::*,
    };

    #[cfg(feature = "state_data")]
    pub use crate::state_data::{self, StateData};

    #[cfg(feature = "hsm")]
    pub use crate::hsm::{
        HsmState, guards::*, state_machine::*, state_tree::*, transition_strategy::*,
    };

    #[cfg(feature = "hsm")]
    pub use crate::bevy_hsm_macros::{hsm, hsm_tree};

    #[cfg(feature = "fsm")]
    pub use crate::fsm::{FsmState, event::*, graph::*, state_machine::*};

    #[cfg(feature = "fsm")]
    pub use crate::bevy_hsm_macros::{fsm, fsm_graph};

    pub use crate::bevy_hsm_macros::combination_condition;
}
