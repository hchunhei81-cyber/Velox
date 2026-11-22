#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Fn,
    Struct,
    Let,
    Mut,
    Return,
    If,
    Else,
    While,
    For,
    In,
    True,
    False,
    Parallel,
    Public,
    Private,
    Export,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Colon,
    Arrow,
    DotDot,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    Lt,
    Gt,
    Le,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub kind: Tok,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }
    fn peek(&self) -> u8 {
        *self.src.get(self.pos).unwrap_or(&0)
    }
    fn peek2(&self) -> u8 {
        *self.src.get(self.pos + 1).unwrap_or(&0)
    }
    fn bump(&mut self) -> u8 {
        let c = self.peek();
        if self.pos < self.src.len() {
            self.pos += 1;
        }
        c
    }

    pub fn tokenize(&mut self) -> Result<Vec<Tok>, String> {
        self.tokenize_inner()
            .map(|tokens| tokens.into_iter().map(|token| token.kind).collect())
    }

    pub fn tokenize_spanned(&mut self) -> Result<Vec<SpannedToken>, LexError> {
        self.tokenize_inner().map_err(|message| {
            let start = self.pos.saturating_sub(1);
            LexError {
                message,
                span: Span {
                    start,
                    end: self.pos,
                },
            }
        })
    }

    fn tokenize_inner(&mut self) -> Result<Vec<SpannedToken>, String> {
        let mut out = Vec::new();
        loop {
            while self.peek().is_ascii_whitespace() {
                self.bump();
            }
            if self.peek() == b'/' && self.peek2() == b'/' {
                while self.peek() != b'\n' && self.peek() != 0 {
                    self.bump();
                }
                continue;
            }
            if self.peek() == b'/' && self.peek2() == b'*' {
                self.bump();
                self.bump();
                let mut closed = false;
                while self.pos < self.src.len() {
                    if self.peek() == b'*' && self.peek2() == b'/' {
                        self.bump();
                        self.bump();
                        closed = true;
                        break;
                    }
                    self.bump();
                }
                if !closed {
                    return Err("unterminated block comment".into());
                }
                continue;
            }
            let c = self.peek();
            if self.pos >= self.src.len() {
                out.push(SpannedToken {
                    kind: Tok::Eof,
                    span: Span {
                        start: self.pos,
                        end: self.pos,
                    },
                });
                break;
            }
            let token_start = self.pos;
            if c == 0 {
                return Err("NUL byte is not allowed in source".into());
            }

            if c.is_ascii_alphabetic() || c == b'_' {
                let start = self.pos;
                while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
                    self.bump();
                }
                let s = std::str::from_utf8(&self.src[start..self.pos])
                    .unwrap()
                    .to_string();
                let kind = match s.as_str() {
                    "fn" => Tok::Fn,
                    "struct" => Tok::Struct,
                    "let" => Tok::Let,
                    "mut" => Tok::Mut,
                    "return" => Tok::Return,
                    "if" => Tok::If,
                    "else" => Tok::Else,
                    "while" => Tok::While,
                    "for" => Tok::For,
                    "in" => Tok::In,
                    "true" => Tok::True,
                    "false" => Tok::False,
                    "parallel" => Tok::Parallel,
                    "public" => Tok::Public,
                    "private" => Tok::Private,
                    "export" => Tok::Export,
                    _ => Tok::Ident(s),
                };
                out.push(SpannedToken {
                    kind,
                    span: Span {
                        start: token_start,
                        end: self.pos,
                    },
                });
                continue;
            }

            if c.is_ascii_digit() {
                let start = self.pos;
                while self.peek().is_ascii_digit() {
                    self.bump();
                }
                // Floating-point literal: for example, 3.14; require a digit after the decimal point.
                if self.peek() == b'.' && self.peek2().is_ascii_digit() {
                    self.bump(); // .
                    while self.peek().is_ascii_digit() {
                        self.bump();
                    }
                    let f: f64 = std::str::from_utf8(&self.src[start..self.pos])
                        .unwrap()
                        .parse()
                        .map_err(|e| format!("bad float: {}", e))?;
                    out.push(SpannedToken {
                        kind: Tok::Float(f),
                        span: Span {
                            start: token_start,
                            end: self.pos,
                        },
                    });
                    continue;
                }
                let n: i64 = std::str::from_utf8(&self.src[start..self.pos])
                    .unwrap()
                    .parse()
                    .map_err(|e| format!("bad int: {}", e))?;
                out.push(SpannedToken {
                    kind: Tok::Int(n),
                    span: Span {
                        start: token_start,
                        end: self.pos,
                    },
                });
                continue;
            }

            if c == b'"' {
                self.bump();
                let mut s = String::new();
                loop {
                    if self.pos >= self.src.len() {
                        return Err("unterminated string".into());
                    }
                    let ch = self.bump();
                    if ch == 0 {
                        return Err("NUL byte is not allowed in source; use the \\0 escape".into());
                    }
                    if ch == b'"' {
                        break;
                    }
                    if ch == b'\\' {
                        if self.pos >= self.src.len() {
                            return Err("unterminated escape sequence".into());
                        }
                        let e = self.bump();
                        s.push(match e {
                            b'n' => '\n',
                            b't' => '\t',
                            b'r' => '\r',
                            b'0' => '\0',
                            b'\\' => '\\',
                            b'"' => '"',
                            _ => return Err(format!("bad escape \\{}", e as char)),
                        });
                    } else {
                        // bump() has consumed the first byte; decode from the start of the character.
                        let start = self.pos - 1;
                        let rest = std::str::from_utf8(&self.src[start..])
                            .map_err(|e| format!("invalid UTF-8 in string: {}", e))?;
                        let unicode_ch = rest
                            .chars()
                            .next()
                            .ok_or_else(|| "unterminated string".to_string())?;
                        self.pos = start + unicode_ch.len_utf8();
                        s.push(unicode_ch);
                    }
                }
                out.push(SpannedToken {
                    kind: Tok::Str(s),
                    span: Span {
                        start: token_start,
                        end: self.pos,
                    },
                });
                continue;
            }

            self.bump();
            let t = match c {
                b'(' => Tok::LParen,
                b')' => Tok::RParen,
                b'{' => Tok::LBrace,
                b'}' => Tok::RBrace,
                b',' => Tok::Comma,
                b';' => Tok::Semi,
                b':' => Tok::Colon,
                b'+' => Tok::Plus,
                b'*' => Tok::Star,
                b'/' => Tok::Slash,
                b'%' => Tok::Percent,
                b'&' => {
                    if self.peek() == b'&' {
                        self.bump();
                        Tok::AndAnd
                    } else {
                        return Err("expected `&&`".into());
                    }
                }
                b'|' => {
                    if self.peek() == b'|' {
                        self.bump();
                        Tok::OrOr
                    } else {
                        return Err("expected `||`".into());
                    }
                }
                b'!' => Tok::Bang,
                b'-' => {
                    if self.peek() == b'>' {
                        self.bump();
                        Tok::Arrow
                    } else {
                        Tok::Minus
                    }
                }
                b'=' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::EqEq
                    } else {
                        Tok::Eq
                    }
                }
                b'<' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::Le
                    } else {
                        Tok::Lt
                    }
                }
                b'>' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::Ge
                    } else {
                        Tok::Gt
                    }
                }
                b'.' => {
                    if self.peek() == b'.' {
                        self.bump();
                        Tok::DotDot
                    } else {
                        Tok::Dot
                    }
                }
                _ => return Err(format!("unexpected char '{}'", c as char)),
            };
            out.push(SpannedToken {
                kind: t,
                span: Span {
                    start: token_start,
                    end: self.pos,
                },
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod comment_tests {
    use super::*;

    #[test]
    fn line_and_block_comments_are_ignored() {
        let source = r#"
            // line comment
            fn main() -> void {
                /* block
                   comment */
                let value: i64 = 42; // trailing comment
            }
        "#;
        let tokens = Lexer::new(source).tokenize().unwrap();
        assert!(tokens.contains(&Tok::Fn));
        assert!(tokens.contains(&Tok::Let));
        assert!(tokens.contains(&Tok::Int(42)));
        assert!(!tokens
            .iter()
            .any(|t| matches!(t, Tok::Ident(s) if s == "comment")));
    }

    #[test]
    fn unterminated_block_comment_is_rejected() {
        let result = Lexer::new("fn main() -> void { /* missing end").tokenize();
        assert!(result.unwrap_err().contains("unterminated block comment"));
    }

    #[test]
    fn comment_markers_inside_strings_are_not_comments() {
        let tokens = Lexer::new(r#"fn main() -> void { let s: str = "/* text */"; }"#)
            .tokenize()
            .unwrap();
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Tok::Str(s) if s == "/* text */")));
    }
}
