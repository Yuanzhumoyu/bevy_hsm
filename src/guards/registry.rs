use std::borrow::Borrow;
use std::hash::Hash;

use bevy::ecs::system::RegisteredSystemError;
use bevy::platform::collections::{Equivalent, HashMap};
use bevy::prelude::*;
use smallvec::SmallVec;

use super::condition::GuardCondition;
use super::{GuardId, GuardResolveError};
use crate::context::GuardContext;
use crate::labels::SystemLabel;

/// 注册用于判断`State`是否满足进入或退出的条件
///
/// Register to determine if `State` meets the conditions for entering or exiting
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn is_ok(_:In<GuardContext>) -> bool {
/// #     true
/// # }
/// # fn foo(mut commands:Commands, mut guard_registry: ResMut<GuardRegistry>) {
/// let system_id = commands.register_system(is_ok);
/// guard_registry.insert("is_ok", system_id);
/// # }
/// ```
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuardRegistry(pub(super) HashMap<SystemLabel, GuardId>);

impl GuardRegistry {
    pub fn to_combinator_condition_id(
        &self,
        condition: &GuardCondition,
    ) -> Result<CompiledGuard, GuardResolveError> {
        match condition {
            GuardCondition::And(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                Ok(CompiledGuard::And(condition_ids))
            }
            GuardCondition::Or(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                Ok(CompiledGuard::Or(condition_ids))
            }
            GuardCondition::Not(condition) => Ok(CompiledGuard::Not(Box::new(
                self.to_combinator_condition_id(condition)?,
            ))),
            GuardCondition::Id(condition_id) => {
                let id = self
                    .get(condition_id)
                    .ok_or_else(|| GuardResolveError::UnregisteredGuard(condition_id.clone()))?;
                Ok(CompiledGuard::Id(id))
            }
        }
    }

    /// 获取一个条件
    //
    /// Get a condition
    pub fn get<Q>(&self, name: &Q) -> Option<GuardId>
    where
        Q: Hash + Equivalent<SystemLabel> + ?Sized,
    {
        self.0.get(name).cloned()
    }

    /// 插入一个条件
    ///
    /// Insert a condition
    pub fn insert(
        &mut self,
        name: impl Into<SystemLabel>,
        condition_id: GuardId,
    ) -> Option<GuardId> {
        self.0.insert(name.into(), condition_id)
    }

    /// 移除一个条件
    ///
    /// Remove a condition
    pub fn remove<Q>(&mut self, name: &Q) -> Option<GuardId>
    where
        Q: Hash + Equivalent<SystemLabel>,
        SystemLabel: Borrow<Q>,
    {
        self.0.remove(name)
    }

    /// 获取已注册守卫的数量
    ///
    /// Get the number of registered guards
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 检查守卫注册表是否为空
    ///
    /// Check if the guard registry is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<S: Into<SystemLabel>> Extend<(S, GuardId)> for GuardRegistry {
    fn extend<T: IntoIterator<Item = (S, GuardId)>>(&mut self, iter: T) {
        self.0.extend(iter.into_iter().map(|(s, a)| (s.into(), a)));
    }
}

impl<S: Into<SystemLabel>, const N: usize> From<[(S, GuardId); N]> for GuardRegistry {
    fn from(value: [(S, GuardId); N]) -> Self {
        Self(HashMap::from(value.map(|(s, a)| (s.into(), a))))
    }
}

/// # 编译后的组合守卫
///
/// * 用于在运行时执行的已编译的守卫条件。
///   [`CompiledGuard`] 是从 [`GuardCondition`] 编译而来的，它将守卫的逻辑（如 `and`, `or`, `not`）
///   与实际的 `SystemId` 结合起来，以便在状态转换时高效地执行。
///
/// # Compiled Combined Guard
///
/// * A compiled guard condition for execution at runtime.
///   [`CompiledGuard`] is compiled from [`GuardCondition`] and combines guard logic (like `and`, `or`, `not`)
///   with the actual `SystemId` for efficient execution during state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledGuard {
    And(SmallVec<[Box<CompiledGuard>; 2]>),
    Or(SmallVec<[Box<CompiledGuard>; 2]>),
    Not(Box<CompiledGuard>),
    Id(GuardId),
}

impl CompiledGuard {
    /// 从一个 `GuardId` 创建一个新的 `CompiledGuard`。
    ///
    /// Creates a new `CompiledGuard` from a `GuardId`.
    pub fn new(id: GuardId) -> Self {
        Self::Id(id)
    }

    /// 添加一个 `AND` 条件。
    ///
    /// Adds an `AND` condition.
    pub fn add_and(self, condition: CompiledGuard) -> Self {
        if let Self::And(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::And(condition_ids)
        } else {
            Self::And(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    /// 添加一个 `OR` 条件。
    ///
    /// Adds an `OR` condition.
    pub fn add_or(self, condition: CompiledGuard) -> Self {
        if let Self::Or(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::Or(condition_ids)
        } else {
            Self::Or(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    /// 添加一个 `NOT` 条件。
    ///
    /// Adds a `NOT` condition.
    pub fn add_not(self) -> Self {
        match self {
            Self::Not(condition) => *condition,
            _ => Self::Not(Box::new(self)),
        }
    }

    /// 在给定的 `World` 中运行守卫条件。
    ///
    /// Runs the guard condition in the given `World`.
    pub fn run(
        &self,
        world: &mut World,
        input: GuardContext,
    ) -> Result<bool, RegisteredSystemError<bevy::ecs::system::In<GuardContext>, bool>> {
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
            CompiledGuard::Not(not) => Ok(!not.run(world, input)?),
            CompiledGuard::Id(system_id) => input.queue_system_command(*system_id).apply(world),
        }
    }
}
