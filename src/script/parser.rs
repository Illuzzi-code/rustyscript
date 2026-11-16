// src/script/parser.rs — Recursive-descent parser for RustyScript.
// Produces an Ast (Vec<TopLevel>) from a flat token stream.

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use super::lexer::{Token, TokenKind};

// ── Public AST types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParseError {
    pub msg:  String,
    pub line: u32,
}

pub type Ast = Vec<TopLevel>;

#[derive(Debug, Clone)]
pub enum TopLevel {
    VarDecl { name: String, init: Expr,                              line: u32 },
    FnDef   { name: String, params: Vec<(String, Type)>, body: Vec<Stmt>, line: u32 },
    Event   { kind: EventKind, body: Vec<Stmt>,                      line: u32 },
}

#[derive(Debug, Clone)]
pub enum EventKind {
    Start,
    Frame,
    Key(u8),
    Held(u8),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let     { name: String, init: Expr, line: u32 },
    Assign  { target: LValue, op: AssignOp, value: Expr, line: u32 },
    If      {
        cond:    Expr,
        then:    Vec<Stmt>,
        elseifs: Vec<(Expr, Vec<Stmt>)>,
        else_:   Option<Vec<Stmt>>,
        line:    u32,
    },
    While   { cond: Expr, body: Vec<Stmt>, line: u32 },
    Break(u32),
    Continue(u32),
    Return { value: Option<Expr>, line: u32 },
    Call    { name: String, args: Vec<Expr>, line: u32 },
    NativeSet { field: NativeField, value: Expr, line: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LValue {
    Var(String),
    Field(NativeField),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeField {
    Health,
    MaxHealth,
    Armor,
    X,
    Y,
    Z,
    Money,
    WantedLevel,
    MaxWanted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOp { Set, Add, Sub, Mul, Div }

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i32),
    Float(f32),
    Bool(bool),
    Str(String),
    Var(String),
    Field(NativeField),
    BinOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    UnOp  { op: UnOp,  operand: Box<Expr> },
    Call  { name: String, args: Vec<Expr> },
    NativeCall { func: NativeFunc, args: Vec<Expr> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeFunc {
    ShowHelp,
    ShowMessage,
    SetTime,
    GetHour,
    GetMinute,
    SetWeather,
    SpawnRhino,
    SpawnHydra,
    SpawnHunter,
    SpawnMonster,
    SpawnHotring,
    SpawnStretch,
    SpawnCaddy,
    Wait,
    // Blips
    AddBlip,
    AddBlipSprite,
    SetBlipColour,
    SetBlipScale,
    SetBlipSprite,
    SetBlipDisplay,
    RemoveBlip,
    // Wanted
    SetWanted,
    ClearWanted,
    SetMaxWanted,
    // Coronas
    DrawCorona,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type { Int, Float, Bool, Str }

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp { Neg, Not }

// ── Parser ───────────────────────────────────────────────────────────────────

pub struct Parser {
    tokens: Vec<Token>,
    pos:    usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // ── Entry point ──────────────────────────────────────────────────────────

    pub fn parse(&mut self) -> Result<Ast, ParseError> {
        let mut ast = Vec::new();
        self.skip_newlines();

        while !self.at_end() {
            let top = self.parse_toplevel()?;
            ast.push(top);
            self.skip_newlines();
        }

        if ast.is_empty() {
            return Err(ParseError {
                msg:  String::from("error[E07]: empty file or no event blocks"),
                line: 1,
            });
        }
        Ok(ast)
    }

    // ── Token helpers ────────────────────────────────────────────────────────

    fn at_end(&self) -> bool {
        matches!(self.cur().kind, TokenKind::Eof)
    }

    fn cur(&self) -> &Token {
        // Tokens always end with Eof so this is always in-bounds.
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.cur().kind
    }

    fn cur_line(&self) -> u32 {
        self.cur().line
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn skip_newlines(&mut self) {
        while matches!(self.cur().kind, TokenKind::Newline) {
            self.advance();
        }
    }

    /// Consume one Newline (or Eof) to end a statement.
    fn expect_newline(&mut self) -> Result<(), ParseError> {
        match self.peek_kind() {
            TokenKind::Newline | TokenKind::Eof => {
                self.advance();
                Ok(())
            }
            _ => Err(self.err("expected newline after statement")),
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let line = self.cur_line();
        match self.cur().kind.clone() {
            TokenKind::Ident(s) => { self.advance(); Ok(s) }
            _ => Err(ParseError {
                msg:  format!("expected identifier, got {:?}", self.peek_kind()),
                line,
            }),
        }
    }

    fn expect_token(&mut self, want: &TokenKind) -> Result<(), ParseError> {
        if core::mem::discriminant(self.peek_kind()) == core::mem::discriminant(want) {
            self.advance();
            Ok(())
        } else {
            Err(self.err(&format!("expected {:?}, got {:?}", want, self.peek_kind())))
        }
    }

    fn err(&self, msg: &str) -> ParseError {
        ParseError { msg: String::from(msg), line: self.cur_line() }
    }

    // ── Top-level items ──────────────────────────────────────────────────────

    fn parse_toplevel(&mut self) -> Result<TopLevel, ParseError> {
        let line = self.cur_line();
        match self.peek_kind().clone() {
            TokenKind::Var => self.parse_var_decl(),
            TokenKind::Fn  => self.parse_fn_def(),
            TokenKind::On  => self.parse_event(),
            _ => Err(ParseError {
                msg:  format!("expected var, fn, or on — got {:?}", self.peek_kind()),
                line,
            }),
        }
    }

    fn parse_var_decl(&mut self) -> Result<TopLevel, ParseError> {
        let line = self.cur_line();
        self.advance(); // consume 'var'
        let name = self.expect_ident()?;
        self.expect_token(&TokenKind::Eq)?;
        let init = self.parse_expr()?;
        self.expect_newline()?;
        Ok(TopLevel::VarDecl { name, init, line })
    }

    fn parse_fn_def(&mut self) -> Result<TopLevel, ParseError> {
        let line = self.cur_line();
        self.advance(); // consume 'fn'
        let name = self.expect_ident()?;
        self.expect_token(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.expect_token(&TokenKind::RParen)?;
        self.skip_newlines();
        let body = self.parse_block()?;
        Ok(TopLevel::FnDef { name, params, body, line })
    }

    fn parse_param_list(&mut self) -> Result<Vec<(String, Type)>, ParseError> {
        let mut params = Vec::new();
        if matches!(self.peek_kind(), TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let pname = self.expect_ident()?;
            self.expect_token(&TokenKind::Colon)?;
            let ty = self.parse_type()?;
            params.push((pname, ty));
            if !matches!(self.peek_kind(), TokenKind::Comma) {
                break;
            }
            self.advance(); // consume ','
        }
        Ok(params)
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        match self.cur().kind.clone() {
            TokenKind::Ident(ref s) => {
                let ty = match s.as_str() {
                    "int"   => Type::Int,
                    "float" => Type::Float,
                    "bool"  => Type::Bool,
                    "string"=> Type::Str,
                    _ => return Err(self.err("unknown type; expected int, float, bool, or string")),
                };
                self.advance();
                Ok(ty)
            }
            _ => Err(self.err("expected type name")),
        }
    }

    fn parse_event(&mut self) -> Result<TopLevel, ParseError> {
        let line = self.cur_line();
        self.advance(); // consume 'on'

        let kind = match self.peek_kind().clone() {
            TokenKind::Start => { self.advance(); EventKind::Start }
            TokenKind::Frame => { self.advance(); EventKind::Frame }
            TokenKind::Key   => {
                self.advance();
                let vk = self.parse_key_name()?;
                EventKind::Key(vk)
            }
            TokenKind::Held  => {
                self.advance();
                let vk = self.parse_key_name()?;
                EventKind::Held(vk)
            }
            _ => return Err(self.err("expected start, frame, key, or held after 'on'")),
        };

        self.skip_newlines();
        let body = self.parse_block()?;
        Ok(TopLevel::Event { kind, body, line })
    }

    fn parse_key_name(&mut self) -> Result<u8, ParseError> {
        match self.cur().kind.clone() {
            TokenKind::KeyName(vk) => { self.advance(); Ok(vk) }
            // Permitir literales numéricos (incluyendo hex 0x..) como VK code
            TokenKind::Int(v) => {
                if v < 0 || v > 255 {
                    return Err(ParseError {
                        msg:  format!("VK code fuera de rango: {}", v),
                        line: self.cur_line(),
                    });
                }
                self.advance();
                Ok(v as u8)
            }
            _ => Err(ParseError {
                msg:  format!("error[E06]: unknown key: {:?}", self.peek_kind()),
                line: self.cur_line(),
            }),
        }
    }

    // ── Block ────────────────────────────────────────────────────────────────

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect_token(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut stmts = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            let s = self.parse_stmt()?;
            stmts.push(s);
            self.skip_newlines();
        }
        self.expect_token(&TokenKind::RBrace)?;
        // Block may be followed by a newline; consume it.
        if matches!(self.peek_kind(), TokenKind::Newline) {
            self.advance();
        }
        Ok(stmts)
    }

    // ── Statements ───────────────────────────────────────────────────────────

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let line = self.cur_line();
        match self.peek_kind().clone() {
            TokenKind::Let      => self.parse_let(line),
            TokenKind::If       => self.parse_if(line),
            TokenKind::While    => self.parse_while(line),
            TokenKind::Break    => { self.advance(); self.expect_newline()?; Ok(Stmt::Break(line)) }
            TokenKind::Continue => { self.advance(); self.expect_newline()?; Ok(Stmt::Continue(line)) }
            TokenKind::Return => self.parse_return(line),
            // "player.<field>" — assignment or native call
            TokenKind::Ident(ref s) if s == "player" => self.parse_player_stmt(line),
            // Could be a user-function call or a variable assignment
            TokenKind::Ident(_) => self.parse_ident_stmt(line),
            // Native function names (they are Idents resolved below, but handle known keywords)
            _ => Err(ParseError {
                msg:  format!("unexpected token in statement: {:?}", self.peek_kind()),
                line,
            }),
        }
    }

    fn parse_let(&mut self, line: u32) -> Result<Stmt, ParseError> {
        self.advance(); // consume 'let'
        let name = self.expect_ident()?;
        self.expect_token(&TokenKind::Eq)?;
        let init = self.parse_expr()?;
        self.expect_newline()?;
        Ok(Stmt::Let { name, init, line })
    }

    fn parse_if(&mut self, line: u32) -> Result<Stmt, ParseError> {
        self.advance(); // consume 'if'
        let cond = self.parse_expr()?;
        self.skip_newlines();
        let then = self.parse_block()?;
        self.skip_newlines();

        let mut elseifs = Vec::new();
        let mut else_   = None;

        loop {
            if !matches!(self.peek_kind(), TokenKind::Else) {
                break;
            }
            self.advance(); // consume 'else'

            if matches!(self.peek_kind(), TokenKind::If) {
                self.advance(); // consume 'if'
                let ei_cond = self.parse_expr()?;
                self.skip_newlines();
                let ei_body = self.parse_block()?;
                self.skip_newlines();
                elseifs.push((ei_cond, ei_body));
            } else {
                self.skip_newlines();
                let eb = self.parse_block()?;
                self.skip_newlines();
                else_ = Some(eb);
                break;
            }
        }

        Ok(Stmt::If { cond, then, elseifs, else_, line })
    }

    fn parse_while(&mut self, line: u32) -> Result<Stmt, ParseError> {
        self.advance(); // consume 'while'
        let cond = self.parse_expr()?;
        self.skip_newlines();
        let body = self.parse_block()?;
        Ok(Stmt::While { cond, body, line })
    }

    fn parse_return(&mut self, line: u32) -> Result<Stmt, ParseError> {
        self.advance();
        if matches!(self.peek_kind(), TokenKind::Newline | TokenKind::RBrace | TokenKind::Eof) {
            self.expect_newline()?;
            return Ok(Stmt::Return { value: None, line });
        }
        let value = self.parse_expr()?;
        self.expect_newline()?;
        Ok(Stmt::Return { value: Some(value), line })
    }

    /// Parse `player.<field>` assignment.
    fn parse_player_stmt(&mut self, line: u32) -> Result<Stmt, ParseError> {
        self.advance(); // consume 'player'
        self.expect_token(&TokenKind::Dot)?;
        let field_name = self.expect_ident()?;
        let field = ident_to_native_field(&field_name).ok_or_else(|| ParseError {
            msg:  format!("unknown player field: {}", field_name),
            line,
        })?;
        let op = self.parse_assign_op()?;
        let value = self.parse_expr()?;
        self.expect_newline()?;
        Ok(Stmt::NativeSet { field: field.clone(), value: apply_assign_op_to_expr(op, LValue::Field(field.clone()), value), line })
    }

    /// Parse a statement that starts with an identifier.
    /// Either a function call (`name(...)`) or a variable assignment (`name = expr`).
    fn parse_ident_stmt(&mut self, line: u32) -> Result<Stmt, ParseError> {
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(s) => s,
            _ => unreachable!(),
        };

        match self.peek_kind().clone() {
            // Function call
            TokenKind::LParen => {
                self.advance(); // consume '('
                let args = self.parse_arg_list()?;
                self.expect_token(&TokenKind::RParen)?;
                self.expect_newline()?;

                // Resolve as native if possible, otherwise user call
                if let Some(func) = ident_to_native_func(&name) {
                    Ok(Stmt::Call { name: format!("__native__{}", name), args: vec![
                        Expr::NativeCall { func, args }
                    ], line })
                } else {
                    Ok(Stmt::Call { name, args, line })
                }
            }
            // Assignment operator
            _ => {
                let op = self.parse_assign_op()?;
                let rhs = self.parse_expr()?;
                self.expect_newline()?;
                let lval = LValue::Var(name.clone());
                Ok(Stmt::Assign {
                    target: lval.clone(),
                    op: op.clone(),
                    value: apply_assign_op_to_expr(op, lval, rhs),
                    line,
                })
            }
        }
    }

    fn parse_assign_op(&mut self) -> Result<AssignOp, ParseError> {
        match self.peek_kind().clone() {
            TokenKind::Eq      => { self.advance(); Ok(AssignOp::Set) }
            TokenKind::PlusEq  => { self.advance(); Ok(AssignOp::Add) }
            TokenKind::MinusEq => { self.advance(); Ok(AssignOp::Sub) }
            TokenKind::StarEq  => { self.advance(); Ok(AssignOp::Mul) }
            TokenKind::SlashEq => { self.advance(); Ok(AssignOp::Div) }
            _ => Err(self.err("expected assignment operator (=, +=, -=, *=, /=)")),
        }
    }

    // ── Argument list ────────────────────────────────────────────────────────

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if matches!(self.peek_kind(), TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if !matches!(self.peek_kind(), TokenKind::Comma) {
                break;
            }
            self.advance(); // consume ','
        }
        Ok(args)
    }

    // ── Expressions (Pratt / recursive descent) ──────────────────────────────

    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_and()?;
        while matches!(self.peek_kind(), TokenKind::Or) {
            self.advance();
            let rhs = self.parse_and()?;
            lhs = Expr::BinOp { op: BinOp::Or, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_cmp()?;
        while matches!(self.peek_kind(), TokenKind::And) {
            self.advance();
            let rhs = self.parse_cmp()?;
            lhs = Expr::BinOp { op: BinOp::And, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_add()?;
        let op = match self.peek_kind() {
            TokenKind::EqEq   => BinOp::Eq,
            TokenKind::BangEq => BinOp::Ne,
            TokenKind::Lt     => BinOp::Lt,
            TokenKind::Gt     => BinOp::Gt,
            TokenKind::LtEq   => BinOp::Le,
            TokenKind::GtEq   => BinOp::Ge,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.parse_add()?;
        Ok(Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) })
    }

    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Plus  => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Star    => BinOp::Mul,
                TokenKind::Slash   => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek_kind().clone() {
            TokenKind::Minus => {
                self.advance();
                let e = self.parse_unary()?;
                Ok(Expr::UnOp { op: UnOp::Neg, operand: Box::new(e) })
            }
            TokenKind::Not => {
                self.advance();
                let e = self.parse_unary()?;
                Ok(Expr::UnOp { op: UnOp::Not, operand: Box::new(e) })
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let line = self.cur_line();
        match self.peek_kind().clone() {
            TokenKind::Int(v)   => { self.advance(); Ok(Expr::Int(v)) }
            TokenKind::Float(v) => { self.advance(); Ok(Expr::Float(v)) }
            TokenKind::Bool(v)  => { self.advance(); Ok(Expr::Bool(v)) }
            TokenKind::True     => { self.advance(); Ok(Expr::Bool(true)) }
            TokenKind::False    => { self.advance(); Ok(Expr::Bool(false)) }
            TokenKind::Str(s)   => { self.advance(); Ok(Expr::Str(s)) }

            TokenKind::LParen => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect_token(&TokenKind::RParen)?;
                Ok(e)
            }

            // `player.<field>`
            TokenKind::Ident(ref s) if s == "player" => {
                self.advance();
                self.expect_token(&TokenKind::Dot)?;
                let fname = self.expect_ident()?;
                match ident_to_native_field(&fname) {
                    Some(f) => Ok(Expr::Field(f)),
                    None    => Err(ParseError {
                        msg:  format!("unknown player field: {}", fname),
                        line,
                    }),
                }
            }

            // Identifier: either a function call, a variable, or a native func call
            TokenKind::Ident(name) => {
                self.advance();
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.advance(); // consume '('
                    let args = self.parse_arg_list()?;
                    self.expect_token(&TokenKind::RParen)?;
                    if let Some(func) = ident_to_native_func(&name) {
                        Ok(Expr::NativeCall { func, args })
                    } else {
                        Ok(Expr::Call { name, args })
                    }
                } else {
                    Ok(Expr::Var(name))
                }
            }

            _ => Err(ParseError {
                msg:  format!("unexpected token in expression: {:?}", self.peek_kind()),
                line,
            }),
        }
    }
}

// ── Free-function helpers ────────────────────────────────────────────────────

fn ident_to_native_field(s: &str) -> Option<NativeField> {
    match s {
        "health"       => Some(NativeField::Health),
        "max_health"   => Some(NativeField::MaxHealth),
        "armor"        => Some(NativeField::Armor),
        "x"            => Some(NativeField::X),
        "y"            => Some(NativeField::Y),
        "z"            => Some(NativeField::Z),
        "money"        => Some(NativeField::Money),
        "wanted_level" => Some(NativeField::WantedLevel),
        "max_wanted"   => Some(NativeField::MaxWanted),
        _              => None,
    }
}

fn ident_to_native_func(s: &str) -> Option<NativeFunc> {
    match s {
        "show_help"      => Some(NativeFunc::ShowHelp),
        "show_message"   => Some(NativeFunc::ShowMessage),
        "set_time"       => Some(NativeFunc::SetTime),
        "get_hour"       => Some(NativeFunc::GetHour),
        "get_minute"     => Some(NativeFunc::GetMinute),
        "set_weather"    => Some(NativeFunc::SetWeather),
        "spawn_rhino"    => Some(NativeFunc::SpawnRhino),
        "spawn_hydra"    => Some(NativeFunc::SpawnHydra),
        "spawn_hunter"   => Some(NativeFunc::SpawnHunter),
        "spawn_monster"  => Some(NativeFunc::SpawnMonster),
        "spawn_hotring"  => Some(NativeFunc::SpawnHotring),
        "spawn_stretch"  => Some(NativeFunc::SpawnStretch),
        "spawn_caddy"    => Some(NativeFunc::SpawnCaddy),
        "wait"           => Some(NativeFunc::Wait),
        // Blips
        "add_blip"         => Some(NativeFunc::AddBlip),
        "add_blip_sprite"  => Some(NativeFunc::AddBlipSprite),
        "set_blip_colour"  => Some(NativeFunc::SetBlipColour),
        "set_blip_scale"   => Some(NativeFunc::SetBlipScale),
        "set_blip_sprite"  => Some(NativeFunc::SetBlipSprite),
        "set_blip_display" => Some(NativeFunc::SetBlipDisplay),
        "remove_blip"      => Some(NativeFunc::RemoveBlip),
        // Wanted
        "set_wanted"     => Some(NativeFunc::SetWanted),
        "clear_wanted"   => Some(NativeFunc::ClearWanted),
        "set_max_wanted" => Some(NativeFunc::SetMaxWanted),
        // Coronas
        "draw_corona"    => Some(NativeFunc::DrawCorona),
        _                => None,
    }
}

/// For compound assignment ops (+= etc.) we rewrite `x += e` as `x = x + e`
/// so the compiler and VM only need a simple Set opcode.
fn apply_assign_op_to_expr(op: AssignOp, lval: LValue, rhs: Expr) -> Expr {
    match op {
        AssignOp::Set => rhs,
        _ => {
            let lhs_expr = match lval {
                LValue::Var(ref n)   => Expr::Var(n.clone()),
                LValue::Field(ref f) => Expr::Field(f.clone()),
            };
            let bin_op = match op {
                AssignOp::Add => BinOp::Add,
                AssignOp::Sub => BinOp::Sub,
                AssignOp::Mul => BinOp::Mul,
                AssignOp::Div => BinOp::Div,
                AssignOp::Set => unreachable!(),
            };
            Expr::BinOp { op: bin_op, lhs: Box::new(lhs_expr), rhs: Box::new(rhs) }
        }
    }
}

// ── Public parsing function ──────────────────────────────────────────────────

/// Parse a token stream into an AST, or return a ParseError.
pub fn parse(tokens: Vec<Token>) -> Result<Ast, ParseError> {
    Parser::new(tokens).parse()
}
