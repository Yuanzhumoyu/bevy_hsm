use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
};

use std::ops::{Deref, DerefMut};

use bevy::ecs::component::Mutable;

use crate::{
    guards::GuardRegistry,
    guards::registry::{CompiledGuardId, CompiledGuardRegistry},
    hsm::HsmState,
    labels::SystemLabel,
    prelude::GuardCondition,
};

/// # 进入守卫
/// * 一个附加到层级状态机（HSM）状态上的组件，定义了进入该状态必须满足的条件。
///
/// 当状态机尝试转换到一个带有 [`GuardEnter`] 的状态时，这个守卫条件会被评估。
/// 只有当条件评估为 `true` 时，转换才会被允许。
///
/// # Enter Guard
/// * A component attached to a Hierarchical State Machine (HSM) state, defining a condition
///   that must be met to enter it.
///
/// When the state machine attempts to transition to a state with an [`GuardEnter`], this guard
/// condition is evaluated. The transition is only permitted if the condition evaluates to `true`.
#[derive(Component, PartialEq, Eq, Debug, Deref, DerefMut)]
#[component(immutable, on_insert = Self::on_insert, on_remove = Self::on_remove)]
pub struct GuardEnter(pub GuardCondition);

impl GuardEnter {
    pub fn new(name: impl Into<SystemLabel>) -> Self {
        Self(GuardCondition::Id(name.into()))
    }

    fn on_insert(world: DeferredWorld, hook_context: HookContext) {
        guard_on_insert::<Self, GuardEnterCache>(world, hook_context);
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let mut buffer = world.resource_mut::<GuardEnterCache>();
        buffer.remove(&hook_context.entity);
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub(crate) struct GuardEnterCache(HashMap<Entity, CompiledGuardId>);

impl FromWorld for GuardEnterCache {
    fn from_world(world: &mut World) -> Self {
        Self(collect_guard_cache::<GuardEnter>(world))
    }
}

/// # 退出守卫
/// * 一个附加到层级状态机（HSM）状态上的组件，定义了退出该状态必须满足的条件。
///
/// 当状态机尝试从一个带有 [`GuardExit`] 的状态转换出去时，这个守卫条件会被评估。
/// 只有当条件评估为 `true` 时，转换才会被允许。
///
/// # Exit Guard
/// * A component attached to a Hierarchical State Machine (HSM) state, defining a condition
///   that must be met to exit it.
///
/// When the state machine attempts to transition away from a state with an [`GuardExit`], this
/// guard condition is evaluated. The transition is only permitted if the condition evaluates to `true`.
#[derive(Component, PartialEq, Eq, Debug, Deref, DerefMut)]
#[component(immutable, on_insert = Self::on_insert, on_remove = Self::on_remove)]
pub struct GuardExit(pub GuardCondition);

impl GuardExit {
    pub fn new(name: impl Into<SystemLabel>) -> Self {
        Self(GuardCondition::Id(name.into()))
    }

    pub fn parse(s: impl AsRef<str>) -> bevy::prelude::Result<Self> {
        Ok(Self(GuardCondition::parse(s)?))
    }

    fn on_insert(world: DeferredWorld, hook_context: HookContext) {
        guard_on_insert::<Self, GuardExitCache>(world, hook_context);
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let mut buffer = world.resource_mut::<GuardExitCache>();
        buffer.remove(&hook_context.entity);
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub(crate) struct GuardExitCache(HashMap<Entity, CompiledGuardId>);

impl FromWorld for GuardExitCache {
    fn from_world(world: &mut World) -> Self {
        Self(collect_guard_cache::<GuardExit>(world))
    }
}

/// Shared on_insert logic for GuardEnter and GuardExit.
fn guard_on_insert<G, C>(mut world: DeferredWorld, hook_context: HookContext)
where
    G: Component + Deref<Target = GuardCondition> + std::fmt::Debug,
    C: Resource<Mutability = Mutable> + DerefMut<Target = HashMap<Entity, CompiledGuardId>>,
{
    let conditions = world.resource::<GuardRegistry>();
    let Some(guard) = world.get::<G>(hook_context.entity) else {
        return;
    };
    match conditions.to_combinator_condition_id(guard) {
        Ok(id) => {
            let id = world.resource_mut::<CompiledGuardRegistry>().insert(id);
            let mut buffer = world.resource_mut::<C>();
            buffer.insert(hook_context.entity, id);
        }
        Err(e) => {
            warn!(
                "[GuardRegistry] This condition<{:?}> does not exist for state {:?}: {}",
                guard, hook_context.entity, e
            );
        }
    }
}

fn collect_guard_cache<G>(world: &mut World) -> HashMap<Entity, CompiledGuardId>
where
    G: Component + std::ops::Deref<Target = GuardCondition>,
{
    world.resource_scope(|world: &mut World, conditions: Mut<GuardRegistry>| {
        world.resource_scope(
            |world: &mut World, mut compiled_guard_registry: Mut<CompiledGuardRegistry>| {
                let mut query = world.query_filtered::<(Entity, &G), With<HsmState>>();
                query
                    .iter(world)
                    .filter_map(|(id, condition)| {
                        match conditions.to_combinator_condition_id(condition) {
                            Ok(condition_id) => {
                                let condition_id = compiled_guard_registry.insert(condition_id);
                                Some((id, condition_id))
                            }
                            Err(e) => {
                                warn!(
                                    "[GuardRegistry] This condition<{:?}> does not exist: {}",
                                    &**condition, e
                                );
                                None
                            }
                        }
                    })
                    .collect::<HashMap<_, _>>()
            },
        )
    })
}
