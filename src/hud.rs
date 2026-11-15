// hud.rs — Muestra mensajes en pantalla usando funciones del juego.
//
// AddMessageJumpQ @ 0x69F1E0: __cdecl (char* text, u32 time_ms, u32 flags, u32 add_to_briefs)
// SetHelpMessage  @ 0x588BE0: __cdecl (char* text, bool quick, bool permanent, bool beep)
//
// IMPORTANTE: AddMessageJumpQ NO copia el texto: guarda el puntero del llamante
// en DAT_00C1A7F0 y el render lo lee en frames posteriores.  El buffer de `text`
// debe seguir vivo hasta que el mensaje expire (ver src/script/natives.rs).
// SetHelpMessage sí copia el texto, por eso `show_help` no tiene ese problema.

use crate::game::{ADD_MESSAGE_JUMPQ, SET_HELP_MESSAGE};

/// Muestra un mensaje en la esquina inferior izquierda durante `time_ms` ms.
///
/// `text` debe ser una cadena UTF-8/ASCII terminada en nulo y **seguir viva**
/// mientras el mensaje esté en pantalla (AddMessageJumpQ solo guarda el puntero).
/// Para texto dinámico usar `script::natives::show_message`, que copia el texto a
/// un ring estático persistente.
pub unsafe fn show_message(text: *const u8, time_ms: u32) {
    let fn_ptr: unsafe extern "C" fn(*const u8, u32, u32, u32) =
        core::mem::transmute(ADD_MESSAGE_JUMPQ);
    fn_ptr(text, time_ms, 1, 1);
}

/// Muestra texto de ayuda (caja amarilla) en pantalla.
/// `text` puede ser una cadena arbitraria terminada en nulo.
/// `beep` = true reproduce el sonido de ayuda.
pub unsafe fn show_help(text: *const u8, quick: bool, beep: bool) {
    let fn_ptr: unsafe extern "C" fn(*const u8, u8, u8, u8) =
        core::mem::transmute(SET_HELP_MESSAGE);
    fn_ptr(text, quick as u8, 0u8, beep as u8);
}
