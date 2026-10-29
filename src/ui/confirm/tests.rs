use super::*;
use crate::app::Mode;
use crate::tasks;
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

/// An app (`width` × `height`, catppuccin-mocha) asking whether to delete the first item of `text`, which
/// removes `count` items, drawn once; returns it, the box's area and the screen
fn asking_to_delete(
    lang: Lang,
    (width, height): (u16, u16),
    text: &str,
    count: usize,
) -> (App, Rect, Buffer) {
    let items = tasks::parse(text).items;
    let mut app = App::new(lang, date(2026, 10, 17), items.clone()).with_theme("catppuccin-mocha");
    app.screen = Rect::new(0, 0, width, height);
    app.mode = Mode::ConfirmDelete(items[0].clone(), count);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| crate::ui::render(frame, &mut app))
        .unwrap();
    let question = question(&app).expect("asking");
    let (area, _) = layout(&app, app.screen, &question);
    (app, area, terminal.backend().buffer().clone())
}

#[test]
fn the_box_asks_in_the_middle_with_two_buttons() {
    // Expected rows from a separate Python script: inside 30 cells (the widest, 「和底下 1 件子任務」 17 + 4,
    // is below the minimum), 2 + 6 rows; in the middle of 70 × 24 → x 19, y 8. Each button with a blank on
    // each side (10 cells) in the middle of its half (15 cells)
    let (app, area, buffer) = asking_to_delete(
        Lang::ZhTw,
        (70, 24),
        "- [ ] Review PRs @2026-10-17\n  - [ ] Amy's PR\n",
        2,
    );
    assert_eq!(area, Rect::new(19, 8, 32, 8));
    assert_eq!(
        crate::ui::tests::area_rows(&buffer, area),
        [
            "╭─────────── 刪除？ ───────────╮",
            "│                              │",
            "│  Review PRs                  │",
            "│  和底下 1 件子任務           │",
            "│                              │",
            "│──────────────────────────────│",
            "│   [Y] 刪除       (N) 取消    │",
            "╰──────────────────────────────╯",
        ]
    );
    // Colors: the question is a notice, the subject text, the detail gray, the keys like the help page's,
    // the line above the buttons the border's
    let theme = &app.theme;
    let fg = |x: u16, y: u16| buffer[(x, y)].fg;
    assert_eq!(fg(32, 8), theme.notice);
    assert_eq!(fg(22, 10), theme.text);
    assert_eq!(fg(22, 11), theme.muted);
    assert_eq!(fg(19, 13), theme.focus);
    assert_eq!(fg(30, 13), theme.focus);
    assert_eq!((fg(23, 14), fg(25, 14)), (theme.help_key, theme.help_key));
    assert_eq!(fg(27, 14), theme.text);
    assert_eq!(fg(38, 14), theme.help_key);
}

#[test]
fn a_long_subject_wraps_at_the_widest_box() {
    // Expected rows from the same Python script: the title is wider than 50, so the inside is 50 and the
    // title wraps at 46; in the middle of 80 × 24 → x 14, y 8
    let title = "Write the design doc for the new task file format and send it to Amy for review";
    let (_, area, buffer) = asking_to_delete(
        Lang::En,
        (80, 24),
        &format!("- [ ] {title} @2026-10-17\n"),
        1,
    );
    assert_eq!(area, Rect::new(14, 8, 52, 8));
    assert_eq!(
        crate::ui::tests::area_rows(&buffer, area),
        [
            "╭──────────────────── Delete? ─────────────────────╮",
            "│                                                  │",
            "│  Write the design doc for the new task file      │",
            "│  format and send it to Amy for review            │",
            "│                                                  │",
            "│──────────────────────────────────────────────────│",
            "│       [Y] Delete               (N) Cancel        │",
            "╰──────────────────────────────────────────────────╯",
        ]
    );
}

#[test]
fn clicks_on_the_buttons_and_outside() {
    // The box of the first test: yes on x 22..32, no on x 37..47, both on row 14 (from the Python script)
    let (app, _, _) = asking_to_delete(
        Lang::ZhTw,
        (70, 24),
        "- [ ] Review PRs @2026-10-17\n  - [ ] Amy's PR\n",
        2,
    );
    let question = question(&app).expect("asking");
    let click = |column, row| super::click(&app, app.screen, &question, column, row);
    // Yes: both ends, the blank around the word included
    assert_eq!((click(22, 14), click(31, 14)), (Some(true), Some(true)));
    // No: the same; outside the box: cancel too
    assert_eq!((click(37, 14), click(46, 14)), (Some(false), Some(false)));
    assert_eq!(
        (click(5, 5), click(18, 14), click(51, 14)),
        (Some(false), Some(false), Some(false))
    );
    // Elsewhere in the box (between the buttons, the subject, the border): nothing
    assert_eq!(
        (click(34, 14), click(25, 10), click(19, 12)),
        (None, None, None)
    );
}
