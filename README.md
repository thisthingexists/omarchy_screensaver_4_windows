# MyScreensaver

A native Windows 11 screensaver in the style of the Omarchy Linux screensaver.
Each monitor turns black and your ASCII art plays through a random terminal
text effect: decrypt, beams, matrix, burn, scattered and 32 more. When an
effect finishes, the art holds for 2–3 seconds and the next effect starts.
Any key, click or real mouse movement closes it.

It is a single `MyScreensaver.scr` file (about 2.7 MB) with no runtime
dependencies.

## How it works

| Omarchy (Linux) | MyScreensaver (Windows) |
|---|---|
| `hypridle` starts `omarchy-launch-screensaver` | Windows starts `MyScreensaver.scr /s` |
| one fullscreen terminal per monitor | one borderless topmost window per monitor, drawn with GDI |
| `tte -i screensaver.txt --random-effect --canvas-width 0 --canvas-height 0 --anchor-canvas c --anchor-text c --frame-rate 120`, in a loop | the same effects from **ttfx** (a Rust port of tte) run in-process on a canvas the size of the monitor, with the text anchored in the centre |
| the terminal's font | Cascadia Mono (Consolas if Cascadia isn't installed), sized so the art fits |

The effects are not re-implemented. [ttfx](https://github.com/omacom/ttfx)
ships a library target, so `MyScreensaver` drives its effect engine one frame
at a time (`src/fx.rs`):

1. It asks ttfx for the frame as text with 24-bit colour codes.
2. It decodes that into a grid of coloured cells.
3. It draws the grid in its own window (`src/gfx.rs`). No console window is
   opened.

Block characters (`█ ▀ ▄ ▌ ▐`) are drawn as solid rectangles, as terminals do,
so block-letter art has no seams.

### CPU use
- Frames are capped at 60 fps. At 100% speed the effect advances 2 steps per
  frame, which matches Omarchy's `--frame-rate 120`.
- Only rows that changed are redrawn and copied to the screen. Frames that
  didn't change aren't drawn at all.
- While the finished art is held, the process sleeps until the hold ends or
  input arrives.
- Measured on a 1920×1080 screen: about 6–8% of one CPU core while
  animating, and close to zero while holding.

## Build

You need Rust (https://rustup.rs). Either Windows toolchain works:
- **`x86_64-pc-windows-gnu`**: self-contained, so you don't need Visual Studio.
  Install it with `rustup-init.exe --default-host x86_64-pc-windows-gnu`.
- **`x86_64-pc-windows-msvc`**: needs the Visual Studio Build Tools (C++ workload).

Then build:

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1          # -> dist\MyScreensaver.scr
powershell -ExecutionPolicy Bypass -File .\build.ps1 -Test    # also runs every effect headless first
```

The first build downloads ttfx from GitHub. It's pinned to a commit in
`Cargo.toml`.

> **Smart App Control:** Windows 11's Smart App Control blocks unsigned
> executables it hasn't seen before. That includes the helper programs Cargo
> builds during compilation, and the `.scr` itself. If the build fails with
> *"An Application Control policy has blocked this file"*, or the screensaver
> won't start, turn Smart App Control off: **Windows Security → App & browser
> control → Smart App Control**. Alternatively, sign the `.scr` with a trusted
> code-signing certificate.

## Install

1. Install it in one of two ways:
   - Right-click `dist\MyScreensaver.scr` and choose **Install**. This opens the
     Screen Saver Settings dialog with it selected.
   - Or copy it to `C:\Windows\System32` (needs admin), then open the dialog
     yourself.
2. Open **Settings → Personalization → Lock screen → Screen saver**, pick
   **MyScreensaver**, set the wait time and click **OK**.
3. Use **Settings...** in that dialog to configure it, and **Preview** to try it.

To uninstall, choose another screensaver (or **(None)**) and delete the `.scr`.
If you want to remove the settings too, delete `HKCU\Software\MyScreensaver`.

## Command line

| Argument | What it does |
|---|---|
| `/s` | Run full screen on every monitor |
| `/p <HWND>` | Draw the small preview inside the Screen Saver Settings dialog |
| `/c` or `/c:<HWND>` | Open the settings dialog |
| *(none)* | Open the settings dialog |

`-s`, `/S` and `/p:<HWND>` work too.

## Settings

The settings are saved in `HKCU\Software\MyScreensaver`.

| Setting | Registry value | Notes |
|---|---|---|
| ASCII art file | `ArtPath` (REG_SZ) | Any UTF-8 `.txt`. Leave it empty to use the built-in art (`assets/screensaver.txt`). |
| Enabled effects | `DisabledEffects` (REG_SZ, comma-separated) | The list stores the effects that are *off*, so effects added later start enabled. If you turn every effect off, all of them are used. |
| Theme | `Theme` (REG_DWORD) | 0 = the effect's own colours (like Omarchy). 1–5 = White, Green phosphor, Amber, Cyan, Purple. 6 = Custom color. A theme tints every effect in one hue and keeps its brightness. |
| Custom color | `CustomColor` (REG_DWORD, 0x00BBGGRR) | Used by theme 6. |
| Animation speed | `Speed` (REG_DWORD, 25–300) | A percentage. 100 = Omarchy's pace. |

To make your own art, any text works; block-letter generators such as
`figlet`, or the "ANSI Shadow" font on patorjk.com, give the Omarchy look.
Lines are trimmed on the right and tabs become 4 spaces.

## Behaviour details

- **Exit:**
  - Any key press, mouse button or wheel closes it.
  - So does the mouse moving more than 8 px from where it was at start;
    smaller movements are treated as jitter and ignored.
  - Input in the first 300 ms is ignored, because Windows can send stray
    events while the screensaver starts.
  - Input is caught by low-level keyboard and mouse hooks as well as window
    messages. That way it works even if the screensaver window didn't get
    keyboard focus.
- **Multiple monitors and DPI:**
  - The process is per-monitor DPI aware (v2), so every window works in
    physical pixels.
  - Each monitor gets its own window, font size and canvas, and runs its own
    random effect, as Omarchy runs one terminal per monitor.
- **No repeats:** the same effect never plays twice in a row, unless only one
  effect is enabled.
- **Crash log:** if the screensaver ever panics, it writes
  `%TEMP%\myscreensaver-panic.log`.
- **Debug log:** set `MYSCREENSAVER_DEBUG=1` and it records why it exited in
  `%TEMP%\myscreensaver-debug.log`.

## Source layout

| File | Purpose |
|---|---|
| `src/main.rs` | argument parsing, DPI awareness, panic log |
| `src/saver.rs` | fullscreen and preview windows, input handling, the 60 fps loop |
| `src/fx.rs` | effect rotation and hold timing, driving ttfx, decoding frames into cells; `cargo test --release` runs all 37 effects |
| `src/gfx.rs` | cell grid, font fitting, GDI back buffer, dirty-row drawing |
| `src/settings.rs` | registry settings and themes |
| `src/config.rs` | settings dialog |
| `src/art.rs` | loading and normalising the art |

## Licenses

This project is MIT-licensed (`LICENSE`). The effects come from ttfx
(MIT, © 37signals / omacom-io), which is a port of TerminalTextEffects
(MIT, © ChrisBuilds). Their notices are in `THIRD_PARTY_NOTICES.md`, and the
build copies that file next to the `.scr`.
