use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::prelude::{StateEntity, SubStates, SuperState};

/// 当拥有该组件时, 状态的优先级会被设置为该组件的值。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[component(immutable,on_insert=Self::on_insert, on_replace = Self::on_replace)]
pub struct StatePriority(pub u32);

impl StatePriority {
    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(super_state) = world.get::<SuperState>(entity).copied() else {
            return;
        };
        let priority = world.get::<StatePriority>(entity).unwrap().0;
        let state_entity = StateEntity::new(priority, entity);
        world
            .commands()
            .entity(super_state.0)
            .entry::<SubStates>()
            .and_modify(move |mut sub| {
                sub.add(state_entity);
            })
            .or_insert_with(move || SubStates::from(state_entity));
    }

    fn on_replace(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(super_state) = world.get::<SuperState>(entity).copied() else {
            return;
        };

        let priority = world.get::<StatePriority>(entity).unwrap().0;
        let state_entity = StateEntity::new(priority, entity);
        let mut sub_states = world.get_mut::<SubStates>(super_state.0).unwrap();
        sub_states.remove(&state_entity);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prelude::{SubStates, SuperState},
        state::HsmState,
    };

    use super::*;

    #[test]
    fn test_priority() {
        let mut world = World::new();
        let super_id = world
            .spawn((Name::new("state"), HsmState::new(Entity::from_bits(1))))
            .id();

        [10, 8, 1, 6, 2, 7, 4]
            .into_iter()
            .enumerate()
            .for_each(|(i, p)| {
                world.spawn((
                    Name::new(format!("state{}", i)),
                    StatePriority(p),
                    SuperState(super_id),
                    HsmState::new(Entity::from_bits(1)),
                ));
            });
        let sub_states = world.get::<SubStates>(super_id).unwrap();
        assert_eq!(
            sub_states
                .collection()
                .0
                .iter()
                .map(|s| s.priority)
                .collect::<Vec<_>>(),
            vec![1, 2, 4, 6, 7, 8, 10]
        );
    }

    #[test]
    fn test_priority_replace() {
        let mut world = World::new();
        let super_id = world
            .spawn((Name::new("state"), HsmState::new(Entity::from_bits(1))))
            .id();

        let sub_states = [10, 8, 1, 6, 2, 7, 4]
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                world
                    .spawn((
                        Name::new(format!("state{}", i)),
                        StatePriority(p),
                        SuperState(super_id),
                        HsmState::new(Entity::from_bits(1)),
                    ))
                    .id()
            })
            .collect::<Vec<_>>();
        world.entity_mut(sub_states[0]).insert(StatePriority(9));
        world.entity_mut(sub_states[2]).insert(StatePriority(5));
        world.entity_mut(sub_states[3]).insert(StatePriority(1));
        let sub_states = world.get::<SubStates>(super_id).unwrap();
        assert_eq!(
            sub_states
                .collection()
                .0
                .iter()
                .map(|s| s.priority)
                .collect::<Vec<_>>(),
            vec![1, 2, 4, 5, 7, 8, 9]
        );
    }
}
