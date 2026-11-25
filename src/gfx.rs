//! A character-cell grid and its GDI renderer (one per window).

use windows_sys::Win32::Foundation::{HWND, RECT, SIZE};
use windows_sys::Win32::Graphics::Gdi::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    /// 0x00BBGGRR (COLORREF layout).
    pub color: u32,
}

impl Cell {
    pub const BLANK: Cell = Cell { ch: ' ', color: 0 };
}

#[derive(Clone)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
    /// Rows changed since the last draw.
    pub dirty: Vec<bool>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Grid { cols, rows, cells: vec![Cell::BLANK; cols * rows], dirty: vec![true; rows] }
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::BLANK);
        self.dirty.fill(true);
    }

    /// Set a cell; returns true if it changed.
    pub fn set(&mut self, col: usize, row: usize, cell: Cell) -> bool {
        if col < self.cols && row < self.rows {
            let slot = &mut self.cells[row * self.cols + col];
            if *slot != cell {
                *slot = cell;
                self.dirty[row] = true;
                return true;
            }
        }
        false
    }

    /// Plain text centred in the grid (used before an effect is running).
    pub fn put_centered(&mut self, text: &str, color: u32) {
        let (tc, tr) = crate::art::size(text);
        let x0 = self.cols.saturating_sub(tc) / 2;
        let y0 = self.rows.saturating_sub(tr) / 2;
        for (r, line) in text.lines().enumerate() {
            for (c, ch) in line.chars().enumerate() {
                if ch != ' ' {
                    self.set(x0 + c, y0 + r, Cell { ch, color });
                }
            }
        }
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Per-window renderer: font, cell metrics, grid geometry and a back buffer.
pub struct Surface {
    pub width: i32,
    pub height: i32,
    pub cell_w: i32,
    pub cell_h: i32,
    pub cols: usize,
    pub rows: usize,
    origin_x: i32,
    origin_y: i32,
    font: HFONT,
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    old_font: HGDIOBJ,
    dx: Vec<i32>,
    text: Vec<u16>,
}

/// Font sizing policy.
pub struct Fit {
    /// Max fraction of the window the art may cover.
    pub max_w: f32,
    pub max_h: f32,
    /// Keep at least this many rows of canvas (so full-screen effects have
    /// room to move, like a terminal). 0 = no minimum.
    pub min_rows: i32,
    pub min_px: i32,
}

fn make_font(face: &str, height: i32) -> HFONT {
    let mut lf: LOGFONTW = unsafe { std::mem::zeroed() };
    lf.lfHeight = -height.max(1); // negative = character height in px
    lf.lfWeight = FW_NORMAL as i32;
    lf.lfCharSet = DEFAULT_CHARSET as u8;
    lf.lfOutPrecision = OUT_TT_ONLY_PRECIS as u8;
    lf.lfQuality = CLEARTYPE_QUALITY as u8;
    lf.lfPitchAndFamily = (FIXED_PITCH | FF_MODERN) as u8;
    let w = wide(face);
    lf.lfFaceName[..w.len().min(32)].copy_from_slice(&w[..w.len().min(32)]);
    unsafe { CreateFontIndirectW(&lf) }
}

/// Cascadia Mono if installed, else Consolas.
fn pick_face(dc: HDC) -> &'static str {
    let probe = make_font("Cascadia Mono", 20);
    let old = unsafe { SelectObject(dc, probe) };
    let mut buf = [0u16; 64];
    let n = unsafe { GetTextFaceW(dc, buf.len() as i32, buf.as_mut_ptr()) };
    unsafe {
        SelectObject(dc, old);
        DeleteObject(probe);
    }
    let face = String::from_utf16_lossy(&buf[..n.max(1) as usize - 1]);
    if face.eq_ignore_ascii_case("Cascadia Mono") {
        "Cascadia Mono"
    } else {
        "Consolas"
    }
}

/// (cell width, cell height) of `font` in px.
fn cell_metrics(dc: HDC, font: HFONT) -> (i32, i32) {
    let old = unsafe { SelectObject(dc, font) };
    let mut tm: TEXTMETRICW = unsafe { std::mem::zeroed() };
    let mut sz = SIZE { cx: 0, cy: 0 };
    let m = wide("M");
    unsafe {
        GetTextMetricsW(dc, &mut tm);
        GetTextExtentPoint32W(dc, m.as_ptr(), 1, &mut sz);
        SelectObject(dc, old);
    }
    (sz.cx.max(1), tm.tmHeight.max(1))
}

impl Surface {
    /// Build a renderer for a `width`x`height` window that shows art of
    /// `art_cols`x`art_rows` cells.
    pub fn new(hwnd: HWND, width: i32, height: i32, art_cols: usize, art_rows: usize, fit: &Fit) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        unsafe {
            let wdc = GetDC(hwnd);
            let mem_dc = CreateCompatibleDC(wdc);
            let bitmap = CreateCompatibleBitmap(wdc, width, height);
            ReleaseDC(hwnd, wdc);
            let old_bitmap = SelectObject(mem_dc, bitmap);
            let all = RECT { left: 0, top: 0, right: width, bottom: height };
            FillRect(mem_dc, &all, GetStockObject(BLACK_BRUSH) as HBRUSH);

            // Measure a 100px reference font, then scale to fit.
            let face = pick_face(mem_dc);
            let reference = make_font(face, 100);
            let (rw, rh) = cell_metrics(mem_dc, reference);
            DeleteObject(reference);
            let (rw, rh) = (rw as f32 / 100.0, rh as f32 / 100.0); // px per px of height
            let mut h = f32::min(
                fit.max_w * width as f32 / (art_cols as f32 * rw),
                fit.max_h * height as f32 / (art_rows as f32 * rh),
            );
            if fit.min_rows > 0 {
                h = h.min(height as f32 / (fit.min_rows as f32 * rh));
            }
            let mut px = (h.floor() as i32).max(fit.min_px);
            let (mut font, mut cw, mut ch);
            loop {
                font = make_font(face, px);
                (cw, ch) = cell_metrics(mem_dc, font);
                let fits = (art_cols as i32) * cw <= width && (art_rows as i32) * ch <= height;
                if fits || px <= fit.min_px {
                    break;
                }
                DeleteObject(font);
                px -= 1;
            }
            let old_font = SelectObject(mem_dc, font);
            SetBkMode(mem_dc, TRANSPARENT as i32);
            SetTextAlign(mem_dc, TA_TOP | TA_LEFT);

            let cols = ((width / cw) as usize).max(art_cols.min(1));
            let rows = ((height / ch) as usize).max(1);
            let origin_x = (width - cols as i32 * cw) / 2;
            let origin_y = (height - rows as i32 * ch) / 2;
            Surface {
                width,
                height,
                cell_w: cw,
                cell_h: ch,
                cols,
                rows,
                origin_x,
                origin_y,
                font,
                mem_dc,
                bitmap,
                old_bitmap,
                old_font,
                dx: Vec::new(),
                text: Vec::new(),
            }
        }
    }

    /// Redraw the grid's dirty rows into the back buffer and clear their
    /// flags. Returns the pixel band (top, bottom) that changed, if any.
    ///
    /// Block elements (█ ▀ ▄ ▌ ▐ ...) are filled rectangles so they tile
    /// without seams, as terminals draw them. Other cells are drawn as text in
    /// runs of one colour, with explicit advances so every glyph lands on its
    /// cell, clipped to its row so redrawing one row never smears another.
    pub fn draw(&mut self, grid: &mut Grid) -> Option<(i32, i32)> {
        let mut band: Option<(i32, i32)> = None;
        unsafe {
            let dc_brush = GetStockObject(DC_BRUSH) as HBRUSH;
            let black = GetStockObject(BLACK_BRUSH) as HBRUSH;
            for r in 0..grid.rows.min(self.rows) {
                if !std::mem::take(&mut grid.dirty[r]) {
                    continue;
                }
                let y = self.origin_y + r as i32 * self.cell_h;
                let row_rc = RECT { left: 0, top: y, right: self.width, bottom: y + self.cell_h };
                FillRect(self.mem_dc, &row_rc, black);
                band = Some(match band {
                    None => (y, y + self.cell_h),
                    Some((t, _)) => (t, y + self.cell_h),
                });
                let row = &grid.cells[r * grid.cols..(r + 1) * grid.cols];
                let end = row.len().min(self.cols);
                let mut c = 0;
                while c < end {
                    if row[c].ch == ' ' {
                        c += 1;
                        continue;
                    }
                    if let Some(b) = block_rect(row[c].ch, self.cell_w, self.cell_h) {
                        let x = self.origin_x + c as i32 * self.cell_w;
                        let rc = RECT { left: x + b.left, top: y + b.top, right: x + b.right, bottom: y + b.bottom };
                        SetDCBrushColor(self.mem_dc, row[c].color);
                        FillRect(self.mem_dc, &rc, dc_brush);
                        c += 1;
                        continue;
                    }
                    let color = row[c].color;
                    let start = c;
                    self.text.clear();
                    self.dx.clear();
                    while c < end
                        && (row[c].ch == ' ' || row[c].color == color)
                        && block_rect(row[c].ch, 1, 1).is_none()
                    {
                        let mut buf = [0u16; 2];
                        let units = row[c].ch.encode_utf16(&mut buf);
                        for (i, u) in units.iter().enumerate() {
                            self.text.push(*u);
                            // a surrogate pair advances once, on its second unit
                            self.dx.push(if i + 1 == units.len() { self.cell_w } else { 0 });
                        }
                        c += 1;
                    }
                    while self.text.last() == Some(&(b' ' as u16)) {
                        self.text.pop();
                        self.dx.pop();
                    }
                    SetTextColor(self.mem_dc, color);
                    ExtTextOutW(
                        self.mem_dc,
                        self.origin_x + start as i32 * self.cell_w,
                        y,
                        ETO_CLIPPED,
                        &row_rc,
                        self.text.as_ptr(),
                        self.text.len() as u32,
                        self.dx.as_ptr(),
                    );
                }
            }
        }
        band
    }

    /// Copy the whole back buffer to the window.
    pub fn present(&self, dc: HDC) {
        self.present_band(dc, 0, self.height);
    }

    /// Copy pixel rows [top, bottom) of the back buffer to the window.
    pub fn present_band(&self, dc: HDC, top: i32, bottom: i32) {
        unsafe {
            BitBlt(dc, 0, top, self.width, bottom - top, self.mem_dc, 0, top, SRCCOPY);
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem_dc, self.old_font);
            SelectObject(self.mem_dc, self.old_bitmap);
            DeleteObject(self.font);
            DeleteObject(self.bitmap);
            DeleteDC(self.mem_dc);
        }
    }
}

/// The filled part of a cell for block-element characters, else None.
fn block_rect(ch: char, w: i32, h: i32) -> Option<RECT> {
    let r = |l, t, rr, b| Some(RECT { left: l, top: t, right: rr, bottom: b });
    match ch {
        '█' => r(0, 0, w, h),
        '▀' => r(0, 0, w, h / 2),
        '▔' => r(0, 0, w, (h / 8).max(1)),
        '▐' => r(w / 2, 0, w, h),
        '▕' => r(w - (w / 8).max(1), 0, w, h),
        // lower n/8 blocks ▁▂▃▄▅▆▇
        '\u{2581}'..='\u{2587}' => {
            let n = ch as i32 - 0x2580;
            r(0, h - h * n / 8, w, h)
        }
        // left n/8 blocks ▉▊▋▌▍▎▏ (7/8 down to 1/8)
        '\u{2589}'..='\u{258F}' => {
            let n = 8 - (ch as i32 - 0x2588);
            r(0, 0, (w * n / 8).max(1), h)
        }
        _ => None,
    }
}
