use std::{any::type_name, fmt::Debug, sync::Arc};

use bevy::{ecs::world::World, prelude::Entity};

/// 一个用于定义子状态应如何遍历的 trait。
///
/// 此 trait 的实现将决定子状态在激活或其他操作中被考虑的顺序。
pub trait StateTraversalStrategy: Send + Sync + 'static {
    /// 给定一个子状态实体列表，按照期望的遍历顺序返回它们。
    fn traverse(&self, world: &World, children: &[Entity]) -> Vec<Entity>;

    fn name(&self) -> &'static str {
        type_name::<Self>()
    }
}

/// 一个包装结构体，用于持有动态的 `StateTraversalStrategy`。
///
/// 这允许在运行时互换使用不同的遍历策略。
pub struct TraversalStrategy(pub(crate) Arc<dyn StateTraversalStrategy>);

impl TraversalStrategy {
    /// 使用给定的实现创建一个新的 `TraversalStrategy`。
    pub fn new<T: StateTraversalStrategy>(strategy: T) -> Self {
        Self(Arc::new(strategy))
    }
}

impl Eq for TraversalStrategy {}

impl PartialEq for TraversalStrategy {
    fn eq(&self, other: &Self) -> bool {
        self.0.name() == other.0.name()
    }
}

impl Clone for TraversalStrategy {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl Default for TraversalStrategy {
    fn default() -> Self {
        Self(Arc::new(SequentialTraversal))
    }
}

impl Debug for TraversalStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.name())
    }
}

/// 一个基本的顺序遍历策略。
///
/// 此策略简单地按照提供的顺序返回子状态。
pub struct SequentialTraversal;

impl StateTraversalStrategy for SequentialTraversal {
    fn traverse(&self, _world: &World, children: &[Entity]) -> Vec<Entity> {
        children.to_vec()
    }
}

pub struct ReverseTraversal;

impl StateTraversalStrategy for ReverseTraversal {
    fn traverse(&self, _world: &World, children: &[Entity]) -> Vec<Entity> {
        children.iter().rev().cloned().collect()
    }
}
