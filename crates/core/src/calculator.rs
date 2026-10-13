use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Op(char),
    LParen,
    RParen,
    Percent,
}

pub fn evaluate(input: &str) -> AppResult<f64> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(AppError::new("CALC", "Expression is empty."));
    }
    let rpn = to_rpn(&tokens)?;
    eval_rpn(&rpn)
}

pub fn looks_like_math(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > 200 {
        return false;
    }
    let mut saw_digit = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            saw_digit = true;
            continue;
        }
        if matches!(ch, '+' | '-' | '*' | '/' | '(' | ')' | '%' | ' ') {
            continue;
        }
        return false;
    }
    saw_digit
        && trimmed
            .chars()
            .any(|ch| matches!(ch, '+' | '-' | '*' | '/' | '%' | '('))
}

fn tokenize(input: &str) -> AppResult<Vec<Token>> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let number = text
                .parse::<f64>()
                .map_err(|_| AppError::new("CALC", format!("Invalid number '{text}'.")))?;
            if !number.is_finite() {
                return Err(AppError::new("CALC", "Number is not finite."));
            }
            tokens.push(Token::Number(number));
            continue;
        }
        match ch {
            '+' | '*' | '/' => tokens.push(Token::Op(ch)),
            '-' => {
                let unary = tokens
                    .last()
                    .is_none_or(|token| matches!(token, Token::Op(_) | Token::LParen));
                if unary {
                    tokens.push(Token::Number(-1.0));
                    tokens.push(Token::Op('*'));
                } else {
                    tokens.push(Token::Op('-'));
                }
            }
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            '%' => tokens.push(Token::Percent),
            other => {
                return Err(AppError::new(
                    "CALC",
                    format!("Unsupported character '{other}'."),
                ));
            }
        }
        i += 1;
    }
    Ok(tokens)
}

fn precedence(op: char) -> i32 {
    match op {
        '+' | '-' => 1,
        '*' | '/' => 2,
        _ => 0,
    }
}

fn to_rpn(tokens: &[Token]) -> AppResult<Vec<Token>> {
    let mut output = Vec::new();
    let mut ops: Vec<Token> = Vec::new();
    for token in tokens {
        match token {
            Token::Number(_) => output.push(token.clone()),
            Token::Percent => output.push(Token::Percent),
            Token::Op(op) => {
                while let Some(Token::Op(top)) = ops.last() {
                    if precedence(*top) >= precedence(*op) {
                        output.push(ops.pop().unwrap());
                    } else {
                        break;
                    }
                }
                ops.push(Token::Op(*op));
            }
            Token::LParen => ops.push(Token::LParen),
            Token::RParen => {
                let mut found = false;
                while let Some(top) = ops.pop() {
                    if matches!(top, Token::LParen) {
                        found = true;
                        break;
                    }
                    output.push(top);
                }
                if !found {
                    return Err(AppError::new("CALC", "Missing opening parenthesis."));
                }
            }
        }
    }
    while let Some(top) = ops.pop() {
        if matches!(top, Token::LParen) {
            return Err(AppError::new("CALC", "Missing closing parenthesis."));
        }
        output.push(top);
    }
    Ok(output)
}

fn eval_rpn(tokens: &[Token]) -> AppResult<f64> {
    let mut stack = Vec::new();
    for token in tokens {
        match token {
            Token::Number(value) => stack.push(*value),
            Token::Percent => {
                let value = stack
                    .pop()
                    .ok_or_else(|| AppError::new("CALC", "Percent is missing a value."))?;
                stack.push(value / 100.0);
            }
            Token::Op(op) => {
                let right = stack
                    .pop()
                    .ok_or_else(|| AppError::new("CALC", "Expression is incomplete."))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| AppError::new("CALC", "Expression is incomplete."))?;
                let value = match op {
                    '+' => left + right,
                    '-' => left - right,
                    '*' => left * right,
                    '/' => {
                        if right == 0.0 {
                            return Err(AppError::new("CALC", "Division by zero."));
                        }
                        left / right
                    }
                    _ => return Err(AppError::new("CALC", "Unknown operator.")),
                };
                if !value.is_finite() {
                    return Err(AppError::new("CALC", "Result is not a finite number."));
                }
                stack.push(value);
            }
            Token::LParen | Token::RParen => {
                return Err(AppError::new("CALC", "Parentheses were not resolved."));
            }
        }
    }
    if stack.len() != 1 {
        return Err(AppError::new("CALC", "Expression is incomplete."));
    }
    Ok(stack[0])
}

pub fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let text = format!("{value:.10}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_arithmetic() {
        assert_eq!(evaluate("2+2").unwrap(), 4.0);
        assert_eq!(evaluate("(1+2)*3").unwrap(), 9.0);
        assert_eq!(evaluate("200*10%").unwrap(), 20.0);
        assert_eq!(evaluate("-4+10").unwrap(), 6.0);
        assert!(evaluate("2/0").is_err());
        assert!(evaluate("2+").is_err());
        assert!(!looks_like_math("notepad"));
        assert!(looks_like_math("2+2"));
    }
}
