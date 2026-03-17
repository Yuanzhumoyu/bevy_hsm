use std::fmt::Debug;

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

use bimap::BiMap;

use crate::prelude::{GuardCondition, StateEvent};

enum FsmTransitionType {
    Unconditional,
    OnEvent,
    OnGuard,
}

/// # 传出转换
/// * 定义从单个状态出发的所有可能的转换规则。
///
/// 这个结构体被用在 `FsmGraph` 中，作为 `transitions` 哈希图的值。
/// 它包含了三种类型的转换：无条件转换、事件触发的转换和守卫条件触发的转换。
///
/// # Outgoing Transitions
/// * Defines all possible transition rules originating from a single state.
///
/// This struct is used as the value in the `FsmGraph`'s `transitions` HashMap.
/// It contains three types of transitions: unconditional, event-triggered, and guard-conditioned.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OutgoingTransitions {
    /// 无条件转换的目标状态集合。
    /// A set of target states for unconditional transitions.
    unconditional: HashSet<Entity>,
    /// 从事件到目标状态的双向映射。
    /// A bidirectional map from an event to a target state.
    on_event: BiMap<Box<dyn StateEvent>, Entity>,
    /// 从守卫条件到目标状态的双向映射。
    /// A bidirectional map from a guard condition to a target state.
    on_guard: BiMap<GuardCondition, Entity>,
}

impl OutgoingTransitions {
    fn clear_outgoing_transitions(&mut self, target: Entity, except: FsmTransitionType) {
        if !matches!(except, FsmTransitionType::Unconditional) {
            self.unconditional.remove(&target);
        }
        if !matches!(except, FsmTransitionType::OnEvent) {
            self.on_event.remove_by_right(&target);
        }
        if !matches!(except, FsmTransitionType::OnGuard) {
            self.on_guard.remove_by_right(&target);
        }
    }

    pub fn with(&mut self, target: Entity) -> &mut Self {
        self.clear_outgoing_transitions(target, FsmTransitionType::Unconditional);

        self.unconditional.insert(target);

        self
    }

    pub fn with_condition(
        &mut self,
        condition: impl Into<GuardCondition>,
        target: Entity,
    ) -> &mut Self {
        self.clear_outgoing_transitions(target, FsmTransitionType::OnGuard);

        self.on_guard.insert(condition.into(), target);

        self
    }

    pub fn with_event(&mut self, event: impl StateEvent, target: Entity) -> &mut Self {
        self.clear_outgoing_transitions(target, FsmTransitionType::OnEvent);

        self.on_event.insert(Box::new(event), target);
        self
    }

    pub fn contains(&self, target: Entity) -> bool {
        self.unconditional.contains(&target)
            || self.on_event.contains_right(&target)
            || self.on_guard.contains_right(&target)
    }

    pub fn get_by_event(&self, event: &dyn StateEvent) -> Option<Entity> {
        self.on_event.get_by_left(event).copied()
    }

    pub fn get_by_condition(&self, target: Entity) -> Option<&GuardCondition> {
        self.on_guard.get_by_right(&target)
    }

    pub fn remove(&mut self, target: Entity) -> bool {
        self.unconditional.remove(&target)
            || self.on_event.remove_by_right(&target).is_some()
            || self.on_guard.remove_by_right(&target).is_some()
    }
}

/// # FSM 图
/// * 表示一个有限状态机（FSM）的拓扑结构。
///
/// 该组件作为一个蓝图，定义了所有可能的状态以及它们之间的转换关系。
/// 它通常被附加到一个单独的“图”实体上，并被一个或多个 `FsmStateMachine` 实例所引用。
///
/// # FSM Graph
/// * Represents the topological structure of a Finite State Machine (FSM).
///
/// This component acts as a blueprint, defining all possible states and the transitions
/// between them. It is typically attached to a separate "graph" entity and is referenced
/// by one or more `FsmStateMachine` instances.
///
/// # Example
///
/// ```
/// # use bevy::prelude::*;
/// # use crate::prelude::{FsmGraph, FsmStateMachine, FsmState, StateEvent, GuardCondition};
/// #
/// # #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// # struct MyEvent;
/// # impl StateEvent for MyEvent {}
/// #
/// # fn setup(mut commands: Commands) {
/// // 1. Define states as entities
/// let idle = commands.spawn(FsmState::default()).id();
/// let walking = commands.spawn(FsmState::default()).id();
/// let running = commands.spawn(FsmState::default()).id();
///
/// // 2. Create the graph, starting in the `idle` state
/// let mut graph = FsmGraph::new(idle);
/// graph
///     // idle -> walking (unconditional)
///     .add(idle, walking)
///     // walking -> running (on event)
///     .add_event(walking, MyEvent, running)
///     // running -> idle (with a guard condition)
///     .add_condition(running, "is_tired", idle);
///
/// // 3. Spawn an entity with the graph component
/// let graph_entity = commands.spawn(graph).id();
///
/// // 4. Spawn a state machine instance that uses this graph
/// commands.spawn(FsmStateMachine::new(graph_entity));
/// # }
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct FsmGraph {
    /// FSM 的入口点或初始状态。
    /// The entry point or initial state of the FSM.
    init_state: Entity,
    /// 一个从源状态 (`Entity`) 到其 `OutgoingTransitions` 的映射，描述了所有可能的转换。
    /// A map from a source state (`Entity`) to its `OutgoingTransitions`, describing all possible transitions.
    transitions: HashMap<Entity, OutgoingTransitions>,
}

impl FsmGraph {
    pub fn new(init_state: Entity) -> Self {
        FsmGraph {
            init_state,
            transitions: HashMap::from([(init_state, OutgoingTransitions::default())]),
        }
    }

    pub fn init_state(&self) -> Entity {
        self.init_state
    }

    pub fn remove(&mut self, from: Entity, to: Entity) -> &mut Self {
        if let Some(state_transitions) = self.transitions.get_mut(&from) {
            state_transitions.remove(to);
        }
        self
    }

    pub fn get(&self, state: Entity) -> Option<&OutgoingTransitions> {
        self.transitions.get(&state)
    }

    pub fn get_mut(&mut self, state: Entity) -> Option<&mut OutgoingTransitions> {
        self.transitions.get_mut(&state)
    }

    pub fn get_mut_or_default(&mut self, state: Entity) -> &mut OutgoingTransitions {
        self.transitions.entry(state).or_default()
    }

    pub fn add(&mut self, from: Entity, to: Entity) -> &mut Self {
        self.get_mut_or_default(from).with(to);
        self
    }

    pub fn add_condition(
        &mut self,
        from: Entity,
        condition: impl Into<GuardCondition>,
        to: Entity,
    ) -> &mut Self {
        self.get_mut_or_default(from).with_condition(condition, to);
        self
    }

    pub fn add_event(&mut self, from: Entity, event: impl StateEvent, to: Entity) -> &mut Self {
        self.get_mut_or_default(from).with_event(event, to);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let mut transitions = OutgoingTransitions::default();
        let state1 = Entity::from_raw_u32(0).unwrap();
        let state2 = Entity::from_raw_u32(1).unwrap();
        let state3 = Entity::from_raw_u32(2).unwrap();
        let state4 = Entity::from_raw_u32(3).unwrap();

        #[derive(Clone, Eq, PartialEq, Hash, Debug)]
        struct MyEvent(u32);
        transitions
            .with_event(MyEvent(1), state1)
            .with_event(1, state3)
            .with_event("event", state4);

        let condition = GuardCondition::parse("test").unwrap();
        transitions.with_condition(condition.clone(), state2);

        assert_eq!(transitions.get_by_event(&MyEvent(1)), Some(state1));
        assert_eq!(transitions.get_by_event(&MyEvent(2)), None);
        assert_eq!(transitions.get_by_event(&1), Some(state3));
        assert_eq!(
            transitions.get_by_event((&Box::new(1)).as_ref()),
            Some(state3)
        );
        assert_eq!(transitions.get_by_event(&"event"), Some(state4));
        assert_eq!(transitions.get_by_condition(state2), Some(&condition));
        assert_eq!(transitions.get_by_condition(state1), None);
        assert!(transitions.contains(state1));
        assert!(transitions.contains(state2));
    }
}
