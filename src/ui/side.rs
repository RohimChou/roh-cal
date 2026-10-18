//! side menu：最左邊的日曆清單（日曆 = 任務檔的分組），terminal 夠寬才畫

use super::{Click, months::scrolled_top, planner::inner_areas, render_scrollbar};
use crate::{
    app::{App, Focus, Group},
    texts::Texts,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// side menu 最窄幾格（含左右框線）；表頭一定放得下
const MIN_WIDTH: u16 = 22;
/// side menu 最寬幾格（含左右框線）；名字再長就切掉
const MAX_WIDTH: u16 = 30;

/// side menu 的寬度（含左右框線）：剛好放得下最長的那個日曆，例：「 ☑ 付款 API 改版 12 」，
/// 但在 `MIN_WIDTH` ～ `MAX_WIDTH` 之間
pub fn width(app: &App) -> u16 {
    let texts = app.lang.texts();
    let longest = app
        .groups
        .iter()
        // 「 ☑ 」3 格 + 名字 + 數字
        .map(|group| 3 + Span::raw(name(group, texts)).width() + count_text(app, group).len())
        .max()
        .unwrap_or(0);
    // 加上左右框線
    u16::try_from(longest + 2)
        .unwrap_or(MAX_WIDTH)
        .clamp(MIN_WIDTH, MAX_WIDTH)
}

/// 日曆的名字；檔案開頭、還沒有標題的項目是「未分組」
fn name<'a>(group: &'a Group, texts: &'static Texts) -> &'a str {
    group.name.as_deref().unwrap_or(texts.ungrouped)
}

/// 日曆右邊的數字（前後各留一格），例：「 12 」：那個日曆有幾件事有日期（沒勾也照算）
fn count_text(app: &App, group: &Group) -> String {
    let count = app
        .items
        .iter()
        .filter(|item| item.date.is_some() && group.includes(item))
        .count();
    format!(" {count} ")
}

/// 畫 side menu：表頭「日曆」，下面一個日曆一列，例：「 ☑ 工作            12 」
///
/// 勾選的是 `☑`、沒勾的是 `☐`（它的事不顯示）；名字太長（框已經是 `MAX_WIDTH`）就切掉。日曆多到放不下時，先捲到選到的那個看得到
/// （記在 `app.side_top`），右框線上畫捲軸。鍵盤在 side menu 時，框線亮起來、選到的那個日曆墊底色
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    // 表頭跟另外兩個框一樣：一列字 + 一條線
    let [header, rule, body] = inner_areas(area);
    let height = i64::from(body.height);
    let top = scrolled_top(
        i64::try_from(app.side_top).unwrap_or(0),
        i64::try_from(app.group_index).unwrap_or(0),
        height,
        // 選到的跑出框外才捲：點看得到的那個日曆不會捲走（再點一次才點得到同一個）
        0,
    );
    // 不捲過頭：最後一個日曆貼著底就好
    let max = i64::try_from(app.groups.len()).unwrap_or(0) - height;
    app.side_top = usize::try_from(top.min(max).max(0)).unwrap_or(0);
    // 接下來只讀
    let app = &*app;
    let theme = &app.theme;
    let texts = app.lang.texts();
    let focused = app.focus == Focus::Side;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { theme.focus } else { theme.line }));
    frame.render_widget(block, area);
    frame.render_widget(
        Line::styled(texts.header_side, Style::new().fg(theme.muted)),
        header,
    );
    frame.render_widget(
        Line::styled(
            line::HORIZONTAL.repeat(usize::from(rule.width)),
            Style::new().fg(theme.line),
        ),
        rule,
    );
    let visible = app.groups.iter().enumerate().skip(app.side_top);
    for ((index, group), y) in visible.zip(body.y..body.bottom()) {
        let row = Rect::new(body.x, y, body.width, 1);
        let count = count_text(app, group);
        // 名字在左、數字靠右；名字太長時切掉，不會蓋到數字（數字和空白都是 1 格寬）
        let [name_area, count_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(count.len() as u16)])
                .areas(row);
        // 勾選的：☑ 用 `checked` 的顏色、名字是一般的字；沒勾的：☐ 和名字都是淡色
        let (mark, mark_color, name_color) = if group.shown {
            ("☑", theme.checked, theme.text)
        } else {
            ("☐", theme.muted, theme.muted)
        };
        frame.render_widget(
            Line::from(vec![
                Span::styled(format!(" {mark} "), Style::new().fg(mark_color)),
                Span::styled(name(group, texts), Style::new().fg(name_color)),
            ]),
            name_area,
        );
        frame.render_widget(
            Line::styled(count, Style::new().fg(theme.muted)),
            count_area,
        );
        if focused && index == app.group_index {
            // 只換底色，字的顏色不動
            frame
                .buffer_mut()
                .set_style(row, Style::new().bg(theme.cursor_line));
        }
    }
    // 放不下：右框線上、日曆那幾列旁邊畫捲軸（框裡連一列都沒有就不畫）
    if let Ok(max) = u16::try_from(max)
        && max > 0
        && body.height > 0
    {
        let rows = Rect {
            y: body.y - 1,
            height: body.height + 2,
            ..area
        };
        let top = u16::try_from(app.side_top).unwrap_or(u16::MAX);
        render_scrollbar(frame, rows, top, max, body.height, theme);
    }
}

/// 滑鼠點在 side menu（`area`）的 (`column`, `row`) 點到哪個日曆；點在表頭、框線、下面的空白回傳 `None`
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let [_, _, body] = inner_areas(area);
    // 一個日曆一列，從 `body` 的第一列開始（捲過的話是第 `side_top` 個）
    if !body.contains(Position::new(column, row)) {
        return None;
    }
    let index = app.side_top + usize::from(row - body.y);
    (index < app.groups.len()).then_some(Click::Group(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;

    /// 任務檔是 `text` 的 app
    fn app_with(text: &str) -> App {
        App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        )
    }

    #[test]
    fn width_fits_the_longest_calendar_between_22_and_30() {
        // 沒有任務檔、名字很短（「 ☑ 個人 1 」只要 9 格）：最窄 22
        assert_eq!(width(&app_with("")), 22);
        assert_eq!(width(&app_with("# 個人\n- a @2026-10-10\n")), 22);
        // 「 ☑ 2026 年底日本旅行 1 」：3 + 17 + 3 = 23 格，加框線 25；最長的那個決定寬度
        let app = app_with("# 個人\n- a @2026-10-10\n# 2026 年底日本旅行\n- b @2026-12-24\n");
        assert_eq!(width(&app), 25);
        // 數字是兩位數：多 1 格
        let ten = "- x @2026-10-10\n".repeat(10);
        assert_eq!(width(&app_with(&format!("# 2026 年底日本旅行\n{ten}"))), 26);
        // 再長：最寬 30
        assert_eq!(
            width(&app_with("# 2026 年底的大小事情和明年計畫\n- a\n")),
            30
        );
    }
}
