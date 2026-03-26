use std::hash::Hash;

use bevy::{
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};

use crate::{
    context::{ActionId, TransitionId},
    error::StateMachineError,
};

/// 注册一次性用于运行[`AfterEnterSystem`] [`BeforeExitSystem`]的系统
///
/// Register a one-time system for running [AfterEnterSystem] [BeforeExitSystem]
/// # 示例\Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn after_enter(entity:In<ActionContext>) {
/// #     println!("进入系统");
/// # }
/// # fn foo(mut commands:Commands, mut action_registry: ResMut<ActionRegistry>) {
/// let system_id = commands.register_system(after_enter);
/// action_registry.insert("after_enter", system_id);
/// # }
/// ```
///
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct ActionRegistry(pub(super) HashMap<String, ActionId>);

impl ActionRegistry {
    /// 注册系统
    ///
    /// Register system
    /// # 示例\Example
    /// ```rust
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn after_enter(entity:In<ActionContext>) {
    /// #     println!("进入系统");
    /// # }
    /// fn foo(mut commands:Commands, mut action_registry: ResMut<ActionRegistry>) {
    ///     let system_id = commands.register_system(after_enter);
    ///     action_registry.insert("after_enter", system_id);
    /// }
    /// ```
    pub fn insert(&mut self, name: impl Into<String>, system_id: ActionId) -> Option<ActionId> {
        self.0.insert(name.into(), system_id)
    }

    /// 移除系统
    ///
    /// Remove system
    /// # 示例\Example
    /// ```rust
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// fn foo(mut commands:Commands, mut action_registry: ResMut<ActionRegistry>) {
    ///     action_registry.remove("after_enter");
    /// }
    /// ```
    pub fn remove<Q>(&mut self, name: &Q) -> Option<ActionId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.remove(name)
    }

    /// 获取系统
    pub fn get<Q>(&self, name: &Q) -> Option<ActionId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get(name).copied()
    }

    pub(crate) fn get_action_id<T: Component + std::ops::Deref<Target = String>>(
        world: &bevy::ecs::world::DeferredWorld,
        state_id: Entity,
    ) -> Option<ActionId> {
        let on_system = world.get::<T>(state_id)?;
        let system = world.resource::<ActionRegistry>();
        let system_name: &str = on_system.as_ref();
        let id = system.get(system_name);
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

impl<S: Into<String>> Extend<(S, ActionId)> for ActionRegistry {
    fn extend<T: IntoIterator<Item = (S, ActionId)>>(&mut self, iter: T) {
        self.0.extend(iter.into_iter().map(|(s, a)| (s.into(), a)));
    }
}

/// 注册用于状态转换的系统
///
/// Register systems for state transitions
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct TransitionRegistry(pub(super) HashMap<String, TransitionId>);

impl TransitionRegistry {
    /// 获取已注册的转换系统
    ///
    /// Get a registered transition system
    pub fn get<Q>(&self, name: &Q) -> Option<TransitionId>
    where
        Q: Hash + Equivalent<String> + ?Sized,
    {
        self.0.get(name).cloned()
    }

    /// 插入一个新的转换系统
    ///
    /// Insert a new transition system
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        transition_id: TransitionId,
    ) -> Option<TransitionId> {
        self.0.insert(name.into(), transition_id)
    }

    /// 移除一个已注册的转换系统
    ///
    /// Remove a registered transition system
    pub fn remove<Q>(&mut self, name: &Q) -> Option<TransitionId>
    where
        Q: Hash + Equivalent<String>,
    {
        self.0.remove(name)
    }

    /// 获取已注册转换系统的数量
    ///
    /// Get the number of registered transition systems
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 检查转换注册表是否为空
    ///
    /// Check if the transition registry is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn get_transition_id<T: Component + std::ops::Deref<Target = String>>(
        world: &bevy::ecs::world::DeferredWorld,
        state_id: Entity,
    ) -> Option<TransitionId> {
        let on_system = world.get::<T>(state_id)?;
        let system = world.resource::<TransitionRegistry>();
        let system_name: &str = on_system.as_ref();
        let id = system.get(system_name);
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

impl<S: Into<String>> Extend<(S, TransitionId)> for TransitionRegistry {
    fn extend<T: IntoIterator<Item = (S, TransitionId)>>(&mut self, iter: T) {
        self.0.extend(iter.into_iter().map(|(s, a)| (s.into(), a)));
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
    /// commands.spawn(BeforeEnterSystem::new("before_enter"));
    /// # }
    /// ```
    BeforeEnterSystem
}

define_state_action_component! {
    /// 进入状态时调用
    ///
    /// Enter state when calling
    /// # 示例\Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn foo(mut commands: Commands) {
    /// commands.spawn(AfterEnterSystem::new("enter"));
    /// # }
    /// ```
    AfterEnterSystem
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
    /// # fn add(contexts:In<Vec<ActionContext>>)->Option<Vec<ActionContext>>{None}
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
    /// 退出状态时调用
    ///
    /// Exit state when calling
    /// # 示例\Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn foo(mut commands: Commands) {
    /// commands.spawn(BeforeExitSystem::new("exit"));
    /// # }
    /// ```
    BeforeExitSystem
}

define_state_action_component! {
    /// 退出状态之后调用
    ///
    /// Called after exiting the state
    /// # 示例\Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_hsm::prelude::*;
    /// # fn foo(mut commands: Commands) {
    /// commands.spawn(AfterExitSystem::new("after_exit"));
    /// # }
    /// ```
    ///
    AfterExitSystem
}

/// # 状态机服务目标
///
/// * 用于将状态机事件委托给另一个实体处理，从而实现状态机与业务逻辑的分离。
///   当 `ServiceTarget` 存在时，所有状态机事件（如 `AfterEnter`、`BeforeExit`）将发送到 `ServiceTarget` 指定的实体，
///   而不是状态机本身。这在实现可复用的状态机蓝图或需要将状态逻辑与实体属性分离时非常有用。
///
/// # State Machine Service Target
///
/// * Used to delegate state machine events to another entity for processing, thereby separating the state machine from business logic.
///   When `ServiceTarget` is present, all state machine events (such as `AfterEnter`, `BeforeExit`) will be sent to the entity specified by `ServiceTarget`,
///   instead of the state machine itself. This is very useful when implementing reusable state machine blueprints or when state logic needs to be separated from entity properties.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[relationship(relationship_target = StateMachineForest)]
pub struct ServiceTarget(pub Entity);

/// # 状态机森林
///
/// * 一个实体，用于聚合多个状态机，形成一个状态机“森林”。
///   `StateMachineForest` 与 `ServiceTarget` 结合使用，允许一个实体（森林）管理多个状态机实例。
///   当一个状态机被添加到森林中时，它的 `ServiceTarget` 会指向这个森林实体，从而将事件委托给森林进行统一处理。
///
/// # State Machine Forest
///
/// * An entity used to aggregate multiple state machines, forming a state machine "forest".
///   `StateMachineForest` is used in conjunction with `ServiceTarget` to allow one entity (the forest) to manage multiple state machine instances.
///   When a state machine is added to the forest, its `ServiceTarget` will point to this forest entity, thus delegating events to the forest for unified processing.
#[derive(Component, Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deref)]
#[relationship_target(relationship = ServiceTarget)]
pub struct StateMachineForest(Vec<Entity>);
