// src/script/lexer.rs — Hand-written lexer for the RustyScript language.
// No external dependencies; runs in no_std with alloc.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

// ── Virtual-key codes for key names ─────────────────────────────────────────
// Duplicated here to avoid a circular dependency on controls::vk.
mod vk_code {
    pub const F1:  u8 = 0x70;
    pub const F2:  u8 = 0x71;
    pub const F3:  u8 = 0x72;
    pub const F4:  u8 = 0x73;
    pub const F5:  u8 = 0x74;
    pub const F6:  u8 = 0x75;
    pub const F7:  u8 = 0x76;
    pub const F8:  u8 = 0x77;
    pub const F9:  u8 = 0x78;
    pub const F10: u8 = 0x79;
    pub const F11: u8 = 0x7A;
    pub const F12: u8 = 0x7B;
    pub const NUM0: u8 = 0x60;
    pub const NUM1: u8 = 0x61;
    pub const NUM2: u8 = 0x62;
    pub const NUM3: u8 = 0x63;
    pub const NUM4: u8 = 0x64;
    pub const NUM5: u8 = 0x65;
    pub const NUM6: u8 = 0x66;
    pub const NUM7: u8 = 0x67;
    pub const NUM8: u8 = 0x68;
    pub const NUM9: u8 = 0x69;
}

// ── Token kinds ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Int(i32),
    Float(f32),
    Bool(bool),
    Str(String),

    // Keywords
    Var,
    Let,
    Fn,
    If,
    Else,
    While,
    Break,
    Continue,
    Return,
    On,
    And,
    Or,
    Not,
    True,
    False,

    // Event sub-keywords
    Start,
    Frame,
    Key,
    Held,

    // Key name literals (hold the VK code)
    KeyName(u8),

    // Identifier
    Ident(String),

    // Punctuation
    Dot,
    Comma,
    Colon,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Newline,

    // Compound assignment operators
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,

    // Single-char operators (= and == share the Eq prefix so order matters)
    Eq,       // =
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Percent,  // %

    // Comparison operators
    EqEq,    // ==
    BangEq,  // !=
    Lt,      // <
    Gt,      // >
    LtEq,    // <=
    GtEq,    // >=

    // Special
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: u32,
}

// ── Lexer ────────────────────────────────────────────────────────────────────

pub struct Lexer<'a> {
    src:  &'a str,
    pos:  usize,
    line: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src, pos: 0, line: 1 }
    }

    // ── Public entry point ───────────────────────────────────────────────────

    /// Tokenize the entire source.  Returns a Vec of tokens ending with Eof.
    /// Newline tokens are emitted at the end of non-blank, non-comment lines.
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut out: Vec<Token> = Vec::new();
        let mut line_had_token = false;

        loop {
            self.skip_spaces(); // skip spaces/tabs only (not newlines)

            if self.at_end() {
                if line_had_token {
                    out.push(Token { kind: TokenKind::Newline, line: self.line });
                }
                out.push(Token { kind: TokenKind::Eof, line: self.line });
                break;
            }

            let ch = self.peek();

            // Comment — skip to end of line, then treat as newline boundary
            if ch == '#' {
                while !self.at_end() && self.peek() != '\n' {
                    self.advance();
                }
                // The '\n' will be consumed below
                continue;
            }

            // Newline character
            if ch == '\n' {
                self.advance();
                self.line += 1;
                if line_had_token {
                    out.push(Token { kind: TokenKind::Newline, line: self.line - 1 });
                    line_had_token = false;
                }
                continue;
            }

            // Carriage return (Windows line endings)
            if ch == '\r' {
                self.advance();
                continue;
            }

            // Everything else produces a real token
            let tok = self.next_token();
            line_had_token = true;
            out.push(tok);
        }

        out
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn at_end(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> char {
        // Safety: we only call peek() after checking !at_end().
        self.src[self.pos..].chars().next().unwrap_or('\0')
    }

    fn peek2(&self) -> char {
        let mut it = self.src[self.pos..].chars();
        it.next();
        it.next().unwrap_or('\0')
    }

    fn advance(&mut self) -> char {
        let ch = self.peek();
        self.pos += ch.len_utf8();
        ch
    }

    /// Skip horizontal whitespace (spaces and tabs).
    fn skip_spaces(&mut self) {
        while !self.at_end() {
            let c = self.peek();
            if c == ' ' || c == '\t' {
                self.advance();
            } else {
                break;
            }
        }
    }

    // ── Token dispatch ───────────────────────────────────────────────────────

    fn next_token(&mut self) -> Token {
        let line = self.line;
        let ch = self.advance();

        let kind = match ch {
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '.' => TokenKind::Dot,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            '%' => TokenKind::Percent,
            '+' => {
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::PlusEq
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::MinusEq
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::StarEq
                } else {
                    TokenKind::Star
                }
            }
            '/' => {
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::SlashEq
                } else {
                    TokenKind::Slash
                }
            }
            '=' => {
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::BangEq
                } else {
                    // Lone '!' is not valid; emit a placeholder Ident so the parser
                    // can produce a meaningful error rather than panicking.
                    TokenKind::Ident(String::from("!"))
                }
            }
            '<' => {
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if !self.at_end() && self.peek() == '=' {
                    self.advance();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            '"' => self.lex_string(),
            c if c.is_ascii_digit() => self.lex_number(c),
            c if c == '_' || c.is_alphabetic() => self.lex_ident_or_keyword(c),
            _ => {
                // Unknown character — emit Ident with the char so parser errors are clear.
                let mut s = String::new();
                s.push(ch);
                TokenKind::Ident(s)
            }
        };

        Token { kind, line }
    }

    // ── String literal ───────────────────────────────────────────────────────

    fn lex_string(&mut self) -> TokenKind {
        let mut s = String::new();
        loop {
            if self.at_end() || self.peek() == '\n' {
                // Unterminated string — return what we have.
                break;
            }
            let c = self.advance();
            if c == '"' {
                break;
            }
            if c == '\\' && !self.at_end() {
                let esc = self.advance();
                match esc {
                    'n'  => s.push('\n'),
                    't'  => s.push('\t'),
                    'r'  => s.push('\r'),
                    '"'  => s.push('"'),
                    '\\' => s.push('\\'),
                    _    => { s.push('\\'); s.push(esc); }
                }
            } else {
                s.push(c);
            }
        }
        TokenKind::Str(s)
    }

    // ── Numeric literal ──────────────────────────────────────────────────────

    fn lex_number(&mut self, first: char) -> TokenKind {
        // Hex literal: 0x… or 0X…
        if first == '0' && !self.at_end() {
            let next = self.peek();
            if next == 'x' || next == 'X' {
                self.advance(); // consume 'x'
                let mut hex = String::new();
                while !self.at_end() {
                    let c = self.peek();
                    if c.is_ascii_hexdigit() {
                        hex.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let val = parse_hex(&hex);
                return TokenKind::Int(val);
            }
        }

        let mut raw = String::new();
        raw.push(first);
        let mut is_float = false;

        while !self.at_end() {
            let c = self.peek();
            if c.is_ascii_digit() {
                raw.push(c);
                self.advance();
            } else if c == '.' && !is_float && self.peek2().is_ascii_digit() {
                is_float = true;
                raw.push(c);
                self.advance();
            } else {
                break;
            }
        }

        if is_float {
            let val = parse_f32(&raw);
            TokenKind::Float(val)
        } else {
            let val = parse_i32(&raw);
            TokenKind::Int(val)
        }
    }

    // ── Identifier / keyword ─────────────────────────────────────────────────

    fn lex_ident_or_keyword(&mut self, first: char) -> TokenKind {
        let mut s = String::new();
        s.push(first);

        while !self.at_end() {
            let c = self.peek();
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        // Map to keyword or key-name token; fall through to Ident.
        match s.as_str() {
            "var"      => TokenKind::Var,
            "let"      => TokenKind::Let,
            "fn"       => TokenKind::Fn,
            "if"       => TokenKind::If,
            "else"     => TokenKind::Else,
            "while"    => TokenKind::While,
            "break"    => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return"   => TokenKind::Return,
            "on"       => TokenKind::On,
            "and"      => TokenKind::And,
            "or"       => TokenKind::Or,
            "not"      => TokenKind::Not,
            "true"     => TokenKind::True,
            "false"    => TokenKind::False,
            "start"    => TokenKind::Start,
            "frame"    => TokenKind::Frame,
            "key"      => TokenKind::Key,
            "held"     => TokenKind::Held,
            // Key names
            "F1"   => TokenKind::KeyName(vk_code::F1),
            "F2"   => TokenKind::KeyName(vk_code::F2),
            "F3"   => TokenKind::KeyName(vk_code::F3),
            "F4"   => TokenKind::KeyName(vk_code::F4),
            "F5"   => TokenKind::KeyName(vk_code::F5),
            "F6"   => TokenKind::KeyName(vk_code::F6),
            "F7"   => TokenKind::KeyName(vk_code::F7),
            "F8"   => TokenKind::KeyName(vk_code::F8),
            "F9"   => TokenKind::KeyName(vk_code::F9),
            "F10"  => TokenKind::KeyName(vk_code::F10),
            "F11"  => TokenKind::KeyName(vk_code::F11),
            "F12"  => TokenKind::KeyName(vk_code::F12),
            "NUM0" => TokenKind::KeyName(vk_code::NUM0),
            "NUM1" => TokenKind::KeyName(vk_code::NUM1),
            "NUM2" => TokenKind::KeyName(vk_code::NUM2),
            "NUM3" => TokenKind::KeyName(vk_code::NUM3),
            "NUM4" => TokenKind::KeyName(vk_code::NUM4),
            "NUM5" => TokenKind::KeyName(vk_code::NUM5),
            "NUM6" => TokenKind::KeyName(vk_code::NUM6),
            "NUM7" => TokenKind::KeyName(vk_code::NUM7),
            "NUM8" => TokenKind::KeyName(vk_code::NUM8),
            "NUM9" => TokenKind::KeyName(vk_code::NUM9),
            _      => TokenKind::Ident(s),
        }
    }
}

// ── Simple numeric parsers (no_std — cannot use str::parse) ─────────────────

fn parse_hex(s: &str) -> i32 {
    let mut acc: u32 = 0;
    for b in s.bytes() {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => continue,
        };
        acc = acc.saturating_mul(16).saturating_add(digit as u32);
    }
    acc as i32
}

fn parse_i32(s: &str) -> i32 {
    let (s, neg) = if s.starts_with('-') { (&s[1..], true) } else { (s, false) };
    let mut acc: i32 = 0;
    for b in s.bytes() {
        if b >= b'0' && b <= b'9' {
            acc = acc.saturating_mul(10).saturating_add((b - b'0') as i32);
        }
    }
    if neg { acc.saturating_neg() } else { acc }
}

fn parse_f32(s: &str) -> f32 {
    // Split on '.'
    let (int_part, frac_part) = if let Some(dot) = s.find('.') {
        (&s[..dot], &s[dot + 1..])
    } else {
        (s, "")
    };

    let (int_part, neg) = if int_part.starts_with('-') {
        (&int_part[1..], true)
    } else {
        (int_part, false)
    };

    let mut val: f32 = 0.0;
    for b in int_part.bytes() {
        if b >= b'0' && b <= b'9' {
            val = val * 10.0 + (b - b'0') as f32;
        }
    }

    let mut frac: f32 = 0.1;
    for b in frac_part.bytes() {
        if b >= b'0' && b <= b'9' {
            val += (b - b'0') as f32 * frac;
            frac *= 0.1;
        }
    }

    if neg { -val } else { val }
}
