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
const WIDTH: u16 = 44;
/// 左邊名字那欄的寬度
const LABEL_WIDTH: usize = 18;
/// 最下面操作提示的行數
const HINT_LINES: u16 = 1;

/// 畫設定頁：照設定檔分段（`[month]`、`[colors]`），一列一個設定，選到的那列整行底色；
/// 放不下時只顯示一段，選到的列一定在框內
pub fn render(frame: &mut Frame, panel: Panel, app: &App) {
    let theme = &app.theme;
    let area = frame.area();
    let lines = row_lines(panel, app);
    // 高度：框線 2 + 段落名和列 + 最下面 1 行操作提示；視窗不夠高就縮
    let wanted = 2 + lines.len() as u16 + HINT_LINES;
    let height = wanted.min(area.height);
    let width = WIDTH.min(area.width);
    let overlay = Rect {
        x: area.right().saturating_sub(width),
        y: area.bottom().saturating_sub(height),
        width,
        height,
    };
    // 框內放得下幾行（扣掉框線和提示）
    let visible = usize::from(height.saturating_sub(2 + HINT_LINES)).max(1);
    // 捲動：選到的列（算進段落名之後是第幾行）超出下面就往下捲
    let selected_line = lines
        .iter()
        .position(|&(index, _)| index == Some(panel.selected))
        .unwrap_or(0);
    let offset = selected_line.saturating_sub(visible - 1);

    let mut lines: Vec<Line> = lines
        .into_iter()
        .skip(offset)
        .take(visible)
        .map(|(_, line)| line)
        .collect();
    // 最下面 1 行：平常是操作說明（r 還原一列、R 全部還原）；按了 R 就換成確認的問題（醒目的顏色）
    if panel.confirm_reset_all {
        lines.push(Line::styled(" reset all? y = yes, other = no", theme.title));
    } else {
        lines.push(Line::styled(
            " j/k select · h/l change · r/R reset/all",
            theme.hint,
        ));
    }

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .title_top(Line::styled(" settings (q / Esc) ", theme.title));
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    frame.render_widget(Clear, overlay);
    frame.render_widget(Paragraph::new(lines).block(block), overlay);
}

/// 設定頁的每一行，照設定檔的樣子：換段的地方先插一行段落名（例：`[month]`），後面接那段的列。
/// 每行附上它是 `ROWS` 的第幾列（段落名是 `None`，不能選）
fn row_lines(panel: Panel, app: &App) -> Vec<(Option<usize>, Line<'static>)> {
    let theme = &app.theme;
    let mut lines = Vec::new();
    let mut section = None;
    for (index, &row) in ROWS.iter().enumerate() {
        if section != Some(row.section()) {
            section = Some(row.section());
            // 段落名用跟星期列一樣的安靜顏色
            let header = Line::styled(format!(" [{}]", row.section()), theme.weekday_header);
            lines.push((None, header));
        }
        let (value, color) = row.value(&app.settings, theme);
        let mut spans = vec![Span::raw(format!("   {:<LABEL_WIDTH$}", row.label()))];
        if let Some(color) = color {
            // 色塊：兩格實心方塊，用那個顏色畫
            spans.push(Span::styled("██ ", Style::new().fg(color)));
        }
        spans.push(Span::raw(value));
        let mut line = Line::from(spans);
        if index == panel.selected {
            line.style = theme.selected_bg;
        }
        lines.push((Some(index), line));
    }
    lines
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
        app.panel = Some(Panel {
            selected,
            ..Panel::default()
        });
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
        assert!(text.contains("settings (q / Esc)"));
        // 跟設定檔一樣分段，名字一模一樣
        assert!(text.contains("[month]"));
        assert!(text.contains("[colors]"));
        assert!(text.contains("grid_lines        off"));
        assert!(text.contains("item              ██ subtext1"));
        assert!(text.contains("sunday            ██ blue"));
        assert!(text.contains("j/k select · h/l change · r/R reset/all"));
    }

    #[test]
    fn panel_scrolls_so_the_selected_row_stays_visible() {
        // 12 行高：框線 2 + 提示 1 → 只放得下 9 行（段落名也算一行）；選最後一列（sunday）時前面的捲掉
        let screen = draw(ROWS.len() - 1, 12);
        let text = screen.join("\n");
        assert!(!text.contains("[month]"));
        assert!(!text.contains("grid_lines"));
        assert!(text.contains("sunday"));
        // 選第一列：段落名 [month] 也在畫面上
        let text = draw(0, 12).join("\n");
        assert!(text.contains("[month]"));
        assert!(text.contains("grid_lines"));
    }

    #[test]
    fn reset_all_asks_for_confirmation_at_the_bottom() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.panel = Some(Panel {
            confirm_reset_all: true,
            ..Panel::default()
        });
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = (0..30)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .map(|(x, y)| buffer[(x, y)].symbol().to_string())
            .collect();
        // 操作說明換成確認的問題
        assert!(text.contains("reset all? y = yes, other = no"));
        assert!(!text.contains("r/R reset/all"));
    }
}
