// hud.rs — Muestra mensajes en pantalla usando funciones del juego.
//
// AddMessageJumpQ @ 0x69F1E0: __cdecl (char* text, u16 time_ms, i16 flag, i32 unk)
// SetHelpMessage  @ 0x588BE0: __cdecl (char* text, bool quick, bool permanent, bool beep)
//
// IMPORTANTE: GTA SA usa GXT keys O texto directo con el prefijo correcto.
// AddMessageJumpQ acepta texto GXT key; para texto libre se usa el mecanismo
// de texto de ayuda (SetHelpMessage) que sí acepta cadenas arbitrarias.

use crate::game::{ADD_MESSAGE_JUMPQ, SET_HELP_MESSAGE};

/// Muestra un mensaje en la esquina inferior izquierda durante `time_ms` ms.
/// `text` debe ser una GXT key terminada en nulo (máx 7 chars + \0).
/// Usar con cadenas estáticas en el binario.
pub unsafe fn show_message(text: *const u8, time_ms: u16) {
    let fn_ptr: unsafe extern "C" fn(*const u8, u16, i16, i32) =
        core::mem::transmute(ADD_MESSAGE_JUMPQ);
    fn_ptr(text, time_ms, 1, 0);
}

/// Muestra texto de ayuda (caja amarilla) en pantalla.
/// `text` puede ser una cadena arbitraria terminada en nulo.
/// `beep` = true reproduce el sonido de ayuda.
pub unsafe fn show_help(text: *const u8, quick: bool, beep: bool) {
    let fn_ptr: unsafe extern "C" fn(*const u8, u8, u8, u8) =
        core::mem::transmute(SET_HELP_MESSAGE);
    fn_ptr(text, quick as u8, 0u8, beep as u8);
}
