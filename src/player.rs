// player.rs — Acceso seguro al jugador (CPed + CPlayerInfo).
//
// CWorld::Players @ 0xB74490 es un array de CPlayerInfo.
// CPlayerInfo[0] contiene en offset 0x0 un puntero a CPed (el ped del jugador).
// Los offsets del ped están en game.rs.

use crate::game::{
    CWORLD_PLAYERS,
    PED_OFFSET_HEALTH, PED_OFFSET_MAX_HEALTH, PED_OFFSET_ARMOR,
    PLAYERINFO_OFFSET_MONEY,
};
use crate::log;

/// Devuelve el puntero al CPed del jugador 0, o null si no está disponible.
///
/// Cadena de punteros confirmada en binario (función 0x439B20):
///   MOV EBP, [0xB74490]       → EBP = *(0xB74490)       = CPlayerInfo base
///   MOV EDI, [EBP+0x00]       → EDI = *(CPlayerInfo+0)  = puntero al CPed
///   MOV ECX, EDI; CALL ...    → usa EDI como this-ptr del CPed
/// Dos dereferencias necesarias.
#[inline]
pub unsafe fn get_ped() -> *mut u8 {
    // 1ª deref: 0xB74490 contiene la dirección base de CPlayerInfo[0]
    let player_info_base = (CWORLD_PLAYERS as *const usize).read();
    if player_info_base == 0 { return core::ptr::null_mut(); }
    // 2ª deref: CPlayerInfo[0]+0x00 contiene el puntero al CPed
    let ped_ptr = (player_info_base as *const usize).read();
    ped_ptr as *mut u8
}

/// Devuelve true si el jugador está disponible (ped != null, salud > 0).
pub unsafe fn is_alive() -> bool {
    let ped = get_ped();
    if ped.is_null() { return false; }
    // Sanity check: la dirección del ped debe estar en el heap del juego
    // (aproximadamente 0x10000000 – 0xE0000000 para Win32)
    let addr = ped as usize;
    if addr < 0x10000 || addr > 0xF0000000 { return false; }
    let health = read_float(ped, PED_OFFSET_HEALTH);
    // Salud válida: positiva y finita (no NaN, no inf, no -0)
    health.is_finite() && health > 0.0
}

#[inline]
unsafe fn read_float(base: *mut u8, offset: usize) -> f32 {
    let ptr = base.add(offset) as *const f32;
    ptr.read()
}

#[inline]
unsafe fn write_float(base: *mut u8, offset: usize, val: f32) {
    let ptr = base.add(offset) as *mut f32;
    ptr.write(val);
}

/// Lee la salud del jugador.
pub unsafe fn get_health() -> f32 {
    let ped = get_ped();
    if ped.is_null() { return 0.0; }
    read_float(ped, PED_OFFSET_HEALTH)
}

/// Lee la salud máxima del jugador.
pub unsafe fn get_max_health() -> f32 {
    let ped = get_ped();
    if ped.is_null() { return 100.0; }
    read_float(ped, PED_OFFSET_MAX_HEALTH)
}

/// Lee la armadura del jugador.
pub unsafe fn get_armor() -> f32 {
    let ped = get_ped();
    if ped.is_null() { return 0.0; }
    read_float(ped, PED_OFFSET_ARMOR)
}

/// Establece la salud del jugador.  No puede superar max_health por seguridad.
pub unsafe fn set_health(val: f32) {
    let ped = get_ped();
    if ped.is_null() { return; }
    let max = read_float(ped, PED_OFFSET_MAX_HEALTH);
    let clamped = val.min(max).max(0.0);
    let old = read_float(ped, PED_OFFSET_HEALTH);
    write_float(ped, PED_OFFSET_HEALTH, clamped);
    log!("RustyMod: health {:.1} → {:.1}", old, clamped);
}

/// Establece la salud al máximo.
pub unsafe fn restore_health() {
    let ped = get_ped();
    if ped.is_null() { return; }
    let max = read_float(ped, PED_OFFSET_MAX_HEALTH);
    let old = read_float(ped, PED_OFFSET_HEALTH);
    write_float(ped, PED_OFFSET_HEALTH, max);
    log!("RustyMod: health {:.1} → {:.1} (max)", old, max);
}

/// Establece la armadura del jugador (0.0 – 100.0).
pub unsafe fn set_armor(val: f32) {
    let ped = get_ped();
    if ped.is_null() { return; }
    let clamped = val.min(100.0).max(0.0);
    let old = read_float(ped, PED_OFFSET_ARMOR);
    write_float(ped, PED_OFFSET_ARMOR, clamped);
    log!("RustyMod: armor {:.1} → {:.1}", old, clamped);
}

/// Establece la armadura al máximo (100).
pub unsafe fn restore_armor() {
    set_armor(100.0);
}

/// Lee el dinero del jugador.
/// Confianza MEDIA — pendiente prueba en juego.
pub unsafe fn get_money() -> i32 {
    let players_ptr = CWORLD_PLAYERS as *const u8;
    // CPlayerInfo struct comienza directamente en CWORLD_PLAYERS
    // (el array tiene CPlayerInfo inline, no punteros a structs separadas).
    // Offset 0xB8 = m_nMoney (i32).
    // ADVERTENCIA: usar solo tras prueba en juego confirmada.
    let money_ptr = players_ptr.add(PLAYERINFO_OFFSET_MONEY) as *const i32;
    money_ptr.read()
}

/// Actualización por frame: detectar jugador disponible y manejar teclas de prueba.
pub unsafe fn update() {
    use crate::controls::just_pressed;
    use crate::hud;
    use crate::config;

    // F6: restaurar salud
    if just_pressed(config::KEY_RESTORE_HEALTH) {
        if is_alive() {
            restore_health();
            // Texto de ayuda con la salud actual
            static MSG_HEALTH: &[u8] = b"Salud restaurada\0";
            hud::show_help(MSG_HEALTH.as_ptr(), true, true);
            log!("RustyMod: F6 -> salud restaurada");
        } else {
            log!("RustyMod: F6 -> sin jugador");
        }
    }

    // F7: restaurar armadura
    if just_pressed(config::KEY_RESTORE_ARMOR) {
        if is_alive() {
            restore_armor();
            static MSG_ARMOR: &[u8] = b"Armadura restaurada\0";
            hud::show_help(MSG_ARMOR.as_ptr(), true, false);
            log!("RustyMod: F7 -> armadura restaurada");
        } else {
            log!("RustyMod: F7 -> sin jugador");
        }
    }

    // F9: log de estado del jugador (diagnóstico)
    if just_pressed(config::KEY_RELOAD_SCRIPTS) {
        if is_alive() {
            let hp  = get_health();
            let mhp = get_max_health();
            let arm = get_armor();
            log!("RustyMod: F9 diagnostico → health={:.1}/{:.1} armor={:.1}", hp, mhp, arm);
        } else {
            log!("RustyMod: F9 -> sin jugador activo");
        }
    }
}
