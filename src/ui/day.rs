//! 版型 3 的「那天的事」跳窗：疊在大月曆正中間，一件事一列（跟版型 1 右邊的事項框同一種寫法），上框線寫選取的日期

use super::{clear_wide_left_of, months::scrolled_top, planner::item_line, render_scrollbar};
use crate::app::App;
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Clear},
};

/// 跳窗最窄幾格（含框線）：上框線的日期放得下
const MIN_WIDTH: u16 = 30;

/// 跳窗（含框線）在大月曆（`main`）上的位置：正中間；寬度跟著最長的那件（最窄 `MIN_WIDTH`），
/// 高度 = 那天有幾件 + 上下框線；都不超過大月曆。畫畫面和滑鼠換算位置都用這個
pub fn area(app: &App, main: Rect) -> Rect {
    let items = app.items_on(app.selected);
    let widest = items
        .iter()
        .map(|item| item_line(item, app, false).width())
        .max()
        .unwrap_or(0);
    // 左右框線 + 最後留一格空白
    let width = u16::try_from(widest + 3)
        .unwrap_or(u16::MAX)
        .max(MIN_WIDTH)
        .min(main.width);
    let height = u16::try_from(items.len() + 2)
        .unwrap_or(u16::MAX)
        .min(main.height);
    Rect::new(
        main.x + (main.width - width) / 2,
        main.y + (main.height - height) / 2,
        width,
        height,
    )
}

/// 畫跳窗：先捲到選到的那件看得到（選到的跑出框外才捲，點看得到的那件不會捲走；記在 `app.day_top`），
/// 清掉底下的大月曆，再一件一列；選到的那件日期加括號 `[10/17]`（跟版型 1 右邊一樣）；放不下時右框線上有捲軸
pub fn render(frame: &mut Frame, main: Rect, app: &mut App) {
    let area = area(app, main);
    let visible = i64::from(Block::bordered().inner(area).height);
    let top = scrolled_top(
        i64::try_from(app.day_top).unwrap_or(0),
        i64::try_from(app.day_index).unwrap_or(0),
        visible,
        0,
    );
    // 不捲過頭：最後一件貼著底就好
    let max = i64::try_from(app.items_on(app.selected).len()).unwrap_or(0) - visible;
    app.day_top = usize::try_from(top.min(max).max(0)).unwrap_or(0);
    // 接下來只讀
    let app = &*app;
    let theme = &app.theme;
    let texts = app.lang.texts();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(Line::styled(
            format!(" {} ", texts.date_title(app.selected)),
            Style::new().fg(theme.text),
        ));
    let inner = block.inner(area);
    clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    let items = app.items_on(app.selected);
    let visible = usize::from(inner.height);
    let top = app.day_top;
    let rows = items.iter().enumerate().skip(top);
    for ((index, item), y) in rows.zip(inner.y..inner.bottom()) {
        frame.render_widget(
            item_line(item, app, index == app.day_index),
            Rect {
                y,
                height: 1,
                ..inner
            },
        );
    }
    if items.len() > visible {
        let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        render_scrollbar(
            frame,
            area,
            to_u16(top),
            to_u16(items.len() - visible),
            inner.height,
            theme,
        );
    }
}

/// 滑鼠點在 (`column`, `row`) 的是跳窗裡第幾件（`items_on(selected)` 裡的位置）；點在框線、框外、最後一件下面是 `None`
pub fn item_at(app: &App, main: Rect, column: u16, row: u16) -> Option<usize> {
    let inner = Block::bordered().inner(area(app, main));
    if !inner.contains(Position::new(column, row)) {
        return None;
    }
    let index = app.day_top + usize::from(row - inner.y);
    (index < app.items_on(app.selected).len()).then_some(index)
}
