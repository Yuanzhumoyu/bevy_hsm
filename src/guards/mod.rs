mod condition;
mod parser;
pub mod registry;

pub use condition::GuardCondition;
pub use registry::{CompiledGuard, CompiledGuardId, CompiledGuardRegistry, GuardRegistry};

use std::fmt::{Debug, Display};

use crate::labels::SystemLabel;

/// 解析 GuardCondition 时的错误类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GuardConditionParseError {
    EmptyInput,
    UnexpectedToken(String),
    UnexpectedEOF,
    InvalidOperator(String),
    TooFewOperands(String),
    TrailingToken(String),
    InvalidCharacter(char),
}

impl Display for GuardConditionParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardConditionParseError::EmptyInput => write!(f, "input is empty"),
            GuardConditionParseError::UnexpectedToken(tok) => {
                write!(f, "unexpected token: {}", tok)
            }
            GuardConditionParseError::UnexpectedEOF => write!(f, "unexpected end of input"),
            GuardConditionParseError::InvalidOperator(op) => write!(f, "invalid operator: {}", op),
            GuardConditionParseError::TooFewOperands(op) => {
                write!(f, "operator '{}' needs at least 2 operands", op)
            }
            GuardConditionParseError::TrailingToken(tok) => write!(f, "trailing token: {}", tok),
            GuardConditionParseError::InvalidCharacter(c) => {
                write!(f, "invalid character: '{}'", c)
            }
        }
    }
}

impl std::error::Error for GuardConditionParseError {}

/// Guard 解析/解析到已注册系统 ID 时可能出现的错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardResolveError {
    UnregisteredGuard(SystemLabel),
}

impl Display for GuardResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardResolveError::UnregisteredGuard(label) => {
                write!(f, "unregistered guard: {}", label)
            }
        }
    }
}

impl std::error::Error for GuardResolveError {}

/// 状态条件的系统ID
///
/// 用于判断`State`是否满足进入或退出的条件,其中上下文中的实体是当前检测的实体
///
/// State condition system ID
///
/// Used to determine if `State` meets the conditions for entering or exiting, where the context entity is the entity currently being checked
pub type GuardId =
    bevy::ecs::system::SystemId<bevy::ecs::system::In<crate::context::GuardContext>, bool>;
