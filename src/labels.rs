use std::borrow::Cow;

use bevy::{ecs::entity::Entity, prelude::Deref};

use crate::error::StateMachineError;

#[derive(Default, Clone, Debug, Eq, PartialEq, Hash, Deref)]
pub struct SystemLabel(pub Cow<'static, str>);

impl SystemLabel {
    /// 找不到该系统的错误
    pub(crate) fn not_found_error(&self, state: Entity) -> StateMachineError {
        StateMachineError::SystemNotFound {
            system_name: self.clone(),
            state,
        }
    }

    pub fn type_name<T: 'static>() -> Self {
        SystemLabel(Cow::Borrowed(std::any::type_name::<T>()))
    }

    pub fn type_name_of<T: ?Sized>(val: &T) -> Self {
        SystemLabel(Cow::Borrowed(std::any::type_name_of_val(val)))
    }
}

impl From<&'static str> for SystemLabel {
    fn from(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for SystemLabel {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value.to_owned()))
    }
}

impl From<Cow<'static, str>> for SystemLabel {
    fn from(value: Cow<'static, str>) -> Self {
        Self(value)
    }
}

impl std::borrow::Borrow<str> for SystemLabel {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

impl std::fmt::Display for SystemLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
