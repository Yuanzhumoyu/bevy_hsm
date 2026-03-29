use bevy::prelude::*;

use crate::guards::GuardCondition;

/// # HSM 触发器
/// * 用于驱动层级状态机（HSM）进行状态转换的核心事件。
///
/// 当这个事件被发送时，它会指定目标 `HsmStateMachine` 实体，并附带一个 `HsmTriggerType`，
/// 该类型描述了要执行的转换的具体种类（例如，转换到父状态、子状态，或带条件的转换）。
///
/// # HSM Trigger
/// * The core event used to drive state transitions in a Hierarchical State Machine (HSM).
///
/// When this event is sent, it specifies the target `HsmStateMachine` entity and includes an
/// `HsmTriggerType`, which describes the specific kind of transition to perform (e.g., transitioning
/// to a super-state, a sub-state, or a conditional transition).
///
/// # Example
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// #
/// # fn hsm_system(mut commands: Commands) {
/// # // Define states
/// # let root = commands.spawn(HsmState::default()).id();
/// # let child_a = commands.spawn(HsmState::default()).id();
/// # let child_b = commands.spawn(HsmState::default()).id();
/// #
/// # // Define tree
/// # let mut tree = StateTree::new(root);
/// # tree.with_child(root, child_a).with_child(root, child_b);
/// # let tree_id = commands.spawn(tree).id();
/// #
/// # // Spawn state machine
/// # let sm_entity = commands.spawn(HsmStateMachine::with(tree_id, root,#[cfg(feature = "history")] 10)).id();
/// #
/// // To transition to a specific sub-state:
/// commands.trigger(HsmTrigger::to_sub(sm_entity, child_a));
///
/// // To transition back to the immediate super-state:
/// commands.trigger(HsmTrigger::to_super(sm_entity));
///
/// // To trigger a conditional transition to a sub-state:
/// commands.trigger(HsmTrigger::guard_sub(sm_entity,GuardCondition::from("sub"), child_b));
///
/// // To trigger a conditional transition to a super-state:
/// commands.trigger(HsmTrigger::guard_super(sm_entity,GuardCondition::from("super")));
/// # }
/// ```
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq, Hash)]
pub struct HsmTrigger {
    #[event_target]
    pub(crate) state_machine: Entity,
    pub(crate) typed: HsmTriggerType,
}

impl HsmTrigger {
    pub const fn new(state_machine: Entity, typed: HsmTriggerType) -> Self {
        Self {
            state_machine,
            typed,
        }
    }

    /// 创建一个向上级状态转换的触发器
    ///
    /// Creates a trigger for transitioning to a parent state
    pub const fn to_super(state_machine: Entity) -> Self {
        Self::new(state_machine, HsmTriggerType::ToSuper)
    }

    /// 创建一个向子状态转换的触发器
    ///
    /// Creates a trigger for transitioning to a child state
    pub const fn to_sub(state_machine: Entity, target: Entity) -> Self {
        Self::new(state_machine, HsmTriggerType::ToSub(target))
    }

    /// 创建一个带条件的向上级状态转换的触发器
    pub const fn guard_super(state_machine: Entity, guard: GuardCondition) -> Self {
        Self::new(state_machine, HsmTriggerType::GuardSuper(guard))
    }

    /// 创建一个带条件的向子状态转换的触发器
    pub const fn guard_sub(state_machine: Entity, guard: GuardCondition, target: Entity) -> Self {
        Self::new(state_machine, HsmTriggerType::GuardSub(guard, target))
    }

    /// 创建一个链式过渡到目标状态的触发器, 该触发器会查询当前状态到目标状态之间的所有子状态，并依次触发子状态的更新
    ///
    /// Creates a trigger for chaining transitions to a target state, querying all intermediate sub-states
    /// and updating them in sequence.
    pub const fn chain(state_machine: Entity, target: Entity) -> Self {
        Self::new(state_machine, HsmTriggerType::Chain(target))
    }

    /// 获取触发器关联的状态机实体
    ///
    /// Gets the state machine entity associated with the trigger
    pub const fn state_machine(&self) -> Entity {
        self.state_machine
    }

    /// 获取触发器的类型
    ///
    /// Gets the type of the trigger
    pub const fn typed(&self) -> &HsmTriggerType {
        &self.typed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HsmTriggerType {
    /// 直接返回父状态
    ///
    /// Directly return to parent state
    ToSuper,
    /// 根据条件跳转父状态
    ///
    /// Jump to parent state based on condition
    GuardSuper(GuardCondition),
    /// 直接跳转下一个状态
    ///
    /// Directly jump to next state
    ToSub(Entity),
    /// 根据条件跳转状态
    ///
    /// Jump to state based on condition
    GuardSub(GuardCondition, Entity),
    /// 直接跳转指定状态
    ///
    /// Directly jump to specified state
    Chain(Entity),
}
