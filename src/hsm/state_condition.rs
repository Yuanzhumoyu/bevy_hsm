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

use crate::{context::*, hsm::HsmState, prelude::CombinationCondition};

/// 状态条件的系统ID
///
/// 用于判断[`HsmState`]是否满足进入或退出的条件,其中上下文中的实体是当前检测的实体
///
/// State condition system ID
///
/// Used to determine if [`HsmState`] meets the conditions for entering or exiting, where the context entity is the entity currently being checked
pub type StateConditionId = SystemId<In<OnStateConditionContext>, bool>;

/// 注册用于判断[`HsmState`]是否满足进入或退出的条件
///
/// Register to determine if [`HsmState`] meets the conditions for entering or exiting
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn is_ok(entity:In<HsmStateContext>) -> bool {
/// #     true
/// # }
/// # fn foo(mut commands:Commands, mut state_conditions: ResMut<StateConditions>) {
/// let system_id = commands.register_system(is_ok);
/// state_conditions.insert("is_ok", system_id);
/// # }
/// ```
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct StateConditions(pub(super) HashMap<String, StateConditionId>);

impl StateConditions {
    pub fn to_combinator_condition_id(
        &self,
        condition: &CombinationCondition,
    ) -> Option<CombinationConditionId> {
        Some(match condition {
            CombinationCondition::And(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                CombinationConditionId::And(condition_ids)
            }
            CombinationCondition::Or(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                CombinationConditionId::Or(condition_ids)
            }
            CombinationCondition::Not(condition) => {
                CombinationConditionId::Not(Box::new(self.to_combinator_condition_id(condition)?))
            }
            CombinationCondition::Id(condition_id) => {
                CombinationConditionId::Id(self.get(condition_id)?)
            }
        })
    }

    /// 获取一个条件
    //
    /// Get a condition
    pub fn get<Q>(&self, name: &Q) -> Option<StateConditionId>
    where
        Q: Hash + Equivalent<String>,
    {
        self.0.get(name).cloned()
    }

    /// 插入一个条件
    ///
    /// Insert a condition
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        condition_id: StateConditionId,
    ) -> Option<StateConditionId> {
        self.0.insert(name.into(), condition_id)
    }

    /// 移除一个条件
    ///
    /// Remove a condition
    pub fn remove<Q>(&mut self, name: &Q) -> Option<StateConditionId>
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
pub struct HsmOnEnterCondition(pub CombinationCondition);

impl HsmOnEnterCondition {
    pub fn new(name: impl Into<String>) -> Self {
        Self(CombinationCondition::Id(name.into()))
    }

    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let conditions = world.resource::<StateConditions>();
        let enter = world
            .get::<Self>(hook_context.entity)
            .expect("Component should be present in on_insert hook");
        let Some(id) = conditions.to_combinator_condition_id(&enter.0) else {
            warn!(
                "[StateConditions] This condition<{:?}> does not exist for state {:?}",
                enter.0, hook_context.entity
            );
            return;
        };
        let mut buffer = world.resource_mut::<StateEnterConditionBuffer>();
        buffer.insert(hook_context.entity, id);
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let mut buffer = world.resource_mut::<StateEnterConditionBuffer>();
        buffer.remove(&hook_context.entity);
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub(crate) struct StateEnterConditionBuffer(HashMap<Entity, CombinationConditionId>);

impl FromWorld for StateEnterConditionBuffer {
    fn from_world(world: &mut World) -> Self {
        let collect =
            world.resource_scope(|world: &mut World, conditions: Mut<StateConditions>| {
                let mut query =
                    world.query_filtered::<(Entity, &HsmOnEnterCondition), With<HsmState>>();
                query
                    .iter(world)
                    .filter_map(|(id, condition)| {
                        match conditions.to_combinator_condition_id(condition) {
                            Some(condition_id) => Some((id, condition_id)),
                            None => {
                                warn!(
                                    "[StateConditions] This condition<{:?}> does not exist",
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
pub struct HsmOnExitCondition(pub CombinationCondition);

impl HsmOnExitCondition {
    pub fn new(name: impl Into<String>) -> Self {
        Self(CombinationCondition::Id(name.into()))
    }

    pub fn parse(s: impl AsRef<str>) -> Result<Self> {
        Ok(Self(CombinationCondition::parse(s)?))
    }

    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let conditions = world.resource::<StateConditions>();
        let exit = world
            .get::<Self>(hook_context.entity)
            .expect("Component should be present in on_insert hook");
        let Some(id) = conditions.to_combinator_condition_id(&exit.0) else {
            warn!(
                "[StateConditions] This condition<{:?}> does not exist for state {:?}",
                exit.0, hook_context.entity
            );
            return;
        };
        let mut buffer = world.resource_mut::<StateExitConditionBuffer>();
        buffer.insert(hook_context.entity, id);
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let mut buffer = world.resource_mut::<StateExitConditionBuffer>();
        buffer.remove(&hook_context.entity);
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub(crate) struct StateExitConditionBuffer(HashMap<Entity, CombinationConditionId>);

impl FromWorld for StateExitConditionBuffer {
    fn from_world(world: &mut World) -> Self {
        let collect =
            world.resource_scope(|world: &mut World, conditions: Mut<StateConditions>| {
                let mut query =
                    world.query_filtered::<(Entity, &HsmOnExitCondition), With<HsmState>>();
                query
                    .iter(world)
                    .filter_map(|(id, condition)| {
                        match conditions.to_combinator_condition_id(condition) {
                            Some(condition_id) => Some((id, condition_id)),
                            None => {
                                warn!(
                                    "[StateConditions] This condition<{:?}> does not exist",
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

/// 组合条件ID
///
/// Combination condition ID
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombinationConditionId {
    And(SmallVec<[Box<CombinationConditionId>; 2]>),
    Or(SmallVec<[Box<CombinationConditionId>; 2]>),
    Not(Box<CombinationConditionId>),
    Id(StateConditionId),
}

impl CombinationConditionId {
    pub fn new(id: StateConditionId) -> Self {
        Self::Id(id)
    }

    pub fn add_and(self, condition: CombinationConditionId) -> Self {
        if let Self::And(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::And(condition_ids)
        } else {
            Self::And(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    pub fn add_or(self, condition: CombinationConditionId) -> Self {
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
        input: OnStateConditionContext,
    ) -> Result<bool, RegisteredSystemError<In<OnStateConditionContext>, bool>> {
        match self {
            CombinationConditionId::And(ids) => {
                for id in ids {
                    if !id.run(world, input)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            CombinationConditionId::Or(ors) => {
                for id in ors {
                    if id.run(world, input)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            CombinationConditionId::Not(not) => not.run(world, input),
            CombinationConditionId::Id(system_id) => world.run_system_with(*system_id, input),
        }
    }
}
