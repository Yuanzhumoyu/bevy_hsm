//!
//! 该模块提供了一个层次化的状态树结构，用于管理状态之间的父子关系和转换路径。
//!
//! # 核心概念
//!
//! - **StateTree**: 状态树的根结构，维护所有状态节点的关系
//! - **HsmStateId**: 树状态标识符，包含树实体和状态实体的组合
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
//!     let mut state_tree = StateTree::new(root_state);
//!     
//!     // 添加子状态
//!     let child_state = commands.spawn(HsmState::default()).id();
//!     state_tree.with_add(root_state, child_state)
//!               .with_traversal(root_state, TraversalStrategy::default());
//!     
//!     // 查询子状态
//!     if let Some(children) = state_tree.get(root_state) {
//!         println!("Root state has {} children", children.len());
//!     }
//! }
//! ```

use std::fmt::Display;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::hsm::transition_strategy::TraversalStrategy;

///# 状态树结构/StateTree
///
/// 管理状态之间的层次关系，支持父子状态的添加、删除和查询操作。
///
/// Manage the hierarchical relationships between states, supporting add, delete, and query operations for parent-child states.
#[derive(Component, Clone, PartialEq, Eq, Debug)]
pub struct StateTree {
    /// 根状态实体/Root state entity
    root: Entity,
    /// 状态树节点映射/State tree node map
    tree: HashMap<Entity, StateTreeNode>,
}

impl StateTree {
    /// 创建新的状态树
    /// # 示例
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn example(mut commands: Commands) {
    /// let root_entity = commands.spawn(HsmState::default()).id();
    /// let state_tree = StateTree::new(root_entity);
    /// # }
    /// ```
    pub fn new(root: Entity) -> Self {
        Self {
            root,
            tree: HashMap::from([(root, StateTreeNode::new(None))]),
        }
    }

    /// 为状态搜索下一个状态添加一个遍历行为
    ///
    /// Add a traversal behavior to search for the next state
    ///
    ///# Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn example(mut commands: Commands, mut state_tree: StateTree) {
    /// let parent = commands.spawn(HsmState::default()).id();
    /// let child = commands.spawn(HsmState::default()).id();
    ///
    /// let traversal = TraversalStrategy::default();
    /// let mut tree = StateTree::new(parent);
    /// tree
    ///     .with_traversal(parent, traversal)
    ///     .with_add(parent, child);
    /// # }
    pub fn with_traversal(&mut self, target: Entity, traversal: TraversalStrategy) -> &mut Self {
        if let Some(node) = self.tree.get_mut(&target) {
            node.set_traversal(traversal);
        }
        self
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
    /// # fn example(mut commands: Commands) {
    /// let parent = commands.spawn(HsmState::default()).id();
    /// let child = commands.spawn(HsmState::default()).id();
    ///
    /// let mut state_tree = StateTree::new(parent);
    /// // 添加成功
    /// assert!(state_tree.add(parent, child));
    ///
    /// // 添加失败（parent不在树中）
    /// let orphan = commands.spawn(HsmState::default()).id();
    /// assert!(!state_tree.add(orphan, child));
    /// # }
    /// ```
    pub fn add(&mut self, from: Entity, to: Entity) -> bool {
        if self.has_link(to, from) {
            return false;
        }

        if let Some(node) = self.tree.get_mut(&from) {
            node.push(to);
            self.tree.insert(to, StateTreeNode::new(Some(from)));
            return true;
        }
        false
    }

    pub fn with_add(&mut self, from: Entity, to: Entity) -> &mut Self {
        self.add(from, to);
        self
    }

    pub fn with_adds(&mut self, from: Entity, to: &[Entity]) -> &mut Self {
        let to = to
            .iter()
            .filter(|to| !self.has_link(from, **to))
            .copied()
            .collect::<Vec<_>>();

        if let Some(node) = self.tree.get_mut(&from) {
            node.sub_states.extend(to.iter());
            to.iter().for_each(|to| {
                self.tree.insert(*to, StateTreeNode::new(Some(from)));
            });
        }
        self
    }

    pub fn remove(&mut self, from: Entity, to: Entity) -> Option<Self> {
        if let Some(node) = self.tree.get_mut(&from) {
            node.sub_states.retain(|&s| s != to);

            let mut node = self.tree.remove(&to)?;
            let mut new_tree = Self {
                root: to,
                tree: HashMap::default(),
            };
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
            if let Some(sub_state) = self.tree.remove(child) {
                self.extract_subtree(new_tree, *child, sub_state);
            }
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

    /// 用于迭代所有的状态实体
    pub fn iter(&self) -> StateTreeIterator<'_> {
        StateTreeIterator(self.tree.keys())
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

    pub fn traversal_iter(&self, world: &World, state: Entity) -> Vec<Entity> {
        match self.tree.get(&state) {
            Some(StateTreeNode {
                super_state: _,
                traversal,
                sub_states,
            }) => match traversal {
                Some(traversal) => traversal.0.traverse(world, sub_states.as_slice()),
                None => sub_states.to_vec(),
            },
            None => Vec::new(),
        }
    }

    pub fn traversal_iter_with(
        &self,
        world: &World,
        state: Entity,
        f: impl Fn(&EntityRef) -> bool,
    ) -> Vec<Entity> {
        match self.tree.get(&state) {
            Some(StateTreeNode {
                super_state: _,
                traversal,
                sub_states,
            }) => {
                let sub_states = world
                    .entity(sub_states.as_slice())
                    .into_iter()
                    .filter(|e| f(e))
                    .map(|e| e.id())
                    .collect::<Vec<_>>();

                match traversal {
                    Some(traversal) => traversal.0.traverse(world, sub_states.as_slice()),
                    None => sub_states,
                }
            }
            None => Vec::new(),
        }
    }

    /// 计算两个状态之间的最近共同祖先（LCA）以及转换所需的退出和进入路径。
    ///
    /// # Arguments
    /// * `from` - 起始状态实体。
    /// * `to` - 目标状态实体。
    ///
    /// # Returns
    /// 返回一个可选元组 `Option<(lca, exit_path, enter_path)>`：
    /// * `exit_path`: 从 `from`（含）到 `lca`（不含）需要退出的状态路径。
    /// * `enter_path`: 从 `lca`（不含）到 `to`（含）需要进入的状态路径。
    /// * 两个路径最后一个状态为最近共同祖先
    pub fn find_lca_and_paths(
        &self,
        from: Entity,
        to: Entity,
    ) -> Option<(Vec<Entity>, Vec<Entity>)> {
        if !self.contains(from) || !self.contains(to) {
            return None;
        }

        if from == to {
            return Some((vec![from], vec![to]));
        }

        // 收集从 `from` 到根的路径
        let mut from_path: Vec<Entity> =
            std::iter::once(from).chain(self.path_iter(from)).collect();

        // 收集从 `to` 到根的路径
        let mut to_path: Vec<Entity> = std::iter::once(to).chain(self.path_iter(to)).collect();

        let lca_index = {
            let mut lac = -1;
            for (a, b) in from_path.iter().rev().zip(to_path.iter().rev()) {
                if a != b {
                    break;
                }
                lac += 1;
            }

            lac
        };

        (lca_index != -1).then(|| {
            let index = lca_index as usize;
            from_path.truncate(from_path.len() - index);
            to_path.truncate(to_path.len() - index);
            (from_path, to_path)
        })
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct StateTreeNode {
    pub super_state: Option<Entity>,
    pub traversal: Option<TraversalStrategy>,
    pub sub_states: Vec<Entity>,
}

impl StateTreeNode {
    pub fn new(super_state: Option<Entity>) -> Self {
        Self {
            super_state,
            traversal: None,
            sub_states: Vec::new(),
        }
    }

    pub fn set_traversal(&mut self, traversal: TraversalStrategy) {
        self.traversal = Some(traversal);
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
pub struct HsmStateId {
    tree: Entity,
    state: Entity,
}

impl HsmStateId {
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

impl Display for HsmStateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.tree, self.state)
    }
}

pub struct StateTreeIterator<'a>(
    bevy::platform::collections::hash_map::Keys<'a, Entity, StateTreeNode>,
);

impl Iterator for StateTreeIterator<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().copied()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_state_tree() {
        let v = (0..8u32)
            .filter_map(Entity::from_raw_u32)
            .collect::<Vec<_>>();
        let traversal = TraversalStrategy::default();
        let mut tree = StateTree::new(v[0]);

        tree.with_traversal(v[0], traversal);

        assert!(tree.add(v[0], v[1]));
        assert!(tree.add(v[0], v[1]));
        assert!(tree.add(v[0], v[1]));
        assert_eq!(tree.get(v[0]), Some([v[1]].as_slice()));

        assert!(!tree.add(v[2], v[1]));

        assert!(!tree.add(v[1], v[0]));

        assert!(tree.add(v[0], v[2]));
        assert!(tree.add(v[1], v[3]));
        assert!(tree.add(v[2], v[4]));
        assert!(tree.add(v[3], v[6]));
        assert!(tree.add(v[4], v[7]));

        let new_tree = tree.remove(v[2], v[4]);
        assert_eq!(
            new_tree,
            Some(StateTree::new(v[4]).with_add(v[4], v[7]).clone())
        );
    }

    #[test]
    fn test_has_link() {
        let v = (0..3u32)
            .filter_map(Entity::from_raw_u32)
            .collect::<Vec<_>>();
        let mut tree = StateTree::new(v[0]);

        assert!(tree.add(v[0], v[1]));
        assert!(tree.add(v[1], v[2]));

        assert!(!tree.has_link(v[1], v[0]));
        assert!(!tree.has_link(v[2], v[1]));
        assert!(tree.has_link(v[1], v[2]));
    }

    #[test]
    fn test_path_iter() {
        let v = (0..3u32)
            .filter_map(Entity::from_raw_u32)
            .collect::<Vec<_>>();
        let mut tree = StateTree::new(v[0]);

        assert!(tree.add(v[0], v[1]));
        assert!(tree.add(v[1], v[2]));

        assert_eq!(tree.path_iter(v[2]).collect::<Vec<_>>(), vec![v[1], v[0]]);
    }

    #[test]
    fn test_state_tree_from_dsl() {
        let entities = (0..5u32)
            .filter_map(Entity::from_raw_u32)
            .collect::<Vec<_>>();
        let traversal = TraversalStrategy::default();
        let dsl: Vec<(TraversalStrategy, Vec<Entity>)> = vec![
            (
                traversal.clone(),
                vec![entities[0], entities[1], entities[2]],
            ),
            (traversal.clone(), vec![]), // Empty path to test the fix
            (traversal.clone(), vec![entities[3], entities[4]]),
        ];
        for (traversal, v) in dsl {
            if v.is_empty() {
                continue;
            }
            let mut tree = StateTree::new(v[0]);
            tree.with_traversal(v[0], traversal);
            for window in v.windows(2) {
                assert!(tree.add(window[0], window[1]));
            }
            if v.len() > 1 {
                let last = v.last().expect("Path should not be empty");
                let expected_path: Vec<Entity> = v.iter().rev().skip(1).cloned().collect();
                let actual_path = tree.path_iter(*last).collect::<Vec<_>>();
                assert_eq!(actual_path, expected_path);
            }
        }
    }

    #[test]
    fn test_lca() {
        let enititys = (0..5u32)
            .filter_map(Entity::from_raw_u32)
            .collect::<Vec<_>>();
        let mut tree = StateTree::new(enititys[0]);
        tree.add(enititys[0], enititys[1]);
        tree.add(enititys[0], enititys[2]);
        tree.add(enititys[1], enititys[3]);
        tree.add(enititys[2], enititys[4]);

        let (exit_path, enter_path) = tree.find_lca_and_paths(enititys[3], enititys[4]).unwrap();

        assert_eq!(exit_path, vec![enititys[3], enititys[1], enititys[0]]);
        assert_eq!(enter_path, vec![enititys[4], enititys[2], enititys[0]]);

        assert_eq!(
            tree.find_lca_and_paths(enititys[1], enititys[2]),
            Some((
                vec![enititys[1], enititys[0]],
                vec![enititys[2], enititys[0]]
            ))
        );

        assert_eq!(
            tree.find_lca_and_paths(enititys[1], enititys[3]),
            Some((vec![enititys[1]], vec![enititys[3], enititys[1]]))
        );
    }
}
