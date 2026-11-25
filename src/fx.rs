//! The effect loop: run a random ttfx effect over the art, hold the final
//! frame for 2-3 s, then the next effect (never the same one twice in a row).
//!
//! ttfx (MIT, https://github.com/omacom/ttfx — a port of TerminalTextEffects,
//! MIT, https://github.com/ChrisBuilds/terminaltexteffects) does all effect
//! work. Each step we ask its renderer for the frame as text with 24-bit SGR
//! colour codes and decode that into our own cell grid.

use std::time::{Duration, Instant};

use ttfx::engine::canvas::Anchor;
use ttfx::engine::ctx::Clock;
use ttfx::engine::terminal::TerminalConfig;
use ttfx::fx::run::Effect;
use ttfx::fx::Engine;
use ttfx::utils::rng::Rng;

use crate::gfx::{Cell, Grid};

/// Effect frame rate the speed setting is relative to. Omarchy runs tte at
/// --frame-rate 120, so 100 % speed = 120 effect frames per second.
const BASE_EFFECT_FPS: f32 = 120.0;
pub const DISPLAY_FPS: f32 = 60.0;

/// A random seed. (ttfx's Rng::from_entropy reads /dev/urandom, which
/// Windows does not have; std's RandomState is seeded from the OS RNG.)
fn entropy_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    h.finish()
}

/// Every effect ttfx provides (its CLI subcommands).
pub fn all_effects() -> Vec<String> {
    use clap::CommandFactory;
    let mut v: Vec<String> = ttfx::cli::Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    v.sort();
    v
}

fn build_effect(name: &str) -> Option<Box<dyn Effect>> {
    let cli = <ttfx::cli::Cli as clap::Parser>::try_parse_from(["ttfx", name]).ok()?;
    ttfx::fx::effects::build(cli.effect.as_ref()?)
}

/// Maps an effect's colour (or None = terminal default foreground) to the
/// colour drawn on screen.
pub trait Palette {
    fn map(&self, rgb: Option<(u8, u8, u8)>) -> u32;
}

pub struct Plain(pub u32);
impl Palette for Plain {
    fn map(&self, rgb: Option<(u8, u8, u8)>) -> u32 {
        match rgb {
            Some((r, g, b)) => r as u32 | (g as u32) << 8 | (b as u32) << 16,
            None => self.0,
        }
    }
}

enum State {
    Playing { engine: Box<Engine>, effect: Box<dyn Effect>, acc: f32 },
    Holding { until: Instant },
}

pub struct Animator {
    art: String,
    effects: Vec<String>,
    last: Option<String>,
    rng: Rng,
    steps_per_tick: f32,
    state: State,
    pub grid: Grid,
    palette: Box<dyn Palette>,
    buf: Vec<u8>,
}

impl Animator {
    pub fn new(
        art: String,
        cols: usize,
        rows: usize,
        effects: Vec<String>,
        speed_percent: u32,
        palette: Box<dyn Palette>,
    ) -> Self {
        let effects = if effects.is_empty() { all_effects() } else { effects };
        let mut a = Animator {
            art,
            effects,
            last: None,
            rng: Rng::seeded(entropy_seed()),
            steps_per_tick: BASE_EFFECT_FPS * speed_percent.clamp(10, 1000) as f32 / 100.0 / DISPLAY_FPS,
            state: State::Holding { until: Instant::now() },
            grid: Grid::new(cols, rows),
            palette,
            buf: Vec::new(),
        };
        a.start_next();
        a
    }

    /// When this animator next needs a tick: None = next display frame.
    pub fn wake_at(&self) -> Option<Instant> {
        match &self.state {
            State::Playing { .. } => None,
            State::Holding { until } => Some(*until),
        }
    }

    fn pick(&mut self) -> String {
        let candidates: Vec<&String> = if self.effects.len() > 1 {
            self.effects.iter().filter(|e| Some(*e) != self.last.as_ref()).collect()
        } else {
            self.effects.iter().collect()
        };
        candidates[self.rng.choice_index(candidates.len())].clone()
    }

    fn start_next(&mut self) {
        // a few tries, in case an effect refuses this canvas
        for _ in 0..8 {
            let name = self.pick();
            self.last = Some(name.clone());
            let Some(mut effect) = build_effect(&name) else { continue };
            let config = TerminalConfig {
                canvas_width: self.grid.cols as i64,
                canvas_height: self.grid.rows as i64,
                anchor_canvas: Anchor::parse("c").unwrap(),
                anchor_text: Anchor::parse("c").unwrap(),
                ignore_terminal_dimensions: true,
                frame_rate: BASE_EFFECT_FPS as i64,
                ..TerminalConfig::default()
            };
            let rng = Rng::seeded(self.rng.choice_index(u32::MAX as usize) as u64);
            let clock = Clock::virtual_with_frame_rate(BASE_EFFECT_FPS as i64);
            let Ok(mut engine) = Engine::new(&self.art, config, rng, clock) else { continue };
            if effect.build(&mut engine).is_err() {
                continue;
            }
            self.state = State::Playing { engine: Box::new(engine), effect, acc: 0.0 };
            return;
        }
        // nothing would start: show the static art and try again later
        self.grid.clear();
        self.grid.put_centered(&self.art, self.palette.map(None));
        self.state = State::Holding { until: Instant::now() + Duration::from_secs(3) };
    }

    /// Advance by one display frame. Returns true if the grid changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        match &mut self.state {
            State::Holding { until } => {
                if now >= *until {
                    self.start_next();
                }
                false
            }
            State::Playing { engine, effect, acc } => {
                *acc += self.steps_per_tick;
                let mut stepped = false;
                let mut finished = false;
                while *acc >= 1.0 {
                    *acc -= 1.0;
                    if !effect.next_frame(engine) {
                        finished = true;
                        break;
                    }
                    engine.clock.advance_frame();
                    engine.render_here();
                    stepped = true;
                }
                let changed = stepped && decode_frame(engine, &mut self.buf, &mut self.grid, &*self.palette);
                if finished {
                    let hold = 2000 + self.rng.choice_index(1001) as u64; // 2.0 - 3.0 s
                    self.state = State::Holding { until: now + Duration::from_millis(hold) };
                }
                changed
            }
        }
    }
}

/// Decode the engine's current frame (rows of text + SGR codes, top row
/// first) into `grid`. Returns true if any cell changed.
fn decode_frame(
    engine: &Engine,
    buf: &mut Vec<u8>,
    grid: &mut Grid,
    palette: &dyn Palette,
) -> bool {
    buf.clear();
    let mut parts = Vec::with_capacity(grid.rows + 2);
    engine.renderer().frame_parts(b"", b"", &mut parts);
    for s in &parts {
        buf.extend_from_slice(s);
    }
    let text = String::from_utf8_lossy(buf);
    let mut changed = false;
    let (mut row, mut col) = (0usize, 0usize);
    let mut fg: Option<(u8, u8, u8)> = None;
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\n' => {
                while col < grid.cols {
                    changed |= grid.set(col, row, Cell::BLANK);
                    col += 1;
                }
                row += 1;
                col = 0;
            }
            '\x1b' => {
                if it.peek() != Some(&'[') {
                    continue;
                }
                it.next();
                let mut params = String::new();
                let mut fin = 'm';
                for d in it.by_ref() {
                    if d.is_ascii_digit() || d == ';' {
                        params.push(d);
                    } else {
                        fin = d;
                        break;
                    }
                }
                if fin == 'm' {
                    apply_sgr(&params, &mut fg);
                }
            }
            _ => {
                let cell = if c == ' ' { Cell::BLANK } else { Cell { ch: c, color: palette.map(fg) } };
                changed |= grid.set(col, row, cell);
                col += 1;
            }
        }
    }
    // blank whatever the frame did not cover
    while row < grid.rows {
        while col < grid.cols {
            changed |= grid.set(col, row, Cell::BLANK);
            col += 1;
        }
        row += 1;
        col = 0;
    }
    changed
}

fn apply_sgr(params: &str, fg: &mut Option<(u8, u8, u8)>) {
    let v: Vec<u32> = params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
    let mut i = 0;
    while i < v.len() {
        match v[i] {
            0 | 39 => *fg = None,
            38 if v.get(i + 1) == Some(&2) && i + 4 < v.len() => {
                *fg = Some((v[i + 2] as u8, v[i + 3] as u8, v[i + 4] as u8));
                i += 4;
            }
            38 | 48 if v.get(i + 1) == Some(&5) => i += 2,
            48 if v.get(i + 1) == Some(&2) => i += 4,
            _ => {}
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every ttfx effect builds, runs to completion through our decoder on a
    /// screen-sized canvas, and ends with the art visible.
    #[test]
    fn every_effect_runs() {
        let art = crate::art::load("");
        let mut report = String::new();
        for name in all_effects() {
            let t = Instant::now();
            let mut a = Animator::new(art.clone(), 120, 40, vec![name.clone()], 100, Box::new(Plain(0xE0E0E0)));
            let mut ticks = 0u32;
            let mut changed = 0u32;
            while a.wake_at().is_none() {
                if a.tick(Instant::now()) {
                    changed += 1;
                }
                ticks += 1;
                assert!(ticks < 60 * 120, "{name} ran over 2 minutes of frames");
            }
            let visible = a.grid.cells.iter().filter(|c| c.ch != ' ').count();
            report += &format!(
                "{name:16} ticks={ticks:5} (~{:5.1}s at 60fps) changed={changed:5} visible={visible:4} cpu={:?}\n",
                ticks as f32 / 60.0,
                t.elapsed()
            );
        }
        println!("{report}");
    }
}
