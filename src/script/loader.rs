// src/script/loader.rs — File-system helpers for loading .rscript files.
// Uses Win32 FindFirstFileA/FindNextFileA and CreateFileA/ReadFile/CloseHandle.
// No std, no external crates.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

// ── Win32 bindings ───────────────────────────────────────────────────────────

#[repr(C)]
struct Win32FindDataA {
    dw_file_attributes:    u32,
    ft_creation_time:      [u32; 2],
    ft_last_access_time:   [u32; 2],
    ft_last_write_time:    [u32; 2],
    n_file_size_high:      u32,
    n_file_size_low:       u32,
    dw_reserved0:          u32,
    dw_reserved1:          u32,
    c_file_name:           [u8; 260],
    c_alternate_file_name: [u8; 14],
    dw_file_type:          u32,
    dw_creator_type:       u32,
    w_finder_flags:        u16,
}

extern "system" {
    fn FindFirstFileA(
        pattern: *const u8,
        data:    *mut Win32FindDataA,
    ) -> *mut core::ffi::c_void;

    fn FindNextFileA(
        handle: *mut core::ffi::c_void,
        data:   *mut Win32FindDataA,
    ) -> i32;

    fn FindClose(handle: *mut core::ffi::c_void) -> i32;

    fn CreateFileA(
        name:    *const u8,
        access:  u32,
        share:   u32,
        sec:     *mut core::ffi::c_void,
        disp:    u32,
        flags:   u32,
        tmpl:    *mut core::ffi::c_void,
    ) -> *mut core::ffi::c_void;

    fn ReadFile(
        handle:  *mut core::ffi::c_void,
        buf:     *mut u8,
        to_read: u32,
        read:    *mut u32,
        ol:      *mut core::ffi::c_void,
    ) -> i32;

    fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
}

const GENERIC_READ:       u32 = 0x80000000;
const FILE_SHARE_READ:    u32 = 0x00000001;
const OPEN_EXISTING:      u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const INVALID_HANDLE:     *mut core::ffi::c_void = !0usize as *mut core::ffi::c_void;

// ── list_script_files ────────────────────────────────────────────────────────

/// Return the full paths of every `*.rscript` file in `dir` (non-recursive).
/// Returns an empty Vec on any error.
pub fn list_script_files(dir: &str) -> Vec<String> {
    let mut results = Vec::new();

    // Build the search pattern: "<dir>\*.rscript\0"
    let mut pattern = String::new();
    pattern.push_str(dir);
    if !dir.ends_with('\\') && !dir.ends_with('/') {
        pattern.push('\\');
    }
    pattern.push_str("*.rscript");
    let pattern_cstr = str_to_cstring(&pattern);

    // Safety: pattern_cstr is a valid null-terminated byte slice we own;
    // Win32FindDataA is a plain C struct with no invariants.
    let mut find_data: Win32FindDataA = unsafe { core::mem::zeroed() };
    let handle = unsafe {
        FindFirstFileA(pattern_cstr.as_ptr(), &mut find_data)
    };

    if handle == INVALID_HANDLE {
        return results;
    }

    loop {
        // c_file_name is a null-terminated ASCII filename (not full path).
        let name = cstr_to_string(&find_data.c_file_name);

        // Skip directories (attribute bit 4 = FILE_ATTRIBUTE_DIRECTORY).
        let is_dir = (find_data.dw_file_attributes & 0x10) != 0;
        if !is_dir && name.ends_with(".rscript") {
            let mut full = String::new();
            full.push_str(dir);
            if !dir.ends_with('\\') && !dir.ends_with('/') {
                full.push('\\');
            }
            full.push_str(&name);
            results.push(full);
        }

        // Safety: handle is a valid HANDLE returned by FindFirstFileA.
        let more = unsafe { FindNextFileA(handle, &mut find_data) };
        if more == 0 {
            break;
        }
    }

    // Safety: handle is a valid HANDLE we opened above.
    unsafe { FindClose(handle); }

    results
}

// ── read_file ────────────────────────────────────────────────────────────────

/// Read up to 65 536 bytes from `path` and return them as a UTF-8 String.
/// Returns `None` on any I/O error or if the content is not valid UTF-8.
pub fn read_file(path: &str) -> Option<String> {
    let path_cstr = str_to_cstring(path);

    // Safety: path_cstr is a valid null-terminated string we own.
    let handle = unsafe {
        CreateFileA(
            path_cstr.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ,
            core::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            core::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE {
        return None;
    }

    const MAX_SIZE: usize = 65_536;
    let mut buf: Vec<u8> = Vec::new();
    // Reserve space up-front; zero-initialise so ReadFile can write into it.
    for _ in 0..MAX_SIZE {
        buf.push(0u8);
    }

    let mut bytes_read: u32 = 0;

    // Safety: buf has at least MAX_SIZE bytes; handle is valid; we pass the
    // correct length.  bytes_read receives the actual count written.
    let ok = unsafe {
        ReadFile(
            handle,
            buf.as_mut_ptr(),
            MAX_SIZE as u32,
            &mut bytes_read,
            core::ptr::null_mut(),
        )
    };

    // Safety: handle is valid.
    unsafe { CloseHandle(handle); }

    if ok == 0 {
        return None;
    }

    buf.truncate(bytes_read as usize);

    // Convert to UTF-8.  Treat invalid sequences as an error so scripts with
    // garbage encoding fail cleanly at load time rather than at runtime.
    match core::str::from_utf8(&buf) {
        Ok(s) => Some(String::from(s)),
        Err(_) => {
            crate::log!("loader: {} is not valid UTF-8, skipped", path);
            None
        }
    }
}

// ── String helpers ───────────────────────────────────────────────────────────

/// Build a null-terminated byte Vec from a Rust &str (ASCII/UTF-8 passthrough).
fn str_to_cstring(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    for b in s.bytes() {
        v.push(b);
    }
    v.push(0);
    v
}

/// Read a null-terminated byte slice into a String, stopping at the first NUL.
fn cstr_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let slice = &bytes[..end];
    // Safety: we constructed this from a Win32 file name; on a Western locale
    // it is ASCII.  Non-ASCII bytes are passed through as Latin-1 equivalents.
    // Using from_utf8_lossy to avoid panics on unusual code pages.
    let cow = alloc::string::String::from_utf8_lossy(slice);
    cow.into_owned()
}
