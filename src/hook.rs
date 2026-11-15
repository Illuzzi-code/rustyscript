// hook.rs — Parchea el único CALL a CGame::Process para insertar nuestro
// callback por frame.
//
// PROBLEMA CONOCIDO: otros ASI (CLEO, etc.) pueden haber patcheado el mismo
// call site ANTES que nosotros. Los bytes en disco son E8 5A D5 FF FF
// (→ 0x53BEE0), pero en memoria pueden apuntar a una trampoline intermedia.
//
// SOLUCIÓN: guardamos los bytes originales EN MEMORIA (lo que haya, sea la
// dirección de CGame::Process o la trampoline de otro ASI) y en nuestro
// on_game_process llamamos a ESA dirección, no a 0x53BEE0 fija.
// Así encadenamos correctamente cualquier hook previo.
//
// CADENA DE LLAMADAS:
//   game_idle (0x53E920)
//     → [E8 nuestro rel32] → on_game_process
//         → llama a PREV_TARGET (lo que hubiera antes: CGame::Process o trampoline CLEO)
//         → llama a tick()
//
// REGISTRO DE BYTES en log: bytes antes y después del parche.

use crate::game::HOOK_CALL_SITE;
use crate::{log, player, controls};
use crate::script::ScriptEngine;

// ── Global ScriptEngine ──────────────────────────────────────────────────────
// Wrapped in an Option so it can be initialised after the allocator is ready.
static mut SCRIPT_ENGINE: Option<ScriptEngine> = None;

/// Tracks whether the first game frame has occurred (safe to call game functions).
static mut FIRST_FRAME_DONE: bool = false;

/// VA a la que apuntaba el CALL original en memoria (puede ser CGame::Process
/// o la trampoline de otro ASI como CLEO).  Se inicializa en install().
static mut PREV_TARGET: usize = 0;
static mut HOOK_INSTALLED: bool = false;

extern "system" {
    fn VirtualProtect(
        addr: *mut core::ffi::c_void,
        size: usize,
        new_protect: u32,
        old_protect: *mut u32,
    ) -> i32;
}

const PAGE_EXECUTE_READWRITE: u32 = 0x40;

/// Instala el hook por frame.  Llamar una sola vez desde DllMain.
pub unsafe fn install() {
    crate::log::init();

    // Verificar firma de CGame::Process en disco (siempre es 0x53BEE0, sin ASLR)
    let cgame = crate::game::CGAME_PROCESS as *const u8;
    let prologue = core::slice::from_raw_parts(cgame, 8);
    let expected = [0x83u8, 0xEC, 0x0C, 0x53, 0x56, 0x57, 0xE8, 0xE5];
    let sig_ok   = prologue == expected;

    log!("RustyMod: GTA SA 1.0 US detectado");
    log!("RustyMod: firmas de direcciones:");
    log!(
        "  CGame::Process @ 0x{:08X}: [{:02X}, {:02X}, {:02X}, {:02X}, {:02X}, {:02X}, {:02X}, {:02X}]  {}",
        crate::game::CGAME_PROCESS,
        prologue[0], prologue[1], prologue[2], prologue[3],
        prologue[4], prologue[5], prologue[6], prologue[7],
        if sig_ok { "OK" } else { "FIRMA NO COINCIDE" }
    );

    if !sig_ok {
        log!("RustyMod: ERROR — firma CGame::Process no coincide. Mod desactivado.");
        return;
    }

    // Leer los 5 bytes ACTUALES en memoria en el call site.
    // Pueden ser el CALL original o una trampoline de otro ASI.
    let call_site = HOOK_CALL_SITE as *mut u8;
    let cur0 = call_site.read();
    let cur1 = call_site.add(1).read();
    let cur2 = call_site.add(2).read();
    let cur3 = call_site.add(3).read();
    let cur4 = call_site.add(4).read();

    log!(
        "RustyMod: call site 0x{:08X} bytes en memoria: {:02X} {:02X} {:02X} {:02X} {:02X}",
        HOOK_CALL_SITE, cur0, cur1, cur2, cur3, cur4
    );

    if cur0 != 0xE8 {
        log!("RustyMod: ERROR — call site no es E8 (CALL). Mod desactivado.");
        return;
    }

    // Decodificar el rel32 del CALL actual para obtener el target previo.
    // rel32 es con signo, instrucción de 5 bytes → target = site+5+rel32
    let rel32_bytes = [cur1, cur2, cur3, cur4];
    let rel32 = i32::from_le_bytes(rel32_bytes);
    let prev_target = HOOK_CALL_SITE.wrapping_add(5).wrapping_add(rel32 as usize);
    PREV_TARGET = prev_target;

    log!(
        "RustyMod: target previo del CALL: 0x{:08X}  (CGame::Process=0x{:08X})  {}",
        prev_target,
        crate::game::CGAME_PROCESS,
        if prev_target == crate::game::CGAME_PROCESS {
            "directo"
        } else {
            "trampoline de otro mod (CLEO?)"
        }
    );

    // Desproteger la página del call site
    let mut old_prot = 0u32;
    let r = VirtualProtect(
        call_site as *mut core::ffi::c_void,
        5, PAGE_EXECUTE_READWRITE, &mut old_prot,
    );
    if r == 0 {
        log!("RustyMod: ERROR — VirtualProtect falló. Mod desactivado.");
        return;
    }

    // Escribir nuevo rel32 → on_game_process
    let hook_fn = on_game_process as *const () as usize;
    let new_rel  = hook_fn.wrapping_sub(HOOK_CALL_SITE.wrapping_add(5)) as i32;
    let new_rel_b = new_rel.to_le_bytes();

    call_site.write(0xE8);
    call_site.add(1).write(new_rel_b[0]);
    call_site.add(2).write(new_rel_b[1]);
    call_site.add(3).write(new_rel_b[2]);
    call_site.add(4).write(new_rel_b[3]);

    // Restaurar protección
    VirtualProtect(
        call_site as *mut core::ffi::c_void,
        5, old_prot, &mut old_prot,
    );

    HOOK_INSTALLED = true;
    log!(
        "RustyMod: hook instalado en 0x{:08X} → 0x{:08X}",
        HOOK_CALL_SITE, hook_fn
    );
    log!(
        "RustyMod: bytes nuevos: E8 {:02X} {:02X} {:02X} {:02X}",
        new_rel_b[0], new_rel_b[1], new_rel_b[2], new_rel_b[3]
    );
    log!("RustyMod: cadena: game_idle → on_game_process → 0x{:08X} → tick", prev_target);

    // ScriptEngine se inicializa en el primer frame, NO aquí.
    // Llamar al juego desde DllMain crashea porque el juego aún no está listo.
    SCRIPT_ENGINE = Some(ScriptEngine::new());
    log!("RustyMod: ScriptEngine creado — scripts se cargarán en el primer frame");
}

/// Callback instalado en el call site.
/// Llama primero al target previo (CGame::Process o trampoline de otro ASI),
/// luego a nuestro tick().
/// Convención: mismo contexto que el CALL original (ningún argumento en stack
/// en este punto — la función idle no pasó args al call).
#[unsafe(naked)]
unsafe extern "C" fn on_game_process() {
    core::arch::naked_asm!(
        "call {prev}",
        "call {tick}",
        "ret",
        prev = sym call_prev_target,
        tick = sym tick,
    )
}

/// Llama a PREV_TARGET (lo que fuera que estaba en el call site antes).
unsafe extern "C" fn call_prev_target() {
    if PREV_TARGET == 0 { return; }
    let fn_ptr: unsafe extern "C" fn() = core::mem::transmute(PREV_TARGET);
    fn_ptr();
}

/// Se ejecuta una vez por frame, en el hilo del juego.
unsafe extern "C" fn tick() {
    controls::poll();
    player::update();

    if let Some(engine) = &mut SCRIPT_ENGINE {
        // Primera vez que llegamos aquí: el juego ya está corriendo.
        // Es seguro llamar a funciones del juego ahora.
        if !FIRST_FRAME_DONE {
            FIRST_FRAME_DONE = true;
            log!("RustyMod: primer frame — cargando scripts");
            engine.load_all("scripts");
            log!("RustyMod: scripts cargados");
        }

        // Run on_frame for all scripts.
        engine.tick();

        // Dispatch on_key / on_held for every virtual key.
        for vk in 0u8..=255u8 {
            if controls::just_pressed(vk) {
                engine.on_key(vk);
            }
            if controls::is_held(vk) {
                engine.on_held(vk);
            }
        }

        // F9: reload all scripts.
        if controls::just_pressed(crate::config::KEY_RELOAD_SCRIPTS) {
            engine.reload_all("scripts");
        }
    }
}
