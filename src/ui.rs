//! 畫整個畫面：最左邊 side menu（夠寬才有；不夠寬時按 `s` 蓋在左邊），右邊是主畫面（版型 1 的月曆框 + 事項框、版型 3 的大月曆、或任務頁），
//! 最下面一列操作說明；跳窗（版型 3 那天的事、編輯框、說明頁、設定頁）蓋在最上面

mod day;
mod editor;
mod help;
mod month;
mod months;
mod planner;
mod settings;
mod side;
mod tasks;

use crate::app::{App, Focus, Mode, SettingsPage, View};
use crate::theme::Theme;
use jiff::civil::Date;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Margin, Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

/// terminal 至少這麼寬才畫 side menu
const SIDE_MIN_WIDTH: u16 = 120;

/// 編輯框的位置（滑鼠點框外 = 取消用）
pub fn editor_area(screen: Rect) -> Rect {
    editor::area(screen)
}

/// 版型 3「那天的事」跳窗的框在畫面上的位置（滑鼠點框外 = 關掉用）
pub fn day_area(app: &App) -> Rect {
    day::area(app, areas(app.screen, app).main)
}

/// 滑鼠點在 (`column`, `row`) 點到跳窗裡的第幾件（`App::items_on(selected)` 的位置）
pub fn day_item_at(app: &App, column: u16, row: u16) -> Option<usize> {
    day::item_at(app, areas(app.screen, app).main, column, row)
}

/// 說明頁的框在畫面上的位置（滑鼠點框外 = 關掉用）
pub fn help_area(app: &App, full: bool) -> Rect {
    help::area(app, app.screen, full)
}

/// 說明頁最多能往下捲幾行（照 terminal 現在的大小算）
pub fn help_max_scroll(app: &App, full: bool) -> u16 {
    help::max_scroll(app, app.screen, full)
}

/// 設定頁的框在畫面上的位置（滑鼠點框外 = 關掉用）
pub fn settings_area(app: &App) -> Rect {
    settings::area(app, app.screen)
}

/// 滑鼠點在 (`column`, `row`) 點到設定頁的哪一列（`app::rows()` 的位置）
pub fn settings_row_at(app: &App, column: u16, row: u16) -> Option<usize> {
    settings::row_at(app, app.screen, column, row)
}

/// 跳窗（`area`）左邊那格如果是中文字（佔 2 格），它的後半格會蓋到跳窗的左框線（terminal 只收到中文字、收不到框線）：
/// 先換成空白再畫跳窗
fn clear_wide_left_of(buffer: &mut Buffer, area: Rect) {
    let Some(x) = area.x.checked_sub(1) else {
        return;
    };
    for y in area.top()..area.bottom() {
        if let Some(cell) = buffer.cell_mut((x, y))
            && Span::raw(cell.symbol()).width() > 1
        {
            cell.set_symbol(" ");
        }
    }
}

/// 跳窗（`area`）放不下時，右框線上畫捲軸：軌道就是原本的框線（不另外畫），粗線那段 = 現在看得到的部分在整頁的哪裡。
/// `scroll` = 往下捲了幾行、`max` = 最多能捲幾行、`visible` = 一次看得到幾行
fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    scroll: u16,
    max: u16,
    visible: u16,
    theme: &Theme,
) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .track_symbol(None)
        .begin_symbol(None)
        .end_symbol(None)
        .thumb_symbol("┃")
        .thumb_style(Style::new().fg(theme.label));
    // 能停的位置有 max + 1 個（0..=max），一次看得到 visible 行
    let mut state = ScrollbarState::new(usize::from(max) + 1)
        .position(usize::from(scroll))
        .viewport_content_length(usize::from(visible));
    // 上下的圓角不算：捲軸只畫在右框線的直線那段
    frame.render_stateful_widget(scrollbar, area.inner(Margin::new(0, 1)), &mut state);
}

/// terminal（`screen`）夠不夠寬畫 side menu
pub fn side_fits(screen: Rect) -> bool {
    screen.width >= SIDE_MIN_WIDTH
}

/// 不夠寬、鍵盤在 side menu（按了 `s`）：side menu 蓋在主畫面左邊的位置；其他時候是 `None`（滑鼠點框外 = 收起用）
pub fn side_overlay(app: &App) -> Option<Rect> {
    side_overlay_in(app.screen, app)
}

/// 同 `side_overlay`，terminal 是 `screen` 那麼大的時候
fn side_overlay_in(screen: Rect, app: &App) -> Option<Rect> {
    if side_fits(screen) || app.focus != Focus::Side {
        return None;
    }
    let main = areas(screen, app).main;
    Some(Rect {
        width: side::width(app).min(main.width),
        ..main
    })
}

/// 畫面上每一塊的位置
struct Areas {
    /// 最左邊：side menu（日曆清單）；沒畫的時候寬度是 0
    side: Rect,
    /// side menu 右邊、操作說明上面的整塊：版型 3 的大月曆、任務頁畫在這裡
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
    /// 任務頁的一列：`App::tasks_rows()` 裡的位置；`on_marker` = 點在 `▸` / `▾` 上（展開 / 收起）
    TaskRow { row: usize, on_marker: bool },
}

/// 滑鼠點在 terminal 的 (`column`, `row`) 點到什麼；沒點在日期、事、日曆上回傳 `None`
pub fn click_at(app: &App, column: u16, row: u16) -> Option<Click> {
    let areas = areas(app.screen, app);
    // side menu：夠寬時在最左邊，不夠寬時叫出來蓋在左邊
    let side = side_overlay(app).unwrap_or(areas.side);
    if side.contains(Position::new(column, row)) {
        return side::click_at(side, app, column, row);
    }
    match app.view {
        View::Planner => planner::click_at(areas.calendar, areas.agenda, app, column, row),
        View::Month => month::click_at(areas.main, app, column, row),
        View::Tasks => tasks::click_at(areas.main, app, column, row),
    }
}

/// 畫整個畫面：中間最左邊是 side menu（夠寬才有），右邊是版型 1 的月曆框 + 事項框、版型 3 的大月曆或任務頁；
/// 最下面一列操作說明；開著的跳窗最後畫，蓋在最上面
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
    // 最下面一列：問要不要刪 > 改期中（在搬哪件、怎麼確定）> 提示（寫檔失敗、重新載入，下一個按鍵就換回來）> 操作說明
    if let Some((item, count)) = app.confirm_delete() {
        frame.render_widget(
            Line::styled(
                texts.confirm_delete(&item.title, count),
                Style::new().fg(theme.notice),
            ),
            hints,
        );
    } else if let Some(moving) = app.moving() {
        frame.render_widget(
            Line::styled(
                texts.moving(&moving.item.title),
                Style::new().fg(theme.notice),
            ),
            hints,
        );
    } else if let Some(notice) = &app.notice {
        frame.render_widget(
            Line::styled(texts.notice(notice), Style::new().fg(theme.notice)),
            hints,
        );
    } else {
        // 任務頁有自己的操作說明；版型 3 的跳窗開著時寫跳窗裡的鍵
        let hint_text = match app.view {
            _ if app.focus == Focus::Day => texts.hints_day,
            View::Tasks => texts.hints_tasks,
            View::Planner | View::Month => texts.hints,
        };
        frame.render_widget(Line::styled(hint_text, Style::new().fg(theme.muted)), hints);
    }
    if side_area.width > 0 {
        side::render(frame, side_area, app);
    }
    // 主畫面在 side menu 之後畫：它會改 `app`（記住捲到哪），這時候上面借的 `theme` 已經用完了；跳窗再蓋在它上面
    match app.view {
        View::Planner => planner::render(frame, calendar_area, agenda_area, app),
        View::Month => month::render(frame, main_area, app),
        View::Tasks => tasks::render(frame, main_area, app),
    }
    // 不夠寬時叫出來的 side menu：蓋在主畫面左邊
    if let Some(area) = side_overlay_in(frame.area(), app) {
        frame.render_widget(Clear, area);
        side::render(frame, area, app);
    }
    // 版型 3 那天的事跳窗：蓋在大月曆上（編輯框、說明頁這些再蓋在它上面）
    if app.view == View::Month && app.focus == Focus::Day {
        day::render(frame, main_area, app);
    }
    // 編輯框、說明頁、設定頁蓋在最上面
    match &app.mode {
        Mode::Editor(editor) => editor::render(frame, editor, app),
        Mode::Help(help) => help::render(frame, app, *help),
        Mode::Settings { page, .. } => {
            // 先捲到選到的那列看得到（記在設定頁的 `top`），再畫
            let page = SettingsPage {
                top: settings::scrolled(app, frame.area(), *page),
                ..*page
            };
            if let Mode::Settings { page: stored, .. } = &mut app.mode {
                *stored = page;
            }
            settings::render(frame, app, page);
        }
        // 問要不要刪、改期中：畫在最下面那一列（上面畫過了）
        Mode::ConfirmDelete(..) | Mode::Moving(_) | Mode::Normal => {}
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
            Click::Item(index) => Some(app.agenda()[index].date),
            Click::Group(_) | Click::TaskRow { .. } => None,
        }
    }

    /// 用現在的 `app` 畫一次 `width` × `height` 的畫面（捲到哪會記在 `app` 裡，下一次接著用）
    fn draw_app(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        terminal
    }

    /// 畫面每一列的字，去掉尾端空白（見 `area_rows`）
    fn text_rows(buffer: &Buffer) -> Vec<String> {
        area_rows(buffer, buffer.area)
            .iter()
            .map(|row| row.trim_end().to_string())
            .collect()
    }

    /// `area` 裡每一列的字；中文字佔 2 格，後面那格（buffer 裡是空白）跳過，接起來才是原本的字。
    /// 說明頁、設定頁的 test 也用這個
    pub(super) fn area_rows(buffer: &Buffer, area: Rect) -> Vec<String> {
        (area.top()..area.bottom())
            .map(|y| {
                let mut row = String::new();
                let mut x = area.left();
                while x < area.right() {
                    let symbol = buffer[(x, y)].symbol();
                    row.push_str(symbol);
                    x += u16::try_from(Span::raw(symbol).width().max(1)).unwrap();
                }
                row
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
    fn a_narrow_side_menu_covers_the_left_of_the_screen() {
        // 60 × 14、鍵盤在 side menu（按了 s）：side menu 22 格寬、蓋在月曆左邊，月曆不縮（x 22 還是月曆的上框線）；
        // 數字 = 那個日曆有日期的事有幾件（生活的 c 沒日期不算）
        let text = "# 工作\n- a @2026-10-10\n# 生活\n- b @2026-10-11\n- c\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        app.focus = Focus::Side;
        let terminal = draw_app(&mut app, 60, 14);
        let buffer = terminal.backend().buffer();
        assert_eq!(
            area_rows(buffer, Rect::new(0, 0, 22, 6)),
            [
                "╭────────────────────╮",
                "│ 日曆               │",
                "│────────────────────│",
                "│ ☑ 工作           1 │",
                "│ ☑ 生活           1 │",
                "│                    │",
            ]
        );
        assert_eq!(buffer[(21, 13 - 1)].symbol(), "╯");
        assert_eq!(buffer[(22, 0)].symbol(), "─");
        // 操作說明窄的時候也有 s（60 格寬，後面切掉）
        assert!(text_rows(buffer)[13].starts_with(" hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆"));
        // 鍵盤不在 side menu：不畫
        app.focus = Focus::Calendar;
        let terminal = draw_app(&mut app, 60, 14);
        assert_eq!(terminal.backend().buffer()[(0, 3)].symbol(), "│");
        assert_ne!(terminal.backend().buffer()[(4, 3)].symbol(), "工");
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl move  Tab switch  [] month  t today  s calendars  ? help  , settings  q quit",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl move  Tab switch  [] month  t today  s calendars  ? help  , settings  q quit",
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
            // 那天的第一件（跨多天的中間那幾天也算）：右邊寫的是它開始那天，例：` 10/28~11/3`
            let first = app
                .agenda()
                .iter()
                .find(|entry| entry.is_on(app.selected))
                .and_then(|entry| entry.item.date);
            if let Some(first) = first {
                let date = format!(" {}/{}", first.month(), first.day());
                assert!(
                    right.iter().any(|row| row.contains(&date)),
                    "{}：右邊看不到那天的事：{body:#?}",
                    app.selected
                );
            }
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
            app.selected = app.agenda()[index].date;
            let first = app.agenda()[index].item.date.expect("有日期");
            let rows = text_rows(draw_app(&mut app, 70, 13).backend().buffer());
            let body = &rows[3..rows.len() - 2];
            // 選到的那件日期加括號；跨多天的括號裡是範圍，例：`[10/28~11/3]`
            let chosen = format!("[{}/{}", first.month(), first.day());
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
    fn the_editor_shows_the_line_and_what_it_reads_as() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse("- [ ] 交週報 @2026-10-16 17:30\n").items,
        );
        app.handle_key(KeyEvent::from(KeyCode::Tab));
        app.handle_key(KeyEvent::from(KeyCode::Char('e')));
        // 70 × 12：編輯框在最下面操作說明的上面（y 7～10），整個 terminal 那麼寬
        let mut terminal = draw_app(&mut app, 70, 12);
        let rows = text_rows(terminal.backend().buffer());
        assert!(rows[7].starts_with("╭ 編輯第 1 行 ─"), "{}", rows[7]);
        assert!(rows[7].ends_with("─╮"));
        assert_eq!(
            rows[8].trim_end_matches([' ', '│']),
            "│ - [ ] 交週報 @2026-10-16 17:30"
        );
        assert_eq!(
            rows[9].trim_end_matches([' ', '│']),
            "│ 任務 · 2026/10/16（五） 17:30 · 「交週報」"
        );
        assert!(
            rows[10].starts_with("╰ Enter 存檔  Esc 取消 ─"),
            "{}",
            rows[10]
        );
        // 游標在打的字最後面：框線 1 + 空白 1 + 「- [ ] 交週報 @2026-10-16 17:30」30 格
        assert_eq!(
            terminal.get_cursor_position().expect("cursor"),
            Position::new(32, 8)
        );
        // 讀成什麼是淡色；改成不存在的時間：那一列換成紅字說明
        let theme = Theme::catppuccin_mocha();
        assert_eq!(terminal.backend().buffer()[(2, 9)].fg, theme.muted);
        for code in [
            KeyCode::Backspace,
            KeyCode::Backspace,
            KeyCode::Char('9'),
            KeyCode::Char('9'),
        ] {
            app.handle_key(KeyEvent::from(code));
        }
        let terminal = draw_app(&mut app, 70, 12);
        let rows = text_rows(terminal.backend().buffer());
        assert_eq!(
            rows[9].trim_end_matches([' ', '│']),
            "│ 17:99：沒有這個時間"
        );
        assert_eq!(terminal.backend().buffer()[(2, 9)].fg, theme.error);
    }

    #[test]
    fn the_bottom_line_says_what_is_being_moved() {
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse("- [ ] 交週報 @2026-10-16\n").items,
        );
        app.focus = Focus::Agenda;
        app.handle_key(ratatui::crossterm::event::KeyEvent::from(
            ratatui::crossterm::event::KeyCode::Char('r'),
        ));
        let rows = text_rows(draw_app(&mut app, 70, 11).backend().buffer());
        assert_eq!(
            rows[10],
            " 改期「交週報」：移到新的日期  Enter 確定  Esc 取消"
        );
    }

    #[test]
    fn a_notice_replaces_the_key_hints_until_the_next_key() {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.notice = Some(crate::app::Notice::NotATask);
        let terminal = draw_app(&mut app, 70, 11);
        let rows = text_rows(terminal.backend().buffer());
        assert_eq!(rows[10], " 行程沒有框，不能打勾");
        let cell = &terminal.backend().buffer()[(1, 10)];
        assert_eq!(cell.fg, Theme::catppuccin_mocha().notice);
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

    /// 跨多天的事 test 用的任務檔（`tools/expected_screen.py` 算預期畫面時也讀這一份）
    const RANGE_TASKS: &str = include_str!("../testdata/ranges.md");

    #[test]
    fn multi_day_items_are_listed_once_per_month() {
        // 右邊在開始那天列一次，日期寫範圍（`10/20~25`、跨月 `10/28~11/3`、跨年 `12/30~1/2`）；
        // 跨月的在之後每個月最上面再列一次；日期比較長，標題往右推
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(RANGE_TASKS).items,
        );
        let terminal = draw_app(&mut app, 70, 30);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項       2026/10/17（六） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  10月 40 │           1  2  3  4 ││ 10/16 17:30 交週報              │",
                "│       41 │  5  6  7  8  9 10 11 ││ 10/20~25 東京出差               │",
                "│       42 │ 12 13 14 15 16[17]18 ││ 10/22 買伴手禮                  │",
                "│       43 │ 19 20 21 22 23 24 25 ││ 10/28~11/3 主管出差             │",
                "│       44 │ 26 27 28 29 30 31    ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  11月 44 │                    1 ││ 10/28~11/3 主管出差             │",
                "│       45 │  2  3  4  5  6  7  8 ││  11/1 準備月報                  │",
                "│       46 │  9 10 11 12 13 14 15 ││                                 │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                 │",
                "│       48 │ 23 24 25 26 27 28 29 ││                                 │",
                "│       49 │ 30                   ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 ││ 12/30~1/2 09:00 跨年旅行        │",
                "│       50 │  7  8  9 10 11 12 13 ││                                 │",
                "│       51 │ 14 15 16 17 18 19 20 ││                                 │",
                "│       52 │ 21 22 23 24 25 26 27 ││                                 │",
                "│       53 │ 28 29 30 31          ││                                 │",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━││━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2  3 ││ 12/30~1/2 09:00 跨年旅行        │",
                "│   1月  2 │  4  5  6  7  8  9 10 ││                                 │",
                "│        3 │ 11 12 13 14 15 16 17 ││                                 │",
                "│        4 │ 18 19 20 21 22 23 24 ││                                 │",
                "│        5 │ 25 26 27 28 29 30 31 ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // 月曆：範圍裡的每一天加底線（10/20～25、10/28～11/3），前後一天不加；顏色不變：開始那天橘色，
        // 中間沒有自己的事的日子（10/21，x 19～20）是一般的顏色
        let buffer = terminal.backend().buffer();
        let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
        // 10/19（一）x 13～14、10/20（二）x 16～17、10/25（日）x 31～32 在 y 6；10/26（一）、10/28（三）在 y 7
        assert!(!underlined(14, 6));
        assert!(underlined(17, 6) && underlined(32, 6));
        assert!(!underlined(14, 7) && underlined(20, 7));
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(17, 6)].fg, theme.has_item);
        assert_eq!(buffer[(20, 6)].fg, theme.text);
        // 鍵盤在右邊、選到 11 月那一行的主管出差（從 11/2 按 Tab）：括號裡是範圍；11 月在最上面
        // （11/2 那週是 11 月第 2 列，要離上緣留 2 列，所以往上多捲一列、露出 10 月下面的線）
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(RANGE_TASKS).items,
        );
        app.focus = Focus::Agenda;
        app.agenda_index = 4;
        app.selected = date(2026, 11, 2);
        assert_eq!(
            text_rows(draw_app(&mut app, 70, 30).backend().buffer()),
            [
                "╭─────────────────────────────────╮╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││ 重要事項        2026/11/2（一） │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  11月 44 │                    1 ││[10/28~11/3]主管出差             │",
                "│       45 │ [2] 3  4  5  6  7  8 ││  11/1 準備月報                  │",
                "│       46 │  9 10 11 12 13 14 15 ││                                 │",
                "│       47 │ 16 17 18 19 20 21 22 ││                                 │",
                "│       48 │ 23 24 25 26 27 28 29 ││                                 │",
                "│       49 │ 30                   ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│  12月 49 │     1  2  3  4  5  6 ││ 12/30~1/2 09:00 跨年旅行        │",
                "│       50 │  7  8  9 10 11 12 13 ││                                 │",
                "│       51 │ 14 15 16 17 18 19 20 ││                                 │",
                "│       52 │ 21 22 23 24 25 26 27 ││                                 │",
                "│       53 │ 28 29 30 31          ││                                 │",
                "│━━━━━━━━━━┿━━━━━━━━━━━━━━━━━━━━━━││━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━│",
                "│2027年  1 │              1  2  3 ││ 12/30~1/2 09:00 跨年旅行        │",
                "│   1月  2 │  4  5  6  7  8  9 10 ││                                 │",
                "│        3 │ 11 12 13 14 15 16 17 ││                                 │",
                "│        4 │ 18 19 20 21 22 23 24 ││                                 │",
                "│        5 │ 25 26 27 28 29 30 31 ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "│   2月  6 │  1  2  3  4  5  6  7 ││                                 │",
                "│        7 │  8  9 10 11 12 13 14 ││                                 │",
                "│        8 │ 15 16 17 18 19 20 21 ││                                 │",
                "│        9 │ 22 23 24 25 26 27 28 ││                                 │",
                "│──────────┼──────────────────────││─────────────────────────────────│",
                "╰─────────────────────────────────╯╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn a_multi_day_item_belongs_to_every_day_it_covers() {
        // 鍵盤在月曆、選 10/22：東京出差（10/20～25）和買伴手禮（10/22）都墊底色，交週報不墊（畫面見上面）
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(RANGE_TASKS).items,
        );
        app.selected = date(2026, 10, 22);
        let terminal = draw_app(&mut app, 70, 30);
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(40, 3)].bg, Color::Reset);
        assert_eq!(buffer[(40, 4)].bg, theme.cursor_line);
        assert_eq!(buffer[(40, 5)].bg, theme.cursor_line);
        assert_eq!(buffer[(40, 6)].bg, Color::Reset);
    }

    #[test]
    fn holidays_are_red() {
        // 國定假日（type:holiday）：國慶連假 10/9～11、光復節 10/25；10/25 還有一件工作的事，假日的紅蓋過橘
        let text = "# 國定假日 type:holiday\n- 國慶連假 @2026-10-09..2026-10-11\n- 光復節 @2026-10-25\n\
                    # 工作\n- [ ] 加班 @2026-10-25\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        let theme = Theme::catppuccin_mocha();
        let fg = |app: &mut App, x: u16, y: u16| {
            let terminal = draw_app(app, 70, 23);
            let cell = &terminal.backend().buffer()[(x, y)];
            (cell.symbol().to_string(), cell.fg)
        };
        // 10/9（五）x 26、10/10（六）x 29、10/11（日）x 32 在 y 4；10/25（日）x 32 在 y 6；10/8（四）x 23 不是假日
        assert_eq!(fg(&mut app, 26, 4), ("9".into(), theme.holiday));
        assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.holiday));
        assert_eq!(fg(&mut app, 32, 4), ("1".into(), theme.holiday));
        assert_eq!(fg(&mut app, 32, 6), ("5".into(), theme.holiday));
        assert_eq!(fg(&mut app, 23, 4), ("8".into(), theme.text));
        // 今天是假日的話，今天的綠色優先
        app.today = date(2026, 10, 10);
        assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.today));
        app.today = date(2026, 10, 17);
        // side menu 上的名字不含 type:holiday；取消勾選：不是假日了（10/25 回到有事的橘、10/10 回到週末的藍）
        assert_eq!(app.groups[0].name.as_deref(), Some("國定假日"));
        app.groups[0].shown = false;
        assert_eq!(fg(&mut app, 32, 6), ("5".into(), theme.has_item));
        assert_eq!(fg(&mut app, 29, 4), ("0".into(), theme.weekend));
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
    fn the_agenda_header_keeps_the_whole_date_when_narrow() {
        // 中文、右邊很窄：日期完整，「重要事項」放不下的整個字切掉、中間至少空一格（56 格：日期的 2 不會被吃掉；
        // 57 格：「重」和日期中間有空格）
        let terminal = draw(Lang::ZhTw, 56, 10, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭───────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││  2026/10/17（六） │",
                "│──────────┼──────────────────────││───────────────────│",
                "│  10月 40 │           1  2  3  4 ││                   │",
                "│       41 │  5  6  7  8  9 10 11 ││                   │",
                "│       42 │ 12 13 14 15 16[17]18 ││                   │",
                "│       43 │ 19 20 21 22 23 24 25 ││                   │",
                "│       44 │ 26 27 28 29 30 31    ││                   │",
                "╰─────────────────────────────────╯╰───────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  ,",
            ]
        );
        let terminal = draw(Lang::ZhTw, 57, 10, date(2026, 10, 17));
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭─────────────────────────────────╮╭────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 ││   2026/10/17（六） │",
                "│──────────┼──────────────────────││────────────────────│",
                "│  10月 40 │           1  2  3  4 ││                    │",
                "│       41 │  5  6  7  8  9 10 11 ││                    │",
                "│       42 │ 12 13 14 15 16[17]18 ││                    │",
                "│       43 │ 19 20 21 22 23 24 25 ││                    │",
                "│       44 │ 26 27 28 29 30 31    ││                    │",
                "╰─────────────────────────────────╯╰────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  ,",
            ]
        );
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl move  Tab switch  [] month  t today  s calendars  ? help  , settings  q qu",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl move  Tab switch  [] month  t today  s calendars  ? help  , settings  q qu",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
    fn month_view_without_grid_lines_gives_the_cells_more_room() {
        // 設定檔 `[month] grid_lines = false`：沒有格線，格子緊貼著，表頭下面也沒有線；
        // 80 × 24：天的寬度 78 − 5 = 73（一、二、三 11 格，其他 10 格），每週 (21 − 1) ÷ 6 = 3 列
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), MONTH_TASKS);
        app.settings.month.grid_lines = false;
        app.screen = Rect::new(0, 0, 80, 24);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 24).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── 2026/10/17（六） ╮",
                "│  週  一         二         三         四        五        六        日       │",
                "│  39  21         22         23         24        25        26        27       │",
                "│                                                 ∙ 中秋烤肉                   │",
                "│                                                                              │",
                "│  40  28         29         30         1 10月    2         3         4        │",
                "│                                                                              │",
                "│                                                                              │",
                "│  41  5          6          7          8         9         10        11       │",
                "│      ∙ 繳電話費                                           ∙ 國慶日           │",
                "│                                                                              │",
                "│  42  12         13         14         15        16       [17]       18       │",
                "│      ∙ 14:00 Co ∙ 15:00 跟 SRE 確認監 ∙ 修 bug  ∙ 整理筆  ∙ 聚餐    ∙ 07:00  │",
                "│                                                 +3        ∙ 08:00 早餐會     │",
                "│  43  19         20         21         22        23        24        25       │",
                "│                 ∙ 一個很長很長很長很長很長的標題                             │",
                "│                                                                              │",
                "│  44  26         27         28         29        30        31        1 11月   │",
                "│                                                                     ∙ 買生日 │",
                "│                                                                              │",
                "│                                                                              │",
                "│                                                                              │",
                "╰──────────────────────────────────────────────────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // 滑鼠：沒有格線的那幾格也點得到。星期二是 x 17～27，第 3 週（10/5 那週）是 y 8～10
        assert_eq!(click_at(&app, 17, 8), Some(Click::Day(date(2026, 10, 6))));
        assert_eq!(click_at(&app, 27, 10), Some(Click::Day(date(2026, 10, 6))));
        assert_eq!(click_at(&app, 28, 10), Some(Click::Day(date(2026, 10, 7))));
    }

    #[test]
    fn month_view_without_item_icons() {
        // 設定檔 `[month] item_icons = false`：格子裡的事前面沒有點，只空一格
        let mut app = month_app(Lang::En, date(2026, 10, 17), MONTH_TASKS);
        app.settings.month.item_icons = false;
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 24).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── Sat Oct 17, 2026 ╮",
                "│  Wk   Mo         Tu         We         Th        Fr        Sa        Su      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  39 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
                "│     │          │          │          │         │ 中秋烤肉│         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 Oct   │ 2       │ 3       │ 4       │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "│     │ 繳電話費 │          │          │         │         │ 國慶日  │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │[17]     │ 18      │",
                "│     │ 14:00 Cod│ 15:00 跟 SRE 確認監 │ 修 bug  │ +4      │ +2      │ 07:00 爬│",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │ 一個很長很長很長很長很長的標題│         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 Nov   │",
                "│     │          │          │          │         │         │         │ 買生日禮│",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl move  Tab switch  [] month  t today  s calendars  ? help  , settings  q qu",
            ]
        );
    }

    #[test]
    fn month_view_keeps_the_selected_week_in_the_middle() {
        // 設定檔 `[month] center_cursor = true`：選取的那週固定在第 3 列（剛打開也是，不是 1 號的前一週在最上面）
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), "");
        app.settings.month.center_cursor = true;
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── 2026/10/17（六） ╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  40 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  41 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │[17]     │ 18      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  45 │ 2        │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // 往下移一週（11/2 不在 6 週的最後面也一樣）：整個畫面跟著往下捲一週
        app.selected = date(2026, 11, 2);
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭───────────────────────────────────────────────────────────── 2026/11/2（一） ╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  45 │[2]       │ 3        │ 4        │ 5       │ 6       │ 7       │ 8       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  46 │ 9        │ 10       │ 11       │ 12      │ 13      │ 14      │ 15      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  47 │ 16       │ 17       │ 18       │ 19      │ 20      │ 21      │ 22      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  48 │ 23       │ 24       │ 25       │ 26      │ 27      │ 28      │ 29      │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn relative_week_numbers_in_both_layouts() {
        // 設定檔 `[calendar] relative_weeks = true`：選取那週（10/12～18）寫 42，其他週寫差幾週；
        // 版型 1 跨月那週（10/26～11/1）在兩個月各一列，兩列都寫 2
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.settings.calendar.relative_weeks = true;
        assert_eq!(
            text_rows(draw_app(&mut app, 35, 14).backend().buffer()),
            [
                "╭─────────────────────────────────╮",
                "│2026年 週 │ 一 二 三 四 五 六 日 │",
                "│──────────┼──────────────────────│",
                "│  10月  2 │           1  2  3  4 │",
                "│        1 │  5  6  7  8  9 10 11 │",
                "│       42 │ 12 13 14 15 16[17]18 │",
                "│        1 │ 19 20 21 22 23 24 25 │",
                "│        2 │ 26 27 28 29 30 31    │",
                "│──────────┼──────────────────────│",
                "│  11月  2 │                    1 │",
                "│        3 │  2  3  4  5  6  7  8 │",
                "│        4 │  9 10 11 12 13 14 15 │",
                "╰─────────────────────────────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今",
            ]
        );
        // 版型 3 也一樣
        app.view = View::Month;
        assert_eq!(
            text_rows(draw_app(&mut app, 80, 16).backend().buffer()),
            [
                "╭──────────────────────────────────────────────────────────── 2026/10/17（六） ╮",
                "│  週   一         二         三         四        五        六        日      │",
                "├─────┬──────────┬──────────┬──────────┬─────────┬─────────┬─────────┬─────────┤",
                "│   3 │ 21       │ 22       │ 23       │ 24      │ 25      │ 26      │ 27      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   2 │ 28       │ 29       │ 30       │ 1 10月  │ 2       │ 3       │ 4       │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   1 │ 5        │ 6        │ 7        │ 8       │ 9       │ 10      │ 11      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  42 │ 12       │ 13       │ 14       │ 15      │ 16      │[17]     │ 18      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   1 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│   2 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn month_view_lists_a_multi_day_item_on_each_of_its_days() {
        // 東京出差 10/20～25：每一天的格子都列；比較早開始的排在那天的最前面（10/22 先東京出差、再買伴手禮）；
        // 主管出差 10/28～11/3 列到畫面上最後一天 11/1
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), RANGE_TASKS);
        let terminal = draw_app(&mut app, 80, 30);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
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
                "│     │          │          │          │         │ ∙ 17:30 交週報    │         │",
                "│     │          │          │          │         │         │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  43 │ 19       │ 20       │ 21       │ 22      │ 23      │ 24      │ 25      │",
                "│     │          │ ∙ 東京出 │ ∙ 東京出 │ ∙ 東京出│ ∙ 東京出│ ∙ 東京出│ ∙ 東京出│",
                "│     │          │          │          │ ∙ 買伴手禮        │         │         │",
                "├─────┼──────────┼──────────┼──────────┼─────────┼─────────┼─────────┼─────────┤",
                "│  44 │ 26       │ 27       │ 28       │ 29      │ 30      │ 31      │ 1 11月  │",
                "│     │          │          │ ∙ 主管出 │ ∙ 主管出│ ∙ 主管出│ ∙ 主管出│ ∙ 主管出│",
                "│     │          │          │          │         │         │         │ ∙ 準備月│",
                "│     │          │          │          │         │         │         │         │",
                "│     │          │          │          │         │         │         │         │",
                "╰─────┴──────────┴──────────┴──────────┴─────────┴─────────┴─────────┴─────────╯",
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
            ]
        );
        // 範圍裡的日期加底線：10/20（二）「 20」在 x 19～20、y 19；10/19（一）在 x 8～9 不加
        let buffer = terminal.backend().buffer();
        let underlined = |x: u16, y: u16| buffer[(x, y)].modifier.contains(Modifier::UNDERLINED);
        assert!(underlined(20, 19));
        assert!(!underlined(9, 19));
    }

    /// 任務頁、今天 2026/10/17、任務檔是範例檔的 app（寫錯的地方也記著，跟 `main` 一樣）
    fn tasks_app(lang: Lang) -> App {
        let parsed = crate::tasks::parse(include_str!("../testdata/tasks.md"));
        let mut app = App::new(lang, date(2026, 10, 17), parsed.items);
        app.warnings = parsed.warnings;
        app.view = View::Tasks;
        app.focus = Focus::Tasks;
        app
    }

    #[test]
    fn tasks_page_lists_the_file_as_a_tree() {
        // 70 × 30：整份任務檔照日曆列出來，沒日期的也列；子任務收著（▸）、後面寫子任務做完幾件；
        // 右邊日期、時間各一欄、上下對齊（沒時間的留空白）；最下面一列是選到的那件（第一件）原本的文字
        let mut app = tasks_app(Lang::ZhTw);
        assert_eq!(
            text_rows(draw_app(&mut app, 70, 30).backend().buffer()),
            [
                "╭────────────────────────────────────────────────────────────────────╮",
                "│ 任務                                                      1 處寫錯 │",
                "│────────────────────────────────────────────────────────────────────│",
                "│ 付款 API 改版                                                      │",
                "│   [x] 跟 PM 對齊退款流程規格                            9/28       │",
                "│   [x] 拆 ticket、估人天                                 9/30 10:00 │",
                "│ ▸ [ ] 實作退款 API  1/4                                10/14       │",
                "│   [ ] Code review：Amy 的 PR #482                      10/12 14:00 │",
                "│   [ ] 跟 SRE 確認監控告警                              10/13 15:00 │",
                "│   [ ] 回覆 Kevin 的 error code 問題                    10/14       │",
                "│   [x] 修 staging 的 DB migration                        10/8       │",
                "│    ∙  上線 freeze                                      10/23       │",
                "│   [ ] 準備上線清冊 @2026-10-23                         10/22       │",
                "│   [ ] 寫 runbook，參考 https://wiki.example.com/payments/runbook   │",
                "│    ∙  Release v2.3                                     10/27 09:30 │",
                "│ 例行                                                               │",
                "│    ∙  週會                                             10/12 10:00 │",
                "│    ∙  週會                                             10/19 10:00 │",
                "│    ∙  週會                                             10/26 10:00 │",
                "│   [ ] 交週報                                           10/16 17:30 │",
                "│   [ ] 整理本週筆記                                     10/16       │",
                "│   [ ] 更新 release note 草稿                           10/16       │",
                "│    ∙  團隊聚餐                                         10/16 18:30 │",
                "│    ∙  跟 PM 開需求會議                                  10/8 14:00 │",
                "│    ∙  部門月會                                          10/8 16:00 │",
                "│    ∙  團隊 offsite                                     10/29       │",
                "│────────────────────────────────────────────────────────────────────│",
                "│ 第 4 行  - [x] 跟 PM 對齊退款流程規格 @2026-09-28                  │",
                "╰────────────────────────────────────────────────────────────────────╯",
                " jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說",
            ]
        );
        // 展開幾件（▾）、選到第 61 行（三層深）：往下捲到它看得到、離下緣留 2 列
        let mut app = tasks_app(Lang::En);
        app.expanded.extend([6, 59, 60]);
        app.tasks_line = Some(61);
        assert_eq!(
            text_rows(draw_app(&mut app, 70, 16).backend().buffer()),
            [
                "╭────────────────────────────────────────────────────────────────────╮",
                "│ Tasks                                                 1 mistake(s) │",
                "│────────────────────────────────────────────────────────────────────│",
                "│    ∙  光復節                                           10/25       │",
                "│ 學習 Rust                                                          │",
                "│   [x] 讀完 The Book 第 4 章 ownership                   10/3       │",
                "│   [ ] 讀 Ratatui 的 Layout 文件                                    │",
                "│ ▾ [ ] 寫 roh-cal 的任務檔 parser  0/1                  10/11       │",
                "│   ▾ [ ] 每條規則都寫 test  0/1                                     │",
                "│       [ ] 測 `@Rukis` 不算日期                                     │",
                "│    ∙  RustConf 線上直播 @Rukis                         10/29 23:00 │",
                "│    ∙  讀書會：Rust async                               10/14 19:30 │",
                "│────────────────────────────────────────────────────────────────────│",
                "│ Line 61  - [ ] 測 `@Rukis` 不算日期                                │",
                "╰────────────────────────────────────────────────────────────────────╯",
                " jk move  hl fold  Space check  e edit  a add  s calendars  Esc calend",
            ]
        );
    }

    #[test]
    fn tasks_page_colors_and_clicks() {
        // 不是今年的日期前面加年份；做完的灰、劃線；過去的行程灰、不劃線；日期用那天的顏色；選到的那列墊底色
        let text = "# 工作\n- [ ] 交週報 @2026-10-16\n- [x] 修 bug @2026-10-15\n- 國慶日 @2026-10-10\n\
                    - 跨年 @2027-01-02\n  - [ ] 買票\n";
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        app.view = View::Tasks;
        app.focus = Focus::Tasks;
        app.screen = Rect::new(0, 0, 40, 14);
        let terminal = draw_app(&mut app, 40, 14);
        let rows = text_rows(terminal.backend().buffer());
        assert_eq!(rows[7], "│ ▸  ∙  跨年  0/1             2027/1/2 │");
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
        // 標題：工作（label 色、粗體）
        assert_eq!(buffer[(2, 3)].fg, theme.label);
        assert!(buffer[(2, 3)].modifier.contains(Modifier::BOLD));
        // 交週報（y 4）：沒做完 → 一般的字、日期（x 33～37）橘色；選到的那列整列墊底色
        assert_eq!(cell(8, 4), ("交", theme.text, false));
        assert_eq!(cell(34, 4), ("0", theme.has_item, false));
        assert_eq!(buffer[(1, 4)].bg, theme.cursor_line);
        assert_eq!(buffer[(1, 5)].bg, Color::Reset);
        // 修 bug（y 5）：做完 → 灰、劃線；國慶日（y 6）：過去的行程 → 灰、不劃線
        assert_eq!(cell(8, 5), ("修", theme.muted, true));
        assert_eq!(cell(8, 6), ("國", theme.muted, false));
        // ▸ 淡色
        assert_eq!(cell(2, 7), ("▸", theme.muted, false));
        // 點跨年（y 7）那列：選它；點 ▸（x 2）：展開
        let click = |app: &mut App, column, row| {
            use ratatui::crossterm::event::{
                KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
            };
            app.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            });
        };
        click(&mut app, 10, 7);
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("跨年")
        );
        assert!(app.expanded.is_empty());
        click(&mut app, 2, 7);
        assert!(app.expanded.contains(&5));
        // 點標題那列：不理
        click(&mut app, 5, 3);
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("跨年")
        );
    }

    #[test]
    fn tasks_page_points_at_format_mistakes() {
        // 表頭右邊寫有幾處寫錯；選到寫錯的那一行（第 43 行 @2026-13-01）：最下面一列改說哪裡錯，標題紅字
        let mut app = tasks_app(Lang::ZhTw);
        app.tasks_line = Some(43);
        let terminal = draw_app(&mut app, 70, 16);
        assert_eq!(
            text_rows(terminal.backend().buffer()),
            [
                "╭────────────────────────────────────────────────────────────────────╮",
                "│ 任務                                                      1 處寫錯 │",
                "│────────────────────────────────────────────────────────────────────│",
                "│   [ ] 繳信用卡                                         10/25       │",
                "│    ∙  看牙醫                                           10/14 10:30 │",
                "│   [ ] 整理書櫃                                                     │",
                "│   [ ] 買生日禮物給媽媽                                  11/2       │",
                "│   [ ] 預約年度健檢                                     10/16       │",
                "│    ∙  媽媽生日                                         10/29       │",
                "│   [ ] 繳房屋稅 @2026-13-01                                         │",
                "│   搬家                                                             │",
                "│   [ ] 找搬家公司估價                                    11/7       │",
                "│────────────────────────────────────────────────────────────────────│",
                "│ 第 43 行：@2026-13-01：沒有這一天                                  │",
                "╰────────────────────────────────────────────────────────────────────╯",
                " jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說",
            ]
        );
        let buffer = terminal.backend().buffer();
        let theme = Theme::catppuccin_mocha();
        // 表頭的「1 處寫錯」、最下面那列都是紅字
        assert_eq!(buffer[(60, 1)].fg, theme.error);
        assert_eq!(buffer[(2, 13)].fg, theme.error);
        // 繳房屋稅那列（y 9）的標題紅字（標題從 x 8 開始）；別列不是
        assert_eq!(buffer[(8, 9)].fg, theme.error);
        assert_eq!(buffer[(8, 8)].fg, theme.text);
    }

    #[test]
    fn tasks_page_with_the_side_menu() {
        // 121 格寬：side menu 在左邊；沒勾的日曆（未分組、付款 API 改版）整組不列
        let mut app = tasks_app(Lang::ZhTw);
        app.groups[0].shown = false;
        app.groups[1].shown = false;
        assert_eq!(
            text_rows(draw_app(&mut app, 121, 14).backend().buffer()),
            [
                "╭───────────────────────╮╭──────────────────────────────────────────────────────────────────────────────────────────────╮",
                "│ 日曆                  ││ 任務                                                                                1 處寫錯 │",
                "│───────────────────────││──────────────────────────────────────────────────────────────────────────────────────────────│",
                "│ ☐ 付款 API 改版    12 ││ 個人                                                                                         │",
                "│ ☐ 例行             14 ││   [ ] 繳信用卡                                                                   10/25       │",
                "│ ☑ 個人              5 ││    ∙  看牙醫                                                                     10/14 10:30 │",
                "│ ☑ 搬家              4 ││   [ ] 整理書櫃                                                                               │",
                "│ ☑ 國定假日          2 ││   [ ] 買生日禮物給媽媽                                                            11/2       │",
                "│ ☑ 學習 Rust         4 ││   [ ] 預約年度健檢                                                               10/16       │",
                "│ ☑ 2026 年底日本旅行 5 ││    ∙  媽媽生日                                                                   10/29       │",
                "│                       ││──────────────────────────────────────────────────────────────────────────────────────────────│",
                "│                       ││ 第 37 行  - [ ] 繳信用卡 @2026-10-25                                                         │",
                "╰───────────────────────╯╰──────────────────────────────────────────────────────────────────────────────────────────────╯",
                " jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說明  , 設定  q 離開",
            ]
        );
    }

    #[test]
    fn the_day_popup_lists_every_item_of_the_day_over_the_big_calendar() {
        // 60 × 20、版型 3、10/17 有 4 件：跳窗在大月曆（60 × 19）正中間，30 格寬（最窄）、6 列高 → x 15、y 6；
        // 上框線寫日期，一件一列（跟版型 1 右邊一樣），選到的第一件日期加括號；最下面一列換成跳窗的鍵
        let text = "- [ ] 交週報 @2026-10-17 17:30\n- 週會 @2026-10-17 10:00\n\
                    - [x] 整理書櫃 @2026-10-17\n- 出差 @2026-10-16..2026-10-18\n";
        let mut app = month_app(Lang::ZhTw, date(2026, 10, 17), text);
        app.focus = Focus::Day;
        let terminal = draw_app(&mut app, 60, 20);
        let buffer = terminal.backend().buffer();
        assert_eq!(
            area_rows(buffer, Rect::new(15, 6, 30, 6)),
            [
                "╭ 2026/10/17（六） ──────────╮",
                "│[10/16~18]出差              │",
                "│ 10/17 整理書櫃             │",
                "│ 10/17 10:00 週會           │",
                "│ 10/17 17:30 交週報         │",
                "╰────────────────────────────╯",
            ]
        );
        // 操作說明 61 格，60 格寬時最後一個字切掉
        assert_eq!(
            text_rows(buffer)[19],
            " jk 移動  Space 打勾  e 編輯  a 新增  r 改期  d 刪除  Esc 關"
        );
        // 框線是鍵盤所在的顏色、括號是選取的紅
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(15, 6)].fg, theme.focus);
        assert_eq!(buffer[(16, 7)].fg, theme.selected);
        // 太矮（60 × 6，大月曆 5 列）：框裡放 3 件，選最後一件時往下捲一件；右框線上有捲軸
        app.day_index = 3;
        let terminal = draw_app(&mut app, 60, 6);
        let buffer = terminal.backend().buffer();
        assert_eq!(
            area_rows(buffer, Rect::new(15, 1, 29, 3)),
            [
                "│ 10/17 整理書櫃             ",
                "│ 10/17 10:00 週會           ",
                "│[10/17]17:30 交週報         ",
            ]
        );
        assert!((1..4).any(|y| buffer[(44, y)].symbol() == "┃"));
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
                " hjkl move  Tab switch  [] month  t today  s calendars  ? help  , settings  q qu",
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
                " hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
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
