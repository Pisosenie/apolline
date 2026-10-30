pub fn answer(input: &str) -> Option<String> {
    let input = input.trim();
    if let Some(converted) = convert(input) {
        return Some(converted);
    }
    if !input.chars().any(|c| "+-*/^%×÷(".contains(c)) && !input.contains(" of ") {
        return None;
    }
    let value = eval(input)?;
    value.is_finite().then(|| format(value))
}

const UNITS: [(&str, &str, f64); 30] = [
    ("mm", "length", 0.001),
    ("cm", "length", 0.01),
    ("m", "length", 1.0),
    ("km", "length", 1000.0),
    ("in", "length", 0.0254),
    ("ft", "length", 0.3048),
    ("yd", "length", 0.9144),
    ("mi", "length", 1609.344),
    ("mg", "mass", 0.000_001),
    ("g", "mass", 0.001),
    ("kg", "mass", 1.0),
    ("t", "mass", 1000.0),
    ("oz", "mass", 0.028_349_523_125),
    ("lb", "mass", 0.453_592_37),
    ("b", "data", 1.0),
    ("kb", "data", 1e3),
    ("mb", "data", 1e6),
    ("gb", "data", 1e9),
    ("tb", "data", 1e12),
    ("kib", "data", 1024.0),
    ("mib", "data", 1_048_576.0),
    ("gib", "data", 1_073_741_824.0),
    ("tib", "data", 1_099_511_627_776.0),
    ("s", "time", 1.0),
    ("min", "time", 60.0),
    ("h", "time", 3600.0),
    ("d", "time", 86400.0),
    ("wk", "time", 604_800.0),
    ("ml", "volume", 0.001),
    ("l", "volume", 1.0),
];

fn convert(input: &str) -> Option<String> {
    let lower = input.to_lowercase();
    let (from, to) = lower
        .split_once(" in ")
        .or_else(|| lower.split_once(" to "))?;
    let from = from.trim();
    let split = from.find(|c: char| c.is_alphabetic() || c == '°')?;
    let (number, unit) = from.split_at(split);
    let value = eval(number.trim())?;
    let (unit, to) = (
        unit.trim().trim_start_matches('°'),
        to.trim().trim_start_matches('°'),
    );
    if let Some(result) = temperature(value, unit, to) {
        return Some(format!("{} °{}", short(result), to.to_uppercase()));
    }
    let find = |name: &str| UNITS.iter().find(|(n, _, _)| *n == name);
    let (_, kind, factor) = find(unit)?;
    let (_, other, into) = find(to)?;
    (kind == other).then(|| format!("{} {}", short(value * factor / into), to_label(to)))
}

fn temperature(value: f64, from: &str, to: &str) -> Option<f64> {
    let kelvin = match from {
        "c" => value + 273.15,
        "f" => (value - 32.0) * 5.0 / 9.0 + 273.15,
        "k" => value,
        _ => return None,
    };
    Some(match to {
        "c" => kelvin - 273.15,
        "f" => (kelvin - 273.15) * 9.0 / 5.0 + 32.0,
        "k" => kelvin,
        _ => return None,
    })
}

fn short(value: f64) -> String {
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn to_label(unit: &str) -> String {
    match unit {
        "kb" | "mb" | "gb" | "tb" => unit.to_uppercase(),
        "kib" | "mib" | "gib" | "tib" => format!("{}iB", unit[..1].to_uppercase()),
        "b" => "B".into(),
        "l" => "L".into(),
        _ => unit.to_owned(),
    }
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
    fn units_convert_within_their_kind() {
        assert_eq!(answer("10 km in mi").as_deref(), Some("6.2137 mi"));
        assert_eq!(answer("100 F to C").as_deref(), Some("37.7778 °C"));
        assert_eq!(answer("5 GB in MB").as_deref(), Some("5000 MB"));
        assert_eq!(answer("1 GiB in MB").as_deref(), Some("1073.7418 MB"));
        assert_eq!(answer("90 min in h").as_deref(), Some("1.5 h"));
        assert_eq!(answer("2*3 kg to lb").as_deref(), Some("13.2277 lb"));
        assert_eq!(answer("10 km in kg"), None, "length isn't mass");
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
