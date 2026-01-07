use std::{fmt::Debug, slice::Iter};

use bevy::{ecs::relationship::RelationshipSourceCollection, prelude::*};

use crate::super_state::SuperState;

/// 用于存储子状态实体
#[derive(Component, Debug, Clone, Default)]
#[component(on_replace=<Self as RelationshipTarget>::on_replace, on_despawn=<Self as RelationshipTarget>::on_despawn)]
pub struct SubStates {
    collection: StateEntityCollection,
}

impl SubStates {
    /// 添加子状态
    ///
    /// Add Child states
    ///  #  参数\Parameters
    ///  * `entity`  - 状态实体
    ///  - `entity`  - State entity
    ///  # 返回值\Return Value
    ///  * `Some(old_priority)` -  如果状态已经存在，则返回旧的优先级
    ///  - `Some(new_priority)` -  If state already exists, returns the old priority
    ///  * `None` -  如果状态不存在，则返回None
    ///  -  `None` -  If state doesn't exist, returns None
    pub(super) fn add(&mut self, entity: StateEntity) -> Option<u32> {
        let Self { collection } = self;
        match collection.0.binary_search(&entity) {
            Ok(index) => {
                let old = collection.0.remove(index);
                let Ok(index) = collection.0.binary_search(&entity) else {
                    return None;
                };
                collection.0.insert(index, entity);
                Some(old.priority)
            }
            Err(index) => {
                collection.0.insert(index, entity);
                None
            }
        }
    }

    ///  移除一个状态
    ///
    ///  Remove a state
    ///  #  参数\ Parameters
    ///  * `name`  - 状态名称
    ///  - `name`  - Staet name
    ///  # 返回值\  Return Value
    ///  * `Some(entity)` -  移除返回状态的实体
    ///  - `Some(entity)` -  Returns the removed state entity
    ///  * `None` -  没有这个状态
    ///  - `None` -  No such state
    pub(super) fn remove(&mut self, entity: &StateEntity) -> Option<Entity> {
        let Self { collection } = self;
        let Ok(index) = collection.0.binary_search(entity) else {
            return None;
        };
        Some(collection.0.remove(index).entity)
    }

    pub fn to_vec(&self) -> Vec<Entity> {
        self.collection.0.iter().map(|e| e.entity).collect()
    }
}

impl From<StateEntity> for SubStates {
    fn from(entity: StateEntity) -> Self {
        Self {
            collection: StateEntityCollection(vec![entity]),
        }
    }
}

impl RelationshipTarget for SubStates {
    const LINKED_SPAWN: bool = false;
    type Collection = StateEntityCollection;
    type Relationship = SuperState;

    #[inline]
    fn collection(&self) -> &Self::Collection {
        &self.collection
    }

    #[inline]
    fn collection_mut_risky(&mut self) -> &mut Self::Collection {
        &mut self.collection
    }

    #[inline]
    fn from_collection_risky(collection: Self::Collection) -> Self {
        SubStates { collection }
    }
}

/// 用于给[`SubStates`]补充状态的相关信息
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StateEntity {
    pub priority: u32,
    pub entity: Entity,
}

impl StateEntity {
    pub fn new(priority: u32, entity: Entity) -> Self {
        Self { priority, entity }
    }
}

impl PartialOrd for StateEntity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StateEntity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.entity == other.entity {
            return std::cmp::Ordering::Equal;
        }
        match self.priority.cmp(&other.priority) {
            std::cmp::Ordering::Equal => self.entity.cmp(&other.entity),
            ordering => ordering,
        }
    }
}

impl Debug for StateEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StateEntity[{}/{:?}]", self.priority, self.entity)
    }
}

///  用于存储状态实体的集合, 为了对接[RelationshipTarget]::Collection, 其实现的[RelationshipSourceCollection]api无用
///
///  Used to store a collection of state entities, to interface with [RelationshipTarget]::Collection, the [RelationshipSourceCollection] api it implements is useless
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateEntityCollection(pub Vec<StateEntity>);

impl RelationshipSourceCollection for StateEntityCollection {
    type SourceIter<'a>
        = std::iter::Map<std::iter::Rev<Iter<'a, StateEntity>>, fn(&StateEntity) -> Entity>
    where
        Self: 'a;

    fn new() -> Self {
        Self(Vec::new())
    }

    fn reserve(&mut self, additional: usize) {
        Vec::reserve(&mut self.0, additional);
    }

    fn with_capacity(capacity: usize) -> Self {
        StateEntityCollection(Vec::with_capacity(capacity))
    }

    fn add(&mut self, entity: Entity) -> bool {
        Vec::push(
            &mut self.0,
            StateEntity {
                priority: 0,
                entity,
            },
        );

        true
    }

    fn remove(&mut self, entity: Entity) -> bool {
        if let Some(index) = <[StateEntity]>::iter(&self.0).position(|e| e.entity == entity) {
            Vec::remove(&mut self.0, index);
            return true;
        }

        false
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        <[StateEntity]>::iter(&self.0).rev().map(|e| e.entity)
    }

    fn len(&self) -> usize {
        Vec::len(&self.0)
    }

    fn clear(&mut self) {
        self.0.clear();
    }

    fn shrink_to_fit(&mut self) {
        Vec::shrink_to_fit(&mut self.0);
    }

    fn extend_from_iter(&mut self, entities: impl IntoIterator<Item = Entity>) {
        self.0
            .extend(entities.into_iter().map(|e| StateEntity::new(0, e)));
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn source_to_remove_before_add(&self) -> Option<Entity> {
        None
    }
}
