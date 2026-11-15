// log.rs — Utilidades de log síncronas a RustyMod.log junto al .asi.

use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};

static LOG_READY: AtomicBool = AtomicBool::new(false);
static mut LOG_HANDLE: *mut core::ffi::c_void = core::ptr::null_mut();

extern "system" {
    fn CreateFileA(
        name: *const u8, access: u32, share: u32,
        sec: *mut core::ffi::c_void, disp: u32, flags: u32,
        tmpl: *mut core::ffi::c_void,
    ) -> *mut core::ffi::c_void;
    fn WriteFile(
        h: *mut core::ffi::c_void, buf: *const u8, len: u32,
        written: *mut u32, ol: *mut core::ffi::c_void,
    ) -> i32;
    fn CloseHandle(h: *mut core::ffi::c_void) -> i32;
    fn GetModuleFileNameA(h: *mut core::ffi::c_void, buf: *mut u8, size: u32) -> u32;
}

const GENERIC_WRITE:  u32 = 0x40000000;
const FILE_SHARE_READ:u32 = 0x00000001;
const CREATE_ALWAYS:  u32 = 2;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const INVALID_HANDLE: *mut core::ffi::c_void = !0usize as *mut core::ffi::c_void;

/// Inicializa el archivo de log.  Llamar una sola vez desde hook::install().
pub fn init() {
    // Construir la ruta: mismo directorio que el .asi + "RustyMod.log"
    let mut path = [0u8; 512];
    let len = unsafe { GetModuleFileNameA(core::ptr::null_mut(), path.as_mut_ptr(), 511) } as usize;
    // Reemplazar el nombre del archivo por RustyMod.log
    if len == 0 { return; }
    // Encontrar última barra
    let mut last_sep = 0usize;
    for i in 0..len {
        if path[i] == b'\\' || path[i] == b'/' { last_sep = i; }
    }
    // Escribir nombre de log después de la última barra
    let log_name = b"RustyMod.log\0";
    let dst = last_sep + 1;
    if dst + log_name.len() >= 512 { return; }
    path[dst..dst + log_name.len()].copy_from_slice(log_name);

    let h = unsafe {
        CreateFileA(
            path.as_ptr(), GENERIC_WRITE, FILE_SHARE_READ,
            core::ptr::null_mut(), CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL,
            core::ptr::null_mut(),
        )
    };
    if h != INVALID_HANDLE {
        unsafe { LOG_HANDLE = h; }
        LOG_READY.store(true, Ordering::SeqCst);
        write_line("RustyMod: DLL cargada");
    }
}

/// Escribe una línea al log (añade \r\n).
pub fn write_line(msg: &str) {
    if !LOG_READY.load(Ordering::Relaxed) { return; }
    let h = unsafe { LOG_HANDLE };
    if h.is_null() || h == INVALID_HANDLE { return; }
    let bytes = msg.as_bytes();
    let crlf  = b"\r\n";
    let mut written = 0u32;
    unsafe {
        WriteFile(h, bytes.as_ptr(), bytes.len() as u32, &mut written, core::ptr::null_mut());
        WriteFile(h, crlf.as_ptr(), 2, &mut written, core::ptr::null_mut());
    }
}

/// Formateador auxiliar para escribir con formato.
pub struct LogWriter {
    buf: [u8; 512],
    pos: usize,
}

impl LogWriter {
    pub fn new() -> Self { LogWriter { buf: [0; 512], pos: 0 } }

    pub fn flush(self) {
        let s = core::str::from_utf8(&self.buf[..self.pos]).unwrap_or("[invalid utf8]");
        write_line(s);
    }
}

impl fmt::Write for LogWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let space = self.buf.len() - self.pos;
        let n = bytes.len().min(space);
        self.buf[self.pos..self.pos + n].copy_from_slice(&bytes[..n]);
        self.pos += n;
        Ok(())
    }
}

/// Macro de log con formato.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut w = $crate::log::LogWriter::new();
        let _ = write!(w, $($arg)*);
        w.flush();
    }};
}
