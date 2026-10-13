//! Small, safe expression evaluator for the launcher calculator. No `eval`, no external input
//! beyond the typed expression.

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    Pow,
    LParen,
    RParen,
    Comma,
}

fn lex(s: &str) -> Option<Vec<Tok>> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        match ch {
            ' ' | '\t' => i += 1,
            '0'..='9' | '.' => {
                let st = i;
                while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                    i += 1;
                }
                if i < c.len() && (c[i] == 'e' || c[i] == 'E') && i + 1 < c.len() && (c[i + 1].is_ascii_digit() || ((c[i + 1] == '-' || c[i + 1] == '+') && i + 2 < c.len() && c[i + 2].is_ascii_digit())) {
                    i += 2;
                    while i < c.len() && c[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let t: String = c[st..i].iter().collect();
                out.push(Tok::Num(t.parse().ok()?));
            }
            '+' | '-' | '/' | '%' | '\u{00d7}' | '\u{00f7}' => {
                out.push(Tok::Op(match ch {
                    '\u{00d7}' => '*',
                    '\u{00f7}' => '/',
                    o => o,
                }));
                i += 1;
            }
            '*' => {
                if i + 1 < c.len() && c[i + 1] == '*' {
                    out.push(Tok::Pow);
                    i += 2;
                } else {
                    out.push(Tok::Op('*'));
                    i += 1;
                }
            }
            'x' | 'X' if matches!(out.last(), Some(Tok::Num(_)) | Some(Tok::RParen)) && !(i + 1 < c.len() && c[i + 1].is_alphanumeric()) => {
                out.push(Tok::Op('*'));
                i += 1;
            }
            '^' => {
                out.push(Tok::Pow);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            a if a.is_alphabetic() => {
                let st = i;
                while i < c.len() && c[i].is_alphanumeric() {
                    i += 1;
                }
                out.push(Tok::Ident(c[st..i].iter().collect::<String>().to_lowercase()));
            }
            _ => return None,
        }
    }
    Some(out)
}

struct P {
    t: Vec<Tok>,
    i: usize,
    depth: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(Tok::Op(o @ ('+' | '-'))) = self.peek().cloned() {
            self.i += 1;
            let r = self.term()?;
            v = if o == '+' { v + r } else { v - r };
        }
        Some(v)
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        while let Some(Tok::Op(o @ ('*' | '/' | '%'))) = self.peek().cloned() {
            self.i += 1;
            let r = self.unary()?;
            v = match o {
                '*' => v * r,
                '/' => {
                    if r == 0.0 {
                        return None;
                    }
                    v / r
                }
                _ => {
                    if r == 0.0 {
                        return None;
                    }
                    v % r
                }
            };
        }
        Some(v)
    }
    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.i += 1;
                Some(-self.unary()?)
            }
            Some(Tok::Op('+')) => {
                self.i += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Option<f64> {
        let base = self.atom()?;
        if let Some(Tok::Pow) = self.peek() {
            self.i += 1;
            let e = self.unary()?;
            return Some(base.powf(e));
        }
        Some(base)
    }
    fn atom(&mut self) -> Option<f64> {
        let t = self.peek()?.clone();
        self.i += 1;
        match t {
            Tok::Num(n) => Some(n),
            Tok::LParen => {
                self.depth += 1;
                if self.depth > 64 {
                    return None;
                }
                let v = self.expr()?;
                self.depth -= 1;
                match self.peek() {
                    Some(Tok::RParen) => {
                        self.i += 1;
                        Some(v)
                    }
                    _ => None,
                }
            }
            Tok::Ident(name) => match name.as_str() {
                "pi" => Some(std::f64::consts::PI),
                "e" => Some(std::f64::consts::E),
                f => {
                    if !matches!(self.peek(), Some(Tok::LParen)) {
                        return None;
                    }
                    self.i += 1;
                    let mut args = vec![self.expr()?];
                    while let Some(Tok::Comma) = self.peek() {
                        self.i += 1;
                        args.push(self.expr()?);
                    }
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        return None;
                    }
                    self.i += 1;
                    apply(f, &args)
                }
            },
            _ => None,
        }
    }
}

fn apply(f: &str, a: &[f64]) -> Option<f64> {
    let one = || if a.len() == 1 { Some(a[0]) } else { None };
    match f {
        "sqrt" => one().filter(|x| *x >= 0.0).map(f64::sqrt),
        "abs" => one().map(f64::abs),
        "round" => one().map(f64::round),
        "floor" => one().map(f64::floor),
        "ceil" => one().map(f64::ceil),
        "sin" => one().map(f64::sin),
        "cos" => one().map(f64::cos),
        "tan" => one().map(f64::tan),
        "ln" => one().filter(|x| *x > 0.0).map(f64::ln),
        "log" => one().filter(|x| *x > 0.0).map(f64::log10),
        "min" if !a.is_empty() => a.iter().cloned().reduce(f64::min),
        "max" if !a.is_empty() => a.iter().cloned().reduce(f64::max),
        _ => None,
    }
}

/// Returns the numeric result if `input` is a complete, valid expression.
pub fn evaluate(input: &str) -> Option<f64> {
    if input.len() > 200 {
        return None;
    }
    let toks = lex(input)?;
    if toks.is_empty() {
        return None;
    }
    let mut p = P { t: toks, i: 0, depth: 0 };
    let v = p.expr()?;
    if p.i != p.t.len() || !v.is_finite() {
        return None;
    }
    Some(v)
}

/// True when the expression is more than a bare number (so plain numeric searches are not hijacked).
pub fn looks_like_expression(input: &str) -> bool {
    let t = input.trim();
    if t.is_empty() {
        return false;
    }
    let Some(toks) = lex(t) else { return false };
    let has_op = toks.iter().enumerate().any(|(i, k)| match k {
        Tok::Op(_) | Tok::Pow => i > 0,
        Tok::Ident(n) => n == "pi" || n == "e" || matches!(toks.get(i + 1), Some(Tok::LParen)),
        _ => false,
    });
    has_op && toks.iter().any(|k| matches!(k, Tok::Num(_) | Tok::Ident(_)))
}

pub fn format_result(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.len() > 20 || (v.abs() < 1e-9 && v != 0.0) {
        format!("{:e}", v)
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> Option<String> {
        evaluate(s).map(format_result)
    }

    #[test]
    fn arithmetic() {
        assert_eq!(ev("1+2*3").as_deref(), Some("7"));
        assert_eq!(ev("(1+2)*3").as_deref(), Some("9"));
        assert_eq!(ev("2^3^2").as_deref(), Some("512"));
        assert_eq!(ev("-2^2").as_deref(), Some("-4"));
        assert_eq!(ev("10/4").as_deref(), Some("2.5"));
        assert_eq!(ev("10 % 4").as_deref(), Some("2"));
        assert_eq!(ev("2 x 3").as_deref(), Some("6"));
        assert_eq!(ev("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(ev("sqrt(16)+max(1,5,3)").as_deref(), Some("9"));
        assert_eq!(ev("round(pi*100)/100").as_deref(), Some("3.14"));
        assert_eq!(ev("2**10").as_deref(), Some("1024"));
        assert_eq!(ev("1e3 + 1").as_deref(), Some("1001"));
    }

    #[test]
    fn invalid_inputs() {
        for bad in ["", "1/0", "(1+2", "1+", "foo", "sqrt(-1)", "1 2", "abc(1)", "2 $ 3", "((((", "ln(0)"] {
            assert!(evaluate(bad).is_none(), "{bad:?}");
        }
        assert!(evaluate(&"(".repeat(500)).is_none());
        assert!(evaluate(&"1+".repeat(300)).is_none());
    }

    #[test]
    fn detection() {
        assert!(looks_like_expression("2+2"));
        assert!(looks_like_expression("sqrt(9)"));
        assert!(!looks_like_expression("42"));
        assert!(!looks_like_expression("chrome"));
        assert!(!looks_like_expression("-5"));
    }
}
