//! side menu：最左邊的日曆清單（日曆 = 任務檔的分組），terminal 夠寬才畫

use super::planner::inner_areas;
use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// side menu 的寬度（含左右框線）
pub const WIDTH: u16 = 22;

/// 畫 side menu：表頭「日曆」，下面一個日曆一列，例：「 ☑ 工作            12 」
///
/// 數字是那個日曆有幾件事在月曆上（有日期的）；勾選的是 `☑`、沒勾的是 `☐`；名字太長就切掉，放不下的日曆不畫
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.line));
    frame.render_widget(block, area);
    // 表頭跟另外兩個框一樣：一列字 + 一條線
    let [header, rule, body] = inner_areas(area);
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
    for (group, y) in app.groups.iter().zip(body.y..body.bottom()) {
        let row = Rect::new(body.x, y, body.width, 1);
        let count = app
            .items
            .iter()
            .filter(|item| item.date.is_some() && group.includes(item))
            .count();
        let count = format!(" {count} ");
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
        let name = group.name.as_deref().unwrap_or(texts.ungrouped);
        frame.render_widget(
            Line::from(vec![
                Span::styled(format!(" {mark} "), Style::new().fg(mark_color)),
                Span::styled(name, Style::new().fg(name_color)),
            ]),
            name_area,
        );
        frame.render_widget(
            Line::styled(count, Style::new().fg(theme.muted)),
            count_area,
        );
    }
}
