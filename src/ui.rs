//! 畫整個畫面：最上面一列標題、中間的框、最下面一列操作說明

mod calendar;

use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::Line,
};

/// 畫整個畫面：中間左邊是月曆框，右邊先空著
pub fn render(frame: &mut Frame, app: &App) {
    let texts = app.lang.texts();
    let theme = &app.theme;
    let [title, main, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let title_style = Style::new().fg(theme.label).add_modifier(Modifier::BOLD);
    frame.render_widget(Line::styled(texts.title, title_style), title);
    let [calendar_area, _] =
        Layout::horizontal([Constraint::Length(calendar::WIDTH), Constraint::Fill(1)]).areas(main);
    calendar::render(frame, calendar_area, app);
    frame.render_widget(
        Line::styled(texts.hints, Style::new().fg(theme.muted)),
        hints,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use crate::theme::Theme;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, text::Span};

    /// 用 `lang` 畫一次 `width` × `height` 的畫面，今天是 2026/10/17
    fn draw(lang: Lang, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let app = App::new(lang, date(2026, 10, 17));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        terminal
    }

    /// 畫面每一列的字，去掉尾端空白；中文字佔 2 格，後面那格（buffer 裡是空白）跳過，接起來才是原本的字
    fn text_rows(buffer: &Buffer) -> Vec<String> {
        let area = buffer.area;
        (0..area.height)
            .map(|y| {
                let mut row = String::new();
                let mut x = 0;
                while x < area.width {
                    let symbol = buffer[(x, y)].symbol();
                    row.push_str(symbol);
                    x += u16::try_from(Span::raw(symbol).width().max(1)).unwrap();
                }
                row.trim_end().to_string()
            })
            .collect()
    }

    #[test]
    fn calendar_from_this_month_down_with_a_thick_line_between_years() {
        // 最上面的月和 1 月先寫年、下一列寫月；跨年那條是粗線
        let terminal = draw(Lang::ZhTw, 40, 27);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                " roh-cal",
                "╭─────────────────────────────────╮",
                "│  年月 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│2026年 40 │           1  2  3  4 │",
                "│  10月 41 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16 17 18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "│  11月 44 │                    1 │",
                "│       45 │  2  3  4  5  6  7  8 │",
                "│       46 │  9 10 11 12 13 14 15 │",
                "│       47 │ 16 17 18 19 20 21 22 │",
                "│       48 │ 23 24 25 26 27 28 29 │",
                "│       49 │ 30                   │",
                "│──────────┼──────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 │",
                "│       50 │  7  8  9 10 11 12 13 │",
                "│       51 │ 14 15 16 17 18 19 20 │",
                "│       52 │ 21 22 23 24 25 26 27 │",
                "│       53 │ 28 29 30 31          │",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2  3 │",
                "│   1月  2 │  4  5  6  7  8  9 10 │",
                "╰─────────────────────────────────╯",
                " q 離開",
            ]
        );
    }

    #[test]
    fn calendar_in_english() {
        let terminal = draw(Lang::En, 40, 12);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                " roh-cal",
                "╭─────────────────────────────────╮",
                "│       Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│  2026 40 │           1  2  3  4 │",
                "│   Oct 41 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16 17 18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "╰─────────────────────────────────╯",
                " q quit",
            ]
        );
    }
    #[test]
    fn colors_by_role() {
        let terminal = draw(Lang::ZhTw, 40, 27);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // (x, y) 那格：字、前景色、是不是粗體
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::BOLD),
            )
        };
        // 標題粗體、操作說明淡色、框線
        assert_eq!(cell(1, 0), ("r", theme.label, true));
        assert_eq!(cell(1, 26), ("q", theme.muted, false));
        assert_eq!(cell(0, 1), ("╭", theme.line, false));
        // 表頭：平日淡色、六日用週末的顏色、直線
        assert_eq!(cell(3, 2), ("年", theme.muted, false));
        assert_eq!(cell(13, 2), ("一", theme.muted, false));
        assert_eq!(cell(28, 2), ("六", theme.weekend, false));
        assert_eq!(cell(31, 2), ("日", theme.weekend, false));
        assert_eq!(cell(11, 2), ("│", theme.line, false));
        // 月和月之間的線、跨年的粗線
        assert_eq!(cell(11, 3), ("┼", theme.line, false));
        assert_eq!(cell(5, 22), ("━", theme.year_line, false));
        assert_eq!(cell(11, 22), ("┿", theme.year_line, false));
        // 年 / 月、週數
        assert_eq!(cell(1, 4), ("2", theme.label, false));
        assert_eq!(cell(3, 5), ("1", theme.label, false));
        assert_eq!(cell(8, 6), ("4", theme.week_number, false));
        // 10/16（五）一般；10/17（六）是今天：今天的顏色蓋過週末，而且粗體；10/18（日）週末
        assert_eq!(cell(25, 6), ("1", theme.text, false));
        assert_eq!(cell(29, 6), ("7", theme.today, true));
        assert_eq!(cell(32, 6), ("8", theme.weekend, false));
    }
}
