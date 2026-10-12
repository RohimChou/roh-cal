//! 設定頁：`,` 打開，疊在畫面右下角的一張清單

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
    let lines = row_lines(panel, app);
    let Placement {
        area: overlay,
        offset,
        visible,
    } = placement(panel, &lines, frame.area());

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

/// 滑鼠點在 (`column`, `row`) 的是設定頁的第幾列（`ROWS` 的 index）；點在段落名、操作說明、框線、框外面回傳 `None`。
/// 位置跟 `render` 用同一個 `placement` 算，畫面捲動過也對得上
pub fn row_at(panel: Panel, app: &App, column: u16, row: u16) -> Option<usize> {
    let lines = row_lines(panel, app);
    let Placement {
        area,
        offset,
        visible,
    } = placement(panel, &lines, app.screen);
    // 框線裡面：左右框線、上框線都不算
    let inside = column > area.x && column < area.right().saturating_sub(1) && row > area.y;
    if !inside {
        return None;
    }
    // 框內第幾行；超過放得下的行數 = 操作說明那行或下框線
    let line = usize::from(row - area.y - 1);
    if line >= visible {
        return None;
    }
    lines.get(offset + line).and_then(|&(index, _)| index)
}

/// 設定頁在畫面上的位置
struct Placement {
    /// 框的範圍（含框線），貼在畫面右下角
    area: Rect,
    /// 捲動：前面跳過幾行（段落名也算一行）
    offset: usize,
    /// 框內放得下幾行（扣掉框線和最下面的操作說明）
    visible: usize,
}

/// 算設定頁放在哪、捲到哪：`lines` 是 `row_lines` 的結果、`screen` 是整個畫面
fn placement(panel: Panel, lines: &[(Option<usize>, Line)], screen: Rect) -> Placement {
    // 高度：框線 2 + 段落名和列 + 最下面 1 行操作提示；視窗不夠高就縮
    let wanted = 2 + lines.len() as u16 + HINT_LINES;
    let height = wanted.min(screen.height);
    let width = WIDTH.min(screen.width);
    let area = Rect {
        x: screen.right().saturating_sub(width),
        y: screen.bottom().saturating_sub(height),
        width,
        height,
    };
    let visible = usize::from(height.saturating_sub(2 + HINT_LINES)).max(1);
    // 捲動：選到的列（算進段落名之後是第幾行）超出下面就往下捲
    let selected_line = lines
        .iter()
        .position(|&(index, _)| index == Some(panel.selected))
        .unwrap_or(0);
    let offset = selected_line.saturating_sub(visible - 1);
    Placement {
        area,
        offset,
        visible,
    }
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
    use crate::app::Overlay;
    use crate::i18n::Texts;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend};

    /// 畫出設定頁，回傳每一行的文字（去掉尾端空白）
    fn draw(selected: usize, height: u16) -> Vec<String> {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.overlay = Some(Overlay::Settings(Panel {
            selected,
            ..Panel::default()
        }));
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
        app.overlay = Some(Overlay::Settings(Panel {
            confirm_reset_all: true,
            ..Panel::default()
        }));
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

    /// `text` 在畫面那一行從第幾格開始（這裡的字都佔 1 格）
    fn column_of(line: &str, text: &str) -> u16 {
        u16::try_from(line[..line.find(text).expect(text)].chars().count()).unwrap()
    }

    #[test]
    fn row_at_finds_the_row_under_the_mouse() {
        let panel = Panel::default();
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.screen = Rect::new(0, 0, 80, 30);
        let screen = draw(0, 30);
        // 每一列：照順序在畫面上找它的名字（grid_lines 有兩列：開關和顏色，所以從上一列的下一行接著找）；
        // 點名字、點那一行右邊的空白都算那一列
        let mut y = 0;
        for (index, row) in ROWS.iter().enumerate() {
            let label = format!("   {:<LABEL_WIDTH$}", row.label());
            y += screen[y..]
                .iter()
                .position(|line| line.contains(&label))
                .unwrap_or_else(|| panic!("{}", row.label()));
            let x = column_of(&screen[y], &label) + 3;
            let row_y = u16::try_from(y).unwrap();
            assert_eq!(
                row_at(panel, &app, x, row_y),
                Some(index),
                "{}",
                row.label()
            );
            assert_eq!(
                row_at(panel, &app, 78, row_y),
                Some(index),
                "{}",
                row.label()
            );
            y += 1;
        }
        // 框線的位置也從畫面上找：上框線那行有標題，左右是圓角；下框線是最後一個 ╰（月曆框的下框線也有 ╰）
        let top = screen
            .iter()
            .position(|line| line.contains("settings (q / Esc)"))
            .unwrap();
        let (left, right) = (column_of(&screen[top], "╭"), column_of(&screen[top], "╮"));
        let bottom = screen.iter().rposition(|line| line.contains("╰")).unwrap();
        let line_of = |text: &str| {
            u16::try_from(screen.iter().position(|line| line.contains(text)).unwrap()).unwrap()
        };
        let week_numbers = line_of("week_numbers");
        // 段落名、操作說明、上下左右框線、框外：都不算
        for (x, y) in [
            (left + 4, line_of("[month]")),
            (left + 4, line_of("j/k select")),
            (left + 4, u16::try_from(top).unwrap()),
            (left + 4, u16::try_from(bottom).unwrap()),
            (left, week_numbers),
            (right, week_numbers),
            (left - 1, week_numbers),
            (0, 0),
        ] {
            assert_eq!(row_at(panel, &app, x, y), None, "({x}, {y})");
        }
    }

    #[test]
    fn row_at_follows_the_scrolling() {
        // 12 行高、選最後一列（sunday）：前面捲掉了，框內第一行不是第一列
        let panel = Panel {
            selected: ROWS.len() - 1,
            ..Panel::default()
        };
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.screen = Rect::new(0, 0, 80, 12);
        let screen = draw(panel.selected, 12);
        let y = screen
            .iter()
            .position(|line| line.contains("sunday"))
            .unwrap();
        let x = column_of(&screen[y], "sunday");
        let y = u16::try_from(y).unwrap();
        assert_eq!(row_at(panel, &app, x, y), Some(ROWS.len() - 1));
        // 上一行是 saturday
        assert_eq!(row_at(panel, &app, x, y - 1), Some(ROWS.len() - 2));
    }
}
