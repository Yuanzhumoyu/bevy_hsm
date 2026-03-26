use std::fmt::Debug;

use bevy::{ecs::system::SystemId, prelude::*};

/// A system ID for a transition, which takes a `TransitionContext` as input.
///
/// 用于标识一个转换的 `SystemId`，该系统接收 `TransitionContext` 作为输入。
pub type TransitionId = SystemId<In<TransitionContext>, ()>;
/// A system ID for an action, which takes an `ActionContext` as input.
///
/// 用于标识一个动作的 `SystemId`，该系统接收 `ActionContext` 作为输入。
pub type ActionId = SystemId<In<ActionContext>, ()>;

/// 用于状态转换的上下文
pub type TransitionContext = StateContext<TransitionRelationship>;
/// 用于条件守卫的上下文
pub type GuardContext = StateContext<ConditionRelationship>;
/// 用于状态动作的上下文
pub type ActionContext = StateContext<Entity>;

mod context_type {
    use super::{ConditionRelationship, TransitionRelationship};
    use bevy::ecs::entity::Entity;

    pub trait ContextRelationship {}

    impl ContextRelationship for Entity {}

    impl ContextRelationship for ConditionRelationship {}

    impl ContextRelationship for TransitionRelationship {}
}

/// 状态上下文
///
/// StateContext
/// # 作用\Purpose
/// * 用于在系统中传递状态上下文
/// - Used to pass state context in systems
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct StateContext<C: context_type::ContextRelationship = Entity> {
    /// 主体实体
    ///
    /// Main body entity
    /// + 当状态机拥有\[ServiceTarget\]时,该成员为\[ServiceTarget\]的值,否则默认为该状态的状态机[Entity]
    /// - When the state machine possesses \[ServiceTarget\], this member is the value of \[ServiceTarget\]; otherwise, it defaults to the state machine's [Entity] state
    pub service_target: Entity,
    /// 状态机实体
    ///
    /// State machine entity
    pub state_machine: Entity,
    relationship: C,
}

impl<T: context_type::ContextRelationship> StateContext<T> {
    pub(crate) const fn with(
        service_target: Entity,
        state_machine: Entity,
        relationship: T,
    ) -> Self {
        Self {
            service_target,
            state_machine,
            relationship,
        }
    }

    pub const fn relationship(&self) -> &T {
        &self.relationship
    }
}

impl<T: Default + context_type::ContextRelationship> Default for StateContext<T> {
    fn default() -> Self {
        Self {
            service_target: Entity::PLACEHOLDER,
            state_machine: Entity::PLACEHOLDER,
            relationship: Default::default(),
        }
    }
}

impl ActionContext {
    pub(crate) const fn new(service_target: Entity, state_machine: Entity, state: Entity) -> Self {
        Self {
            service_target,
            state_machine,
            relationship: state,
        }
    }

    /// Returns the state entity associated with this action.
    ///
    /// 返回与此动作关联的状态实体。
    pub const fn state(&self) -> Entity {
        self.relationship
    }
}

impl Debug for ActionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ActionContext")
            .field(&self.service_target)
            .field(&self.state_machine)
            .field(&self.relationship)
            .finish()
    }
}

impl GuardContext {
    pub(crate) const fn new(
        service_target: Entity,
        state_machine: Entity,
        from_state: Entity,
        to_state: Entity,
    ) -> Self {
        Self {
            service_target,
            state_machine,
            relationship: ConditionRelationship::new(from_state, to_state),
        }
    }

    /// The state the transition is coming from.
    ///
    /// 转换的起始状态。
    pub fn from_state(&self) -> Entity {
        self.relationship.from
    }

    /// The state the transition is going to.
    ///
    /// 转换的目标状态。
    pub fn to_state(&self) -> Entity {
        self.relationship.to
    }
}

impl Debug for GuardContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GuardContext")
            .field(&self.service_target)
            .field(&self.state_machine)
            .field(&self.relationship)
            .finish()
    }
}

impl TransitionContext {
    pub(crate) const fn with_transition(
        service_target: Entity,
        state_machine: Entity,
        from: Entity,
        to: Entity,
    ) -> Self {
        Self {
            service_target,
            state_machine,
            relationship: TransitionRelationship::Transition(from, to),
        }
    }

    pub(crate) const fn with_final(
        service_target: Entity,
        state_machine: Entity,
        r#final: Entity,
    ) -> Self {
        Self {
            service_target,
            state_machine,
            relationship: TransitionRelationship::Final(r#final),
        }
    }

    pub(crate) const fn with_initial(
        service_target: Entity,
        state_machine: Entity,
        initial: Entity,
    ) -> Self {
        Self {
            service_target,
            state_machine,
            relationship: TransitionRelationship::Initial(initial),
        }
    }

    /// 获取转换的起始状态。
    /// - `Initial`: 返回 `None`
    /// - `Transition`: 返回 `from` 状态
    /// - `Final`: 返回 `from` 状态
    pub fn from_state(&self) -> Option<Entity> {
        match self.relationship {
            TransitionRelationship::Initial(_) => None,
            TransitionRelationship::Transition(from, _) => Some(from),
            TransitionRelationship::Final(from) => Some(from),
        }
    }

    /// 获取转换的目标状态。
    /// - `Initial`: 返回 `to` 状态
    /// - `Transition`: 返回 `to` 状态
    /// - `Final`: 返回 `None`
    pub fn to_state(&self) -> Option<Entity> {
        match self.relationship {
            TransitionRelationship::Initial(to) => Some(to),
            TransitionRelationship::Transition(_, to) => Some(to),
            TransitionRelationship::Final(_) => None,
        }
    }

    /// 获取转换的（起始，目标）状态元组。
    pub fn transition(&self) -> (Option<Entity>, Option<Entity>) {
        match self.relationship {
            TransitionRelationship::Initial(to) => (None, Some(to)),
            TransitionRelationship::Transition(from, to) => (Some(from), Some(to)),
            TransitionRelationship::Final(from) => (Some(from), None),
        }
    }
}

impl Debug for TransitionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TransitionContext")
            .field(&self.service_target)
            .field(&self.state_machine)
            .field(&self.relationship)
            .finish()
    }
}

/// Represents the relationship between two states for a guard condition.
///
/// 表示守卫条件中两个状态之间的关系。
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct ConditionRelationship {
    pub(super) from: Entity,
    pub(super) to: Entity,
}

impl ConditionRelationship {
    pub const fn new(from: Entity, to: Entity) -> Self {
        Self { from, to }
    }
}

impl Debug for ConditionRelationship {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("[{} -> {}]", self.from, self.to))
    }
}

/// Represents the type of a state transition.
///
/// 表示状态转换的类型。
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum TransitionRelationship {
    /// A transition from the initial state.
    ///
    /// 从初始状态开始的转换。
    Initial(Entity),
    /// A transition between two regular states.
    ///
    /// 两个常规状态之间的转换。
    Transition(Entity, Entity),
    /// A transition to a final state.
    ///
    /// 到达最终状态的转换。
    Final(Entity),
}

impl Debug for TransitionRelationship {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TransitionRelationship::Initial(to) => format!("<{}]", to),
            TransitionRelationship::Transition(from, to) => format!("[{} -> {}]", from, to),
            TransitionRelationship::Final(from) => format!("[{}>", from),
        };
        f.write_str(&s)
    }
}
