use std::{fmt::Display, str::FromStr};

use smallvec::SmallVec;

use super::{GuardConditionParseError, parser::Parser};
use crate::labels::SystemLabel;

/// 组合条件
///
/// Combination condition
///
/// 用于组合多个状态条件，支持AND、OR、NOT操作。
///
/// Use to combine multiple state conditions, support AND, OR, NOT operations.
/// # 示例\Example
///
/// ```rust
/// use bevy_hsm::prelude::*;
///
/// # fn main(){
/// // 使用宏创建组合条件
/// // Using macro to create combination conditions
/// let condition1 = combination_condition!(and("condition_a", "condition_b"));
///
/// // 使用解析方法创建
/// // Using the parsing method to create
/// let condition2 = GuardCondition::parse("and(condition_a, condition_b)").unwrap();
///
/// // 使用构造方法创建
/// // Using the constructor method to create
/// let condition3 = GuardCondition::new("condition_a").add_and(GuardCondition::new("condition_b"));
///
/// assert_eq!(condition1, condition3);
/// assert_eq!(condition2, condition3);
/// # }
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum GuardCondition {
    And(SmallVec<[Box<GuardCondition>; 2]>),
    Or(SmallVec<[Box<GuardCondition>; 2]>),
    Not(Box<GuardCondition>),
    Id(SystemLabel),
}

impl GuardCondition {
    pub fn new(name: impl Into<SystemLabel>) -> Self {
        Self::Id(name.into())
    }

    /// 创建一个and组合条件, 相同条件则合并
    ///
    /// Create an and combination condition, same condition will be merged
    pub fn and(conditions: impl IntoIterator<Item = Self>) -> Result<Self, &'static str> {
        let conditions: SmallVec<[Box<GuardCondition>; 2]> =
            conditions.into_iter().map(Box::new).collect();

        if conditions.len() < 2 {
            return Err("and condition must have at least 2 conditions");
        }

        Ok(GuardCondition::And(conditions))
    }

    /// 创建一个or组合条件, 相同条件则合并
    ///
    /// Create an or combination condition, same condition will be merged
    pub fn or(conditions: impl IntoIterator<Item = Self>) -> Result<Self, &'static str> {
        let conditions: SmallVec<[Box<GuardCondition>; 2]> =
            conditions.into_iter().map(Box::new).collect();

        if conditions.len() < 2 {
            return Err("or condition must have at least 2 conditions");
        }

        Ok(GuardCondition::Or(conditions))
    }

    /// 创建一个not组合条件，相同条件则不变
    ///
    /// Create a not combination condition, same condition will not change
    #[inline(always)]
    #[allow(clippy::should_implement_trait)]
    pub fn not(condition: GuardCondition) -> Self {
        condition.add_not()
    }

    /// 添加一个 `AND` 条件。
    ///
    /// Adds an `AND` condition.
    pub fn add_and(self, condition: GuardCondition) -> Self {
        let mut conditions = SmallVec::new();
        match self {
            Self::And(mut inner) => conditions.append(&mut inner),
            other => conditions.push(Box::new(other)),
        }
        match condition {
            Self::And(mut inner) => conditions.append(&mut inner),
            other => conditions.push(Box::new(other)),
        }
        Self::And(conditions)
    }

    /// 添加一个 `OR` 条件。
    ///
    /// Adds an `OR` condition.
    pub fn add_or(self, condition: GuardCondition) -> Self {
        let mut conditions = SmallVec::new();
        match self {
            Self::Or(mut inner) => conditions.append(&mut inner),
            other => conditions.push(Box::new(other)),
        }
        match condition {
            Self::Or(mut inner) => conditions.append(&mut inner),
            other => conditions.push(Box::new(other)),
        }
        Self::Or(conditions)
    }

    /// 添加一个 `NOT` 条件。
    ///
    /// Adds a `NOT` condition.
    pub fn add_not(self) -> Self {
        match self {
            Self::Not(condition) => *condition,
            _ => Self::Not(Box::new(self)),
        }
    }
}

impl GuardCondition {
    ///# 编写规则\Write rules
    ///- combination_condition := not_condition | and_condition | or_condition | id_condition
    ///- not_condition := `not` `(` combination_condition `)`
    ///- and_condition := `and` `(` combination_condition `,` ( combination_condition )+ `)`
    ///- or_condition := `or` `(` combination_condition `,` ( combination_condition )+ `)`
    ///- id_condition := ident
    pub fn parse(s: impl AsRef<str>) -> Result<Self, GuardConditionParseError> {
        let input = s.as_ref().trim();
        if input.is_empty() {
            return Err(GuardConditionParseError::EmptyInput);
        }
        let mut parser = Parser::new(input);
        let cond = parser.parse_combination_condition()?;
        // 检查是否有多余 token
        if parser.current_token.is_some() {
            return Err(GuardConditionParseError::TrailingToken(format!(
                "{:?}",
                parser.current_token
            )));
        }
        Ok(cond)
    }
}

impl Display for GuardCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardCondition::And(ands) => {
                write!(f, "and(")?;
                for (i, x) in ands.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", x)?;
                }
                write!(f, ")")
            }
            GuardCondition::Or(ors) => {
                write!(f, "or(")?;
                for (i, x) in ors.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", x)?;
                }
                write!(f, ")")
            }
            GuardCondition::Not(not) => write!(f, "not({})", not),
            GuardCondition::Id(id) => write!(f, "{}", id),
        }
    }
}

impl std::fmt::Debug for GuardCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}

impl From<SystemLabel> for GuardCondition {
    fn from(value: SystemLabel) -> Self {
        GuardCondition::Id(value)
    }
}

impl<'a> From<&'a str> for GuardCondition {
    fn from(value: &'a str) -> Self {
        GuardCondition::Id(SystemLabel::from(value.to_string()))
    }
}

impl FromStr for GuardCondition {
    type Err = GuardConditionParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_hsm_macros::combination_condition;

    #[test]
    fn test_combination_condition() {
        let conditions = GuardCondition::new("a").add_and(GuardCondition::new("b"));
        assert_eq!(
            conditions,
            GuardCondition::And(SmallVec::from_buf([
                Box::new(GuardCondition::new("a")),
                Box::new(GuardCondition::new("b")),
            ]))
        );

        let conditions = GuardCondition::new("a").add_or(GuardCondition::new("c"));
        assert_eq!(
            conditions,
            GuardCondition::Or(SmallVec::from_buf([
                Box::new(GuardCondition::new("a")),
                Box::new(GuardCondition::new("c")),
            ]))
        );

        let conditions = GuardCondition::new("a")
            .add_and(GuardCondition::new("b"))
            .add_or(GuardCondition::new("c"));
        assert_eq!(
            conditions,
            GuardCondition::Or(SmallVec::from_buf([
                Box::new(GuardCondition::And(SmallVec::from_buf([
                    Box::new(GuardCondition::new("a")),
                    Box::new(GuardCondition::new("b")),
                ]))),
                Box::new(GuardCondition::new("c")),
            ]))
        );

        let a_conditions = GuardCondition::new("a").add_and(GuardCondition::new("b"));
        let b_conditions = GuardCondition::new("c").add_and(GuardCondition::new("d"));
        let conditions = a_conditions.add_and(b_conditions);
        assert_eq!(format!("{}", conditions), "and(a, b, c, d)");

        let a_conditions = GuardCondition::new("a").add_or(GuardCondition::new("b"));
        let b_conditions = GuardCondition::new("c").add_or(GuardCondition::new("d"));
        let conditions = a_conditions.add_or(b_conditions);
        assert_eq!(format!("{}", conditions), "or(a, b, c, d)");
    }

    #[test]
    fn test_debug_combination_condition() {
        let conditions = GuardCondition::new("a")
            .add_and(GuardCondition::new("b"))
            .add_or(GuardCondition::new("c"));
        assert_eq!(format!("{}", conditions), "or(and(a, b), c)");
        assert_eq!(format!("{:?}", conditions), "or(and(a, b), c)");
    }

    #[test]
    fn test_hsm_combination_condition() {
        let and_condition = combination_condition!(and("a", "b"));
        assert_eq!(format!("{}", and_condition), "and(a, b)");

        let or_condition = combination_condition!(or("a", "b"));
        assert_eq!(format!("{}", or_condition), "or(a, b)");

        let not_condition = combination_condition!(not("a"));
        assert_eq!(format!("{}", not_condition), "not(a)");

        let id_condition = combination_condition!("a");
        assert_eq!(format!("{}", id_condition), "a");

        let combination_condition = combination_condition!(and(or("a", "b"), "c"));
        assert_eq!(format!("{}", combination_condition), "and(or(a, b), c)");

        let combination_condition =
            combination_condition!(and(#and_condition, #not_condition, #or_condition));
        assert_eq!(
            format!("{}", combination_condition),
            "and(and(a, b), not(a), or(a, b))"
        );
    }

    #[test]
    fn test_parse_combination_condition() {
        let condition = GuardCondition::parse("and(a, b)")
            .expect("failed to parse guard condition 'and(a, b)'");
        assert_eq!(format!("{}", condition), "and(a, b)");

        let condition =
            GuardCondition::parse("or(a, b)").expect("failed to parse guard condition 'or(a, b)'");
        assert_eq!(format!("{}", condition), "or(a, b)");

        let condition =
            GuardCondition::parse("not(a)").expect("failed to parse guard condition 'not(a)'");
        assert_eq!(format!("{}", condition), "not(a)");

        let condition = GuardCondition::parse("a").expect("failed to parse guard condition 'a'");
        assert_eq!(format!("{}", condition), "a");

        let condition = GuardCondition::parse("and(a, not(b), or(c, b))")
            .expect("failed to parse guard condition 'and(a, not(b), or(c, b))'");
        assert_eq!(format!("{}", condition), "and(a, not(b), or(c, b))");
    }

    #[test]
    fn test_combination_condition_creation() {
        let and_condition =
            GuardCondition::and([GuardCondition::new("a"), GuardCondition::new("b")])
                .expect("failed to create 'and' combination condition");
        assert_eq!(format!("{}", and_condition), "and(a, b)");

        let or_condition = GuardCondition::or([GuardCondition::new("a"), GuardCondition::new("b")])
            .expect("failed to create 'or' combination condition");
        assert_eq!(format!("{}", or_condition), "or(a, b)");

        let not_condition = GuardCondition::not(GuardCondition::new("a"));
        assert_eq!(format!("{}", not_condition), "not(a)");
    }

    #[test]
    fn test_parse_error_handling() {
        assert!(GuardCondition::parse("and(a)").is_err());
        assert!(GuardCondition::parse("or(b)").is_err());
        assert!(GuardCondition::parse("").is_err());
        assert!(GuardCondition::parse("InvalidOp(a, b)").is_err());
        assert!(GuardCondition::parse("and(Op(a, b), c)").is_err());
    }
}
