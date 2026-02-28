use std::{fmt::Debug, hash::Hash};

use bevy::prelude::*;

use dyn_clone::{DynClone, clone_trait_object};
use dyn_eq::{DynEq, eq_trait_object};
use dyn_hash::{DynHash, hash_trait_object};

/// # 有限状态机状态转换事件/Finite state machine state transition event
/// # 作用\Effect
/// * 用于在状态机系统中发送状态转换事件
/// - Used to send state transition events in the state machine system
#[derive(EntityEvent, Clone)]
pub struct FsmTrigger {
    #[event_target]
    pub(crate) state_machine: Entity,
    pub(crate) typed: FsmTriggerType,
}

impl FsmTrigger {
    pub fn new(state_machine: Entity, typed: FsmTriggerType) -> Self {
        Self {
            state_machine,
            typed,
        }
    }

    pub fn with_next(state_machine: Entity, target: Entity) -> Self {
        Self {
            state_machine,
            typed: FsmTriggerType::next(target),
        }
    }

    pub fn with_condition(state_machine: Entity, target: Entity) -> Self {
        Self {
            state_machine,
            typed: FsmTriggerType::transition(target),
        }
    }

    pub fn with_event(state_machine: Entity, event: impl StateEvent + 'static) -> Self {
        Self {
            state_machine,
            typed: FsmTriggerType::event(event),
        }
    }

    pub const fn state_machine(&self) -> Entity {
        self.state_machine
    }

    pub const fn typed(&self) -> &FsmTriggerType {
        &self.typed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsmTriggerType {
    /// 直接跳转下一个状态
    Next(Entity),
    /// 根据条件跳转状态
    Transition(Entity),
    /// 根据事件跳转状态
    Event(Box<dyn StateEvent>),
}

impl FsmTriggerType {
    pub const fn next(target: Entity) -> Self {
        Self::Next(target)
    }

    pub const fn transition(target: Entity) -> Self {
        Self::Transition(target)
    }

    pub fn event(event: impl StateEvent + 'static) -> Self {
        Self::Event(Box::new(event))
    }
}

pub trait StateEvent: DynClone + DynEq + DynHash + Send + Sync + Debug + 'static {}

impl<T> StateEvent for T where T: Clone + Eq + PartialEq + Hash + Send + Sync + Debug + 'static {}

clone_trait_object!(StateEvent);
eq_trait_object!(StateEvent);
hash_trait_object!(StateEvent);
