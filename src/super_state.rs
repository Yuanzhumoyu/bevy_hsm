use bevy::{
    ecs::{
        error::CommandWithEntity,
        lifecycle::HookContext,
        relationship::{Relationship, RelationshipHookMode, RelationshipSourceCollection},
        world::DeferredWorld,
    },
    prelude::*,
};

use crate::{
    prelude::StateEntity, priority::StatePriority, state::HsmState, sub_states::SubStates,
};

/// 用于存储父状态
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[require(Name = Name::new("state"))]
#[component(on_insert=<Self as Relationship>::on_insert, on_replace=<Self as Relationship>::on_replace)]
pub struct SuperState(pub Entity);

impl Relationship for SuperState {
    type RelationshipTarget = SubStates;

    fn from(entity: Entity) -> Self {
        Self(entity)
    }

    fn get(&self) -> Entity {
        self.0
    }

    fn set_risky(&mut self, entity: Entity) {
        *self = Self(entity);
    }

    fn on_insert(
        mut world: DeferredWorld,
        HookContext {
            entity,
            caller,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {}
            RelationshipHookMode::Skip => return,
            RelationshipHookMode::RunIfNotLinked => {
                if <Self::RelationshipTarget as RelationshipTarget>::LINKED_SPAWN {
                    return;
                }
            }
        }
        let entity_ref = world.entity(entity);
        if entity_ref.get::<HsmState>().is_none() {
            warn!(
                "Entity {:?} does not have a HsmState component, cannot create SuperState relationship",
                entity
            );
            world.commands().entity(entity).remove::<Self>();
            return;
        }
        let Some(target_entity) = entity_ref.get::<Self>().map(|s| s.get()) else {
            warn!(
                "Entity {:?} does not have a SuperState component, cannot create relationship",
                entity
            );
            world.commands().entity(entity).remove::<Self>();
            return;
        };
        if target_entity == entity {
            warn!(
                "{}The {}({target_entity:?}) relationship on entity {entity:?} points to itself. The invalid {} relationship has been removed.",
                caller
                    .map(|location| format!("{location}: "))
                    .unwrap_or_default(),
                DebugName::type_name::<Self>(),
                DebugName::type_name::<Self>()
            );
            world.commands().entity(entity).remove::<Self>();
            return;
        }
        // For one-to-one relationships, remove existing relationship before adding new one
        let current_source_to_remove = world
            .get_entity(target_entity)
            .ok()
            .and_then(|target_entity_ref| target_entity_ref.get::<Self::RelationshipTarget>())
            .and_then(|relationship_target| {
                relationship_target
                    .collection()
                    .source_to_remove_before_add()
            });

        if let Some(current_source) = current_source_to_remove {
            world.commands().entity(current_source).try_remove::<Self>();
        }
    }

    fn on_replace(
        mut world: DeferredWorld,
        HookContext {
            entity,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {}
            RelationshipHookMode::Skip => return,
            RelationshipHookMode::RunIfNotLinked => {
                if <Self::RelationshipTarget as RelationshipTarget>::LINKED_SPAWN {
                    return;
                }
            }
        }
        let Some(target_entity) = world.entity(entity).get::<Self>().map(|s| s.get()) else {
            warn!("Entity {:?} does not have a SuperState component", entity);
            return;
        };
        if let Ok(mut target_entity_mut) = world.get_entity_mut(target_entity)
            && let Some((mut relationship_target, priority)) = unsafe {
                target_entity_mut
                    .get_components_mut_unchecked::<(&mut Self::RelationshipTarget, &StatePriority)>()
            }
        {
            relationship_target.remove(&StateEntity::new(priority.0, entity));
            if relationship_target.len() == 0 {
                let command = |mut entity: EntityWorldMut| {
                    // this "remove" operation must check emptiness because in the event that an identical
                    // relationship is inserted on top, this despawn would result in the removal of that identical
                    // relationship ... not what we want!
                    if entity
                        .get::<Self::RelationshipTarget>()
                        .is_some_and(RelationshipTarget::is_empty)
                    {
                        entity.remove::<Self::RelationshipTarget>();
                    }
                };

                world
                    .commands()
                    .queue_silenced(command.with_entity(target_entity));
            }
        }
    }
}
