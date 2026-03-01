use std::{collections::VecDeque, fmt::Debug};

use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::{
    context::StateActionContext,
    error::StateMachineError,
    hsm::{
        HsmState,
        event::HsmTrigger,
        state_tree::HsmStateId,
        transition_strategy::{
            CheckOnTransitionStates, handle_on_enter_state_command, handle_on_exit_state_command,
        },
    },
    markers::{Paused, Terminated},
    prelude::{ServiceTarget, StateActionBuffer, StateTree},
    state_actions::*,
};

#[cfg(feature = "state_data")]
use crate::state_data::StateData;

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
/// let state_machine = HsmStateMachine::new(HsmStateId::new(tree_id, id), 10);
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
    transition_queue: VecDeque<Transition>,
    curr_state: HsmStateId,
    /// 初始状态
    ///
    /// Initial state
    init_state: HsmStateId,
}

impl HsmStateMachine {
    #[cfg(feature = "history")]
    pub fn new(curr_state: HsmStateId, history_len: usize) -> Self {
        let history = StateHistory::new(history_len);
        Self {
            history,
            curr_state,
            transition_queue: VecDeque::new(),
            init_state: curr_state,
        }
    }

    #[cfg(not(feature = "history"))]
    pub fn new(curr_state: HsmStateId) -> Self {
        Self {
            curr_state,
            transition_queue: VecDeque::new(),
            init_state: curr_state,
        }
    }

    pub const fn init_state(&self) -> HsmStateId {
        self.init_state
    }

    /// 获取当前状态的ID
    ///
    /// Get the ID of the current state
    pub fn curr_state_id(&self) -> HsmStateId {
        self.curr_state
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn next_state_id(&self) -> Option<&Transition> {
        self.transition_queue.front()
    }

    /// 设置初始状态
    ///
    /// Set the initial state
    pub fn set_init_state(&mut self, state: HsmStateId) {
        self.init_state = state;
    }

    /// 设置当前状态
    ///
    /// Set the current state
    pub fn set_curr_state(&mut self, state: HsmStateId) {
        self.curr_state = state;
    }

    /// 添加历史记录
    ///
    /// Add history record
    #[cfg(feature = "history")]
    fn push_history(&mut self, node: HistoricalNode) {
        self.history.push(node);
    }

    /// 添加下一个状态
    ///
    /// Add next state
    pub fn push_next_state(&mut self, next_state: Transition) {
        self.transition_queue.push_front(next_state);
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

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn get_next_state(&self) -> Option<HsmStateId> {
        self.transition_queue.front().and_then(|next| match next {
            Transition::Next((state_id, _)) => Some(*state_id),
            Transition::None => None,
        })
    }

    /// 获取下一个状态的OnState
    ///
    /// Get the OnState of the next state
    pub fn get_next_state_on_state(&self) -> Option<StateLifecycle> {
        self.transition_queue.front().and_then(|next| match next {
            Transition::Next((_, on_state)) => Some(*on_state),
            Transition::None => None,
        })
    }

    /// 弹出下一个状态
    ///
    /// Pop next state
    pub fn pop_next_state(&mut self) -> Transition {
        self.transition_queue
            .pop_front()
            .unwrap_or(Transition::None)
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

    /// 检查是否处于指定状态
    ///
    /// Check if in specified state
    pub fn is_in_state(&self, state: HsmStateId) -> bool {
        self.curr_state_id() == state
    }

    /// 清空下一个状态队列
    ///
    /// Clear the next state queue
    pub fn clear_next_states(&mut self) {
        self.transition_queue.clear();
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
        self.transition_queue
            .front()
            .is_some_and(|n| *n != Transition::None)
    }

    pub(crate) fn handle_hsm_trigger(
        on: On<HsmTrigger>,
        mut commands: Commands,
        query_state: Query<&HsmState>,
        query_state_tree: Query<&StateTree>,
        query: Query<&HsmStateMachine, Without<Paused>>,
    ) {
        let HsmTrigger {
            state_machine: state_machine_id,
            typed,
        } = on.event().clone();

        let Ok(state_machine) = query.get(state_machine_id) else {
            error!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return;
        };

        let curr_state_id = state_machine.curr_state_id();

        let Ok(state_tree) = query_state_tree.get(curr_state_id.tree()) else {
            warn!(
                "{}",
                StateMachineError::StateTreeNotFound(curr_state_id.tree())
            );
            return;
        };

        match typed {
            super::event::HsmTriggerType::Super => {
                if let Some(super_state_id) = state_tree.get_super_state(curr_state_id.state()) {
                    commands.queue(handle_on_exit_state_command(
                        state_machine_id,
                        curr_state_id,
                        super_state_id,
                    ));
                }
            }
            super::event::HsmTriggerType::SuperTransition(super_state_id) => {
                commands.queue(handle_on_exit_state_command(
                    state_machine_id,
                    curr_state_id,
                    super_state_id,
                ));
            }
            super::event::HsmTriggerType::Sub(enter_state_id) => {
                let Some(sub_states) = state_tree.get_sub_states(curr_state_id.state()) else {
                    error!(
                        "{}",
                        StateMachineError::SubStateNotFound {
                            state_tree: curr_state_id.tree(),
                            state: curr_state_id.state()
                        }
                    );
                    return;
                };
                let Ok(strategy) = query_state
                    .get(curr_state_id.state())
                    .map(|state| state.strategy)
                else {
                    warn!(
                        "{}",
                        StateMachineError::HsmStateMissing(curr_state_id.state())
                    );
                    return;
                };
                if sub_states.contains(&enter_state_id) {
                    commands.queue(handle_on_enter_state_command(
                        state_machine_id,
                        curr_state_id,
                        enter_state_id,
                        strategy,
                    ));
                }
            }
            super::event::HsmTriggerType::SubTransition(enter_state_id) => {
                let Ok(strategy) = query_state
                    .get(curr_state_id.state())
                    .map(|state| state.strategy)
                else {
                    warn!(
                        "{}",
                        StateMachineError::HsmStateMissing(curr_state_id.state())
                    );
                    return;
                };
                commands.queue(handle_on_enter_state_command(
                    state_machine_id,
                    curr_state_id,
                    enter_state_id,
                    strategy,
                ));
            }
        };
    }
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
                .finish()
        }
        #[cfg(not(feature = "history"))]
        {
            f.debug_struct("HsmStateMachine")
                .field("transition_queue", &self.transition_queue)
                .field("curr_state", &self.curr_state)
                .field("init_state", &self.init_state)
                .finish()
        }
    }
}

/// # 状态转换\State Transition
/// * 状态转换的枚举，包含下一个状态的ID和OnState
/// - The enum of state transitions, including the ID of the next state and OnState
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// 下一个状态的ID和OnState
    ///
    /// The ID of the next state and OnState
    Next((HsmStateId, StateLifecycle)),
    /// 无下一个状态
    ///
    /// No next state
    None,
}

/// # 状态变化检测组件\State Change Detection Component
/// * 用于检测状态变化，实时更新状态机的状态
/// - Used for detecting state changes and updating the state machine's state in real time
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(immutable, storage = "SparseSet", on_insert = Self::on_insert)]
pub enum StateLifecycle {
    /// 进入状态\Enter State
    #[default]
    Enter,
    /// 更新状态\Update State
    Update,
    /// 退出状态\Exit State
    Exit,
}

impl StateLifecycle {
    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let state_machine_id = hook_context.entity;
        #[cfg(all(feature = "hybrid", feature = "history"))]
        let (mut entities, mut commands) = world.entities_and_commands();
        #[cfg(all(feature = "hybrid", feature = "history"))]
        let Ok(mut state_machine_mut) = entities.get_mut(state_machine_id) else {
            warn!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return;
        };

        #[cfg(not(all(feature = "hybrid", feature = "history")))]
        let (entities, mut commands) = world.entities_and_commands();
        #[cfg(not(all(feature = "hybrid", feature = "history")))]
        let Ok(state_machine_mut) = entities.get(state_machine_id) else {
            warn!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return;
        };

        let Some(hsm_state) = state_machine_mut.get::<StateLifecycle>().copied() else {
            warn!(
                "{}",
                StateMachineError::StateLifecycleMissing(state_machine_id)
            );
            return;
        };

        #[cfg(not(all(feature = "hybrid", feature = "history")))]
        if state_machine_mut.contains::<crate::fsm::state_machine::FsmStateMachine>() {
            commands
                .entity(state_machine_id)
                .remove::<crate::fsm::state_machine::FsmStateMachine>();
        }
        #[cfg(all(feature = "hybrid", feature = "history"))]
        let fsm_history = state_machine_mut
            .get_mut::<crate::fsm::state_machine::FsmStateMachine>()
            .map(|mut h| {
                commands
                    .entity(state_machine_id)
                    .remove::<crate::fsm::state_machine::FsmStateMachine>();
                h.history.take()
            });
        #[cfg(all(feature = "hybrid", feature = "history"))]
        let Some(mut state_machine) = state_machine_mut.get_mut::<HsmStateMachine>() else {
            warn!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return;
        };
        #[cfg(all(feature = "hybrid", feature = "history"))]
        if let Some(fsm_history) = fsm_history {
            state_machine
                .history
                .set_last_state_fsm_history(fsm_history);
        }
        #[cfg(not(all(feature = "hybrid", feature = "history")))]
        let Some(state_machine) = state_machine_mut.get::<HsmStateMachine>() else {
            warn!(
                "{}",
                StateMachineError::HsmStateMachineMissing(state_machine_id)
            );
            return;
        };
        let curr_state_id = state_machine.curr_state_id();
        #[cfg(feature = "history")]
        state_machine.push_history(HistoricalNode::new(curr_state_id, hsm_state.into()));

        #[cfg(feature = "hybrid")]
        match entities
            .get(curr_state_id.state())
            .ok()
            .and_then(|entity_ref| entity_ref.get::<HsmState>().cloned())
        {
            Some(HsmState {
                fsm_config: Some(fsm_config),
                ..
            }) => {
                if matches!(hsm_state, StateLifecycle::Update) {
                    commands.entity(state_machine_id).insert(
                        crate::fsm::state_machine::FsmStateMachine::new(
                            fsm_config.graph_id,
                            fsm_config.init_state,
                            #[cfg(feature = "history")]
                            fsm_config.history_size,
                        ),
                    );
                }
            }
            None => {
                warn!(
                    "{}",
                    StateMachineError::HsmStateMissing(curr_state_id.state())
                );
                return;
            }
            _ => {}
        };

        let state_context = StateActionContext::new(
            match world.get::<ServiceTarget>(state_machine_id) {
                Some(service_target) => service_target.0,
                None => state_machine_id,
            },
            state_machine_id,
            curr_state_id.state(),
        );
        match hsm_state {
            StateLifecycle::Enter => {
                #[cfg(feature = "state_data")]
                StateData::clone_components(
                    &mut world,
                    curr_state_id.state(),
                    state_context.service_target,
                );

                // 运行进入系统
                'on_enter: {
                    let Some(on_enter_system) = world.get::<OnEnterSystem>(curr_state_id.state())
                    else {
                        break 'on_enter;
                    };
                    let action_registry = world.resource::<StateActionRegistry>();

                    let Some(action_system_id) =
                        action_registry.get(on_enter_system.as_str()).copied()
                    else {
                        warn!(
                            "{}",
                            StateMachineError::SystemNotFound {
                                system_name: on_enter_system.to_string(),
                                state: curr_state_id.state()
                            }
                        );
                        break 'on_enter;
                    };
                    if let Err(e) = unsafe { world.as_unsafe_world_cell().world_mut() }
                        .run_system_with(action_system_id, state_context)
                    {
                        let Some(on_enter_system) =
                            world.get::<OnEnterSystem>(curr_state_id.state())
                        else {
                            break 'on_enter;
                        };
                        error!(
                            "{}",
                            StateMachineError::SystemRunFailed {
                                system_name: on_enter_system.to_string(),
                                state: curr_state_id.state(),
                                source: e.into()
                            }
                        );
                    }
                }
                unsafe { world.as_unsafe_world_cell().world_mut() }
                    .entity_mut(state_machine_id)
                    .insert(StateLifecycle::Update);
            }
            StateLifecycle::Update => {
                // 添加过渡条件检查系统
                let curr_state = world.entity(curr_state_id.state());
                if curr_state.contains::<OnEnterSystem>() || curr_state.contains::<OnExitSystem>() {
                    let mut check_on_transition_states =
                        world.resource_mut::<CheckOnTransitionStates>();
                    check_on_transition_states.insert(state_machine_id);
                }

                if !world
                    .entity(curr_state_id.state())
                    .contains::<OnUpdateSystem>()
                {
                    return;
                }

                // 运行更新系统
                StateActionBuffer::buffer_scope(
                    world.as_unsafe_world_cell(),
                    curr_state_id.state(),
                    move |_world, buff| {
                        buff.add(state_context);
                    },
                );
            }
            StateLifecycle::Exit => {
                // 过滤条件
                StateActionBuffer::buffer_scope(
                    world.as_unsafe_world_cell(),
                    curr_state_id.state(),
                    move |_world, buff| {
                        buff.remove_interceptor(state_context);

                        buff.add_filter(state_context);
                    },
                );

                #[cfg(feature = "state_data")]
                StateData::remove_components(
                    &mut world,
                    curr_state_id.state(),
                    state_context.service_target,
                );

                // 运行退出系统
                'on_exit: {
                    let Some(on_exit_system) = world.get::<OnExitSystem>(curr_state_id.state())
                    else {
                        break 'on_exit;
                    };
                    let action_registry = world.resource::<StateActionRegistry>();
                    let Some(action_system_id) =
                        action_registry.get(on_exit_system.as_str()).copied()
                    else {
                        warn!(
                            "{}",
                            StateMachineError::SystemNotFound {
                                system_name: on_exit_system.to_string(),
                                state: curr_state_id.state()
                            }
                        );
                        break 'on_exit;
                    };
                    if let Err(e) = unsafe { world.as_unsafe_world_cell().world_mut() }
                        .run_system_with(action_system_id, state_context)
                    {
                        let Some(on_exit_system) = world.get::<OnExitSystem>(curr_state_id.state())
                        else {
                            break 'on_exit;
                        };
                        error!(
                            "{}",
                            StateMachineError::SystemRunFailed {
                                system_name: on_exit_system.to_string(),
                                state: curr_state_id.state(),
                                source: e.into()
                            }
                        );
                    };
                }

                world.commands().queue(move |world: &mut World| {
                    let Some(mut state_machine) =
                        world.get_mut::<HsmStateMachine>(state_machine_id)
                    else {
                        warn!(
                            "{}",
                            StateMachineError::HsmStateMachineMissing(state_machine_id)
                        );
                        return;
                    };
                    let Transition::Next((curr_state, on_state)) = state_machine.pop_next_state()
                    else {
                        world.entity_mut(state_machine_id).insert(Terminated);
                        return;
                    };
                    state_machine.set_curr_state(curr_state);
                    world.entity_mut(state_machine_id).insert(on_state);
                });
            }
        };
    }
}
