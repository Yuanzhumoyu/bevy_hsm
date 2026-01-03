use std::hash::Hash;

use bevy::{
    ecs::system::SystemId,
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};

pub type DisposableSystemId = SystemId<In<HsmStateContext>, ()>;

/// 状态上下文
/// # 作用
/// * 用于在系统中传递状态上下文
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct HsmStateContext {
    /// 主体实体
    pub main_body: Entity,
    /// 当前状态实体
    pub state: Entity,
}

impl HsmStateContext {
    pub fn new(main_body: Entity, state: Entity) -> Self {
        Self { main_body, state }
    }
}

/// 注册一次性的进入时系统 
/// # 示例
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

    pub fn insert(&mut self, name: impl Into<String>, system_id: DisposableSystemId) {
        self.0.insert(name.into(), system_id);
    }

    pub fn remove<Q>(&mut self, name: &Q) -> Option<DisposableSystemId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.remove(name)
    }

    pub fn get<Q>(&self, name: &Q) -> Option<&DisposableSystemId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get(name)
    }

    pub fn get_mut<Q>(&mut self, name: &Q) -> Option<&mut DisposableSystemId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get_mut(name)
    }
}

/// 注册一次性的退出时系统 
/// # 示例
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

    pub fn insert(&mut self, name: impl Into<String>, system_id: DisposableSystemId) {
        self.0.insert(name.into(), system_id);
    }

    pub fn remove<Q>(&mut self, name: &Q) -> Option<DisposableSystemId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.remove(name)
    }

    pub fn get<Q>(&self, name: &Q) -> Option<&DisposableSystemId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get(name)
    }

    pub fn get_mut<Q>(&mut self, name: &Q) -> Option<&mut DisposableSystemId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get_mut(name)
    }
}
