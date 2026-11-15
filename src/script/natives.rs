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
// string encoded in Windows-1252.  For calls that copy the text (SetHelpMessage)
// a temporary Vec<u8> is enough.  Calls that keep the pointer (AddMessageJumpQ)
// must use the persistent arena further down.

fn to_cstring(s: &str, max_len: usize) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(max_len + 1);
    for c in s.chars() {
        if buf.len() >= max_len {
            break;
        }
        buf.push(cp1252_byte(c));
    }
    buf.push(0); // null terminator
    buf
}

/// GTA SA renders text with the Windows-1252 charset, not UTF-8.  Rust strings
/// are UTF-8, so "ñ" (C3 B1) would be drawn as two garbage glyphs.  Translate
/// the characters that matter and degrade anything else to '?'.  ASCII is
/// unchanged, which is the common case.
fn cp1252_byte(c: char) -> u8 {
    match c {
        '\u{0000}'..='\u{007F}' => c as u8,
        '\u{00A0}'..='\u{00FF}' => c as u8, // Latin-1 is identity in CP1252
        '\u{20AC}' => 0x80, // €
        '\u{201A}' => 0x82, // ‚
        '\u{0192}' => 0x83, // ƒ
        '\u{201E}' => 0x84, // „
        '\u{2026}' => 0x85, // …
        '\u{2020}' => 0x86, // †
        '\u{2021}' => 0x87, // ‡
        '\u{02C6}' => 0x88, // ˆ
        '\u{2030}' => 0x89, // ‰
        '\u{0160}' => 0x8A, // Š
        '\u{2039}' => 0x8B, // ‹
        '\u{0152}' => 0x8C, // Œ
        '\u{017D}' => 0x8E, // Ž
        '\u{2018}' => 0x91, // ‘
        '\u{2019}' => 0x92, // ’
        '\u{201C}' => 0x93, // “
        '\u{201D}' => 0x94, // ”
        '\u{2022}' => 0x95, // •
        '\u{2013}' => 0x96, // –
        '\u{2014}' => 0x97, // —
        '\u{02DC}' => 0x98, // ˜
        '\u{2122}' => 0x99, // ™
        '\u{0161}' => 0x9A, // š
        '\u{203A}' => 0x9B, // ›
        '\u{0153}' => 0x9C, // œ
        '\u{017E}' => 0x9E, // ž
        '\u{0178}' => 0x9F, // Ÿ
        _ => b'?',
    }
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
//
// LIFETIME WARNING — verified with Ghidra (GTA SA 1.0 US):
//
//   FUN_0069F1E0 (CMessages::AddMessageJumpQ)
//     FUN_0069db70(&local_320, param_1, 400);  // bounded copy into a LOCAL buffer
//     FUN_0069e160(&local_320);                // processes ~...~ colour codes
//     ...
//     DAT_00C1A7F0 = param_1;                  // stores the CALLER's pointer
//
// The render path reads DAT_00C1A7F0 in later frames (28 xrefs in .text), so
// the text buffer MUST outlive the call.  Passing a temporary Vec<u8> here
// (as v0.1 did) left a dangling pointer and nothing was drawn on screen.
// We therefore keep the text in a static ring of slots.  The game itself only
// copies 400 bytes (including the NUL) in FUN_0069db70, so 400 is the slot
// size.  The message queue holds 8 entries (FUN_0069E360: `sVar4 < 8`) and the
// briefs history holds 20 (`FUN_0069DD50`: `0x13 < uVar3` break); both keep the
// raw pointer, so we keep 32 slots to cover both without overwriting text that
// is still reachable.
//
// Call convention (from the decompiled signature): the game calls it as
// `AddMessageJumpQ(resolved_text, time_ms:u32, flags:u16, add_to_briefs:bool)`
// with (text, ms, 1, 1), e.g. in FUN_005B1720.  Under cdecl every slot is 4
// bytes; the callee only reads the low 16 bits of `flags`.

const MSG_SLOTS:    usize = 32;
const MSG_SLOT_LEN: usize = 400;

static mut MSG_ARENA: [[u8; MSG_SLOT_LEN]; MSG_SLOTS] = [[0u8; MSG_SLOT_LEN]; MSG_SLOTS];
static mut MSG_NEXT:  usize = 0;

/// Copy `text` into the next static slot and return a pointer that stays valid
/// after this call returns (the game keeps it until the message expires).
unsafe fn store_message_text(text: &str) -> *const u8 {
    let slot = MSG_NEXT;
    MSG_NEXT = if slot + 1 == MSG_SLOTS { 0 } else { slot + 1 };

    let base = core::ptr::addr_of_mut!(MSG_ARENA) as *mut u8;
    let dst  = base.add(slot * MSG_SLOT_LEN);

    // Encode as CP1252 (the game's charset).  The slot keeps the NUL terminator
    // because the game walks the string one byte at a time in later frames.
    let mut len = 0;
    for c in text.chars() {
        if len >= MSG_SLOT_LEN - 1 {
            break;
        }
        dst.add(len).write(cp1252_byte(c));
        len += 1;
    }
    dst.add(len).write(0);
    dst as *const u8
}

fn native_show_message(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 2 {
        return Err(VmError { msg: String::from("show_message expects 2 arguments"), line: 0 });
    }
    let text       = arg_str(args, 0)?;
    let time_value = arg_i32(args, 1)?;
    if !(0..=u16::MAX as i32).contains(&time_value) {
        return Err(VmError { msg: String::from("show_message time_ms must be between 0 and 65535"), line: 0 });
    }

    // Safety: store_message_text returns a pointer into a static, writable
    // arena that lives for the whole process.  The game thread is the only
    // writer and the only reader.  AddMessageJumpQ is a __cdecl function at a
    // verified address (docs/evidence/add_message_jumpq.md).
    unsafe {
        let ptr = store_message_text(text);
        let fn_ptr: unsafe extern "C" fn(*const u8, u32, u32, u32) =
            core::mem::transmute(crate::game::ADD_MESSAGE_JUMPQ);
        fn_ptr(ptr, time_value as u32, 1, 1);
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
