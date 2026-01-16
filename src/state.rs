use std::{collections::VecDeque, fmt::Debug, hash::Hash};

use bevy::{
    ecs::{
        error::CommandWithEntity,
        lifecycle::HookContext,
        relationship::{Relationship, RelationshipHookMode},
        schedule::ScheduleLabel,
        world::DeferredWorld,
    },
    prelude::*,
};

use crate::{
    history::StateHistory,
    hook_system::{HsmOnEnterDisposableSystems, HsmOnExitDisposableSystems, HsmStateContext},
    on_transition::CheckOnTransitionStates,
    prelude::{
        ExitTransitionBehavior, HsmActionSystemBuffer, ServiceTarget, StateTransitionStrategy,
    },
    priority::StatePriority,
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
/// let start_state_id = commands.spawn_empty().id();
/// let state_machine = StateMachine::new(10, start_state_id);
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
    next_state: VecDeque<NextState>,
    /// 初始状态
    ///
    /// Initial state
    initial_state: Entity,
}

impl StateMachine {
    pub fn new(history_len: usize, current_state: Entity) -> Self {
        let mut history = StateHistory::new(history_len);
        history.push(current_state);
        Self {
            history,
            next_state: VecDeque::new(),
            initial_state: current_state,
        }
    }

    /// 获取当前状态的ID
    ///
    /// Get the ID of the current state
    pub fn curr_state_id(&self) -> Option<Entity> {
        self.history.get_current()
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn next_state_id(&self) -> Option<&NextState> {
        self.next_state.front()
    }

    /// 设置初始状态
    ///
    /// Set the initial state
    pub fn set_initial_state(&mut self, state: Entity) {
        self.initial_state = state;
    }

    /// 添加历史记录
    ///
    /// Add history record
    pub fn push_history(&mut self, state: Entity) {
        self.history.push(state);
    }

    /// 添加下一个状态
    ///
    /// Add next state
    pub fn push_next_state(&mut self, next_state: NextState) {
        self.next_state.push_front(next_state);
    }

    /// 批量添加下一个状态
    ///
    /// Add multiple next states
    pub fn push_next_states<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = NextState>,
    {
        self.next_state.extend(iter);
    }

    /// 获取下一个状态的ID
    ///
    /// Get the ID of the next state
    pub fn get_next_state(&self) -> Option<Entity> {
        self.next_state.front().and_then(|next| match next {
            NextState::Next((id, _)) => Some(*id),
            NextState::None => None,
        })
    }

    /// 获取下一个状态的OnState
    ///
    /// Get the OnState of the next state
    pub fn get_next_state_on_state(&self) -> Option<HsmOnState> {
        self.next_state.front().and_then(|next| match next {
            NextState::Next((_, on_state)) => Some(*on_state),
            NextState::None => None,
        })
    }

    /// 弹出下一个状态
    ///
    /// Pop next state
    pub fn pop_next_state(&mut self) -> Option<NextState> {
        self.next_state.pop_front()
    }

    /// 更新状态
    ///
    /// Update state
    pub fn update(&mut self) {
        let Some(NextState::Next((curr_state, _))) = self.next_state.pop_front() else {
            return;
        };
        self.history.push(curr_state);
    }

    /// 获取上一个状态的ID
    ///
    /// Get the ID of the previous state
    pub fn prev_state_id(&self) -> Option<Entity> {
        self.history.get_previous()
    }

    /// 检查是否有上一个状态
    ///
    /// Check if there is a previous state
    pub fn has_prev_state(&self) -> bool {
        self.prev_state_id().is_some()
    }

    /// 获取状态历史记录
    ///
    /// Get state history
    pub fn get_history(&self) -> Vec<Entity> {
        self.history.get_history()
    }
}

impl Debug for StateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateMachine")
            .field("history", &self.history.get_history())
            .field("next_state", &self.next_state)
            .finish()
    }
}

/// 下一个状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextState {
    /// 下一个状态的ID和OnState
    ///
    /// The ID of the next state and OnState
    Next((Entity, HsmOnState)),
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
        state_machine.next_state.clear();
        let curr_state = state_machine.initial_state;
        state_machine.history.push(curr_state);
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
        let Some(curr_state_id) = state_machine.curr_state_id() else {
            return;
        };
        let state_context = HsmStateContext::new(
            match world.get::<ServiceTarget>(entity) {
                Some(service_target) => service_target.0,
                None => entity,
            },
            entity,
            curr_state_id,
        );

        let world = unsafe { world.as_unsafe_world_cell().world_mut() };
        HsmActionSystemBuffer::buffer_scope(world, curr_state_id, move |_world, buff| {
            buff.add(state_context);
        });
    }

    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(state_machine) = world.get::<StateMachine>(entity) else {
            return;
        };
        let Some(curr_state_id) = state_machine.curr_state_id() else {
            return;
        };
        let state_context = HsmStateContext::new(
            match world.get::<ServiceTarget>(entity) {
                Some(service_target) => service_target.0,
                None => entity,
            },
            entity,
            curr_state_id,
        );

        let world = unsafe { world.as_unsafe_world_cell().world_mut() };
        HsmActionSystemBuffer::buffer_scope(world, curr_state_id, move |_world, buff| {
            buff.add(state_context);
        });
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
        match hsm_state {
            HsmOnState::Enter => {
                state_machine.update();
                let Some(curr_state_id) = state_machine.curr_state_id() else {
                    warn!("Current state not found in states map",);
                    return;
                };

                // 运行进入系统
                let Some(on_enter_system) = world.get::<HsmOnEnterSystem>(curr_state_id) else {
                    return;
                };
                let disposable_systems = world.resource::<HsmOnEnterDisposableSystems>();
                let Some(action_system_id) =
                    disposable_systems.get(on_enter_system.as_str()).copied()
                else {
                    return;
                };
                let state_context = HsmStateContext::new(
                    match world.get::<ServiceTarget>(state_machine_id) {
                        Some(service_target) => service_target.0,
                        None => state_machine_id,
                    },
                    state_machine_id,
                    curr_state_id,
                );
                world.commands().queue(move |world: &mut World| {
                    if let Err(e) = world.run_system_with(action_system_id, state_context) {
                        warn!("Error running enter system: {:?}", e);
                    }
                    world
                        .entity_mut(state_machine_id)
                        .insert(HsmOnState::Update);
                });
            }
            HsmOnState::Update => {
                let Some(curr_state_id) = state_machine.curr_state_id() else {
                    warn!("Current state not found in states map",);
                    return;
                };

                // 添加过渡条件检查系统
                let curr_state = world.entity(curr_state_id);
                if curr_state.contains::<HsmOnEnterSystem>()
                    && curr_state.contains::<HsmOnExitSystem>()
                {
                    let mut check_on_transition_states =
                        world.resource_mut::<CheckOnTransitionStates>();
                    check_on_transition_states.insert(state_machine_id);
                }

                // 运行更新系统
                let state_context = HsmStateContext::new(
                    match world.get::<ServiceTarget>(state_machine_id) {
                        Some(service_target) => service_target.0,
                        None => state_machine_id,
                    },
                    state_machine_id,
                    curr_state_id,
                );

                let world = unsafe { world.as_unsafe_world_cell().world_mut() };
                HsmActionSystemBuffer::buffer_scope(world, curr_state_id, move |_world, buff| {
                    buff.add(state_context);
                });
            }
            HsmOnState::Exit => {
                let Some(curr_state_id) = state_machine.curr_state_id() else {
                    warn!("Current state not found in states map",);
                    return;
                };
                let state_context = HsmStateContext::new(
                    match world.get::<ServiceTarget>(state_machine_id) {
                        Some(service_target) => service_target.0,
                        None => state_machine_id,
                    },
                    state_machine_id,
                    curr_state_id,
                );

                // 过滤条件
                let world = unsafe { world.as_unsafe_world_cell().world_mut() };
                HsmActionSystemBuffer::buffer_scope(world, curr_state_id, move |_world, buff| {
                    buff.remove_interceptor(state_context);

                    buff.add_filter(state_context);
                });

                // 运行退出系统
                let Some(on_exit_system) = world.get::<HsmOnExitSystem>(curr_state_id) else {
                    return;
                };
                let disposable_systems = world.resource::<HsmOnExitDisposableSystems>();
                let Some(action_system_id) =
                    disposable_systems.get(on_exit_system.as_str()).copied()
                else {
                    return;
                };

                world.commands().queue(move |world: &mut World| {
                    if let Err(e) = world.run_system_with(action_system_id, state_context) {
                        warn!("Error running exit system: {:?}", e);
                    }
                    let Some(mut state_machine) = world.get_mut::<StateMachine>(state_machine_id)
                    else {
                        warn!("StateMachine not found: {}", state_machine_id);
                        return;
                    };
                    let Some(next_state) = state_machine.pop_next_state() else {
                        world
                            .entity_mut(state_machine_id)
                            .insert(HsmOnState::Update);
                        return;
                    };
                    let NextState::Next((curr_state, on_state)) = next_state else {
                        world.entity_mut(state_machine_id).insert(Terminated);
                        return;
                    };
                    state_machine.push_history(curr_state);
                    world.entity_mut(state_machine_id).insert(on_state);
                });
            }
        };
    }
}

/// 状态组
#[derive(Component, Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[relationship_target(relationship = HsmState)]
pub struct HsmStateGroup(Vec<Entity>);

impl HsmStateGroup {
    pub fn contains(&self, entity: Entity) -> bool {
        self.0.binary_search(&entity).is_ok()
    }

    pub fn add(&mut self, entity: Entity) {
        let Err(index) = self.0.binary_search(&entity) else {
            return;
        };
        self.0.insert(index, entity);
    }

    pub fn remove(&mut self, entity: Entity) -> Option<usize> {
        let Ok(index) = self.0.binary_search(&entity) else {
            return None;
        };
        self.0.remove(index);
        Some(index)
    }

    pub const fn len(&self) -> usize {
        self.0.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// # 状态组件\State Component
/// * 标记状态的组件，需要绑定[`StateMachine`]所在实体的id
/// - Used to mark a state component, which requires the id of the entity that has the [`StateMachine`] component
#[derive(Component, ScheduleLabel, Hash, Debug, Clone, Copy, PartialEq, Eq)]
#[component(immutable, on_insert = <Self as Relationship>::on_insert, on_replace = <Self as Relationship>::on_replace)]
#[require(StatePriority)]
pub struct HsmState {
    state_group_id: Entity,
    pub strategy: StateTransitionStrategy,
    pub behavior: ExitTransitionBehavior,
}

impl HsmState {
    pub fn with_id(state_group_id: Entity) -> Self {
        Self {
            state_group_id,
            strategy: StateTransitionStrategy::default(),
            behavior: ExitTransitionBehavior::default(),
        }
    }

    pub fn with(
        state_group_id: Entity,
        strategy: StateTransitionStrategy,
        behavior: ExitTransitionBehavior,
    ) -> Self {
        Self {
            state_group_id,
            strategy,
            behavior,
        }
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

impl Relationship for HsmState {
    type RelationshipTarget = HsmStateGroup;

    fn get(&self) -> Entity {
        self.state_group_id
    }

    fn from(entity: Entity) -> Self {
        Self::with_id(entity)
    }

    fn set_risky(&mut self, entity: Entity) {
        self.state_group_id = entity;
    }

    fn on_insert(
        mut world: DeferredWorld,
        HookContext {
            entity,
            caller,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {}
            RelationshipHookMode::Skip => return,
            RelationshipHookMode::RunIfNotLinked => {
                if <Self::RelationshipTarget as RelationshipTarget>::LINKED_SPAWN {
                    return;
                }
            }
        }
        let target_entity = world.entity(entity).get::<Self>().unwrap().get();
        if target_entity == entity {
            warn!(
                "{}The {}({target_entity:?}) relationship on entity {entity:?} points to itself. The invalid {} relationship has been removed.",
                caller
                    .map(|location| format!("{location}: "))
                    .unwrap_or_default(),
                DebugName::type_name::<Self>(),
                DebugName::type_name::<Self>()
            );
            world.commands().entity(entity).remove::<Self>();
            return;
        }

        if let Ok(mut entity_commands) = world.commands().get_entity(target_entity) {
            // Deferring is necessary for batch mode
            entity_commands
                .entry::<Self::RelationshipTarget>()
                .and_modify(move |mut relationship_target| {
                    relationship_target.add(entity);
                })
                .or_insert_with(move || {
                    let mut target = Self::RelationshipTarget::with_capacity(1);
                    target.add(entity);
                    target
                });
        } else {
            warn!(
                "{}The {}({target_entity:?}) relationship on entity {entity:?} relates to an entity that does not exist. The invalid {} relationship has been removed.",
                caller
                    .map(|location| format!("{location}: "))
                    .unwrap_or_default(),
                DebugName::type_name::<Self>(),
                DebugName::type_name::<Self>()
            );
            world.commands().entity(entity).remove::<Self>();
        }
    }

    fn on_replace(
        mut world: DeferredWorld,
        HookContext {
            entity,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {}
            RelationshipHookMode::Skip => return,
            RelationshipHookMode::RunIfNotLinked => {
                if <Self::RelationshipTarget as RelationshipTarget>::LINKED_SPAWN {
                    return;
                }
            }
        }
        let target_entity = world.entity(entity).get::<Self>().unwrap().state_group_id;
        if let Ok(mut target_entity_mut) = world.get_entity_mut(target_entity) {
            if let Some(mut relationship_target) =
                target_entity_mut.get_mut::<Self::RelationshipTarget>()
            {
                relationship_target.remove(entity);
                if relationship_target.is_empty() {
                    let command = |mut entity: EntityWorldMut| {
                        // this "remove" operation must check emptiness because in the event that an identical
                        // relationship is inserted on top, this despawn would result in the removal of that identical
                        // relationship ... not what we want!
                        if entity
                            .get::<Self::RelationshipTarget>()
                            .is_some_and(RelationshipTarget::is_empty)
                        {
                            entity.remove::<Self::RelationshipTarget>();
                        }
                    };

                    world
                        .commands()
                        .queue_silenced(command.with_entity(target_entity));
                }
            }
        }
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
