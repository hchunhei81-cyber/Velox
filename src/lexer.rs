#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Fn,
    Let,
    Mut,
    Return,
    If,
    Else,
    While,
    For,
    In,
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
    Eof,
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
            let c = self.peek();
            if c == 0 {
                out.push(Tok::Eof);
                break;
            }

            if c.is_ascii_alphabetic() || c == b'_' {
                let start = self.pos;
                while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
                    self.bump();
                }
                let s = std::str::from_utf8(&self.src[start..self.pos])
                    .unwrap()
                    .to_string();
                out.push(match s.as_str() {
                    "fn" => Tok::Fn,
                    "let" => Tok::Let,
                    "mut" => Tok::Mut,
                    "return" => Tok::Return,
                    "if" => Tok::If,
                    "else" => Tok::Else,
                    "while" => Tok::While,
                    "for" => Tok::For,
                    "in" => Tok::In,
                    "parallel" => Tok::Parallel,
                    "public" => Tok::Public,
                    "private" => Tok::Private,
                    "export" => Tok::Export,
                    _ => Tok::Ident(s),
                });
                continue;
            }

            if c.is_ascii_digit() {
                let start = self.pos;
                while self.peek().is_ascii_digit() {
                    self.bump();
                }
                // 浮点：3.14  → 检查 . 后跟数字
                if self.peek() == b'.' && self.peek2().is_ascii_digit() {
                    self.bump(); // .
                    while self.peek().is_ascii_digit() {
                        self.bump();
                    }
                    let f: f64 = std::str::from_utf8(&self.src[start..self.pos])
                        .unwrap()
                        .parse()
                        .map_err(|e| format!("bad float: {}", e))?;
                    out.push(Tok::Float(f));
                    continue;
                }
                let n: i64 = std::str::from_utf8(&self.src[start..self.pos])
                    .unwrap()
                    .parse()
                    .map_err(|e| format!("bad int: {}", e))?;
                out.push(Tok::Int(n));
                continue;
            }

            if c == b'"' {
                self.bump();
                let mut s = String::new();
                loop {
                    let ch = self.bump();
                    if ch == 0 {
                        return Err("unterminated string".into());
                    }
                    if ch == b'"' {
                        break;
                    }
                    if ch == b'\\' {
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
                        // bump() 已读取首字节，从该字符的起始位置解码。
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
                out.push(Tok::Str(s));
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
                        return Err("unexpected '.'".into());
                    }
                }
                _ => return Err(format!("unexpected char '{}'", c as char)),
            };
            out.push(t);
        }
        Ok(out)
    }
}
