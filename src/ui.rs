//! 畫整個畫面：左邊月曆框、右邊事項框，最下面一列操作說明

mod months;
mod planner;

use crate::app::App;
use jiff::civil::Date;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
};

/// 畫面上每一塊的位置
struct Areas {
    /// 左邊：月曆框
    calendar: Rect,
    /// 右邊：事項框，跟月曆框同一列對齊；表頭右邊寫選取的日期
    agenda: Rect,
    /// 最下面一列：操作說明
    hints: Rect,
}

/// 把整個 terminal（`screen`）切成幾塊；畫畫面和滑鼠換算位置都用這個，兩邊才對得起來
fn areas(screen: Rect) -> Areas {
    let [main, hints] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(screen);
    let [calendar, agenda] = Layout::horizontal([
        Constraint::Length(planner::CALENDAR_WIDTH),
        Constraint::Fill(1),
    ])
    .areas(main);
    Areas {
        calendar,
        agenda,
        hints,
    }
}

/// 滑鼠點到的東西
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// 月曆上的一天
    Day(Date),
    /// 右邊的一件事：`App::agenda()` 裡的位置
    Item(usize),
}

/// 滑鼠點在 terminal 的 (`column`, `row`) 點到什麼；沒點在日期或事上回傳 `None`
pub fn click_at(app: &App, column: u16, row: u16) -> Option<Click> {
    let areas = areas(app.screen);
    planner::click_at(areas.calendar, areas.agenda, app, column, row)
}

/// 畫整個畫面：中間左邊是月曆框、右邊是事項框
pub fn render(frame: &mut Frame, app: &mut App) {
    let texts = app.lang.texts();
    let theme = &app.theme;
    let Areas {
        calendar: calendar_area,
        agenda: agenda_area,
        hints,
    } = areas(frame.area());
    frame.render_widget(
        Line::styled(texts.hints, Style::new().fg(theme.muted)),
        hints,
    );
    // 月曆框、事項框最後畫：它會改 `app`（記住捲到哪），這時候上面借的 `theme` 已經用完了
    planner::render(frame, calendar_area, agenda_area, app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Focus;
    use crate::texts::Lang;
    use crate::theme::Theme;
    use jiff::{
        ToSpan,
        civil::{Date, date},
    };
    use ratatui::{
        Terminal,
        backend::TestBackend,
        buffer::Buffer,
        style::{Color, Modifier},
        text::Span,
    };

    /// 用 `lang` 畫一次 `width` × `height` 的畫面：今天是 2026/10/17、選取 `selected`
    fn draw(lang: Lang, width: u16, height: u16, selected: Date) -> Terminal<TestBackend> {
        let mut app = App::new(lang, date(2026, 10, 17), Vec::new());
        app.selected = selected;
        draw_app(&mut app, width, height)
    }

    /// 滑鼠點在 (`column`, `row`) 點到哪一天：點到月曆的日期就是那天、點到右邊的事就是那件事那天
    fn date_at(app: &App, column: u16, row: u16) -> Option<Date> {
        match click_at(app, column, row)? {
            Click::Day(date) => Some(date),
            Click::Item(index) => app.agenda()[index].date,
        }
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
        // 只有 1 月寫年份（下一列寫月），畫面上有 1 月所以表頭照常寫「年月」；跨年那條是粗線；選取的今天加括號、標題列右邊寫日期
        let terminal = draw(Lang::ZhTw, 35, 26, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  年月 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│  10月 40 │           1  2  3  4 │",
                "│       41 │  5  6  7  8  9 10 11 │",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今",
            ]
        );
    }

    #[test]
    fn calendar_in_english() {
        let terminal = draw(Lang::En, 35, 11, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│   Oct 40 │           1  2  3  4 │",
                "│       41 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16[17]18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "╰─────────────────────────────────╯",
                " hjkl move  Tab switch  [] month  t",
            ]
        );
    }

    #[test]
    fn selection_brackets_and_the_month_on_top() {
        // 選到別的月：那個月排在最上面；畫面上沒有 1 月，表頭寫年份；一位數的日期括號貼著數字
        let terminal = draw(Lang::En, 35, 8, date(2026, 11, 2));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│   Nov 44 │                    1 │",
                "│       45 │ [2] 3  4  5  6  7  8 │",
                "│       46 │  9 10 11 12 13 14 15 │",
                "╰─────────────────────────────────╯",
                " hjkl move  Tab switch  [] month  t",
            ]
        );
        // 星期日：`]` 放在最後多留的那格；月曆只有 3 列、要留 1 列，所以往上多捲一列（跨年的粗線）
        let terminal = draw(Lang::ZhTw, 35, 8, date(2027, 1, 3));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  年月 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2 [3]│",
                "│   1月  2 │  4  5  6  7  8  9 10 │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今",
            ]
        );
    }

    #[test]
    fn calendar_scrolls_down_only_when_the_selection_gets_close_to_the_bottom() {
        // 月曆有 6 列：10/17 → 10/24 → 10/31 → 11/7 一路往下，選取的那列離下緣至少留 2 列，不夠才捲
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        draw_app(&mut app, 35, 11);
        for day in [date(2026, 10, 24), date(2026, 10, 31), date(2026, 11, 7)] {
            app.selected = day;
            draw_app(&mut app, 35, 11);
        }
        // 10 月的前 4 列捲到上面了；畫面上沒有 1 月，表頭寫年份
        assert_eq!(
            text_rows(draw_app(&mut app, 35, 11).backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│       44 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "│   Nov 44 │                    1 │",
                "│       45 │  2  3  4  5  6 [7] 8 │",
                "│       46 │  9 10 11 12 13 14 15 │",
                "│       47 │ 16 17 18 19 20 21 22 │",
                "╰─────────────────────────────────╯",
                " hjkl move  Tab switch  [] month  t",
            ]
        );
        // 往回一週：選取的那列離上緣至少留 2 列，往上捲 2 列
        app.selected = date(2026, 10, 31);
        assert_eq!(
            text_rows(draw_app(&mut app, 35, 11).backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su │",
                "│──────────┼──────────────────────│",
                "│       42 │ 12 13 14 15 16 17 18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30[31]   │",
                "│──────────┼──────────────────────│",
                "│   Nov 44 │                    1 │",
                "│       45 │  2  3  4  5  6  7  8 │",
                "╰─────────────────────────────────╯",
                " hjkl move  Tab switch  [] month  t",
            ]
        );
    }

    #[test]
    fn calendar_scrolls_up_into_earlier_months() {
        // 10/17 往上一週到 10/10：上面只剩 1 列，往上捲 1 列，露出 9 月後面的線
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        draw_app(&mut app, 35, 11);
        app.selected = date(2026, 10, 10);
        assert_eq!(
            text_rows(draw_app(&mut app, 35, 11).backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│──────────┼──────────────────────│",
                "│  10月 40 │           1  2  3  4 │",
                "│       41 │  5  6  7  8  9[10]11 │",
                "│       42 │ 12 13 14 15 16 17 18 │",
                "│       43 │ 19 20 21 22 23 24 25 │",
                "│       44 │ 26 27 28 29 30 31    │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今",
            ]
        );
    }

    #[test]
    fn selection_always_stays_two_rows_from_the_edges() {
        // 一天一天往後 40 天、一週一週往後 120 週、再往前 240 週：每畫一次都檢查括號在畫面上、離上下緣至少 2 列
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        let steps = std::iter::repeat_n(1, 40)
            .chain(std::iter::repeat_n(7, 120))
            .chain(std::iter::repeat_n(-7, 240));
        for step in steps {
            app.selected = app.selected.saturating_add(step.days());
            let rows = text_rows(draw_app(&mut app, 40, 13).backend().buffer());
            // 月曆的列：上面是標題、框線、表頭、表頭的線，下面是框線和操作說明
            let body = &rows[3..rows.len() - 2];
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
    fn click_positions_map_to_the_dates_on_screen() {
        // 40 × 26、10 月在最上面（畫面見 `calendar_from_this_month_down_with_a_thick_line_between_years`）
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 40, 26);
        draw_app(&mut app, 40, 26);
        // 一天 3 格，前面的空白也算那天：16 號是 x 24～26
        assert_eq!(date_at(&app, 24, 5), Some(date(2026, 10, 16)));
        assert_eq!(date_at(&app, 26, 5), Some(date(2026, 10, 16)));
        assert_eq!(date_at(&app, 27, 5), Some(date(2026, 10, 17)));
        assert_eq!(date_at(&app, 32, 5), Some(date(2026, 10, 18)));
        // 11/1 在 11 月第一列的最後一格；12/31 是星期四
        assert_eq!(date_at(&app, 31, 9), Some(date(2026, 11, 1)));
        assert_eq!(date_at(&app, 22, 20), Some(date(2026, 12, 31)));
        // 沒有日期：最後多留的那格、10/1 前面的空格、月和月之間的線、年月欄、週數欄、表頭、框線、操作說明、右邊空白
        for (x, y) in [
            (33, 5),
            (13, 3),
            (20, 8),
            (5, 5),
            (9, 5),
            (20, 1),
            (0, 5),
            (20, 25),
            (38, 5),
        ] {
            assert_eq!(date_at(&app, x, y), None, "({x}, {y})");
        }
    }

    #[test]
    fn click_positions_follow_the_scroll() {
        // 捲到 11/7（畫面見 `calendar_scrolls_down_only_when_the_selection_gets_close_to_the_bottom`）
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 40, 11);
        draw_app(&mut app, 40, 11);
        for day in [date(2026, 10, 24), date(2026, 10, 31), date(2026, 11, 7)] {
            app.selected = day;
            draw_app(&mut app, 40, 11);
        }
        // 最上面那列是 10/26 那週、下一列是線、再來是 11/1、11/2 那週
        assert_eq!(date_at(&app, 13, 3), Some(date(2026, 10, 26)));
        assert_eq!(date_at(&app, 13, 4), None);
        assert_eq!(date_at(&app, 31, 5), Some(date(2026, 11, 1)));
        assert_eq!(date_at(&app, 13, 6), Some(date(2026, 11, 2)));
    }

    /// 事項框 test 用的任務檔（跟產生預期畫面的 Python 程式用同一份）
    const AGENDA_TASKS: &str = "- [ ] 交週報 @2026-10-16 17:30\n\
                                - [x] 修 bug @2026-10-15\n\
                                - 聚餐 @2026-10-17\n\
                                - [ ] 早餐會 @2026-10-17 08:00\n\
                                - [ ] 爬山 @2026-10-18 07:00\n\
                                - [ ] 繳電話費 @2026-10-05\n\
                                - [ ] 國慶日 @2026-10-10\n\
                                - [ ] 一個很長很長很長很長很長很長很長的標題 @2026-10-20\n\
                                - [ ] 沒日期的事\n\
                                - [ ] 跨年倒數 @2026-12-31 23:00\n\
                                - [ ] Release v2.3 @2026-11-03 09:30\n";

    /// 今天 2026/10/17、選取今天、任務檔是 `AGENDA_TASKS` 的 app
    fn agenda_app(lang: Lang) -> App {
        App::new(
            lang,
            date(2026, 10, 17),
            crate::tasks::parse(AGENDA_TASKS).items,
        )
    }

    #[test]
    fn agenda_lists_each_months_items_next_to_it() {
        // 右邊一欄：10 月 8 件事比 5 週多，月份往下長 3 列；照日期、時間排（同一天沒時間的在前）；
        // 沒日期的不列；太長的標題被切掉
        let mut app = agenda_app(Lang::ZhTw);
        assert_eq!(
            text_rows(draw_app(&mut app, 70, 23).backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/17（六） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││  10/5 繳電話費                  │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/10 國慶日                    │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/15 修 bug                    │",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/16 17:30 交週報              │",
                "│       44 │ 26 27 28 29 30 31    ││ 10/17 聚餐                      │",
                "│          │                      ││ 10/17 08:00 早餐會              │",
                "│          │                      ││ 10/18 07:00 爬山                │",
                "│          │                      ││ 10/20 一個很長很長很長很長很長很│",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  11月 44 │                    1 ││  11/3 09:30 Release v2.3        │",
                "│       45 │  2  3  4  5  6  7  8 ││                                 │",
                "│       46 │  9 10 11 12 13 14 15 ││                                 │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                 │",
                "│       48 │ 23 24 25 26 27 28 29 ││                                 │",
                "│       49 │ 30                   ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 ││ 12/31 23:00 跨年倒數            │",
                "│       50 │  7  8  9 10 11 12 13 ││                                 │",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
    }

    #[test]
    fn agenda_flows_into_columns_when_wide() {
        // 右邊 58 格寬：分 2 欄，像報紙一樣先排滿第一欄（10 月 5 列）再排第二欄
        let mut app = agenda_app(Lang::En);
        assert_eq!(
            text_rows(draw_app(&mut app, 95, 15).backend().buffer()),
            [
                "╭─────────────────────────────────╮╭──────────────────────────────────────────────────────────╮",
                "│  2026 Wk │ Mo Tu We Th Fr Sa Su ││ Agenda                                  Sat Oct 17, 2026 │",
                "│──────────┼──────────────────────││──────────────────────────────────────────────────────────│",
                "│   Oct 40 │           1  2  3  4 ││  10/5 繳電話費                10/17 08:00 早餐會         │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/10 國慶日                  10/18 07:00 爬山           │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/15 修 bug                  10/20 一個很長很長很長很長 │",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/16 17:30 交週報                                       │",
                "│       44 │ 26 27 28 29 30 31    ││ 10/17 聚餐                                               │",
                "│──────────┼──────────────────────││──────────────────────────────────────────────────────────│",
                "│   Nov 44 │                    1 ││  11/3 09:30 Release v2.3                                 │",
                "│       45 │  2  3  4  5  6  7  8 ││                                                          │",
                "│       46 │  9 10 11 12 13 14 15 ││                                                          │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                                          │",
                "╰─────────────────────────────────╯╰──────────────────────────────────────────────────────────╯",
                " hjkl move  Tab switch  [] month  t today  q quit",
            ]
        );
    }

    #[test]
    fn clicking_an_item_selects_its_day() {
        // 一欄（畫面見 `agenda_lists_each_months_items_next_to_it`）：右邊框裡面從 x 36 開始
        let mut app = agenda_app(Lang::ZhTw);
        app.screen = Rect::new(0, 0, 70, 23);
        draw_app(&mut app, 70, 23);
        assert_eq!(date_at(&app, 40, 3), Some(date(2026, 10, 5)));
        assert_eq!(date_at(&app, 68, 8), Some(date(2026, 10, 17)));
        assert_eq!(date_at(&app, 40, 12), Some(date(2026, 11, 3)));
        // 右邊沒有事的列、表頭；左邊只有事的那幾列沒有日期
        assert_eq!(date_at(&app, 40, 13), None);
        assert_eq!(date_at(&app, 40, 1), None);
        assert_eq!(date_at(&app, 20, 8), None);
        // 點到的是右邊的第幾件事（`App::agenda()` 的順序）
        assert_eq!(click_at(&app, 40, 3), Some(Click::Item(0)));
        assert_eq!(click_at(&app, 20, 5), Some(Click::Day(date(2026, 10, 14))));
        // 兩欄（畫面見 `agenda_flows_into_columns_when_wide`）：第二欄從 x 36 + 28 + 2 = 66 開始，中間 2 格空白不算
        let mut app = agenda_app(Lang::En);
        app.screen = Rect::new(0, 0, 95, 15);
        draw_app(&mut app, 95, 15);
        assert_eq!(date_at(&app, 70, 3), Some(date(2026, 10, 17)));
        assert_eq!(date_at(&app, 63, 3), Some(date(2026, 10, 5)));
        assert_eq!(date_at(&app, 64, 3), None);
        assert_eq!(date_at(&app, 65, 3), None);
        assert_eq!(date_at(&app, 66, 3), Some(date(2026, 10, 17)));
    }

    #[test]
    fn selection_stays_on_screen_with_tall_months() {
        // 範例任務檔有的月比週數高：一週一週往後 60 週、往前 120 週，括號都在畫面上、離上下緣至少 2 列
        let mut app = App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(include_str!("../testdata/tasks.md")).items,
        );
        let steps = std::iter::repeat_n(7, 60).chain(std::iter::repeat_n(-7, 120));
        for step in steps {
            app.selected = app.selected.saturating_add(step.days());
            let rows = text_rows(draw_app(&mut app, 70, 13).backend().buffer());
            let body = &rows[3..rows.len() - 2];
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
    fn item_colors_follow_their_day() {
        // 畫面見 `agenda_lists_each_months_items_next_to_it`：右邊框裡面從 x 36 開始
        let mut app = agenda_app(Lang::ZhTw);
        let terminal = draw_app(&mut app, 70, 23);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // (x, y) 那格：字、前景色、有沒有劃線
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::CROSSED_OUT),
            )
        };
        // 右邊的表頭：「重要事項」淡色、右邊選取的日期一般顏色（2026/10/17 從 x 52 開始）
        assert_eq!(cell(37, 1), ("重", theme.muted, false));
        assert_eq!(cell(52, 1), ("2", theme.text, false));
        // 10/5 有沒做完的事：日期、標題都跟月曆上一樣是橘色
        assert_eq!(cell(38, 3), ("1", theme.has_item, false));
        assert_eq!(cell(43, 3), ("繳", theme.has_item, false));
        // 10/15 做完的事：整行變灰（日期也是），標題劃線
        assert_eq!(cell(37, 5), ("1", theme.muted, false));
        assert_eq!(cell(43, 5), ("修", theme.muted, true));
        // 10/17 今天：綠色；時間淡色
        assert_eq!(cell(43, 7), ("聚", theme.today, false));
        assert_eq!(cell(43, 8), ("0", theme.muted, false));
        assert_eq!(cell(49, 8), ("早", theme.today, false));
        // 10/18（日）有沒做完的事：橘色蓋過週末
        assert_eq!(cell(49, 9), ("爬", theme.has_item, false));
    }

    #[test]
    fn the_selected_week_row_has_a_background() {
        // 10/17 那週在 y 5：月曆框裡整列（x 1～33）墊底色；上下列、右邊框的同一列不墊
        let mut app = agenda_app(Lang::ZhTw);
        let theme = Theme::catppuccin_mocha();
        let bg = |terminal: &Terminal<TestBackend>, x: u16, y: u16| {
            terminal.backend().buffer()[(x, y)].bg
        };
        let terminal = draw_app(&mut app, 70, 23);
        for x in 1..=33 {
            assert_eq!(bg(&terminal, x, 5), theme.cursor_line, "x {x}");
        }
        assert_eq!(bg(&terminal, 0, 5), Color::Reset);
        assert_eq!(bg(&terminal, 20, 4), Color::Reset);
        assert_eq!(bg(&terminal, 20, 6), Color::Reset);
        assert_eq!(bg(&terminal, 40, 5), Color::Reset);
        // 跨月那週拆成兩列：選 10/31，只有 10 月那一半（y 7）墊底色，11 月那一半（y 12）不墊
        app.selected = date(2026, 10, 31);
        let terminal = draw_app(&mut app, 70, 23);
        assert_eq!(bg(&terminal, 20, 7), theme.cursor_line);
        assert_eq!(bg(&terminal, 20, 12), Color::Reset);
    }

    #[test]
    fn agenda_focus_brackets_the_chosen_item() {
        // 鍵盤在右邊、選到第 6 件（10/17 08:00 早餐會）：日期加括號
        let mut app = agenda_app(Lang::ZhTw);
        app.focus = Focus::Agenda;
        app.agenda_index = 5;
        let terminal = draw_app(&mut app, 70, 23);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/17（六） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││  10/5 繳電話費                  │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/10 國慶日                    │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/15 修 bug                    │",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/16 17:30 交週報              │",
                "│       44 │ 26 27 28 29 30 31    ││ 10/17 聚餐                      │",
                "│          │                      ││[10/17]08:00 早餐會              │",
                "│          │                      ││ 10/18 07:00 爬山                │",
                "│          │                      ││ 10/20 一個很長很長很長很長很長很│",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  11月 44 │                    1 ││  11/3 09:30 Release v2.3        │",
                "│       45 │  2  3  4  5  6  7  8 ││                                 │",
                "│       46 │  9 10 11 12 13 14 15 ││                                 │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                 │",
                "│       48 │ 23 24 25 26 27 28 29 ││                                 │",
                "│       49 │ 30                   ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 ││ 12/31 23:00 跨年倒數            │",
                "│       50 │  7  8  9 10 11 12 13 ││                                 │",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // 右邊框的框線亮起來、月曆框的變回一般的線
        assert_eq!(buffer[(35, 0)].fg, theme.focus);
        assert_eq!(buffer[(0, 0)].fg, theme.line);
        // 括號是選取的顏色；選到的那件整欄墊底色（x 36～68），上下那件不墊
        assert_eq!(
            (buffer[(36, 8)].symbol(), buffer[(36, 8)].fg),
            ("[", theme.selected)
        );
        assert_eq!(
            (buffer[(42, 8)].symbol(), buffer[(42, 8)].fg),
            ("]", theme.selected)
        );
        // 寬字元（中文）後半那格不會送到 terminal，跳過（真的 terminal 上底色會蓋滿兩格）
        let mut x = 36;
        while x <= 68 {
            assert_eq!(buffer[(x, 8)].bg, theme.cursor_line, "x {x}");
            x += u16::try_from(Span::raw(buffer[(x, 8)].symbol()).width().max(1)).unwrap();
        }
        assert_eq!(buffer[(40, 7)].bg, Color::Reset);
        assert_eq!(buffer[(40, 9)].bg, Color::Reset);
    }

    #[test]
    fn the_chosen_item_is_always_on_screen() {
        // 範例任務檔：10 月的事比週數多很多，一欄時 10 月那一塊很高；一件一件往下選、再往上選，
        // 選到的那件（日期有括號）都在畫面上，離上下緣至少 2 列
        let mut app = App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(include_str!("../testdata/tasks.md")).items,
        );
        app.focus = Focus::Agenda;
        let count = app.agenda().len();
        for index in (0..count).chain((0..count).rev()) {
            app.agenda_index = index;
            app.selected = app.agenda()[index].date.unwrap();
            let rows = text_rows(draw_app(&mut app, 70, 13).backend().buffer());
            let body = &rows[3..rows.len() - 2];
            let chosen = format!("[{}/{}]", app.selected.month(), app.selected.day());
            let y = body
                .iter()
                .position(|row| row.contains(&chosen))
                .unwrap_or_else(|| panic!("第 {index} 件不在畫面上：{body:#?}"));
            assert!(y >= 2 && y + 2 < body.len(), "第 {index} 件：在第 {y} 列");
        }
    }

    #[test]
    fn colors_by_role() {
        let terminal = draw(Lang::ZhTw, 40, 26, date(2026, 10, 17));
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
        // 操作說明淡色
        assert_eq!(cell(1, 25), ("h", theme.muted, false));
        // 鍵盤在月曆：月曆框的框線用 focus 的顏色
        assert_eq!(cell(0, 0), ("╭", theme.focus, false));
        // 表頭：平日淡色、六日用週末的顏色、直線
        assert_eq!(cell(3, 1), ("年", theme.muted, false));
        assert_eq!(cell(13, 1), ("一", theme.muted, false));
        assert_eq!(cell(28, 1), ("六", theme.weekend, false));
        assert_eq!(cell(31, 1), ("日", theme.weekend, false));
        assert_eq!(cell(11, 1), ("│", theme.line, false));
        // 月和月之間的線、跨年的粗線
        assert_eq!(cell(11, 2), ("┼", theme.line, false));
        assert_eq!(cell(5, 21), ("━", theme.year_line, false));
        assert_eq!(cell(11, 21), ("┿", theme.year_line, false));
        // 月（10月）、1 月的年（2027年）、週數
        assert_eq!(cell(3, 3), ("1", theme.label, false));
        assert_eq!(cell(1, 22), ("2", theme.label, false));
        // 週數：選取那週（42）亮、其他週（41）暗
        assert_eq!(cell(8, 5), ("4", theme.week_number, false));
        assert_eq!(cell(8, 4), ("4", theme.week_number_dim, false));
        // 10/16（五）一般；10/17（六）是今天：今天的顏色蓋過週末，而且粗體；10/18（日）週末
        assert_eq!(cell(25, 5), ("1", theme.text, false));
        assert_eq!(cell(29, 5), ("7", theme.today, true));
        assert_eq!(cell(32, 5), ("8", theme.weekend, false));
        // 選取的括號
        assert_eq!(cell(27, 5), ("[", theme.selected, true));
        assert_eq!(cell(30, 5), ("]", theme.selected, true));
    }

    #[test]
    fn year_hint_in_the_header_is_muted() {
        // 畫面上沒有 1 月：表頭 `Wk` 左邊寫 2026，跟表頭一樣淡（畫面見 `calendar_in_english`）
        let terminal = draw(Lang::En, 40, 11, date(2026, 10, 17));
        let cell = &terminal.backend().buffer()[(3, 1)];
        assert_eq!(
            (cell.symbol(), cell.fg),
            ("2", Theme::catppuccin_mocha().muted)
        );
    }

    #[test]
    fn days_with_open_items_are_orange() {
        // 16 號有沒做完的事 → 橘；15 號只有做完的 → 跟沒事一樣；17 號（今天）有事 → 還是今天的顏色；
        // 18 號（日）有事 → 橘蓋過週末；14 號沒事
        let text = "- [ ] 交週報 @2026-10-16\n- [x] 修 bug @2026-10-15\n- 聚餐 @2026-10-17\n\
                    - [ ] 爬山 @2026-10-18 07:00\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        let terminal = draw_app(&mut app, 40, 26);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let fg = |x: u16| (buffer[(x, 5)].symbol(), buffer[(x, 5)].fg);
        assert_eq!(fg(20), ("4", theme.text));
        assert_eq!(fg(23), ("5", theme.text));
        assert_eq!(fg(26), ("6", theme.has_item));
        assert_eq!(fg(29), ("7", theme.today));
        assert_eq!(fg(32), ("8", theme.has_item));
    }

    #[test]
    fn selected_day_is_bold_in_its_own_color() {
        // 選 10/16（五）：數字是一般的顏色、加粗；今天 10/17 還是今天的顏色
        let terminal = draw(Lang::ZhTw, 40, 26, date(2026, 10, 16));
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
        assert_eq!(cell(24, 5), ("[", theme.selected, true));
        assert_eq!(cell(26, 5), ("6", theme.text, true));
        assert_eq!(cell(27, 5), ("]", theme.selected, true));
        assert_eq!(cell(29, 5), ("7", theme.today, true));
    }
}
