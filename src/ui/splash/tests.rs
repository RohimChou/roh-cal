use super::*;
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::{Terminal, backend::TestBackend};

/// The logo's 5 rows, written out by hand from figlet's Standard font
const LOGO_TEXT: [&str; 5] = [
    r"              _                         _ ",
    r" _ __   ___  | |__          ___   __ _ | |",
    r"| '__| / _ \ | '_ \  ____  / __| / _` || |",
    r"| |   | (_) || | | ||____|| (__ | (_| || |",
    r"|_|    \___/ |_| |_|       \___| \__,_||_|",
];

/// Draws the intro on a `width` × `height` screen, `seconds` in (today = Tuesday 2026/10/20)
fn draw(lang: Lang, width: u16, height: u16, seconds: f64) -> Buffer {
    let app = App::new(lang, date(2026, 10, 20), Vec::new());
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render(frame, &app, seconds)).unwrap();
    terminal.backend().buffer().clone()
}

/// The `width` cells from (`x`, `y`) (one character each: ASCII only)
fn text_at(buffer: &Buffer, x: u16, y: u16, width: u16) -> String {
    (x..x + width).map(|x| buffer[(x, y)].symbol()).collect()
}

/// Whether `symbol` is drawn by the stars or the shooting star (a braille dot, `*`, `.`, `∙`, `+`, `✦`)
fn is_sky(symbol: &str) -> bool {
    let braille = symbol
        .chars()
        .next()
        .is_some_and(|c| ('\u{2801}'..='\u{28ff}').contains(&c));
    braille || ["*", ".", "∙", "+", "✦"].contains(&symbol)
}

#[test]
fn logo_glyphs_put_side_by_side_make_the_logo() {
    for glyph in &LOGO {
        assert!(
            glyph.iter().all(|row| row.len() == glyph[0].len()),
            "{glyph:?}"
        );
    }
    assert_eq!(logo_rows(), LOGO_TEXT);
    assert!(LOGO_TEXT.iter().all(|row| row.len() as i32 == LOGO_WIDTH));
}

#[test]
fn once_settled_the_logo_tagline_and_date_sit_in_the_middle() {
    // 80 × 24: the block of 9 rows (logo 5, blank, tagline, blank, date) starts at row (24 - 9) / 2 = 7;
    // the logo at column (80 - 42) / 2 = 19, the tagline (55 + 1 for the cursor) at (80 - 56) / 2 = 12,
    // the date (25) at (80 - 25) / 2 = 27. 4.5 s in, the cursor after the tagline has stopped blinking
    let buffer = draw(Lang::En, 80, 24, 4.5);
    for (row, text) in LOGO_TEXT.iter().enumerate() {
        assert_eq!(text_at(&buffer, 19, 7 + row as u16, 42), *text, "row {row}");
    }
    assert_eq!(
        text_at(&buffer, 12, 13, 56),
        "a simple, keyboard-driven calendar for plain-text tasks "
    );
    assert_eq!(text_at(&buffer, 27, 15, 25), "Tuesday, October 20, 2026");
    // Every stroke of the logo is bold
    for y in 7..12 {
        for x in 19..61 {
            let cell = &buffer[(x, y)];
            if cell.symbol() != " " {
                assert!(cell.modifier.contains(Modifier::BOLD), "({x}, {y})");
            }
        }
    }
}

#[test]
fn every_letter_has_stopped_when_the_tagline_starts() {
    // The tagline starts at `SETTLED`: by then the last letter (its delay at most
    // 0.008 × 41 + 0.12 = 0.45 s, then the 0.53 s fall and the 0.25 s bounce) is in place
    let buffer = draw(Lang::En, 80, 24, SETTLED);
    for (row, text) in LOGO_TEXT.iter().enumerate() {
        assert_eq!(text_at(&buffer, 19, 7 + row as u16, 42), *text, "row {row}");
    }
}

#[test]
fn the_glint_sweeps_once_near_the_end() {
    // Nothing of it before 3.6 s, or after it has swept across (4.5 s); the intro closes at 5.5 s
    for step in 0..36 {
        let seconds = f64::from(step) * 0.1;
        for along in 0..60 {
            assert_eq!(
                glint(seconds, f64::from(along)),
                0.0,
                "{seconds} s, {along}"
            );
        }
    }
    for seconds in [4.55, 5.0, 8.0] {
        assert_eq!(glint(seconds, 30.0), 0.0, "{seconds} s");
    }
    // Halfway (4.05 s) the band's middle is at spot 0.5 × (42 + 16) - 8 = 21: white there, nothing 3 or more
    // spots away
    assert!(glint(4.05, 21.0) > 0.99);
    assert_eq!(glint(4.05, 25.0), 0.0);
    assert_eq!(glint(4.05, 17.0), 0.0);
}

#[test]
fn letters_fall_faster_and_faster_then_bounce_once_a_little() {
    // Falling from 15 rows up: only ever down, a bigger step each time, until it lands at `FALL`
    let mut last = above(0.0, 15.0, 4.0);
    assert_eq!(last, 15.0);
    let mut last_step = 0.0;
    for step in 1..=53 {
        let now = above(f64::from(step) / 100.0, 15.0, 4.0);
        assert!(
            now <= last && last - now >= last_step,
            "{step}: {last} → {now}"
        );
        (last_step, last) = (last - now, now);
    }
    assert_eq!(last, 0.0);
    // Then one bounce: 4 rows up halfway through `BOUNCE`, back down at its end, and it stays there
    assert!((above(FALL + BOUNCE / 2.0, 15.0, 4.0) - 4.0).abs() < 1e-9);
    assert!(above(FALL + BOUNCE / 4.0, 15.0, 4.0) < 4.0);
    for after in [0.0, 0.1, 1.0, 10.0] {
        assert_eq!(above(FALL + BOUNCE + after, 15.0, 4.0), 0.0);
    }
}

#[test]
fn bounces_are_1_to_3_rows_high_and_differ_by_column() {
    let heights: Vec<f64> = (0..LOGO_WIDTH).map(bounce_height).collect();
    assert!(
        heights
            .iter()
            .all(|height| [1.0, 2.0, 3.0].contains(height)),
        "{heights:?}"
    );
    for height in [1.0, 2.0, 3.0] {
        assert!(heights.contains(&height), "{heights:?}");
    }
}

#[test]
fn before_the_letters_start_falling_only_the_sky_shows() {
    // (and the hint on the bottom row, see below)
    let buffer = draw(Lang::En, 80, 24, 0.0);
    for y in 0..23 {
        for x in 0..80 {
            let symbol = buffer[(x, y)].symbol();
            assert!(symbol == " " || is_sky(symbol), "({x}, {y}): {symbol}");
        }
    }
}

#[test]
fn falling_letters_are_gray_above_where_they_land() {
    // 0.45 s in: some letters are still on their way down, above the logo's rows (7 to 11), in `muted`
    let app = App::new(Lang::En, date(2026, 10, 20), Vec::new());
    let buffer = draw(Lang::En, 80, 24, 0.45);
    let falling: Vec<_> = (0..7)
        .flat_map(|y| (19..61).map(move |x| (x, y)))
        .filter(|&(x, y)| "_|/\\()'`,".contains(buffer[(x, y)].symbol()))
        .collect();
    assert!(!falling.is_empty());
    for (x, y) in falling {
        assert_eq!(buffer[(x, y)].fg, app.theme.muted, "({x}, {y})");
    }
}

#[test]
fn the_tagline_is_typed_out_once_the_letters_have_stopped() {
    // Typing starts 1.4 s in, 45 characters a second: 10.5 characters' time later (1.63 s), 10 are shown,
    // then the cursor (it shows in the first half of every half second: 1.63 × 2 = 3.27, .27 < .5)
    let buffer = draw(Lang::En, 80, 24, 1.4 + 10.5 / 45.0);
    assert_eq!(text_at(&buffer, 12, 13, 12), "a simple, _ ");
    let buffer = draw(Lang::En, 80, 24, 1.35);
    assert_eq!(text_at(&buffer, 12, 13, 56).trim(), "");
    // The date comes after the whole tagline: not yet 1.4 + 55 / 45 = 2.62 s in
    let buffer = draw(Lang::En, 80, 24, 2.6);
    assert_eq!(text_at(&buffer, 27, 15, 25).trim(), "");
}

#[test]
fn stars_and_shooting_stars_stay_out_of_the_middle() {
    // 80 × 24: the quiet block is the widest line (the tagline, 56) plus 3 columns on each side, from the
    // row above the logo to the one under the date: columns 9 to 70, rows 6 to 16. From 2 s on (the dust
    // under the letters is long gone) nothing of the sky is drawn there
    let mut sky_seen = false;
    for step in 0..160 {
        let seconds = 2.0 + f64::from(step) * 0.37;
        let buffer = draw(Lang::En, 80, 24, seconds);
        for y in 0..24 {
            for x in 0..80 {
                let symbol = buffer[(x, y)].symbol();
                if (9..71).contains(&x) && (6..17).contains(&y) {
                    assert!(!is_sky(symbol), "{seconds} s ({x}, {y}): {symbol}");
                } else {
                    sky_seen |= is_sky(symbol);
                }
            }
        }
    }
    assert!(sky_seen);
}

#[test]
fn three_accent_stars_far_apart_drift_in_color() {
    // 3 s and 4.5 s in (no shooting star then): outside the middle block (columns 9 to 70, rows 6 to 16),
    // only stars are drawn; the plain ones are `text`, `muted` or `other_month`, so the others are the
    // accents. nord: gold `notice` #ebcb8b → amber `priority_medium` #d08770, blue `weekend` #81a1c1 →
    // violet `focus` #b48ead
    let app = App::new(Lang::En, date(2026, 10, 20), Vec::new());
    let plain = [app.theme.text, app.theme.muted, app.theme.other_month];
    let accents_at = |seconds: f64| {
        let buffer = draw(Lang::En, 80, 24, seconds);
        let mut accents = Vec::new();
        for y in 0..24 {
            for x in 0..80 {
                let cell = &buffer[(x, y)];
                let middle = (9..71).contains(&x) && (6..17).contains(&y);
                if !middle && cell.symbol() != " " && !plain.contains(&cell.fg) {
                    assert!(["+", "✦"].contains(&cell.symbol()), "({x}, {y})");
                    accents.push(((x, y), cell.fg));
                }
            }
        }
        accents
    };
    let (first, later) = (accents_at(3.0), accents_at(4.5));
    // The same three stars, their colors moved
    assert_eq!(first.len(), 3, "{first:?}");
    assert_eq!(
        first.iter().map(|star| star.0).collect::<Vec<_>>(),
        later.iter().map(|star| star.0).collect::<Vec<_>>()
    );
    assert_ne!(first, later);
    // In the corners (the outer thirds: columns 0 to 25 and 54 to 79, rows 0 to 7 and 16 to 23): top left,
    // top right, bottom right
    let corner = |(x, y): (u16, u16)| match (x, y) {
        (0..26, 0..8) => "top left",
        (54.., 0..8) => "top right",
        (54.., 16..) => "bottom right",
        _ => "elsewhere",
    };
    let mut corners: Vec<_> = first.iter().map(|star| corner(star.0)).collect();
    corners.sort();
    assert_eq!(
        corners,
        ["bottom right", "top left", "top right"],
        "{first:?}"
    );
    // Gold toward amber at the top left and the bottom right, blue toward violet at the top right
    let between = |color: Color, (r1, g1, b1): (u8, u8, u8), (r2, g2, b2): (u8, u8, u8)| {
        let Color::Rgb(red, green, blue) = color else {
            return false;
        };
        let within =
            |value: u8, one: u8, other: u8| (one.min(other)..=one.max(other)).contains(&value);
        within(red, r1, r2) && within(green, g1, g2) && within(blue, b1, b2)
    };
    for ((x, y), color) in first.iter().chain(&later) {
        let (from, to) = if corner((*x, *y)) != "top right" {
            ((0xeb, 0xcb, 0x8b), (0xd0, 0x87, 0x70))
        } else {
            ((0x81, 0xa1, 0xc1), (0xb4, 0x8e, 0xad))
        };
        assert!(between(*color, from, to), "({x}, {y}): {color:?}");
    }
}

#[test]
fn accent_stars_sit_in_the_corners_of_a_bigger_screen_too() {
    // 120 × 35: the outer thirds are columns 0 to 39 and 80 to 119, rows 0 to 10 and 24 to 34; outside the
    // middle block (columns 29 to 90, rows 12 to 24) only stars are drawn, so the ones in neither plain color
    // are the accents
    let app = App::new(Lang::En, date(2026, 10, 20), Vec::new());
    let plain = [app.theme.text, app.theme.muted, app.theme.other_month];
    let buffer = draw(Lang::En, 120, 35, 3.0);
    let mut corners = Vec::new();
    for y in 0..35 {
        for x in 0..120 {
            let cell = &buffer[(x, y)];
            let middle = (29..91).contains(&x) && (12..25).contains(&y);
            if !middle && cell.symbol() != " " && !plain.contains(&cell.fg) {
                corners.push(match (x, y) {
                    (0..40, 0..11) => "top left",
                    (80.., 0..11) => "top right",
                    (80.., 24..) => "bottom right",
                    _ => "elsewhere",
                });
            }
        }
    }
    corners.sort();
    assert_eq!(corners, ["bottom right", "top left", "top right"]);
}

#[test]
fn a_shooting_star_crosses_the_sky_above_the_logo() {
    // 1.2 s in, the first one (from 0.5 s) is 0.7 s into its flight: its braille tail and its head `*` are
    // in the rows above the quiet block (0 to 5)
    let buffer = draw(Lang::En, 80, 24, 1.2);
    let rows = crate::ui::tests::area_rows(&buffer, buffer.area);
    let braille = |row: &String| row.chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c));
    assert!(rows[..6].iter().any(braille), "{rows:#?}");
    assert!(rows[..6].iter().any(|row| row.contains('*')), "{rows:#?}");
    assert!(!rows[6..].iter().any(braille), "{rows:#?}");
    // Between two of them (every 6 s, 2.2 s long), none
    let buffer = draw(Lang::En, 80, 24, 3.0);
    let rows = crate::ui::tests::area_rows(&buffer, buffer.area);
    assert!(!rows.iter().any(braille), "{rows:#?}");
}

#[test]
fn a_shooting_star_flies_slowly() {
    // 35 cells a second for 2.2 s: on a wide screen (it starts in the right 45%, so it is still on
    // screen) the first one (from 0.5 s) is still flying 2.1 s into its flight, and gone 2.3 s in
    let braille = |rows: &[String]| {
        rows.iter()
            .any(|row| row.chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)))
    };
    let buffer = draw(Lang::En, 200, 40, 0.5 + 2.1);
    assert!(braille(&crate::ui::tests::area_rows(&buffer, buffer.area)));
    let buffer = draw(Lang::En, 200, 40, 0.5 + 2.3);
    assert!(!braille(&crate::ui::tests::area_rows(&buffer, buffer.area)));
    // Its head `*` moves 35 cells left in a second (give or take a cell from rounding)
    let head = |seconds: f64| {
        let buffer = draw(Lang::En, 200, 40, seconds);
        (0..40)
            .flat_map(|y| (0..200).map(move |x| (x, y)))
            .find(|&(x, y)| buffer[(x, y)].symbol() == "*")
            .map(|(x, _)| i32::from(x))
            .expect("the head is on screen")
    };
    let moved = head(0.5 + 0.5) - head(0.5 + 1.5);
    assert!((34..=36).contains(&moved), "{moved}");
}

#[test]
fn the_bottom_left_corner_says_any_key_skips_it() {
    // 80 × 24: on the last row, one column in, in `muted`, all the time it plays
    let app = App::new(Lang::En, date(2026, 10, 20), Vec::new());
    for seconds in [0.0, 2.0, 5.4] {
        let buffer = draw(Lang::En, 80, 24, seconds);
        assert_eq!(
            text_at(&buffer, 1, 23, 21),
            "Press any key to skip",
            "{seconds} s"
        );
        assert_eq!(buffer[(1, 23)].fg, app.theme.muted);
    }
    let buffer = draw(Lang::ZhTw, 80, 24, 1.0);
    assert_eq!(buffer[(1, 23)].symbol(), "按");
    let rows = crate::ui::tests::area_rows(&buffer, buffer.area);
    assert!(rows[23].contains("按任意鍵跳過"), "{}", rows[23]);
    // 80 × 9: the bottom row is the date's, so no hint
    let buffer = draw(Lang::En, 80, 9, 3.0);
    let rows = crate::ui::tests::area_rows(&buffer, buffer.area);
    assert!(rows.iter().all(|row| !row.contains("Press")), "{rows:#?}");
    assert!(rows[8].contains("Tuesday, October 20, 2026"), "{rows:#?}");
}

#[test]
fn chinese_tagline_and_date() {
    let buffer = draw(Lang::ZhTw, 80, 24, 3.0);
    let rows = crate::ui::tests::area_rows(&buffer, buffer.area);
    assert!(
        rows[13].contains("用純文字記事、全鍵盤操作的 terminal 月曆"),
        "{}",
        rows[13]
    );
    assert!(
        rows[15].contains("2026 年 10 月 20 日 星期二"),
        "{}",
        rows[15]
    );
    // The tagline is 40 cells wide (+ 1 for the cursor): it starts at column (80 - 41) / 2 = 19
    assert_eq!(buffer[(19, 13)].symbol(), "用");
}

#[test]
fn tiny_screens_do_not_break_it() {
    for (width, height) in [(1, 1), (10, 3), (30, 8), (42, 9), (200, 60)] {
        for seconds in [0.0, 0.3, 1.2, 3.0, 9.9] {
            draw(Lang::En, width, height, seconds);
            draw(Lang::ZhTw, width, height, seconds);
        }
    }
}
