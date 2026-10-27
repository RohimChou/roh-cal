//! The intro played the very first time roh-cal starts (and with every `--sample`): a calm starry night over
//! the whole screen, where the big `roh-cal` logo drops in letter by letter, bounces once a little and
//! kicks up a little dust; then a color wave runs along it, the tagline is typed out and today's date fades
//! in, and near the end a white glint sweeps across the logo once. Stars twinkle slowly (three of
//! them in color) and a shooting star crosses the sky.
//! Everything is worked out from how long the intro has been playing (`seconds`), so the same moment always
//! looks the same

use crate::app::App;
use crate::theme::{Theme, mix};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Clear,
};
use std::f64::consts::TAU;

/// `roh-cal` in big ASCII letters (figlet's Standard font, a little apart): one glyph per letter, 5 rows each
const LOGO: [[&str; 5]; 7] = [
    ["      ", r" _ __ ", r"| '__|", r"| |   ", r"|_|   "],
    ["       ", r"  ___  ", r" / _ \ ", r"| (_) |", r" \___/ "],
    [r" _     ", r"| |__  ", r"| '_ \ ", r"| | | |", r"|_| |_|"],
    ["      ", "      ", " ____ ", "|____|", "      "],
    ["      ", r"  ___ ", r" / __|", r"| (__ ", r" \___|"],
    ["       ", r"  __ _ ", r" / _` |", r"| (_| |", r" \__,_|"],
    [r" _ ", r"| |", r"| |", r"| |", r"|_|"],
];

/// How wide the logo is
const LOGO_WIDTH: i32 = 42;

/// How tall the logo is
const LOGO_HEIGHT: i32 = 5;

/// How long a letter takes to fall to where it lands, in seconds
const FALL: f64 = 0.53;

/// How long a letter's one small bounce takes, up and back down, in seconds
const BOUNCE: f64 = 0.25;

/// When every letter has landed and stopped, in seconds: the last one starts falling at most
/// 0.008 × 41 + 0.12 = 0.45 s in, then takes `FALL` and `BOUNCE` (1.23 s). The tagline waits for it
const SETTLED: f64 = 1.4;

/// When the white glint sweeps across the logo, in seconds: once, after the date has faded in and well before
/// the intro ends (it plays 5.5 seconds, `app::SPLASH_FOR`), so the parts of the intro come one after another
const GLINT_AT: f64 = 3.6;

/// How fast the tagline is typed, in characters a second
const TYPE_SPEED: f64 = 45.0;

/// How fast a shooting star flies, in cells a second
const METEOR_SPEED: f64 = 35.0;

/// How long a shooting star flies, in seconds (about 75 cells at `METEOR_SPEED`)
const METEOR_FOR: f64 = 2.2;

/// How long a shooting star's tail is, in seconds of its flight (about 14 cells at `METEOR_SPEED`)
const METEOR_TAIL: f64 = 0.4;

/// Draws the intro over the whole screen, `seconds` after it started
pub fn render(frame: &mut Frame, app: &App, seconds: f64) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    let buf = frame.buffer_mut();
    let theme = &app.theme;
    let texts = app.lang.texts();
    // The logo, the tagline and the date in the middle, with a blank row between each
    let left = centered(area, LOGO_WIDTH);
    let top = i32::from(area.y) + (i32::from(area.height) - LOGO_HEIGHT - 4) / 2;
    let tagline = texts.splash_tagline;
    // The tagline is centered with room for the cursor after it
    let tagline_width = Span::raw(tagline).width() as i32 + 1;
    // No stars around the logo and the lines under it (the tagline may be wider than the logo)
    let block = LOGO_WIDTH.max(tagline_width);
    let quiet = rect(
        centered(area, block) - 3,
        top - 1,
        block + 6,
        LOGO_HEIGHT + 6,
    );
    stars(buf, area, theme, seconds, quiet);
    shooting_star(buf, area, theme, seconds, quiet);
    // In the bottom-left corner, in gray: any key skips it (not on a screen so short that the bottom row is
    // part of the middle block)
    let bottom = i32::from(area.bottom()) - 1;
    if bottom >= i32::from(quiet.bottom()) {
        let hint = Line::styled(texts.splash_skip, Style::new().fg(theme.muted));
        put_line(buf, area, i32::from(area.x) + 1, bottom, &hint);
    }
    // The letters, each falling from above the screen on its own (a little after the one to its left) and
    // bouncing once
    let palette = [
        theme.focus,
        theme.label,
        theme.today,
        theme.weekend,
        theme.priority_medium,
    ];
    for (row, text) in logo_rows().iter().enumerate() {
        let row = row as i32;
        for (column, symbol) in text.chars().enumerate() {
            if symbol == ' ' {
                continue;
            }
            let column = column as i32;
            let delay = 0.008 * f64::from(column) + 0.12 * noise(column as u32, row as u32);
            let since = seconds - delay;
            if since <= 0.0 {
                continue;
            }
            let target = top + row;
            let drop = f64::from(target - i32::from(area.y) + 6);
            let y = target - above(since, drop, bounce_height(column)).round() as i32;
            // Gray while falling and bouncing; stopped, the color wave
            let color = if since < FALL + BOUNCE {
                theme.muted
            } else {
                wave(theme, &palette, seconds, column, row)
            };
            // Always bold: thicker strokes
            let style = Style::new().fg(color).add_modifier(Modifier::BOLD);
            put(buf, area, left + column, y, symbol, style);
            // The bottom row kicks up a little dust as it lands
            if row == LOGO_HEIGHT - 1 {
                let age = seconds - delay - FALL;
                dust(buf, area, theme, left + column, top + LOGO_HEIGHT, age);
            }
        }
    }
    // The tagline, typed out once the letters have stopped; the cursor blinks while typing and a little after
    let typing = seconds - SETTLED;
    if typing <= 0.0 {
        return;
    }
    let total = tagline.chars().count();
    let shown: String = tagline
        .chars()
        .take(((typing * TYPE_SPEED) as usize).min(total))
        .collect();
    let finished = typing - total as f64 / TYPE_SPEED;
    let cursor = if finished < 1.5 && (seconds * 2.0).fract() < 0.5 {
        "_"
    } else {
        " "
    };
    let line = Line::from(vec![
        Span::styled(shown, Style::new().fg(theme.text)),
        Span::styled(cursor, Style::new().fg(theme.focus)),
    ]);
    // Centered on the whole tagline, so it does not shift while typing
    let x = centered(area, tagline_width);
    put_line(buf, area, x, top + LOGO_HEIGHT + 1, &line);
    // Today's date fades in after the tagline is done
    if finished <= 0.0 {
        return;
    }
    let date = texts.splash_date(app.today);
    let color = mix(theme.other_month, theme.muted, finished / 0.8);
    let x = centered(area, Span::raw(date.as_str()).width() as i32);
    let line = Line::styled(date, Style::new().fg(color));
    put_line(buf, area, x, top + LOGO_HEIGHT + 3, &line);
}

/// The rows of `LOGO`: its glyphs side by side
fn logo_rows() -> Vec<String> {
    (0..5)
        .map(|row| LOGO.iter().map(|glyph| glyph[row]).collect())
        .collect()
}

/// A landed letter's color at `column`, `row` of the logo: a slow color wave along the diagonal, whitened
/// where the glint is
fn wave(theme: &Theme, palette: &[Color], seconds: f64, column: i32, row: i32) -> Color {
    let along = f64::from(column) + f64::from(row) * 2.0;
    let base = cycle(
        palette,
        along / f64::from(LOGO_WIDTH) * 0.7 - seconds * 0.25,
    );
    let shine = glint(seconds, f64::from(column) + f64::from(row) * 1.5);
    mix(base, theme.text, shine)
}

/// How much of the white glint is on spot `along` of the logo (0..1): once, at `GLINT_AT`, a slanted band
/// sweeps across it in 0.9 seconds
fn glint(seconds: f64, along: f64) -> f64 {
    let phase = (seconds - GLINT_AT) / 0.9;
    if !(0.0..=1.0).contains(&phase) {
        return 0.0;
    }
    let band = phase * f64::from(LOGO_WIDTH + 16) - 8.0;
    (1.0 - (along - band).abs() / 3.0).max(0.0)
}

/// Stars at fixed spots (about 1 cell in 70), each one twinkling slowly (one round every 6 to 20 seconds)
/// from a faint `.` up to a bright `✦` and back (mostly small, now and then big). Three of them are a small
/// accent toward the corners, far apart: gold at the top left and the bottom right, blue at the top right;
/// never smaller than `+`, their color drifting slowly (gold toward amber, blue toward violet). None inside
/// `quiet`
fn stars(buf: &mut Buffer, area: Rect, theme: &Theme, seconds: f64, quiet: Rect) {
    let spots: Vec<(u16, u16)> = (area.top()..area.bottom())
        .flat_map(|y| (area.left()..area.right()).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            !quiet.contains(Position { x, y }) && noise(u32::from(x), u32::from(y)) <= 0.014
        })
        .collect();
    // An accent star: in its corner (the left or right third of the width, the top or bottom third of the
    // height), the star ranking highest by another pseudo-random number, so the same one every time (none if
    // no star is there)
    let rank = |&(x, y): &(u16, u16)| noise(u32::from(x) + 31, u32::from(y) + 17);
    let (third_x, third_y) = (area.width / 3, area.height / 3);
    let (left_edge, right_edge) = (area.x + third_x, area.right() - third_x);
    let (top_edge, bottom_edge) = (area.y + third_y, area.bottom() - third_y);
    let best_in = |right: bool, bottom: bool| {
        spots
            .iter()
            .copied()
            .filter(|&(x, y)| {
                let side = if right {
                    x >= right_edge
                } else {
                    x < left_edge
                };
                let end = if bottom {
                    y >= bottom_edge
                } else {
                    y < top_edge
                };
                side && end
            })
            .max_by(|a, b| rank(a).total_cmp(&rank(b)))
    };
    // (where, the color it starts from, the color it drifts toward)
    let accents = [
        (best_in(false, false), theme.notice, theme.priority_medium),
        (best_in(true, true), theme.notice, theme.priority_medium),
        (best_in(true, false), theme.weekend, theme.focus),
    ];
    for &(x, y) in &spots {
        let (x_seed, y_seed) = (u32::from(x), u32::from(y));
        let phase = noise(y_seed + 7, x_seed + 3);
        let speed = 0.3 + 0.8 * noise(x_seed + 11, y_seed + 5);
        let glow = (seconds * speed + phase * TAU).sin() * 0.5 + 0.5;
        let accent = accents.iter().find(|(spot, ..)| *spot == Some((x, y)));
        let (symbol, color) = if let Some(&(_, from, to)) = accent {
            // Up to 0.6 of the way and back, about once every 10 seconds
            let drift = (seconds * 0.6 + phase * TAU).sin() * 0.5 + 0.5;
            let symbol = if glow > 0.5 { '✦' } else { '+' };
            (symbol, mix(from, to, drift * 0.6))
        } else {
            match glow {
                glow if glow > 0.93 => ('✦', theme.text),
                glow if glow > 0.7 => ('+', theme.text),
                glow if glow > 0.4 => ('∙', theme.muted),
                _ => ('.', theme.other_month),
            }
        };
        let style = Style::new().fg(color);
        put(buf, area, i32::from(x), i32::from(y), symbol, style);
    }
}

/// Every 6 seconds (from 0.5 s on) a shooting star flies a long way across the sky above `quiet`, from
/// right to left (`METEOR_SPEED` for `METEOR_FOR`, so it often leaves the screen), at a different spot each
/// time, curving down to just above `quiet` like something falling. Its tail is braille dots (4 rows of dots
/// in a cell), so it slants smoothly instead of stepping down a whole row at a time; the tail fades behind
/// the head, and the head dims at the end. It passes behind `quiet`
fn shooting_star(buf: &mut Buffer, area: Rect, theme: &Theme, seconds: f64, quiet: Rect) {
    let since = seconds - 0.5;
    if since < 0.0 {
        return;
    }
    let round = (since / 6.0).floor();
    let age = since - round * 6.0;
    if age > METEOR_FOR {
        return;
    }
    let seed = round as u32 + 100;
    let start_x = f64::from(area.x) + f64::from(area.width) * (0.55 + 0.45 * noise(seed, 5));
    // It starts in the upper half of the sky above `quiet` and falls to just above it (at least 2 rows)
    let sky = f64::from(quiet.y.saturating_sub(area.y));
    let start_y = f64::from(area.y) + sky * 0.5 * noise(seed, 6);
    let fall = (f64::from(quiet.y) - 1.0 - start_y).max(2.0);
    // Where the head is `at` seconds in (in cells): `METEOR_SPEED` to the left; it falls slowly at first,
    // then faster and faster
    let position = |at: f64| {
        let part = at / METEOR_FOR;
        (
            start_x - at * METEOR_SPEED,
            start_y + fall * (0.25 * part + 0.75 * part * part),
        )
    };
    let head = if age > METEOR_FOR - 0.3 {
        theme.muted
    } else {
        theme.text
    };
    let colors = [head, theme.muted, theme.other_month];
    // The tail: where the head was over the last `METEOR_TAIL`, from the far end up to the head (a cell
    // takes the color of its last dot, the one nearest the head)
    let mut dots = Dots::new(area);
    let samples = 48;
    for sample in (1..=samples).rev() {
        let back = f64::from(sample) / f64::from(samples);
        let at = age - back * METEOR_TAIL;
        if at < 0.0 {
            continue;
        }
        let (x, y) = position(at);
        if inside(quiet, x.floor() as i32, y.floor() as i32) {
            continue;
        }
        let (dot_x, dot_y) = ((x - f64::from(area.x)) * 2.0, (y - f64::from(area.y)) * 4.0);
        dots.set(dot_x, dot_y, gradient(&colors, back));
    }
    dots.render(buf, area);
    // The head: a star at the front
    let (x, y) = position(age);
    let (x, y) = (x.floor() as i32, y.floor() as i32);
    if !inside(quiet, x, y) {
        put(buf, area, x, y, '*', Style::new().fg(head));
    }
}

/// A puff of dust at (`x`, `y`) `age` seconds after a letter lands: two specks flying out to the sides
fn dust(buf: &mut Buffer, area: Rect, theme: &Theme, x: i32, y: i32, age: f64) {
    if !(0.0..0.35).contains(&age) {
        return;
    }
    let reach = 1 + (age / 0.35 * 2.0) as i32;
    let color = mix(theme.muted, theme.other_month, age / 0.35);
    for side in [-reach, reach] {
        put(buf, area, x + side, y, '.', Style::new().fg(color));
    }
}

/// A canvas of braille dots: each cell holds 2 × 4 dots, so lines look smooth.
/// A cell takes the color of the last dot set in it
struct Dots {
    width: usize,
    height: usize,
    bits: Vec<u8>,
    colors: Vec<Color>,
}

impl Dots {
    /// An empty canvas the size of `area` (in dots: twice its width, four times its height)
    fn new(area: Rect) -> Self {
        let (width, height) = (usize::from(area.width), usize::from(area.height));
        Self {
            width,
            height,
            bits: vec![0; width * height],
            colors: vec![Color::Reset; width * height],
        }
    }

    /// Sets the dot at (`x`, `y`) (in dots from the top left); ones off the canvas are ignored
    fn set(&mut self, x: f64, y: f64, color: Color) {
        // Which bit each dot in a cell is (braille's dot numbering)
        const BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];
        if x < 0.0 || y < 0.0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.width * 2 || y >= self.height * 4 {
            return;
        }
        let cell = y / 4 * self.width + x / 2;
        self.bits[cell] |= BITS[y % 4][x % 2];
        self.colors[cell] = color;
    }

    /// Writes the dots into `area` of the screen (cells without dots are left as they are)
    fn render(&self, buf: &mut Buffer, area: Rect) {
        for (cell, &bits) in self.bits.iter().enumerate() {
            if bits == 0 {
                continue;
            }
            let symbol = char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ');
            let x = i32::from(area.x) + (cell % self.width) as i32;
            let y = i32::from(area.y) + (cell / self.width) as i32;
            put(buf, area, x, y, symbol, Style::new().fg(self.colors[cell]));
        }
    }
}

/// One character at (`x`, `y`); off `area` it is not drawn
fn put(buf: &mut Buffer, area: Rect, x: i32, y: i32, symbol: char, style: Style) {
    let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
        return;
    };
    if !area.contains(Position { x, y }) {
        return;
    }
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char(symbol).set_style(style);
    }
}

/// A line of text starting at (`x`, `y`), cut off at `area`'s edges; a row off `area` is not drawn
fn put_line(buf: &mut Buffer, area: Rect, x: i32, y: i32, line: &Line) {
    let x = x.max(i32::from(area.x));
    let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
        return;
    };
    if !area.contains(Position { x, y }) {
        return;
    }
    buf.set_line(x, y, line, area.right() - x);
}

/// The column where something `width` cells wide starts when centered in `area`
fn centered(area: Rect, width: i32) -> i32 {
    i32::from(area.x) + (i32::from(area.width) - width) / 2
}

/// A `Rect` from numbers that may be negative (cut to 0)
fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    let clamp = |value: i32| u16::try_from(value.max(0)).unwrap_or(u16::MAX);
    Rect::new(clamp(x), clamp(y), clamp(width), clamp(height))
}

/// Whether (`x`, `y`) is inside `rect`
fn inside(rect: Rect, x: i32, y: i32) -> bool {
    (i32::from(rect.x)..i32::from(rect.right())).contains(&x)
        && (i32::from(rect.y)..i32::from(rect.bottom())).contains(&y)
}

/// A fixed pseudo-random number in 0..=1 for each (`x`, `y`): where the stars are, how they twinkle, when a
/// letter starts falling
fn noise(x: u32, y: u32) -> f64 {
    let mut hash = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2C1B_3C6D);
    hash ^= hash >> 12;
    hash = hash.wrapping_mul(0x297A_2D39);
    hash ^= hash >> 15;
    f64::from(hash) / f64::from(u32::MAX)
}

/// How many rows above where it lands a letter is, `since` seconds after it started falling from `drop` rows
/// up: it falls faster and faster (like something dropped) for `FALL`, then bounces once `height` rows up
/// and back down for `BOUNCE` (a smooth arc), then stays
fn above(since: f64, drop: f64, height: f64) -> f64 {
    if since < FALL {
        let part = since / FALL;
        return drop * (1.0 - part * part);
    }
    let part = (since - FALL) / BOUNCE;
    if part < 1.0 {
        height * 4.0 * part * (1.0 - part)
    } else {
        0.0
    }
}

/// How high the letters in `column` of the logo bounce: 1 to 3 rows, different from column to column
fn bounce_height(column: i32) -> f64 {
    1.0 + (noise(column as u32, 7) * 3.0).floor().min(2.0)
}

/// A color along `colors` from the first (`position` 0) to the last (1)
fn gradient(colors: &[Color], position: f64) -> Color {
    let spot = position.clamp(0.0, 1.0) * (colors.len() - 1) as f64;
    let index = (spot.floor() as usize).min(colors.len() - 2);
    mix(colors[index], colors[index + 1], spot - index as f64)
}

/// A color along `colors` going round in a loop (`position` 0..1, then it starts over)
fn cycle(colors: &[Color], position: f64) -> Color {
    let spot = position.rem_euclid(1.0) * colors.len() as f64;
    let index = spot.floor() as usize % colors.len();
    mix(
        colors[index],
        colors[(index + 1) % colors.len()],
        spot.fract(),
    )
}

#[cfg(test)]
mod tests;
