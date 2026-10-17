//! 畫整個畫面：最上面一列標題、中間的框、最下面一列操作說明

mod calendar;

use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
};

/// 畫整個畫面：中間左邊是月曆框，右邊先空著
pub fn render(frame: &mut Frame, app: &mut App) {
    let texts = app.lang.texts();
    let theme = &app.theme;
    let [title, main, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    // 標題列：左邊 app 的名字、右邊選取的日期（後面留一格空白）
    // 兩段畫在同一列，顏色要放在 `Span` 上：`Line::styled` 的顏色會套到整列，後畫的會蓋掉先畫的
    let title_style = Style::new().fg(theme.label).add_modifier(Modifier::BOLD);
    frame.render_widget(Line::from(Span::styled(texts.title, title_style)), title);
    let date = format!("{} ", texts.date_title(app.selected));
    frame.render_widget(
        Line::from(Span::styled(date, Style::new().fg(theme.text))).right_aligned(),
        title,
    );
    frame.render_widget(
        Line::styled(texts.hints, Style::new().fg(theme.muted)),
        hints,
    );
    // 月曆框最後畫：它會改 `app`（記住捲到哪），這時候上面借的 `theme` 已經用完了
    let [calendar_area, _] =
        Layout::horizontal([Constraint::Length(calendar::WIDTH), Constraint::Fill(1)]).areas(main);
    calendar::render(frame, calendar_area, app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use crate::theme::Theme;
    use jiff::{
        ToSpan,
        civil::{Date, date},
    };
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    /// 用 `lang` 畫一次 `width` × `height` 的畫面：今天是 2026/10/17、選取 `selected`
    fn draw(lang: Lang, width: u16, height: u16, selected: Date) -> Terminal<TestBackend> {
        let mut app = App::new(lang, date(2026, 10, 17));
        app.selected = selected;
        draw_app(&mut app, width, height)
    }

    /// 用現在的 `app` 畫一次 `width` × `height` 的畫面（捲到哪會記在 `app` 裡，下一次接著用）
    fn draw_app(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
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
        // 最上面的月和 1 月先寫年、下一列寫月；跨年那條是粗線；選取的今天加括號、標題列右邊寫日期
        let terminal = draw(Lang::ZhTw, 40, 27, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                " roh-cal               2026/10/17（六）",
                "╭─────────────────────────────────╮",
                "│  年月 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│2026年 40 │           1  2  3  4 │",
                "│  10月 41 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16[17]18 │",
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
                " hjkl 移動  t 今天  q 離開",
            ]
        );
    }

    #[test]
    fn calendar_in_english() {
        let terminal = draw(Lang::En, 40, 12, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                " roh-cal               Sat Oct 17, 2026",
                "╭─────────────────────────────────╮",
                "│       Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│  2026 40 │           1  2  3  4 │",
                "│   Oct 41 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16[17]18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "╰─────────────────────────────────╯",
                " hjkl move  t today  q quit",
            ]
        );
    }

    #[test]
    fn selection_brackets_and_the_month_on_top() {
        // 選到別的月：那個月排在最上面（寫年份）；一位數的日期括號貼著數字
        let terminal = draw(Lang::En, 40, 9, date(2026, 11, 2));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                " roh-cal                Mon Nov 2, 2026",
                "╭─────────────────────────────────╮",
                "│       Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│  2026 44 │                    1 │",
                "│   Nov 45 │ [2] 3  4  5  6  7  8 │",
                "│       46 │  9 10 11 12 13 14 15 │",
                "╰─────────────────────────────────╯",
                " hjkl move  t today  q quit",
            ]
        );
        // 星期日：`]` 放在最後多留的那格；月曆只有 3 列、要留 1 列，所以往上多捲一列（跨年的粗線）
        let terminal = draw(Lang::ZhTw, 40, 9, date(2027, 1, 3));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                " roh-cal                 2027/1/3（日）",
                "╭─────────────────────────────────╮",
                "│  年月 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2 [3]│",
                "│   1月  2 │  4  5  6  7  8  9 10 │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  t 今天  q 離開",
            ]
        );
    }

    #[test]
    fn calendar_scrolls_down_only_when_the_selection_gets_close_to_the_bottom() {
        // 月曆有 6 列：10/17 → 10/24 → 10/31 → 11/7 一路往下，選取的那列離下緣至少留 2 列，不夠才捲
        let mut app = App::new(Lang::En, date(2026, 10, 17));
        draw_app(&mut app, 40, 12);
        for day in [date(2026, 10, 24), date(2026, 10, 31), date(2026, 11, 7)] {
            app.selected = day;
            draw_app(&mut app, 40, 12);
        }
        // 10 月的第一列捲到上面了，畫面上第一個從頭開始的 11 月寫年份
        assert_eq!(
            text_rows(draw_app(&mut app, 40, 12).backend().buffer()),
            [
                " roh-cal                Sat Nov 7, 2026",
                "╭─────────────────────────────────╮",
                "│       Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "│  2026 44 │                    1 │",
                "│   Nov 45 │  2  3  4  5  6 [7] 8 │",
                "│       46 │  9 10 11 12 13 14 15 │",
                "│       47 │ 16 17 18 19 20 21 22 │",
                "╰─────────────────────────────────╯",
                " hjkl move  t today  q quit",
            ]
        );
        // 往回一週：選取的那列離上緣至少留 2 列，往上捲 2 列
        app.selected = date(2026, 10, 31);
        assert_eq!(
            text_rows(draw_app(&mut app, 40, 12).backend().buffer()),
            [
                " roh-cal               Sat Oct 31, 2026",
                "╭─────────────────────────────────╮",
                "│       Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│       42 │ 12 13 14 15 16 17 18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30[31]   │",
                "│──────────┼──────────────────────│",
                "│  2026 44 │                    1 │",
                "│   Nov 45 │  2  3  4  5  6  7  8 │",
                "╰─────────────────────────────────╯",
                " hjkl move  t today  q quit",
            ]
        );
    }

    #[test]
    fn calendar_scrolls_up_into_earlier_months() {
        // 10/17 往上一週到 10/10：上面只剩 1 列，往上捲 1 列，露出 9 月後面的線
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17));
        draw_app(&mut app, 40, 12);
        app.selected = date(2026, 10, 10);
        assert_eq!(
            text_rows(draw_app(&mut app, 40, 12).backend().buffer()),
            [
                " roh-cal               2026/10/10（六）",
                "╭─────────────────────────────────╮",
                "│  年月 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│──────────┼──────────────────────│",
                "│2026年 40 │           1  2  3  4 │",
                "│  10月 41 │  5  6  7  8  9[10]11 │",
                "│       42 │ 12 13 14 15 16 17 18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  t 今天  q 離開",
            ]
        );
    }

    #[test]
    fn selection_always_stays_two_rows_from_the_edges() {
        // 一天一天往後 40 天、一週一週往後 120 週、再往前 240 週：每畫一次都檢查括號在畫面上、離上下緣至少 2 列
        let mut app = App::new(Lang::En, date(2026, 10, 17));
        let steps = std::iter::repeat_n(1, 40)
            .chain(std::iter::repeat_n(7, 120))
            .chain(std::iter::repeat_n(-7, 240));
        for step in steps {
            app.selected = app.selected.saturating_add(step.days());
            let rows = text_rows(draw_app(&mut app, 40, 14).backend().buffer());
            // 月曆的列：上面是標題、框線、表頭、表頭的線，下面是框線和操作說明
            let body = &rows[4..rows.len() - 2];
            let selected = format!("[{}]", app.selected.day());
            let y = body
                .iter()
                .position(|row| row.contains(&selected))
                .unwrap_or_else(|| panic!("{} 不在畫面上：{body:#?}", app.selected));
            assert!(
                y >= 2 && y + 2 < body.len(),
                "{}：在第 {y} 列",
                app.selected
            );
        }
    }

    #[test]
    fn colors_by_role() {
        let terminal = draw(Lang::ZhTw, 40, 27, date(2026, 10, 17));
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
        // 標題粗體、右邊的日期一般顏色、操作說明淡色、框線
        assert_eq!(cell(1, 0), ("r", theme.label, true));
        assert_eq!(cell(23, 0), ("2", theme.text, false));
        assert_eq!(cell(1, 26), ("h", theme.muted, false));
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
        // 選取的括號
        assert_eq!(cell(27, 6), ("[", theme.selected, true));
        assert_eq!(cell(30, 6), ("]", theme.selected, true));
    }

    #[test]
    fn selected_day_is_bold_in_its_own_color() {
        // 選 10/16（五）：數字是一般的顏色、加粗；今天 10/17 還是今天的顏色
        let terminal = draw(Lang::ZhTw, 40, 27, date(2026, 10, 16));
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::BOLD),
            )
        };
        assert_eq!(cell(24, 6), ("[", theme.selected, true));
        assert_eq!(cell(26, 6), ("6", theme.text, true));
        assert_eq!(cell(27, 6), ("]", theme.selected, true));
        assert_eq!(cell(29, 6), ("7", theme.today, true));
    }
}
