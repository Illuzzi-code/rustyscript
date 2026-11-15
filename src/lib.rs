// lib.rs — Punto de entrada del .asi (DLL para GTA SA).
// Compilar con: cargo build --release --target i686-pc-windows-msvc
// El resultado en target/i686-pc-windows-msvc/release/rustymod.dll
// se copia como rustymod.asi en la carpeta del juego.

#![no_std]
#![allow(non_snake_case)]

extern crate alloc;

mod allocator;
mod log;
mod game;
mod hook;
mod player;
mod hud;
mod controls;
mod config;
pub mod script;

use core::panic::PanicInfo;

// ── DLL entry point ─────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn DllMain(
    _hmodule: *mut core::ffi::c_void,
    fdw_reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if fdw_reason == DLL_PROCESS_ATTACH {
        // Instalar el hook en el hilo actual (el hilo del juego llama DllMain).
        // El hook parchea el único CALL a CGame::Process para insertar nuestro
        // callback por frame.
        unsafe { hook::install(); }
    }
    1
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    log::write_line("[PANIC] rustymod panic — mod desactivado");
    loop {}
}
