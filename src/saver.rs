//! Screensaver windows (/s full screen on every monitor, /p preview child),
//! input-to-exit handling and the message loop.

use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};



use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::fx::{Animator, DISPLAY_FPS};
use crate::gfx::{wide, Fit, Surface};
use windows_sys::Win32::Media::{timeBeginPeriod, timeEndPeriod};

/// Mouse travel (px, from where the cursor was at start) that counts as "moved".
const MOVE_THRESHOLD: i32 = 8;
/// Input in the first moments after launch is ignored: Windows (and the click
/// on "Preview") can deliver stray events while the saver starts.
const GRACE_MS: u128 = 300;

struct Screen {
    hwnd: HWND,
    surface: Surface,
    anim: Animator,
}

struct App {
    screens: Vec<Screen>,
    preview: bool,
    start: Instant,
    start_cursor: POINT,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn quit(reason: &str) {
    if std::env::var_os("MYSCREENSAVER_DEBUG").is_some() {
        let path = std::env::temp_dir().join("myscreensaver-debug.log");
        let _ = std::fs::write(path, format!("exit: {reason}
"));
    }
    unsafe { PostQuitMessage(0) };
}

fn in_grace() -> bool {
    APP.with(|a| a.try_borrow().ok().and_then(|a| a.as_ref().map(|a| a.start.elapsed().as_millis() < GRACE_MS)))
        .unwrap_or(true)
}

fn is_preview() -> bool {
    APP.with(|a| a.try_borrow().ok().and_then(|a| a.as_ref().map(|a| a.preview)))
        .unwrap_or(false)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            APP.with(|a| {
                if let Ok(a) = a.try_borrow() {
                    if let Some(s) = a.as_ref().and_then(|a| a.screens.iter().find(|s| s.hwnd == hwnd)) {
                        s.surface.present(dc);
                    }
                }
            });
            EndPaint(hwnd, &ps);
            return 0;
        }
        WM_SETCURSOR if !is_preview() => {
            SetCursor(null_mut());
            return 1;
        }
        WM_DESTROY if is_preview() => {
            quit("preview window destroyed");
            return 0;
        }
        _ => {}
    }
    if !is_preview() {
        match msg {
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
            | WM_XBUTTONDOWN | WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                if !in_grace() {
                    quit(&format!("key/button message 0x{msg:x} wp={wp}"));
                }
                return 0;
            }
            WM_MOUSEMOVE => {
                let mut p = POINT { x: 0, y: 0 };
                GetCursorPos(&mut p);
                if moved_far(p) && !in_grace() {
                    quit(&format!("mouse moved to {},{}", p.x, p.y));
                }
                return 0;
            }
            WM_ACTIVATEAPP if wp == 0 && !in_grace() => {
                // another app took focus (e.g. Ctrl+Alt+Del): get out of the way
                quit("deactivated");
                return 0;
            }
            WM_SYSCOMMAND if (wp & 0xFFF0) as u32 == SC_SCREENSAVE || (wp & 0xFFF0) as u32 == SC_MONITORPOWER => {
                return 0;
            }
            WM_CLOSE => {
                quit("WM_CLOSE");
                return 0;
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

fn moved_far(p: POINT) -> bool {
    APP.with(|a| {
        a.try_borrow().ok().and_then(|a| {
            a.as_ref().map(|a| {
                (p.x - a.start_cursor.x).abs() > MOVE_THRESHOLD
                    || (p.y - a.start_cursor.y).abs() > MOVE_THRESHOLD
            })
        })
    }) == Some(true)
}

/// Low-level hooks see input whether or not our window has focus (a saver
/// started by another process is not always given the foreground).
unsafe extern "system" fn keyboard_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && (wp as u32 == WM_KEYDOWN || wp as u32 == WM_SYSKEYDOWN) && !in_grace() {
        quit("key (hook)");
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && !in_grace() {
        let info = &*(lp as *const MSLLHOOKSTRUCT);
        match wp as u32 {
            WM_MOUSEMOVE => {
                if moved_far(info.pt) {
                    quit(&format!("mouse moved to {},{} (hook)", info.pt.x, info.pt.y));
                }
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN | WM_MOUSEWHEEL
            | WM_MOUSEHWHEEL => quit("mouse button (hook)"),
            _ => {}
        }
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}

fn register_class() -> *const u16 {
    static CLASS: &str = "MyScreensaverWindow\0";
    let name: Vec<u16> = CLASS.encode_utf16().collect();
    let name = Box::leak(name.into_boxed_slice()).as_ptr();
    unsafe {
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(null()),
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
            lpszMenuName: null(),
            lpszClassName: name,
        };
        RegisterClassW(&wc);
    }
    name
}

unsafe extern "system" fn monitor_cb(_: HMONITOR, _: HDC, rc: *mut RECT, data: LPARAM) -> BOOL {
    let v = &mut *(data as *mut Vec<RECT>);
    v.push(*rc);
    TRUE
}

fn monitor_rects() -> Vec<RECT> {
    let mut v: Vec<RECT> = Vec::new();
    unsafe {
        EnumDisplayMonitors(null_mut(), null(), Some(monitor_cb), &mut v as *mut _ as LPARAM);
    }
    v
}

/// Paced loop: at most DISPLAY_FPS ticks a second; a window is redrawn only
/// when its frame changed, and while every screen is holding a finished frame
/// the thread sleeps until the hold ends (or input arrives).
fn frame_loop(parent: Option<HWND>) {
    let frame = Duration::from_secs_f32(1.0 / DISPLAY_FPS);
    let mut next_tick = Instant::now();
    unsafe { timeBeginPeriod(1) };
    'outer: loop {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    break 'outer;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if let Some(p) = parent {
                if IsWindow(p) == 0 {
                    break 'outer;
                }
            }
        }
        let now = Instant::now();
        if now >= next_tick {
            APP.with(|a| {
                if let Some(app) = a.borrow_mut().as_mut() {
                    for s in &mut app.screens {
                        if s.anim.tick(now) {
                            // only the rows that changed are redrawn and copied
                            if let Some((top, bottom)) = s.surface.draw(&mut s.anim.grid) {
                                unsafe {
                                    let dc = GetDC(s.hwnd);
                                    s.surface.present_band(dc, top, bottom);
                                    ReleaseDC(s.hwnd, dc);
                                }
                            }
                        }
                    }
                }
            });
            next_tick += frame;
            if next_tick < now {
                next_tick = now + frame; // fell behind: don't try to catch up
            }
        }
        // sleep until the next tick, or until the earliest hold ends
        let wake = APP.with(|a| {
            a.borrow().as_ref().map(|app| {
                app.screens
                    .iter()
                    .map(|s| s.anim.wake_at().unwrap_or(next_tick).max(next_tick))
                    .min()
                    .unwrap_or(next_tick)
            })
        });
        let wake = wake.unwrap_or(next_tick);
        let ms = wake.saturating_duration_since(Instant::now()).as_millis() as u32;
        if ms > 0 {
            unsafe { MsgWaitForMultipleObjects(0, null(), FALSE, ms.min(250), QS_ALLINPUT) };
        }
    }
    unsafe { timeEndPeriod(1) };
}

/// Run the screensaver. `parent` = Some(hwnd) for the settings preview box.
pub fn run(parent: Option<HWND>) {
    let settings = crate::settings::Settings::load();
    let art = crate::art::load(&settings.art_path);
    let effects = settings.enabled_effects();
    let (art_cols, art_rows) = crate::art::size(&art);
    let class = register_class();
    let title = wide("MyScreensaver");
    let preview = parent.is_some();

    let mut screens = Vec::new();
    unsafe {
        let rects: Vec<RECT> = match parent {
            Some(p) => {
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(p, &mut rc);
                vec![rc]
            }
            None => monitor_rects(),
        };
        let fit = if preview {
            Fit { max_w: 0.9, max_h: 0.6, min_rows: 0, min_px: 2 }
        } else {
            Fit { max_w: 0.8, max_h: 0.6, min_rows: 28, min_px: 6 }
        };
        for rc in rects {
            let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
            let hwnd = match parent {
                Some(p) => CreateWindowExW(
                    0, class, title.as_ptr(), WS_CHILD | WS_VISIBLE,
                    0, 0, w, h, p, null_mut(), GetModuleHandleW(null()), null(),
                ),
                None => CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW, class, title.as_ptr(), WS_POPUP | WS_VISIBLE,
                    rc.left, rc.top, w, h, null_mut(), null_mut(), GetModuleHandleW(null()), null(),
                ),
            };
            if hwnd.is_null() {
                continue;
            }
            let surface = Surface::new(hwnd, w, h, art_cols, art_rows, &fit);
            let anim = Animator::new(
                art.clone(),
                surface.cols,
                surface.rows,
                effects.clone(),
                settings.speed,
                settings.palette(),
            );
            screens.push(Screen { hwnd, surface, anim });
        }
    }
    if screens.is_empty() {
        return;
    }
    for s in &mut screens {
        s.surface.draw(&mut s.anim.grid);
    }
    let mut start_cursor = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut start_cursor);
        if !preview {
            ShowCursor(FALSE);
            SetForegroundWindow(screens[0].hwnd);
            SetFocus(screens[0].hwnd);
        }
    }
    let hwnds: Vec<HWND> = screens.iter().map(|s| s.hwnd).collect();
    APP.with(|a| {
        *a.borrow_mut() = Some(App { screens, preview, start: Instant::now(), start_cursor })
    });
    for &h in &hwnds {
        unsafe {
            InvalidateRect(h, null(), FALSE);
            UpdateWindow(h);
        }
    }

    let hooks = if preview {
        Vec::new()
    } else {
        unsafe {
            let hinst = GetModuleHandleW(null());
            vec![
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), hinst, 0),
                SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), hinst, 0),
            ]
        }
    };
    frame_loop(parent);
    for h in hooks {
        if !h.is_null() {
            unsafe { UnhookWindowsHookEx(h) };
        }
    }

    let app = APP.with(|a| a.borrow_mut().take());
    if let Some(app) = app {
        for s in &app.screens {
            unsafe {
                if IsWindow(s.hwnd) != 0 {
                    DestroyWindow(s.hwnd);
                }
            }
        }
        if !app.preview {
            unsafe { ShowCursor(TRUE) };
        }
    }
}
