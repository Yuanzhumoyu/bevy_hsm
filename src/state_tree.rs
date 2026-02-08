//!
//! 该模块提供了一个层次化的状态树结构，用于管理状态之间的父子关系和转换路径。
//!
//! # 核心概念
//!
//! - **StateTree**: 状态树的根结构，维护所有状态节点的关系
//! - **TreeStateId**: 树状态标识符，包含树实体和状态实体的组合
//! - **TraversalStrategy**: 状态遍历策略，定义子状态的访问顺序
//!
//! # 使用示例
//!
//! ```
//! use bevy::prelude::*;
//! use bevy_hsm::prelude::*;
//!
//! fn setup_state_tree(mut commands: Commands) {
//!     // 创建根状态
//!     let root_state = commands.spawn(HsmState::default()).id();
//!     
//!     // 创建状态树
//!     let mut state_tree = StateTree::new(root_state, TraversalStrategy::default());
//!     
//!     // 添加子状态
//!     let child_state = commands.spawn(HsmState::default()).id();
//!     state_tree.add(root_state, child_state, TraversalStrategy::default());
//!     
//!     // 查询子状态
//!     if let Some(children) = state_tree.get(root_state) {
//!         println!("Root state has {} children", children.len());
//!     }
//! }
//! ```

use std::fmt::Display;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::state_traversal::TraversalStrategy;

///# 状态树结构/StateTree
///
/// 管理状态之间的层次关系，支持父子状态的添加、删除和查询操作。
///
/// Manage the hierarchical relationships between states, supporting add, delete, and query operations for parent-child states.
#[derive(Component, Clone, PartialEq, Eq, Debug)]
pub struct StateTree {
    root: Entity,
    tree: HashMap<Entity, StateTreeNode>,
}

impl StateTree {
    /// 创建新的状态树
    ///
    /// # 参数
    /// * `root` - 根状态实体
    /// * `traversal` - 默认的遍历策略
    ///
    /// # 返回值
    /// 返回初始化的状态树实例
    ///
    /// # 示例
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn example(mut commands: Commands) {
    /// let root_entity = commands.spawn(HsmState::default()).id();
    /// let state_tree = StateTree::new(root_entity, TraversalStrategy::default());
    /// # }
    /// ```
    pub fn new(root: Entity, traversal: TraversalStrategy) -> Self {
        Self {
            root,
            tree: HashMap::from([(root, StateTreeNode::new(None, traversal))]),
        }
    }

    /// 向状态树中添加父子关系
    ///
    /// # 参数
    /// * `from` - 父状态实体
    /// * `to` - 子状态实体
    /// * `traversal` - 子状态的遍历策略
    ///
    /// # 返回值
    /// 成功添加返回 `true`，失败返回 `false`
    ///
    /// # 失败条件
    /// - 父状态不存在于树中
    /// - 形成循环引用（子状态已经是父状态的祖先）
    ///
    /// # 示例
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn example(mut commands: Commands, mut state_tree: StateTree) {
    /// let parent = commands.spawn(HsmState::default()).id();
    /// let child = commands.spawn(HsmState::default()).id();
    ///
    /// // 添加成功
    /// assert!(state_tree.add(parent, child, TraversalStrategy::default()));
    ///
    /// // 添加失败（parent不在树中）
    /// let orphan = commands.spawn(HsmState::default()).id();
    /// assert!(!state_tree.add(orphan, child, TraversalStrategy::default()));
    /// # }
    /// ```
    pub fn add(&mut self, from: Entity, to: Entity, traversal: TraversalStrategy) -> bool {
        if self.has_link(to, from) {
            return false;
        }

        if let Some(node) = self.tree.get_mut(&from) {
            node.push(to);
            self.tree
                .insert(to, StateTreeNode::new(Some(from), traversal));
            return true;
        }
        false
    }

    pub fn with_add(mut self, from: Entity, to: Entity, traversal: TraversalStrategy) -> Self {
        self.add(from, to, traversal);
        self
    }

    pub fn remove(&mut self, from: Entity, to: Entity) -> Option<Self> {
        if let Some(node) = self.tree.get_mut(&from) {
            for (i, e) in node.sub_states.iter().enumerate() {
                if *e == to {
                    node.sub_states.remove(i);
                    break;
                }
            }

            let mut new_tree = Self {
                root: to,
                tree: HashMap::default(),
            };
            let mut node = self.tree.remove(&to).unwrap();
            node.super_state = None;
            self.extract_subtree(&mut new_tree, to, node);

            return Some(new_tree);
        }
        None
    }

    /// 将指定节点及其所有子节点从源树移动到目标树
    fn extract_subtree(
        &mut self,
        new_tree: &mut StateTree,
        target: Entity,
        target_node: StateTreeNode,
    ) {
        for child in &target_node.sub_states {
            let sub_state = self.tree.remove(child).unwrap();
            self.extract_subtree(new_tree, *child, sub_state);
        }
        new_tree.tree.insert(target, target_node);
    }

    pub fn get(&self, state: Entity) -> Option<&[Entity]> {
        self.tree.get(&state).map(|v| v.get_sub_states())
    }

    pub fn get_root(&self) -> Entity {
        self.root
    }

    pub fn contains(&self, state: Entity) -> bool {
        self.tree.contains_key(&state)
    }

    pub fn has_link(&self, from: Entity, to: Entity) -> bool {
        if let Some(v) = self.get(from) {
            return v.contains(&to);
        };
        false
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// 从target开始，迭代其所有父节点
    pub fn path_iter(&self, target: Entity) -> impl Iterator<Item = Entity> {
        std::iter::successors(
            self.tree.get(&target).and_then(|node| node.super_state),
            |&parent| self.tree.get(&parent).and_then(|node| node.super_state),
        )
    }

    pub fn get_sub_states(&self, state: Entity) -> Option<&[Entity]> {
        self.tree.get(&state).map(|node| node.get_sub_states())
    }

    pub fn get_super_state(&self, state: Entity) -> Option<Entity> {
        self.tree.get(&state).and_then(|node| node.super_state)
    }

    pub fn traversal_iter(&self, world: &World, state: Entity) -> TraversalIter {
        match self.tree.get(&state) {
            Some(StateTreeNode {
                super_state: _,
                traversal,
                sub_states,
            }) => TraversalIter {
                data: traversal.0.traverse(world, sub_states.as_slice()),
                down: 0,
                up: sub_states.len(),
            },
            None => TraversalIter::default(),
        }
    }
}

#[derive(Default)]
pub struct TraversalIter {
    data: Vec<Entity>,
    down: usize,
    up: usize,
}

impl Iterator for TraversalIter {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        if self.down >= self.up {
            return None;
        }
        let e = self.data[self.down];
        self.down += 1;
        Some(e)
    }
}

impl DoubleEndedIterator for TraversalIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.down >= self.up {
            return None;
        }
        self.up -= 1;
        Some(self.data[self.up])
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct StateTreeNode {
    pub super_state: Option<Entity>,
    pub traversal: TraversalStrategy,
    pub sub_states: Vec<Entity>,
}

impl StateTreeNode {
    pub fn new(super_state: Option<Entity>, traversal: TraversalStrategy) -> Self {
        Self {
            super_state,
            traversal,
            sub_states: Vec::new(),
        }
    }

    pub const fn get_sub_states(&self) -> &[Entity] {
        self.sub_states.as_slice()
    }

    pub fn push(&mut self, state: Entity) {
        for (i, e) in self.sub_states.iter().enumerate() {
            if *e == state {
                self.sub_states.remove(i);
                break;
            }
        }
        self.sub_states.push(state);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TreeStateId {
    tree: Entity,
    state: Entity,
}

impl TreeStateId {
    pub fn new(tree: Entity, state: Entity) -> Self {
        Self { tree, state }
    }

    pub const fn tree(&self) -> Entity {
        self.tree
    }

    pub const fn state(&self) -> Entity {
        self.state
    }
}

impl Display for TreeStateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.tree, self.state)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_state_tree() {
        let v = (0..8)
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .collect::<Vec<_>>();
        let traversal = TraversalStrategy::default();
        let mut tree = StateTree::new(v[0], traversal.clone());

        assert!(tree.add(v[0], v[1], traversal.clone()));
        assert!(tree.add(v[0], v[1], traversal.clone()));
        assert!(tree.add(v[0], v[1], traversal.clone()));
        assert_eq!(tree.get(v[0]), Some([v[1]].as_slice()));

        assert!(!tree.add(v[2], v[1], traversal.clone()));

        assert!(!tree.add(v[1], v[0], traversal.clone()));

        assert!(tree.add(v[0], v[2], traversal.clone()));
        assert!(tree.add(v[1], v[3], traversal.clone()));
        assert!(tree.add(v[2], v[4], traversal.clone()));
        assert!(tree.add(v[3], v[6], traversal.clone()));
        assert!(tree.add(v[4], v[7], traversal.clone()));

        let new_tree = tree.remove(v[2], v[4]);
        assert_eq!(
            new_tree,
            Some(StateTree::new(v[4], traversal.clone()).with_add(v[4], v[7], traversal.clone()))
        );
    }

    #[test]
    fn test_state_tree_iter() {
        let v = (0..8)
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .collect::<Vec<_>>();
        let traversal = TraversalStrategy::default();
        let mut tree = StateTree::new(v[0], traversal.clone());

        for i in 1..8 {
            tree.add(v[0], v[i], traversal.clone());
        }

        let world = World::new();
        let mut iter = tree.traversal_iter(&world, v[0]);
        assert_eq!(iter.next(), Some(v[1]));
        assert_eq!(iter.next_back(), Some(v[7]));
        assert_eq!(iter.next(), Some(v[2]));
        assert_eq!(iter.next_back(), Some(v[6]));
        assert_eq!(iter.next(), Some(v[3]));
        assert_eq!(iter.next_back(), Some(v[5]));
        assert_eq!(iter.next(), Some(v[4]));
        assert_eq!(iter.next_back(), None);

        tree.add(v[0], v[3], traversal.clone());

        assert_eq!(
            tree.get_sub_states(v[0]),
            Some([v[1], v[2], v[4], v[5], v[6], v[7], v[3]].as_slice())
        );
    }

    #[test]
    fn test_has_link() {
        let v = (0..3)
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .collect::<Vec<_>>();
        let traversal = TraversalStrategy::default();
        let mut tree = StateTree::new(v[0], traversal.clone());

        assert!(tree.add(v[0], v[1], traversal.clone()));
        assert!(tree.add(v[1], v[2], traversal.clone()));

        assert!(!tree.has_link(v[1], v[0]));
        assert!(!tree.has_link(v[2], v[1]));
        assert!(tree.has_link(v[1], v[2]));
    }

    #[test]
    fn test_path_iter() {
        let v = (0..3)
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .collect::<Vec<_>>();
        let traversal = TraversalStrategy::default();
        let mut tree = StateTree::new(v[0], traversal.clone());

        assert!(tree.add(v[0], v[1], traversal.clone()));
        assert!(tree.add(v[1], v[2], traversal.clone()));

        assert_eq!(tree.path_iter(v[2]).collect::<Vec<_>>(), vec![v[1], v[0]]);
    }
}
