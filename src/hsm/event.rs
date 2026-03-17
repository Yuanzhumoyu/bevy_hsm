use bevy::prelude::*;

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
/// # fn hsm_system(
/// #     mut commands: Commands,
/// #     mut trigger_writer: EventWriter<HsmTrigger>
/// # ) {
/// # // Define states
/// # let root = commands.spawn(HsmState::default()).id();
/// # let child_a = commands.spawn(HsmState::default()).id();
/// # let child_b = commands.spawn(HsmState::default()).id();
/// #
/// # // Define tree
/// # let tree = commands.spawn(StateTree::new(root).with_child(root, child_a).with_child(root, child_b)).id();
/// #
/// # // Spawn state machine
/// # let sm_entity = commands.spawn(HsmStateMachine::new(tree)).id();
/// #
/// // To transition to a specific sub-state:
/// trigger_writer.send(HsmTrigger::with_sub(sm_entity, child_a));
///
/// // To transition back to the immediate super-state:
/// trigger_writer.send(HsmTrigger::with_super(sm_entity));
///
/// // To trigger a conditional transition to a sub-state:
/// trigger_writer.send(HsmTrigger::with_transition(sm_entity, child_b, false));
/// # }
/// ```
#[derive(EntityEvent, Clone, PartialEq, Eq, Hash)]
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

    pub const fn with_super(state_machine: Entity) -> Self {
        Self::new(state_machine, HsmTriggerType::Super)
    }

    pub const fn with_sub(state_machine: Entity, target: Entity) -> Self {
        Self::new(state_machine, HsmTriggerType::Sub(target))
    }

    pub const fn with_transition(state_machine: Entity, target: Entity, is_super: bool) -> Self {
        Self::new(
            state_machine,
            if is_super {
                HsmTriggerType::SuperTransition(target)
            } else {
                HsmTriggerType::SubTransition(target)
            },
        )
    }

    pub const fn with_next(state_machine: Entity, target: Entity) -> Self {
        Self::new(state_machine, HsmTriggerType::Next(target))
    }

    pub const fn state_machine(&self) -> Entity {
        self.state_machine
    }

    pub const fn typed(&self) -> &HsmTriggerType {
        &self.typed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HsmTriggerType {
    /// 直接返回父状态
    Super,
    /// 根据条件跳转父状态
    SuperTransition(Entity),
    /// 直接跳转下一个状态
    Sub(Entity),
    /// 根据条件跳转状态
    SubTransition(Entity),
    /// 直接跳转指定状态
    Next(Entity),
}
