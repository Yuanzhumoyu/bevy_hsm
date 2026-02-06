use std::hash::Hash;

use bevy::{
    ecs::system::SystemId,
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};

use crate::hook_system::context::{ConditionContext, ContextRelationship};

pub type DisposableSystemId = SystemId<In<HsmStateContext>, ()>;
/// 状态条件上下文
pub type HsmStateConditionContext = HsmStateContext<ConditionContext>;

mod context {
    use bevy::ecs::entity::Entity;

    pub trait ContextRelationship {}

    impl ContextRelationship for Entity {}

    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    pub struct ConditionContext {
        pub(super) from: Entity,
        pub(super) to: Entity,
    }

    impl ContextRelationship for ConditionContext {}

    impl ConditionContext {
        pub const fn new(from: Entity, to: Entity) -> Self {
            Self { from, to }
        }
    }
}

/// 状态上下文
///
/// StateContext
/// # 作用\Purpose
/// * 用于在系统中传递状态上下文
/// - Used to pass state context in systems
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct HsmStateContext<C: ContextRelationship = Entity> {
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
    relationship: C,
}

impl HsmStateContext {
    pub(crate) const fn new(service_target: Entity, state_machine: Entity, state: Entity) -> Self {
        Self {
            service_target,
            state_machine,
            relationship: state,
        }
    }

    pub const fn state(&self) -> Entity {
        self.relationship
    }
}

impl HsmStateContext<ConditionContext> {
    pub(crate) const fn new(
        service_target: Entity,
        state_machine: Entity,
        from_state: Entity,
        to_state: Entity,
    ) -> Self {
        Self {
            service_target,
            state_machine,
            relationship: ConditionContext::new(from_state, to_state),
        }
    }

    pub fn from_state(&self) -> Entity {
        self.relationship.from
    }

    pub fn to_state(&self) -> Entity {
        self.relationship.to
    }
}

/// 注册一次性用于运行[`HsmOnEnterSystem`] [`HsmOnExitSystem`]的系统
///
/// Register a one-time system for running [HsmOnEnterSystem] [HsmOnExitSystem]
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
pub struct HsmOnStateDisposableSystems(pub(super) HashMap<String, DisposableSystemId>);

impl HsmOnStateDisposableSystems {
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
    /// fn foo(mut commands:Commands, mut disposable_systems: ResMut<HsmOnStateDisposableSystems>) {
    ///     let system_id = commands.register_system(on_enter);
    ///     disposable_systems.insert("on_enter", system_id);
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
    /// fn foo(mut commands:Commands, mut on_enter_disposable_systems: ResMut<HsmOnStateDisposableSystems>) {
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

/// 状态机服务目标
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[relationship(relationship_target = StateMachineForest)]
pub struct ServiceTarget(pub Entity);

/// 状态机森林
#[derive(Component, Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deref)]
#[relationship_target(relationship = ServiceTarget)]
pub struct StateMachineForest(Vec<Entity>);
