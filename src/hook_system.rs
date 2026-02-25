use std::hash::Hash;

use bevy::{
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};

use crate::context::DisposableSystemId;

/// 注册一次性用于运行[`OnEnterSystem`] [`OnExitSystem`]的系统
///
/// Register a one-time system for running [OnEnterSystem] [OnExitSystem]
/// # 示例\Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn on_enter(entity:In<OnStateContext>) {
/// #     println!("进入系统");
/// # }
/// # fn foo(mut commands:Commands, mut on_enter_named_state_systems: ResMut<NamedStateSystems>) {
/// let system_id = commands.register_system(on_enter);
/// on_enter_named_state_systems.insert("on_enter", system_id);
/// # }
/// ```
///
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct NamedStateSystems(pub(super) HashMap<String, DisposableSystemId>);

impl NamedStateSystems {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// 注册系统
    ///
    /// Register system
    /// # 示例\Example
    /// ```rust
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn on_enter(entity:In<HsmStateContext>) {
    /// #     println!("进入系统");
    /// # }
    /// fn foo(mut commands:Commands, mut named_state_systems: ResMut<NamedStateSystems>) {
    ///     let system_id = commands.register_system(on_enter);
    ///     named_state_systems.insert("on_enter", system_id);
    /// }
    /// ```
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        system_id: DisposableSystemId,
    ) -> Option<DisposableSystemId> {
        self.0.insert(name.into(), system_id)
    }

    /// 移除系统
    ///
    /// Remove system
    /// # 示例\Example
    /// ```rust
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// fn foo(mut commands:Commands, mut on_enter_named_state_systems: ResMut<NamedStateSystems>) {
    ///     on_enter_named_state_systems.remove("on_enter");
    /// }
    /// ```
    pub fn remove<Q>(&mut self, name: &Q) -> Option<DisposableSystemId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.remove(name)
    }

    /// 获取系统
    pub fn get<Q>(&self, name: &Q) -> Option<&DisposableSystemId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get(name)
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
/// commands.spawn(OnEnterSystem::new("enter"));
/// # }
/// ```
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct OnEnterSystem(String);

impl OnEnterSystem {
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
/// commands.spawn(OnUpdateSystem::new("Update:add"));
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
/// commands.spawn(OnUpdateSystem::new("Update"));
/// # }
/// ```
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct OnUpdateSystem(String);

impl OnUpdateSystem {
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
/// commands.spawn(OnExitSystem::new("exit"));
/// # }
/// ```
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct OnExitSystem(String);

impl OnExitSystem {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// 状态机服务目标
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[relationship(relationship_target = StateMachineForest)]
pub struct ServiceTarget(pub Entity);

/// 状态机森林
#[derive(Component, Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deref)]
#[relationship_target(relationship = ServiceTarget)]
pub struct StateMachineForest(Vec<Entity>);
