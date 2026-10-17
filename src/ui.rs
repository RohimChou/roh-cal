//! 畫整個畫面：最上面一列標題、中間的框、最下面一列操作說明

mod calendar;

use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
};

/// 畫整個畫面：中間左邊是月曆框，右邊先空著
pub fn render(frame: &mut Frame, app: &App) {
    let texts = app.lang.texts();
    let [title, main, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    frame.render_widget(texts.title, title);
    let [calendar_area, _] =
        Layout::horizontal([Constraint::Length(calendar::WIDTH), Constraint::Fill(1)]).areas(main);
    calendar::render(frame, calendar_area, app);
    frame.render_widget(texts.hints, hints);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend};

    /// 用 `lang` 畫一次 `width` × `height` 的畫面，今天是 2026/10/17
    fn draw(lang: Lang, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let app = App::new(lang, date(2026, 10, 17));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        terminal
    }

    #[test]
    fn calendar_from_this_month_down_with_a_thick_line_between_years() {
        // 最上面的月和 1 月先寫年、下一列寫月；跨年那條是粗線
        draw(Lang::ZhTw, 40, 27).backend().assert_buffer_lines([
            " roh-cal                                ",
            "╭─────────────────────────────────╮     ",
            "│  年月 週 │ 一 二 三 四 五 六 日 │     ",
            "│──────────┼──────────────────────│     ",
            "│2026年 40 │           1  2  3  4 │     ",
            "│  10月 41 │  5  6  7  8  9 10 11 │     ",
            "│       42 │ 12 13 14 15 16 17 18 │     ",
            "│       43 │ 19 20 21 22 23 24 25 │     ",
            "│       44 │ 26 27 28 29 30 31    │     ",
            "│──────────┼──────────────────────│     ",
            "│  11月 44 │                    1 │     ",
            "│       45 │  2  3  4  5  6  7  8 │     ",
            "│       46 │  9 10 11 12 13 14 15 │     ",
            "│       47 │ 16 17 18 19 20 21 22 │     ",
            "│       48 │ 23 24 25 26 27 28 29 │     ",
            "│       49 │ 30                   │     ",
            "│──────────┼──────────────────────│     ",
            "│  12月 49 │     1  2  3  4  5  6 │     ",
            "│       50 │  7  8  9 10 11 12 13 │     ",
            "│       51 │ 14 15 16 17 18 19 20 │     ",
            "│       52 │ 21 22 23 24 25 26 27 │     ",
            "│       53 │ 28 29 30 31          │     ",
            "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━│     ",
            "│2027年  1 │              1  2  3 │     ",
            "│   1月  2 │  4  5  6  7  8  9 10 │     ",
            "╰─────────────────────────────────╯     ",
            " q 離開                                 ",
        ]);
    }

    #[test]
    fn calendar_in_english() {
        draw(Lang::En, 40, 12).backend().assert_buffer_lines([
            " roh-cal                                ",
            "╭─────────────────────────────────╮     ",
            "│       Wk │ Mo Tu We Th Fr Sa Su │     ",
            "│──────────┼──────────────────────│     ",
            "│  2026 40 │           1  2  3  4 │     ",
            "│   Oct 41 │  5  6  7  8  9 10 11 │     ",
            "│       42 │ 12 13 14 15 16 17 18 │     ",
            "│       43 │ 19 20 21 22 23 24 25 │     ",
            "│       44 │ 26 27 28 29 30 31    │     ",
            "│──────────┼──────────────────────│     ",
            "╰─────────────────────────────────╯     ",
            " q quit                                 ",
        ]);
    }
}
