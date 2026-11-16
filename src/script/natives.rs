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
        // Blips
        NativeFunc::AddBlip        => native_add_blip(args),
        NativeFunc::AddBlipSprite  => native_add_blip_sprite(args),
        NativeFunc::SetBlipColour  => native_set_blip_colour(args),
        NativeFunc::SetBlipScale   => native_set_blip_scale(args),
        NativeFunc::SetBlipSprite  => native_set_blip_sprite(args),
        NativeFunc::SetBlipDisplay => native_set_blip_display(args),
        NativeFunc::RemoveBlip     => native_remove_blip(args),
        // Wanted
        NativeFunc::SetWanted    => native_set_wanted(args),
        NativeFunc::ClearWanted  => native_clear_wanted(),
        NativeFunc::SetMaxWanted => native_set_max_wanted(args),
        // Coronas
        NativeFunc::DrawCorona => native_draw_corona(args),
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

fn arg_f32(args: &[Value], idx: usize) -> Result<f32, VmError> {
    match args.get(idx) {
        Some(Value::Float(v)) => Ok(*v),
        Some(Value::Int(v))   => Ok(*v as f32),
        _ => Err(VmError { msg: String::from("expected float argument"), line: 0 }),
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

// ════════════════════════════════════════════════════════════════════════════
// BLIPS DEL RADAR
// ════════════════════════════════════════════════════════════════════════════
//
// Todas las funciones del radar son __cdecl con el handle (i32) devuelto en EAX.
// El handle == 0xFFFFFFFF (-1 en i32) significa "sin slot libre"; en ese caso
// devolvemos Value::Int(-1) para que el script pueda comprobarlo.
//
// Firmas y layout del array verificados en Ghidra; ver docs/evidence/blips.md.

// add_blip(x: float, y: float, z: float) -> int
//
// Llama a CRadar::SetCoordBlip con tipo 4 (COORD/checkpoint), display 3 (ambos),
// y escala 3 (igual que el opcode SCM 0x018A).  Devuelve el handle del blip.
fn native_add_blip(args: &[Value]) -> Result<Value, VmError> {
    let x = arg_f32(args, 0)?;
    let y = arg_f32(args, 1)?;
    let z = arg_f32(args, 2)?;
    // Safety: SetCoordBlip es una función cdecl a dirección verificada.
    // Es segura en el hilo del juego con argumentos en rango; devuelve el handle.
    let handle = unsafe {
        type Fn = unsafe extern "C" fn(u8, f32, f32, f32, u32, u8, *const u8) -> i32;
        let f: Fn = core::mem::transmute(crate::game::RADAR_SET_COORD_BLIP);
        // type=4 (COORD), ignored=5, display=3, name=null
        let h = f(4, x, y, z, 5, 3, core::ptr::null());
        // Set scale 3 to match the standard SCM behaviour (0x018A).
        if h != -1 {
            type Fn2 = unsafe extern "C" fn(i32, u32);
            let f2: Fn2 = core::mem::transmute(crate::game::RADAR_CHANGE_BLIP_SCALE);
            f2(h, 3);
        }
        h
    };
    crate::log!("add_blip({},{},{}) -> {}", x, y, z, handle);
    Ok(Value::Int(handle))
}

// add_blip_sprite(x: float, y: float, z: float, sprite: int) -> int
//
// Equivalente al opcode SCM 0x02A8: SetCoordBlip(4,...) + SetBlipSprite.
// El sprite debe ser 1..63 (0 = sin icono).
fn native_add_blip_sprite(args: &[Value]) -> Result<Value, VmError> {
    let x      = arg_f32(args, 0)?;
    let y      = arg_f32(args, 1)?;
    let z      = arg_f32(args, 2)?;
    let sprite = arg_i32(args, 3)?.clamp(0, 63) as u8;
    let handle = unsafe {
        type Fn = unsafe extern "C" fn(u8, f32, f32, f32, u32, u8, *const u8) -> i32;
        let f: Fn = core::mem::transmute(crate::game::RADAR_SET_COORD_BLIP);
        let h = f(4, x, y, z, 5, 3, core::ptr::null());
        if h != -1 {
            type Fn2 = unsafe extern "C" fn(i32, u8);
            let f2: Fn2 = core::mem::transmute(crate::game::RADAR_SET_BLIP_SPRITE);
            f2(h, sprite);
        }
        h
    };
    crate::log!("add_blip_sprite({},{},{},s={}) -> {}", x, y, z, sprite, handle);
    Ok(Value::Int(handle))
}

// set_blip_colour(handle: int, colour: int)
//
// eBlipColour: 0 rojo · 1 verde · 2 azul · 3 blanco · 4 amarillo · 7 amenaza · 8 destino
fn native_set_blip_colour(args: &[Value]) -> Result<Value, VmError> {
    let handle  = arg_i32(args, 0)?;
    let colour  = arg_i32(args, 1)? as u32;
    if handle == -1 { return Ok(Value::Nil); }
    // Safety: ChangeBlipColour valida el handle internamente (no crashea con handle inválido).
    unsafe {
        type Fn = unsafe extern "C" fn(i32, u32);
        let f: Fn = core::mem::transmute(crate::game::RADAR_CHANGE_BLIP_COLOUR);
        f(handle, colour);
    }
    Ok(Value::Nil)
}

// set_blip_scale(handle: int, size: int)
fn native_set_blip_scale(args: &[Value]) -> Result<Value, VmError> {
    let handle = arg_i32(args, 0)?;
    let size   = arg_i32(args, 1)? as u32;
    if handle == -1 { return Ok(Value::Nil); }
    unsafe {
        type Fn = unsafe extern "C" fn(i32, u32);
        let f: Fn = core::mem::transmute(crate::game::RADAR_CHANGE_BLIP_SCALE);
        f(handle, size);
    }
    Ok(Value::Nil)
}

// set_blip_sprite(handle: int, sprite: int)
fn native_set_blip_sprite(args: &[Value]) -> Result<Value, VmError> {
    let handle = arg_i32(args, 0)?;
    let sprite = arg_i32(args, 1)?.clamp(0, 63) as u8;
    if handle == -1 { return Ok(Value::Nil); }
    unsafe {
        type Fn = unsafe extern "C" fn(i32, u8);
        let f: Fn = core::mem::transmute(crate::game::RADAR_SET_BLIP_SPRITE);
        f(handle, sprite);
    }
    Ok(Value::Nil)
}

// set_blip_display(handle: int, display: int)
// display: 0 ninguno · 1 solo marcador · 2 solo icono · 3 ambos
fn native_set_blip_display(args: &[Value]) -> Result<Value, VmError> {
    let handle  = arg_i32(args, 0)?;
    let display = arg_i32(args, 1)? as u32;
    if handle == -1 { return Ok(Value::Nil); }
    unsafe {
        type Fn = unsafe extern "C" fn(i32, u32);
        let f: Fn = core::mem::transmute(crate::game::RADAR_CHANGE_BLIP_DISPLAY);
        f(handle, display);
    }
    Ok(Value::Nil)
}

// remove_blip(handle: int)
fn native_remove_blip(args: &[Value]) -> Result<Value, VmError> {
    let handle = arg_i32(args, 0)?;
    if handle == -1 { return Ok(Value::Nil); }
    // Safety: ClearBlip valida el handle antes de escribir; no crashea con handle inválido.
    unsafe {
        type Fn = unsafe extern "C" fn(i32);
        let f: Fn = core::mem::transmute(crate::game::RADAR_CLEAR_BLIP);
        f(handle);
    }
    crate::log!("remove_blip({})", handle);
    Ok(Value::Nil)
}

// ════════════════════════════════════════════════════════════════════════════
// NIVEL DE BÚSQUEDA (WANTED)
// ════════════════════════════════════════════════════════════════════════════
//
// La cadena de punteros verificada es:
//   *(0xB7CD98 + 0x00) = m_pPed (CPed*)
//   *(m_pPed + 0x480)  = m_pPlayerData (CPlayerData*)
//   *(m_pPlayerData)   = m_pWanted (CWanted*)
//
// La escritura SIEMPRE pasa por CWanted::SetWantedLevel (0x562470, __thiscall)
// para que el motor actualice caos, delitos y el HUD correctamente.
// NUNCA escribir +0x2C directamente.
//
// Ver docs/evidence/wanted.md para la tabla completa de caos y trampas.

/// Obtiene un puntero válido a CWanted del jugador 0, o null si no disponible.
/// Safety: solo llamar desde el hilo principal del juego.
unsafe fn get_wanted_ptr() -> *mut u8 {
    let ped = crate::player::get_ped();
    if ped.is_null() { return core::ptr::null_mut(); }
    // m_pPlayerData: [ped + 0x480] = CPlayerData*
    let player_data = (ped.add(crate::game::PED_OFFSET_PLAYER_DATA) as *const usize).read();
    if player_data == 0 { return core::ptr::null_mut(); }
    // m_pWanted: *player_data = CWanted*
    let wanted = (player_data as *const usize).read();
    wanted as *mut u8
}

// set_wanted(level: int)
// Fija el nivel de búsqueda (0..6) pasando por CWanted::SetWantedLevel.
fn native_set_wanted(args: &[Value]) -> Result<Value, VmError> {
    let level = arg_i32(args, 0)?.clamp(0, 6);
    // Safety: get_wanted_ptr() valida la cadena completa; SetWantedLevel es
    // __thiscall con ECX = CWanted* y es seguro con niveles 0..6.
    unsafe {
        let wanted = get_wanted_ptr();
        if wanted.is_null() {
            return Err(VmError { msg: String::from("set_wanted: jugador no disponible"), line: 0 });
        }
        call_thiscall_1arg(crate::game::CWANTED_SET_WANTED_LEVEL, wanted, level);
    }
    crate::log!("set_wanted({})", level);
    Ok(Value::Nil)
}

// clear_wanted()
fn native_clear_wanted() -> Result<Value, VmError> {
    native_set_wanted(&[Value::Int(0)])
}

// set_max_wanted(level: int)
// Fija el techo global de estrellas (0..6).  Llama a SetMaximumWantedLevel (cdecl).
fn native_set_max_wanted(args: &[Value]) -> Result<Value, VmError> {
    let level = arg_i32(args, 0)?.clamp(0, 6);
    // Safety: SetMaximumWantedLevel es __cdecl con 1 arg entero.
    // Valores > 6 se ignoran según el decompilado (jump-table sin rama > 6).
    unsafe {
        type Fn = unsafe extern "C" fn(i32);
        let f: Fn = core::mem::transmute(crate::game::SET_MAX_WANTED_LEVEL);
        f(level);
    }
    crate::log!("set_max_wanted({})", level);
    Ok(Value::Nil)
}

/// Llama a una función __thiscall(this, arg0: i32).
/// En MSVC 32-bit __thiscall pone `this` en ECX y el resto de args en la pila
/// (igual que __cdecl).  Rust no tiene ABI thiscall, así que lo emulamos
/// manualmente: copiamos `this` a ECX, ponemos `arg` en la pila, CALL, restauramos.
#[inline(never)]
unsafe fn call_thiscall_1arg(addr: usize, this: *mut u8, arg: i32) {
    // Safety: addr es una función __thiscall verificada en el ejecutable.
    // `this` es un puntero válido a CWanted (comprobado por get_wanted_ptr).
    // Usamos sub/mov para hacer el push sin la macro push (evita confusión con
    // options(nostack) en Rust inline asm).
    core::arch::asm!(
        "mov ecx, {this}",   // ECX = this  (convención __thiscall)
        "sub esp, 4",        // hueco para el argumento
        "mov [esp], {arg}",  // [esp] = level
        "call {addr}",       // llama a SetWantedLevel(level) con ECX=this
        "add esp, 4",        // restaurar pila (cdecl para los args normales)
        this = in(reg) this,
        arg  = in(reg) arg,
        addr = in(reg) addr,
        out("eax") _,
        out("ecx") _,
        out("edx") _,
        // No ponemos options(nostack) porque SÍ modificamos esp.
    );
}

// ════════════════════════════════════════════════════════════════════════════
// CORONAS 3D
// ════════════════════════════════════════════════════════════════════════════
//
// CCoronas::RegisterCorona (variante "por tipo") = 0x6FC580.
// Firma decompilada (21 argumentos cdecl):
//   RegisterCorona(id, pTex, r, g, b, alpha, &pos, radius, farClip, type,
//                  flare, reflect, checkObstacles, unused, nearClip,
//                  unused2, fadeSpeed, unused3, unused4, unused5, unused6)
// La variante 0x6FC580 recibe tipo (u8) en lugar de pTex y lo convierte:
//   pTex = gpCoronaTexture[tipo]
// Tipos disponibles: 0 DEFAULT · 1 MOON · 2 REFLECTION · 3..N otros
//
// draw_corona se llama CADA FRAME para mantener la corona visible (igual que
// el opcode SCM 0x04D5 "draw corona").  El id se calcula a partir de un
// contador estático incremental; como el script solo crea coronas desde sitios
// de llamada fijos en el código, el id es estable por frame.
//
// Ver docs/evidence/markers.md §4 y §5 para los argumentos completos.

static mut CORONA_NEXT_ID: u32 = 0x1000; // base alejada de ids del SCM

// draw_corona(x: float, y: float, z: float, radius: float, r: int, g: int, b: int)
//
// Dibuja una corona blanca (tipo 0) en la posición dada con color RGB.
// Debe llamarse cada frame para que permanezca visible.
fn native_draw_corona(args: &[Value]) -> Result<Value, VmError> {
    let x      = arg_f32(args, 0)?;
    let y      = arg_f32(args, 1)?;
    let z      = arg_f32(args, 2)?;
    let radius = arg_f32(args, 3)?;
    let r      = arg_i32(args, 4)?.clamp(0, 255) as u8;
    let g      = arg_i32(args, 5)?.clamp(0, 255) as u8;
    let b      = arg_i32(args, 6)?.clamp(0, 255) as u8;

    // Safety: RegisterCorona (overload tipo) es __cdecl con 21 argumentos.
    // &pos es un puntero a un array [f32; 3] en la pila; la función solo lee
    // tres f32 consecutivos (x, y, z) durante la llamada, así que la vida
    // del array en la pila es suficiente.
    // El id estático es único por punto de llamada (el script solo llega aquí
    // desde un único call-site por frame), lo que garantiza que el motor
    // reutilice el slot de corona en lugar de crear uno nuevo cada frame.
    unsafe {
        let id = CORONA_NEXT_ID;
        // No incrementar: queremos el mismo id cada frame para reutilizar el slot.
        // Si el script necesita varias coronas simultáneas, debe hacerlo en frames
        // separados (el SCM tampoco tiene otro mecanismo).

        let pos: [f32; 3] = [x, y, z];

        // Firma efectiva de la variante "tipo":
        //   void RegisterCorona(u32 id, void* unused0, u8 r, u8 g, u8 b, u8 alpha,
        //                       const float* pos, f32 radius, f32 farClip, u8 tipo,
        //                       u8 flare, u8 reflect, u8 checkObs, u8 unused1,
        //                       f32 nearClip, u8 unused2, f32 fadeSpeed,
        //                       u8 unused3, f32 unused4, u8 unused5, u8 unused6)
        // Los tipos de args son los que el motor empuja a la pila; los `u8` se
        // expanden a u32 en cdecl (no se usa struct packing).
        type Fn = unsafe extern "C" fn(
            u32,          // id
            *const u8,    // pTex (ignorado en la variante tipo; motor lo reemplaza)
            u32, u32, u32, u32,   // r, g, b, alpha
            *const f32,   // &pos
            f32,          // radius
            f32,          // farClip
            u32,          // tipo corona (0 = DEFAULT)
            u32,          // flare (0 = ninguno)
            u32,          // reflejar en suelo (0)
            u32,          // comprobar obstáculos (0)
            u32,          // unused
            f32,          // nearClip
            u32,          // unused
            f32,          // fadeSpeed
            u32,          // unused
            f32,          // unused
            u32,          // unused
            u32,          // unused
        );
        let f: Fn = core::mem::transmute(crate::game::CORONAS_REGISTER_CORONA_TYPE);
        f(
            id,
            core::ptr::null(),              // pTex: ignorado (overload tipo)
            r as u32, g as u32, b as u32,
            0xFF,                           // alpha = opaco
            pos.as_ptr(),
            radius,
            450.0_f32,                      // farClip: distancia máxima de visión
            0_u32,                          // tipo 0 = DEFAULT
            0_u32,                          // sin flare
            0_u32, 0_u32, 0_u32,
            0.2_f32,                        // nearClip
            0_u32,
            1.5_f32,                        // fadeSpeed
            0_u32,
            15.0_f32,                       // unused fadeout range
            0_u32, 0_u32,
        );
    }
    Ok(Value::Nil)
}
