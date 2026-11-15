fn main() {
    // Enlazar las librerías de sistema necesarias.
    // kernel32 = GetProcessHeap, HeapAlloc, HeapFree, HeapReAlloc,
    //            VirtualProtect, GetCurrentProcess, GetCurrentThreadId,
    //            GetModuleFileNameA, CreateFileA, WriteFile, CloseHandle
    // user32   = GetAsyncKeyState, GetForegroundWindow, GetWindowThreadProcessId
    println!("cargo:rustc-link-lib=kernel32");
    println!("cargo:rustc-link-lib=user32");
}
