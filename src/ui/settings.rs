//! 設定頁：Option+q 打開，疊在畫面右下角的一張清單

use crate::app::App;
use crate::settings::{Panel, ROWS};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
};

/// 框的寬度（含框線）
const WIDTH: u16 = 40;
/// 左邊名字那欄的寬度
const LABEL_WIDTH: usize = 18;

/// 畫設定頁：一列一個設定，選到的那列整行底色；列太多放不下時只顯示一段，選到的列一定在框內
pub fn render(frame: &mut Frame, panel: Panel, app: &App) {
    let theme = &app.theme;
    let area = frame.area();
    // 高度：框線 2 + 列 + 最下面 1 行操作提示；視窗不夠高就縮
    let wanted = 2 + ROWS.len() as u16 + 1;
    let height = wanted.min(area.height);
    let width = WIDTH.min(area.width);
    let overlay = Rect {
        x: area.right().saturating_sub(width),
        y: area.bottom().saturating_sub(height),
        width,
        height,
    };
    // 框內放得下幾列（扣掉框線和提示）
    let visible = usize::from(height.saturating_sub(3)).max(1);
    // 捲動：選到的列超出下面就往下捲
    let offset = panel.selected.saturating_sub(visible - 1);

    let mut lines: Vec<Line> = ROWS
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .map(|(index, &row)| {
            let (value, color) = row.value(&app.settings, theme);
            let mut spans = vec![Span::raw(format!(" {:<LABEL_WIDTH$}", row.label()))];
            if let Some(color) = color {
                // 色塊：兩格實心方塊，用那個顏色畫
                spans.push(Span::styled("██ ", Style::new().fg(color)));
            }
            spans.push(Span::raw(value));
            let mut line = Line::from(spans);
            if index == panel.selected {
                line.style = theme.cell_selected;
            }
            line
        })
        .collect();
    lines.push(Line::styled(" j/k select · h/l change", theme.hint));

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .title_top(Line::styled(" settings (Option+q / Esc) ", theme.title));
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    frame.render_widget(Clear, overlay);
    frame.render_widget(Paragraph::new(lines).block(block), overlay);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Texts;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend};

    /// 畫出設定頁，回傳每一行的文字（去掉尾端空白）
    fn draw(selected: usize, height: u16) -> Vec<String> {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.panel = Some(Panel { selected });
        let mut terminal = Terminal::new(TestBackend::new(80, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..80)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn panel_lists_every_row_when_the_screen_is_tall_enough() {
        let screen = draw(0, 30);
        let text = screen.join("\n");
        assert!(text.contains("settings (Option+q / Esc)"));
        assert!(text.contains("grid lines        on"));
        assert!(text.contains("item              ██ subtext1"));
        assert!(text.contains("weekday_header    ██ subtext0"));
        assert!(text.contains("j/k select · h/l change"));
    }

    #[test]
    fn panel_scrolls_so_the_selected_row_stays_visible() {
        // 12 行高：框線 2 + 提示 1 → 只放得下 9 列；選最後一列（第 14 列）時前面的捲掉
        let screen = draw(ROWS.len() - 1, 12);
        let text = screen.join("\n");
        assert!(!text.contains("grid lines"));
        assert!(text.contains("weekday_header"));
    }
}
