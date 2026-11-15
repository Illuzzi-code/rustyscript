// controls.rs — Detección de teclas sin dependencias externas.
// Usa GetAsyncKeyState (Win32) para leer el estado de teclas.
// Se llama una vez por frame desde hook::tick().

use core::sync::atomic::{AtomicU8, Ordering};

extern "system" {
    fn GetAsyncKeyState(vk: i32) -> i16;
    fn GetForegroundWindow() -> *mut core::ffi::c_void;
    fn GetCurrentThreadId() -> u32;
    fn GetWindowThreadProcessId(hwnd: *mut core::ffi::c_void, pid: *mut u32) -> u32;
}

// Estado de tecla: 0=libre, 1=pressed this frame, 2=held
static KEY_STATE: [AtomicU8; 256] = {
    // AtomicU8 no implementa Copy, inicializar con un const array hack
    const ZERO: AtomicU8 = AtomicU8::new(0);
    [ZERO; 256]
};

/// Debe llamarse exactamente una vez por frame.
pub unsafe fn poll() {
    // Solo procesar teclas si el juego está en primer plano
    let fg = GetForegroundWindow();
    if fg.is_null() { return; }
    let tid = GetWindowThreadProcessId(fg, core::ptr::null_mut());
    let my_tid = GetCurrentThreadId();
    // Si la ventana del primer plano no pertenece a nuestro hilo, no leer teclas.
    // (El juego y el mod comparten hilo, así que tid == my_tid cuando el juego está activo.)
    if tid != my_tid { return; }

    for vk in 0u8..=255 {
        let s = GetAsyncKeyState(vk as i32);
        // Bit 15: tecla presionada ahora. Bit 0: fue presionada desde el último call.
        let down = (s as u16) & 0x8000 != 0;
        let prev = KEY_STATE[vk as usize].load(Ordering::Relaxed);
        let next = if down {
            if prev == 0 { 1 } else { 2 }
        } else {
            0
        };
        KEY_STATE[vk as usize].store(next, Ordering::Relaxed);
    }
}

/// Devuelve true solo el primer frame en que la tecla se presiona.
#[inline]
pub fn just_pressed(vk: u8) -> bool {
    KEY_STATE[vk as usize].load(Ordering::Relaxed) == 1
}

/// Devuelve true mientras la tecla esté mantenida.
#[inline]
pub fn is_held(vk: u8) -> bool {
    KEY_STATE[vk as usize].load(Ordering::Relaxed) >= 1
}

// Virtual key codes para usar en scripts
pub mod vk {
    pub const F1:  u8 = 0x70;
    pub const F2:  u8 = 0x71;
    pub const F3:  u8 = 0x72;
    pub const F4:  u8 = 0x73;
    pub const F5:  u8 = 0x74;
    pub const F6:  u8 = 0x75;
    pub const F7:  u8 = 0x76;
    pub const F8:  u8 = 0x77;
    pub const F9:  u8 = 0x78;
    pub const F10: u8 = 0x79;
    pub const F11: u8 = 0x7A;
    pub const F12: u8 = 0x7B;
    pub const NUM0: u8 = 0x60;
    pub const NUM1: u8 = 0x61;
    pub const NUM2: u8 = 0x62;
    pub const NUM3: u8 = 0x63;
    pub const NUM4: u8 = 0x64;
    pub const NUM5: u8 = 0x65;
    pub const NUM6: u8 = 0x66;
    pub const NUM7: u8 = 0x67;
    pub const NUM8: u8 = 0x68;
    pub const NUM9: u8 = 0x69;
}
