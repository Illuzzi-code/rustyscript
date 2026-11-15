// src/script/natives.rs — Implementation of every NativeFunc variant.
// Calls game functions via raw function pointers using verified addresses
// from crate::game.  All unsafe operations carry an inline safety comment.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use super::compiler::{NativeFunc, Value};
use super::vm::VmError;

// ── Public dispatch ──────────────────────────────────────────────────────────

pub fn call_native(func: &NativeFunc, args: &[Value]) -> Result<Value, VmError> {
    match func {
        NativeFunc::ShowHelp     => native_show_help(args),
        NativeFunc::ShowMessage  => native_show_message(args),
        NativeFunc::SetTime      => native_set_time(args),
        NativeFunc::GetHour      => native_get_hour(),
        NativeFunc::GetMinute    => native_get_minute(),
        NativeFunc::SetWeather   => native_set_weather(args),
        NativeFunc::SpawnRhino   => native_spawn(crate::game::CHEAT_SPAWN_RHINO),
        NativeFunc::SpawnHydra   => native_spawn(crate::game::CHEAT_SPAWN_HYDRA),
        NativeFunc::SpawnHunter  => native_spawn(crate::game::CHEAT_SPAWN_HUNTER),
        NativeFunc::SpawnMonster => native_spawn(crate::game::CHEAT_SPAWN_MONSTER),
        NativeFunc::SpawnHotring => native_spawn(crate::game::CHEAT_SPAWN_HOTRING),
        NativeFunc::SpawnStretch => native_spawn(crate::game::CHEAT_SPAWN_STRETCH),
        NativeFunc::SpawnCaddy   => native_spawn(crate::game::CHEAT_SPAWN_CADDY),
        // Wait is handled directly by the VM's Opcode::Wait, never reaches here.
        NativeFunc::Wait => Err(VmError {
            msg: String::from("wait() must be compiled to Opcode::Wait"),
            line: 0,
        }),
    }
}

// ── Argument coercions ───────────────────────────────────────────────────────

fn arg_str(args: &[Value], idx: usize) -> Result<&str, VmError> {
    match args.get(idx) {
        Some(Value::Str(s)) => Ok(s.as_str()),
        _ => Err(VmError { msg: String::from("expected string argument"), line: 0 }),
    }
}

fn arg_i32(args: &[Value], idx: usize) -> Result<i32, VmError> {
    match args.get(idx) {
        Some(Value::Int(v))   => Ok(*v),
        Some(Value::Float(v)) => Ok(*v as i32),
        _ => Err(VmError { msg: String::from("expected integer argument"), line: 0 }),
    }
}

// ── String → null-terminated byte buffer ────────────────────────────────────
// GTA SA native functions expect a *const u8 pointing to a null-terminated C
// string.  We build a temporary Vec<u8> and pass a pointer to it.  The Vec
// lives for the duration of the call.

fn to_cstring(s: &str, max_len: usize) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    for b in s.bytes().take(max_len) {
        buf.push(b);
    }
    buf.push(0); // null terminator
    buf
}

// ── show_help(text: string) ──────────────────────────────────────────────────

fn native_show_help(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError { msg: String::from("show_help expects 1 argument"), line: 0 });
    }
    let text = arg_str(args, 0)?;
    let buf  = to_cstring(text, 127);

    crate::log!("show_help: text='{}', calling SetHelpMessage @ 0x{:08X}", text, crate::game::SET_HELP_MESSAGE);
    // Safety: buf is valid for the duration of this call; SetHelpMessage is a
    // cdecl function at a verified address; we are on the game thread.
    unsafe {
        let fn_ptr: unsafe extern "C" fn(*const u8, u8, u8, u8) =
            core::mem::transmute(crate::game::SET_HELP_MESSAGE);
        fn_ptr(buf.as_ptr(), 1u8, 0u8, 0u8);
    }
    crate::log!("show_help: returned from SetHelpMessage");
    Ok(Value::Nil)
}

// ── show_message(text: string, time_ms: int) ─────────────────────────────────

fn native_show_message(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 2 {
        return Err(VmError { msg: String::from("show_message expects 2 arguments"), line: 0 });
    }
    let text    = arg_str(args, 0)?;
    let time_value = arg_i32(args, 1)?;
    if !(0..=u16::MAX as i32).contains(&time_value) {
        return Err(VmError { msg: String::from("show_message time_ms must be between 0 and 65535"), line: 0 });
    }
    let time_ms = time_value as u16;
    let buf     = to_cstring(text, 127);

    // Safety: buf is valid for the duration of this call; AddMessageJumpQ is a
    // cdecl function at a verified address; we are on the game thread.
    unsafe {
        let fn_ptr: unsafe extern "C" fn(*const u8, u16, i16, i32) =
            core::mem::transmute(crate::game::ADD_MESSAGE_JUMPQ);
        fn_ptr(buf.as_ptr(), time_ms, 1i16, 0i32);
    }
    Ok(Value::Nil)
}

// ── set_time(hour: int, minute: int) ─────────────────────────────────────────

fn native_set_time(args: &[Value]) -> Result<Value, VmError> {
    let hour   = arg_i32(args, 0)?.clamp(0, 23) as u8;
    let minute = arg_i32(args, 1)?.clamp(0, 59) as u8;

    // Safety: CLOCK_HOURS and CLOCK_MINUTES are verified static global addresses
    // in the GTA SA binary with no ASLR; writing a u8 is atomic on x86.
    unsafe {
        (crate::game::CLOCK_HOURS   as *mut u8).write(hour);
        (crate::game::CLOCK_MINUTES as *mut u8).write(minute);
    }
    crate::log!("RustyScript: set_time({}, {})", hour, minute);
    Ok(Value::Nil)
}

// ── get_hour() -> int ────────────────────────────────────────────────────────

fn native_get_hour() -> Result<Value, VmError> {
    // Safety: CLOCK_HOURS is a verified static address; read-only u8 access.
    let h = unsafe { (crate::game::CLOCK_HOURS as *const u8).read() };
    Ok(Value::Int(h as i32))
}

// ── get_minute() -> int ──────────────────────────────────────────────────────

fn native_get_minute() -> Result<Value, VmError> {
    // Safety: CLOCK_MINUTES is a verified static address; read-only u8 access.
    let m = unsafe { (crate::game::CLOCK_MINUTES as *const u8).read() };
    Ok(Value::Int(m as i32))
}

// ── set_weather(id: int) ─────────────────────────────────────────────────────

fn native_set_weather(args: &[Value]) -> Result<Value, VmError> {
    let id = arg_i32(args, 0)? as u16;

    // Safety: WEATHER_OLD / WEATHER_NEW are verified static addresses; u16 write.
    unsafe {
        (crate::game::WEATHER_OLD as *mut u16).write(id);
        (crate::game::WEATHER_NEW as *mut u16).write(id);
    }
    crate::log!("RustyScript: set_weather({})", id);
    Ok(Value::Nil)
}

// ── spawn_* helpers ──────────────────────────────────────────────────────────

fn native_spawn(stub_addr: usize) -> Result<Value, VmError> {
    // Require a live player before spawning to avoid crashing on the main menu.
    // Safety: get_ped() reads through verified pointer chains; null check guards
    // the dereference.
    let ped = unsafe { crate::player::get_ped() };
    if ped.is_null() {
        return Err(VmError { msg: String::from("error[E13]: player not available"), line: 0 });
    }

    // Safety: stub_addr is a verified ALTA-confidence cheat stub at a fixed
    // address in the GTA SA binary (no ASLR).  The stub is a 12-byte cdecl
    // leaf function that takes no arguments and returns void.
    unsafe {
        let fn_ptr: unsafe extern "C" fn() = core::mem::transmute(stub_addr);
        fn_ptr();
    }
    crate::log!("RustyScript: spawn @ 0x{:08X}", stub_addr);
    Ok(Value::Nil)
}
