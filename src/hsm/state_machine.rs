use std::{collections::VecDeque, fmt::Debug};

use bevy::{
    ecs::{lifecycle::HookContext, schedule::ScheduleLabel, world::DeferredWorld},
    prelude::*,
};

use crate::{
    context::OnStateContext,
    error::HsmError,
    hook_system::*,
    hsm::{HsmState, history::*, on_transition::CheckOnTransitionStates, state_tree::TreeStateId},
    prelude::{HsmActionSystemBuffer, ServiceTarget},
    state_machine_component::Terminated,
};

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
/// let state_machine = HsmStateMachine::new(10, TreeStateId::new(tree_id, id));
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
    pub history: StateHistory,
    /// 下一个状态
    ///
    /// Next state
    ///
    /// 实体下一个要转换到的状态
    ///
    /// Next state to transition to for the entity
    next_states: VecDeque<NextState>,
    curr_state: TreeStateId,
    /// 初始状态
    ///
    /// Initial state
    init_state: TreeStateId,
}

impl HsmStateMachine {
    pub fn new(history_len: usize, curr_state: TreeStateId) -> Self {
        let history = StateHistory::new(history_len);
        Self {
            history,
            curr_state,
            next_states: VecDeque::new(),
            init_state: curr_state,
        }
    }

    pub const fn init_state(&self) -> TreeStateId {
        self.init_state
    }

    /// 获取当前状态的ID
    ///
    /// Get the ID of the current state
    pub fn curr_state_id(&self) -> TreeStateId {
        self.curr_state
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn next_state_id(&self) -> Option<&NextState> {
        self.next_states.front()
    }

    /// 设置初始状态
    ///
    /// Set the initial state
    pub fn set_init_state(&mut self, state: TreeStateId) {
        self.init_state = state;
    }

    /// 设置当前状态
    ///
    /// Set the current state
    pub fn set_curr_state(&mut self, state: TreeStateId) {
        self.curr_state = state;
    }

    /// 添加历史记录
    ///
    /// Add history record
    fn push_history(&mut self, node: HistoricalNode) {
        self.history.push(node);
    }

    /// 添加下一个状态
    ///
    /// Add next state
    pub fn push_next_state(&mut self, next_state: NextState) {
        self.next_states.push_front(next_state);
    }

    /// 批量添加下一个状态
    ///
    /// Add multiple next states
    pub fn push_next_states<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = NextState>,
    {
        self.next_states.extend(iter);
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn get_next_state(&self) -> Option<TreeStateId> {
        self.next_states.front().and_then(|next| match next {
            NextState::Next((state_id, _)) => Some(*state_id),
            NextState::None => None,
        })
    }

    /// 获取下一个状态的OnState
    ///
    /// Get the OnState of the next state
    pub fn get_next_state_on_state(&self) -> Option<HsmOnState> {
        self.next_states.front().and_then(|next| match next {
            NextState::Next((_, on_state)) => Some(*on_state),
            NextState::None => None,
        })
    }

    /// 弹出下一个状态
    ///
    /// Pop next state
    pub fn pop_next_state(&mut self) -> NextState {
        self.next_states.pop_front().unwrap_or(NextState::None)
    }

    /// 获取状态历史记录
    ///
    /// Get state history
    pub fn history_iter(&self) -> StateHistoryIterator<'_> {
        self.history.iter()
    }

    /// 获取历史记录长度
    ///
    /// Obtain the length of historical records
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// 检查是否处于指定状态
    ///
    /// Check if in specified state
    pub fn is_in_state(&self, state: TreeStateId) -> bool {
        self.curr_state_id() == state
    }

    /// 清空下一个状态队列
    ///
    /// Clear the next state queue
    pub fn clear_next_states(&mut self) {
        self.next_states.clear();
    }

    /// 清空状态历史队列
    ///
    /// Clear the state history queue
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 检查是否正在转换状态
    ///
    /// Check if the state is transitioning
    pub fn is_transitioning(&self) -> bool {
        self.next_states
            .front()
            .is_some_and(|n| *n != NextState::None)
    }
}

impl Debug for HsmStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HsmStateMachine")
            .field("history", &self.history.iter().collect::<Vec<_>>())
            .field("next_states", &self.next_states)
            .finish()
    }
}

/// 下一个状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextState {
    /// 下一个状态的ID和OnState
    ///
    /// The ID of the next state and OnState
    Next((TreeStateId, HsmOnState)),
    /// 无下一个状态
    ///
    /// No next state
    None,
}

/// # 状态变化检测组件\State Change Detection Component
/// * 用于检测状态变化，实时更新状态机的状态
/// - Used for detecting state changes and updating the state machine's state in real time
#[derive(Component, ScheduleLabel, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(immutable, storage = "SparseSet", on_insert = Self::on_insert)]
pub enum HsmOnState {
    /// 进入状态\Enter State
    #[default]
    Enter,
    /// 更新状态\Update State
    Update,
    /// 退出状态\Exit State
    Exit,
}

impl HsmOnState {
    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let state_machine_id = hook_context.entity;
        let (mut entities, mut commands) = world.entities_and_commands();
        let Ok(mut state_machine_mut) = entities.get_mut(state_machine_id) else {
            warn!("{}", HsmError::StateMachineMissing(state_machine_id));
            return;
        };

        let Some(hsm_state) = state_machine_mut.get::<HsmOnState>().copied() else {
            warn!("{}", HsmError::HsmOnStateMissing(state_machine_id));
            return;
        };

        let fsm_history = state_machine_mut
            .get_mut::<crate::fsm::state_machine::FsmStateMachine>()
            .map(|mut h| {
                commands
                    .entity(state_machine_id)
                    .remove::<crate::fsm::state_machine::FsmStateMachine>();
                h.history.take()
            });

        let Some(mut state_machine) = state_machine_mut.get_mut::<HsmStateMachine>() else {
            warn!("{}", HsmError::StateMachineMissing(state_machine_id));
            return;
        };
        if let Some(fsm_history) = fsm_history {
            state_machine
                .history
                .set_last_state_fsm_history(fsm_history);
        }

        let curr_state_id = state_machine.curr_state_id();
        state_machine.push_history(HistoricalNode::new(curr_state_id, hsm_state.into()));

        match entities
            .get(curr_state_id.state())
            .ok()
            .and_then(|entity_ref| entity_ref.get::<HsmState>().cloned())
        {
            Some(HsmState {
                fsm_config: Some(fsm_config),
                ..
            }) => {
                if matches!(hsm_state, HsmOnState::Update) {
                    commands.entity(state_machine_id).insert(
                        crate::fsm::state_machine::FsmStateMachine::new(
                            fsm_config.graph_id,
                            fsm_config.init_state,
                            fsm_config.max_history_size,
                        ),
                    );
                }
            }
            None => {
                warn!("{}", HsmError::HsmStateMissing(curr_state_id.state()));
                return;
            }
            _ => {}
        };

        let state_context = OnStateContext::new(
            match world.get::<ServiceTarget>(state_machine_id) {
                Some(service_target) => service_target.0,
                None => state_machine_id,
            },
            state_machine_id,
            curr_state_id.state(),
        );
        match hsm_state {
            HsmOnState::Enter => {
                // 运行进入系统
                'on_enter: {
                    let Some(on_enter_system) = world.get::<OnEnterSystem>(curr_state_id.state())
                    else {
                        break 'on_enter;
                    };
                    let named_state_systems = world.resource::<NamedStateSystems>();

                    let Some(action_system_id) =
                        named_state_systems.get(on_enter_system.as_str()).copied()
                    else {
                        warn!(
                            "{}",
                            HsmError::SystemNotFound {
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
                            HsmError::SystemRunFailed {
                                system_name: on_enter_system.to_string(),
                                state: curr_state_id.state(),
                                source: e.into()
                            }
                        );
                    }
                }
                unsafe { world.as_unsafe_world_cell().world_mut() }
                    .entity_mut(state_machine_id)
                    .insert(HsmOnState::Update);
            }
            HsmOnState::Update => {
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
                HsmActionSystemBuffer::buffer_scope(
                    world.as_unsafe_world_cell(),
                    curr_state_id.state(),
                    move |_world, buff| {
                        buff.add(state_context);
                    },
                );
            }
            HsmOnState::Exit => {
                // 过滤条件
                HsmActionSystemBuffer::buffer_scope(
                    world.as_unsafe_world_cell(),
                    curr_state_id.state(),
                    move |_world, buff| {
                        buff.remove_interceptor(state_context);

                        buff.add_filter(state_context);
                    },
                );
                // 运行退出系统
                'on_exit: {
                    let Some(on_exit_system) = world.get::<OnExitSystem>(curr_state_id.state())
                    else {
                        break 'on_exit;
                    };
                    let named_state_systems = world.resource::<NamedStateSystems>();
                    let Some(action_system_id) =
                        named_state_systems.get(on_exit_system.as_str()).copied()
                    else {
                        warn!(
                            "{}",
                            HsmError::SystemNotFound {
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
                            HsmError::SystemRunFailed {
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
                        warn!("{}", HsmError::StateMachineMissing(state_machine_id));
                        return;
                    };
                    let NextState::Next((curr_state, on_state)) = state_machine.pop_next_state()
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
