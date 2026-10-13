//! Safe arithmetic evaluator for the launcher calculator. A hand-written
//! recursive-descent parser; no `eval`, no code execution.

const MAX_LEN: usize = 256;
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Op(char),
    LParen,
    RParen,
    Ident(String),
    Comma,
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() || ch == '_' {
            i += 1;
        } else if ch.is_ascii_digit() || ch == '.' {
            let start = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.' || c[i] == '_') {
                i += 1;
            }
            if i < c.len()
                && (c[i] == 'e' || c[i] == 'E')
                && i + 1 < c.len()
                && (c[i + 1].is_ascii_digit()
                    || ((c[i + 1] == '-' || c[i + 1] == '+')
                        && i + 2 < c.len()
                        && c[i + 2].is_ascii_digit()))
            {
                i += 2;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let txt: String = c[start..i].iter().filter(|&&x| x != '_').collect();
            out.push(Tok::Num(txt.parse().ok()?));
        } else if ch.is_alphabetic() {
            let start = i;
            while i < c.len() && c[i].is_alphanumeric() {
                i += 1;
            }
            out.push(Tok::Ident(
                c[start..i].iter().collect::<String>().to_lowercase(),
            ));
        } else {
            out.push(match ch {
                '+' | '-' | '*' | '/' | '%' | '^' => Tok::Op(ch),
                '×' => Tok::Op('*'),
                '÷' => Tok::Op('/'),
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                ',' => Tok::Comma,
                _ => return None,
            });
            i += 1;
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
    fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }
    fn expr(&mut self) -> Option<f64> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        let mut v = self.term()?;
        while let Some(Tok::Op(o @ ('+' | '-'))) = self.peek().cloned() {
            self.i += 1;
            let r = self.term()?;
            v = if o == '+' { v + r } else { v - r };
        }
        self.depth -= 1;
        Some(v)
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            match self.peek().cloned() {
                Some(Tok::Op(o @ ('*' | '/' | '%'))) => {
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
                // Implicit multiplication: 2(3) or 2pi
                Some(Tok::LParen) | Some(Tok::Ident(_)) => {
                    let r = self.unary()?;
                    v *= r;
                }
                _ => return Some(v),
            }
        }
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
        if let Some(Tok::Op('^')) = self.peek() {
            self.i += 1;
            let exp = self.unary()?;
            return Some(base.powf(exp));
        }
        Some(base)
    }
    fn atom(&mut self) -> Option<f64> {
        match self.next()? {
            Tok::Num(n) => Some(n),
            Tok::LParen => {
                let v = self.expr()?;
                matches!(self.next()?, Tok::RParen).then_some(v)
            }
            Tok::Ident(name) => match name.as_str() {
                "pi" => Some(std::f64::consts::PI),
                "e" => Some(std::f64::consts::E),
                "tau" => Some(std::f64::consts::TAU),
                f => {
                    if !matches!(self.next()?, Tok::LParen) {
                        return None;
                    }
                    let a = self.expr()?;
                    let b = if let Some(Tok::Comma) = self.peek() {
                        self.i += 1;
                        Some(self.expr()?)
                    } else {
                        None
                    };
                    if !matches!(self.next()?, Tok::RParen) {
                        return None;
                    }
                    Some(match (f, b) {
                        ("sqrt", None) if a >= 0.0 => a.sqrt(),
                        ("abs", None) => a.abs(),
                        ("round", None) => a.round(),
                        ("floor", None) => a.floor(),
                        ("ceil", None) => a.ceil(),
                        ("sin", None) => a.sin(),
                        ("cos", None) => a.cos(),
                        ("tan", None) => a.tan(),
                        ("ln", None) if a > 0.0 => a.ln(),
                        ("log", None) if a > 0.0 => a.log10(),
                        ("log", Some(base)) if a > 0.0 && base > 0.0 => a.log(base),
                        ("min", Some(b)) => a.min(b),
                        ("max", Some(b)) => a.max(b),
                        ("pow", Some(b)) => a.powf(b),
                        _ => return None,
                    })
                }
            },
            _ => None,
        }
    }
}

/// Evaluate `input` if it looks like arithmetic. Plain numbers and words are
/// not treated as calculations so normal searches are unaffected.
pub fn evaluate(input: &str) -> Option<f64> {
    let s = input.trim().trim_start_matches('=').trim();
    if s.is_empty() || s.len() > MAX_LEN {
        return None;
    }
    let toks = tokenize(s)?;
    let has_op = toks.iter().any(|t| matches!(t, Tok::Op(_) | Tok::LParen))
        || toks.windows(2).any(|w| {
            matches!(
                (&w[0], &w[1]),
                (Tok::Num(_), Tok::Ident(_))
                    | (Tok::Num(_), Tok::LParen)
                    | (Tok::Ident(_), Tok::LParen)
            )
        });
    let explicit = input.trim_start().starts_with('=');
    if !has_op && !explicit {
        return None;
    }
    let mut p = P {
        t: toks,
        i: 0,
        depth: 0,
    };
    let v = p.expr()?;
    if p.i != p.t.len() || !v.is_finite() {
        return None;
    }
    Some(v)
}

pub fn format(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.len() > 20 {
        format!("{v:e}")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_expressions() {
        assert_eq!(evaluate("1+2*3"), Some(7.0));
        assert_eq!(evaluate("(1+2)*3"), Some(9.0));
        assert_eq!(evaluate("2^3^2"), Some(512.0));
        assert_eq!(evaluate("-2^2"), Some(-4.0));
        assert_eq!(evaluate("10 % 4"), Some(2.0));
        assert_eq!(evaluate("sqrt(16) + abs(-2)"), Some(6.0));
        assert_eq!(evaluate("2pi").map(|v| (v * 1000.0).round()), Some(6283.0));
        assert_eq!(evaluate("=42"), Some(42.0));
        assert_eq!(evaluate("1_000 * 3"), Some(3000.0));
        assert_eq!(evaluate("1.5e3 / 3"), Some(500.0));
    }

    #[test]
    fn ignores_non_math_and_errors() {
        assert_eq!(evaluate("code"), None);
        assert_eq!(evaluate("42"), None);
        assert_eq!(evaluate("1/0"), None);
        assert_eq!(evaluate("2+"), None);
        assert_eq!(evaluate("rm -rf /"), None);
        assert_eq!(evaluate(&"(".repeat(500)), None);
        assert_eq!(evaluate("sqrt(-1)"), None);
    }

    #[test]
    fn formats() {
        assert_eq!(format(7.0), "7");
        assert_eq!(format(0.1 + 0.2), "0.3");
        assert_eq!(format(1.0 / 3.0), "0.3333333333");
    }
}
