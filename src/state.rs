use std::{collections::VecDeque, fmt::Debug, hash::Hash};

use bevy::{
    ecs::{lifecycle::HookContext, schedule::ScheduleLabel, world::DeferredWorld},
    prelude::*,
};

use crate::{
    history::{HistoricalNode, StateHistory, StateHistoryIterator},
    hook_system::{HsmOnStateDisposableSystems, HsmStateContext},
    on_transition::CheckOnTransitionStates,
    prelude::{
        ExitTransitionBehavior, HsmActionSystemBuffer, ServiceTarget, StateTransitionStrategy,
    },
    state_tree::TreeStateId,
};

/// 状态机\State Machines
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
/// let traversal = TraversalStrategy::default();
/// let tree_id = commands.spawn(StateTree::new(id, traversal)).id();
/// let state_machine = StateMachine::new(10, TreeStateId::new(tree_id, id));
/// # }
/// ```
#[derive(Component, Clone, PartialEq, Eq)]
pub struct StateMachine {
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

impl StateMachine {
    pub fn new(history_len: usize, curr_state: TreeStateId) -> Self {
        let history = StateHistory::new(history_len);
        Self {
            history,
            curr_state,
            next_states: VecDeque::new(),
            init_state: curr_state,
        }
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

impl Debug for StateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateMachine")
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

/// # 终止状态机标记组件\Termination Marker Component
/// 表示状态机已经终止，不再处理状态转换
///
/// Indicates that the state machine has terminated and no longer processes state transitions
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(on_remove = Self::on_remove)]
#[require(StationaryStateMachine)]
pub struct Terminated;

impl Terminated {
    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(mut state_machine) = world.get_mut::<StateMachine>(entity) else {
            return;
        };
        state_machine.clear_next_states();
        state_machine.clear_history();

        let init_state = state_machine.init_state;
        state_machine.set_curr_state(init_state);
    }
}

/// # 状态机组件\State Machine Component
/// * 用于静止拥有该组件的状态机
/// - Used for state machines that statically possess this component
/// * 如果存在, 系统不会在运行状态机的状态转换时调用状态的OnEnter、OnExit、OnUpdate系统
/// - If it exists, the OnEnter, OnExit, and OnUpdate systems of the state machine will not be called during the running of the state machine's state transition
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(on_insert = Self::on_insert,on_remove = Self::on_remove)]
pub struct StationaryStateMachine;

impl StationaryStateMachine {
    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(state_machine) = world.get::<StateMachine>(entity) else {
            world.commands().entity(entity).remove::<Self>();
            warn!(
                "StationaryStateMachine component added to non-StateMachine entity<{}>",
                entity
            );
            return;
        };
        // 查看当前状态是否有HsmOnUpdateSystem,则将其添加进延期表中
        let curr_state_id = state_machine.curr_state_id();
        let state_context = HsmStateContext::<Entity>::new(
            match world.get::<ServiceTarget>(entity) {
                Some(service_target) => service_target.0,
                None => entity,
            },
            entity,
            curr_state_id.state(),
        );

        let unsafe_world_cell = world.as_unsafe_world_cell();
        HsmActionSystemBuffer::buffer_scope(
            unsafe_world_cell,
            curr_state_id.state(),
            move |_world, buff| {
                buff.add(state_context);
            },
        );
    }

    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(state_machine) = world.get::<StateMachine>(entity) else {
            return;
        };
        let curr_state_id = state_machine.curr_state_id();
        let state_context = HsmStateContext::<Entity>::new(
            match world.get::<ServiceTarget>(entity) {
                Some(service_target) => service_target.0,
                None => entity,
            },
            entity,
            curr_state_id.state(),
        );

        let unsafe_world_cell = world.as_unsafe_world_cell();
        HsmActionSystemBuffer::buffer_scope(
            unsafe_world_cell,
            curr_state_id.state(),
            move |_world, buff| {
                buff.add(state_context);
            },
        );
    }
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
        let Some(hsm_state) = world.get::<HsmOnState>(state_machine_id).copied() else {
            return;
        };
        let Some(mut state_machine) = world.get_mut::<StateMachine>(state_machine_id) else {
            return;
        };
        let curr_state_id = state_machine.curr_state_id();
        state_machine.push_history(HistoricalNode::new(curr_state_id, hsm_state));

        let state_context = HsmStateContext::<Entity>::new(
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
                    let Some(on_enter_system) =
                        world.get::<HsmOnEnterSystem>(curr_state_id.state())
                    else {
                        break 'on_enter;
                    };
                    let disposable_systems = world.resource::<HsmOnStateDisposableSystems>();

                    let Some(action_system_id) =
                        disposable_systems.get(on_enter_system.as_str()).copied()
                    else {
                        warn!(
                            "状态<{}>在OnEnter系统中的{}不存在.",
                            curr_state_id,
                            on_enter_system.as_str()
                        );
                        break 'on_enter;
                    };
                    if let Err(e) = unsafe { world.as_unsafe_world_cell().world_mut() }
                        .run_system_with(action_system_id, state_context)
                    {
                        warn!("Error running enter system: {:?}", e);
                    }
                }
                unsafe { world.as_unsafe_world_cell().world_mut() }
                    .entity_mut(state_machine_id)
                    .insert(HsmOnState::Update);
            }
            HsmOnState::Update => {
                // 添加过渡条件检查系统
                let curr_state = world.entity(curr_state_id.state());
                if curr_state.contains::<HsmOnEnterSystem>()
                    || curr_state.contains::<HsmOnExitSystem>()
                {
                    let mut check_on_transition_states =
                        world.resource_mut::<CheckOnTransitionStates>();
                    check_on_transition_states.insert(state_machine_id);
                }

                if !world
                    .entity(curr_state_id.state())
                    .contains::<HsmOnUpdateSystem>()
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
                    let Some(on_exit_system) = world.get::<HsmOnExitSystem>(curr_state_id.state())
                    else {
                        break 'on_exit;
                    };
                    let disposable_systems = world.resource::<HsmOnStateDisposableSystems>();
                    let Some(action_system_id) =
                        disposable_systems.get(on_exit_system.as_str()).copied()
                    else {
                        warn!(
                            "状态<{}>在OnEnter系统中的{}不存在.",
                            curr_state_id,
                            on_exit_system.as_str()
                        );
                        break 'on_exit;
                    };
                    if let Err(e) = unsafe { world.as_unsafe_world_cell().world_mut() }
                        .run_system_with(action_system_id, state_context)
                    {
                        error!("Error running exit system: {:?}", e);
                    };
                }

                world.commands().queue(move |world: &mut World| {
                    let Some(mut state_machine) = world.get_mut::<StateMachine>(state_machine_id)
                    else {
                        warn!("StateMachine not found: {}", state_machine_id);
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

/// # 状态组件\State Component
/// * 标记状态的组件，需要绑定[`StateMachine`]所在实体的id
/// - Used to mark a state component, which requires the id of the entity that has the [`StateMachine`] component
#[derive(Component, ScheduleLabel, Hash, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HsmState {
    pub strategy: StateTransitionStrategy,
    pub behavior: ExitTransitionBehavior,
}

impl HsmState {
    pub fn with(strategy: StateTransitionStrategy, behavior: ExitTransitionBehavior) -> Self {
        Self { strategy, behavior }
    }

    #[inline]
    pub fn set_strategy(mut self, strategy: StateTransitionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    #[inline]
    pub fn set_behavior(mut self, behavior: ExitTransitionBehavior) -> Self {
        self.behavior = behavior;
        self
    }
}

/// 进入状态前调用
///
/// Enter state before calling
/// # 示例\Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn foo(mut commands: Commands) {
/// commands.spawn(HsmOnEnterSystem::new("enter"));
/// # }
/// ```
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct HsmOnEnterSystem(String);

impl HsmOnEnterSystem {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// 更新状态时调用
///
/// Update state when calling
/// # 使用方法\Usage
///  由于注册动作系统时，通过[ScheduleLabel]来确定系统调用时间，
///  所以在使用对应[ScheduleLabel]的系统时，需要特定格式。
///
///  When registering an action system, the system call time is determined through [ScheduleLabel],
///  Therefore, when using the system corresponding to [ScheduleLabel], a specific format is required.
/// * 正常格式: `ScheduleLabel` + `:` + `方法名称`
/// - Normal format: `ScheduleLabel` + `:` + `method name`
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn add(contexts:In<Vec<HsmStateContext>>)->Option<Vec<HsmStateContext>>{None}
/// # fn my_fn(){
/// # let mut app = App::new();
///
/// app.add_action_system(Update, "add", add);
///
/// # }
/// # fn foo(mut commands: Commands) {
/// commands.spawn(HsmOnUpdateSystem::new("Update:add"));
/// # }
/// ```
/// * 特殊格式: `ScheduleLabel`
/// - Special format: `ScheduleLabel`
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn my_fn(){
/// # let mut app = App::new();
///
/// app.add_action_system_anchor_point(Update);
///
/// # }
/// # fn foo(mut commands: Commands) {
/// commands.spawn(HsmOnUpdateSystem::new("Update"));
/// # }
/// ```
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct HsmOnUpdateSystem(String);

impl HsmOnUpdateSystem {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// 退出状态后调用
///
/// Exit state after calling
/// # 示例\Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn foo(mut commands: Commands) {
/// commands.spawn(HsmOnExitSystem::new("exit"));
/// # }
/// ```
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct HsmOnExitSystem(String);

impl HsmOnExitSystem {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;

    use super::*;

    fn hello_world(mut local: Local<usize>) -> bool {
        *local += 1;
        println!("hello world {}", *local);
        *local % 2 == 0
    }

    #[test]
    fn test_hsm_state() {
        let mut world = World::new();
        let mut commands = world.commands();
        let id = commands.register_system(hello_world);
        let mut command_queue = CommandQueue::default();
        let mut command = Commands::new(&mut command_queue, &mut world);
        for _ in 0..10 {
            command.queue(move |world: &mut World| {
                let Ok(res) = world.run_system(id) else {
                    return;
                };
                if res {
                    println!("这是偶数");
                }
            });
        }

        command_queue.apply(&mut world);
    }
}
