use std::borrow::Cow;

use bevy::prelude::Deref;

#[cfg(any(feature = "hsm", feature = "fsm"))]
use crate::error::StateMachineError;
#[cfg(any(feature = "hsm", feature = "fsm"))]
use bevy::ecs::entity::Entity;

/// # 系统标签\System Label
/// * 用于标识已注册系统的唯一名称，支持从多种字符串类型构造。
/// - A unique name for identifying registered systems, constructible from various string types.
#[derive(Default, Clone, Debug, Eq, PartialEq, Hash, Deref)]
pub struct SystemLabel(pub Cow<'static, str>);

impl SystemLabel {
    /// 找不到该系统的错误
    #[cfg(any(feature = "hsm", feature = "fsm"))]
    pub(crate) fn not_found_error(&self, state: Entity) -> StateMachineError {
        StateMachineError::SystemNotFound {
            system_name: self.clone(),
            state,
        }
    }

    /// 根据类型创建 [`SystemLabel`]，用于通过组件类型自动推导系统名称。
    ///
    /// Creates a [`SystemLabel`] from a type, useful for auto-deriving system names from component types.
    pub fn type_name<T: 'static>() -> Self {
        SystemLabel(Cow::Borrowed(std::any::type_name::<T>()))
    }

    /// 根据值的类型创建 [`SystemLabel`]。
    ///
    /// Creates a [`SystemLabel`] from the type of a value.
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
        Self(Cow::Owned(value))
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
