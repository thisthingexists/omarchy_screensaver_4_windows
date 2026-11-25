//! Settings, stored under HKCU\Software\MyScreensaver.
//!
//!   ArtPath         REG_SZ     custom ASCII art file ("" = built-in art)
//!   DisabledEffects REG_SZ     comma-separated effect names that are off
//!                              (stored as "off" so effects added later default to on)
//!   Theme           REG_DWORD  index into THEMES
//!   CustomColor     REG_DWORD  0x00BBGGRR, used by the "Custom color" theme
//!   Speed           REG_DWORD  animation speed in percent (100 = Omarchy's pace)

use std::ptr::{null, null_mut};

use windows_sys::Win32::System::Registry::*;

use crate::fx::Palette;
use crate::gfx::wide;

pub const REG_PATH: &str = "Software\\MyScreensaver";
pub const SPEED_MIN: u32 = 25;
pub const SPEED_MAX: u32 = 300;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub art_path: String,
    pub disabled_effects: Vec<String>,
    pub theme: u32,
    pub custom_color: u32,
    pub speed: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            art_path: String::new(),
            disabled_effects: Vec::new(),
            theme: 0,
            custom_color: 0x00F7A27A, // soft blue (RGB 7A A2 F7)
            speed: 100,
        }
    }
}

/// (name, base colour). `None` base = keep the effect's own colours.
pub const THEMES: &[(&str, Option<u32>)] = &[
    ("Effect colors (original)", None),
    ("White", Some(0x00F0F0F0)),
    ("Green phosphor", Some(0x0066FF33)),
    ("Amber", Some(0x0000B0FF)),
    ("Cyan", Some(0x00FFE000)),
    ("Purple", Some(0x00F070C0)),
    ("Custom color", None),
];
pub const CUSTOM_THEME: u32 = THEMES.len() as u32 - 1;

impl Settings {
    /// The effects to rotate through (all effects if every one is disabled).
    pub fn enabled_effects(&self) -> Vec<String> {
        let all = crate::fx::all_effects();
        let on: Vec<String> = all.iter().filter(|e| !self.disabled_effects.contains(e)).cloned().collect();
        if on.is_empty() {
            all
        } else {
            on
        }
    }

    pub fn palette(&self) -> Box<dyn Palette> {
        let base = if self.theme == CUSTOM_THEME {
            Some(self.custom_color)
        } else {
            THEMES.get(self.theme as usize).and_then(|t| t.1)
        };
        match base {
            None => Box::new(crate::fx::Plain(0x00E0E0E0)),
            Some(c) => Box::new(Tint(c)),
        }
    }

    pub fn load() -> Settings {
        let d = Settings::default();
        let Some(key) = Key::open(false) else { return d };
        Settings {
            art_path: key.get_str("ArtPath").unwrap_or(d.art_path),
            disabled_effects: key
                .get_str("DisabledEffects")
                .map(|s| s.split(',').map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect())
                .unwrap_or_default(),
            theme: key.get_u32("Theme").filter(|t| (*t as usize) < THEMES.len()).unwrap_or(d.theme),
            custom_color: key.get_u32("CustomColor").unwrap_or(d.custom_color) & 0x00FF_FFFF,
            speed: key.get_u32("Speed").unwrap_or(d.speed).clamp(SPEED_MIN, SPEED_MAX),
        }
    }

    pub fn save(&self) -> bool {
        let Some(key) = Key::open(true) else { return false };
        key.set_str("ArtPath", &self.art_path)
            && key.set_str("DisabledEffects", &self.disabled_effects.join(","))
            && key.set_u32("Theme", self.theme)
            && key.set_u32("CustomColor", self.custom_color)
            && key.set_u32("Speed", self.speed)
    }
}

/// Monochrome theme: each colour becomes the theme colour scaled by its
/// brightness, so effect gradients keep their shape in one hue.
struct Tint(u32);

impl Palette for Tint {
    fn map(&self, rgb: Option<(u8, u8, u8)>) -> u32 {
        let level = match rgb {
            None => 1.0,
            Some((r, g, b)) => {
                let l = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0;
                (l * 1.6).clamp(0.0, 1.0).powf(0.8)
            }
        };
        let ch = |shift: u32| (((self.0 >> shift) & 0xFF) as f32 * level).round() as u32;
        ch(0) | ch(8) << 8 | ch(16) << 16
    }
}

struct Key(HKEY);

impl Key {
    fn open(write: bool) -> Option<Key> {
        let path = wide(REG_PATH);
        let mut hkey: HKEY = null_mut();
        let rc = unsafe {
            if write {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER, path.as_ptr(), 0, null(), REG_OPTION_NON_VOLATILE,
                    KEY_READ | KEY_WRITE, null(), &mut hkey, null_mut(),
                )
            } else {
                RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_READ, &mut hkey)
            }
        };
        (rc == 0).then_some(Key(hkey))
    }

    fn get_str(&self, name: &str) -> Option<String> {
        let n = wide(name);
        let mut ty = 0u32;
        let mut len = 0u32;
        unsafe {
            if RegQueryValueExW(self.0, n.as_ptr(), null(), &mut ty, null_mut(), &mut len) != 0 || ty != REG_SZ {
                return None;
            }
            let mut buf = vec![0u16; len as usize / 2 + 1];
            if RegQueryValueExW(self.0, n.as_ptr(), null(), &mut ty, buf.as_mut_ptr() as *mut u8, &mut len) != 0 {
                return None;
            }
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..end]))
        }
    }

    fn get_u32(&self, name: &str) -> Option<u32> {
        let n = wide(name);
        let mut ty = 0u32;
        let mut v = 0u32;
        let mut len = 4u32;
        let rc = unsafe { RegQueryValueExW(self.0, n.as_ptr(), null(), &mut ty, &mut v as *mut u32 as *mut u8, &mut len) };
        (rc == 0 && ty == REG_DWORD).then_some(v)
    }

    fn set_str(&self, name: &str, value: &str) -> bool {
        let n = wide(name);
        let v = wide(value);
        unsafe { RegSetValueExW(self.0, n.as_ptr(), 0, REG_SZ, v.as_ptr() as *const u8, (v.len() * 2) as u32) == 0 }
    }

    fn set_u32(&self, name: &str, value: u32) -> bool {
        let n = wide(name);
        unsafe { RegSetValueExW(self.0, n.as_ptr(), 0, REG_DWORD, &value as *const u32 as *const u8, 4) == 0 }
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}
