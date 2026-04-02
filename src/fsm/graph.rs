use std::fmt::Debug;

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

use bimap::BiMap;

use crate::prelude::{GuardCondition, StateEvent};

enum FsmTransitionType {
    Unconditional,
    OnEvent,
    OnGuard,
}

/// # 传出转换
/// * 定义从单个状态出发的所有可能的转换规则。
///
/// 这个结构体被用在 [`FsmGraph`] 中，作为 `transitions` 哈希图的值。
/// 它包含了三种类型的转换：无条件转换、事件触发的转换和守卫条件触发的转换。
///
/// # Outgoing Transitions
/// * Defines all possible transition rules originating from a single state.
///
/// This struct is used as the value in the [`FsmGraph`]'s `transitions` HashMap.
/// It contains three types of transitions: unconditional, event-triggered, and guard-conditioned.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OutgoingTransitions {
    /// 无条件转换的目标状态集合。
    /// A set of target states for unconditional transitions.
    unconditional: HashSet<Entity>,
    /// 从事件到目标状态的双向映射。
    /// A bidirectional map from an event to a target state.
    on_event: BiMap<Box<dyn StateEvent>, Entity>,
    /// 从守卫条件到目标状态的双向映射。
    /// A bidirectional map from a guard condition to a target state.
    on_guard: BiMap<GuardCondition, Entity>,
}

impl OutgoingTransitions {
    fn clear_outgoing_transitions(&mut self, target: Entity, except: FsmTransitionType) {
        match except {
            FsmTransitionType::Unconditional => {
                self.on_event.remove_by_right(&target);
                self.on_guard.remove_by_right(&target);
            }
            FsmTransitionType::OnEvent => {
                self.unconditional.remove(&target);
                self.on_guard.remove_by_right(&target);
            }
            FsmTransitionType::OnGuard => {
                self.unconditional.remove(&target);
                self.on_event.remove_by_right(&target);
            }
        }
    }

    pub fn with(&mut self, target: Entity) -> &mut Self {
        self.clear_outgoing_transitions(target, FsmTransitionType::Unconditional);

        self.unconditional.insert(target);

        self
    }

    pub fn with_condition(
        &mut self,
        condition: impl Into<GuardCondition>,
        target: Entity,
    ) -> &mut Self {
        self.clear_outgoing_transitions(target, FsmTransitionType::OnGuard);

        self.on_guard.insert(condition.into(), target);

        self
    }

    pub fn with_event(&mut self, event: impl StateEvent, target: Entity) -> &mut Self {
        self.clear_outgoing_transitions(target, FsmTransitionType::OnEvent);

        self.on_event.insert(Box::new(event), target);
        self
    }

    pub fn contains(&self, target: Entity) -> bool {
        self.unconditional.contains(&target)
            || self.on_event.contains_right(&target)
            || self.on_guard.contains_right(&target)
    }

    pub fn get_by_event(&self, event: &dyn StateEvent) -> Option<Entity> {
        self.on_event.get_by_left(event).copied()
    }

    pub fn get_by_guard(&self, target: Entity) -> Option<&GuardCondition> {
        self.on_guard.get_by_right(&target)
    }

    pub fn remove(&mut self, target: Entity) -> bool {
        self.unconditional.remove(&target)
            || self.on_event.remove_by_right(&target).is_some()
            || self.on_guard.remove_by_right(&target).is_some()
    }

    pub fn retain(&mut self, f: impl Fn(&Entity) -> bool) {
        let f = &f;
        self.unconditional.retain(f);
        self.on_event.retain(|_, v| f(v));
        self.on_guard.retain(|_, v| f(v));
    }

    pub fn is_empty(&self) -> bool {
        self.unconditional.is_empty() && self.on_event.is_empty() && self.on_guard.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = Entity> {
        self.unconditional
            .iter()
            .copied()
            .chain(self.on_event.right_values().copied())
            .chain(self.on_guard.right_values().copied())
    }
}

/// # FSM 图
/// * 表示一个有限状态机（FSM）的拓扑结构。
///
/// 该组件作为一个蓝图，定义了所有可能的状态以及它们之间的转换关系。
/// 它通常被附加到一个单独的“图”实体上，并被一个或多个 [`FsmStateMachine`] 实例所引用。
///
/// # FSM Graph
/// * Represents the topological structure of a Finite State Machine (FSM).
///
/// This component acts as a blueprint, defining all possible states and the transitions
/// between them. It is typically attached to a separate "graph" entity and is referenced
/// by one or more [`FsmStateMachine`] instances.
///
/// # Example
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::{FsmGraph, FsmStateMachine, FsmState};
/// #
/// # #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// # struct MyEvent;
/// #
/// # fn setup(mut commands: Commands) {
/// // 1. Define states as entities
/// let idle = commands.spawn(FsmState::default()).id();
/// let walking = commands.spawn(FsmState::default()).id();
/// let running = commands.spawn(FsmState::default()).id();
///
/// // 2. Create the graph, starting in the `idle` state
/// let mut graph = FsmGraph::new(idle);
/// graph
///     // idle -> walking (unconditional)
///     .with_add(idle, walking)
///     // walking -> running (on event)
///     .with_event(walking, MyEvent, running)
///     // running -> idle (with a guard condition)
///     .with_condition(running, "is_tired", idle);
///
/// // 3. Spawn an entity with the graph component
/// let graph_id = commands.spawn(graph).id();
///
/// // 4. Spawn a state machine instance that uses this graph
/// commands.spawn(FsmStateMachine::with(graph_id, idle, 10));
/// # }
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct FsmGraph {
    /// FSM 的入口点或初始状态。
    /// The entry point or initial state of the FSM.
    init_state: Entity,
    /// 一个从源状态 (`Entity`) 到其 `OutgoingTransitions` 的映射，描述了所有可能的转换。
    /// A map from a source state (`Entity`) to its `OutgoingTransitions`, describing all possible transitions.
    transitions: HashMap<Entity, OutgoingTransitions>,
}

impl FsmGraph {
    pub fn new(init_state: Entity) -> Self {
        FsmGraph {
            init_state,
            transitions: HashMap::from([(init_state, OutgoingTransitions::default())]),
        }
    }

    pub fn init_state(&self) -> Entity {
        self.init_state
    }

    pub fn set_init_state(&mut self, state: Entity) {
        self.init_state = state;
    }

    pub fn remove_state(&mut self, state: Entity) -> Option<Vec<FsmGraph>> {
        if self.init_state == state {
            return None;
        }

        let mut transitions = std::mem::take(&mut self.transitions);
        let potential_roots: HashSet<Entity> = match transitions.remove(&state) {
            Some(outgoing) => outgoing.iter().collect(),
            None => {
                self.transitions = transitions;
                return None;
            }
        };

        let mut all_nodes: HashSet<Entity> = HashSet::new();
        let mut predecessors: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for (&source, outgoing) in transitions.iter_mut() {
            outgoing.remove(state);
            all_nodes.insert(source);
            all_nodes.extend(outgoing.iter());
            for successor in outgoing.iter() {
                predecessors.entry(successor).or_default().push(source);
            }
        }

        let mut all_components: Vec<HashSet<Entity>> = Vec::new();
        let mut visited_nodes = HashSet::new();

        for node in all_nodes {
            if visited_nodes.contains(&node) {
                continue;
            }

            let mut component_nodes = HashSet::new();
            let mut queue = std::collections::VecDeque::new();

            queue.push_back(node);
            visited_nodes.insert(node);
            component_nodes.insert(node);

            while let Some(current_node) = queue.pop_front() {
                if let Some(outgoing) = transitions.get(&current_node) {
                    for successor in outgoing.iter() {
                        if visited_nodes.insert(successor) {
                            component_nodes.insert(successor);
                            queue.push_back(successor);
                        }
                    }
                }

                if let Some(preds) = predecessors.get(&current_node) {
                    for &pred in preds {
                        if visited_nodes.insert(pred) {
                            component_nodes.insert(pred);
                            queue.push_back(pred);
                        }
                    }
                }
            }
            all_components.push(component_nodes);
        }

        if all_components.len() <= 1 {
            self.transitions = transitions;
            return Some(Vec::new());
        }

        let mut all_graphs: Vec<FsmGraph> = Vec::new();
        for component in all_components {
            let mut subgraph_transitions = HashMap::new();
            for &node in &component {
                if let Some(mut outgoing) = transitions.remove(&node) {
                    outgoing.retain(|target| component.contains(target));
                    if !outgoing.is_empty() {
                        subgraph_transitions.insert(node, outgoing);
                    }
                }
            }

            let init_state = potential_roots
                .iter()
                .find(|&&root| component.contains(&root))
                .copied()
                .unwrap_or_else(|| *component.iter().next().unwrap());

            all_graphs.push(FsmGraph {
                init_state,
                transitions: subgraph_transitions,
            });
        }

        let main_graph_index = all_graphs
            .iter()
            .position(|g| g.transitions.contains_key(&self.init_state))
            .expect("Main graph component with init_state should always be found");

        let mut main_graph = all_graphs.remove(main_graph_index);
        main_graph.set_init_state(self.init_state);

        *self = main_graph;

        Some(all_graphs)
    }

    pub fn remove(&mut self, from: Entity, to: Entity) -> Option<FsmGraph> {
        if !(self.transitions.get_mut(&from)?.remove(to)) {
            return None;
        }

        if from == to || self.is_bridge(from, to) {
            return None;
        }

        let mut component_nodes = HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        queue.push_back(to);
        component_nodes.insert(to);

        while let Some(current) = queue.pop_front() {
            if let Some(outgoing) = self.transitions.get(&current) {
                let successors = outgoing.iter();
                for successor in successors {
                    if component_nodes.insert(successor) {
                        queue.push_back(successor);
                    }
                }
            }

            for (&source, outgoing) in &self.transitions {
                if outgoing.contains(current) && component_nodes.insert(source) {
                    queue.push_back(source);
                }
            }
        }

        let original_transitions = std::mem::take(&mut self.transitions);
        let (subgraph_transitions, mut remaining_transitions): (
            HashMap<Entity, OutgoingTransitions>,
            HashMap<Entity, OutgoingTransitions>,
        ) = original_transitions
            .into_iter()
            .partition(|(k, _)| component_nodes.contains(k));

        let mut edge_emptys = HashSet::new();
        for (node, outgoing) in remaining_transitions.iter_mut() {
            outgoing.retain(|e| !component_nodes.contains(e));
            if outgoing.is_empty() {
                edge_emptys.insert(*node);
            }
        }

        remaining_transitions.retain(|e, _| !edge_emptys.contains(e));
        self.transitions = remaining_transitions;

        let mut subgraph = FsmGraph {
            init_state: to,
            transitions: subgraph_transitions,
        };

        if component_nodes.contains(&self.init_state) {
            std::mem::swap(self, &mut subgraph);
        }

        Some(subgraph)
    }

    pub fn get(&self, state: Entity) -> Option<&OutgoingTransitions> {
        self.transitions.get(&state)
    }

    pub fn get_mut(&mut self, state: Entity) -> Option<&mut OutgoingTransitions> {
        self.transitions.get_mut(&state)
    }

    pub fn get_mut_or_default(&mut self, state: Entity) -> &mut OutgoingTransitions {
        self.transitions.entry(state).or_default()
    }

    pub fn with_add(&mut self, from: Entity, to: Entity) -> &mut Self {
        self.get_mut_or_default(from).with(to);
        self
    }

    pub fn with_condition(
        &mut self,
        from: Entity,
        condition: impl Into<GuardCondition>,
        to: Entity,
    ) -> &mut Self {
        self.get_mut_or_default(from).with_condition(condition, to);
        self
    }

    pub fn with_event(&mut self, from: Entity, event: impl StateEvent, to: Entity) -> &mut Self {
        self.get_mut_or_default(from).with_event(event, to);
        self
    }

    pub fn is_bridge(&self, from: Entity, to: Entity) -> bool {
        if from == to {
            return true;
        }

        let mut queue = std::collections::VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back(from);
        visited.insert(from);

        while let Some(current) = queue.pop_front() {
            if let Some(transitions) = self.transitions.get(&current) {
                let successors = transitions.iter();

                for successor in successors {
                    if successor == to {
                        return true;
                    }

                    if !visited.contains(&successor) {
                        visited.insert(successor);
                        queue.push_back(successor);
                    }
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let mut transitions = OutgoingTransitions::default();
        let state1 = Entity::from_raw_u32(0).unwrap();
        let state2 = Entity::from_raw_u32(1).unwrap();
        let state3 = Entity::from_raw_u32(2).unwrap();
        let state4 = Entity::from_raw_u32(3).unwrap();

        #[derive(Clone, Eq, PartialEq, Hash, Debug)]
        struct MyEvent(u32);
        transitions
            .with_event(MyEvent(1), state1)
            .with_event(1, state3)
            .with_event("event", state4);

        let condition = GuardCondition::parse("test").unwrap();
        transitions.with_condition(condition.clone(), state2);

        assert_eq!(transitions.get_by_event(&MyEvent(1)), Some(state1));
        assert_eq!(transitions.get_by_event(&MyEvent(2)), None);
        assert_eq!(transitions.get_by_event(&1), Some(state3));
        assert_eq!(transitions.get_by_event(&"event"), Some(state4));
        assert_eq!(transitions.get_by_guard(state2), Some(&condition));
        assert_eq!(transitions.get_by_guard(state1), None);
        assert!(transitions.contains(state1));
        assert!(transitions.contains(state2));
    }

    #[test]
    fn test_graph_remove_edge() {
        let ids = [0, 1, 2, 3, 4, 5, 6, 7]
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .to_vec();

        let mut graph = FsmGraph::new(ids[0]);
        graph.with_add(ids[0], ids[1]);
        graph.with_add(ids[0], ids[2]);
        graph.with_add(ids[1], ids[3]);
        graph.with_add(ids[2], ids[3]);
        graph.with_add(ids[3], ids[4]);
        graph.with_add(ids[4], ids[5]);
        graph.with_add(ids[5], ids[6]);
        graph.with_add(ids[5], ids[7]);

        let subgraph = graph.remove(ids[3], ids[4]);

        let mut sub_graph = FsmGraph::new(ids[4]);
        sub_graph.with_add(ids[4], ids[5]);
        sub_graph.with_add(ids[5], ids[6]);
        sub_graph.with_add(ids[5], ids[7]);

        assert_eq!(subgraph, Some(sub_graph));

        let mut new_graph = FsmGraph::new(ids[0]);
        new_graph.with_add(ids[0], ids[1]);
        new_graph.with_add(ids[0], ids[2]);
        new_graph.with_add(ids[1], ids[3]);
        new_graph.with_add(ids[2], ids[3]);

        assert_eq!(graph, new_graph);
    }

    #[test]
    fn test_graph_remove_state() {
        let ids = [0, 1, 2, 3, 4, 5, 6, 7]
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .to_vec();

        let mut graph = FsmGraph::new(ids[0]);
        graph.with_add(ids[0], ids[1]);
        graph.with_add(ids[0], ids[2]);
        graph.with_add(ids[0], ids[3]);
        graph.with_add(ids[1], ids[4]);
        graph.with_add(ids[2], ids[5]);
        graph.with_add(ids[3], ids[6]);
        graph.with_add(ids[3], ids[7]);

        assert_eq!(graph.remove_state(ids[0]), None);

        graph.set_init_state(ids[1]);

        let mut new_subgraph = graph.remove_state(ids[0]).unwrap();
        new_subgraph.sort_by_key(|a| a.init_state);

        let mut graph1 = FsmGraph::new(ids[1]);
        graph1.with_add(ids[1], ids[4]);
        assert_eq!(graph1, graph);

        let mut graph2 = FsmGraph::new(ids[2]);
        graph2.with_add(ids[2], ids[5]);
        assert_eq!(new_subgraph[1], graph2);

        let mut graph3 = FsmGraph::new(ids[3]);
        graph3.with_add(ids[3], ids[6]);
        graph3.with_add(ids[3], ids[7]);
        assert_eq!(new_subgraph[0], graph3);
    }
}
