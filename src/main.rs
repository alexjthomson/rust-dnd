use rand::Rng;
use std::env;
use thiserror::Error;

fn main() -> Result<(), AppError> {
    let input = env::args().skip(1).collect::<Vec<_>>().join(" ");
    if input.trim().is_empty() {
        return Err(ParseError::MissingInput.into());
    }

    let expr = parse_dice_expression(&input)?;
    println!("AST:\n{expr:#?}");

    let mut rng = rand::thread_rng();
    let result = expr.eval(&mut rng)?;

    println!("Expression: {input}");

    if !result.dice_rolls.is_empty() {
        println!("Dice rolls:");
        for roll in &result.dice_rolls {
            println!("  {}", format_roll_record(roll));
        }
    }

    println!("Result: {}", result.value);
    Ok(())
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Eval(#[from] EvalError),
}

/// A parsed expression node.
#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Dice {
        count: Box<Expr>,
        sides: Box<Expr>,
        modifier: Option<DiceModifier>,
    },
    FunctionCall {
        name: FunctionName,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionName {
    Min,
    Max,
    Floor,
    Ceil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiceModifier {
    KeepHighest(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    D,
    Comma,
    LParen,
    RParen,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiceRollRecord {
    pub count: u64,
    pub sides: u64,
    pub rolls: Vec<u64>,
    pub kept: Vec<u64>,
    pub modifier: Option<DiceModifier>,
    pub total: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    pub value: f64,
    pub dice_rolls: Vec<DiceRollRecord>,
}

#[derive(Debug, Error, Clone)]
pub enum ParseError {
    #[error("unexpected character '{ch}' at byte index {index}")]
    UnexpectedChar { ch: char, index: usize },

    #[error("expected {expected}, found {found:?}")]
    UnexpectedToken {
        expected: &'static str,
        found: Token,
    },

    #[error("expected expression")]
    ExpectedExpression,

    #[error("invalid function name '{0}'")]
    InvalidFunctionName(String),

    #[error("invalid dice modifier '{0}'")]
    InvalidDiceModifier(String),

    #[error("expected command-line expression argument")]
    MissingInput,
}

#[derive(Debug, Error, Clone)]
pub enum EvalError {
    #[error("dice count must be a positive integer, got {0}")]
    InvalidDiceCount(f64),

    #[error("dice sides must be a positive integer, got {0}")]
    InvalidDiceSides(f64),

    #[error("keep-high count must be a non-negative integer, got {0}")]
    InvalidKeepHighestCount(f64),

    #[error("division by zero")]
    DivisionByZero,

    #[error("function {name} expected {expected} argument(s), got {got}")]
    InvalidFunctionArity {
        name: &'static str,
        expected: &'static str,
        got: usize,
    },

    #[error("function min requires at least 1 argument")]
    MinRequiresArgument,

    #[error("function max requires at least 1 argument")]
    MaxRequiresArgument,
}

/// Turns an input string into tokens.
pub struct Lexer<'a> {
    input: &'a str,
    chars: Vec<char>,
    index: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().collect(),
            index: 0,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, ParseError> {
        let mut tokens = Vec::new();

        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\n' | '\r' => {
                    self.index += 1;
                }
                '+' => {
                    self.index += 1;
                    tokens.push(Token::Plus);
                }
                '-' => {
                    self.index += 1;
                    tokens.push(Token::Minus);
                }
                '*' => {
                    self.index += 1;
                    tokens.push(Token::Star);
                }
                '/' => {
                    self.index += 1;
                    tokens.push(Token::Slash);
                }
                '^' => {
                    self.index += 1;
                    tokens.push(Token::Caret);
                }
                ',' => {
                    self.index += 1;
                    tokens.push(Token::Comma);
                }
                '(' => {
                    self.index += 1;
                    tokens.push(Token::LParen);
                }
                ')' => {
                    self.index += 1;
                    tokens.push(Token::RParen);
                }
                'd' | 'D' => {
                    self.index += 1;
                    tokens.push(Token::D);
                }
                '0'..='9' | '.' => {
                    tokens.push(self.lex_number()?);
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    tokens.push(self.lex_ident());
                }
                _ => {
                    return Err(ParseError::UnexpectedChar {
                        ch,
                        index: self.byte_index(),
                    });
                }
            }
        }

        tokens.push(Token::Eof);
        Ok(tokens)
    }

    fn lex_number(&mut self) -> Result<Token, ParseError> {
        let start = self.index;
        let mut seen_dot = false;

        while let Some(ch) = self.peek() {
            match ch {
                '0'..='9' => self.index += 1,
                '.' if !seen_dot => {
                    seen_dot = true;
                    self.index += 1;
                }
                _ => break,
            }
        }

        let text: String = self.chars[start..self.index].iter().collect();

        if text == "." {
            return Err(ParseError::UnexpectedChar {
                ch: '.',
                index: self.byte_index(),
            });
        }

        let number = text
            .parse::<f64>()
            .map_err(|_| ParseError::UnexpectedChar {
                ch: self.peek().unwrap_or('?'),
                index: self.byte_index(),
            })?;

        Ok(Token::Number(number))
    }

    fn lex_ident(&mut self) -> Token {
        let start = self.index;

        while let Some(ch) = self.peek() {
            match ch {
                'a'..='z' | 'A'..='Z' | '_' | '0'..='9' => self.index += 1,
                _ => break,
            }
        }

        let text: String = self.chars[start..self.index].iter().collect();
        Token::Ident(text)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.index)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len())
    }
}

/// Recursive-descent parser.
pub struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    pub fn parse(mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_expr()?;
        if self.peek() != &Token::Eof {
            return Err(ParseError::UnexpectedToken {
                expected: "end of input",
                found: self.peek().clone(),
            });
        }
        Ok(expr)
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_mul_div()?;

        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul_div()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_mul_div(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_power()?;

        loop {
            let op = match self.peek() {
                Token::Star => {
                    self.advance();
                    Some(BinaryOp::Mul)
                }
                Token::Slash => {
                    self.advance();
                    Some(BinaryOp::Div)
                }
                t if Self::starts_implicit_mul_rhs(t) => Some(BinaryOp::Mul),
                _ => None,
            };

            let Some(op) = op else {
                break;
            };

            let right = self.parse_power()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_power(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_unary()?;

        if self.peek() == &Token::Caret {
            self.advance();
            let right = self.parse_power()?;
            Ok(Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::Pow,
                right: Box::new(right),
            })
        } else {
            Ok(left)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Token::Plus => {
                self.advance();
                Ok(Expr::Unary {
                    op: UnaryOp::Plus,
                    expr: Box::new(self.parse_unary()?),
                })
            }
            Token::Minus => {
                self.advance();
                Ok(Expr::Unary {
                    op: UnaryOp::Minus,
                    expr: Box::new(self.parse_unary()?),
                })
            }
            _ => self.parse_dice(),
        }
    }

    fn parse_dice(&mut self) -> Result<Expr, ParseError> {
        let mut expr = if self.peek() == &Token::D {
            self.advance();
            let sides = self.parse_postfix_primary()?;
            let modifier = self.parse_dice_modifier()?;
            Expr::Dice {
                count: Box::new(Expr::Number(1.0)),
                sides: Box::new(sides),
                modifier,
            }
        } else {
            self.parse_postfix_primary()?
        };

        while self.peek() == &Token::D {
            self.advance();
            let sides = self.parse_postfix_primary()?;
            let modifier = self.parse_dice_modifier()?;
            expr = Expr::Dice {
                count: Box::new(expr),
                sides: Box::new(sides),
                modifier,
            };
        }

        Ok(expr)
    }

    fn parse_dice_modifier(&mut self) -> Result<Option<DiceModifier>, ParseError> {
        match self.peek().clone() {
            Token::Ident(name) => {
                let lower = name.to_ascii_lowercase();

                if let Some(rest) = lower.strip_prefix("kh") {
                    self.advance();

                    let count = if rest.is_empty() {
                        match self.peek().clone() {
                            Token::Number(n) => {
                                self.advance();
                                n
                            }
                            other => {
                                return Err(ParseError::UnexpectedToken {
                                    expected: "number after kh",
                                    found: other,
                                });
                            }
                        }
                    } else {
                        rest.parse::<f64>()
                            .map_err(|_| ParseError::InvalidDiceModifier(name.clone()))?
                    };

                    if count.fract() != 0.0 || count < 0.0 || count > u32::MAX as f64 {
                        return Err(ParseError::InvalidDiceModifier(name));
                    }

                    Ok(Some(DiceModifier::KeepHighest(count as u32)))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    fn parse_postfix_primary(&mut self) -> Result<Expr, ParseError> {
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(Token::RParen, "')'")?;
                Ok(expr)
            }
            Token::Ident(name) => self.parse_function_call(name),
            Token::Eof => Err(ParseError::ExpectedExpression),
            other => Err(ParseError::UnexpectedToken {
                expected: "number, identifier, or '('",
                found: other,
            }),
        }
    }

    fn parse_function_call(&mut self, name: String) -> Result<Expr, ParseError> {
        let function_name = match name.to_ascii_lowercase().as_str() {
            "min" => FunctionName::Min,
            "max" => FunctionName::Max,
            "floor" => FunctionName::Floor,
            "ceil" => FunctionName::Ceil,
            _ => return Err(ParseError::InvalidFunctionName(name)),
        };

        self.advance();
        self.expect(Token::LParen, "'(' after function name")?;

        let mut args = Vec::new();

        if self.peek() != &Token::RParen {
            loop {
                args.push(self.parse_expr()?);

                if self.peek() == &Token::Comma {
                    self.advance();
                    continue;
                }

                break;
            }
        }

        self.expect(Token::RParen, "')' after function arguments")?;

        Ok(Expr::FunctionCall {
            name: function_name,
            args,
        })
    }

    fn expect(&mut self, expected: Token, expected_name: &'static str) -> Result<(), ParseError> {
        if self.peek() == &expected {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                expected: expected_name,
                found: self.peek().clone(),
            })
        }
    }

    fn starts_implicit_mul_rhs(token: &Token) -> bool {
        matches!(
            token,
            Token::Number(_) | Token::LParen | Token::Ident(_) | Token::D
        )
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.index).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) {
        if self.index < self.tokens.len() {
            self.index += 1;
        }
    }
}

impl Expr {
    pub fn eval<R: Rng + ?Sized>(&self, rng: &mut R) -> Result<EvalResult, EvalError> {
        match self {
            Expr::Number(n) => Ok(EvalResult {
                value: *n,
                dice_rolls: Vec::new(),
            }),
            Expr::Unary { op, expr } => {
                let mut result = expr.eval(rng)?;
                match op {
                    UnaryOp::Plus => {}
                    UnaryOp::Minus => {
                        result.value = -result.value;
                    }
                }
                Ok(result)
            }
            Expr::Binary { left, op, right } => {
                let mut left_result = left.eval(rng)?;
                let right_result = right.eval(rng)?;
                left_result.dice_rolls.extend(right_result.dice_rolls);

                left_result.value = match op {
                    BinaryOp::Add => left_result.value + right_result.value,
                    BinaryOp::Sub => left_result.value - right_result.value,
                    BinaryOp::Mul => left_result.value * right_result.value,
                    BinaryOp::Div => {
                        if right_result.value == 0.0 {
                            return Err(EvalError::DivisionByZero);
                        }
                        left_result.value / right_result.value
                    }
                    BinaryOp::Pow => left_result.value.powf(right_result.value),
                };

                Ok(left_result)
            }
            Expr::Dice {
                count,
                sides,
                modifier,
            } => {
                let count_result = count.eval(rng)?;
                let sides_result = sides.eval(rng)?;

                let count_val = count_result.value;
                let sides_val = sides_result.value;

                if count_val <= 0.0 || count_val.fract() != 0.0 {
                    return Err(EvalError::InvalidDiceCount(count_val));
                }
                if sides_val <= 0.0 || sides_val.fract() != 0.0 {
                    return Err(EvalError::InvalidDiceSides(sides_val));
                }

                let count_u64 = count_val as u64;
                let sides_u64 = sides_val as u64;

                let mut rolls = Vec::with_capacity(count_u64 as usize);
                for _ in 0..count_u64 {
                    rolls.push(rng.gen_range(1..=sides_u64));
                }

                let kept = match modifier {
                    None => rolls.clone(),
                    Some(DiceModifier::KeepHighest(k)) => {
                        let k_f64 = *k as f64;
                        if k_f64.fract() != 0.0 || k_f64 < 0.0 {
                            return Err(EvalError::InvalidKeepHighestCount(k_f64));
                        }

                        let mut sorted = rolls.clone();
                        sorted.sort_unstable_by(|a, b| b.cmp(a));
                        sorted.truncate((*k as usize).min(sorted.len()));
                        sorted
                    }
                };

                let total = kept.iter().map(|&r| r as f64).sum::<f64>();

                let mut dice_rolls = count_result.dice_rolls;
                dice_rolls.extend(sides_result.dice_rolls);
                dice_rolls.push(DiceRollRecord {
                    count: count_u64,
                    sides: sides_u64,
                    rolls,
                    kept,
                    modifier: *modifier,
                    total,
                });

                Ok(EvalResult {
                    value: total,
                    dice_rolls,
                })
            }
            Expr::FunctionCall { name, args } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                let mut dice_rolls = Vec::new();

                for arg in args {
                    let result = arg.eval(rng)?;
                    dice_rolls.extend(result.dice_rolls);
                    evaluated_args.push(result.value);
                }

                let value = match name {
                    FunctionName::Min => {
                        if evaluated_args.is_empty() {
                            return Err(EvalError::MinRequiresArgument);
                        }
                        evaluated_args
                            .into_iter()
                            .reduce(f64::min)
                            .expect("checked non-empty")
                    }
                    FunctionName::Max => {
                        if evaluated_args.is_empty() {
                            return Err(EvalError::MaxRequiresArgument);
                        }
                        evaluated_args
                            .into_iter()
                            .reduce(f64::max)
                            .expect("checked non-empty")
                    }
                    FunctionName::Floor => {
                        if evaluated_args.len() != 1 {
                            return Err(EvalError::InvalidFunctionArity {
                                name: "floor",
                                expected: "1",
                                got: evaluated_args.len(),
                            });
                        }
                        evaluated_args[0].floor()
                    }
                    FunctionName::Ceil => {
                        if evaluated_args.len() != 1 {
                            return Err(EvalError::InvalidFunctionArity {
                                name: "ceil",
                                expected: "1",
                                got: evaluated_args.len(),
                            });
                        }
                        evaluated_args[0].ceil()
                    }
                };

                Ok(EvalResult { value, dice_rolls })
            }
        }
    }
}

pub fn parse_dice_expression(input: &str) -> Result<Expr, ParseError> {
    let tokens = Lexer::new(input).tokenize()?;
    Parser::new(tokens).parse()
}

fn format_roll_record(record: &DiceRollRecord) -> String {
    let base = format!("{}d{}", record.count, record.sides);

    match record.modifier {
        None => format!("{base}: rolls={:?}, total={}", record.rolls, record.total),
        Some(DiceModifier::KeepHighest(k)) => format!(
            "{base}kh{k}: rolls={:?}, kept={:?}, total={}",
            record.rolls, record.kept, record.total
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::mock::StepRng;

    fn eval_with_step_rng(input: &str) -> EvalResult {
        let expr = parse_dice_expression(input).expect("parse should succeed");
        let mut rng = StepRng::new(0, 1);
        expr.eval(&mut rng).expect("eval should succeed")
    }

    #[test]
    fn parses_plain_number() {
        let expr = parse_dice_expression("42").unwrap();
        match expr {
            Expr::Number(n) => assert_eq!(n, 42.0),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn parses_d20_shorthand() {
        let expr = parse_dice_expression("d20").unwrap();
        match expr {
            Expr::Dice {
                count,
                sides,
                modifier,
            } => {
                match *count {
                    Expr::Number(n) => assert_eq!(n, 1.0),
                    _ => panic!("expected numeric count"),
                }
                match *sides {
                    Expr::Number(n) => assert_eq!(n, 20.0),
                    _ => panic!("expected numeric sides"),
                }
                assert_eq!(modifier, None);
            }
            _ => panic!("expected dice expression"),
        }
    }

    #[test]
    fn parses_keep_highest_modifier() {
        let expr = parse_dice_expression("4d6kh3").unwrap();
        match expr {
            Expr::Dice { modifier, .. } => {
                assert_eq!(modifier, Some(DiceModifier::KeepHighest(3)));
            }
            _ => panic!("expected dice expression"),
        }
    }

    #[test]
    fn parses_keep_highest_with_space() {
        let expr = parse_dice_expression("4d6kh 3").unwrap();
        match expr {
            Expr::Dice { modifier, .. } => {
                assert_eq!(modifier, Some(DiceModifier::KeepHighest(3)));
            }
            _ => panic!("expected dice expression"),
        }
    }

    #[test]
    fn parses_keep_highest_uppercase() {
        let expr = parse_dice_expression("4d6KH3").unwrap();
        match expr {
            Expr::Dice { modifier, .. } => {
                assert_eq!(modifier, Some(DiceModifier::KeepHighest(3)));
            }
            _ => panic!("expected dice expression"),
        }
    }

    #[test]
    fn parses_functions() {
        let expr = parse_dice_expression("max(1, floor(2.9), ceil(3.1))").unwrap();
        match expr {
            Expr::FunctionCall {
                name: FunctionName::Max,
                args,
            } => {
                assert_eq!(args.len(), 3);
            }
            _ => panic!("expected function call"),
        }
    }

    #[test]
    fn implicit_multiplication_still_works() {
        let result = eval_with_step_rng("3(2 + 1)");
        assert_eq!(result.value, 9.0);
    }

    #[test]
    fn evaluates_floor_and_ceil() {
        let floor_result = eval_with_step_rng("floor(2.9)");
        let ceil_result = eval_with_step_rng("ceil(2.1)");
        assert_eq!(floor_result.value, 2.0);
        assert_eq!(ceil_result.value, 3.0);
    }

    #[test]
    fn evaluates_min_and_max() {
        let min_result = eval_with_step_rng("min(5, 3, 9)");
        let max_result = eval_with_step_rng("max(5, 3, 9)");
        assert_eq!(min_result.value, 3.0);
        assert_eq!(max_result.value, 9.0);
    }

    #[test]
    fn evaluates_d20() {
        let result = eval_with_step_rng("d20");
        assert_eq!(result.value, 1.0);
        assert_eq!(result.dice_rolls.len(), 1);
        assert_eq!(result.dice_rolls[0].count, 1);
        assert_eq!(result.dice_rolls[0].sides, 20);
        assert_eq!(result.dice_rolls[0].rolls, vec![1]);
    }

    #[test]
    fn evaluates_keep_highest() {
        let result = eval_with_step_rng("4d6kh3");
        assert_eq!(result.dice_rolls.len(), 1);

        let roll = &result.dice_rolls[0];
        assert_eq!(roll.rolls, vec![1, 1, 1, 1]);
        assert_eq!(roll.kept, vec![1, 1, 1]);
        assert_eq!(roll.total, 3.0);
        assert_eq!(result.value, 3.0);
    }

    #[test]
    fn evaluates_complex_expression_shape() {
        let result =
            eval_with_step_rng("1 / (((3(4d2 + 8d100) - 10) * 10) / 2 + 3d2 - 1d3 + 100)^1.5");
        assert!(result.value.is_finite());
        assert_eq!(result.dice_rolls.len(), 4);
    }

    #[test]
    fn division_by_zero_errors() {
        let expr = parse_dice_expression("1 / 0").unwrap();
        let mut rng = StepRng::new(0, 1);
        let err = expr.eval(&mut rng).unwrap_err();
        match err {
            EvalError::DivisionByZero => {}
            _ => panic!("expected division by zero"),
        }
    }

    #[test]
    fn invalid_floor_arity_errors() {
        let expr = parse_dice_expression("floor(1, 2)").unwrap();
        let mut rng = StepRng::new(0, 1);
        let err = expr.eval(&mut rng).unwrap_err();
        match err {
            EvalError::InvalidFunctionArity { name, got, .. } => {
                assert_eq!(name, "floor");
                assert_eq!(got, 2);
            }
            _ => panic!("expected invalid function arity"),
        }
    }

    #[test]
    fn invalid_function_name_errors() {
        let err = parse_dice_expression("sqrt(4)").unwrap_err();
        match err {
            ParseError::InvalidFunctionName(name) => assert_eq!(name, "sqrt"),
            _ => panic!("expected invalid function name"),
        }
    }
}
