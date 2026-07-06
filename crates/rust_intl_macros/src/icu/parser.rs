use super::{ArmKey, AstNode};

pub fn parse(src: &str) -> Result<Vec<AstNode>, String> {
    let mut p = Parser {
        chars: src.chars().collect(),
        pos: 0,
    };
    let nodes = p.parse_message(false)?;
    if p.pos != p.chars.len() {
        return Err(format!(
            "unexpected trailing '{}' at position {}",
            p.chars[p.pos], p.pos
        ));
    }
    Ok(nodes)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.bump();
        }
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.bump();
            Ok(())
        } else {
            Err(format!(
                "expected '{}' at position {}, found {:?}",
                c,
                self.pos,
                self.peek()
            ))
        }
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        let start = self.pos;
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        if s.is_empty() {
            Err(format!("expected identifier at position {start}"))
        } else {
            Ok(s)
        }
    }

    fn parse_number(&mut self) -> Result<i64, String> {
        let start = self.pos;
        let mut s = String::new();
        if self.peek() == Some('-') {
            s.push('-');
            self.bump();
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            s.push(self.bump().unwrap());
        }
        s.parse::<i64>()
            .map_err(|_| format!("expected number at position {start}"))
    }

    /// `stop_at_close`: if true, return (without consuming) as soon as an
    /// unmatched `}` is seen, so the caller can consume it as the delimiter
    /// of an enclosing construct. If false (top level), a stray `}` is
    /// treated as a literal character.
    fn parse_message(&mut self, stop_at_close: bool) -> Result<Vec<AstNode>, String> {
        let mut nodes = Vec::new();
        let mut text = String::new();
        loop {
            match self.peek() {
                None => break,
                Some('}') if stop_at_close => break,
                Some('\'') => {
                    self.bump();
                    if self.peek() == Some('\'') {
                        text.push('\'');
                        self.bump();
                    } else {
                        // Quoted literal run until the next single quote (or EOF)
                        while let Some(c) = self.peek() {
                            if c == '\'' {
                                self.bump();
                                break;
                            }
                            text.push(c);
                            self.bump();
                        }
                    }
                }
                Some('{') => {
                    if !text.is_empty() {
                        nodes.push(AstNode::Text(std::mem::take(&mut text)));
                    }
                    self.bump();
                    nodes.push(self.parse_placeholder()?);
                }
                Some(c) => {
                    text.push(c);
                    self.bump();
                }
            }
        }
        if !text.is_empty() {
            nodes.push(AstNode::Text(text));
        }
        Ok(nodes)
    }

    /// Called right after the opening `{` of a placeholder has been consumed.
    fn parse_placeholder(&mut self) -> Result<AstNode, String> {
        self.skip_ws();
        let name = self.parse_ident()?;
        self.skip_ws();

        if self.peek() == Some('}') {
            self.bump();
            return Ok(AstNode::Var(name));
        }
        self.expect(',')?;
        self.skip_ws();
        let kind = self.parse_ident()?;
        self.skip_ws();

        match kind.as_str() {
            "plural" | "selectordinal" => {
                if self.peek() == Some(',') {
                    self.bump();
                }
                let arms = self.parse_arms()?;
                self.expect('}')?;
                if !arms
                    .iter()
                    .any(|(k, _)| matches!(k, ArmKey::Category(c) if c == "other"))
                {
                    return Err(format!(
                        "'{name}, {kind}, ...' is missing a required `other` arm"
                    ));
                }
                for (k, _) in &arms {
                    if let ArmKey::Category(c) = k
                        && !matches!(
                            c.as_str(),
                            "zero" | "one" | "two" | "few" | "many" | "other"
                        )
                    {
                        return Err(format!(
                            "'{c}' is not a valid plural category for '{name}, {kind}' \
                                 (expected one of: zero, one, two, few, many, other, or =N)"
                        ));
                    }
                }
                Ok(AstNode::Plural {
                    var: name,
                    ordinal: kind == "selectordinal",
                    arms,
                })
            }
            "select" => {
                if self.peek() == Some(',') {
                    self.bump();
                }
                let arms = self.parse_arms()?;
                self.expect('}')?;
                if !arms
                    .iter()
                    .any(|(k, _)| matches!(k, ArmKey::Category(c) if c == "other"))
                {
                    return Err(format!(
                        "'{name}, select, ...' is missing a required `other` arm"
                    ));
                }
                let arms = arms
                    .into_iter()
                    .map(|(k, body)| match k {
                        ArmKey::Category(s) => (s, body),
                        ArmKey::Exact(n) => (n.to_string(), body),
                    })
                    .collect();
                Ok(AstNode::Select { var: name, arms })
            }
            "number" | "date" | "time" => {
                // Skip an optional style/skeleton, then close
                if self.peek() == Some(',') {
                    self.bump();
                    self.skip_ws();
                    let _ = self.parse_ident();
                    self.skip_ws();
                }
                self.expect('}')?;
                Ok(AstNode::Var(name))
            }
            other => Err(format!(
                "unsupported placeholder type '{other}' for '{name}'"
            )),
        }
    }

    fn parse_arms(&mut self) -> Result<Vec<(ArmKey, Vec<AstNode>)>, String> {
        let mut arms = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some('}') | None => break,
                Some('=') => {
                    self.bump();
                    let n = self.parse_number()?;
                    self.skip_ws();
                    self.expect('{')?;
                    let body = self.parse_message(true)?;
                    self.expect('}')?;
                    arms.push((ArmKey::Exact(n), body));
                }
                _ => {
                    let word = self.parse_ident()?;
                    self.skip_ws();
                    if word == "offset" && self.peek() == Some(':') {
                        self.bump();
                        self.skip_ws();
                        self.parse_number()?;
                        continue;
                    }
                    self.expect('{')?;
                    let body = self.parse_message(true)?;
                    self.expect('}')?;
                    arms.push((ArmKey::Category(word), body));
                }
            }
        }
        Ok(arms)
    }
}
