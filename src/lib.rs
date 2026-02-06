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

pub mod history;
pub mod hook_system;
mod on_transition;
// pub mod priority;
pub mod state;
pub mod state_condition;
pub mod state_traversal;
pub mod state_tree;
// pub mod sub_states;
// pub mod super_state;
pub mod system_state;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use crate::{
    hook_system::HsmOnStateDisposableSystems,
    on_transition::{CheckOnTransitionStates, add_handle_on_state},
    state_condition::StateConditions,
};

#[derive(Debug, Default)]
pub struct HsmPlugin<T: ScheduleLabel = Last> {
    /// 状态转换的调度器
    transition_schedule: T,
}

impl Plugin for HsmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateConditions>();
        app.init_resource::<HsmOnStateDisposableSystems>();
        app.init_resource::<CheckOnTransitionStates>();

        add_handle_on_state(app, self.transition_schedule.clone());
    }
}

pub mod prelude {
    pub use crate::{
        HsmPlugin, hook_system::*, on_transition::*, state::*, state_condition::*,
        state_traversal::*, state_tree::*, system_state::*,
    };

    pub use crate::bevy_hsm_macros::combination_condition;
}
