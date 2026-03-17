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

/// # 进入守卫
/// * 一个附加到层级状态机（HSM）状态上的组件，定义了进入该状态必须满足的条件。
///
/// 当状态机尝试转换到一个带有 `EnterGuard` 的状态时，这个守卫条件会被评估。
/// 只有当条件评估为 `true` 时，转换才会被允许。
///
/// # Enter Guard
/// * A component attached to a Hierarchical State Machine (HSM) state, defining a condition
///   that must be met to enter it.
///
/// When the state machine attempts to transition to a state with an `EnterGuard`, this guard
/// condition is evaluated. The transition is only permitted if the condition evaluates to `true`.
#[derive(Component, PartialEq, Eq, Debug, Deref, DerefMut)]
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

/// # 退出守卫
/// * 一个附加到层级状态机（HSM）状态上的组件，定义了退出该状态必须满足的条件。
///
/// 当状态机尝试从一个带有 `ExitGuard` 的状态转换出去时，这个守卫条件会被评估。
/// 只有当条件评估为 `true` 时，转换才会被允许。
///
/// # Exit Guard
/// * A component attached to a Hierarchical State Machine (HSM) state, defining a condition
///   that must be met to exit it.
///
/// When the state machine attempts to transition away from a state with an `ExitGuard`, this
/// guard condition is evaluated. The transition is only permitted if the condition evaluates to `true`.
#[derive(Component, PartialEq, Eq, Debug, Deref, DerefMut)]
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
