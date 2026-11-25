//! The settings dialog (/c, or no arguments).
//!
//! Built from plain Win32 controls at runtime; a common-controls v6
//! activation context is created on the fly so the controls get the modern
//! look without a resource compiler.

use std::cell::RefCell;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::ApplicationInstallationAndServicing::{ActivateActCtx, CreateActCtxW, ACTCTXW};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::gfx::wide;
use crate::settings::{Settings, CUSTOM_THEME, SPEED_MAX, SPEED_MIN, THEMES};

const ID_ART: i32 = 101;
const ID_BROWSE: i32 = 102;
const ID_DEFAULT_ART: i32 = 103;
const ID_EFFECTS: i32 = 104;
const ID_ALL: i32 = 105;
const ID_NONE: i32 = 106;
const ID_THEME: i32 = 107;
const ID_COLOR: i32 = 108;
const ID_SPEED: i32 = 109;
const ID_SPEED_LABEL: i32 = 110;
const ID_PREVIEW: i32 = 111;
const ID_SWATCH: i32 = 112;

// from winuser.h / commctrl.h (not in the windows-sys modules we enable)
const SS_RIGHT: u32 = 0x0002;
const SS_OWNERDRAW: u32 = 0x000D;
const TBM_GETPOS: u32 = WM_USER;

const W: i32 = 540; // client size in 96-dpi pixels
const H: i32 = 452;

struct Dlg {
    settings: Settings,
    effects: Vec<String>,
    dpi: i32,
    font: HFONT,
    custom_colors: [u32; 16],
}

thread_local! {
    static DLG: RefCell<Option<Dlg>> = const { RefCell::new(None) };
}

fn s(v: i32, dpi: i32) -> i32 {
    v * dpi / 96
}

/// Activate comctl32 v6 for this thread (writes a tiny manifest to %TEMP%).
fn enable_visual_styles() {
    const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
      processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
</assembly>"#;
    let path = std::env::temp_dir().join("myscreensaver.manifest");
    if std::fs::write(&path, MANIFEST).is_err() {
        return;
    }
    let p = wide(&path.to_string_lossy());
    unsafe {
        let mut ctx: ACTCTXW = std::mem::zeroed();
        ctx.cbSize = std::mem::size_of::<ACTCTXW>() as u32;
        ctx.lpSource = p.as_ptr();
        let h = CreateActCtxW(&ctx);
        if h != INVALID_HANDLE_VALUE {
            let mut cookie = 0usize;
            ActivateActCtx(h, &mut cookie);
        }
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_BAR_CLASSES | ICC_STANDARD_CLASSES,
        };
        InitCommonControlsEx(&icc);
    }
}

unsafe fn item(hwnd: HWND, id: i32) -> HWND {
    GetDlgItem(hwnd, id)
}

unsafe fn control(parent: HWND, class: &str, text: &str, style: u32, ex: u32, x: i32, y: i32, w: i32, h: i32, id: i32, dpi: i32, font: HFONT) -> HWND {
    let c = wide(class);
    let t = wide(text);
    let hwnd = CreateWindowExW(
        ex, c.as_ptr(), t.as_ptr(), WS_CHILD | WS_VISIBLE | style,
        s(x, dpi), s(y, dpi), s(w, dpi), s(h, dpi),
        parent, id as isize as HMENU, GetModuleHandleW(null()), null(),
    );
    SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1);
    hwnd
}

unsafe fn get_text(hwnd: HWND) -> String {
    let n = GetWindowTextLengthW(hwnd);
    let mut buf = vec![0u16; n as usize + 1];
    GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    String::from_utf16_lossy(&buf[..n as usize])
}

unsafe fn set_checked(list: HWND, i: usize, on: bool) {
    let mut it: LVITEMW = std::mem::zeroed();
    it.stateMask = LVIS_STATEIMAGEMASK;
    it.state = (if on { 2u32 } else { 1u32 }) << 12;
    SendMessageW(list, LVM_SETITEMSTATE, i, &it as *const _ as LPARAM);
}

unsafe fn is_checked(list: HWND, i: usize) -> bool {
    let st = SendMessageW(list, LVM_GETITEMSTATE, i, LVIS_STATEIMAGEMASK as LPARAM) as u32;
    (st >> 12) == 2
}

fn speed_label(v: u32) -> String {
    format!("Animation speed: {v}%")
}

unsafe fn create_controls(hwnd: HWND) {
    let (dpi, font, settings, effects) = DLG.with(|d| {
        let d = d.borrow();
        let d = d.as_ref().unwrap();
        (d.dpi, d.font, d.settings.clone(), d.effects.clone())
    });
    let c = |class: &str, text: &str, style: u32, ex: u32, x, y, w, h, id| {
        control(hwnd, class, text, style, ex, x, y, w, h, id, dpi, font)
    };
    // ASCII art file
    c("STATIC", "ASCII art file (leave empty for the built-in art):", 0, 0, 12, 12, 516, 20, -1);
    c("EDIT", &settings.art_path, WS_TABSTOP | ES_AUTOHSCROLL as u32, WS_EX_CLIENTEDGE, 12, 34, 338, 26, ID_ART);
    c("BUTTON", "Browse...", WS_TABSTOP, 0, 356, 33, 84, 28, ID_BROWSE);
    c("BUTTON", "Built-in", WS_TABSTOP, 0, 446, 33, 82, 28, ID_DEFAULT_ART);

    // effects checklist
    c("STATIC", "Effects (a random one plays each round):", 0, 0, 12, 76, 260, 20, -1);
    let list = c(
        "SysListView32", "", WS_TABSTOP | LVS_REPORT | LVS_NOCOLUMNHEADER | LVS_SINGLESEL | LVS_SHOWSELALWAYS,
        WS_EX_CLIENTEDGE, 12, 98, 260, 290, ID_EFFECTS,
    );
    SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, (LVS_EX_CHECKBOXES | LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as LPARAM);
    let mut col: LVCOLUMNW = std::mem::zeroed();
    col.mask = LVCF_WIDTH;
    col.cx = s(230, dpi);
    SendMessageW(list, LVM_INSERTCOLUMNW, 0, &col as *const _ as LPARAM);
    for (i, name) in effects.iter().enumerate() {
        let text = wide(name);
        let mut it: LVITEMW = std::mem::zeroed();
        it.mask = LVIF_TEXT;
        it.iItem = i as i32;
        it.pszText = text.as_ptr() as *mut u16;
        SendMessageW(list, LVM_INSERTITEMW, 0, &it as *const _ as LPARAM);
        set_checked(list, i, !settings.disabled_effects.contains(name));
    }
    c("BUTTON", "Select all", WS_TABSTOP, 0, 12, 394, 126, 28, ID_ALL);
    c("BUTTON", "Select none", WS_TABSTOP, 0, 146, 394, 126, 28, ID_NONE);

    // theme
    c("STATIC", "Text color / theme:", 0, 0, 292, 76, 236, 20, -1);
    let combo = c("COMBOBOX", "", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32, 0, 292, 98, 236, 300, ID_THEME);
    for (name, _) in THEMES {
        let t = wide(name);
        SendMessageW(combo, CB_ADDSTRING, 0, t.as_ptr() as LPARAM);
    }
    SendMessageW(combo, CB_SETCURSEL, settings.theme as WPARAM, 0);
    c("BUTTON", "Pick custom color...", WS_TABSTOP, 0, 292, 134, 190, 28, ID_COLOR);
    c("STATIC", "", SS_OWNERDRAW, WS_EX_CLIENTEDGE, 490, 134, 38, 28, ID_SWATCH);

    // speed
    c("STATIC", &speed_label(settings.speed), 0, 0, 292, 184, 236, 20, ID_SPEED_LABEL);
    let tb = c("msctls_trackbar32", "", WS_TABSTOP | TBS_HORZ | TBS_AUTOTICKS, 0, 286, 206, 248, 34, ID_SPEED);
    SendMessageW(tb, TBM_SETRANGEMIN, 0, SPEED_MIN as LPARAM);
    SendMessageW(tb, TBM_SETRANGEMAX, 0, SPEED_MAX as LPARAM);
    SendMessageW(tb, TBM_SETTICFREQ, 25, 0);
    SendMessageW(tb, TBM_SETPAGESIZE, 0, 25);
    SendMessageW(tb, TBM_SETLINESIZE, 0, 5);
    SendMessageW(tb, TBM_SETPOS, 1, settings.speed as LPARAM);
    c("STATIC", "slower", 0, 0, 292, 242, 80, 18, -1);
    c("STATIC", "faster", SS_RIGHT, 0, 448, 242, 80, 18, -1);
    c(
        "STATIC",
        "100% matches the Omarchy screensaver's pace. Each effect ends by holding the finished art for 2-3 seconds.",
        0, 0, 292, 276, 236, 100, -1,
    );

    // bottom row
    c("BUTTON", "Preview", WS_TABSTOP, 0, 292, 394, 70, 28, ID_PREVIEW);
    c("BUTTON", "OK", WS_TABSTOP | BS_DEFPUSHBUTTON as u32, 0, 372, 394, 74, 28, IDOK);
    c("BUTTON", "Cancel", WS_TABSTOP, 0, 454, 394, 74, 28, IDCANCEL);
    update_color_button(hwnd);
}

unsafe fn update_color_button(hwnd: HWND) {
    let theme = SendMessageW(item(hwnd, ID_THEME), CB_GETCURSEL, 0, 0) as u32;
    EnableWindow(item(hwnd, ID_COLOR), (theme == CUSTOM_THEME) as BOOL);
    ShowWindow(item(hwnd, ID_SWATCH), if theme == CUSTOM_THEME { SW_SHOW } else { SW_HIDE });
    InvalidateRect(item(hwnd, ID_SWATCH), null(), TRUE);
}

/// Read the controls into a Settings value.
unsafe fn collect(hwnd: HWND) -> Settings {
    let (mut st, effects) = DLG.with(|d| {
        let d = d.borrow();
        let d = d.as_ref().unwrap();
        (d.settings.clone(), d.effects.clone())
    });
    st.art_path = get_text(item(hwnd, ID_ART)).trim().to_string();
    let list = item(hwnd, ID_EFFECTS);
    st.disabled_effects = effects.iter().enumerate().filter(|(i, _)| !is_checked(list, *i)).map(|(_, e)| e.clone()).collect();
    st.theme = SendMessageW(item(hwnd, ID_THEME), CB_GETCURSEL, 0, 0).max(0) as u32;
    st.speed = SendMessageW(item(hwnd, ID_SPEED), TBM_GETPOS, 0, 0) as u32;
    st
}

unsafe fn browse(hwnd: HWND) {
    let mut file = [0u16; 1024];
    let current = get_text(item(hwnd, ID_ART));
    let cur = wide(&current);
    if cur.len() < file.len() {
        file[..cur.len()].copy_from_slice(&cur);
    }
    let filter: Vec<u16> = "Text files (*.txt)\0*.txt\0All files (*.*)\0*.*\0\0".encode_utf16().collect();
    let title = wide("Choose ASCII art");
    let mut ofn: OPENFILENAMEW = std::mem::zeroed();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = hwnd;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = file.as_mut_ptr();
    ofn.nMaxFile = file.len() as u32;
    ofn.lpstrTitle = title.as_ptr();
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY;
    if GetOpenFileNameW(&mut ofn) != 0 {
        SetWindowTextW(item(hwnd, ID_ART), file.as_ptr());
    }
}

unsafe fn pick_color(hwnd: HWND) {
    let (current, mut custom) = DLG.with(|d| {
        let d = d.borrow();
        let d = d.as_ref().unwrap();
        (d.settings.custom_color, d.custom_colors)
    });
    let mut cc: CHOOSECOLORW = std::mem::zeroed();
    cc.lStructSize = std::mem::size_of::<CHOOSECOLORW>() as u32;
    cc.hwndOwner = hwnd;
    cc.rgbResult = current;
    cc.lpCustColors = custom.as_mut_ptr();
    cc.Flags = CC_RGBINIT | CC_FULLOPEN;
    if ChooseColorW(&mut cc) != 0 {
        DLG.with(|d| {
            if let Some(d) = d.borrow_mut().as_mut() {
                d.settings.custom_color = cc.rgbResult & 0x00FF_FFFF;
                d.custom_colors = custom;
            }
        });
        InvalidateRect(item(hwnd, ID_SWATCH), null(), TRUE);
    }
}

fn close(hwnd: HWND) {
    unsafe {
        DestroyWindow(hwnd);
        PostQuitMessage(0); // wakes the message loop below
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            create_controls(hwnd);
            0
        }
        WM_CTLCOLORSTATIC => {
            // labels on the window's background colour
            SetBkMode(wp as HDC, TRANSPARENT as i32);
            GetSysColorBrush(COLOR_WINDOW) as LRESULT
        }
        WM_DRAWITEM => {
            let di = &*(lp as *const DRAWITEMSTRUCT);
            if di.CtlID as i32 == ID_SWATCH {
                let color = DLG.with(|d| d.borrow().as_ref().map(|d| d.settings.custom_color).unwrap_or(0));
                let b = CreateSolidBrush(color);
                FillRect(di.hDC, &di.rcItem, b);
                DeleteObject(b);
            }
            1
        }
        WM_HSCROLL => {
            if lp as HWND == item(hwnd, ID_SPEED) {
                let v = SendMessageW(lp as HWND, TBM_GETPOS, 0, 0) as u32;
                let t = wide(&speed_label(v));
                SetWindowTextW(item(hwnd, ID_SPEED_LABEL), t.as_ptr());
            }
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            let code = ((wp >> 16) & 0xFFFF) as u32;
            match id {
                ID_BROWSE => browse(hwnd),
                ID_DEFAULT_ART => {
                    let e = wide("");
                    SetWindowTextW(item(hwnd, ID_ART), e.as_ptr());
                }
                ID_ALL | ID_NONE => {
                    let list = item(hwnd, ID_EFFECTS);
                    let n = SendMessageW(list, LVM_GETITEMCOUNT, 0, 0) as usize;
                    for i in 0..n {
                        set_checked(list, i, id == ID_ALL);
                    }
                }
                ID_THEME if code == CBN_SELCHANGE => update_color_button(hwnd),
                ID_COLOR => pick_color(hwnd),
                ID_PREVIEW => {
                    // Run the saver with the settings as currently shown,
                    // then put the saved ones back: only OK commits.
                    let st = collect(hwnd);
                    let prev = Settings::load();
                    st.save();
                    if let Ok(exe) = std::env::current_exe() {
                        if let Ok(mut child) = std::process::Command::new(exe).arg("/s").spawn() {
                            ShowWindow(hwnd, SW_HIDE);
                            let _ = child.wait();
                            ShowWindow(hwnd, SW_SHOW);
                            SetForegroundWindow(hwnd);
                        }
                    }
                    prev.save(); // Preview does not commit; OK does
                }
                IDOK => {
                    let st = collect(hwnd);
                    if !st.save() {
                        let t = wide("Could not save settings to the registry.");
                        let c = wide("MyScreensaver");
                        MessageBoxW(hwnd, t.as_ptr(), c.as_ptr(), MB_ICONERROR);
                    }
                    close(hwnd);
                }
                IDCANCEL => close(hwnd),
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            close(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn run(owner: Option<HWND>) {
    enable_visual_styles();
    let class = wide("MyScreensaverSettings");
    unsafe {
        let hinst = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: LoadIconW(null_mut(), IDI_APPLICATION),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);

        // DPI of the monitor the dialog will open on (primary, or the owner's)
        let dpi = match owner {
            Some(o) if GetDpiForWindow(o) > 0 => GetDpiForWindow(o) as i32,
            _ => {
                let dc = GetDC(null_mut());
                let d = GetDeviceCaps(dc, LOGPIXELSY as i32);
                ReleaseDC(null_mut(), dc);
                d.max(96)
            }
        };
        let face = wide("Segoe UI");
        let font = CreateFontW(
            -s(15, dpi), 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_SWISS) as u32, face.as_ptr(),
        );
        DLG.with(|d| {
            *d.borrow_mut() = Some(Dlg {
                settings: Settings::load(),
                effects: crate::fx::all_effects(),
                dpi,
                font,
                custom_colors: [0x00FFFFFF; 16],
            })
        });

        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut rc = RECT { left: 0, top: 0, right: s(W, dpi), bottom: s(H, dpi) };
        windows_sys::Win32::UI::HiDpi::AdjustWindowRectExForDpi(&mut rc, style, 0, WS_EX_DLGMODALFRAME, dpi as u32);
        let (ww, wh) = (rc.right - rc.left, rc.bottom - rc.top);
        let sx = (GetSystemMetrics(SM_CXSCREEN) - ww) / 2;
        let sy = (GetSystemMetrics(SM_CYSCREEN) - wh) / 2;
        let title = wide("MyScreensaver Settings");
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME, class.as_ptr(), title.as_ptr(), style,
            sx, sy, ww, wh, owner.unwrap_or(null_mut()), null_mut(), hinst, null(),
        );
        if let Some(o) = owner {
            EnableWindow(o, FALSE);
        }
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if let Some(o) = owner {
            EnableWindow(o, TRUE);
            SetForegroundWindow(o);
        }
        DeleteObject(font);
    }
}
