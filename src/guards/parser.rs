use std::str::Chars;

use super::GuardConditionParseError;
use super::condition::GuardCondition;
use crate::labels::SystemLabel;

/// 表示解析器可以识别的词法单元。
#[derive(Debug, Clone)]
pub(super) enum Token {
    Identifier(String),
    LeftParen,
    RightParen,
    Comma,
}

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

/// 用于解析守卫条件的语法分析器。
///
/// `Parser` 从 `Lexer` 获取 `Token`，并根据预定义的语法规则构建 `GuardCondition`。
pub(super) struct Parser<'a> {
    lexer: Lexer<'a>,
    pub current_token: Option<Token>,
}

impl<'a> Parser<'a> {
    /// 创建一个新的 `Parser`。
    pub fn new(input: &'a str) -> Self {
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
    pub fn parse_combination_condition(
        &mut self,
    ) -> Result<GuardCondition, GuardConditionParseError> {
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

use smallvec::SmallVec;
