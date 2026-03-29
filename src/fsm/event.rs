use std::{fmt::Debug, hash::Hash};

use bevy::prelude::*;

use dyn_clone::{DynClone, clone_trait_object};
use dyn_eq::{DynEq, eq_trait_object};
use dyn_hash::{DynHash, hash_trait_object};

use crate::prelude::OutgoingTransitions;

/// # FSM 触发器
/// * 用于驱动有限状态机（FSM）进行状态转换的核心事件。
///
/// 当这个事件被发送时，它会指定目标 `FsmStateMachine` 实体，并附带一个 `FsmTriggerType`，
/// 该类型描述了要执行的转换的具体种类（例如，无条件转换、事件触发的转换或带守卫的转换）。
///
/// # FSM Trigger
/// * The core event used to drive state transitions in a Finite State Machine (FSM).
///
/// When this event is sent, it specifies the target `FsmStateMachine` entity and includes an
/// `FsmTriggerType`, which describes the specific kind of transition to perform (e.g., an
/// unconditional, event-triggered, or guard-conditioned transition).
///
/// # Example
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// #
/// # #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// # struct MyEvent;
/// #
/// # fn fsm_system(mut commands: Commands) {
/// # // Define states
/// # let idle = commands.spawn(FsmState::default()).id();
/// # let walking = commands.spawn(FsmState::default()).id();
/// #
/// # // Define graph
/// # let mut graph = FsmGraph::new(idle);
/// # graph.with_add(idle, walking);
/// # let graph_id = commands.spawn(graph).id();
/// #
/// # // Spawn state machine
/// # let sm_entity = commands.spawn(FsmStateMachine::with(graph_id, idle,#[cfg(feature = "history")] 10)).id();
/// #
/// // To trigger an unconditional transition to a specific state:
/// commands.trigger(FsmTrigger::with_next(sm_entity, walking));
///
/// // To trigger a transition based on an event:
/// commands.trigger(FsmTrigger::with_event(sm_entity, MyEvent));
///
/// // To trigger a transition that needs to be checked by a guard:
/// commands.trigger(FsmTrigger::with_guard(sm_entity, idle));
/// # }
/// ```
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

    pub fn with_guard(state_machine: Entity, target: Entity) -> Self {
        Self {
            state_machine,
            typed: FsmTriggerType::guard(target),
        }
    }

    pub fn with_event(state_machine: Entity, event: impl StateEventType) -> Self {
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

#[derive(Clone, PartialEq, Eq)]
pub enum FsmTriggerType {
    /// 直接跳转下一个状态
    Next(Entity),
    /// 根据条件跳转状态
    Guard(Entity),
    /// 根据事件跳转状态
    Event(Box<dyn StateEventType>),
}

impl FsmTriggerType {
    pub const fn next(target: Entity) -> Self {
        Self::Next(target)
    }

    pub const fn guard(target: Entity) -> Self {
        Self::Guard(target)
    }

    pub fn event(event: impl StateEventType + 'static) -> Self {
        Self::Event(Box::new(event))
    }
}

impl Debug for FsmTriggerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Next(target) => f.debug_tuple("Next").field(target).finish(),
            Self::Guard(target) => f.debug_tuple("Guard").field(target).finish(),
            Self::Event(event) => f.debug_tuple("Event").field(event).finish(),
        }
    }
}

pub trait StateEvent: DynClone + DynEq + DynHash + Send + Sync + Debug + 'static {}

impl<T> StateEvent for T where T: Clone + Eq + PartialEq + Hash + Send + Sync + Debug + 'static {}

clone_trait_object!(StateEvent);
eq_trait_object!(StateEvent);
hash_trait_object!(StateEvent);

pub trait StateEventType: DynClone + DynEq + Debug + Send + Sync {
    fn get_target(&mut self, state_transitions: &OutgoingTransitions) -> Option<Entity>;
}

clone_trait_object!(StateEventType);
eq_trait_object!(StateEventType);

#[derive(PartialEq, Debug, Eq, Clone)]
pub struct EventData(Box<dyn StateEvent>);

impl EventData {
    pub fn new(event: impl StateEvent + 'static) -> Self {
        Self(Box::new(event))
    }
}

impl StateEventType for EventData {
    fn get_target(&mut self, state_transitions: &OutgoingTransitions) -> Option<Entity> {
        state_transitions.get_by_event(self.0.as_ref())
    }
}

macro_rules! impl_state_event_type_for_ranges {
    ($($range:ident),*) => {
        $(
            impl<T: StateEvent> StateEventType for std::ops::$range<T>
            where
                std::ops::$range<T>: std::iter::Iterator<Item = T> + Debug + Clone + Eq,
            {
                fn get_target(&mut self, state_transitions: &OutgoingTransitions) -> Option<Entity> {
                    self.find_map(|v|state_transitions.get_by_event(&v))
                }
            }
        )*
    };
}

impl_state_event_type_for_ranges!(Range, RangeInclusive, RangeFrom);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_event_type_for_ranges() {
        let [a, b, c, d] = [0, 1, 2, 3].map(|i| Entity::from_raw_u32(i).unwrap());

        let mut state_transitions = OutgoingTransitions::default();
        state_transitions.with_event(1, a);
        state_transitions.with_event(2, b);
        state_transitions.with_event(16, c);
        state_transitions.with_event(4, d);

        fn get(
            value: &mut dyn StateEventType,
            state_transitions: &OutgoingTransitions,
        ) -> Option<Entity> {
            value.get_target(state_transitions)
        }

        let target = get(&mut EventData::new(16), &state_transitions);
        assert_eq!(target, Some(c));

        let target = get(&mut (0..4), &state_transitions);
        assert_eq!(target, Some(a));

        let target = get(&mut (2..=3), &state_transitions);
        assert_eq!(target, Some(b));

        let target = get(&mut (16..), &state_transitions);
        assert_eq!(target, Some(c));
    }
}
