// src/script/mod.rs — Top-level scripting engine module.
// Owns all loaded scripts and drives the per-frame execution loop.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

pub mod lexer;
pub mod parser;
pub mod compiler;
pub mod vm;
pub mod natives;
pub mod loader;

pub use lexer::*;
pub use parser::*;
pub use compiler::*;
pub use vm::*;
pub use natives::*;

// ── Win32 for timing ─────────────────────────────────────────────────────────

extern "system" {
    fn QueryPerformanceCounter(out: *mut i64) -> i32;
    fn QueryPerformanceFrequency(out: *mut i64) -> i32;
}

/// Read the high-resolution counter value.
#[inline]
fn qpc() -> i64 {
    let mut v: i64 = 0;
    // Safety: &mut v is a valid aligned pointer; QPC always succeeds on WinXP+.
    unsafe { QueryPerformanceCounter(&mut v); }
    v
}

/// Return the counter frequency (counts per second), cached after first call.
fn qpc_freq() -> i64 {
    static mut FREQ: i64 = 0;
    // Safety: single-threaded game loop; no data race possible.
    unsafe {
        if FREQ == 0 {
            QueryPerformanceFrequency(&mut FREQ);
        }
        FREQ
    }
}

/// Elapsed microseconds between two QPC values.
#[inline]
fn elapsed_us(start: i64, end: i64) -> u64 {
    let diff = (end - start).max(0) as u64;
    let freq = qpc_freq() as u64;
    if freq == 0 { return 0; }
    diff.saturating_mul(1_000_000) / freq
}

// ── ScriptState ───────────────────────────────────────────────────────────────

/// All runtime state for one loaded `.rscript` file.
pub struct ScriptState {
    pub path:        String,
    pub source:      String,
    pub chunk:       Chunk,
    pub vm_instance: Vm,
    pub globals:     Vec<Value>,
    pub error_count: u32,
    pub disabled:    bool,
    error_times_ms:  Vec<u64>,
    /// When Some(ip), the script is paused inside the handler that starts at
    /// `ip`, waiting for `vm_instance.wait_until_ms` to elapse.  While paused,
    /// no other handler of this script may run.
    paused_in:      Option<usize>,
    budget_paused:  bool,
}

impl ScriptState {
    fn new(path: String, source: String, chunk: Chunk) -> Self {
        ScriptState {
            path,
            source,
            chunk,
            vm_instance: Vm::new(),
            globals:     Vec::new(),
            error_count: 0,
            disabled:    false,
            error_times_ms: Vec::new(),
            paused_in:   None,
            budget_paused:  false,
        }
    }

    /// Run the `on_start` handler once (at load time).
    fn run_start(&mut self) {
        if self.disabled { return; }
        if let Some(ip) = self.chunk.on_start {
            if let Err(e) = self.run_handler_internal(ip, None) {
                self.handle_error("on_start", &e);
            }
        }
    }

    /// Run the `on_frame` handler with a time budget (microseconds).
    fn run_frame(&mut self, budget_us: u64) {
        if self.disabled { return; }

        // A handler paused by wait() must resume independently of the event
        // that started it. Key-only handlers otherwise never get called again.
        if let Some(paused_ip) = self.paused_in {
            if self.chunk.on_frame != Some(paused_ip) {
                if let Err(e) = self.run_handler_internal(paused_ip, None) {
                    self.handle_error("resume", &e);
                }
                return;
            }
        }

        let ip = match self.chunk.on_frame {
            Some(ip) => ip,
            None     => return,
        };

        // If paused in a *different* handler, don't start on_frame — let the
        // paused one resume first.
        if let Some(p) = self.paused_in {
            if p != ip { return; }
        }

        let start = qpc();
        let result = self.run_handler_internal(ip, Some(budget_us));
        let end   = qpc();
        let us    = elapsed_us(start, end);

        if us > budget_us {
            crate::log!(
                "warn[W01]: script '{}' on_frame exceeded budget ({}us > {}us)",
                self.path, us, budget_us
            );
        }

        if let Err(e) = result {
            self.handle_error("on_frame", &e);
        }
    }

    /// Run the `on_key` handler matching the given VK code.
    fn run_key(&mut self, vk: u8) {
        if self.disabled { return; }
        let ip = self.chunk.on_keys.iter()
            .find(|(k, _)| *k == vk)
            .map(|(_, ip)| *ip);
        if let Some(ip) = ip {
            // If paused in another handler, ignore this key press.
            if let Some(p) = self.paused_in {
                if p != ip { return; }
            }
            if let Err(e) = self.run_handler_internal(ip, None) {
                self.handle_error("on_key", &e);
            }
        }
    }

    /// Run the `on_held` handler matching the given VK code.
    fn run_held(&mut self, vk: u8) {
        if self.disabled { return; }
        let ip = self.chunk.on_helds.iter()
            .find(|(k, _)| *k == vk)
            .map(|(_, ip)| *ip);
        if let Some(ip) = ip {
            if let Some(p) = self.paused_in {
                if p != ip { return; }
            }
            if let Err(e) = self.run_handler_internal(ip, None) {
                self.handle_error("on_held", &e);
            }
        }
    }

    /// Internal: runs a handler, handling pause/resume.
    /// Returns Ok(()) if it completed or paused, Err if it errored.
    fn run_handler_internal(&mut self, ip: usize, budget_us: Option<u64>) -> Result<(), VmError> {
        let resume;

        // Are we resuming a paused handler?
        if self.paused_in == Some(ip) {
            // Check if the wait is over.
            if let Some(target) = self.vm_instance.wait_until_ms {
                // Safety: now_ms just calls GetTickCount.
                let now = unsafe { vm::now_ms() };
                if now < target {
                    // Still waiting — skip this frame for this handler.
                    return Ok(());
                }
                // Wait is over — clear and resume.
                self.vm_instance.wait_until_ms = None;
                resume = true;
            } else {
                resume = self.budget_paused;
            }
        } else {
            // Starting fresh.
            resume = false;
        }

        let result = self.vm_instance.run(
            &self.chunk,
            &mut self.globals,
            ip,
            resume,
            budget_us,
        );

        match result {
            Ok(vm::RunResult::Complete) => {
                self.paused_in = None;
                self.budget_paused = false;
                Ok(())
            }
            Ok(vm::RunResult::Paused) => {
                self.paused_in = Some(ip);
                self.budget_paused = false;
                Ok(())
            }
            Ok(vm::RunResult::BudgetExceeded) => {
                self.paused_in = Some(ip);
                self.budget_paused = true;
                crate::log!(
                    "warn[W01]: script '{}' exceeded its execution budget; paused",
                    self.path
                );
                Ok(())
            }
            Err(e) => {
                self.paused_in = None;
                self.budget_paused = false;
                Err(e)
            }
        }
    }

    fn handle_error(&mut self, handler: &str, e: &VmError) {
        let now = unsafe { vm::now_ms() };
        self.error_times_ms.retain(|time| now.saturating_sub(*time) <= 5_000);
        self.error_times_ms.push(now);
        self.error_count = self.error_times_ms.len() as u32;
        crate::log!(
            "error[E10]: script '{}' {} error #{}: {} (line {})",
            self.path, handler, self.error_count, e.msg, e.line
        );
        if self.error_times_ms.len() >= 3 {
            self.disabled = true;
            crate::log!(
                "error[E10]: script '{}' disabled after {} errors",
                self.path, self.error_count
            );
        }
    }
}

// ── ScriptEngine ──────────────────────────────────────────────────────────────

/// Top-level owner of all scripts.  One instance lives in the mod's global state.
pub struct ScriptEngine {
    scripts:    Vec<ScriptState>,
    scripts_dir: String,
}

impl ScriptEngine {
    pub fn new() -> Self {
        ScriptEngine {
            scripts:     Vec::new(),
            scripts_dir: String::new(),
        }
    }

    /// Load all `*.rscript` files found in `scripts_dir`.
    /// Does not clear previously loaded scripts — call `reload_all` for a clean reload.
    pub fn load_all(&mut self, scripts_dir: &str) {
        self.scripts_dir = String::from(scripts_dir);
        let paths = loader::list_script_files(scripts_dir);

        if paths.is_empty() {
            crate::log!("ScriptEngine: no .rscript files in '{}'", scripts_dir);
            return;
        }

        for path in paths {
            self.load_one(&path);
        }
    }

    /// Drop all scripts and reload from disk.
    pub fn reload_all(&mut self, scripts_dir: &str) {
        crate::log!("ScriptEngine: reloading all scripts from '{}'", scripts_dir);
        self.scripts.clear();
        self.load_all(scripts_dir);
    }

    /// Called every game frame.
    /// Runs `on_frame` for every enabled script within the per-script budget.
    pub fn tick(&mut self) {
        let budget = crate::config::SCRIPT_FRAME_BUDGET_US;
        for s in &mut self.scripts {
            s.run_frame(budget);
        }
    }

    /// Called when a key is just-pressed (single frame).
    /// Runs the matching `on_key` handler in every enabled script.
    pub fn on_key(&mut self, vk: u8) {
        for s in &mut self.scripts {
            s.run_key(vk);
        }
    }

    /// Called every frame while a key is held.
    /// Runs the matching `on_held` handler in every enabled script.
    pub fn on_held(&mut self, vk: u8) {
        for s in &mut self.scripts {
            s.run_held(vk);
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn load_one(&mut self, path: &str) {
        crate::log!("ScriptEngine: loading '{}'", path);

        // Read source text.
        let source = match loader::read_file(path) {
            Some(s) => s,
            None => {
                crate::log!("ScriptEngine: failed to read '{}'", path);
                return;
            }
        };

        // Lex.
        let mut lex = lexer::Lexer::new(&source);
        let tokens  = lex.tokenize();

        // Parse.
        let ast = match parser::parse(tokens) {
            Ok(a)  => a,
            Err(e) => {
                crate::log!(
                    "ScriptEngine: parse error in '{}' line {}: {}",
                    path, e.line, e.msg
                );
                return;
            }
        };

        // Compile.
        let chunk = match compiler::compile(&ast) {
            Ok(c)  => c,
            Err(e) => {
                crate::log!(
                    "ScriptEngine: compile error in '{}' line {}: {}",
                    path, e.line, e.msg
                );
                return;
            }
        };

        // Warn if file has no event blocks (not an error — still load).
        let has_events = chunk.on_start.is_some()
            || chunk.on_frame.is_some()
            || !chunk.on_keys.is_empty()
            || !chunk.on_helds.is_empty();
        if !has_events {
            crate::log!("warn: script '{}' has no event handlers", path);
        }

        let mut state = ScriptState::new(
            String::from(path),
            source,
            chunk,
        );

        // Run the on_start handler immediately.
        state.run_start();

        self.scripts.push(state);
        crate::log!("ScriptEngine: loaded '{}' OK", path);
    }
}
