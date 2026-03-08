use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use bevy::{
    ecs::system::{RegisteredSystemError, SystemId},
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};
use smallvec::SmallVec;

/// 状态条件的系统ID
///
/// 用于判断[`HsmState`]是否满足进入或退出的条件,其中上下文中的实体是当前检测的实体
///
/// State condition system ID
///
/// Used to determine if [`HsmState`] meets the conditions for entering or exiting, where the context entity is the entity currently being checked
pub type GuardId = SystemId<In<GuardContext>, bool>;

/// 注册用于判断[`HsmState`]是否满足进入或退出的条件
///
/// Register to determine if [`HsmState`] meets the conditions for entering or exiting
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn is_ok(entity:In<GuardContext>) -> bool {
/// #     true
/// # }
/// # fn foo(mut commands:Commands, mut guard_registry: ResMut<GuardRegistry>) {
/// let system_id = commands.register_system(is_ok);
/// guard_registry.insert("is_ok", system_id);
/// # }
/// ```
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuardRegistry(pub(super) HashMap<String, GuardId>);

impl GuardRegistry {
    pub fn to_combinator_condition_id(&self, condition: &GuardCondition) -> Option<CompiledGuard> {
        Some(match condition {
            GuardCondition::And(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                CompiledGuard::And(condition_ids)
            }
            GuardCondition::Or(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                CompiledGuard::Or(condition_ids)
            }
            GuardCondition::Not(condition) => {
                CompiledGuard::Not(Box::new(self.to_combinator_condition_id(condition)?))
            }
            GuardCondition::Id(condition_id) => CompiledGuard::Id(self.get(condition_id)?),
        })
    }

    /// 获取一个条件
    //
    /// Get a condition
    pub fn get<Q>(&self, name: &Q) -> Option<GuardId>
    where
        Q: Hash + Equivalent<String>,
    {
        self.0.get(name).cloned()
    }

    /// 插入一个条件
    ///
    /// Insert a condition
    pub fn insert(&mut self, name: impl Into<String>, condition_id: GuardId) -> Option<GuardId> {
        self.0.insert(name.into(), condition_id)
    }

    /// 移除一个条件
    ///
    /// Remove a condition
    pub fn remove<Q>(&mut self, name: &Q) -> Option<GuardId>
    where
        Q: Hash + Equivalent<String>,
    {
        self.0.remove(name)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

///# 组合守卫/Combined guard
///
/// 用于组合多个守卫，支持And、Or、Not操作。
///
/// Used to combine multiple guards, supporting And, Or, Not operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledGuard {
    And(SmallVec<[Box<CompiledGuard>; 2]>),
    Or(SmallVec<[Box<CompiledGuard>; 2]>),
    Not(Box<CompiledGuard>),
    Id(GuardId),
}

impl CompiledGuard {
    pub fn new(id: GuardId) -> Self {
        Self::Id(id)
    }

    pub fn add_and(self, condition: CompiledGuard) -> Self {
        if let Self::And(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::And(condition_ids)
        } else {
            Self::And(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    pub fn add_or(self, condition: CompiledGuard) -> Self {
        if let Self::Or(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::Or(condition_ids)
        } else {
            Self::Or(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    pub fn add_not(self) -> Self {
        match self {
            Self::Not(condition) => *condition,
            _ => Self::Not(Box::new(self)),
        }
    }

    pub fn run(
        &self,
        world: &mut World,
        input: GuardContext,
    ) -> Result<bool, RegisteredSystemError<In<GuardContext>, bool>> {
        match self {
            CompiledGuard::And(ids) => {
                for id in ids {
                    if !id.run(world, input)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            CompiledGuard::Or(ors) => {
                for id in ors {
                    if id.run(world, input)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            CompiledGuard::Not(not) => Ok(!not.run(world, input)?),
            CompiledGuard::Id(system_id) => world.run_system_with(*system_id, input),
        }
    }
}

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
/// let condition2 = GuardCondition::parse("And(condition_a, condition_b)").unwrap();
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
    Id(String),
}

impl GuardCondition {
    pub fn new(name: impl Into<String>) -> Self {
        Self::Id(name.into())
    }

    /// 创建一个and组合条件, 相同条件则合并
    ///
    /// Create an and combination condition, same condition will be merged
    pub fn and(conditions: impl IntoIterator<Item = Self>) -> Result<Self, &'static str> {
        let conditions: SmallVec<[Box<GuardCondition>; 2]> =
            conditions.into_iter().map(Box::new).collect();

        if conditions.len() < 2 {
            return Err("And condition must have at least 2 conditions");
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
            return Err("Or condition must have at least 2 conditions");
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
    ///- not_condition := `Not` `(` combination_condition `)`
    ///- and_condition := `And` `(` combination_condition `,` ( combination_condition )+ `)`
    ///- or_condition := `Or` `(` combination_condition `,` ( combination_condition )+ `)`
    ///- id_condition := ident
    pub fn parse(s: impl AsRef<str>) -> Result<Self, String> {
        let input = s.as_ref().trim();
        let mut parser = Parser::new(input);
        parser.parse_combination_condition()
    }
}

use std::str::Chars;

use crate::context::GuardContext;

// 词法分析器
struct Lexer<'a> {
    chars: Chars<'a>,
    current_char: Option<char>,
    position: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let current_char = chars.next();
        Self {
            chars,
            current_char,
            position: 0,
        }
    }

    pub fn peek(&self) -> Option<char> {
        self.current_char
    }

    fn advance(&mut self) {
        self.current_char = self.chars.next();
        self.position += 1;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current_char {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace();

        if let Some(c) = self.current_char {
            match c {
                '(' => {
                    self.advance();
                    Some(Token::LeftParen)
                }
                ')' => {
                    self.advance();
                    Some(Token::RightParen)
                }
                ',' => {
                    self.advance();
                    Some(Token::Comma)
                }
                c if c.is_alphabetic() => {
                    let mut identifier = String::new();
                    while let Some(ch) = self.current_char {
                        if ch.is_alphanumeric() || ch == '_' {
                            identifier.push(ch);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    Some(Token::Identifier(identifier))
                }
                _ => {
                    self.advance();
                    None
                }
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
enum Token {
    Identifier(String),
    LeftParen,
    RightParen,
    Comma,
}

// 语法分析器
struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Option<Token>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token();
        Self {
            lexer,
            current_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn expect_identifier(&mut self) -> Result<String, String> {
        match self.current_token.take() {
            Some(Token::Identifier(id)) => {
                self.advance();
                Ok(id)
            }
            _ => Err("combination_condition: expect identifier".to_string()),
        }
    }

    fn parse_combination_condition(&mut self) -> Result<GuardCondition, String> {
        match &self.current_token {
            Some(Token::Identifier(id)) if id == "Not" => self.parse_not_condition(),
            Some(Token::Identifier(id)) if id == "And" => self.parse_and_condition(),
            Some(Token::Identifier(id)) if id == "Or" => self.parse_or_condition(),
            Some(Token::Identifier(id)) => {
                let next_token = self.lexer.peek();
                if matches!(next_token, Some('(')) {
                    return Err(format!(
                        "combination_condition: invalid operator '{}', only 'And', 'Or', 'Not' are allowed",
                        id
                    ));
                }

                // 否则，这是一个普通的标识符
                let id = self.expect_identifier()?;
                Ok(GuardCondition::Id(id))
            }
            _ => Err("combination_condition: expect 'Not', 'And', 'Or' or identifier".to_string()),
        }
    }

    fn parse_not_condition(&mut self) -> Result<GuardCondition, String> {
        // 期望 "Not("
        self.expect_identifier()?; // "Not"
        if !matches!(self.current_token, Some(Token::LeftParen)) {
            return Err("combination_condition: expect '(' after 'Not'".to_string());
        }
        self.advance(); // '('

        let inner_condition = self.parse_combination_condition()?;

        if !matches!(self.current_token, Some(Token::RightParen)) {
            return Err("combination_condition: expect ')' after inner condition".to_string());
        }
        self.advance(); // ')'

        Ok(GuardCondition::Not(Box::new(inner_condition)))
    }

    fn parse_and_condition(&mut self) -> Result<GuardCondition, String> {
        // 期望 "And("
        self.expect_identifier()?; // "And"
        if !matches!(self.current_token, Some(Token::LeftParen)) {
            return Err("combination_condition: expect '(' after 'And'".to_string());
        }
        self.advance(); // '('

        let mut conditions = SmallVec::new();
        conditions.push(Box::new(self.parse_combination_condition()?));

        while matches!(self.current_token, Some(Token::Comma)) {
            self.advance(); // ','
            conditions.push(Box::new(self.parse_combination_condition()?));
        }

        if !matches!(self.current_token, Some(Token::RightParen)) {
            return Err("combination_condition: expect ')' after inner conditions".to_string());
        }
        self.advance(); // ')'

        if conditions.len() == 1 {
            Err("combination_condition: expect at least 2 conditions after 'And'".to_string())
        } else {
            Ok(GuardCondition::And(conditions))
        }
    }

    fn parse_or_condition(&mut self) -> Result<GuardCondition, String> {
        // 期望 "Or("
        self.expect_identifier()?; // "Or"
        if !matches!(self.current_token, Some(Token::LeftParen)) {
            return Err("combination_condition: expect '(' after 'Or'".to_string());
        }
        self.advance(); // '('

        let mut conditions = SmallVec::new();
        conditions.push(Box::new(self.parse_combination_condition()?));

        while matches!(self.current_token, Some(Token::Comma)) {
            self.advance(); // ','
            conditions.push(Box::new(self.parse_combination_condition()?));
        }

        if !matches!(self.current_token, Some(Token::RightParen)) {
            return Err("combination_condition: expect ')' after inner conditions".to_string());
        }
        self.advance(); // ')'

        if conditions.len() == 1 {
            Err("combination_condition: expect at least 2 conditions after 'Or'".to_string())
        } else {
            Ok(GuardCondition::Or(conditions))
        }
    }
}

impl Display for GuardCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardCondition::And(ands) => {
                let joined = ands
                    .iter()
                    .map(|x| format!("{}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "And({})", joined)
            }
            GuardCondition::Or(ors) => {
                let joined = ors
                    .iter()
                    .map(|x| format!("{}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "Or({})", joined)
            }
            GuardCondition::Not(not) => write!(f, "Not({})", not),
            GuardCondition::Id(id) => write!(f, "{}", id),
        }
    }
}

impl Debug for GuardCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}

impl From<String> for GuardCondition {
    fn from(value: String) -> Self {
        GuardCondition::Id(value)
    }
}

impl<'a> From<&'a str> for GuardCondition {
    fn from(value: &'a str) -> Self {
        GuardCondition::Id(value.into())
    }
}

impl Default for GuardCondition {
    fn default() -> Self {
        Self::Id("".into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bevy_hsm_macros::combination_condition;

    #[test]
    fn test_combination_condition() {
        // 测试从原子条件开始，添加AND条件
        // Test adding AND condition from atomic condition
        let conditions = GuardCondition::new("a").add_and(GuardCondition::new("b"));
        assert_eq!(
            conditions,
            GuardCondition::And(SmallVec::from_buf([
                Box::new(GuardCondition::new("a")),
                Box::new(GuardCondition::new("b")),
            ]))
        );

        // 从原子条件开始，添加OR条件
        // Test adding OR condition from atomic condition
        let conditions = GuardCondition::new("a").add_or(GuardCondition::new("c"));
        assert_eq!(
            conditions,
            GuardCondition::Or(SmallVec::from_buf([
                Box::new(GuardCondition::new("a")),
                Box::new(GuardCondition::new("c")),
            ]))
        );

        // 测试链式操作：(a AND b) OR c
        // Test chain operation: (a AND b) OR c
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
        assert_eq!(format!("{}", conditions), "And(a, b, c, d)");

        let a_conditions = GuardCondition::new("a").add_or(GuardCondition::new("b"));
        let b_conditions = GuardCondition::new("c").add_or(GuardCondition::new("d"));
        let conditions = a_conditions.add_or(b_conditions);
        assert_eq!(format!("{}", conditions), "Or(a, b, c, d)");
    }

    #[test]
    fn test_debug_combination_condition() {
        let conditions = GuardCondition::new("a")
            .add_and(GuardCondition::new("b"))
            .add_or(GuardCondition::new("c"));
        assert_eq!(format!("{}", conditions), "Or(And(a, b), c)");
        assert_eq!(format!("{:?}", conditions), "Or(And(a, b), c)");
    }

    #[test]
    fn test_hsm_combination_condition() {
        let and_condition = combination_condition!(and("a", "b"));
        assert_eq!(format!("{}", and_condition), "And(a, b)");

        let or_condition = combination_condition!(or("a", "b"));
        assert_eq!(format!("{}", or_condition), "Or(a, b)");

        let not_condition = combination_condition!(not("a"));
        assert_eq!(format!("{}", not_condition), "Not(a)");

        let id_condition = combination_condition!("a");
        assert_eq!(format!("{}", id_condition), "a");

        let combination_condition = combination_condition!(and(or("a", "b"), "c"));
        assert_eq!(format!("{}", combination_condition), "And(Or(a, b), c)");

        let combination_condition =
            combination_condition!(and(and_condition, not_condition, or_condition));
        assert_eq!(
            format!("{}", combination_condition),
            "And(And(a, b), Not(a), Or(a, b))"
        );
    }

    #[test]
    fn test_parse_combination_condition() {
        let condition = GuardCondition::parse("And(a, b)").unwrap();
        assert_eq!(format!("{}", condition), "And(a, b)");

        let condition = GuardCondition::parse("Or(a, b)").unwrap();
        assert_eq!(format!("{}", condition), "Or(a, b)");

        let condition = GuardCondition::parse("Not(a)").unwrap();
        assert_eq!(format!("{}", condition), "Not(a)");

        let condition = GuardCondition::parse("a").unwrap();
        assert_eq!(format!("{}", condition), "a");

        let condition = GuardCondition::parse("And(a, Not(b), Or(c, b))").unwrap();
        assert_eq!(format!("{}", condition), "And(a, Not(b), Or(c, b))");
    }

    #[test]
    fn test_combination_condition_creation() {
        // 测试新的构造方法
        // Test new construction method
        let and_condition =
            GuardCondition::and([GuardCondition::new("a"), GuardCondition::new("b")]).unwrap();
        assert_eq!(format!("{}", and_condition), "And(a, b)");

        let or_condition =
            GuardCondition::or([GuardCondition::new("a"), GuardCondition::new("b")]).unwrap();
        assert_eq!(format!("{}", or_condition), "Or(a, b)");

        let not_condition = GuardCondition::not(GuardCondition::new("a"));
        assert_eq!(format!("{}", not_condition), "Not(a)");
    }

    #[test]
    fn test_parse_error_handling() {
        // 测试错误处理
        // Test error handling
        // 至少需要2个条件
        // Need at least 2 conditions
        assert!(GuardCondition::parse("And(a)").is_err());
        // 至少需要2个条件
        // Need at least 2 conditions
        assert!(GuardCondition::parse("Or(b)").is_err());
        // 空输入
        // Empty input
        assert!(GuardCondition::parse("").is_err());
        // 无效的操作符
        // Invalid operator
        assert!(GuardCondition::parse("InvalidOp(a, b)").is_err());
        // 无效的操作符
        // Invalid operator
        assert!(GuardCondition::parse("And(Op(a, b), c)").is_err());
    }
}
