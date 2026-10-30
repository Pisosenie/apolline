pub fn answer(input: &str) -> Option<String> {
    let input = input.trim();
    if !input.chars().any(|c| "+-*/^%×÷(".contains(c)) && !input.contains(" of ") {
        return None;
    }
    let value = eval(input)?;
    value.is_finite().then(|| format(value))
}

fn eval(input: &str) -> Option<f64> {
    if let Some((share, whole)) = input.split_once(" of ") {
        let share = share.trim().strip_suffix('%')?;
        return Some(eval(share)? / 100.0 * eval(whole)?);
    }
    let tokens = tokens(input)?;
    let mut parser = Parser { tokens, at: 0 };
    let value = parser.sum()?;
    (parser.at == parser.tokens.len()).then_some(value)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Token {
    Number(f64),
    Op(char),
}

fn tokens(input: &str) -> Option<Vec<Token>> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '_' => {
                chars.next();
            }
            '0'..='9' | '.' | ',' => {
                let mut number = String::new();
                while let Some(&d) = chars.peek() {
                    match d {
                        '0'..='9' | '.' => number.push(d),
                        ',' => number.push('.'),
                        '_' => {}
                        _ => break,
                    }
                    chars.next();
                }
                out.push(Token::Number(number.parse().ok()?));
            }
            '+' | '-' | '*' | '/' | '^' | '%' | '(' | ')' => {
                out.push(Token::Op(c));
                chars.next();
            }
            '×' | 'x' => {
                out.push(Token::Op('*'));
                chars.next();
            }
            '÷' => {
                out.push(Token::Op('/'));
                chars.next();
            }
            _ => return None,
        }
    }
    Some(out)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.at).copied()
    }

    fn eat(&mut self, op: char) -> bool {
        let found = self.peek() == Some(Token::Op(op));
        if found {
            self.at += 1;
        }
        found
    }

    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        loop {
            if self.eat('+') {
                value += self.product()?;
            } else if self.eat('-') {
                value -= self.product()?;
            } else {
                return Some(value);
            }
        }
    }

    fn product(&mut self) -> Option<f64> {
        let mut value = self.unary()?;
        loop {
            if self.eat('*') {
                value *= self.unary()?;
            } else if self.eat('/') {
                value /= self.unary()?;
            } else {
                return Some(value);
            }
        }
    }

    fn unary(&mut self) -> Option<f64> {
        if self.eat('-') {
            return Some(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.primary()?;
        if self.eat('^') {
            return Some(base.powf(self.unary()?));
        }
        Some(base)
    }

    fn primary(&mut self) -> Option<f64> {
        let mut value = match self.peek()? {
            Token::Number(n) => {
                self.at += 1;
                n
            }
            Token::Op('(') => {
                self.at += 1;
                let inner = self.sum()?;
                if !self.eat(')') {
                    return None;
                }
                inner
            }
            Token::Op(_) => return None,
        };
        while self.eat('%') {
            value /= 100.0;
        }
        Some(value)
    }
}

fn format(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{value:.0}");
    }
    let text = format!("{value:.10}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everyday_sums() {
        assert_eq!(answer("2^10").as_deref(), Some("1024"));
        assert_eq!(answer("15% of 340").as_deref(), Some("51"));
        assert_eq!(answer("(1 + 2) * 3 - 4 / 8").as_deref(), Some("8.5"));
        assert_eq!(answer("-2^2").as_deref(), Some("-4"));
        assert_eq!(answer("2^3^2").as_deref(), Some("512"));
        assert_eq!(answer("1,5 × 4").as_deref(), Some("6"));
        assert_eq!(answer("1/3").as_deref(), Some("0.3333333333"));
        assert_eq!(answer("50%").as_deref(), Some("0.5"));
    }

    #[test]
    fn words_and_bare_numbers_are_left_to_the_launcher() {
        assert_eq!(answer("firefox"), None);
        assert_eq!(answer("42"), None);
        assert_eq!(answer("2 +"), None);
        assert_eq!(answer("1/0"), None);
        assert_eq!(answer("(2"), None);
    }
}
