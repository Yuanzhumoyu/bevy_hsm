use std::hash::Hash;

use bevy::{
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};

use crate::{context::StateActionId, error::StateMachineError};

/// 注册一次性用于运行[`OnEnterSystem`] [`OnExitSystem`]的系统
///
/// Register a one-time system for running [OnEnterSystem] [OnExitSystem]
/// # 示例\Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn on_enter(entity:In<StateActionContext>) {
/// #     println!("进入系统");
/// # }
/// # fn foo(mut commands:Commands, mut action_registry: ResMut<StateActionRegistry>) {
/// let system_id = commands.register_system(on_enter);
/// action_registry.insert("on_enter", system_id);
/// # }
/// ```
///
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct StateActionRegistry(pub(super) HashMap<String, StateActionId>);

impl StateActionRegistry {
    /// 注册系统
    ///
    /// Register system
    /// # 示例\Example
    /// ```rust
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn on_enter(entity:In<StateActionContext>) {
    /// #     println!("进入系统");
    /// # }
    /// fn foo(mut commands:Commands, mut action_registry: ResMut<StateActionRegistry>) {
    ///     let system_id = commands.register_system(on_enter);
    ///     action_registry.insert("on_enter", system_id);
    /// }
    /// ```
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        system_id: StateActionId,
    ) -> Option<StateActionId> {
        self.0.insert(name.into(), system_id)
    }

    /// 移除系统
    ///
    /// Remove system
    /// # 示例\Example
    /// ```rust
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// fn foo(mut commands:Commands, mut action_registry: ResMut<StateActionRegistry>) {
    ///     action_registry.remove("on_enter");
    /// }
    /// ```
    pub fn remove<Q>(&mut self, name: &Q) -> Option<StateActionId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.remove(name)
    }

    /// 获取系统
    pub fn get<Q>(&self, name: &Q) -> Option<&StateActionId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get(name)
    }

    pub(crate) fn get_system_id<T: Component + std::ops::Deref<Target = String>>(
        world: &bevy::ecs::world::DeferredWorld,
        state_id: Entity,
    ) -> Option<StateActionId> {
        let on_system = world.get::<T>(state_id)?;
        let system = world.resource::<StateActionRegistry>();
        let system_name: &str = on_system.as_ref();
        let id = system.get(system_name).cloned();
        if id.is_none() {
            warn!(
                "{}",
                StateMachineError::SystemNotFound {
                    system_name: system_name.to_string(),
                    state: state_id
                }
            )
        }
        id
    }
}

macro_rules! define_state_action_component {
    ($(#[$outer:meta])* $name:ident) => {
        $(#[$outer])*
        #[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(name: impl Into<String>) -> Self {
                Self(name.into())
            }
        }
    };
}

define_state_action_component! {
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
    OnEnterSystem
}

define_state_action_component! {
    /// 更新状态时调用
    ///
    /// Update state when calling
    /// # 使用方法\Usage
    ///  由于注册动作系统时，通过`ScheduleLabel`来确定系统调用时间，
    ///  所以在使用对应`ScheduleLabel`的系统时，需要特定格式。
    ///
    ///  When registering an action system, the system call time is determined through `ScheduleLabel`,
    ///  Therefore, when using the system corresponding to `ScheduleLabel`, a specific format is required.
    /// * 正常格式: `ScheduleLabel` + `:` + `方法名称`
    /// - Normal format: `ScheduleLabel` + `:` + `method name`
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn add(contexts:In<Vec<StateActionContext>>)->Option<Vec<StateActionContext>>{None}
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
    OnUpdateSystem
}

define_state_action_component! {
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
    OnExitSystem
}

/// 状态机服务目标
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[relationship(relationship_target = StateMachineForest)]
pub struct ServiceTarget(pub Entity);

/// 状态机森林
#[derive(Component, Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deref)]
#[relationship_target(relationship = ServiceTarget)]
pub struct StateMachineForest(Vec<Entity>);
