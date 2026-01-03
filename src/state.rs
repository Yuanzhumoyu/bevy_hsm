use std::{fmt::Debug, hash::Hash};

use bevy::{
    ecs::{lifecycle::HookContext, schedule::ScheduleLabel, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
};

use crate::{
    history::StateHistory,
    hook_system::{HsmOnEnterDisposableSystems, HsmOnExitDisposableSystems, HsmStateContext},
    on_transition::CheckOnTransitionStates,
    prelude::StateTransitionStrategy,
    priority::StatePriority,
    system_state::HsmActionSystems,
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
    pub states: HashMap<String, Entity>,
    /// 历史记录
    ///
    /// 记录实体的状态转换历史，用于回溯状态
    /// 最后一个状态始终为最新的状态
    pub history: StateHistory,
    /// 下一个状态
    ///
    /// 实体下一个要转换到的状态
    pub next_state: Option<String>,
}

impl StateMachines {
    pub fn new(
        states: HashMap<String, Entity>,
        history_len: usize,
        current_state: impl Into<String>,
    ) -> Self {
        let current_state = current_state.into();
        let mut history = StateHistory::new(history_len);
        history.push(current_state);
        Self {
            states,
            history,
            next_state: None,
        }
    }

    pub fn curr_state_id(&self) -> Option<Entity> {
        self.history
            .get_current()
            .and_then(|s| self.states.get(s))
            .copied()
    }

    pub fn next_state_id(&self) -> Option<Entity> {
        self.next_state
            .as_ref()
            .and_then(|s| self.states.get(s).copied())
    }

    /// 获取当前状态名称
    pub fn current_state_name(&self) -> Option<&str> {
        self.history.get_current()
    }

    pub fn update(&mut self) {
        let Some(curr_state) = self.next_state.take() else {
            return;
        };
        self.history.push(curr_state);
    }

    /// 获取上一个状态的ID
    pub fn prev_state_id(&self) -> Option<Entity> {
        self.history
            .get_previous()
            .and_then(|s| self.states.get(s))
            .copied()
    }

    /// 检查是否有上一个状态
    pub fn has_prev_state(&self) -> bool {
        self.prev_state_id().is_some()
    }

    /// 返回到上一个状态
    pub fn back_to_prev(&mut self) -> bool {
        if let Some(prev_state) = self.history.get_previous() {
            if self.states.contains_key(prev_state) {
                self.next_state = Some(prev_state.to_string());
                true
            } else {
                warn!("Previous state '{}' not found in states map", prev_state);
                false
            }
        } else {
            false
        }
    }

    /// 获取状态历史记录
    pub fn get_history(&self) -> Vec<&str> {
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
                let mut check_on_transition_states =
                    world.resource_mut::<CheckOnTransitionStates>();
                check_on_transition_states.insert(main_body_id);

                // 运行更新系统
                let Some(on_update_system) = world.get::<HsmOnUpdateSystem>(curr_state_id) else {
                    return;
                };
                let action_systems = world.resource::<HsmActionSystems>();
                let Some(get_buffer_scope) = action_systems.get(on_update_system.as_str()) else {
                    warn!("未找到系统: {}", on_update_system.0);
                    return;
                };
                let state_context = HsmStateContext::new(main_body_id, curr_state_id);

                (get_buffer_scope)(
                    unsafe { world.as_unsafe_world_cell().world_mut() },
                    Box::new(move |_world, buff| {
                        buff.add(state_context);
                    }),
                );
            }
            HsmOnState::Exit => {
                let Some(curr_state_id) = state_machines.curr_state_id() else {
                    warn!("Current state not found in states map",);
                    return;
                };
                let state_context = HsmStateContext::new(main_body_id, curr_state_id);

                // 过滤条件
                if let Some(on_update_system) = world.get::<HsmOnUpdateSystem>(curr_state_id) {
                    let action_systems = world.resource::<HsmActionSystems>();
                    let Some(get_buffer_scope) = action_systems.get(on_update_system.as_str())
                    else {
                        warn!("未找到系统: {}", on_update_system.0);
                        return;
                    };

                    (get_buffer_scope)(
                        unsafe { world.as_unsafe_world_cell().world_mut() },
                        Box::new(move |_world, buff| {
                            buff.add_filter(state_context);
                        }),
                    );
                }

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
                    if let Some(mut state_machines) = world.get_mut::<StateMachines>(main_body_id) {
                        state_machines.update();
                    }
                    let transition_strategy: HsmOnState = world
                        .get::<StateTransitionStrategy>(curr_state_id)
                        .copied()
                        .unwrap()
                        .into();
                    world.entity_mut(main_body_id).insert(transition_strategy);
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
        let state_machines_id = world
            .get::<HsmState>(hook_context.entity)
            .map(|s| s.main_body)
            .unwrap();
        let Some(name) = world
            .get::<Name>(hook_context.entity)
            .map(ToString::to_string)
        else {
            warn!(
                "State entity<{}> does not have Name component",
                hook_context.entity
            );
            return;
        };
        let Some(mut state_machines) = world.get_mut::<StateMachines>(state_machines_id) else {
            warn!(
                "Main body entity<{:?}> does not have StateMachines component",
                state_machines_id
            );
            return;
        };
        match state_machines.states.get(&name) {
            Some(old_entity) => {
                warn!("状态<{}:{}> 已存在", name, old_entity);
            }
            None => {
                state_machines.states.insert(name, hook_context.entity);
            }
        }
    }

    fn on_remove(mut world: DeferredWorld, hook_context: HookContext) {
        let state_machines_id = world
            .get::<HsmState>(hook_context.entity)
            .map(|s| s.main_body)
            .unwrap();
        let Some(name) = world
            .get::<Name>(hook_context.entity)
            .map(ToString::to_string)
        else {
            warn!(
                "State entity<{}> does not have Name component",
                hook_context.entity
            );
            return;
        };
        let Some(mut state_machines) = world.get_mut::<StateMachines>(state_machines_id) else {
            warn!(
                "Main body entity<{:?}> does not have StateMachines component",
                state_machines_id
            );
            return;
        };
        state_machines.states.remove(&name);
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
