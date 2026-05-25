use std::fmt::Debug;

use bevy::prelude::*;

use crate::{
    context::GuardContext,
    error::{StateMachineError, error_event, error_event_world, warn_event, warn_event_world},
    guards::{GuardCondition, GuardRegistry},
    hsm::{
        HsmState,
        event::HsmTrigger,
        state_lifecycle::StateLifecycle,
        transition::{Transition, TransitionQueue},
        transition_strategy::{
            ExitTransitionBehavior, build_enter_transition_plan, build_exit_transition_plan,
            handle_enter_transition, handle_exit_transition,
        },
    },
    interrupt::{InterruptFrame, InterruptStack},
    markers::Paused,
    prelude::StateTree,
};

#[cfg(feature = "history")]
use crate::hsm::history::*;

/// 分层状态机\Hierarchical state machines
/// # 作用\Effect
/// * 管理实体的状态转换，包括当前状态、下一状态
/// - Manages entity state transitions, including current state, next state
/// # 示例\Example
///
/// ```rust
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
///
/// # fn  foo(mut commands: Commands) {
/// let id = commands.spawn_empty().id();
/// let tree_id = commands.spawn(StateTree::new(id)).id();
/// let state_machine = HsmStateMachine::with(tree_id, id,#[cfg(feature = "history")] 10);
/// # }
/// ```
#[derive(Component, Clone, PartialEq, Eq)]
pub struct HsmStateMachine {
    /// 历史记录
    ///
    /// History
    ///
    /// 记录实体的状态转换历史，用于回溯状态
    /// 最后一个状态始终为最新的状态
    ///
    /// Records entity's state transition history, used for state backtracking
    /// The last state is always the most recent state
    #[cfg(feature = "history")]
    pub history: StateHistory,
    /// 下一个状态
    ///
    /// Next state
    ///
    /// 实体下一个要转换到的状态
    ///
    /// Next state to transition to for the entity
    pub(crate) transition_queue: TransitionQueue,
    state_tree: Entity,
    curr_state: Entity,
    /// 初始状态
    ///
    /// Initial state
    init_state: Entity,
    /// 中断状态栈，保存被中断的状态图和状态位置
    /// 支持嵌套中断，最后被中断的状态最先恢复
    ///
    /// Interrupt state stack, stores interrupted state graph and position.
    /// Supports nested interrupts; most recently interrupted state resumes first.
    pub(crate) interrupt_stack: InterruptStack,
}

impl HsmStateMachine {
    /// 创建一个新的状态机
    ///
    /// Create a new state machine
    pub fn new(
        state_tree: Entity,
        init_state: Entity,
        curr_state: Entity,
        #[cfg(feature = "history")] history_len: usize,
    ) -> Self {
        Self {
            state_tree,
            init_state,
            curr_state,
            transition_queue: TransitionQueue::default(),
            #[cfg(feature = "history")]
            history: StateHistory::new(history_len),
            interrupt_stack: InterruptStack::default(),
        }
    }

    /// 使用初始状态创建一个新的状态机，当前状态也为初始状态
    ///
    /// Create a new state machine with an initial state, the current state is also the initial state
    pub fn with(
        state_tree: Entity,
        init_state: Entity,
        #[cfg(feature = "history")] history_len: usize,
    ) -> Self {
        Self::new(
            state_tree,
            init_state,
            init_state,
            #[cfg(feature = "history")]
            history_len,
        )
    }

    /// 获取状态树
    /// Get the state tree
    pub const fn state_tree(&self) -> Entity {
        self.state_tree
    }

    /// 获取初始状态
    ///
    /// Get the initial state
    pub const fn init_state(&self) -> Entity {
        self.init_state
    }

    /// 获取当前状态的ID
    ///
    /// Get the ID of the current state
    pub const fn curr_state_id(&self) -> Entity {
        self.curr_state
    }

    /// 获取下一个状态转换
    ///
    /// Get the next state transition
    pub fn next_transition(&self) -> Transition {
        self.transition_queue.next()
    }

    /// 设置初始状态
    ///
    /// Set the initial state
    pub fn set_init_state(&mut self, state: Entity) {
        self.init_state = state;
    }

    /// 设置当前状态
    ///
    /// Set the current state
    pub fn set_curr_state(&mut self, state: Entity) {
        self.curr_state = state;
    }

    /// 添加历史记录
    ///
    /// Add history record
    #[cfg(feature = "history")]
    pub(crate) fn push_history(&mut self, node: HistoricalNode) {
        self.history.push(node);
    }

    /// 添加下一个状态
    ///
    /// Add next state
    pub fn push_next_state(&mut self, next_state: Transition) {
        self.transition_queue.push(next_state);
    }

    /// 批量添加下一个状态
    ///
    /// Add multiple next states
    pub fn push_next_states<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = Transition>,
    {
        self.transition_queue.extend(iter);
    }

    /// 替换前一个状态转换并返回旧值
    ///
    /// Replaces the previous state transition and returns the old value
    pub fn replace_prev_state(&mut self, prev_state: Transition) -> Transition {
        self.transition_queue.replace_prev(prev_state)
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn next_state_id(&self) -> Option<Entity> {
        self.transition_queue.next().get_state_id()
    }

    /// 获取下一个状态的OnState
    ///
    /// Get the OnState of the next state
    pub fn next_state_lifecycle(&self) -> Option<StateLifecycle> {
        self.transition_queue.next().get_lifecyle()
    }

    /// 弹出下一个状态
    ///
    /// Pop next state
    pub fn pop_next_state(&mut self) -> Transition {
        self.transition_queue.pop()
    }

    /// 获取状态历史记录
    ///
    /// Get state history
    #[cfg(feature = "history")]
    pub fn history_iter(&self) -> StateHistoryIterator<'_> {
        self.history.iter()
    }

    /// 获取历史记录长度
    ///
    /// Obtain the length of historical records
    #[cfg(feature = "history")]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// 获取状态转换队列长度
    ///
    /// Obtain the length of the state transition queue
    pub fn transition_queue_len(&self) -> usize {
        self.transition_queue.len()
    }

    /// 状态转换队列是否为空
    ///
    /// Is the state transition queue empty?
    pub fn transition_queue_is_empty(&self) -> bool {
        self.transition_queue.is_empty()
    }

    /// 检查是否处于指定状态
    ///
    /// Check if in specified state
    pub fn is_in_state(&self, state: Entity) -> bool {
        self.curr_state_id() == state
    }

    /// 清空下一个状态队列
    ///
    /// Clear the next state queue
    pub fn clear_next_states(&mut self) {
        self.transition_queue.clear();
    }

    /// 检查是否处于中断状态
    ///
    /// Check if currently in an interrupted state
    #[inline]
    pub fn is_interrupted(&self) -> bool {
        self.interrupt_stack.is_interrupted()
    }

    /// 获取当前中断嵌套深度
    ///
    /// Get the current interrupt nesting depth
    #[inline]
    pub fn interrupt_depth(&self) -> usize {
        self.interrupt_stack.interrupt_depth()
    }

    /// 将被中断的状态图和状态压入中断栈
    ///
    /// Push an interrupted state graph and state onto the interrupt stack
    #[inline]
    pub fn push_interrupt(&mut self, graph_id: Entity, saved_state: Entity) {
        self.interrupt_stack.push_interrupt(graph_id, saved_state);
    }

    /// 从中断栈弹出最近被中断的状态帧
    ///
    /// Pop the most recently interrupted frame from the interrupt stack
    #[inline]
    pub fn pop_interrupt(&mut self) -> Option<InterruptFrame> {
        self.interrupt_stack.pop_interrupt()
    }

    /// 清空中断栈，放弃所有未恢复的中断
    ///
    /// Clear the interrupt stack, abandoning all pending interrupts
    #[inline]
    pub fn clear_interrupt_stack(&mut self) {
        self.interrupt_stack.clear_interrupt_stack();
    }

    /// 清空状态历史队列
    ///
    /// Clear the state history queue
    #[cfg(feature = "history")]
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 检查是否正在转换状态
    ///
    /// Check if the state is transitioning
    pub fn is_transitioning(&self) -> bool {
        self.transition_queue.next() != Transition::End
    }

    #[inline]
    fn get_state_tree<'w>(
        query_state_tree: &'w Query<&StateTree>,
        state_tree_id: Entity,
    ) -> Option<&'w StateTree> {
        query_state_tree.get(state_tree_id).ok()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_hsm_trigger(
        on: On<HsmTrigger>,
        mut commands: Commands,
        query_state_tree: Query<&StateTree>,
        query_state_machine: Query<&HsmStateMachine, Without<Paused>>,
        guard_registry: Res<GuardRegistry>,
    ) {
        let HsmTrigger {
            state_machine,
            typed,
        } = on.event();
        let state_machine_id = *state_machine;

        let Ok(state_machine) = query_state_machine.get(state_machine_id) else {
            return;
        };

        let state_tree_id = state_machine.state_tree();

        match typed {
            super::event::HsmTriggerType::ToSuper => {
                Self::handle_to_super(&mut commands, state_machine_id, state_tree_id);
            }
            super::event::HsmTriggerType::ToSub(enter_state_id) => {
                Self::handle_to_sub(
                    &mut commands,
                    state_machine_id,
                    state_tree_id,
                    *enter_state_id,
                );
            }
            super::event::HsmTriggerType::Chain(next_state_id) => {
                handle_chain_transition(
                    &mut commands,
                    state_machine_id,
                    state_tree_id,
                    *next_state_id,
                );
            }
            super::event::HsmTriggerType::Interrupt(target_tree_id, interrupt_state_id) => {
                let target_tree_id = *target_tree_id;
                let interrupt_state_id = *interrupt_state_id;
                if target_tree_id == state_tree_id {
                    // Same tree: validate LCA inside queue so curr_state is fresh.
                    commands.queue(move |world: &mut World| {
                        let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
                            Some(sm) => sm.curr_state_id(),
                            None => return,
                        };
                        if curr_state_id == interrupt_state_id {
                            return;
                        }
                        let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
                            error_event_world(
                                world,
                                state_machine_id,
                                StateMachineError::StateTreeNotFound(state_tree_id),
                            );
                            return;
                        };
                        let Some((exit_path, enter_path)) =
                            state_tree.find_lca_and_paths(curr_state_id, interrupt_state_id)
                        else {
                            error_event_world(
                                world,
                                state_machine_id,
                                StateMachineError::LcaNotFound {
                                    state_machine: state_machine_id,
                                    from: curr_state_id,
                                    to: interrupt_state_id,
                                },
                            );
                            return;
                        };
                        let lca = *exit_path.last().unwrap();
                        if let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) {
                            sm.push_interrupt(state_tree_id, curr_state_id);
                        }
                        apply_chain_transitions(
                            world,
                            state_machine_id,
                            state_tree_id,
                            curr_state_id,
                            exit_path,
                            enter_path,
                            lca,
                        );
                    });
                } else {
                    // Cross-tree: validate target tree first, then queue push +
                    // transition atomically.
                    let Some(target_tree) = Self::get_state_tree(&query_state_tree, target_tree_id)
                    else {
                        warn_event(
                            &mut commands,
                            state_machine_id,
                            StateMachineError::StateTreeNotFound(target_tree_id),
                        );
                        return;
                    };
                    let enter_path: Vec<Entity> = std::iter::once(interrupt_state_id)
                        .chain(target_tree.path_iter(interrupt_state_id))
                        .collect();
                    commands.queue(move |world: &mut World| {
                        let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
                            Some(sm) => sm.curr_state_id(),
                            None => return,
                        };
                        if let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) {
                            sm.push_interrupt(state_tree_id, curr_state_id);
                        }
                        apply_cross_tree_transition(
                            world,
                            state_machine_id,
                            curr_state_id,
                            state_tree_id,
                            target_tree_id,
                            enter_path,
                        );
                    });
                }
            }
            super::event::HsmTriggerType::Resume => {
                commands.queue(move |world: &mut World| {
                    // Peek first — validate before consuming the interrupt frame
                    let frame = {
                        let Some(sm) = world.get::<HsmStateMachine>(state_machine_id) else {
                            return;
                        };
                        match sm.interrupt_stack.peek_interrupt() {
                            Some(frame) => frame,
                            None => {
                                error_event_world(
                                    world,
                                    state_machine_id,
                                    StateMachineError::InterruptStackEmpty(state_machine_id),
                                );
                                return;
                            }
                        }
                    };

                    let (from_state, curr_tree) =
                        match world.get::<HsmStateMachine>(state_machine_id) {
                            Some(sm) => (sm.curr_state_id(), sm.state_tree()),
                            None => return,
                        };

                    if from_state == frame.state_id && curr_tree == frame.graph_id {
                        // Already at the target — pop and discard the frame
                        if let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) {
                            sm.pop_interrupt();
                        }
                        return;
                    }

                    // Validate preconditions before popping
                    if frame.graph_id == curr_tree {
                        // Same tree: validate LCA paths
                        let state_tree_id = curr_tree;
                        let paths = {
                            let Some(state_tree) = world.get::<StateTree>(curr_tree) else {
                                error_event_world(
                                    world,
                                    state_machine_id,
                                    StateMachineError::StateTreeNotFound(curr_tree),
                                );
                                return;
                            };
                            let Some(paths) =
                                state_tree.find_lca_and_paths(from_state, frame.state_id)
                            else {
                                error_event_world(
                                    world,
                                    state_machine_id,
                                    StateMachineError::LcaNotFound {
                                        state_machine: state_machine_id,
                                        from: from_state,
                                        to: frame.state_id,
                                    },
                                );
                                return;
                            };
                            paths
                        };

                        // All valid — now safe to pop
                        {
                            let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id)
                            else {
                                return;
                            };
                            sm.pop_interrupt();
                        }

                        let (exit_path, enter_path) = paths;
                        let Some(lca) = exit_path.last().copied() else {
                            error_event_world(
                                world,
                                state_machine_id,
                                StateMachineError::LcaNotFound {
                                    state_machine: state_machine_id,
                                    from: from_state,
                                    to: frame.state_id,
                                },
                            );
                            return;
                        };
                        apply_chain_transitions(
                            world,
                            state_machine_id,
                            state_tree_id,
                            from_state,
                            exit_path,
                            enter_path,
                            lca,
                        );
                    } else {
                        // Cross-tree resume: validate saved tree exists
                        let enter_path: Vec<Entity> = {
                            let Some(saved_tree) = world.get::<StateTree>(frame.graph_id) else {
                                error_event_world(
                                    world,
                                    state_machine_id,
                                    StateMachineError::StateTreeNotFound(frame.graph_id),
                                );
                                return;
                            };
                            std::iter::once(frame.state_id)
                                .chain(saved_tree.path_iter(frame.state_id))
                                .collect()
                        };

                        // All valid — now safe to pop
                        {
                            let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id)
                            else {
                                return;
                            };
                            sm.pop_interrupt();
                        }

                        apply_cross_tree_transition(
                            world,
                            state_machine_id,
                            from_state,
                            curr_tree,
                            frame.graph_id,
                            enter_path,
                        );
                    }
                });
            }
            _ => match typed {
                super::event::HsmTriggerType::GuardSub(guard, enter_state_id) => {
                    Self::handle_guard_sub(
                        &mut commands,
                        state_machine_id,
                        state_tree_id,
                        *enter_state_id,
                        guard,
                        &guard_registry,
                    );
                }
                crate::prelude::HsmTriggerType::GuardSuper(guard) => {
                    Self::handle_guard_super(
                        &mut commands,
                        state_machine_id,
                        state_tree_id,
                        guard,
                        &guard_registry,
                    );
                }
                _ => unreachable!("Unexpected HsmTriggerType: {:?}", typed),
            },
        };
    }

    fn handle_to_super(commands: &mut Commands, state_machine_id: Entity, state_tree_id: Entity) {
        commands.queue(move |world: &mut World| {
            let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
                Some(sm) => sm.curr_state_id(),
                None => return,
            };
            let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::StateTreeNotFound(state_tree_id),
                );
                return;
            };
            let Some(super_state_id) = state_tree.get_super_state(curr_state_id) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::SuperStateNotFound {
                        state_tree: state_tree_id,
                        state: curr_state_id,
                    },
                );
                return;
            };
            let _ = handle_exit_transition(
                state_machine_id,
                state_tree_id,
                curr_state_id,
                super_state_id,
            )
            .apply(world);
        });
    }

    fn handle_to_sub(
        commands: &mut Commands,
        state_machine_id: Entity,
        state_tree_id: Entity,
        enter_state_id: Entity,
    ) {
        commands.queue(move |world: &mut World| {
            let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
                Some(sm) => sm.curr_state_id(),
                None => return,
            };
            let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::StateTreeNotFound(state_tree_id),
                );
                return;
            };
            if state_tree
                .get_sub_states(curr_state_id)
                .is_none_or(|sub_states| !sub_states.contains(&enter_state_id))
            {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::SubStateNotFound {
                        state_tree: state_tree_id,
                        state: curr_state_id,
                    },
                );
                return;
            }
            let Some(strategy) = world.get::<HsmState>(curr_state_id).map(|s| s.strategy) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::HsmStateMissing(curr_state_id),
                );
                return;
            };
            let _ =
                handle_enter_transition(state_machine_id, curr_state_id, enter_state_id, strategy)
                    .apply(world);
        });
    }

    fn handle_guard_super(
        commands: &mut Commands,
        state_machine_id: Entity,
        state_tree_id: Entity,
        guard: &GuardCondition,
        guard_registry: &GuardRegistry,
    ) {
        let guard = match guard_registry.to_combinator_condition_id(guard) {
            Ok(guard) => guard,
            Err(err) => {
                warn!("{}", err);
                error_event(
                    commands,
                    state_machine_id,
                    StateMachineError::GuardNotFound {
                        condition: format!("{:?}", guard),
                        target: Entity::PLACEHOLDER,
                    },
                );
                return;
            }
        };
        commands.queue(move |world: &mut World| {
            let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
                Some(sm) => sm.curr_state_id(),
                None => return,
            };
            let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::StateTreeNotFound(state_tree_id),
                );
                return;
            };
            let Some(exit_state_id) = state_tree.get_super_state(curr_state_id) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::SuperStateNotFound {
                        state_tree: state_tree_id,
                        state: curr_state_id,
                    },
                );
                return;
            };
            let service_target = crate::state_actions::get_service_target(world, state_machine_id);
            let context = GuardContext::new(
                service_target,
                state_machine_id,
                curr_state_id,
                exit_state_id,
            );
            match guard.run(world, context) {
                Ok(true) => {
                    let _ = handle_exit_transition(
                        state_machine_id,
                        state_tree_id,
                        curr_state_id,
                        exit_state_id,
                    )
                    .apply(world);
                }
                Ok(false) => {}
                Err(e) => {
                    error_event_world(
                        world,
                        state_machine_id,
                        StateMachineError::GuardRunFailed {
                            state_machine: state_machine_id,
                            from_state: curr_state_id,
                            to_state: None,
                            source: e.to_string(),
                        },
                    );
                }
            }
        });
    }

    fn handle_guard_sub(
        commands: &mut Commands,
        state_machine_id: Entity,
        state_tree_id: Entity,
        enter_state_id: Entity,
        guard: &GuardCondition,
        guard_registry: &GuardRegistry,
    ) {
        let guard = match guard_registry.to_combinator_condition_id(guard) {
            Ok(guard) => guard,
            Err(err) => {
                warn!("{}", err);
                error_event(
                    commands,
                    state_machine_id,
                    StateMachineError::GuardNotFound {
                        condition: format!("{:?}", guard),
                        target: enter_state_id,
                    },
                );
                return;
            }
        };
        commands.queue(move |world: &mut World| {
            let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
                Some(sm) => sm.curr_state_id(),
                None => return,
            };
            let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::StateTreeNotFound(state_tree_id),
                );
                return;
            };
            if state_tree
                .get_sub_states(curr_state_id)
                .is_none_or(|sub_states| !sub_states.contains(&enter_state_id))
            {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::SubStateNotFound {
                        state_tree: state_tree_id,
                        state: curr_state_id,
                    },
                );
                return;
            }
            let Some(strategy) = world.get::<HsmState>(curr_state_id).map(|s| s.strategy) else {
                warn_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::HsmStateMissing(curr_state_id),
                );
                return;
            };
            let service_target = crate::state_actions::get_service_target(world, state_machine_id);
            let context = GuardContext::new(
                service_target,
                state_machine_id,
                curr_state_id,
                enter_state_id,
            );
            match guard.run(world, context) {
                Ok(true) => {
                    let _ = handle_enter_transition(
                        state_machine_id,
                        curr_state_id,
                        enter_state_id,
                        strategy,
                    )
                    .apply(world);
                }
                Ok(false) => {}
                Err(e) => {
                    error_event_world(
                        world,
                        state_machine_id,
                        StateMachineError::GuardRunFailed {
                            state_machine: state_machine_id,
                            from_state: curr_state_id,
                            to_state: Some(enter_state_id),
                            source: e.to_string(),
                        },
                    );
                }
            }
        });
    }
}

/// Performs a cross-tree transition: exits the current state from the old tree,
/// switches the state machine to the new tree, and enters the target state.
/// Used by the Interrupt and Resume handlers when source and target are in
/// different state trees.
fn apply_cross_tree_transition(
    world: &mut World,
    state_machine_id: Entity,
    curr_state_id: Entity,
    old_tree_id: Entity,
    new_tree_id: Entity,
    enter_path: Vec<Entity>,
) {
    let mut transition_table = Vec::new();

    // ── Exit from old tree ────────────────────────────────────
    // Exit the current state, then cascade up to the root
    // (no LCA bound since we're leaving the tree entirely).
    transition_table.push(Transition::Exit(curr_state_id));

    if let Some(super_state) = world
        .get::<StateTree>(old_tree_id)
        .and_then(|tree| tree.get_super_state(curr_state_id))
    {
        let hsm = match world.get::<HsmState>(super_state).copied() {
            Some(h) => h,
            None => {
                error_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::HsmStateMissing(super_state),
                );
                return;
            }
        };

        // Force Death behavior: cross-tree exits must fully leave
        // the old tree, never re-enter or update via Rebirth/Resurrection.
        match build_exit_transition_plan(
            world,
            old_tree_id,
            super_state,
            hsm.strategy,
            ExitTransitionBehavior::Death,
            None, // exit all the way to root
        ) {
            Ok(ts) => transition_table.extend(ts),
            Err(e) => {
                error_event_world(world, state_machine_id, e);
                return;
            }
        }
    }

    // ── Enter path in new tree ────────────────────────────────
    // Cross-tree entry mirrors initial HSM boot: only the target leaf
    // state receives the Enter lifecycle. Ancestors are implicit from
    // the state tree hierarchy and must NOT be entered separately —
    // doing so would trigger their OnUpdate systems with no cleanup.
    //
    // build_enter_transition_plan treats the last element of enter_path
    // as an already-active reference point (LCA), so for a two-element
    // [target, root] path it correctly returns only [Enter(target)].
    // For single-element [root] paths we enter the root directly.
    match build_enter_transition_plan(world, &enter_path) {
        Ok(enter_plan) => {
            transition_table.extend(enter_plan);
        }
        Err(err) => {
            error_event_world(world, state_machine_id, err);
            return;
        }
    };

    // ── Switch to new tree ────────────────────────────────────
    // Only mutate after all validation succeeds, so a failed enter
    // plan does not leave the state machine pointing to the wrong tree.
    let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) else {
        error_event_world(
            world,
            state_machine_id,
            StateMachineError::HsmStateMachineMissing(state_machine_id),
        );
        return;
    };
    sm.state_tree = new_tree_id;

    // ── Apply first transition, queue the rest ───────────────
    let first = transition_table.first().copied().unwrap_or(Transition::End);
    let rest = &transition_table[1..];

    let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) else {
        error!(
            "{}",
            StateMachineError::HsmStateMachineMissing(state_machine_id)
        );
        return;
    };

    sm.push_next_states(rest.iter().copied());

    if let Some((state_id, lifecycle)) = first.to() {
        sm.set_curr_state(state_id);
        world.entity_mut(state_machine_id).insert(lifecycle);
    }
}

/// Builds and applies chain transition plans directly on the world.
/// Used by [`handle_chain_transition`] and the Resume interrupt handler.
fn apply_chain_transitions(
    world: &mut World,
    state_machine_id: Entity,
    state_tree_id: Entity,
    curr_state_id: Entity,
    exit_path: Vec<Entity>,
    enter_path: Vec<Entity>,
    lca: Entity,
) {
    let mut transition_table = Vec::new();

    // ── Exit path ────────────────────────────────────────────
    // exit_path = [curr_state, …, LCA]
    if exit_path.len() > 1 {
        transition_table.push(Transition::Exit(curr_state_id));

        let state_tree = match world.get::<StateTree>(state_tree_id) {
            Some(tree) => tree,
            None => {
                error_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::StateTreeNotFound(state_tree_id),
                );
                return;
            }
        };

        // Let build_exit_transition_plan cascade from the first
        // super-state, bounded by the LCA.
        if let Some(super_state) = state_tree.get_super_state(curr_state_id) {
            let Ok(hsm) = world
                .get::<HsmState>(super_state)
                .copied()
                .ok_or(StateMachineError::HsmStateMissing(super_state))
            else {
                error_event_world(
                    world,
                    state_machine_id,
                    StateMachineError::HsmStateMissing(super_state),
                );
                return;
            };

            match build_exit_transition_plan(
                world,
                state_tree_id,
                super_state,
                hsm.strategy,
                hsm.behavior,
                Some(lca),
            ) {
                Ok(ts) => transition_table.extend(ts),
                Err(e) => {
                    error_event_world(world, state_machine_id, e);
                    return;
                }
            }
        }
    }

    // ── Enter path ───────────────────────────────────────────
    match build_enter_transition_plan(world, &enter_path) {
        Ok(enter_plan) => {
            transition_table.extend(enter_plan);
        }
        Err(err) => {
            error_event_world(world, state_machine_id, err);
            return;
        }
    };

    // ── Apply first transition, queue the rest ───────────────
    let first = transition_table.first().copied().unwrap_or(Transition::End);
    let rest = &transition_table[1..];

    let Some(mut state_machine) = world.get_mut::<HsmStateMachine>(state_machine_id) else {
        error_event_world(
            world,
            state_machine_id,
            StateMachineError::HsmStateMachineMissing(state_machine_id),
        );
        return;
    };

    state_machine.push_next_states(rest.iter().copied());

    if let Some((state_id, lifecycle)) = first.to() {
        state_machine.set_curr_state(state_id);
        world.entity_mut(state_machine_id).insert(lifecycle);
    }
}

/// Queues a command that builds the chain transition plan using the unified
/// exit/enter helpers in [`transition_strategy`], then applies the first
/// transition immediately and queues the rest.
fn handle_chain_transition(
    commands: &mut Commands,
    state_machine_id: Entity,
    state_tree_id: Entity,
    next_state_id: Entity,
) {
    commands.queue(move |world: &mut World| {
        let curr_state_id = match world.get::<HsmStateMachine>(state_machine_id) {
            Some(sm) => sm.curr_state_id(),
            None => return,
        };
        if curr_state_id == next_state_id {
            return;
        }
        let Some(state_tree) = world.get::<StateTree>(state_tree_id) else {
            error_event_world(
                world,
                state_machine_id,
                StateMachineError::StateTreeNotFound(state_tree_id),
            );
            return;
        };
        let Some((exit_path, enter_path)) =
            state_tree.find_lca_and_paths(curr_state_id, next_state_id)
        else {
            error_event_world(
                world,
                state_machine_id,
                StateMachineError::LcaNotFound {
                    state_machine: state_machine_id,
                    from: curr_state_id,
                    to: next_state_id,
                },
            );
            return;
        };
        let lca = *exit_path.last().unwrap();
        apply_chain_transitions(
            world,
            state_machine_id,
            state_tree_id,
            curr_state_id,
            exit_path,
            enter_path,
            lca,
        );
    });
}

impl Debug for HsmStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(feature = "history")]
        {
            f.debug_struct("HsmStateMachine")
                .field("history", &self.history.iter().collect::<Vec<_>>())
                .field("transition_queue", &self.transition_queue)
                .field("curr_state", &self.curr_state)
                .field("init_state", &self.init_state)
                .field("interrupt_stack", &self.interrupt_stack)
                .finish()
        }
        #[cfg(not(feature = "history"))]
        {
            f.debug_struct("HsmStateMachine")
                .field("transition_queue", &self.transition_queue)
                .field("curr_state", &self.curr_state)
                .field("init_state", &self.init_state)
                .field("interrupt_stack", &self.interrupt_stack)
                .finish()
        }
    }
}

#[cfg(test)]
#[path = "../tests/hsm_state_machine_interrupt_tests.rs"]
mod interrupt_tests;
