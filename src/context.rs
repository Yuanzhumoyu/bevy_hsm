use bevy::{ecs::system::SystemId, prelude::*};

pub type DisposableSystemId = SystemId<In<OnStateContext>, ()>;
/// 状态条件上下文
pub type OnStateConditionContext = StateContext<context_type::ConditionContext>;
/// 状态上下文
pub type OnStateContext = StateContext;

mod context_type {
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
pub struct StateContext<C: context_type::ContextRelationship = Entity> {
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

impl StateContext<Entity> {
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

impl StateContext<context_type::ConditionContext> {
    pub(crate) const fn new(
        service_target: Entity,
        state_machine: Entity,
        from_state: Entity,
        to_state: Entity,
    ) -> Self {
        Self {
            service_target,
            state_machine,
            relationship: context_type::ConditionContext::new(from_state, to_state),
        }
    }

    pub fn from_state(&self) -> Entity {
        self.relationship.from
    }

    pub fn to_state(&self) -> Entity {
        self.relationship.to
    }
}
