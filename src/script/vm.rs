// src/script/vm.rs — Stack-based interpreter for compiled RustyScript Chunks.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use super::compiler::{Chunk, NativeField, Opcode, Value};
// NativeFunc appears only inside Opcode::CallNative pattern arms which Rust resolves
// through the Opcode type; no direct name use needed here.
#[allow(unused_imports)]
use super::compiler::NativeFunc;
use super::natives::call_native;

// ── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct VmError {
    pub msg:  String,
    pub line: u32,
}

/// Result of a VM run: either the handler finished, or it paused on a `wait()`.
#[derive(Debug, Clone, PartialEq)]
pub enum RunResult {
    Complete,
    Paused,
    BudgetExceeded,
}

impl VmError {
    fn new(msg: &str) -> Self {
        VmError { msg: String::from(msg), line: 0 }
    }
    fn fmt(msg: String) -> Self {
        VmError { msg, line: 0 }
    }
}

// ── Call frame ───────────────────────────────────────────────────────────────

struct Frame {
    return_ip:   usize,
    locals_base: usize,  // index into the unified locals vec at frame entry
}

// ── VM ────────────────────────────────────────────────────────────────────────

pub struct Vm {
    stack:  Vec<Value>,  // operand stack
    locals: Vec<Value>,  // flat locals array; each frame has a window into it
    frames: Vec<Frame>,  // call stack
    ip:     usize,
    /// When Some(target_ms), the VM is paused in a `wait()` and should not be
    /// re-entered until GetTickCount() >= target_ms.  Resumed by the caller
    /// passing `resume=true` to `run()`.
    pub wait_until_ms: Option<u64>,
}

const MAX_STACK:  usize = 256;
const MAX_LOCALS: usize = 64;
const MAX_FRAMES: usize = 32;

impl Vm {
    pub fn new() -> Self {
        Vm {
            stack:        Vec::new(),
            locals:       Vec::new(),
            frames:       Vec::new(),
            ip:           0,
            wait_until_ms: None,
        }
    }

    // ── Public entry point ───────────────────────────────────────────────────

    /// Execute the chunk starting at `entry_ip` until Halt, Return at depth 0,
    /// or error.  `globals` is shared with the owning ScriptState.
    ///
    /// If `resume` is true, the VM continues from `self.ip` without clearing
    /// any state — used to resume after a `wait()` pause.
    /// If `resume` is false, the VM resets its stack/locals/frames and starts
    /// fresh at `entry_ip`.
    pub fn run(
        &mut self,
        chunk:    &Chunk,
        globals:  &mut Vec<Value>,
        entry_ip: usize,
        resume:   bool,
        budget_us: Option<u64>,
    ) -> Result<RunResult, VmError> {
        let deadline = budget_us.map(|budget| {
            let mut now = 0i64;
            let mut freq = 0i64;
            unsafe {
                QueryPerformanceCounter(&mut now);
                QueryPerformanceFrequency(&mut freq);
            }
            if freq <= 0 {
                now
            } else {
                now.saturating_add(
                    (budget as i64).saturating_mul(freq) / 1_000_000,
                )
            }
        });
        let mut steps = 0u32;

        if !resume {
            self.ip     = entry_ip;
            self.stack.clear();
            self.locals.clear();
            self.frames.clear();
            // Push an implicit root frame so Return from depth 0 halts cleanly.
            self.frames.push(Frame { return_ip: usize::MAX, locals_base: 0 });
        }

        loop {
            steps = steps.wrapping_add(1);
            if steps & 31 == 0 {
                if let Some(limit) = deadline {
                    let mut now = 0i64;
                    unsafe { QueryPerformanceCounter(&mut now); }
                    if now >= limit {
                        return Ok(RunResult::BudgetExceeded);
                    }
                }
            }

            if self.ip >= chunk.code.len() {
                return Err(VmError::new("ip out of bounds"));
            }

            // Clone opcode so we can mutate `self` freely inside the match.
            let op = chunk.code[self.ip].clone();
            self.ip += 1;

            match op {
                // ── Stack ────────────────────────────────────────────────────
                Opcode::Push(idx) => {
                    if idx >= chunk.constants.len() {
                        return Err(VmError::new("constant index out of bounds"));
                    }
                    self.push(chunk.constants[idx].clone())?;
                }
                Opcode::Pop => { self.pop()?; }
                Opcode::Dup => {
                    let v = self.top()?.clone();
                    self.push(v)?;
                }

                // ── Locals ───────────────────────────────────────────────────
                Opcode::LoadLocal(slot) => {
                    let base = self.frame_locals_base();
                    let idx  = base + slot as usize;
                    let v = self.locals.get(idx)
                        .ok_or_else(|| VmError::new("local slot out of range"))?
                        .clone();
                    self.push(v)?;
                }
                Opcode::StoreLocal(slot) => {
                    let v    = self.pop()?;
                    let base = self.frame_locals_base();
                    let idx  = base + slot as usize;
                    // Extend if needed (first store to a new slot).
                    while self.locals.len() <= idx {
                        self.locals.push(Value::Nil);
                    }
                    if self.locals.len() > MAX_LOCALS {
                        return Err(VmError::new("too many locals"));
                    }
                    self.locals[idx] = v;
                    // StoreLocal does NOT pop from operand stack — the caller emits Pop.
                    self.push(v_nil())?;
                }

                // ── Globals ──────────────────────────────────────────────────
                Opcode::LoadGlobal(idx) => {
                    let v = globals.get(idx as usize)
                        .ok_or_else(|| VmError::new("global index out of range"))?
                        .clone();
                    self.push(v)?;
                }
                Opcode::StoreGlobal(idx) => {
                    let v = self.pop()?;
                    while globals.len() <= idx as usize {
                        globals.push(Value::Nil);
                    }
                    globals[idx as usize] = v.clone();
                    self.push(v)?;
                }

                // ── Native fields ────────────────────────────────────────────
                Opcode::LoadField(f) => {
                    let v = load_field(&f)?;
                    self.push(v)?;
                }
                Opcode::StoreField(f) => {
                    let v = self.pop()?;
                    store_field(&f, v.clone())?;
                    self.push(v)?;
                }

                // ── Arithmetic ───────────────────────────────────────────────
                Opcode::Add => {
                    let (a, b) = self.pop2()?;
                    self.push(arith_add(a, b)?)?;
                }
                Opcode::Sub => {
                    let (a, b) = self.pop2()?;
                    self.push(arith_sub(a, b)?)?;
                }
                Opcode::Mul => {
                    let (a, b) = self.pop2()?;
                    self.push(arith_mul(a, b)?)?;
                }
                Opcode::Div => {
                    let (a, b) = self.pop2()?;
                    self.push(arith_div(a, b)?)?;
                }
                Opcode::Mod => {
                    let (a, b) = self.pop2()?;
                    self.push(arith_mod(a, b)?)?;
                }
                Opcode::Neg => {
                    let v = self.pop()?;
                    self.push(arith_neg(v)?)?;
                }

                // ── Comparison ───────────────────────────────────────────────
                Opcode::Eq => { let (a,b)=self.pop2()?; self.push(Value::Bool(values_eq(&a,&b)))?; }
                Opcode::Ne => { let (a,b)=self.pop2()?; self.push(Value::Bool(!values_eq(&a,&b)))?; }
                Opcode::Lt => { let (a,b)=self.pop2()?; self.push(Value::Bool(values_lt(&a,&b)?))?; }
                Opcode::Gt => { let (a,b)=self.pop2()?; self.push(Value::Bool(values_lt(&b,&a)?))?; }
                Opcode::Le => { let (a,b)=self.pop2()?; self.push(Value::Bool(!values_lt(&b,&a)?))?; }
                Opcode::Ge => { let (a,b)=self.pop2()?; self.push(Value::Bool(!values_lt(&a,&b)?))?; }

                // ── Logic ────────────────────────────────────────────────────
                Opcode::And => {
                    let (a, b) = self.pop2()?;
                    self.push(Value::Bool(is_truthy(&a) && is_truthy(&b)))?;
                }
                Opcode::Or => {
                    let (a, b) = self.pop2()?;
                    self.push(Value::Bool(is_truthy(&a) || is_truthy(&b)))?;
                }
                Opcode::Not => {
                    let v = self.pop()?;
                    self.push(Value::Bool(!is_truthy(&v)))?;
                }

                // ── Control flow ─────────────────────────────────────────────
                Opcode::Jump(target) => {
                    self.ip = target;
                }
                Opcode::JumpIfFalse(target) => {
                    let v = self.pop()?;
                    if !is_truthy(&v) {
                        self.ip = target;
                    }
                }

                // ── Calls ────────────────────────────────────────────────────
                Opcode::CallNative(func, argc) => {
                    crate::log!("VM: CallNative({:?}, argc={})", func, argc);
                    // Pop argc args in reverse order so args[0] is first argument.
                    let mut args: Vec<Value> = Vec::new();
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    crate::log!("VM: calling native {:?} with args: {:?}", func, args);
                    let result = call_native(&func, &args)?;
                    crate::log!("VM: native returned {:?}", result);
                    self.push(result)?;
                }

                Opcode::CallUser(fn_ip, arity) => {
                    if self.frames.len() >= MAX_FRAMES {
                        return Err(VmError::new("call stack overflow"));
                    }
                    // Args are already on the stack; set up a new frame.
                    let new_base = self.locals.len();
                    // Move the top `arity` stack values into locals.
                    let mut arg_vals: Vec<Value> = Vec::new();
                    for _ in 0..arity {
                        arg_vals.push(self.pop()?);
                    }
                    arg_vals.reverse();
                    // Extend locals to accommodate the new frame's slots.
                    for v in arg_vals {
                        if self.locals.len() >= MAX_LOCALS {
                            return Err(VmError::new("too many locals"));
                        }
                        self.locals.push(v);
                    }
                    self.frames.push(Frame { return_ip: self.ip, locals_base: new_base });
                    self.ip = fn_ip;
                }

                Opcode::Return => {
                    let frame = self.frames.pop()
                        .ok_or_else(|| VmError::new("return with empty call stack"))?;
                    let result = self.stack.pop().unwrap_or(Value::Nil);
                    if frame.return_ip == usize::MAX {
                        // Returning from the root frame — stop execution.
                        return Ok(RunResult::Complete);
                    }
                    // Trim locals back to parent frame.
                    self.locals.truncate(frame.locals_base);
                    self.ip = frame.return_ip;
                    self.push(result)?;
                }

                Opcode::Halt => {
                    return Ok(RunResult::Complete);
                }

                // ── Wait (yield) ─────────────────────────────────────────────
                Opcode::Wait => {
                    // Pop the wait duration in milliseconds (int).
                    let v = self.pop()?;
                    let ms = match v {
                        Value::Int(n)  => n.max(0) as u64,
                        Value::Float(f) => (f.max(0.0) as u64),
                        _ => return Err(VmError::new("wait() expects a number (ms)")),
                    };
                    // Safety: GetTickCount is always safe to call.
                    let now = unsafe { now_ms() };
                    self.wait_until_ms = Some(now + ms);
                    // Push Nil so the compiler's trailing Pop has something to consume.
                    self.push(Value::Nil)?;
                    // Do NOT advance past this opcode on resume — we already
                    // incremented ip at the top of the loop, so the next
                    // instruction is correct.
                    return Ok(RunResult::Paused);
                }

                // Break and Continue are patched to Jump by the compiler;
                // if they ever reach the VM it means the compiler had a bug.
                Opcode::Break | Opcode::Continue => {
                    return Err(VmError::new("unpatched break/continue reached VM"));
                }
            }
        }
    }

    // ── Stack helpers ────────────────────────────────────────────────────────

    fn push(&mut self, v: Value) -> Result<(), VmError> {
        if self.stack.len() >= MAX_STACK {
            return Err(VmError::new("stack overflow"));
        }
        self.stack.push(v);
        Ok(())
    }

    fn pop(&mut self) -> Result<Value, VmError> {
        self.stack.pop().ok_or_else(|| VmError::new("stack underflow"))
    }

    fn pop2(&mut self) -> Result<(Value, Value), VmError> {
        let b = self.pop()?;
        let a = self.pop()?;
        Ok((a, b))
    }

    fn top(&self) -> Result<&Value, VmError> {
        self.stack.last().ok_or_else(|| VmError::new("stack underflow"))
    }

    fn frame_locals_base(&self) -> usize {
        self.frames.last().map(|f| f.locals_base).unwrap_or(0)
    }
}

// ── Helper: Nil value shorthand ──────────────────────────────────────────────

#[inline]
fn v_nil() -> Value { Value::Nil }

// ── Arithmetic helpers ───────────────────────────────────────────────────────

fn coerce_numeric(a: Value, b: Value) -> Result<(bool, f32, f32), VmError> {
    // Returns (is_int_op, fa, fb). is_int_op is true only if both are Int.
    match (&a, &b) {
        (Value::Int(x),   Value::Int(y))   => Ok((true,  *x as f32, *y as f32)),
        (Value::Int(x),   Value::Float(y)) => Ok((false, *x as f32, *y)),
        (Value::Float(x), Value::Int(y))   => Ok((false, *x,        *y as f32)),
        (Value::Float(x), Value::Float(y)) => Ok((false, *x,        *y)),
        _ => Err(VmError::fmt(format!(
            "error[E03]: type error in arithmetic: {:?} and {:?}", a, b
        ))),
    }
}

fn arith_add(a: Value, b: Value) -> Result<Value, VmError> {
    // String concatenation
    if let (Value::Str(ref sa), Value::Str(ref sb)) = (&a, &b) {
        let mut s = sa.clone();
        s.push_str(sb);
        return Ok(Value::Str(s));
    }
    let (int_op, fa, fb) = coerce_numeric(a, b)?;
    if int_op { Ok(Value::Int(fa as i32 + fb as i32)) } else { Ok(Value::Float(fa + fb)) }
}

fn arith_sub(a: Value, b: Value) -> Result<Value, VmError> {
    let (int_op, fa, fb) = coerce_numeric(a, b)?;
    if int_op { Ok(Value::Int(fa as i32 - fb as i32)) } else { Ok(Value::Float(fa - fb)) }
}

fn arith_mul(a: Value, b: Value) -> Result<Value, VmError> {
    let (int_op, fa, fb) = coerce_numeric(a, b)?;
    if int_op { Ok(Value::Int(fa as i32 * fb as i32)) } else { Ok(Value::Float(fa * fb)) }
}

fn arith_div(a: Value, b: Value) -> Result<Value, VmError> {
    let (int_op, fa, fb) = coerce_numeric(a, b)?;
    if int_op {
        let ib = fb as i32;
        if ib == 0 { return Err(VmError::new("error[E11]: division by zero")); }
        Ok(Value::Int(fa as i32 / ib))
    } else {
        if fb == 0.0 { return Err(VmError::new("error[E11]: division by zero")); }
        Ok(Value::Float(fa / fb))
    }
}

fn arith_mod(a: Value, b: Value) -> Result<Value, VmError> {
    let (int_op, fa, fb) = coerce_numeric(a, b)?;
    if int_op {
        let ib = fb as i32;
        if ib == 0 { return Err(VmError::new("error[E11]: modulo by zero")); }
        Ok(Value::Int(fa as i32 % ib))
    } else {
        if fb == 0.0 { return Err(VmError::new("error[E11]: modulo by zero")); }
        Ok(Value::Float(fa % fb))
    }
}

fn arith_neg(v: Value) -> Result<Value, VmError> {
    match v {
        Value::Int(x)   => Ok(Value::Int(-x)),
        Value::Float(x) => Ok(Value::Float(-x)),
        _ => Err(VmError::new("type error: unary minus on non-numeric value")),
    }
}

// ── Comparison helpers ───────────────────────────────────────────────────────

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x),   Value::Int(y))   => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x),   Value::Float(y)) => (*x as f32) == *y,
        (Value::Float(x), Value::Int(y))   => *x == (*y as f32),
        (Value::Bool(x),  Value::Bool(y))  => x == y,
        (Value::Str(x),   Value::Str(y))   => x == y,
        (Value::Nil,       Value::Nil)      => true,
        _                                  => false,
    }
}

fn values_lt(a: &Value, b: &Value) -> Result<bool, VmError> {
    match (a, b) {
        (Value::Int(x),   Value::Int(y))   => Ok(x < y),
        (Value::Float(x), Value::Float(y)) => Ok(x < y),
        (Value::Int(x),   Value::Float(y)) => Ok((*x as f32) < *y),
        (Value::Float(x), Value::Int(y))   => Ok(*x < (*y as f32)),
        _ => Err(VmError::new("type error: comparison of incompatible types")),
    }
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b)  => *b,
        Value::Int(i)   => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::Str(s)   => !s.is_empty(),
        Value::Nil      => false,
    }
}

// ── Native field read/write ──────────────────────────────────────────────────
// These call into player.rs / game.rs, all of which are unsafe by nature.

fn load_field(f: &NativeField) -> Result<Value, VmError> {
    // Safety: we are running inside the game thread; the player module validates
    // the ped pointer before dereferencing it.
    unsafe {
        match f {
            NativeField::Health      => Ok(Value::Float(crate::player::get_health())),
            NativeField::MaxHealth   => Ok(Value::Float(crate::player::get_max_health())),
            NativeField::Armor       => Ok(Value::Float(crate::player::get_armor())),
            NativeField::X => {
                let ped = crate::player::get_ped();
                if ped.is_null() { return Err(VmError::new("error[E13]: player not available")); }
                let v = (ped.add(crate::game::PED_OFFSET_X) as *const f32).read();
                Ok(Value::Float(v))
            }
            NativeField::Y => {
                let ped = crate::player::get_ped();
                if ped.is_null() { return Err(VmError::new("error[E13]: player not available")); }
                let v = (ped.add(crate::game::PED_OFFSET_Y) as *const f32).read();
                Ok(Value::Float(v))
            }
            NativeField::Z => {
                let ped = crate::player::get_ped();
                if ped.is_null() { return Err(VmError::new("error[E13]: player not available")); }
                let v = (ped.add(crate::game::PED_OFFSET_Z) as *const f32).read();
                Ok(Value::Float(v))
            }
            NativeField::Money => {
                // MEDIA confidence — returns 0 until verified in-game
                Ok(Value::Int(0))
            }
            NativeField::WantedLevel => {
                // Lee m_nWantedLevel (+0x2C) desde la cadena Players[0]->m_pPed->m_pPlayerData->m_pWanted.
                // Safety: get_ped() valida el puntero; los offsets están verificados en docs/evidence/wanted.md.
                let ped = crate::player::get_ped();
                if ped.is_null() { return Ok(Value::Int(0)); }
                let player_data_ptr = (ped.add(crate::game::PED_OFFSET_PLAYER_DATA) as *const usize).read();
                if player_data_ptr == 0 { return Ok(Value::Int(0)); }
                let wanted_ptr = (player_data_ptr as *const usize).read();
                if wanted_ptr == 0 { return Ok(Value::Int(0)); }
                let level = ((wanted_ptr + crate::game::CWANTED_OFFSET_WANTED_LEVEL) as *const u32).read();
                Ok(Value::Int(level.min(6) as i32))
            }
            NativeField::MaxWanted => {
                // Lee el nivel máximo global en 0x8CDEE4.
                let level = (0x8CDEE4_usize as *const u32).read();
                Ok(Value::Int(level.min(6) as i32))
            }
        }
    }
}

fn store_field(f: &NativeField, v: Value) -> Result<(), VmError> {
    // Safety: we are on the game thread; player module clamps and validates values.
    unsafe {
        match f {
            NativeField::Health => {
                let fv = coerce_to_f32(&v)?;
                crate::player::set_health(fv);
            }
            NativeField::Armor => {
                let fv = coerce_to_f32(&v)?;
                crate::player::set_armor(fv);
            }
            NativeField::MaxHealth => {
                // read-only field — silently ignore
            }
            NativeField::X | NativeField::Y | NativeField::Z => {
                // position is read-only in v0.1
            }
            NativeField::Money => {
                // MEDIA confidence — ignore until verified
            }
            NativeField::WantedLevel => {
                // Escribe vía CWanted::SetWantedLevel para mantener caos y HUD sincronizados.
                // Mismo código que native_set_wanted para evitar duplicar la cadena.
                let level = match &v {
                    Value::Int(i)   => (*i).clamp(0, 6),
                    Value::Float(f) => (*f as i32).clamp(0, 6),
                    _ => return Err(VmError::new("wanted_level: expected int")),
                };
                let _ = crate::script::natives::call_native(
                    &crate::script::compiler::NativeFunc::SetWanted,
                    &[Value::Int(level)],
                );
            }
            NativeField::MaxWanted => {
                let level = match &v {
                    Value::Int(i)   => (*i).clamp(0, 6),
                    Value::Float(f) => (*f as i32).clamp(0, 6),
                    _ => return Err(VmError::new("max_wanted: expected int")),
                };
                let _ = crate::script::natives::call_native(
                    &crate::script::compiler::NativeFunc::SetMaxWanted,
                    &[Value::Int(level)],
                );
            }
        }
    }
    Ok(())
}

fn coerce_to_f32(v: &Value) -> Result<f32, VmError> {
    match v {
        Value::Float(f) => Ok(*f),
        Value::Int(i)   => Ok(*i as f32),
        _ => Err(VmError::new("expected numeric value")),
    }
}

// ── Time source for wait() ───────────────────────────────────────────────────
// GetTickCount returns ms since system boot; wraps every ~49 days.
// For wait() durations up to ~4 billion ms (~49 days) this is fine.

extern "system" {
    fn QueryPerformanceCounter(out: *mut i64) -> i32;
    fn QueryPerformanceFrequency(out: *mut i64) -> i32;
    fn GetTickCount() -> u32;
}

/// Current time in milliseconds (monotonic, wraps at ~49 days).
///
/// # Safety
/// GetTickCount is thread-safe and has no preconditions.
#[inline]
pub unsafe fn now_ms() -> u64 {
    GetTickCount() as u64
}
