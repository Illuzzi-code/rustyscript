// allocator.rs — Allocator global usando HeapAlloc de Windows.
// Necesario para colecciones (Vec, String) en no_std.

use core::alloc::{GlobalAlloc, Layout};

struct WinHeap;

unsafe impl GlobalAlloc for WinHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            HeapAlloc(GetProcessHeap(), 0, layout.size()) as *mut u8
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe {
            HeapFree(GetProcessHeap(), 0, ptr as *mut core::ffi::c_void);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, _layout: Layout, new_size: usize) -> *mut u8 {
        unsafe {
            HeapReAlloc(GetProcessHeap(), 0, ptr as *mut core::ffi::c_void, new_size) as *mut u8
        }
    }
}

#[global_allocator]
static HEAP: WinHeap = WinHeap;

// Win32 bindings (manual, sin winapi crate)
extern "system" {
    fn GetProcessHeap() -> *mut core::ffi::c_void;
    fn HeapAlloc(heap: *mut core::ffi::c_void, flags: u32, size: usize)
        -> *mut core::ffi::c_void;
    fn HeapFree(heap: *mut core::ffi::c_void, flags: u32, mem: *mut core::ffi::c_void)
        -> i32;
    fn HeapReAlloc(heap: *mut core::ffi::c_void, flags: u32,
                   mem: *mut core::ffi::c_void, size: usize) -> *mut core::ffi::c_void;
}
