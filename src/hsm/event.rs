use bevy::prelude::*;

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
}
