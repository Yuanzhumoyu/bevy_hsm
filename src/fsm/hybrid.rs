use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
};

/// Maps HSM state entities to their owned FSM entities.
/// Used to track nested FSMs within an HSM.
///
/// Note: Individual FSM cleanup happens in [`handle_hybrid_exit`].
/// The `on_remove` hook cannot read `Self` because Bevy fires hooks
/// after component removal, so it serves only as a safety net if Bevy
/// ever changes this ordering.
#[derive(Component, PartialEq, Eq, Clone, Debug, Default, Deref)]
#[component(on_remove = Self::on_remove)]
pub struct HsmOwnedFsms(pub(crate) HashMap<Entity, Entity>);

impl HsmOwnedFsms {
    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        // Bevy fires on_remove hooks AFTER the component has been removed
        // from the archetype, so get::<Self>() always returns None here.
        // Owned FSM cleanup occurs in handle_hybrid_exit instead.
        let (entitys, mut commands) = world.entities_and_commands();
        let Ok(state_machine) = entitys.get(entity) else {
            return;
        };
        let Some(mapping) = state_machine.get::<Self>() else {
            // This branch is always taken — component already removed.
            // Individual FSMs are already despawned in handle_hybrid_exit;
            // for entity-despawn cleanup, see HsmStateMachine hooks.
            return;
        };

        mapping.values().copied().for_each(|fsm_id| {
            commands.entity(fsm_id).despawn();
        });
    }
}

impl From<(Entity, Entity)> for HsmOwnedFsms {
    fn from(value: (Entity, Entity)) -> Self {
        Self(HashMap::from([value]))
    }
}

/// Links an FSM instance to its parent HSM state.
/// When inserted, this component registers the FSM in the parent's [`HsmOwnedFsms`] mapping.
#[derive(Component, PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[component(on_insert = Self::on_insert)]
pub struct NestedFsm {
    pub(crate) state_machine: Entity,
    pub(crate) state: Entity,
}

impl NestedFsm {
    pub(crate) const fn new(state_machine: Entity, state: Entity) -> Self {
        Self {
            state,
            state_machine,
        }
    }

    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(child_of) = world.get::<NestedFsm>(entity).copied() else {
            return;
        };

        match world.get_mut::<HsmOwnedFsms>(child_of.state_machine) {
            Some(mut mapping) => {
                mapping.0.insert(child_of.state, entity);
            }
            None => {
                world
                    .commands()
                    .entity(child_of.state_machine)
                    .insert(HsmOwnedFsms::from((child_of.state, entity)));
            }
        }
    }
}
