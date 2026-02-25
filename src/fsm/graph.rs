use std::fmt::Debug;

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

use bimap::BiMap;

use crate::{fsm::event::FsmOnEvent, prelude::CombinationCondition};

#[derive(Debug, Default, Clone)]
pub struct StateTransitions {
    by: HashSet<Entity>,
    by_event: BiMap<Box<dyn FsmOnEvent>, Entity>,
    by_condition: BiMap<CombinationCondition, Entity>,
}
impl StateTransitions {
    pub fn with(&mut self, target: Entity) -> &mut Self {
        'value: {
            if self.by_event.remove_by_right(&target).is_some() {
                break 'value;
            }

            self.by_condition.remove_by_right(&target);
        };

        self.by.insert(target);

        self
    }

    pub fn with_condition(
        &mut self,
        condition: impl Into<CombinationCondition>,
        target: Entity,
    ) -> &mut Self {
        'value: {
            if self.by.remove(&target) {
                break 'value;
            }

            self.by_event.remove_by_right(&target);
        };

        self.by_condition.insert(condition.into(), target);

        self
    }

    pub fn with_event(&mut self, event: impl FsmOnEvent, target: Entity) -> &mut Self {
        'value: {
            if self.by.remove(&target) {
                break 'value;
            }

            self.by_condition.remove_by_right(&target);
        };

        self.by_event.insert(Box::new(event), target);
        self
    }

    pub fn contains(&self, target: Entity) -> bool {
        self.by.contains(&target)
            || self.by_event.contains_right(&target)
            || self.by_condition.contains_right(&target)
    }

    pub fn get_by_event(&self, event: &dyn FsmOnEvent) -> Option<Entity> {
        self.by_event.get_by_left(event).copied()
    }

    pub fn get_by_condition(&self, target: Entity) -> Option<&CombinationCondition> {
        self.by_condition.get_by_right(&target)
    }

    pub fn remove(&mut self, target: Entity) -> bool {
        self.by.remove(&target)
            || self.by_event.remove_by_right(&target).is_some()
            || self.by_condition.remove_by_right(&target).is_some()
    }
}

#[derive(Component, Debug, Clone)]
pub struct FsmGraph {
    init_state: Entity,
    transitions: HashMap<Entity, StateTransitions>,
}

impl FsmGraph {
    pub fn new(init_state: Entity) -> Self {
        FsmGraph {
            init_state,
            transitions: HashMap::from([(init_state, StateTransitions::default())]),
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

    pub fn get(&self, state: Entity) -> Option<&StateTransitions> {
        self.transitions.get(&state)
    }

    pub fn get_mut(&mut self, state: Entity) -> Option<&mut StateTransitions> {
        self.transitions.get_mut(&state)
    }

    pub fn get_mut_or_default(&mut self, state: Entity) -> &mut StateTransitions {
        self.transitions.entry(state).or_default()
    }

    pub fn add(&mut self, from: Entity, to: Entity) -> &mut Self {
        self.get_mut_or_default(from).with(to);
        self
    }

    pub fn add_condition(
        &mut self,
        from: Entity,
        condition: impl Into<CombinationCondition>,
        to: Entity,
    ) -> &mut Self {
        self.get_mut_or_default(from).with_condition(condition, to);
        self
    }

    pub fn add_event(&mut self, from: Entity, event: impl FsmOnEvent, to: Entity) -> &mut Self {
        self.get_mut_or_default(from).with_event(event, to);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let mut transitions = StateTransitions::default();
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

        let condition = CombinationCondition::parse("test").unwrap();
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
