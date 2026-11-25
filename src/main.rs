//! MyScreensaver — an Omarchy-style terminal text effects screensaver for Windows.
//!
//! Standard screensaver arguments:
//!   /s           run full screen on every monitor
//!   /p <HWND>    draw a preview inside the given window (Screen Saver Settings box)
//!   /c[:HWND]    settings dialog
//!   (none)       settings dialog
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod config;
mod fx;
mod gfx;
mod saver;
mod settings;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

enum Mode {
    Saver,
    Preview(HWND),
    /// Settings dialog, optionally owned by the Control Panel window.
    Config(Option<HWND>),
}

/// Parse "/s", "-S", "/p 1234", "/p:1234", "/c:1234", ... (case-insensitive).
fn parse_args() -> Mode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(first) = args.first() else { return Mode::Config(None) };
    let a = first.trim_start_matches(['/', '-']).to_ascii_lowercase();
    let (flag, inline) = match a.split_once(':') {
        Some((f, v)) => (f.to_string(), Some(v.to_string())),
        None => (a.clone(), None),
    };
    let hwnd = || {
        inline
            .clone()
            .or_else(|| args.get(1).cloned())
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v as HWND)
    };
    match flag.as_str() {
        "s" => Mode::Saver,
        "p" | "l" => match hwnd() {
            Some(h) => Mode::Preview(h),
            None => Mode::Config(None),
        },
        // "/c" alone, "/c:HWND", or anything unknown
        _ => Mode::Config(inline.and_then(|v| v.trim().parse::<usize>().ok()).map(|v| v as HWND)),
    }
}

fn main() {
    // Physical pixels on every monitor, whatever its scale factor.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    // No console in a screensaver: leave a note if we ever crash.
    std::panic::set_hook(Box::new(|info| {
        let path = std::env::temp_dir().join("myscreensaver-panic.log");
        let _ = std::fs::write(path, format!("{info}\n{}", std::backtrace::Backtrace::force_capture()));
    }));
    match parse_args() {
        Mode::Saver => saver::run(None),
        Mode::Preview(h) => saver::run(Some(h)),
        Mode::Config(owner) => config::run(owner),
    }
}
