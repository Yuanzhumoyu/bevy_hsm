use std::{collections::VecDeque, fmt::Debug, hash::Hash};

use bevy::{
    ecs::{lifecycle::HookContext, schedule::ScheduleLabel, world::DeferredWorld},
    prelude::*,
};

use crate::{
    history::StateHistory,
    hook_system::{HsmOnEnterDisposableSystems, HsmOnExitDisposableSystems, HsmStateContext},
    on_transition::CheckOnTransitionStates,
    prelude::{HsmActionSystemBuffer, StateTransitionStrategy},
    priority::StatePriority,
};

/// 状态机
///
/// 管理实体的状态转换，包括当前状态、下一状态以及状态映射表
///
/// # 示例
///
/// ```rust
/// # use bevy_hsm::StateMachines;
/// # use bevy::platform::collections::HashMap;
///
/// let mut state_machines = StateMachines::new(HashMap::new(), "初始状态");
/// ```
#[derive(Component, Clone, PartialEq, Eq)]
pub struct StateMachines {
    /// 状态映射表
    states: Vec<Entity>,
    /// 历史记录
    ///
    /// 记录实体的状态转换历史，用于回溯状态
    /// 最后一个状态始终为最新的状态
    pub history: StateHistory,
    /// 下一个状态
    ///
    /// 实体下一个要转换到的状态
    next_state: VecDeque<(Entity, HsmOnState)>,
}

impl StateMachines {
    pub fn new(states: Vec<Entity>, history_len: usize, current_state: Entity) -> Self {
        let mut history = StateHistory::new(history_len);
        history.push(current_state);
        Self {
            states,
            history,
            next_state: VecDeque::new(),
        }
    }

    pub fn states(&self) -> &[Entity] {
        &self.states
    }

    pub fn curr_state_id(&self) -> Option<Entity> {
        self.history.get_current()
    }

    pub fn next_state_id(&self) -> Option<Entity> {
        self.next_state.front().map(|(id, _)| *id)
    }

    pub fn push_history(&mut self, state: Entity) {
        self.history.push(state);
    }

    /// 添加下一个状态
    pub fn push_next_state(&mut self, state: Entity, on_state: HsmOnState) {
        self.next_state.push_front((state, on_state));
    }

    pub fn get_next_state(&self) -> Option<Entity> {
        self.next_state.front().map(|(id, _)| *id)
    }

    pub fn get_next_state_on_state(&self) -> Option<HsmOnState> {
        self.next_state.front().map(|(_, on_state)| *on_state)
    }

    pub fn pop_next_state(&mut self) -> Option<(Entity, HsmOnState)> {
        self.next_state.pop_front()
    }

    pub fn update(&mut self) {
        let Some((curr_state, _)) = self.next_state.pop_front() else {
            return;
        };
        self.history.push(curr_state);
    }

    /// 获取上一个状态的ID
    pub fn prev_state_id(&self) -> Option<Entity> {
        self.history.get_previous()
    }

    /// 检查是否有上一个状态
    pub fn has_prev_state(&self) -> bool {
        self.prev_state_id().is_some()
    }

    /// 获取状态历史记录
    pub fn get_history(&self) -> Vec<Entity> {
        self.history.get_history()
    }
}

impl Debug for StateMachines {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateMachines")
            .field("states", &self.states)
            .field("history", &self.history.get_history())
            .field("next_state", &self.next_state)
            .finish()
    }
}

/// # 状态机组件
/// - 用于静止所在状态机的状态
/// - 如果存在, 系统不会在运行状态机的状态转换
#[derive(Component, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(on_insert = Self::on_insert,on_remove = Self::on_remove)]
pub struct StationaryStateMachines;

impl StationaryStateMachines {
    fn on_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(state_machines) = world.get::<StateMachines>(entity) else {
            world.commands().entity(entity).remove::<Self>();
            warn!(
                "StationaryStateMachines component added to non-StateMachines entity<{}>",
                entity
            );
            return;
        };
        // 查看当前状态是否有HsmOnUpdateSystem,则将其添加进延期表中
        let Some(curr_state_id) = state_machines.curr_state_id() else {
            return;
        };
        let state_context = HsmStateContext::new(entity, curr_state_id);

        let world = unsafe { world.as_unsafe_world_cell().world_mut() };
        HsmActionSystemBuffer::buffer_scope(world, curr_state_id, move |_world, buff| {
            buff.add(state_context);
        });
    }

    fn on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
        let Some(state_machines) = world.get::<StateMachines>(entity) else {
            return;
        };
        let Some(curr_state_id) = state_machines.curr_state_id() else {
            return;
        };
        let state_context = HsmStateContext::new(entity, curr_state_id);

        let world = unsafe { world.as_unsafe_world_cell().world_mut() };
        HsmActionSystemBuffer::buffer_scope(world, curr_state_id, move |_world, buff| {
            buff.add(state_context);
        });
    }
}

/// 用于检测状态变化，实时更新状态
#[derive(Component, ScheduleLabel, Default, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[component(immutable, storage = "SparseSet", on_insert = Self::on_insert)]
pub enum HsmOnState {
    /// 进入
    #[default]
    Enter,
    /// 更新
    Update,
    /// 退出
    Exit,
}

impl HsmOnState {
    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let main_body_id = hook_context.entity;
        let Some(hsm_state) = world.get::<HsmOnState>(main_body_id).copied() else {
            return;
        };
        let Some(mut state_machines) = world.get_mut::<StateMachines>(main_body_id) else {
            return;
        };
        match hsm_state {
            HsmOnState::Enter => {
                state_machines.update();
                let Some(curr_state_id) = state_machines.curr_state_id() else {
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
                let state_context = HsmStateContext::new(main_body_id, curr_state_id);
                world.commands().queue(move |world: &mut World| {
                    if let Err(e) = world.run_system_with(action_system_id, state_context) {
                        warn!("Error running enter system: {:?}", e);
                    }
                    world.entity_mut(main_body_id).insert(HsmOnState::Update);
                });
            }
            HsmOnState::Update => {
                let Some(curr_state_id) = state_machines.curr_state_id() else {
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
                    check_on_transition_states.insert(main_body_id);
                }

                // 运行更新系统
                let state_context = HsmStateContext::new(main_body_id, curr_state_id);

                let world = unsafe { world.as_unsafe_world_cell().world_mut() };
                HsmActionSystemBuffer::buffer_scope(world, curr_state_id, move |_world, buff| {
                    buff.add(state_context);
                });
            }
            HsmOnState::Exit => {
                let Some(curr_state_id) = state_machines.curr_state_id() else {
                    warn!("Current state not found in states map",);
                    return;
                };
                let state_context = HsmStateContext::new(main_body_id, curr_state_id);

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
                    let Some(mut state_machines) = world.get_mut::<StateMachines>(main_body_id)
                    else {
                        warn!("StateMachines not found: {}", main_body_id);
                        return;
                    };
                    let Some((curr_state, on_state)) = state_machines.pop_next_state() else {
                        world.entity_mut(main_body_id).insert(HsmOnState::Update);
                        return;
                    };
                    state_machines.push_history(curr_state);
                    world.entity_mut(main_body_id).insert(on_state);
                });
            }
        };
    }
}

/// 标记状态的组件，需要绑定[`StateMachines`]所在实体的id
#[derive(Component, ScheduleLabel, Hash, Debug, Clone, PartialEq, Eq)]
#[component(immutable, on_insert = Self::on_insert, on_remove = Self::on_remove)]
#[require(StatePriority, StateTransitionStrategy)]
pub struct HsmState {
    pub main_body: Entity,
}

impl HsmState {
    pub fn new(main_body: Entity) -> Self {
        Self { main_body }
    }

    fn on_insert(mut world: DeferredWorld, hook_context: HookContext) {
        let state_id = hook_context.entity;
        let state_machines_id = world
            .get::<HsmState>(state_id)
            .map(|s| s.main_body)
            .unwrap();
        let Some(mut state_machines) = world.get_mut::<StateMachines>(state_machines_id) else {
            warn!(
                "Main body entity<{:?}> does not have StateMachines component",
                state_machines_id
            );
            return;
        };

        match state_machines.states.binary_search(&state_id) {
            Ok(old_index) => {
                warn!("状态<{}> 已存在", state_machines.states[old_index]);
            }
            Err(index) => {
                state_machines.states.insert(index, state_id);
            }
        }
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let state_id = hook_context.entity;
        let state_machines_id = world
            .get::<HsmState>(state_id)
            .map(|s| s.main_body)
            .unwrap();
        let Some(mut state_machines) = world.get_mut::<StateMachines>(state_machines_id) else {
            warn!(
                "Main body entity<{:?}> does not have StateMachines component",
                state_machines_id
            );
            return;
        };

        match state_machines.states.binary_search(&state_id) {
            Ok(index) => {
                state_machines.states.remove(index);
            }
            Err(_) => {
                warn!(
                    "State<{:?}> does not exist in StateMachines<{:?}>",
                    state_id, state_machines_id
                );
            }
        }
    }
}

/// 进入状态前调用
/// # 示例
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
/// # 使用方法
///  由于注册动作系统时，通过[`ScheduleLabel`]来确定系统调用时间，
///  所以在使用对应[`ScheduleLabel`]的系统时，需要特定格式。
/// - 正常格式: `ScheduleLabel` + `:` + `方法名称`
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
/// - 特殊格式: `ScheduleLabel`
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
/// # 示例
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
