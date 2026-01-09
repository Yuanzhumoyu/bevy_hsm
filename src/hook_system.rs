use std::hash::Hash;

use bevy::{
    ecs::system::SystemId,
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};

pub type DisposableSystemId = SystemId<In<HsmStateContext>, ()>;

/// 状态上下文
///
/// StateContext
/// # 作用\Purpose
/// * 用于在系统中传递状态上下文
/// - Used to pass state context in systems
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct HsmStateContext {
    /// 主体实体
    ///
    /// Main body entity
    /// + 当状态机拥有[ServiceTarget]时,该成员为[ServiceTarget]的值,否则默认为该状态的状态机[Entity]
    /// - When the state machine possesses [ServiceTarget], this member is the value of [ServiceTarget]; otherwise, it defaults to the state machine's [Entity] state
    pub service_target: Entity,
    /// 状态机实体
    ///
    /// State machine entity
    pub state_machine: Entity,
    /// 当前状态实体
    ///
    /// Current state entity
    pub state: Entity,
}

impl HsmStateContext {
    pub fn new(service_target: Entity, state_machine: Entity, state: Entity) -> Self {
        Self {
            service_target,
            state_machine,
            state,
        }
    }
}

/// 注册一次性的进入时系统
///
/// Register disposable enter systems
/// # 示例\Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn on_enter(entity:In<HsmStateContext>) {
/// #     println!("进入系统");
/// # }
/// # fn foo(mut commands:Commands, mut on_enter_disposable_systems: ResMut<HsmOnEnterDisposableSystems>) {
/// let system_id = commands.register_system(on_enter);
/// on_enter_disposable_systems.insert("on_enter", system_id);
/// # }
/// ```
///
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct HsmOnEnterDisposableSystems(HashMap<String, DisposableSystemId>);

impl HsmOnEnterDisposableSystems {
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
    /// fn foo(mut commands:Commands, mut on_enter_disposable_systems: ResMut<HsmOnEnterDisposableSystems>) {
    ///     let system_id = commands.register_system(on_enter);
    ///     on_enter_disposable_systems.insert("on_enter", system_id);
    /// }
    /// ```
    pub fn insert(&mut self, name: impl Into<String>, system_id: DisposableSystemId) {
        self.0.insert(name.into(), system_id);
    }

    /// 移除系统
    ///
    /// Remove system
    /// # 示例\Example
    /// ```rust
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// fn foo(mut commands:Commands, mut on_enter_disposable_systems: ResMut<HsmOnEnterDisposableSystems>) {
    ///     on_enter_disposable_systems.remove("on_enter");
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

/// 注册一次性的退出时系统
///
/// Register disposable exit systems
/// # 示例\Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn on_exit(entity:In<HsmStateContext>) {
/// #     println!("退出系统");
/// # }
/// # fn foo(mut commands:Commands, mut on_exit_disposable_systems: ResMut<HsmOnExitDisposableSystems>) {
/// let system_id = commands.register_system(on_exit);
/// on_exit_disposable_systems.insert("on_exit", system_id);
/// # }
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct HsmOnExitDisposableSystems(HashMap<String, DisposableSystemId>);

impl HsmOnExitDisposableSystems {
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
    /// fn foo(mut commands:Commands, mut on_exit_disposable_systems: ResMut<HsmOnExitDisposableSystems>) {
    ///     let system_id = commands.register_system(on_exit);
    ///     on_exit_disposable_systems.insert("on_exit", system_id);
    /// }
    /// ```
    pub fn insert(&mut self, name: impl Into<String>, system_id: DisposableSystemId) {
        self.0.insert(name.into(), system_id);
    }

    /// 移除系统
    ///
    /// Remove system
    /// # 示例\Example
    /// ```rust
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// fn foo(mut commands:Commands, mut on_exit_disposable_systems: ResMut<HsmOnExitDisposableSystems>) {
    ///     on_exit_disposable_systems.remove("on_exit");
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

/// 状态机服务目标
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[relationship(relationship_target = StateMachineForest)]
pub struct ServiceTarget(pub Entity);

/// 状态机森林
#[derive(Component, Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[relationship_target(relationship = ServiceTarget)]
pub struct StateMachineForest(Vec<Entity>);
