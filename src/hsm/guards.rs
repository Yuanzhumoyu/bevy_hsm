use std::hash::Hash;

use bevy::{
    ecs::{
        lifecycle::HookContext,
        system::{RegisteredSystemError, SystemId},
        world::DeferredWorld,
    },
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};
use smallvec::SmallVec;

use crate::{context::*, hsm::HsmState, prelude::GuardCondition};

/// 状态条件的系统ID
///
/// 用于判断[`HsmState`]是否满足进入或退出的条件,其中上下文中的实体是当前检测的实体
///
/// State condition system ID
///
/// Used to determine if [`HsmState`] meets the conditions for entering or exiting, where the context entity is the entity currently being checked
pub type GuardId = SystemId<In<GuardContext>, bool>;

/// 注册用于判断[`HsmState`]是否满足进入或退出的条件
///
/// Register to determine if [`HsmState`] meets the conditions for entering or exiting
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn is_ok(entity:In<GuardContext>) -> bool {
/// #     true
/// # }
/// # fn foo(mut commands:Commands, mut guard_registry: ResMut<GuardRegistry>) {
/// let system_id = commands.register_system(is_ok);
/// guard_registry.insert("is_ok", system_id);
/// # }
/// ```
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuardRegistry(pub(super) HashMap<String, GuardId>);

impl GuardRegistry {
    pub fn to_combinator_condition_id(&self, condition: &GuardCondition) -> Option<CompiledGuard> {
        Some(match condition {
            GuardCondition::And(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                CompiledGuard::And(condition_ids)
            }
            GuardCondition::Or(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                CompiledGuard::Or(condition_ids)
            }
            GuardCondition::Not(condition) => {
                CompiledGuard::Not(Box::new(self.to_combinator_condition_id(condition)?))
            }
            GuardCondition::Id(condition_id) => CompiledGuard::Id(self.get(condition_id)?),
        })
    }

    /// 获取一个条件
    //
    /// Get a condition
    pub fn get<Q>(&self, name: &Q) -> Option<GuardId>
    where
        Q: Hash + Equivalent<String>,
    {
        self.0.get(name).cloned()
    }

    /// 插入一个条件
    ///
    /// Insert a condition
    pub fn insert(&mut self, name: impl Into<String>, condition_id: GuardId) -> Option<GuardId> {
        self.0.insert(name.into(), condition_id)
    }

    /// 移除一个条件
    ///
    /// Remove a condition
    pub fn remove<Q>(&mut self, name: &Q) -> Option<GuardId>
    where
        Q: Hash + Equivalent<String>,
    {
        self.0.remove(name)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

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

///# 组合守卫/Combined guard
///
/// 用于组合多个守卫，支持And、Or、Not操作。
///
/// Used to combine multiple guards, supporting And, Or, Not operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledGuard {
    And(SmallVec<[Box<CompiledGuard>; 2]>),
    Or(SmallVec<[Box<CompiledGuard>; 2]>),
    Not(Box<CompiledGuard>),
    Id(GuardId),
}

impl CompiledGuard {
    pub fn new(id: GuardId) -> Self {
        Self::Id(id)
    }

    pub fn add_and(self, condition: CompiledGuard) -> Self {
        if let Self::And(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::And(condition_ids)
        } else {
            Self::And(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    pub fn add_or(self, condition: CompiledGuard) -> Self {
        if let Self::Or(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::Or(condition_ids)
        } else {
            Self::Or(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    pub fn add_not(self) -> Self {
        match self {
            Self::Not(condition) => *condition,
            _ => Self::Not(Box::new(self)),
        }
    }

    pub fn run(
        &self,
        world: &mut World,
        input: GuardContext,
    ) -> Result<bool, RegisteredSystemError<In<GuardContext>, bool>> {
        match self {
            CompiledGuard::And(ids) => {
                for id in ids {
                    if !id.run(world, input)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            CompiledGuard::Or(ors) => {
                for id in ors {
                    if id.run(world, input)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            CompiledGuard::Not(not) => not.run(world, input),
            CompiledGuard::Id(system_id) => world.run_system_with(*system_id, input),
        }
    }
}
