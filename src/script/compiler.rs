// src/script/compiler.rs — Compiles an AST into a Chunk of Opcodes.
// Performs semantic checks: type compatibility, undefined variables,
// recursion detection, break/continue placement, function count limit.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

pub use super::parser::{
    Ast, BinOp as PBinOp, Expr, EventKind, LValue,
    NativeField, NativeFunc, Stmt, TopLevel, UnOp as PUnOp,
};

// ── Public output types ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompileError {
    pub msg:  String,
    pub line: u32,
}

/// A compiled script ready to be executed by the VM.
#[derive(Clone)]
pub struct Chunk {
    pub code:      Vec<Opcode>,
    pub constants: Vec<Value>,
    // Handler entry points (instruction index into `code`)
    pub on_start:  Option<usize>,
    pub on_frame:  Option<usize>,
    pub on_keys:   Vec<(u8, usize)>,
    pub on_helds:  Vec<(u8, usize)>,
}

#[derive(Clone, Debug)]
pub enum Opcode {
    // Stack
    Push(usize),          // push constants[idx]
    Pop,
    Dup,
    // Locals (slot index within current frame)
    LoadLocal(u8),
    StoreLocal(u8),
    // Script-level globals (index into ScriptState.globals)
    LoadGlobal(u8),
    StoreGlobal(u8),
    // Native fields (player.*)
    LoadField(NativeField),
    StoreField(NativeField),
    // Arithmetic & logic
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or, Not, Neg,
    // Control flow
    Jump(usize),          // unconditional jump to absolute ip
    JumpIfFalse(usize),   // pop and jump if falsy
    // Calls
    CallNative(NativeFunc, u8),  // u8 = arg count
    CallUser(usize, u8),         // ip, arg count
    Return,
    // Loop control (addresses patched after emit)
    Break,
    Continue,
    // Yield: pops an int (ms) from the stack and pauses the VM until
    // that many real-time milliseconds have elapsed.
    Wait,
    // Frame end
    Halt,
}

#[derive(Clone, Debug)]
pub enum Value {
    Int(i32),
    Float(f32),
    Bool(bool),
    Str(String),
    Nil,
}

// ── Compiler internals ───────────────────────────────────────────────────────

/// A named user function recorded during the first pass.
struct UserFn {
    name:     String,
    ip:       usize,            // entry point in code
    arity:    u8,
    // param names in declaration order (used to allocate local slots)
    params:   Vec<String>,
}

/// Scope for a currently-compiling block.
struct Scope {
    locals: Vec<String>,
    // Indices of Break/Continue opcodes that need patching once the loop ends.
    breaks:    Vec<usize>,
    continues: Vec<usize>,
}

impl Scope {
    fn new() -> Self { Scope { locals: Vec::new(), breaks: Vec::new(), continues: Vec::new() } }
}

struct Compiler {
    chunk:         Chunk,
    globals:       Vec<String>,   // var-declared names
    functions:     Vec<UserFn>,   // fn-declared functions
    // Call stack of fn names being compiled (recursion detection)
    call_stack:    Vec<String>,
    // Local variable scopes (stack of frames)
    scope_stack:   Vec<Scope>,
    in_loop:       bool,
    // Pending break/continue indices from the innermost loop
    loop_breaks:   Vec<usize>,
    loop_conts:    Vec<usize>,
    in_function:   bool,
}

impl Compiler {
    fn new() -> Self {
        Compiler {
            chunk: Chunk {
                code:      Vec::new(),
                constants: Vec::new(),
                on_start:  None,
                on_frame:  None,
                on_keys:   Vec::new(),
                on_helds:  Vec::new(),
            },
            globals:     Vec::new(),
            functions:   Vec::new(),
            call_stack:  Vec::new(),
            scope_stack: Vec::new(),
            in_loop:     false,
            loop_breaks: Vec::new(),
            loop_conts:  Vec::new(),
            in_function: false,
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn emit(&mut self, op: Opcode) -> usize {
        let idx = self.chunk.code.len();
        self.chunk.code.push(op);
        idx
    }

    fn current_ip(&self) -> usize {
        self.chunk.code.len()
    }

    fn patch_jump(&mut self, idx: usize, target: usize) {
        match &mut self.chunk.code[idx] {
            Opcode::Jump(ref mut t) | Opcode::JumpIfFalse(ref mut t) => *t = target,
            _ => {}
        }
    }

    fn add_constant(&mut self, v: Value) -> usize {
        self.chunk.constants.push(v);
        self.chunk.constants.len() - 1
    }

    fn err(&self, msg: &str, line: u32) -> CompileError {
        CompileError { msg: String::from(msg), line }
    }

    // ── Variable lookup ──────────────────────────────────────────────────────

    /// Resolve a name to the nearest local slot.
    fn find_local(&self, name: &str) -> Option<u8> {
        // Search from innermost scope outward within the same function.
        // For simplicity we use a single flat locals vec per compile invocation;
        // scopes are only used for naming — the slot index is the position.
        if let Some(scope) = self.scope_stack.last() {
            for (i, n) in scope.locals.iter().enumerate().rev() {
                if n == name {
                    return Some(i as u8);
                }
            }
        }
        None
    }

    fn find_global(&self, name: &str) -> Option<u8> {
        self.globals.iter().position(|n| n == name).map(|i| i as u8)
    }

    fn declare_local(&mut self, name: String) -> u8 {
        let scope = self.scope_stack.last_mut().expect("no scope");
        let idx = scope.locals.len() as u8;
        scope.locals.push(name);
        idx
    }

    fn find_function(&self, name: &str) -> Option<&UserFn> {
        self.functions.iter().find(|f| f.name == name)
    }

    // ── First pass: collect globals and function stubs ───────────────────────

    fn first_pass(&mut self, ast: &Ast) -> Result<(), CompileError> {
        for top in ast {
            match top {
                TopLevel::VarDecl { name, .. } => {
                    if self.globals.len() >= 255 {
                        return Err(self.err("too many global variables (max 255)", 0));
                    }
                    if !self.globals.contains(name) {
                        self.globals.push(name.clone());
                    }
                }
                TopLevel::FnDef { name, params, line, .. } => {
                    if self.functions.len() >= 32 {
                        return Err(CompileError {
                            msg:  String::from("error[E04]: too many functions (max 32)"),
                            line: *line,
                        });
                    }
                    if self.functions.iter().any(|f| &f.name == name) {
                        return Err(CompileError {
                            msg:  format!("function '{}' already defined", name),
                            line: *line,
                        });
                    }
                    // ip placeholder — filled in second pass
                    self.functions.push(UserFn {
                        name:   name.clone(),
                        ip:     0,
                        arity:  params.len() as u8,
                        params: params.iter().map(|(n, _)| n.clone()).collect(),
                    });
                }
                TopLevel::Event { .. } => {}
            }
        }
        Ok(())
    }

    // ── Second pass: emit code ───────────────────────────────────────────────

    fn second_pass(&mut self, ast: &Ast) -> Result<(), CompileError> {
        // Emit global initialisers first (run before any event).
        for top in ast {
            if let TopLevel::VarDecl { name, init, line } = top {
                let gidx = self.globals.iter().position(|n| n == name)
                    .expect("global not found") as u8;
                self.scope_stack.push(Scope::new());
                self.emit_expr(init, *line)?;
                self.scope_stack.pop();
                self.emit(Opcode::StoreGlobal(gidx));
                self.emit(Opcode::Pop);
            }
        }

        // Emit function bodies (jump over them so they're only reached by CallUser).
        for top in ast {
            if let TopLevel::FnDef { name, params, body, line } = top {
                // Emit a jump to skip over the function body during linear execution.
                let skip_jump_idx = self.emit(Opcode::Jump(0));
                let fn_ip = self.current_ip();

                // Update the stub with the real ip.
                if let Some(f) = self.functions.iter_mut().find(|f| &f.name == name) {
                    f.ip = fn_ip;
                }

                // Compile body with params as locals.
                self.scope_stack.push(Scope::new());
                let previous_in_function = self.in_function;
                self.in_function = true;
                for pname in params.iter().map(|(n, _)| n.clone()) {
                    self.declare_local(pname);
                }
                self.call_stack.push(name.clone());
                self.compile_stmts(body, *line)?;
                self.call_stack.pop();
                self.scope_stack.pop();
                self.in_function = previous_in_function;

                self.emit(Opcode::Return);
                let after_fn = self.current_ip();
                self.patch_jump(skip_jump_idx, after_fn);
            }
        }

        // Emit event handlers.
        for top in ast {
            if let TopLevel::Event { kind, body, line } = top {
                let entry_ip = self.current_ip();
                self.scope_stack.push(Scope::new());
                self.compile_stmts(body, *line)?;
                self.scope_stack.pop();
                self.emit(Opcode::Halt);

                match kind {
                    EventKind::Start       => self.chunk.on_start = Some(entry_ip),
                    EventKind::Frame       => self.chunk.on_frame = Some(entry_ip),
                    EventKind::Key(vk)     => self.chunk.on_keys.push((*vk, entry_ip)),
                    EventKind::Held(vk)    => self.chunk.on_helds.push((*vk, entry_ip)),
                }
            }
        }

        Ok(())
    }

    // ── Statement emitter ────────────────────────────────────────────────────

    fn compile_stmts(&mut self, stmts: &[Stmt], default_line: u32) -> Result<(), CompileError> {
        for stmt in stmts {
            self.compile_stmt(stmt, default_line)?;
        }
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Stmt, _default_line: u32) -> Result<(), CompileError> {
        match stmt {
            Stmt::Let { name, init, line } => {
                self.emit_expr(init, *line)?;
                let slot = self.declare_local(name.clone());
                self.emit(Opcode::StoreLocal(slot));
                self.emit(Opcode::Pop);
            }

            Stmt::Assign { target, op: _, value, line } => {
                // op is already baked into value by the parser's apply_assign_op_to_expr.
                self.emit_expr(value, *line)?;
                match target {
                    LValue::Var(name) => {
                        if let Some(slot) = self.find_local(name) {
                            self.emit(Opcode::StoreLocal(slot));
                        } else if let Some(gidx) = self.find_global(name) {
                            self.emit(Opcode::StoreGlobal(gidx));
                        } else {
                            return Err(CompileError {
                                msg:  format!("error[E02]: variable not declared: {}", name),
                                line: *line,
                            });
                        }
                        self.emit(Opcode::Pop);
                    }
                    LValue::Field(f) => {
                        self.emit(Opcode::StoreField(f.clone()));
                        self.emit(Opcode::Pop);
                    }
                }
            }

            Stmt::NativeSet { field, value, line } => {
                self.emit_expr(value, *line)?;
                self.emit(Opcode::StoreField(field.clone()));
                self.emit(Opcode::Pop);
            }

            Stmt::If { cond, then, elseifs, else_, line } => {
                self.compile_if(cond, then, elseifs, else_.as_deref(), *line)?;
            }

            Stmt::While { cond, body, line } => {
                self.compile_while(cond, body, *line)?;
            }

            Stmt::Break(line) => {
                if !self.in_loop {
                    return Err(CompileError {
                        msg:  String::from("break outside loop"),
                        line: *line,
                    });
                }
                let idx = self.emit(Opcode::Break);
                self.loop_breaks.push(idx);
            }

            Stmt::Continue(line) => {
                if !self.in_loop {
                    return Err(CompileError {
                        msg:  String::from("continue outside loop"),
                        line: *line,
                    });
                }
                let idx = self.emit(Opcode::Continue);
                self.loop_conts.push(idx);
            }

            Stmt::Return { value, line } => {
                if !self.in_function {
                    return Err(self.err("return outside function", *line));
                }
                if let Some(expr) = value {
                    self.emit_expr(expr, *line)?;
                } else {
                    let idx = self.add_constant(Value::Nil);
                    self.emit(Opcode::Push(idx));
                }
                self.emit(Opcode::Return);
            }

            // A Stmt::Call whose name starts with "__native__" is a wrapper
            // around a native call that was stored as the single argument.
            Stmt::Call { name, args, line } if name.starts_with("__native__") => {
                // The single arg is the NativeCall expression.
                if let Some(expr) = args.first() {
                    self.emit_expr(expr, *line)?;
                    self.emit(Opcode::Pop); // discard return value
                }
            }

            Stmt::Call { name, args, line } => {
                let function = self.find_function(name).ok_or_else(|| CompileError {
                    msg: format!("error[E02]: undefined function '{}'", name),
                    line: *line,
                })?;
                if args.len() != function.arity as usize {
                    return Err(CompileError {
                        msg: format!(
                            "function '{}' expects {} arguments, got {}",
                            name, function.arity, args.len()
                        ),
                        line: *line,
                    });
                }
                // Emit arguments left-to-right (VM pops right-to-left).
                for arg in args {
                    self.emit_expr(arg, *line)?;
                }
                // Resolve user function.
                let (ip, arity) = {
                    if let Some(f) = self.find_function(name) {
                        // Recursion check
                        if self.call_stack.contains(&f.name) {
                            return Err(CompileError {
                                msg:  format!("error[E05]: recursion detected in function '{}'", name),
                                line: *line,
                            });
                        }
                        (f.ip, f.arity)
                    } else { unreachable!("function was checked above"); }
                };
                self.emit(Opcode::CallUser(ip, arity));
                self.emit(Opcode::Pop); // discard return value from statement context
            }
        }
        Ok(())
    }

    fn compile_if(
        &mut self,
        cond:    &Expr,
        then:    &[Stmt],
        elseifs: &[(Expr, Vec<Stmt>)],
        else_:   Option<&[Stmt]>,
        line:    u32,
    ) -> Result<(), CompileError> {
        // Emit condition
        self.emit_expr(cond, line)?;
        let jif_idx = self.emit(Opcode::JumpIfFalse(0)); // patched later

        // Then branch
        self.compile_stmts(then, line)?;
        let jump_end_idx = self.emit(Opcode::Jump(0)); // jumps past all else branches

        // Patch the JumpIfFalse to here (start of else-if / else)
        let else_start = self.current_ip();
        self.patch_jump(jif_idx, else_start);

        // Collect end-of-branch jumps so they can all be patched.
        let mut end_jumps: Vec<usize> = vec![jump_end_idx];

        for (ei_cond, ei_body) in elseifs {
            self.emit_expr(ei_cond, line)?;
            let jif2 = self.emit(Opcode::JumpIfFalse(0));
            self.compile_stmts(ei_body, line)?;
            let je = self.emit(Opcode::Jump(0));
            end_jumps.push(je);
            let next = self.current_ip();
            self.patch_jump(jif2, next);
        }

        if let Some(eb) = else_ {
            self.compile_stmts(eb, line)?;
        }

        let end_ip = self.current_ip();
        for idx in end_jumps {
            self.patch_jump(idx, end_ip);
        }
        Ok(())
    }

    fn compile_while(&mut self, cond: &Expr, body: &[Stmt], line: u32) -> Result<(), CompileError> {
        let was_in_loop = self.in_loop;
        self.in_loop = true;
        let saved_breaks = core::mem::take(&mut self.loop_breaks);
        let saved_conts  = core::mem::take(&mut self.loop_conts);

        let loop_start = self.current_ip();
        self.emit_expr(cond, line)?;
        let jif_idx = self.emit(Opcode::JumpIfFalse(0));

        self.scope_stack.push(Scope::new());
        self.compile_stmts(body, line)?;
        self.scope_stack.pop();

        self.emit(Opcode::Jump(loop_start));
        let loop_end = self.current_ip();
        self.patch_jump(jif_idx, loop_end);

        // Patch break → loop_end, continue → loop_start
        for idx in &self.loop_breaks {
            self.chunk.code[*idx] = Opcode::Jump(loop_end);
        }
        for idx in &self.loop_conts {
            self.chunk.code[*idx] = Opcode::Jump(loop_start);
        }

        self.loop_breaks = saved_breaks;
        self.loop_conts  = saved_conts;
        self.in_loop     = was_in_loop;
        Ok(())
    }

    // ── Expression emitter ───────────────────────────────────────────────────

    fn emit_expr(&mut self, expr: &Expr, line: u32) -> Result<(), CompileError> {
        match expr {
            Expr::Int(v) => {
                let idx = self.add_constant(Value::Int(*v));
                self.emit(Opcode::Push(idx));
            }
            Expr::Float(v) => {
                let idx = self.add_constant(Value::Float(*v));
                self.emit(Opcode::Push(idx));
            }
            Expr::Bool(v) => {
                let idx = self.add_constant(Value::Bool(*v));
                self.emit(Opcode::Push(idx));
            }
            Expr::Str(s) => {
                let idx = self.add_constant(Value::Str(s.clone()));
                self.emit(Opcode::Push(idx));
            }

            Expr::Var(name) => {
                if let Some(slot) = self.find_local(name) {
                    self.emit(Opcode::LoadLocal(slot));
                } else if let Some(gidx) = self.find_global(name) {
                    self.emit(Opcode::LoadGlobal(gidx));
                } else {
                    return Err(CompileError {
                        msg:  format!("error[E02]: variable not declared: {}", name),
                        line,
                    });
                }
            }

            Expr::Field(f) => {
                self.emit(Opcode::LoadField(f.clone()));
            }

            Expr::BinOp { op, lhs, rhs } => {
                self.emit_expr(lhs, line)?;
                self.emit_expr(rhs, line)?;
                let opcode = match op {
                    PBinOp::Add => Opcode::Add,
                    PBinOp::Sub => Opcode::Sub,
                    PBinOp::Mul => Opcode::Mul,
                    PBinOp::Div => Opcode::Div,
                    PBinOp::Mod => Opcode::Mod,
                    PBinOp::Eq  => Opcode::Eq,
                    PBinOp::Ne  => Opcode::Ne,
                    PBinOp::Lt  => Opcode::Lt,
                    PBinOp::Gt  => Opcode::Gt,
                    PBinOp::Le  => Opcode::Le,
                    PBinOp::Ge  => Opcode::Ge,
                    PBinOp::And => Opcode::And,
                    PBinOp::Or  => Opcode::Or,
                };
                self.emit(opcode);
            }

            Expr::UnOp { op, operand } => {
                self.emit_expr(operand, line)?;
                match op {
                    PUnOp::Neg => { self.emit(Opcode::Neg); }
                    PUnOp::Not => { self.emit(Opcode::Not); }
                }
            }

            Expr::Call { name, args } => {
                let function = self.find_function(name).ok_or_else(|| CompileError {
                    msg: format!("error[E02]: undefined function '{}'", name),
                    line,
                })?;
                if args.len() != function.arity as usize {
                    return Err(CompileError {
                        msg: format!(
                            "function '{}' expects {} arguments, got {}",
                            name, function.arity, args.len()
                        ),
                        line,
                    });
                }
                for arg in args {
                    self.emit_expr(arg, line)?;
                }
                let (ip, arity) = {
                    if let Some(f) = self.find_function(name) {
                        if self.call_stack.contains(&f.name) {
                            return Err(CompileError {
                                msg:  format!("error[E05]: recursion detected in function '{}'", name),
                                line,
                            });
                        }
                        (f.ip, f.arity)
                    } else { unreachable!("function was checked above"); }
                };
                self.emit(Opcode::CallUser(ip, arity));
            }

            Expr::NativeCall { func, args } => {
                let expected = native_arity(func);
                if args.len() != expected {
                    return Err(CompileError {
                        msg: format!(
                            "native function expects {} arguments, got {}",
                            expected, args.len()
                        ),
                        line,
                    });
                }
                // Wait is a special opcode, not a normal native call.
                if *func == NativeFunc::Wait {
                    // wait(ms) — push the ms value, then emit Wait.
                    for arg in args {
                        self.emit_expr(arg, line)?;
                    }
                    self.emit(Opcode::Wait);
                    return Ok(());
                }
                // Check disabled natives.
                match func {
                    NativeFunc::ShowHelp
                    | NativeFunc::ShowMessage
                    | NativeFunc::SetTime
                    | NativeFunc::GetHour
                    | NativeFunc::GetMinute
                    | NativeFunc::SetWeather
                    | NativeFunc::SpawnRhino
                    | NativeFunc::SpawnHydra
                    | NativeFunc::SpawnHunter
                    | NativeFunc::SpawnMonster
                    | NativeFunc::SpawnHotring
                    | NativeFunc::SpawnStretch
                    | NativeFunc::SpawnCaddy
                    | NativeFunc::Wait => {} // allowed
                }
                for arg in args {
                    self.emit_expr(arg, line)?;
                }
                self.emit(Opcode::CallNative(func.clone(), args.len() as u8));
            }
        }
        Ok(())
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Compile an AST into a Chunk ready for the VM.
pub fn compile(ast: &Ast) -> Result<Chunk, CompileError> {
    let mut c = Compiler::new();
    c.first_pass(ast)?;
    c.second_pass(ast)?;
    Ok(c.chunk)
}

fn native_arity(func: &NativeFunc) -> usize {
    match func {
        NativeFunc::ShowHelp => 1,
        NativeFunc::ShowMessage => 2,
        NativeFunc::SetTime => 2,
        NativeFunc::GetHour | NativeFunc::GetMinute => 0,
        NativeFunc::SetWeather => 1,
        NativeFunc::SpawnRhino
        | NativeFunc::SpawnHydra
        | NativeFunc::SpawnHunter
        | NativeFunc::SpawnMonster
        | NativeFunc::SpawnHotring
        | NativeFunc::SpawnStretch
        | NativeFunc::SpawnCaddy => 0,
        NativeFunc::Wait => 1,
    }
}
