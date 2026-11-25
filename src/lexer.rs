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
    Break,
    Continue,
    If,
    Else,
    While,
    For,
    In,
    True,
    False,
    Parallel,
    Public,
    Export,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    Arrow,
    FatArrow,
    DotDot,
    Dot,
    Plus,
    PlusPlus,
    PlusEq,
    Minus,
    MinusMinus,
    MinusEq,
    Star,
    StarEq,
    Slash,
    SlashEq,
    Percent,
    Caret,
    Question,
    Shl,
    Shr,
    Eq,
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    Amp,
    Pipe,
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
                    "break" => Tok::Break,
                    "continue" => Tok::Continue,
                    "if" => Tok::If,
                    "else" => Tok::Else,
                    "while" => Tok::While,
                    "for" => Tok::For,
                    "in" => Tok::In,
                    "true" => Tok::True,
                    "false" => Tok::False,
                    "parallel" => Tok::Parallel,
                    "pub" => Tok::Public,
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

                let mut is_float = false;

                // Decimal fraction; preserve `1..10` as a range.
                if self.peek() == b'.' && self.peek2().is_ascii_digit() {
                    is_float = true;
                    self.bump();
                    while self.peek().is_ascii_digit() {
                        self.bump();
                    }
                }

                // Scientific notation: 1e3, 1E-3, 2.5e+8.
                if matches!(self.peek(), b'e' | b'E') {
                    is_float = true;
                    self.bump();
                    if matches!(self.peek(), b'+' | b'-') {
                        self.bump();
                    }
                    if !self.peek().is_ascii_digit() {
                        return Err("malformed exponent: expected a digit".into());
                    }
                    while self.peek().is_ascii_digit() {
                        self.bump();
                    }
                }

                if is_float {
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
                b'[' => Tok::LBracket,
                b']' => Tok::RBracket,
                b',' => Tok::Comma,
                b';' => Tok::Semi,
                b':' => Tok::Colon,
                b'+' => {
                    if self.peek() == b'+' {
                        self.bump();
                        Tok::PlusPlus
                    } else if self.peek() == b'=' {
                        self.bump();
                        Tok::PlusEq
                    } else {
                        Tok::Plus
                    }
                }
                b'*' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::StarEq
                    } else {
                        Tok::Star
                    }
                }
                b'/' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::SlashEq
                    } else {
                        Tok::Slash
                    }
                }
                b'%' => Tok::Percent,
                b'^' => Tok::Caret,
                b'?' => Tok::Question,
                b'&' => {
                    if self.peek() == b'&' {
                        self.bump();
                        Tok::AndAnd
                    } else {
                        Tok::Amp
                    }
                }
                b'|' => {
                    if self.peek() == b'|' {
                        self.bump();
                        Tok::OrOr
                    } else {
                        Tok::Pipe
                    }
                }
                b'!' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::NotEq
                    } else {
                        Tok::Bang
                    }
                }
                b'-' => {
                    if self.peek() == b'>' {
                        self.bump();
                        Tok::Arrow
                    } else if self.peek() == b'-' {
                        self.bump();
                        Tok::MinusMinus
                    } else if self.peek() == b'=' {
                        self.bump();
                        Tok::MinusEq
                    } else {
                        Tok::Minus
                    }
                }
                b'=' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::EqEq
                    } else if self.peek() == b'>' {
                        self.bump();
                        Tok::FatArrow
                    } else {
                        Tok::Eq
                    }
                }
                b'<' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::Le
                    } else if self.peek() == b'<' {
                        self.bump();
                        Tok::Shl
                    } else {
                        Tok::Lt
                    }
                }
                b'>' => {
                    if self.peek() == b'=' {
                        self.bump();
                        Tok::Ge
                    } else if self.peek() == b'>' {
                        self.bump();
                        Tok::Shr
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

#[cfg(test)]
mod syntax_keyword_tests {
    use super::*;

    #[test]
    fn pub_is_keyword_and_old_visibility_words_are_identifiers() {
        let tokens = Lexer::new("pub fn visible() {} public private")
            .tokenize()
            .unwrap();
        assert!(tokens.contains(&Tok::Public));
        assert!(tokens.contains(&Tok::Ident("public".into())));
        assert!(tokens.contains(&Tok::Ident("private".into())));
    }
}

#[cfg(test)]
mod scientific_notation_tests {
    use super::*;

    #[test]
    fn scientific_notation_is_tokenized_as_float() {
        let tokens = Lexer::new("1e3 1E-3 2.5e+8").tokenize().unwrap();
        assert!(tokens.contains(&Tok::Float(1000.0)));
        assert!(tokens.contains(&Tok::Float(0.001)));
        assert!(tokens.contains(&Tok::Float(250_000_000.0)));
    }

    #[test]
    fn incomplete_exponents_are_rejected() {
        for source in ["1e", "1e+", "2.5E-"] {
            assert!(
                Lexer::new(source).tokenize().is_err(),
                "accepted malformed literal: {source}"
            );
        }
    }

    #[test]
    fn range_operator_is_not_consumed_as_decimal_point() {
        let tokens = Lexer::new("1..10").tokenize().unwrap();
        assert!(tokens.contains(&Tok::Int(1)));
        assert!(tokens.contains(&Tok::Int(10)));
        assert!(tokens.iter().any(|t| matches!(t, Tok::DotDot)));
    }
}
