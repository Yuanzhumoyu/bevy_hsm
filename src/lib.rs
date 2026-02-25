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

pub mod condition;
pub mod context;
mod error;
pub mod fsm;
pub mod hook_system;
pub mod hsm;
pub mod state_machine_component;
pub mod system_state;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

use crate::{
    hook_system::NamedStateSystems,
    hsm::on_transition::{CheckOnTransitionStates, add_handle_on_state},
    prelude::{StateConditions, StateEnterConditionBuffer, StateExitConditionBuffer},
};

#[derive(Debug, Default)]
pub struct HsmPlugin<T: ScheduleLabel = Last> {
    /// 状态转换的调度器
    transition_schedule: T,
}

impl Plugin for HsmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateConditions>();
        app.init_resource::<NamedStateSystems>();
        app.init_resource::<CheckOnTransitionStates>();
        app.init_resource::<StateEnterConditionBuffer>();
        app.init_resource::<StateExitConditionBuffer>();

        app.add_observer(prelude::FsmStateMachine::observer);

        add_handle_on_state(app, self.transition_schedule.clone());
    }
}

pub mod prelude {
    pub use crate::{
        HsmPlugin,
        condition::*,
        context::*,
        fsm::{FsmState, event::*, graph::*, state_machine::*},
        hook_system::*,
        hsm::{
            HsmState, on_transition::*, state_condition::*, state_machine::*, state_traversal::*,
            state_tree::*,
        },
        state_machine_component::*,
        system_state::*,
    };

    pub use crate::bevy_hsm_macros::combination_condition;
}
