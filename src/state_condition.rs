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

use crate::prelude::HsmStateConditionContext;

/// 状态条件的系统ID
///
/// 用于判断[`HsmState`]是否满足进入或退出的条件,其中上下文中的实体是当前检测的实体
///
/// State condition system ID
///
/// Used to determine if [`HsmState`] meets the conditions for entering or exiting, where the context entity is the entity currently being checked
pub type StateConditionId = SystemId<In<HsmStateConditionContext>, bool>;

/// 注册用于判断[`HsmState`]是否满足进入或退出的条件
///
/// Register to determine if [`HsmState`] meets the conditions for entering or exiting
/// ```
/// # use bevy::prelude::*;
/// # use bevy_hsm::prelude::*;
/// # fn is_ok(entity:In<HsmStateContext>) -> bool {
/// #     true
/// # }
/// # fn foo(mut commands:Commands, mut state_conditions: ResMut<StateConditions>) {
/// let system_id = commands.register_system(is_ok);
/// state_conditions.insert("is_ok", system_id);
/// # }
/// ```
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct StateConditions(pub(super) HashMap<String, StateConditionId>);

impl StateConditions {
    pub fn to_combinator_condition_id(
        &self,
        condition: &CombinationCondition,
    ) -> Option<CombinationConditionId> {
        Some(match condition {
            CombinationCondition::And(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                CombinationConditionId::And(condition_ids)
            }
            CombinationCondition::Or(conditions) => {
                let mut condition_ids = SmallVec::new();
                for condition in conditions {
                    condition_ids.push(Box::new(self.to_combinator_condition_id(condition)?));
                }
                CombinationConditionId::Or(condition_ids)
            }
            CombinationCondition::Not(condition) => {
                CombinationConditionId::Not(Box::new(self.to_combinator_condition_id(condition)?))
            }
            CombinationCondition::Id(condition_id) => {
                CombinationConditionId::Id(self.get(condition_id)?)
            }
        })
    }

    /// 获取一个条件
    //
    /// Get a condition
    pub fn get<Q>(&self, name: &Q) -> Option<StateConditionId>
    where
        Q: Hash + Equivalent<String>,
    {
        self.0.get(name).cloned()
    }

    /// 插入一个条件
    ///
    /// Insert a condition
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        condition_id: StateConditionId,
    ) -> Option<StateConditionId> {
        self.0.insert(name.into(), condition_id)
    }

    /// 移除一个条件
    ///
    /// Remove a condition
    pub fn remove<Q>(&mut self, name: &Q) -> Option<StateConditionId>
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

/// 进入该状态的条件
///
/// Condition for entering this state
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct HsmOnEnterCondition(pub CombinationCondition);

impl HsmOnEnterCondition {
    pub fn new(name: impl Into<String>) -> Self {
        Self(CombinationCondition::Id(name.into()))
    }
}

/// 退出该状态的条件
///
/// Condition for exiting this state
#[derive(Component, PartialEq, Eq, Default, Debug, Deref, DerefMut)]
pub struct HsmOnExitCondition(pub CombinationCondition);

impl HsmOnExitCondition {
    pub fn new(name: impl Into<String>) -> Self {
        Self(CombinationCondition::Id(name.into()))
    }

    pub fn parse(s: impl AsRef<str>) -> Result<Self> {
        Ok(Self(CombinationCondition::parse(s)?))
    }
}

/// 组合条件ID
///
/// Combination condition ID
#[derive(Clone, PartialEq, Eq)]
pub enum CombinationConditionId {
    And(SmallVec<[Box<CombinationConditionId>; 2]>),
    Or(SmallVec<[Box<CombinationConditionId>; 2]>),
    Not(Box<CombinationConditionId>),
    Id(StateConditionId),
}

impl CombinationConditionId {
    pub fn new(id: StateConditionId) -> Self {
        Self::Id(id)
    }

    pub fn add_and(self, condition: CombinationConditionId) -> Self {
        if let Self::And(mut condition_ids) = self {
            condition_ids.push(Box::new(condition));
            Self::And(condition_ids)
        } else {
            Self::And(SmallVec::from_buf([Box::new(self), Box::new(condition)]))
        }
    }

    pub fn add_or(self, condition: CombinationConditionId) -> Self {
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
        input: HsmStateConditionContext,
    ) -> Result<bool, RegisteredSystemError<In<HsmStateConditionContext>, bool>>{
        match self {
            CombinationConditionId::And(ids) => {
                for id in ids {
                    if !id.run(world, input)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            CombinationConditionId::Or(ors) => {
                for id in ors {
                    if id.run(world, input)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            CombinationConditionId::Not(not) => not.run(world, input),
            CombinationConditionId::Id(system_id) => world.run_system_with(*system_id, input),
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
/// let condition2 = CombinationCondition::parse("And(condition_a, condition_b)").unwrap();
///
/// // 使用构造方法创建
/// // Using the constructor method to create   
/// let condition3 = CombinationCondition::new("condition_a").add_and(CombinationCondition::new("condition_b"));
///
/// assert_eq!(condition1, condition3);
/// assert_eq!(condition2, condition3);
/// # }
/// ```
#[derive(Clone, PartialEq, Eq)]
pub enum CombinationCondition {
    And(SmallVec<[Box<CombinationCondition>; 2]>),
    Or(SmallVec<[Box<CombinationCondition>; 2]>),
    Not(Box<CombinationCondition>),
    Id(String),
}

impl CombinationCondition {
    pub fn new(name: impl Into<String>) -> Self {
        Self::Id(name.into())
    }

    /// 创建一个and组合条件, 相同条件则合并
    ///
    /// Create an and combination condition, same condition will be merged
    pub fn and(conditions: impl IntoIterator<Item = Self>) -> Self {
        let conditions: SmallVec<[Box<CombinationCondition>; 2]> =
            conditions.into_iter().map(Box::new).collect();

        if conditions.len() < 2 {
            panic!("And condition must have at least 2 conditions");
        }

        CombinationCondition::And(conditions)
    }

    /// 创建一个or组合条件, 相同条件则合并
    ///
    /// Create an or combination condition, same condition will be merged
    pub fn or(conditions: impl IntoIterator<Item = Self>) -> Self {
        let conditions: SmallVec<[Box<CombinationCondition>; 2]> =
            conditions.into_iter().map(Box::new).collect();

        if conditions.len() < 2 {
            panic!("Or condition must have at least 2 conditions");
        }

        CombinationCondition::Or(conditions)
    }

    /// 创建一个not组合条件，相同条件则不变
    ///
    /// Create a not combination condition, same condition will not change
    #[inline(always)]
    #[allow(clippy::should_implement_trait)]
    pub fn not(condition: CombinationCondition) -> Self {
        condition.add_not()
    }

    pub fn add_and(self, condition: CombinationCondition) -> Self {
        match (self, condition) {
            (Self::And(l), Self::And(r)) => Self::And(l.into_iter().chain(r).collect()),
            (l, r) => Self::And(SmallVec::from_buf([Box::new(l), Box::new(r)])),
        }
    }

    pub fn add_or(self, condition: CombinationCondition) -> Self {
        match (self, condition) {
            (Self::Or(l), Self::Or(r)) => Self::Or(l.into_iter().chain(r).collect()),
            (l, r) => Self::Or(SmallVec::from_buf([Box::new(l), Box::new(r)])),
        }
    }

    pub fn add_not(self) -> Self {
        match self {
            Self::Not(condition) => *condition,
            _ => Self::Not(Box::new(self)),
        }
    }
}

impl CombinationCondition {
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

    fn parse_combination_condition(&mut self) -> Result<CombinationCondition, String> {
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
                Ok(CombinationCondition::Id(id))
            }
            _ => Err("combination_condition: expect 'Not', 'And', 'Or' or identifier".to_string()),
        }
    }

    fn parse_not_condition(&mut self) -> Result<CombinationCondition, String> {
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

        Ok(CombinationCondition::Not(Box::new(inner_condition)))
    }

    fn parse_and_condition(&mut self) -> Result<CombinationCondition, String> {
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
            Ok(CombinationCondition::And(conditions))
        }
    }

    fn parse_or_condition(&mut self) -> Result<CombinationCondition, String> {
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
            Ok(CombinationCondition::Or(conditions))
        }
    }
}

impl Display for CombinationCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CombinationCondition::And(ands) => {
                let joined = ands
                    .iter()
                    .map(|x| format!("{}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "And({})", joined)
            }
            CombinationCondition::Or(ors) => {
                let joined = ors
                    .iter()
                    .map(|x| format!("{}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "Or({})", joined)
            }
            CombinationCondition::Not(not) => write!(f, "Not({})", not),
            CombinationCondition::Id(id) => write!(f, "{}", id),
        }
    }
}

impl Debug for CombinationCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}

impl From<String> for CombinationCondition {
    fn from(value: String) -> Self {
        CombinationCondition::Id(value)
    }
}

impl<'a> From<&'a str> for CombinationCondition {
    fn from(value: &'a str) -> Self {
        CombinationCondition::Id(value.into())
    }
}

impl Default for CombinationCondition {
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
        let conditions = CombinationCondition::new("a").add_and(CombinationCondition::new("b"));
        assert_eq!(
            conditions,
            CombinationCondition::And(SmallVec::from_buf([
                Box::new(CombinationCondition::new("a")),
                Box::new(CombinationCondition::new("b")),
            ]))
        );

        // 从原子条件开始，添加OR条件
        // Test adding OR condition from atomic condition
        let conditions = CombinationCondition::new("a").add_or(CombinationCondition::new("c"));
        assert_eq!(
            conditions,
            CombinationCondition::Or(SmallVec::from_buf([
                Box::new(CombinationCondition::new("a")),
                Box::new(CombinationCondition::new("c")),
            ]))
        );

        // 测试链式操作：(a AND b) OR c
        // Test chain operation: (a AND b) OR c
        let conditions = CombinationCondition::new("a")
            .add_and(CombinationCondition::new("b"))
            .add_or(CombinationCondition::new("c"));
        assert_eq!(
            conditions,
            CombinationCondition::Or(SmallVec::from_buf([
                Box::new(CombinationCondition::And(SmallVec::from_buf([
                    Box::new(CombinationCondition::new("a")),
                    Box::new(CombinationCondition::new("b")),
                ]))),
                Box::new(CombinationCondition::new("c")),
            ]))
        );

        let a_conditions = CombinationCondition::new("a").add_and(CombinationCondition::new("b"));
        let b_conditions = CombinationCondition::new("c").add_and(CombinationCondition::new("d"));
        let conditions = a_conditions.add_and(b_conditions);
        assert_eq!(format!("{}", conditions), "And(a, b, c, d)");

        let a_conditions = CombinationCondition::new("a").add_or(CombinationCondition::new("b"));
        let b_conditions = CombinationCondition::new("c").add_or(CombinationCondition::new("d"));
        let conditions = a_conditions.add_or(b_conditions);
        assert_eq!(format!("{}", conditions), "Or(a, b, c, d)");
    }

    #[test]
    fn test_debug_combination_condition() {
        let conditions = CombinationCondition::new("a")
            .add_and(CombinationCondition::new("b"))
            .add_or(CombinationCondition::new("c"));
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
        let condition = CombinationCondition::parse("And(a, b)").unwrap();
        assert_eq!(format!("{}", condition), "And(a, b)");

        let condition = CombinationCondition::parse("Or(a, b)").unwrap();
        assert_eq!(format!("{}", condition), "Or(a, b)");

        let condition = CombinationCondition::parse("Not(a)").unwrap();
        assert_eq!(format!("{}", condition), "Not(a)");

        let condition = CombinationCondition::parse("a").unwrap();
        assert_eq!(format!("{}", condition), "a");

        let condition = CombinationCondition::parse("And(a, Not(b), Or(c, b))").unwrap();
        assert_eq!(format!("{}", condition), "And(a, Not(b), Or(c, b))");
    }

    #[test]
    fn test_combination_condition_creation() {
        // 测试新的构造方法
        // Test new construction method
        let and_condition = CombinationCondition::and([
            CombinationCondition::new("a"),
            CombinationCondition::new("b"),
        ]);
        assert_eq!(format!("{}", and_condition), "And(a, b)");

        let or_condition = CombinationCondition::or([
            CombinationCondition::new("a"),
            CombinationCondition::new("b"),
        ]);
        assert_eq!(format!("{}", or_condition), "Or(a, b)");

        let not_condition = CombinationCondition::not(CombinationCondition::new("a"));
        assert_eq!(format!("{}", not_condition), "Not(a)");
    }

    #[test]
    fn test_parse_error_handling() {
        // 测试错误处理
        // Test error handling
        // 至少需要2个条件
        // Need at least 2 conditions
        assert!(CombinationCondition::parse("And(a)").is_err());
        // 至少需要2个条件
        // Need at least 2 conditions
        assert!(CombinationCondition::parse("Or(b)").is_err());
        // 空输入
        // Empty input
        assert!(CombinationCondition::parse("").is_err());
        // 无效的操作符
        // Invalid operator
        assert!(CombinationCondition::parse("InvalidOp(a, b)").is_err());
        // 无效的操作符
        // Invalid operator
        assert!(CombinationCondition::parse("And(Op(a, b), c)").is_err());
    }
}
