//! 畫整個畫面：最左邊 side menu（夠寬才有），右邊是版型 1（月曆框 + 事項框）或版型 3（大月曆），最下面一列操作說明

mod month;
mod months;
mod planner;
mod side;

use crate::app::{App, View};
use jiff::civil::Date;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    text::Line,
};

/// terminal 至少這麼寬才畫 side menu
const SIDE_MIN_WIDTH: u16 = 120;

/// terminal（`screen`）夠不夠寬畫 side menu
pub fn side_fits(screen: Rect) -> bool {
    screen.width >= SIDE_MIN_WIDTH
}

/// 畫面上每一塊的位置
struct Areas {
    /// 最左邊：side menu（日曆清單）；沒畫的時候寬度是 0
    side: Rect,
    /// side menu 右邊、操作說明上面的整塊：版型 3 的大月曆畫在這裡
    main: Rect,
    /// 版型 1 的左邊：月曆框
    calendar: Rect,
    /// 版型 1 的右邊：事項框，跟月曆框同一列對齊；表頭右邊寫選取的日期
    agenda: Rect,
    /// 最下面一列：操作說明
    hints: Rect,
}

/// 把整個 terminal（`screen`）切成幾塊；畫畫面和滑鼠換算位置都用這個，兩邊才對得起來
///
/// side menu 只在 terminal 夠寬、沒按 `s` 收起時才有，寬度跟著日曆的名字
fn areas(screen: Rect, app: &App) -> Areas {
    let [main, hints] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(screen);
    let side_width = if side_fits(screen) && !app.side_hidden {
        side::width(app)
    } else {
        0
    };
    let [side, main] =
        Layout::horizontal([Constraint::Length(side_width), Constraint::Fill(1)]).areas(main);
    let [calendar, agenda] = Layout::horizontal([
        Constraint::Length(planner::CALENDAR_WIDTH),
        Constraint::Fill(1),
    ])
    .areas(main);
    Areas {
        side,
        main,
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
    /// side menu 上的一個日曆：`App::groups` 裡的位置
    Group(usize),
}

/// 滑鼠點在 terminal 的 (`column`, `row`) 點到什麼；沒點在日期、事、日曆上回傳 `None`
pub fn click_at(app: &App, column: u16, row: u16) -> Option<Click> {
    let areas = areas(app.screen, app);
    if areas.side.contains(Position::new(column, row)) {
        return side::click_at(areas.side, app, column, row);
    }
    match app.view {
        View::Planner => planner::click_at(areas.calendar, areas.agenda, app, column, row),
        View::Month => month::click_at(areas.main, app, column, row),
    }
}

/// 畫整個畫面：中間最左邊是 side menu（夠寬才有），右邊是版型 1 的月曆框 + 事項框，或版型 3 的大月曆
pub fn render(frame: &mut Frame, app: &mut App) {
    let texts = app.lang.texts();
    let theme = &app.theme;
    let Areas {
        side: side_area,
        main: main_area,
        calendar: calendar_area,
        agenda: agenda_area,
        hints,
    } = areas(frame.area(), app);
    // 夠寬時操作說明多一個 `s`（收起 / 打開 side menu）
    let hint_text = if side_fits(frame.area()) {
        texts.hints_side
    } else {
        texts.hints
    };
    frame.render_widget(Line::styled(hint_text, Style::new().fg(theme.muted)), hints);
    if side_area.width > 0 {
        side::render(frame, side_area, app);
    }
    // 月曆最後畫：它會改 `app`（記住捲到哪），這時候上面借的 `theme` 已經用完了
    match app.view {
        View::Planner => planner::render(frame, calendar_area, agenda_area, app),
        View::Month => month::render(frame, main_area, app),
    }
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
            Click::Group(_) => None,
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
                "│2026年 週 │ 一 二 三 四 五 六 日 │",
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
                "│2027年 週 │ 一 二 三 四 五 六 日 │",
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

    /// 事項框 test 用的任務檔（`tools/expected_screen.py` 算預期畫面時也讀這一份）
    const AGENDA_TASKS: &str = include_str!("../testdata/agenda.md");

    /// 今天 2026/10/17、選取今天、任務檔是 `AGENDA_TASKS` 的 app
    fn agenda_app(lang: Lang) -> App {
        App::new(
            lang,
            date(2026, 10, 17),
            crate::tasks::parse(AGENDA_TASKS).items,
        )
    }

    #[test]
    fn the_side_menu_needs_120_columns() {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        assert_eq!(areas(Rect::new(0, 0, 119, 20), &app).side.width, 0);
        assert_eq!(areas(Rect::new(0, 0, 120, 20), &app).side.width, 22);
        // 按 `s` 收起：月曆貼回最左邊
        app.side_hidden = true;
        let hidden = areas(Rect::new(0, 0, 120, 20), &app);
        assert_eq!((hidden.side.width, hidden.calendar.x), (0, 0));
    }

    #[test]
    fn side_menu_lists_the_calendars_when_wide() {
        // 121 格寬：最左邊列出日曆（任務檔的分組，照出現的順序；標題之前的是「未分組」），
        // 數字是有日期的事有幾件（「工作」有一件沒日期）；最後一個名字太長，框寬到 30 格還放不下，切掉；
        // 操作說明多一個 `s`。右邊只剩一欄、10 月的 8 件放不下：捲 1 列，選取那天（10/17）的事後面多留一列
        let mut app = agenda_app(Lang::ZhTw);
        let terminal = draw_app(&mut app, 121, 14);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────────────╮╭─────────────────────────────────╮╭──────────────────────────────────────────────────────╮",
                "│ 日曆                       ││2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項                            2026/10/17（六） │",
                "│────────────────────────────││──────────┼──────────────────────││──────────────────────────────────────────────────────│",
                "│ ☑ 未分組                 2 ││  10月 40 │           1  2  3  4 ││ 10/10 國慶日                                         │",
                "│ ☑ 生活                   5 ││       41 │  5  6  7  8  9 10 11 ││ 10/15 修 bug                                         ┃",
                "│ ☑ 工作                   1 ││       42 │ 12 13 14 15 16[17]18 ││ 10/16 17:30 交週報                                   ┃",
                "│ ☑ 2026 年底的大小事情和  2 ││       43 │ 19 20 21 22 23 24 25 ││ 10/17 聚餐                                           ┃",
                "│                            ││       44 │ 26 27 28 29 30 31    ││ 10/17 08:00 早餐會                                   ┃",
                "│                            ││          │                      ││ 10/18 07:00 爬山                                     │",
                "│                            ││──────────┼──────────────────────││────────────────────────────────────────── 還有 1 件 ─│",
                "│                            ││  11月 44 │                    1 ││  11/3 09:30 Release v2.3                             │",
                "│                            ││       45 │  2  3  4  5  6  7  8 ││                                                      │",
                "╰────────────────────────────╯╰─────────────────────────────────╯╰──────────────────────────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  q 離開",
            ]
        );
        // ☑ 用 `checked` 的顏色、名字是一般的字、數字和表頭淡色、框線是一般的線
        let theme = Theme::catppuccin_mocha();
        let buffer = terminal.backend().buffer();
        let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
        assert_eq!(cell(2, 3), ("☑", theme.checked));
        assert_eq!(cell(4, 4), ("生", theme.text));
        assert_eq!(cell(27, 4), ("5", theme.muted));
        assert_eq!(cell(2, 1), ("日", theme.muted));
        assert_eq!(cell(0, 0), ("╭", theme.line));
        // 按 `s` 收起：月曆貼回最左邊，操作說明照樣有 `s`（再按一次打開）
        app.side_hidden = true;
        assert_eq!(
            text_rows(draw_app(&mut app, 121, 14).backend().buffer()),
            [
                "╭─────────────────────────────────╮╭────────────────────────────────────────────────────────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項                                                          2026/10/17（六） │",
                "│──────────┼──────────────────────││────────────────────────────────────────────────────────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││  10/5 繳電話費                             10/17 08:00 早餐會                      │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/10 國慶日                               10/18 07:00 爬山                        │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/15 修 bug                               10/20 一個很長很長很長很長很長很長很長的│",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/16 17:30 交週報                                                                 │",
                "│       44 │ 26 27 28 29 30 31    ││ 10/17 聚餐                                                                         │",
                "│──────────┼──────────────────────││────────────────────────────────────────────────────────────────────────────────────│",
                "│  11月 44 │                    1 ││  11/3 09:30 Release v2.3                                                           │",
                "│       45 │  2  3  4  5  6  7  8 ││                                                                                    │",
                "│       46 │  9 10 11 12 13 14 15 ││                                                                                    │",
                "╰─────────────────────────────────╯╰────────────────────────────────────────────────────────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  q 離開",
            ]
        );
    }

    #[test]
    fn unchecked_calendars_hide_their_items() {
        // 「生活」沒勾：☐ 和名字淡色、數字照算；它的 5 件事不列，10/5、10/10 也不是橘色了
        let mut app = agenda_app(Lang::En);
        app.groups[1].shown = false;
        let terminal = draw_app(&mut app, 121, 14);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────────────╮╭─────────────────────────────────╮╭──────────────────────────────────────────────────────╮",
                "│ Calendars                  ││  2026 Wk │ Mo Tu We Th Fr Sa Su ││ Agenda                              Sat Oct 17, 2026 │",
                "│────────────────────────────││──────────┼──────────────────────││──────────────────────────────────────────────────────│",
                "│ ☑ Ungrouped              2 ││   Oct 40 │           1  2  3  4 ││ 10/15 修 bug                                         │",
                "│ ☐ 生活                   5 ││       41 │  5  6  7  8  9 10 11 ││ 10/16 17:30 交週報                                   │",
                "│ ☑ 工作                   1 ││       42 │ 12 13 14 15 16[17]18 ││ 10/20 一個很長很長很長很長很長很長很長的標題         │",
                "│ ☑ 2026 年底的大小事情和  2 ││       43 │ 19 20 21 22 23 24 25 ││                                                      │",
                "│                            ││       44 │ 26 27 28 29 30 31    ││                                                      │",
                "│                            ││──────────┼──────────────────────││──────────────────────────────────────────────────────│",
                "│                            ││   Nov 44 │                    1 ││  11/3 09:30 Release v2.3                             │",
                "│                            ││       45 │  2  3  4  5  6  7  8 ││                                                      │",
                "│                            ││       46 │  9 10 11 12 13 14 15 ││                                                      │",
                "╰────────────────────────────╯╰─────────────────────────────────╯╰──────────────────────────────────────────────────────╯",
                " hjkl move  Tab switch  [] month  t today  s calendars  q quit",
            ]
        );
        let theme = Theme::catppuccin_mocha();
        let buffer = terminal.backend().buffer();
        let cell = |x: u16, y: u16| (buffer[(x, y)].symbol(), buffer[(x, y)].fg);
        assert_eq!(cell(2, 4), ("☐", theme.muted));
        assert_eq!(cell(4, 4), ("生", theme.muted));
        assert_eq!(cell(2, 3), ("☑", theme.checked));
        // 10/5（星期一）、10/10（星期六）回到平常的顏色；10/16 是沒分組的事，照樣橘色
        assert_eq!(cell(44, 4), ("5", theme.text));
        assert_eq!(cell(59, 4), ("0", theme.weekend));
        assert_eq!(cell(56, 5), ("6", theme.has_item));
    }

    #[test]
    fn side_menu_focus_lights_its_border_and_the_chosen_calendar() {
        // 鍵盤在 side menu、選到第 2 個（生活）：side menu 的框線亮、月曆的不亮；那一列整列墊底色，字的顏色不變
        let mut app = agenda_app(Lang::ZhTw);
        app.focus = Focus::Side;
        app.group_index = 1;
        let terminal = draw_app(&mut app, 121, 14);
        let theme = Theme::catppuccin_mocha();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].fg, theme.focus);
        assert_eq!(buffer[(30, 0)].fg, theme.line);
        // 中文字後半那格（「生」「活」的 x 5、7）TestBackend 拿不到樣式，跳過
        for x in (1..29).filter(|x| ![5, 7].contains(x)) {
            assert_eq!(buffer[(x, 4)].bg, theme.cursor_line, "x = {x}");
            assert_ne!(buffer[(x, 3)].bg, theme.cursor_line, "x = {x}");
        }
        assert_eq!(
            (buffer[(2, 4)].symbol(), buffer[(2, 4)].fg),
            ("☑", theme.checked)
        );
    }

    #[test]
    fn click_positions_in_the_side_menu() {
        // 121 × 14（畫面見 `side_menu_lists_the_calendars_when_wide`）：日曆從 y 3 開始，一個一列
        let mut app = agenda_app(Lang::ZhTw);
        app.screen = Rect::new(0, 0, 121, 14);
        draw_app(&mut app, 121, 14);
        assert_eq!(click_at(&app, 5, 3), Some(Click::Group(0)));
        assert_eq!(click_at(&app, 1, 6), Some(Click::Group(3)));
        assert_eq!(click_at(&app, 20, 4), Some(Click::Group(1)));
        // 沒有日曆：下面的空白、表頭、表頭的線、左右框線、操作說明
        for (x, y) in [(5, 7), (5, 1), (5, 2), (0, 3), (29, 3), (5, 13)] {
            assert_eq!(click_at(&app, x, y), None, "({x}, {y})");
        }
        // 月曆往右移了 30 格：16 號在 x 54～56
        assert_eq!(date_at(&app, 54, 5), Some(date(2026, 10, 16)));
        assert_eq!(date_at(&app, 57, 5), Some(date(2026, 10, 17)));
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
    fn the_selected_day_and_its_items_stay_on_screen_with_tall_months() {
        // 範例任務檔有的月比週數高（10 月很多事）：鍵盤在月曆，一天一天往後 120 天、往前 240 天，
        // 每畫一次都檢查：月曆上看得到選取那天的括號；那天有事的話，右邊看得到那天的第一件
        let mut app = App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(include_str!("../testdata/tasks.md")).items,
        );
        app.selected = date(2026, 9, 1);
        let steps = std::iter::repeat_n(1, 120).chain(std::iter::repeat_n(-1, 240));
        for step in steps {
            app.selected = app.selected.saturating_add(step.days());
            let rows = text_rows(draw_app(&mut app, 70, 13).backend().buffer());
            let body = &rows[3..rows.len() - 2];
            // 每一列 `││` 左邊是月曆框、右邊是事項框
            let (left, right): (Vec<&str>, Vec<&str>) = body
                .iter()
                .map(|row| row.split_once("││").unwrap_or((row, "")))
                .unzip();
            let day = format!("[{}]", app.selected.day());
            assert!(
                left.iter().any(|row| row.contains(&day)),
                "{}：月曆上看不到 {day}：{body:#?}",
                app.selected
            );
            let has_items = app
                .agenda()
                .iter()
                .any(|item| item.date == Some(app.selected));
            let date = format!(" {}/{} ", app.selected.month(), app.selected.day());
            assert!(
                !has_items || right.iter().any(|row| row.contains(&date)),
                "{}：右邊看不到那天的事：{body:#?}",
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
        // 範例任務檔：10 月的事比週數多很多（一個月放不下，右邊要在裡面捲）；一件一件往下選、再往上選，
        // 選到的那件（日期有括號）和月曆上選取的那天都在畫面上
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
            assert!(
                body.iter().any(|row| row.contains(&chosen)),
                "第 {index} 件不在畫面上：{body:#?}"
            );
            // 月曆框（每一列 `││` 左邊那半）上也看得到選取的那天
            let day = format!("[{}]", app.selected.day());
            assert!(
                body.iter().any(|row| row
                    .split("││")
                    .next()
                    .is_some_and(|left| left.contains(&day))),
                "第 {index} 件：月曆上看不到 {day}：{body:#?}"
            );
        }
    }

    #[test]
    fn a_capped_month_scrolls_inside_while_choosing_items() {
        // 月曆 6 列 → 一個月最多 3 列，但至少是週數（10 月 5 週）：10 月 8 件事只放得下 5 件。
        // 鍵盤在右邊、選到第 8 件（10/20）：10 月的右邊捲 3 列，月曆不動
        let mut app = agenda_app(Lang::ZhTw);
        app.screen = Rect::new(0, 0, 70, 11);
        app.focus = Focus::Agenda;
        app.agenda_index = 7;
        app.selected = date(2026, 10, 20);
        let terminal = draw_app(&mut app, 70, 11);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/20（二） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││ 10/16 17:30 交週報              │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/17 聚餐                      │",
                "│       42 │ 12 13 14 15 16 17 18 ││ 10/17 08:00 早餐會              ┃",
                "│       43 │ 19[20]21 22 23 24 25 ││ 10/18 07:00 爬山                ┃",
                "│       44 │ 26 27 28 29 30 31    ││[10/20]一個很長很長很長很長很長很┃",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
        assert_eq!(app.calendar_top, Some(0));
        // 捲軸的亮塊跟框線同一個顏色（鍵盤在右邊：focus 色）
        let theme = Theme::catppuccin_mocha();
        let cell = &terminal.backend().buffer()[(69, 5)];
        assert_eq!((cell.symbol(), cell.fg), ("┃", theme.focus));
        // 點擊跟畫面一樣：右邊第一列是第 4 件（10/16 交週報）
        assert_eq!(click_at(&app, 40, 3), Some(Click::Item(3)));
        // 選回第 1 件（10/5）：右邊捲回最上面
        app.agenda_index = 0;
        app.selected = date(2026, 10, 5);
        draw_app(&mut app, 70, 11);
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 0)));
    }

    #[test]
    fn the_agenda_follows_the_calendar() {
        // 鍵盤在月曆、10 月只放得下 5 件：選 10/20，右邊捲到看得到 10/20 那件，月曆不動
        let mut app = agenda_app(Lang::ZhTw);
        draw_app(&mut app, 70, 11);
        app.selected = date(2026, 10, 20);
        let terminal = draw_app(&mut app, 70, 11);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/20（二） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││ 10/16 17:30 交週報              │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/17 聚餐                      │",
                "│       42 │ 12 13 14 15 16 17 18 ││ 10/17 08:00 早餐會              ┃",
                "│       43 │ 19[20]21 22 23 24 25 ││ 10/18 07:00 爬山                ┃",
                "│       44 │ 26 27 28 29 30 31    ││ 10/20 一個很長很長很長很長很長很┃",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
        assert_eq!(app.calendar_top, Some(0));
        // 那天的事墊底色（跟月曆上那週一樣），別天的不墊
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(40, 7)].bg, theme.cursor_line);
        assert_eq!(buffer[(40, 6)].bg, Color::Reset);
        // 選沒事的 10/19：右邊不捲（不會跳上跳下）
        app.selected = date(2026, 10, 19);
        draw_app(&mut app, 70, 11);
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 3)));
        // 選 10/5：右邊捲回最上面
        app.selected = date(2026, 10, 5);
        draw_app(&mut app, 70, 11);
        assert_eq!(app.agenda_window, Some((date(2026, 10, 1), 0)));
    }

    #[test]
    fn the_year_in_the_corner_is_the_top_weeks() {
        // 月曆 6 列：先選 12/28，再往下一週到 2027/1/4，畫面最上面還是 12 月那週 → 左上角寫 2026年
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.selected = date(2026, 12, 28);
        draw_app(&mut app, 35, 11);
        app.selected = date(2027, 1, 4);
        let rows = text_rows(draw_app(&mut app, 35, 11).backend().buffer());
        assert_eq!(rows[1], "│2026年 週 │ 一 二 三 四 五 六 日 │");
        assert_eq!(rows[3], "│       53 │ 28 29 30 31          │");
    }

    #[test]
    fn a_busy_month_takes_at_most_two_thirds() {
        // 範例任務檔、70 × 24：月曆 19 列，10 月事很多 → 佔 19 × 2 / 3 = 12 列，下面的 11 月看得到幾週
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(include_str!("../testdata/tasks.md")).items,
        );
        let rows = text_rows(draw_app(&mut app, 70, 24).backend().buffer());
        let body = &rows[3..rows.len() - 2];
        assert_eq!(body.len(), 19);
        let separator = body.iter().position(|row| row.contains('┼'));
        assert_eq!(separator, Some(12));
        assert!(body[13].contains("11月"));
    }

    #[test]
    fn a_hint_tells_how_many_more_items_a_month_has() {
        // 剛打開（選今天 10/17）、10 月只放得下 5 件：右邊捲 2 列看 10/17 的事，
        // 10 月下面那條線寫「還有 1 件」（10/20 那件）
        let mut app = agenda_app(Lang::ZhTw);
        let terminal = draw_app(&mut app, 70, 11);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/17（六） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││ 10/15 修 bug                    │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/16 17:30 交週報              ┃",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/17 聚餐                      ┃",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/17 08:00 早餐會              ┃",
                "│       44 │ 26 27 28 29 30 31    ││ 10/18 07:00 爬山                │",
                "│──────────┼──────────────────────││───────────────────── 還有 1 件 ─│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
        // 提示是淡色；提示左右的線還是線的顏色（「還」在 x 58，線的最後一格在 x 68）
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        assert_eq!(
            (buffer[(58, 8)].symbol(), buffer[(58, 8)].fg),
            ("還", theme.muted)
        );
        assert_eq!(
            (buffer[(40, 8)].symbol(), buffer[(40, 8)].fg),
            ("─", theme.line)
        );
        assert_eq!(
            (buffer[(68, 8)].symbol(), buffer[(68, 8)].fg),
            ("─", theme.line)
        );
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
        // 左上角的年份（2026年，畫面上有 1 月也一樣寫）
        assert_eq!(cell(5, 1), ("年", theme.muted, false));
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
    fn events_gray_out_once_their_day_is_past() {
        // 10/9 過期沒做完的任務（橘）、10/10 過去的行程（灰、不劃線，那天回到週末的藍）、10/18 還沒到的行程（橘）
        let text = "- [ ] 交週報 @2026-10-09\n- 國慶日 @2026-10-10\n- 爬山 @2026-10-18 07:00\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        let terminal = draw_app(&mut app, 70, 23);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        let cell = |x: u16, y: u16| {
            let cell = &buffer[(x, y)];
            (
                cell.symbol(),
                cell.fg,
                cell.modifier.contains(Modifier::CROSSED_OUT),
            )
        };
        // 月曆：10/9（五）x 26、10/10（六）x 29 在 y 4；10/18（日）x 32 在 y 5
        assert_eq!(cell(26, 4), ("9", theme.has_item, false));
        assert_eq!(cell(29, 4), ("0", theme.weekend, false));
        assert_eq!(cell(32, 5), ("8", theme.has_item, false));
        // 右邊（從 x 36 開始，一件一列）：標題在 x 43；爬山前面有時間，在 x 49
        assert_eq!(cell(43, 3), ("交", theme.has_item, false));
        assert_eq!(cell(43, 4), ("國", theme.muted, false));
        assert_eq!(cell(38, 4), ("0", theme.muted, false));
        assert_eq!(cell(49, 5), ("爬", theme.has_item, false));
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

    /// 版型 3、今天 2026/10/17、選取 `selected`、任務檔是 `text` 的 app
    fn month_app(lang: Lang, selected: Date, text: &str) -> App {
        let mut app = App::new(lang, date(2026, 10, 17), crate::tasks::parse(text).items);
        app.view = View::Month;
        app.selected = selected;
        app
    }

    #[test]
    fn month_view_fills_the_space_with_six_equal_weeks() {
        // 80 × 30：框裡面 78 × 27 → 天的寬度 78 − 5 − 7 = 66，一、二、三 10 格、其他 9 格；每週 (27 − 7) ÷ 6 = 3 列，
        // 零頭 2 列留在最下面（直線照樣畫到下框）。最上面是 10/1 那週的前一週（9/21）；1 號後面寫月份；
        // 選取的今天加括號；右上角寫選取的日期；跨月那週的週數看選取那個月的日子（9/28 那週寫 10/1 的 40）
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 30).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── 2026/10/17（六） ╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  39 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │[17]     │ 18      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
        // 80 × 28：每週 3 列剛好填滿，最後一週貼著下框（外框接成 ┴）
        let mut app = month_app(Lang::En, date(2026, 10, 17), "");
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 28).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── Sat Oct 17, 2026 ╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  39 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 Oct   │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │[17]     │ 18      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 Nov   │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab switch  [] month  t today  q quit",
            ]
        );
    }

    /// 大月曆 test 用的任務檔（`tools/expected_screen.py` 算預期畫面時也讀這一份）
    const MONTH_TASKS: &str = include_str!("../testdata/month.md");

    #[test]
    fn month_view_lists_each_days_items_in_its_cell() {
        // 80 × 30、每週 3 列：日期下面列 2 件；同一天沒時間的在前（跟版型 1 一樣）；
        // 10/16 有 4 件放不下：列 1 件、最後一行「+3」；其他月份（9/25、11/1）的事照列。
        // 太長的標題往右借隔壁格（同一行是空的才借，蓋過中間的格線），借不到就切掉：10/13 借到 10/14、停在 10/15 前面；
        // 10/12 隔壁有事，借不到
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 30).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── 2026/10/17（六） ╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  39 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
                "│     │          │          │          │         │ ∙ 中秋烤肉        │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │ ∙ 繳電話費          │          │         │         │ ∙ 國慶日│         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │[17]     │ 18      │",
                "│     │ ∙ 14:00 C│ ∙ 15:00 跟 SRE 確認 │ ∙ 修 bug│ ∙ 整理筆│ ∙ 聚餐  │ ∙ 07:00 │",
                "│     │          │          │          │         │ +3      │ ∙ 08:00 早餐會    │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │ ∙ 一個很長很長很長很長很長的標題        │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
                "│     │          │          │          │         │         │         │ ∙ 買生日│",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
        // 80 × 24、每週 2 列：只放得下 1 件，2 件以上就只寫「+N」
        let mut app = month_app(Lang::En, date(2026, 10, 17), MONTH_TASKS);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 24).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── Sat Oct 17, 2026 ╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  39 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
                "│     │          │          │          │         │ ∙ 中秋烤肉        │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 Oct   │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │ ∙ 繳電話費          │          │         │         │ ∙ 國慶日│         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │[17]     │ 18      │",
                "│     │ ∙ 14:00 C│ ∙ 15:00 跟 SRE 確認 │ ∙ 修 bug│ +4      │ +2      │ ∙ 07:00 │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │ ∙ 一個很長很長很長很長很長的標題        │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 Nov   │",
                "│     │          │          │          │         │         │         │ ∙ 買生日│",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab switch  [] month  t today  q quit",
            ]
        );
        // 80 × 16、每週只有 1 列：不列事，日期後面畫點點（1 件 ⠂、2 件 ⠒、4 件以上 ⠶）
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── 2026/10/17（六） ╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  39 │ 21       │ 22       │ 23       │ 24      │ 25 ⠂    │ 26      │ 27      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5 ⠂      │ 6        │ 7        │ 8       │ 9       │ 10 ⠂    │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12 ⠂     │ 13 ⠂     │ 14       │ 15 ⠂    │ 16 ⠶    │[17]⠒    │ 18 ⠂    │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20 ⠂     │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月⠂ │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
    }

    #[test]
    fn month_view_item_colors() {
        // 80 × 30（畫面見 `month_view_lists_each_days_items_in_its_cell`）：格子從 x 7、18、29、40、50、60、70 開始，
        // 第 1、4 週在 y 3、15
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        let terminal = draw_app(&mut app, 80, 30);
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
        // 10/12（一）沒做完的事：點和標題橘色、時間淡色
        assert_eq!(cell(8, 16), ("∙", theme.has_item, false));
        assert_eq!(cell(10, 16), ("1", theme.muted, false));
        // 10/15（四）做完的事：整行灰、標題劃線（點不劃）
        assert_eq!(cell(41, 16), ("∙", theme.muted, false));
        assert_eq!(cell(43, 16), ("修", theme.muted, true));
        // 10/17（六）今天：綠色
        assert_eq!(cell(63, 16), ("聚", theme.today, false));
        // 10/16（五）放不下：「+3」淡色
        assert_eq!(cell(52, 17), ("3", theme.muted, false));
        // 9/25（五）其他月份：整行灰
        assert_eq!(cell(51, 4), ("∙", theme.other_month, false));
        assert_eq!(cell(53, 4), ("中", theme.other_month, false));
        // 10/20（二）的長標題往右借到 10/23（五）：蓋過格線的字是它自己的顏色（x 49 原本是直線）；
        // 字後面的格線還是格線的顏色
        assert_eq!(cell(49, 20), ("題", theme.has_item, false));
        assert_eq!(buffer[(59, 20)].symbol(), "│");
        assert_eq!(buffer[(59, 20)].fg, theme.grid);
        // 每週只有 1 列：點點跟著那天的顏色（10/16 橘）；都做完了（10/15）淡色
        let terminal = draw_app(&mut app, 80, 16);
        let buffer = terminal.backend().buffer();
        // 80 × 16：第 4 週在 y 9；10/16 是「 16 ⠶」、10/15 是「 15 ⠂」
        assert_eq!(
            (buffer[(54, 9)].symbol(), buffer[(54, 9)].fg),
            ("⠶", theme.has_item)
        );
        assert_eq!(
            (buffer[(44, 9)].symbol(), buffer[(44, 9)].fg),
            ("⠂", theme.muted)
        );
    }

    #[test]
    fn month_view_with_the_side_menu_hides_unchecked_calendars() {
        // 121 格寬：side menu 在最左邊，大月曆佔剩下的整塊
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        let terminal = draw_app(&mut app, 121, 22);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────╮╭─────────────────────────────────────────────────────────────────────────────── 2026/10/17（六） ╮",
                "│ 日曆               ││  週   一            二           三           四           五           六           日         │",
                "│────────────────────│├─────┬─────────────┬────────────┬────────────┬────────────┬────────────┬────────────┬────────────┤",
                "│ ☑ 未分組         7 ││  39 │ 21          │ 22         │ 23         │ 24         │ 25         │ 26         │ 27         │",
                "│ ☑ 生活           8 ││     │             │            │            │            │ ∙ 中秋烤肉 │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  40 │ 28          │ 29         │ 30         │ 1 10月     │ 2          │ 3          │ 4          │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  41 │ 5           │ 6          │ 7          │ 8          │ 9          │ 10         │ 11         │",
                "│                    ││     │ ∙ 繳電話費  │            │            │            │            │ ∙ 國慶日   │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  42 │ 12          │ 13         │ 14         │ 15         │ 16         │[17]        │ 18         │",
                "│                    ││     │ ∙ 14:00 Code│ ∙ 15:00 跟 SRE 確認監控 │ ∙ 修 bug   │ +4         │ +2         │ ∙ 07:00 爬 │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  43 │ 19          │ 20         │ 21         │ 22         │ 23         │ 24         │ 25         │",
                "│                    ││     │             │ ∙ 一個很長很長很長很長很長的標題     │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  44 │ 26          │ 27         │ 28         │ 29         │ 30         │ 31         │ 1 11月     │",
                "│                    ││     │             │            │            │            │            │            │ ∙ 買生日禮 │",
                "╰────────────────────╯╰─────┴─────────────┴────────────┴────────────┴────────────┴────────────┴────────────┴────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  q 離開",
            ]
        );
        // 「生活」沒勾：它的事從格子裡拿掉，那幾天也不是橘色了（10/18 回到週末的藍）
        app.groups[1].shown = false;
        let terminal = draw_app(&mut app, 121, 22);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────╮╭─────────────────────────────────────────────────────────────────────────────── 2026/10/17（六） ╮",
                "│ 日曆               ││  週   一            二           三           四           五           六           日         │",
                "│────────────────────│├─────┬─────────────┬────────────┬────────────┬────────────┬────────────┬────────────┬────────────┤",
                "│ ☑ 未分組         7 ││  39 │ 21          │ 22         │ 23         │ 24         │ 25         │ 26         │ 27         │",
                "│ ☐ 生活           8 ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  40 │ 28          │ 29         │ 30         │ 1 10月     │ 2          │ 3          │ 4          │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  41 │ 5           │ 6          │ 7          │ 8          │ 9          │ 10         │ 11         │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  42 │ 12          │ 13         │ 14         │ 15         │ 16         │[17]        │ 18         │",
                "│                    ││     │ ∙ 14:00 Code│ ∙ 15:00 跟 SRE 確認監控 │ ∙ 修 bug   │ +4         │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  43 │ 19          │ 20         │ 21         │ 22         │ 23         │ 24         │ 25         │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "│                    │├─────┼─────────────┼────────────┼────────────┼────────────┼────────────┼────────────┼────────────┤",
                "│                    ││  44 │ 26          │ 27         │ 28         │ 29         │ 30         │ 31         │ 1 11月     │",
                "│                    ││     │             │            │            │            │            │            │            │",
                "╰────────────────────╯╰─────┴─────────────┴────────────┴────────────┴────────────┴────────────┴────────────┴────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  q 離開",
            ]
        );
        // 10/18（日）：大月曆從 x 22 開始，星期日那欄從 x 108 開始、第 4 週在 y 12（每週 2 列）；「 18」的「1」在 x 109
        let theme = Theme::catppuccin_mocha();
        let cell = &terminal.backend().buffer()[(109, 12)];
        assert_eq!((cell.symbol(), cell.fg), ("1", theme.weekend));
        // 勾回來：橘色
        app.groups[1].shown = true;
        let terminal = draw_app(&mut app, 121, 22);
        let cell = &terminal.backend().buffer()[(109, 12)];
        assert_eq!((cell.symbol(), cell.fg), ("1", theme.has_item));
    }

    #[test]
    fn month_view_scrolls_a_week_at_a_time() {
        // 80 × 16：每週只有 1 列。剛打開最上面是 9/21；選到 11/2（第 7 週）→ 往下捲一週，最上面變 9/28
        let mut app = month_app(Lang::En, date(2026, 10, 17), "");
        draw_app(&mut app, 80, 16);
        assert_eq!(app.month_top, Some(date(2026, 9, 21)));
        app.selected = date(2026, 11, 2);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭───────────────────────────────────────────────────────────── Mon Nov 2, 2026 ╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 Oct   │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 Nov   │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  45 │[2]       │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab switch  [] month  t today  q quit",
            ]
        );
        assert_eq!(app.month_top, Some(date(2026, 9, 28)));
        // 選 9/20（最上面那週的前一週）：那週變成最上面；選取那個月變成 9 月，10 月的日子變灰
        app.lang = Lang::ZhTw;
        app.selected = date(2026, 9, 20);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭───────────────────────────────────────────────────────────── 2026/9/20（日） ╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  38 │ 14       │ 15       │ 16       │ 17      │ 18      │ 19      │[20]     │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  39 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │ 17      │ 18      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
            ]
        );
        assert_eq!(app.month_top, Some(date(2026, 9, 14)));
        // 6 週裡面移動：不捲
        app.selected = date(2026, 10, 18);
        draw_app(&mut app, 80, 16);
        assert_eq!(app.month_top, Some(date(2026, 9, 14)));
    }

    #[test]
    fn month_view_click_positions_map_to_the_cells() {
        // 80 × 30（畫面見 `month_view_fills_the_space_with_six_equal_weeks`）：一 x 7～16、二 x 18～27，
        // 第 1 週 y 3～5、第 2 週 y 7～9；最上面那週是 9/21
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
        app.screen = Rect::new(0, 0, 80, 30);
        draw_app(&mut app, 80, 30);
        // 格子裡任何一格都算那天（不只第一行的日期）
        assert_eq!(date_at(&app, 7, 3), Some(date(2026, 9, 21)));
        assert_eq!(date_at(&app, 16, 5), Some(date(2026, 9, 21)));
        assert_eq!(date_at(&app, 18, 7), Some(date(2026, 9, 29)));
        assert_eq!(date_at(&app, 78, 25), Some(date(2026, 11, 1)));
        // 最後一週下面的零頭也算最後一週
        assert_eq!(date_at(&app, 10, 27), Some(date(2026, 10, 26)));
        // 沒有日期：直線、橫線、週數欄、表頭、下框、左框、操作說明
        for (x, y) in [
            (17, 3),
            (10, 6),
            (3, 3),
            (10, 1),
            (10, 28),
            (0, 3),
            (10, 29),
        ] {
            assert_eq!(date_at(&app, x, y), None, "({x}, {y})");
        }
    }

    #[test]
    fn month_view_colors_by_role() {
        // 80 × 30（畫面見 `month_view_fills_the_space_with_six_equal_weeks`）：一 x 7、二 x 18、三 x 29、四 x 40、
        // 五 x 50、六 x 60、日 x 70；第 1～6 週在 y 3、7、11、15、19、23。選 10/16（五），10/15（四）有沒做完的事
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 16), "- [ ] 交週報 @2026-10-15\n");
        let terminal = draw_app(&mut app, 80, 30);
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
        // 選取的括號紅色；日期本身是那天的顏色（平日一般色），選取那個月的日期加粗
        assert_eq!(cell(50, 15), ("[", theme.selected, true));
        assert_eq!(cell(52, 15), ("6", theme.text, true));
        assert_eq!(cell(53, 15), ("]", theme.selected, true));
        // 今天綠、有沒做完的事橘、週末藍
        assert_eq!(cell(62, 15), ("7", theme.today, true));
        assert_eq!(cell(42, 15), ("5", theme.has_item, true));
        assert_eq!(cell(72, 15), ("8", theme.weekend, true));
        // 其他月份的日期灰、不加粗：9/21、11/1（月份也灰）；10/1 的月份是 label 的顏色
        assert_eq!(cell(9, 3), ("1", theme.other_month, false));
        assert_eq!(cell(71, 23), ("1", theme.other_month, false));
        assert_eq!(cell(73, 23), ("1", theme.other_month, false));
        assert_eq!(cell(43, 7), ("1", theme.label, false));
        // 週數：選取那週（42）亮、其他週（39）暗
        assert_eq!(cell(3, 15), ("4", theme.week_number, false));
        assert_eq!(cell(3, 3), ("3", theme.week_number_dim, false));
        // 表頭：平日淡色、六日週末色
        assert_eq!(cell(8, 1), ("一", theme.muted, false));
        assert_eq!(cell(61, 1), ("六", theme.weekend, false));
        // 格線淡色；碰到外框的接頭是外框的顏色（鍵盤在月曆：focus 色）
        assert_eq!(cell(6, 2), ("┬", theme.grid, false));
        assert_eq!(cell(6, 3), ("│", theme.grid, false));
        assert_eq!(cell(6, 6), ("┼", theme.grid, false));
        // 最後一週下面的零頭：直線照樣畫到下框，下框接成 ┴（外框的顏色）
        assert_eq!(cell(6, 26), ("│", theme.grid, false));
        assert_eq!(cell(6, 28), ("┴", theme.focus, false));
        assert_eq!(cell(0, 6), ("├", theme.focus, false));
        assert_eq!(cell(79, 6), ("┤", theme.focus, false));
        // 右上角的日期一般色
        assert_eq!(cell(64, 0), ("2", theme.text, false));
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
