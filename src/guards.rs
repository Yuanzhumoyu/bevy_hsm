use std::{
    borrow::Borrow,
    fmt::{Debug, Display},
    hash::Hash,
    str::FromStr,
};

use bevy::{
    ecs::system::{RegisteredSystemError, SystemId},
    platform::collections::{Equivalent, HashMap},
    prelude::*,
};
use smallvec::SmallVec;

/// 解析 GuardCondition 时的错误类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GuardConditionParseError {
    EmptyInput,
    UnexpectedToken(String),
    UnexpectedEOF,
    InvalidOperator(String),
    TooFewOperands(String),
    TrailingToken(String),
}

impl std::fmt::Display for GuardConditionParseError {
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
        }
    }
}

impl std::error::Error for GuardConditionParseError {}

impl FromStr for GuardCondition {
    type Err = GuardConditionParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Guard 解析/解析到已注册系统 ID 时可能出现的错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardResolveError {
    UnregisteredGuard(SystemLabel),
}

impl std::fmt::Display for GuardResolveError {
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
pub type GuardId = SystemId<In<GuardContext>, bool>;

/// 注册用于判断`State`是否满足进入或退出的条件
///
/// Register to determine if `State` meets the conditions for entering or exiting
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn is_ok(_:In<GuardContext>) -> bool {
/// #     true
/// # }
/// # fn foo(mut commands:Commands, mut guard_registry: ResMut<GuardRegistry>) {
/// let system_id = commands.register_system(is_ok);
/// guard_registry.insert("is_ok", system_id);
/// # }
/// ```
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuardRegistry(pub(super) HashMap<SystemLabel, GuardId>);

impl GuardRegistry {
    pub fn to_combinator_condition_id(
        &self,
        condition: &GuardCondition,
    ) -> Result<CompiledGuard, GuardResolveError> {
        match condition {
            GuardCondition::And(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                Ok(CompiledGuard::And(condition_ids))
            }
            GuardCondition::Or(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                Ok(CompiledGuard::Or(condition_ids))
            }
            GuardCondition::Not(condition) => Ok(CompiledGuard::Not(Box::new(
                self.to_combinator_condition_id(condition)?,
            ))),
            GuardCondition::Id(condition_id) => {
                let id = self
                    .get(condition_id)
                    .ok_or_else(|| GuardResolveError::UnregisteredGuard(condition_id.clone()))?;
                Ok(CompiledGuard::Id(id))
            }
        }
    }

    /// 获取一个条件
    //
    /// Get a condition
    pub fn get<Q>(&self, name: &Q) -> Option<GuardId>
    where
        Q: Hash + Equivalent<SystemLabel> + ?Sized,
    {
        self.0.get(name).cloned()
    }

    /// 插入一个条件
    ///
    /// Insert a condition
    pub fn insert(
        &mut self,
        name: impl Into<SystemLabel>,
        condition_id: GuardId,
    ) -> Option<GuardId> {
        self.0.insert(name.into(), condition_id)
    }

    /// 移除一个条件
    ///
    /// Remove a condition
    pub fn remove<Q>(&mut self, name: &Q) -> Option<GuardId>
    where
        Q: Hash + Equivalent<SystemLabel>,
        SystemLabel: Borrow<Q>,
    {
        self.0.remove(name)
    }

    /// 获取已注册守卫的数量
    ///
    /// Get the number of registered guards
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 检查守卫注册表是否为空
    ///
    /// Check if the guard registry is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<S: Into<SystemLabel>> Extend<(S, GuardId)> for GuardRegistry {
    fn extend<T: IntoIterator<Item = (S, GuardId)>>(&mut self, iter: T) {
        self.0.extend(iter.into_iter().map(|(s, a)| (s.into(), a)));
    }
}

impl<S: Into<SystemLabel>, const N: usize> From<[(S, GuardId); N]> for GuardRegistry {
    fn from(value: [(S, GuardId); N]) -> Self {
        Self(HashMap::from(value.map(|(s, a)| (s.into(), a))))
    }
}

/// # 编译后的组合守卫
///
/// * 用于在运行时执行的已编译的守卫条件。
///   [`CompiledGuard`] 是从 [`GuardCondition`] 编译而来的，它将守卫的逻辑（如 `and`, `or`, `not`）
///   与实际的 `SystemId` 结合起来，以便在状态转换时高效地执行。
///
/// # Compiled Combined Guard
///
/// * A compiled guard condition for execution at runtime.
///   [`CompiledGuard`] is compiled from [`GuardCondition`] and combines guard logic (like `and`, `or`, `not`)
///   with the actual `SystemId` for efficient execution during state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledGuard {
    And(SmallVec<[Box<CompiledGuard>; 2]>),
    Or(SmallVec<[Box<CompiledGuard>; 2]>),
    Not(Box<CompiledGuard>),
    Id(GuardId),
}

impl CompiledGuard {
    /// 从一个 `GuardId` 创建一个新的 `CompiledGuard`。
    ///
    /// Creates a new `CompiledGuard` from a `GuardId`.
    pub fn new(id: GuardId) -> Self {
        Self::Id(id)
    }

    /// 添加一个 `AND` 条件。
    ///
    /// Adds an `AND` condition.
    pub fn add_and(self, condition: CompiledGuard) -> Self {
        if let Self::And(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::And(condition_ids)
        } else {
            Self::And(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    /// 添加一个 `OR` 条件。
    ///
    /// Adds an `OR` condition.
    pub fn add_or(self, condition: CompiledGuard) -> Self {
        if let Self::Or(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::Or(condition_ids)
        } else {
            Self::Or(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
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

    /// 在给定的 `World` 中运行守卫条件。
    ///
    /// Runs the guard condition in the given `World`.
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
            CompiledGuard::Id(system_id) => input.queue_system_command(*system_id).apply(world),
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
                let joined = ands
                    .iter()
                    .map(|x| format!("{}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "and({})", joined)
            }
            GuardCondition::Or(ors) => {
                let joined = ors
                    .iter()
                    .map(|x| format!("{}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "or({})", joined)
            }
            GuardCondition::Not(not) => write!(f, "not({})", not),
            GuardCondition::Id(id) => write!(f, "{}", id),
        }
    }
}

impl Debug for GuardCondition {
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

use std::str::Chars;

use crate::{context::GuardContext, labels::SystemLabel};

/// 用于解析守卫条件的词法分析器。
///
/// `Lexer` 将输入的字符串分解为一系列的 `Token`，为 `Parser` 提供基础。
struct Lexer<'a> {
    chars: Chars<'a>,
    current_char: Option<char>,
}

impl<'a> Lexer<'a> {
    /// 创建一个新的 `Lexer`。
    fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let current_char = chars.next();
        Self {
            chars,
            current_char,
        }
    }

    /// 查看下一个字符而不消耗它。
    pub fn peek(&self) -> Option<char> {
        self.current_char
    }

    /// 向前移动一个字符。
    fn advance(&mut self) {
        self.current_char = self.chars.next();
    }

    /// 跳过空白字符。
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current_char {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// 获取下一个 `Token`。
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

/// 表示解析器可以识别的词法单元。
#[derive(Debug, Clone)]
enum Token {
    Identifier(String),
    LeftParen,
    RightParen,
    Comma,
}

/// 用于解析守卫条件的语法分析器。
///
/// `Parser` 从 `Lexer` 获取 `Token`，并根据预定义的语法规则构建 `GuardCondition`。
struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Option<Token>,
}

impl<'a> Parser<'a> {
    /// 创建一个新的 `Parser`。
    fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token();
        Self {
            lexer,
            current_token,
        }
    }

    /// 向前移动一个 `Token`。
    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    /// 期望并消耗一个标识符 `Token`。
    fn expect_identifier(&mut self) -> Result<String, GuardConditionParseError> {
        match self.current_token.take() {
            Some(Token::Identifier(id)) => {
                self.advance();
                Ok(id)
            }
            Some(tok) => Err(GuardConditionParseError::UnexpectedToken(format!(
                "{:?}",
                tok
            ))),
            None => Err(GuardConditionParseError::UnexpectedEOF),
        }
    }

    /// 解析一个组合条件。
    fn parse_combination_condition(&mut self) -> Result<GuardCondition, GuardConditionParseError> {
        match &self.current_token {
            Some(Token::Identifier(id)) if id == "not" => self.parse_not_condition(),
            Some(Token::Identifier(id)) if id == "and" => self.parse_and_condition(),
            Some(Token::Identifier(id)) if id == "or" => self.parse_or_condition(),
            Some(Token::Identifier(id)) => {
                let next_token = self.lexer.peek();
                if matches!(next_token, Some('(')) {
                    return Err(GuardConditionParseError::InvalidOperator(id.clone()));
                }
                // 否则，这是一个普通的标识符
                let id = self.expect_identifier()?;
                Ok(GuardCondition::Id(SystemLabel::from(id)))
            }
            Some(tok) => Err(GuardConditionParseError::UnexpectedToken(format!(
                "{:?}",
                tok
            ))),
            None => Err(GuardConditionParseError::UnexpectedEOF),
        }
    }

    /// 解析一个 `NOT` 条件。
    fn parse_not_condition(&mut self) -> Result<GuardCondition, GuardConditionParseError> {
        // 期望 "not("
        self.expect_identifier()?; // "not"
        if !matches!(self.current_token, Some(Token::LeftParen)) {
            return Err(GuardConditionParseError::UnexpectedToken(
                "expected '(' after 'not'".to_string(),
            ));
        }
        self.advance(); // '('

        let inner_condition = self.parse_combination_condition()?;

        if !matches!(self.current_token, Some(Token::RightParen)) {
            return Err(GuardConditionParseError::UnexpectedToken(
                "expected ')' after inner condition".to_string(),
            ));
        }
        self.advance(); // ')'

        Ok(GuardCondition::Not(Box::new(inner_condition)))
    }

    /// 解析一个 `AND` 条件。
    fn parse_and_condition(&mut self) -> Result<GuardCondition, GuardConditionParseError> {
        // 期望 "and("
        self.expect_identifier()?; // "and"
        if !matches!(self.current_token, Some(Token::LeftParen)) {
            return Err(GuardConditionParseError::UnexpectedToken(
                "expected '(' after 'and'".to_string(),
            ));
        }
        self.advance(); // '('

        let mut conditions = SmallVec::new();
        conditions.push(Box::new(self.parse_combination_condition()?));

        while matches!(self.current_token, Some(Token::Comma)) {
            self.advance(); // ','
            conditions.push(Box::new(self.parse_combination_condition()?));
        }

        if !matches!(self.current_token, Some(Token::RightParen)) {
            return Err(GuardConditionParseError::UnexpectedToken(
                "expected ')' after inner conditions".to_string(),
            ));
        }
        self.advance(); // ')'

        if conditions.len() == 1 {
            Err(GuardConditionParseError::TooFewOperands("and".to_string()))
        } else {
            Ok(GuardCondition::And(conditions))
        }
    }

    /// 解析一个 `OR` 条件。
    fn parse_or_condition(&mut self) -> Result<GuardCondition, GuardConditionParseError> {
        // 期望 "or("
        self.expect_identifier()?; // "or"
        if !matches!(self.current_token, Some(Token::LeftParen)) {
            return Err(GuardConditionParseError::UnexpectedToken(
                "expected '(' after 'or'".to_string(),
            ));
        }
        self.advance(); // '('

        let mut conditions = SmallVec::new();
        conditions.push(Box::new(self.parse_combination_condition()?));

        while matches!(self.current_token, Some(Token::Comma)) {
            self.advance(); // ','
            conditions.push(Box::new(self.parse_combination_condition()?));
        }

        if !matches!(self.current_token, Some(Token::RightParen)) {
            return Err(GuardConditionParseError::UnexpectedToken(
                "expected ')' after inner conditions".to_string(),
            ));
        }
        self.advance(); // ')'

        if conditions.len() == 1 {
            Err(GuardConditionParseError::TooFewOperands("or".to_string()))
        } else {
            Ok(GuardCondition::Or(conditions))
        }
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
        // 测试新的构造方法
        // Test new construction method
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
        // 测试错误处理
        // Test error handling
        // 至少需要2个条件
        // Need at least 2 conditions
        assert!(GuardCondition::parse("and(a)").is_err());
        // 至少需要2个条件
        // Need at least 2 conditions
        assert!(GuardCondition::parse("or(b)").is_err());
        // 空输入
        // Empty input
        assert!(GuardCondition::parse("").is_err());
        // 无效的操作符
        // Invalid operator
        assert!(GuardCondition::parse("InvalidOp(a, b)").is_err());
        // 无效的操作符
        // Invalid operator
        assert!(GuardCondition::parse("and(Op(a, b), c)").is_err());
    }
}
