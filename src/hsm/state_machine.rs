use std::{collections::VecDeque, fmt::Debug};

use bevy::prelude::*;

use crate::{
    context::{GuardContext, TransitionRelationship},
    error::{StateMachineError, error_event, error_event_world, warn_event},
    guards::{CompiledGuard, GuardCondition, GuardRegistry},
    hsm::{
        HsmState,
        event::HsmTrigger,
        state_lifecycle::StateLifecycle,
        transition_strategy::{
            StateTransitionStrategy, build_enter_transition_plan, build_exit_transition_plan,
            handle_enter_transition, handle_exit_transition,
        },
    },
    interrupt::{InterruptFrame, InterruptStack},
    markers::Paused,
    prelude::{ServiceTarget, StateTree},
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

    /// 将一个状态转换插入到队列的前面
    ///
    /// Insert a state transition at the front of the queue
    pub fn push_prev_state(&mut self, prev_state: Transition) -> Transition {
        self.transition_queue.push_prev(prev_state)
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
        self.transition_queue.prev() != Transition::Start
            && self.transition_queue.next() != Transition::End
    }

    #[inline]
    fn get_state_tree<'w>(
        query_state_tree: &'w Query<&StateTree>,
        state_tree_id: Entity,
    ) -> Option<&'w StateTree> {
        query_state_tree.get(state_tree_id).ok()
    }

    #[inline]
    fn get_state_strategy(
        query_state: &Query<&HsmState>,
        state_id: Entity,
    ) -> Option<StateTransitionStrategy> {
        query_state
            .get(state_id)
            .ok()
            .map(|hsm_state| hsm_state.strategy)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_hsm_trigger(
        on: On<HsmTrigger>,
        mut commands: Commands,
        query_state: Query<&HsmState>,
        query_state_tree: Query<&StateTree>,
        query_state_machine: Query<&HsmStateMachine, Without<Paused>>,
        query_service_target: Query<&ServiceTarget, With<HsmStateMachine>>,
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
        let curr_state_id = state_machine.curr_state_id();

        let Some(state_tree) = Self::get_state_tree(&query_state_tree, state_tree_id) else {
            warn_event(
                &mut commands,
                state_machine_id,
                StateMachineError::StateTreeNotFound(state_tree_id),
            );
            return;
        };

        match typed {
            super::event::HsmTriggerType::ToSuper => {
                Self::handle_to_super(
                    &mut commands,
                    state_machine_id,
                    state_tree_id,
                    curr_state_id,
                    state_tree,
                );
            }
            super::event::HsmTriggerType::ToSub(enter_state_id) => {
                Self::handle_to_sub(
                    &mut commands,
                    state_machine_id,
                    state_tree_id,
                    curr_state_id,
                    *enter_state_id,
                    state_tree,
                    &query_state,
                );
            }
            super::event::HsmTriggerType::Chain(next_state_id) => {
                handle_chain_transition(
                    &mut commands,
                    state_machine_id,
                    curr_state_id,
                    state_tree_id,
                    *next_state_id,
                    state_tree,
                );
            }
            super::event::HsmTriggerType::Interrupt(target_tree_id, interrupt_state_id) => {
                let target_tree_id = *target_tree_id;
                let interrupt_state_id = *interrupt_state_id;
                if curr_state_id == interrupt_state_id && state_tree_id == target_tree_id {
                    return;
                }
                // Save current state tree and state on interrupt stack
                commands.queue(move |world: &mut World| {
                    if let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) {
                        let saved = sm.curr_state_id();
                        sm.push_interrupt(state_tree_id, saved);
                    }
                });
                if target_tree_id == state_tree_id {
                    // Same tree: use existing LCA-based chain transition
                    handle_chain_transition(
                        &mut commands,
                        state_machine_id,
                        curr_state_id,
                        state_tree_id,
                        interrupt_state_id,
                        state_tree,
                    );
                } else {
                    // Cross-tree: exit old tree, switch tree, enter new tree
                    let Some(target_tree) = Self::get_state_tree(&query_state_tree, target_tree_id)
                    else {
                        warn_event(
                            &mut commands,
                            state_machine_id,
                            StateMachineError::StateTreeNotFound(target_tree_id),
                        );
                        return;
                    };
                    let enter_path: Vec<Entity> =
                        target_tree.path_iter(interrupt_state_id).collect();
                    commands.queue(move |world: &mut World| {
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
                    let frame = {
                        let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id)
                        else {
                            return;
                        };
                        match sm.pop_interrupt() {
                            Some(frame) => frame,
                            None => {
                                warn!(
                                    "[HSM] Resume triggered on state machine {:?} with empty \
                                     interrupt stack",
                                    state_machine_id
                                );
                                return;
                            }
                        }
                    };

                    let from_state = match world.get::<HsmStateMachine>(state_machine_id) {
                        Some(sm) => sm.curr_state_id(),
                        None => return,
                    };
                    let curr_tree = match world.get::<HsmStateMachine>(state_machine_id) {
                        Some(sm) => sm.state_tree(),
                        None => return,
                    };

                    if from_state == frame.state_id && curr_tree == frame.graph_id {
                        return;
                    }

                    if frame.graph_id == curr_tree {
                        // Same tree: use existing LCA-based resume
                        let (exit_path, enter_path) = {
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
                        let lca = *exit_path.last().unwrap();
                        apply_chain_transitions(
                            world,
                            state_machine_id,
                            curr_tree,
                            from_state,
                            exit_path,
                            enter_path,
                            lca,
                        );
                    } else {
                        // Cross-tree resume: exit current tree, switch, enter saved tree
                        let enter_path: Vec<Entity> = {
                            let Some(saved_tree) = world.get::<StateTree>(frame.graph_id) else {
                                error_event_world(
                                    world,
                                    state_machine_id,
                                    StateMachineError::StateTreeNotFound(frame.graph_id),
                                );
                                return;
                            };
                            saved_tree.path_iter(frame.state_id).collect()
                        };
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
            _ => {
                let service_target = match query_service_target.get(state_machine_id) {
                    Ok(target) => target.0,
                    Err(_) => state_machine_id,
                };
                match typed {
                    super::event::HsmTriggerType::GuardSub(guard, enter_state_id) => {
                        let context = GuardContext::new(
                            service_target,
                            state_machine_id,
                            curr_state_id,
                            *enter_state_id,
                        );
                        Self::handle_guard_sub(
                            &mut commands,
                            state_tree_id,
                            state_tree,
                            context,
                            guard,
                            &guard_registry,
                            &query_state,
                        );
                    }
                    crate::prelude::HsmTriggerType::GuardSuper(guard) => {
                        Self::handle_guard_super(
                            &mut commands,
                            service_target,
                            state_machine_id,
                            state_tree_id,
                            curr_state_id,
                            state_tree,
                            guard,
                            &guard_registry,
                        );
                    }
                    _ => unreachable!("Unexpected HsmTriggerType: {:?}", typed),
                }
            }
        };
    }

    fn handle_guard_transition<F, C>(
        guard: CompiledGuard,
        context: GuardContext,
        handle_transition: F,
    ) -> impl Command<Out = Result<()>>
    where
        F: FnOnce() -> C + Send + Sync + 'static,
        C: Command<Out = Result<()>> + 'static,
    {
        move |world: &mut World| -> Result<()> {
            if guard.run(world, context)? {
                return handle_transition().apply(world);
            }
            Ok(())
        }
    }

    fn handle_to_super(
        commands: &mut Commands,
        state_machine_id: Entity,
        state_tree_id: Entity,
        curr_state_id: Entity,
        state_tree: &StateTree,
    ) {
        if let Some(super_state_id) = state_tree.get_super_state(curr_state_id) {
            commands.queue(handle_exit_transition(
                state_machine_id,
                state_tree_id,
                curr_state_id,
                super_state_id,
            ));
        }
    }

    fn handle_to_sub(
        commands: &mut Commands,
        state_machine_id: Entity,
        state_tree_id: Entity,
        curr_state_id: Entity,
        enter_state_id: Entity,
        state_tree: &StateTree,
        query_state: &Query<&HsmState>,
    ) {
        if state_tree
            .get_sub_states(curr_state_id)
            .is_none_or(|sub_states| !sub_states.contains(&enter_state_id))
        {
            warn_event(
                commands,
                state_machine_id,
                StateMachineError::SubStateNotFound {
                    state_tree: state_tree_id,
                    state: curr_state_id,
                },
            );
            return;
        }

        let Some(strategy) = Self::get_state_strategy(query_state, curr_state_id) else {
            warn_event(
                commands,
                state_machine_id,
                StateMachineError::HsmStateMissing(curr_state_id),
            );
            return;
        };

        commands.queue(handle_enter_transition(
            state_machine_id,
            curr_state_id,
            enter_state_id,
            strategy,
        ));
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_guard_super(
        commands: &mut Commands,
        service_target: Entity,
        state_machine_id: Entity,
        state_tree_id: Entity,
        curr_state_id: Entity,
        state_tree: &StateTree,
        guard: &GuardCondition,
        guard_registry: &GuardRegistry,
    ) {
        let Some(exit_state_id) = state_tree.get_super_state(curr_state_id) else {
            warn_event(
                commands,
                state_machine_id,
                StateMachineError::SuperStateNotFound {
                    state_tree: state_tree_id,
                    state: curr_state_id,
                },
            );
            return;
        };

        let guard = match guard_registry.to_combinator_condition_id(guard) {
            Ok(guard) => guard,
            Err(err) => {
                warn!("{}", err);
                return;
            }
        };
        let context = GuardContext::new(
            service_target,
            state_machine_id,
            curr_state_id,
            exit_state_id,
        );
        commands.queue(Self::handle_guard_transition(guard, context, move || {
            handle_exit_transition(
                state_machine_id,
                state_tree_id,
                curr_state_id,
                exit_state_id,
            )
        }));
    }

    fn handle_guard_sub(
        commands: &mut Commands,
        state_tree_id: Entity,
        state_tree: &StateTree,
        context: GuardContext,
        guard: &GuardCondition,
        guard_registry: &GuardRegistry,
        query_state: &Query<&HsmState>,
    ) {
        if state_tree
            .get_sub_states(context.from_state())
            .is_none_or(|sub_states| !sub_states.contains(&context.to_state()))
        {
            warn_event(
                commands,
                context.state_machine,
                StateMachineError::SubStateNotFound {
                    state_tree: state_tree_id,
                    state: context.from_state(),
                },
            );
            return;
        }

        let Some(strategy) = Self::get_state_strategy(query_state, context.from_state()) else {
            warn_event(
                commands,
                context.state_machine,
                StateMachineError::HsmStateMissing(context.from_state()),
            );
            return;
        };

        let guard = match guard_registry.to_combinator_condition_id(guard) {
            Ok(guard) => guard,
            Err(err) => {
                warn!("{}", err);
                return;
            }
        };
        commands.queue(Self::handle_guard_transition(guard, context, move || {
            handle_enter_transition(
                context.state_machine,
                context.from_state(),
                context.to_state(),
                strategy,
            )
        }));
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

        match build_exit_transition_plan(
            world,
            old_tree_id,
            super_state,
            hsm.strategy,
            hsm.behavior,
            None, // exit all the way to root
        ) {
            Ok(ts) => transition_table.extend(ts),
            Err(e) => {
                error_event_world(world, state_machine_id, e);
                return;
            }
        }
    }

    // ── Switch to new tree ────────────────────────────────────
    {
        let Some(mut sm) = world.get_mut::<HsmStateMachine>(state_machine_id) else {
            error_event_world(
                world,
                state_machine_id,
                StateMachineError::HsmStateMachineMissing(state_machine_id),
            );
            return;
        };
        sm.state_tree = new_tree_id;
    }

    // ── Enter path in new tree ────────────────────────────────
    match build_enter_transition_plan(world, &enter_path) {
        Ok(enter_plan) => {
            transition_table.extend(enter_plan);
        }
        Err(err) => {
            error!("{}", err);
            return;
        }
    };

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
            error!("{}", err);
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
    curr_state_id: Entity,
    state_tree_id: Entity,
    next_state_id: Entity,
    state_tree: &StateTree,
) {
    if curr_state_id == next_state_id {
        return;
    }

    let Some((exit_path, enter_path)) = state_tree.find_lca_and_paths(curr_state_id, next_state_id)
    else {
        error_event(
            commands,
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

    commands.queue(move |world: &mut World| {
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

/// # 状态转换\State Transition
/// * 状态转换的枚举，包含下一个状态的ID和OnState
/// - The enum of state transitions, including the ID of the next state and OnState
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Enter(Entity),
    Update(Entity),
    Exit(Entity),
    Start,
    End,
}

impl Transition {
    pub const fn to(self) -> Option<(Entity, StateLifecycle)> {
        match self {
            Transition::Enter(id) => Some((id, StateLifecycle::Enter)),
            Transition::Update(id) => Some((id, StateLifecycle::Update)),
            Transition::Exit(id) => Some((id, StateLifecycle::Exit)),
            Transition::Start | Transition::End => None,
        }
    }

    pub fn to_transition(self, next: Self) -> Option<TransitionRelationship> {
        use Transition::*;
        match (self, next) {
            // Represents the initial entry into the state machine.
            (Start, Enter(to)) | (Start, Update(to)) | (Start, Exit(to)) => {
                Some(TransitionRelationship::Final(to))
            }

            // Represents the final exit from the state machine.
            (Enter(from), End) | (Update(from), End) | (Exit(from), End) => {
                Some(TransitionRelationship::Initial(from))
            }

            // Represents a standard transition between two different states.
            (Enter(from), Enter(to))
            | (Enter(from), Update(to))
            | (Enter(from), Exit(to))
            | (Update(from), Enter(to))
            | (Update(from), Update(to))
            | (Update(from), Exit(to))
            | (Exit(from), Enter(to))
            | (Exit(from), Update(to))
            | (Exit(from), Exit(to)) => Some(TransitionRelationship::Transition(from, to)),
            // All other combinations are considered invalid transitions.
            _ => {
                error!("Invalid state transition pair: {:?} -> {:?}", self, next);
                None
            }
        }
    }

    pub const fn with_behavior(
        state_id: Entity,
        behavior: crate::prelude::ExitTransitionBehavior,
    ) -> Self {
        use crate::prelude::ExitTransitionBehavior;
        match behavior {
            ExitTransitionBehavior::Rebirth => Self::Enter(state_id),
            ExitTransitionBehavior::Resurrection => Self::Update(state_id),
            ExitTransitionBehavior::Death => Self::Exit(state_id),
        }
    }

    pub const fn with_lifecycle(state_id: Entity, lifecycle: StateLifecycle) -> Self {
        match lifecycle {
            StateLifecycle::Enter => Self::Enter(state_id),
            StateLifecycle::Update => Self::Update(state_id),
            StateLifecycle::Exit => Self::Exit(state_id),
        }
    }

    pub const fn get_state_id(&self) -> Option<Entity> {
        match self {
            Self::Enter(id) | Self::Update(id) | Self::Exit(id) => Some(*id),
            Self::Start | Self::End => None,
        }
    }

    pub const fn get_lifecyle(&self) -> Option<StateLifecycle> {
        match self {
            Transition::Enter(_) => Some(StateLifecycle::Enter),
            Transition::Update(_) => Some(StateLifecycle::Update),
            Transition::Exit(_) => Some(StateLifecycle::Exit),
            Transition::Start | Transition::End => None,
        }
    }
}

impl Debug for Transition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enter(id) => write!(f, "Enter({})", id),
            Self::Update(id) => write!(f, "Update({})", id),
            Self::Exit(id) => write!(f, "Exit({})", id),
            Self::Start => write!(f, "Start"),
            Self::End => write!(f, "End"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransitionQueue {
    prev_transition: Transition,
    next_transitions: VecDeque<Transition>,
}

impl TransitionQueue {
    pub fn push(&mut self, transition: Transition) {
        self.next_transitions.push_back(transition);
    }

    pub fn pop(&mut self) -> Transition {
        self.next_transitions.pop_front().unwrap_or(Transition::End)
    }

    pub fn next(&self) -> Transition {
        self.next_transitions
            .front()
            .copied()
            .unwrap_or(Transition::End)
    }

    pub fn push_prev(&mut self, transition: Transition) -> Transition {
        std::mem::replace(&mut self.prev_transition, transition)
    }

    pub fn prev(&self) -> Transition {
        self.prev_transition
    }

    pub fn clear(&mut self) {
        self.next_transitions.clear();
    }

    pub fn len(&self) -> usize {
        self.next_transitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.next_transitions.is_empty()
    }
}

impl Default for TransitionQueue {
    fn default() -> Self {
        Self {
            prev_transition: Transition::Start,
            next_transitions: VecDeque::new(),
        }
    }
}

impl Extend<Transition> for TransitionQueue {
    fn extend<T: IntoIterator<Item = Transition>>(&mut self, iter: T) {
        self.next_transitions.extend(iter);
    }
}

#[cfg(test)]
mod interrupt_tests {
    use bevy::prelude::*;

    use crate::{StateMachinePlugin, context::*, prelude::*, state_actions::*};

    use super::*;

    #[derive(Resource, Default)]
    struct EventLog(Vec<String>);

    fn log_enter(ctx: In<ActionContext>, query: Query<&Name>, mut log: ResMut<EventLog>) {
        let name = query.get(ctx.state()).unwrap();
        log.0.push(format!("{}:Enter", name));
    }

    fn log_exit(ctx: In<ActionContext>, query: Query<&Name>, mut log: ResMut<EventLog>) {
        let name = query.get(ctx.state()).unwrap();
        log.0.push(format!("{}:Exit", name));
    }

    fn log_update(
        contexts: In<Vec<ActionContext>>,
        query: Query<&Name>,
        mut log: ResMut<EventLog>,
    ) -> Option<Vec<ActionContext>> {
        for ctx in &contexts.0 {
            if let Ok(name) = query.get(ctx.state()) {
                log.0.push(format!("{}:Update", name));
            }
        }
        None
    }

    fn create_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StateMachinePlugin::default());
        app
    }

    fn register_log_systems(app: &mut App) {
        app.add_action_system(Update, "log_update", log_update);
        let world = app.world_mut();
        let registry = ActionRegistry::from([
            ("log_enter", world.register_system(log_enter)),
            ("log_exit", world.register_system(log_exit)),
        ]);
        world.insert_resource(registry);
        world.insert_resource(EventLog::default());
    }

    /// Creates two states A (root) and B (child of A), both Parallel+Rebirth.
    /// Returns (state_machine, state_a, state_b).
    fn create_two_state_setup(app: &mut App) -> (Entity, Entity, Entity) {
        register_log_systems(app);

        let world = app.world_mut();
        let state_a = world
            .spawn((
                Name::new("A"),
                HsmState::with(
                    StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Rebirth,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
            ))
            .id();

        let state_b = world
            .spawn((
                Name::new("B"),
                HsmState::with(
                    StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Rebirth,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
                OnUpdateSystem::new("Update:log_update"),
            ))
            .id();

        let mut state_tree = StateTree::new(state_a);
        state_tree.with_child(state_a, state_b);

        let sm = world.spawn_empty().id();
        world.entity_mut(sm).insert((
            state_tree,
            Name::new("TestSM"),
            StateLifecycle::default(),
            HsmStateMachine::with(
                sm,
                state_a,
                #[cfg(feature = "history")]
                10,
            ),
        ));

        (sm, state_a, state_b)
    }

    /// Creates three states A (root), B (child of A), C (child of B).
    fn create_three_state_setup(app: &mut App) -> (Entity, Entity, Entity, Entity) {
        register_log_systems(app);

        let world = app.world_mut();
        let state_a = world
            .spawn((
                Name::new("A"),
                HsmState::with(
                    StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Rebirth,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
            ))
            .id();

        let state_b = world
            .spawn((
                Name::new("B"),
                HsmState::with(
                    StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Rebirth,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
                OnUpdateSystem::new("Update:log_update"),
            ))
            .id();

        let state_c = world
            .spawn((
                Name::new("C"),
                HsmState::with(
                    StateTransitionStrategy::Parallel,
                    ExitTransitionBehavior::Rebirth,
                ),
                AfterEnterSystem::new("log_enter"),
                BeforeExitSystem::new("log_exit"),
                OnUpdateSystem::new("Update:log_update"),
            ))
            .id();

        let mut state_tree = StateTree::new(state_a);
        state_tree.with_child(state_a, state_b);
        state_tree.with_child(state_b, state_c);

        let sm = world.spawn_empty().id();
        world.entity_mut(sm).insert((
            state_tree,
            Name::new("TestSM"),
            StateLifecycle::default(),
            HsmStateMachine::with(
                sm,
                state_a,
                #[cfg(feature = "history")]
                10,
            ),
        ));

        (sm, state_a, state_b, state_c)
    }

    fn get_event_log(app: &App) -> Vec<String> {
        app.world().get_resource::<EventLog>().unwrap().0.clone()
    }

    // ── Unit tests ────────────────────────────────────────────────

    #[test]
    fn interrupt_stack_push_pop() {
        let e = |i| Entity::from_raw_u32(i).unwrap();
        let mut sm = HsmStateMachine::with(
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
            #[cfg(feature = "history")]
            0,
        );

        assert!(!sm.is_interrupted());
        assert_eq!(sm.interrupt_depth(), 0);
        assert_eq!(sm.pop_interrupt(), None);

        sm.push_interrupt(Entity::PLACEHOLDER, e(10));
        assert!(sm.is_interrupted());
        assert_eq!(sm.interrupt_depth(), 1);

        sm.push_interrupt(Entity::PLACEHOLDER, e(20));
        assert_eq!(sm.interrupt_depth(), 2);

        assert_eq!(
            sm.pop_interrupt(),
            Some(InterruptFrame::new(Entity::PLACEHOLDER, e(20)))
        );
        assert_eq!(sm.interrupt_depth(), 1);
        assert!(sm.is_interrupted());

        assert_eq!(
            sm.pop_interrupt(),
            Some(InterruptFrame::new(Entity::PLACEHOLDER, e(10)))
        );
        assert_eq!(sm.interrupt_depth(), 0);
        assert!(!sm.is_interrupted());

        assert_eq!(sm.pop_interrupt(), None);
    }

    #[test]
    fn clear_interrupt_stack() {
        let mut sm = HsmStateMachine::with(
            Entity::PLACEHOLDER,
            Entity::PLACEHOLDER,
            #[cfg(feature = "history")]
            0,
        );

        sm.push_interrupt(Entity::PLACEHOLDER, Entity::from_raw_u32(1).unwrap());
        sm.push_interrupt(Entity::PLACEHOLDER, Entity::from_raw_u32(2).unwrap());
        sm.push_interrupt(Entity::PLACEHOLDER, Entity::from_raw_u32(3).unwrap());
        assert_eq!(sm.interrupt_depth(), 3);

        sm.clear_interrupt_stack();
        assert_eq!(sm.interrupt_depth(), 0);
        assert!(!sm.is_interrupted());
    }

    // ── Integration tests ─────────────────────────────────────────

    #[test]
    fn basic_interrupt_and_resume() {
        let mut app = create_app();
        let (sm, _state_a, state_b) = create_two_state_setup(&mut app);

        // Boot: A enters.
        app.update();
        let initial_log = get_event_log(&app);
        assert!(
            initial_log.iter().any(|e| e == "A:Enter"),
            "Expected A:Enter, got {initial_log:?}"
        );

        // Interrupt A → B (Parallel strategy: A stays active, B enters)
        app.world_mut()
            .entity_mut(sm)
            .trigger(|id| HsmTrigger::interrupt(id, id, state_b));
        app.update();

        let after_interrupt_log = get_event_log(&app);
        assert!(
            after_interrupt_log.iter().any(|e| e == "B:Enter"),
            "Expected B:Enter in {after_interrupt_log:?}"
        );

        // Verify state machine is in state B and interrupted
        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        assert_eq!(sm_comp.curr_state_id(), state_b);
        assert!(sm_comp.is_interrupted());
        assert_eq!(sm_comp.interrupt_depth(), 1);

        // Resume B → A
        app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
        app.update();

        let after_resume_log = get_event_log(&app);
        assert!(
            after_resume_log.iter().any(|e| e == "B:Exit"),
            "Expected B:Exit in {after_resume_log:?}"
        );
        assert!(
            after_resume_log.iter().filter(|e| *e == "A:Enter").count() >= 1,
            "Expected A re-enter in {after_resume_log:?}"
        );

        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        assert_eq!(sm_comp.curr_state_id(), _state_a);
        assert!(!sm_comp.is_interrupted());
    }

    #[test]
    fn nested_interrupt() {
        let mut app = create_app();
        let (sm, _state_a, state_b, state_c) = create_three_state_setup(&mut app);

        // Boot: A enters
        app.update();

        // Interrupt A → B
        app.world_mut()
            .entity_mut(sm)
            .trigger(|id| HsmTrigger::interrupt(id, id, state_b));
        app.update();

        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        assert_eq!(sm_comp.curr_state_id(), state_b);
        assert_eq!(sm_comp.interrupt_depth(), 1);

        // Nested interrupt B → C
        app.world_mut()
            .entity_mut(sm)
            .trigger(|id| HsmTrigger::interrupt(id, id, state_c));
        app.update();

        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        assert_eq!(sm_comp.curr_state_id(), state_c);
        assert_eq!(sm_comp.interrupt_depth(), 2);

        // Resume C → B
        app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
        app.update();

        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        assert_eq!(sm_comp.curr_state_id(), state_b);
        assert_eq!(sm_comp.interrupt_depth(), 1);

        // Resume B → A
        app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
        app.update();

        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        assert_eq!(sm_comp.curr_state_id(), _state_a);
        assert_eq!(sm_comp.interrupt_depth(), 0);
        assert!(!sm_comp.is_interrupted());
    }

    #[test]
    fn resume_with_empty_stack_is_noop() {
        let mut app = create_app();
        let (sm, state_a, _state_b) = create_two_state_setup(&mut app);

        // Boot: A enters
        app.update();
        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        assert_eq!(sm_comp.curr_state_id(), state_a);

        // Resume with empty stack — should be a no-op
        app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
        app.update();

        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        // State should still be A
        assert_eq!(sm_comp.curr_state_id(), state_a);
        assert!(!sm_comp.is_interrupted());
    }

    #[test]
    fn interrupt_to_self_is_noop() {
        let mut app = create_app();
        let (sm, state_a, _state_b) = create_two_state_setup(&mut app);

        // Boot: A enters
        app.update();

        // Interrupt to self
        app.world_mut()
            .entity_mut(sm)
            .trigger(|id| HsmTrigger::interrupt(id, id, state_a));
        app.update();

        let sm_comp = app.world().get::<HsmStateMachine>(sm).unwrap();
        assert_eq!(sm_comp.curr_state_id(), state_a);
        // Stack should be empty since self-interrupt is skipped
        assert!(!sm_comp.is_interrupted());
    }

    #[test]
    fn multiple_interrupts_and_resumes_event_order() {
        let mut app = create_app();
        let (sm, _state_a, state_b, state_c) = create_three_state_setup(&mut app);

        // Boot: A enters
        app.update();

        // Clear the boot log so we only see interrupt-related events
        app.world_mut()
            .get_resource_mut::<EventLog>()
            .unwrap()
            .0
            .clear();

        // Interrupt A → B (Parallel: A stays active, B enters)
        app.world_mut()
            .entity_mut(sm)
            .trigger(|id| HsmTrigger::interrupt(id, id, state_b));
        app.update();

        // Interrupt B → C (Parallel: B stays active, C enters)
        app.world_mut()
            .entity_mut(sm)
            .trigger(|id| HsmTrigger::interrupt(id, id, state_c));
        app.update();

        // Resume C → B
        app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
        app.update();

        // Resume B → A
        app.world_mut().entity_mut(sm).trigger(HsmTrigger::resume);
        app.update();

        let log = get_event_log(&app);

        // With Parallel+Rebirth, the parent does not exit when a child enters.
        // Expected sequence: B:Enter, B:Update, C:Enter, C:Update,
        //                    C:Exit, B:Enter, B:Update, B:Exit, A:Enter
        assert!(
            log.contains(&"B:Enter".to_string()),
            "Expected B:Enter in {log:?}"
        );
        assert!(
            log.contains(&"C:Enter".to_string()),
            "Expected C:Enter in {log:?}"
        );
        assert!(
            log.contains(&"C:Exit".to_string()),
            "Expected C:Exit in {log:?}"
        );
        assert!(
            log.contains(&"B:Exit".to_string()),
            "Expected B:Exit in {log:?}"
        );

        // Verify correct causal ordering
        let idx = |s: &str| {
            log.iter()
                .enumerate()
                .filter(|(_, e)| *e == s)
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        };

        let b_enter = idx("B:Enter");
        let c_enter = idx("C:Enter");
        let c_exit = idx("C:Exit");
        let b_exit = idx("B:Exit");
        let a_enter = idx("A:Enter");

        // B first enters before C enters
        assert!(
            b_enter[0] < c_enter[0],
            "B:Enter({}) before C:Enter({}), log: {log:?}",
            b_enter[0],
            c_enter[0]
        );
        // C exits before B re-enters
        assert!(
            c_exit[0] < b_enter[1],
            "C:Exit({}) before B:Enter({}), log: {log:?}",
            c_exit[0],
            b_enter[1]
        );
        // B exits before A re-enters
        assert!(
            b_exit[0] < a_enter[0],
            "B:Exit({}) before A:Enter({}), log: {log:?}",
            b_exit[0],
            a_enter[0]
        );
    }
}
