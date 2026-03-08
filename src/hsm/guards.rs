use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
};

use crate::{
    guards::{CompiledGuard, GuardRegistry},
    hsm::HsmState,
    prelude::GuardCondition,
};

/// 进入该状态的条件
///
/// Condition for entering this state
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
#[component(immutable, on_insert = Self::on_insert, on_remove = Self::on_remove)]
pub struct EnterGuard(pub GuardCondition);

impl EnterGuard {
    pub fn new(name: impl Into<String>) -> Self {
        Self(GuardCondition::Id(name.into()))
    }

    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let conditions = world.resource::<GuardRegistry>();
        let enter = world
            .get::<Self>(hook_context.entity)
            .expect("Component should be present in on_insert hook");
        let Some(id) = conditions.to_combinator_condition_id(&enter.0) else {
            warn!(
                "[GuardRegistry] This condition<{:?}> does not exist for state {:?}",
                enter.0, hook_context.entity
            );
            return;
        };
        let mut buffer = world.resource_mut::<EnterGuardCache>();
        buffer.insert(hook_context.entity, id);
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let mut buffer = world.resource_mut::<EnterGuardCache>();
        buffer.remove(&hook_context.entity);
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub(crate) struct EnterGuardCache(HashMap<Entity, CompiledGuard>);

impl FromWorld for EnterGuardCache {
    fn from_world(world: &mut World) -> Self {
        let collect = world.resource_scope(|world: &mut World, conditions: Mut<GuardRegistry>| {
            let mut query = world.query_filtered::<(Entity, &EnterGuard), With<HsmState>>();
            query
                .iter(world)
                .filter_map(|(id, condition)| {
                    match conditions.to_combinator_condition_id(condition) {
                        Some(condition_id) => Some((id, condition_id)),
                        None => {
                            warn!(
                                "[GuardRegistry] This condition<{:?}> does not exist",
                                condition.0
                            );
                            None
                        }
                    }
                })
                .collect::<Vec<_>>()
        });

        Self(HashMap::from_iter(collect))
    }
}

/// 退出该状态的条件
///
/// Condition for exiting this state
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
#[component(immutable, on_insert = Self::on_insert, on_remove = Self::on_remove)]
pub struct ExitGuard(pub GuardCondition);

impl ExitGuard {
    pub fn new(name: impl Into<String>) -> Self {
        Self(GuardCondition::Id(name.into()))
    }

    pub fn parse(s: impl AsRef<str>) -> Result<Self> {
        Ok(Self(GuardCondition::parse(s)?))
    }

    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let conditions = world.resource::<GuardRegistry>();
        let exit = world
            .get::<Self>(hook_context.entity)
            .expect("Component should be present in on_insert hook");
        let Some(id) = conditions.to_combinator_condition_id(&exit.0) else {
            warn!(
                "[GuardRegistry] This condition<{:?}> does not exist for state {:?}",
                exit.0, hook_context.entity
            );
            return;
        };
        let mut buffer = world.resource_mut::<ExitGuardCache>();
        buffer.insert(hook_context.entity, id);
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let mut buffer = world.resource_mut::<ExitGuardCache>();
        buffer.remove(&hook_context.entity);
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub(crate) struct ExitGuardCache(HashMap<Entity, CompiledGuard>);

impl FromWorld for ExitGuardCache {
    fn from_world(world: &mut World) -> Self {
        let collect = world.resource_scope(|world: &mut World, conditions: Mut<GuardRegistry>| {
            let mut query = world.query_filtered::<(Entity, &ExitGuard), With<HsmState>>();
            query
                .iter(world)
                .filter_map(|(id, condition)| {
                    match conditions.to_combinator_condition_id(condition) {
                        Some(condition_id) => Some((id, condition_id)),
                        None => {
                            warn!(
                                "[GuardRegistry] This condition<{:?}> does not exist",
                                condition.0
                            );
                            None
                        }
                    }
                })
                .collect::<Vec<_>>()
        });

        Self(HashMap::from_iter(collect))
    }
}
