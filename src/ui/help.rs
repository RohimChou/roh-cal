//! 說明頁：按 `?` 打開，疊在畫面正中間（排法照 tuxedo）
//!
//! 左右兩欄列按鍵（左：移動；右：編輯）；畫面不夠寬放兩欄時，改成一欄由上往下排。內容在 `Texts::help`

use crate::app::App;
use crate::i18n::HelpSection;
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::Constraint,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Padding, Paragraph},
};

/// 按鍵那欄的寬度（按鍵都是英文字，一個字一格）
const KEY_WIDTH: usize = 12;
/// 兩欄之間至少空幾格
const GAP: usize = 4;
/// 框線和內容之間：上下各空 1 行（左邊靠段落名的縮排留空）
const PADDING: Padding = Padding::vertical(1);

/// 畫說明頁：大小剛好包住內容，放在整個 terminal 的正中間（說明頁不屬於哪一天，不用對齊月曆框）；
/// terminal 太矮時下面的會被切掉
pub fn render(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let help = app.texts.help;
    let screen = frame.area();

    // 兩欄：右欄從 `split` 那格開始；整個寬度放不下（扣掉框線）就改成一欄
    let left = section_lines(help.left, theme);
    let right = section_lines(help.right, theme);
    let split = width(&left) + GAP;
    let lines = if split + width(&right) + 2 <= usize::from(screen.width) {
        side_by_side(left, right, split)
    } else {
        // 一欄：右欄接在左欄下面，中間空一行
        let mut lines = left;
        lines.push(Line::default());
        lines.extend(right);
        lines
    };

    // 大小：最寬那行 + 右邊留 1 格 + 框線 2；行數 + 上下 padding + 框線 2（超過畫面會被縮到畫面大小）
    let box_width = u16::try_from(width(&lines) + 1 + 2).unwrap_or(u16::MAX);
    let box_height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(PADDING.top + PADDING.bottom + 2);
    let area = screen.centered(
        Constraint::Length(box_width),
        Constraint::Length(box_height),
    );

    // 上框線：`roh-cal · keys`（app 名稱醒目，後面淡淡的）；下框線：操作說明
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("roh-cal", theme.title),
        Span::styled(format!(" · {} ", help.title), theme.hint),
    ]);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .padding(PADDING)
        .title_top(title)
        .title_bottom(Line::styled(format!(" {} ", help.hint), theme.hint));
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// 幾段按鍵說明排成一行一行：段落名（縮排 2）、每個按鍵一行（縮排 4，按鍵那欄固定寬），段落之間空一行。
/// 段落名是空字串時，那一行留空（給右欄對齊左欄的段落名用）
fn section_lines(sections: &[HelpSection], theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            lines.push(Line::default());
        }
        lines.push(Line::styled(
            format!("  {}", section.title),
            theme.help_section,
        ));
        for &(key, action) in section.keys {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(format!("{key:<KEY_WIDTH$}"), theme.help_key),
                Span::raw(action),
            ]));
        }
    }
    lines
}

/// 最寬那行佔幾格（中文字算 2 格）
fn width(lines: &[Line]) -> usize {
    lines.iter().map(Line::width).max().unwrap_or(0)
}

/// 左右兩欄並排：左欄每一行補空白到 `split` 格，再接上右欄同一行；比較短的那欄下面留空
fn side_by_side(
    left: Vec<Line<'static>>,
    right: Vec<Line<'static>>,
    split: usize,
) -> Vec<Line<'static>> {
    let rows = left.len().max(right.len());
    let mut left = left.into_iter();
    let mut right = right.into_iter();
    (0..rows)
        .map(|_| {
            let mut line = left.next().unwrap_or_default();
            let padding = split.saturating_sub(line.width());
            line.push_span(Span::raw(" ".repeat(padding)));
            for span in right.next().unwrap_or_default() {
                line.push_span(span);
            }
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::app::{App, DayList, Overlay};
    use crate::i18n::Texts;
    use crate::ui::rows;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend};

    /// 打開說明頁（`back` = 從哪個清單打開的），畫在 `width` × 40 的畫面上，回傳每一行的文字
    fn draw(texts: Texts, back: Option<DayList>, width: u16) -> Vec<String> {
        let mut app = App::on_date(date(2026, 10, 10), texts, Vec::new());
        app.overlay = Some(Overlay::Help { back });
        let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        rows(terminal.backend().buffer())
    }

    /// 含有 `text` 的那一行是第幾行
    fn row_of(rows: &[String], text: &str) -> usize {
        rows.iter().position(|row| row.contains(text)).expect(text)
    }

    #[test]
    fn help_lists_navigation_and_editing_in_two_columns() {
        let rows = draw(Texts::en(), None, 100);
        let text = rows.join("\n");
        for expected in [
            "roh-cal · keys",
            // 按鍵那欄 12 格："o / Enter" 9 個字 + 3 個空白
            "t           jump to today",
            "o / Enter   open detail",
            "Option+q    open settings",
            "Ctrl+C      quit from anywhere",
            "d           delete (with subtasks)",
            "?/q/Esc close",
        ] {
            assert!(text.contains(expected), "{expected}");
        }
        // 兩欄：NAVIGATION 和 EDITING 在同一行；上框線和段落名之間空一行（padding）
        let top = row_of(&rows, "roh-cal · keys");
        assert_eq!(row_of(&rows, "NAVIGATION"), top + 2);
        assert_eq!(row_of(&rows, "EDITING"), top + 2);
        // 右欄對齊：EDITING 和它底下的按鍵從固定的地方開始（段落名縮排 2、按鍵縮排 4）
        assert_eq!(
            column_of(&rows, "EDITING") + 2,
            column_of(&rows, "Space / x")
        );
        // 下框線和最後一行之間也空一行
        assert_eq!(row_of(&rows, "?/q/Esc close"), row_of(&rows, "Ctrl+C") + 2);
    }

    /// `text` 在它那一行從第幾格開始（這裡的字都佔 1 格）
    fn column_of(rows: &[String], text: &str) -> usize {
        let row = &rows[row_of(rows, text)];
        row[..row.find(text).expect(text)].chars().count()
    }

    #[test]
    fn section_names_and_keys_use_the_theme_colors() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.overlay = Some(Overlay::Help { back: None });
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rows = rows(buffer);
        // 找某段字第一個字那格的顏色
        let fg = |text: &str| {
            let y = row_of(&rows, text);
            let x = column_of(&rows, text);
            buffer[(u16::try_from(x).unwrap(), u16::try_from(y).unwrap())].fg
        };
        assert_eq!(Some(fg("NAVIGATION")), app.theme.help_section.fg);
        assert_eq!(Some(fg("o / Enter")), app.theme.help_key.fg);
    }

    #[test]
    fn narrow_screen_stacks_the_columns() {
        let rows = draw(Texts::en(), None, 60);
        // 一欄：EDITING 在 NAVIGATION 的最後一列下面，中間空一行
        assert_eq!(row_of(&rows, "EDITING"), row_of(&rows, "Ctrl+C") + 2);
    }

    #[test]
    fn help_in_traditional_chinese() {
        let text = draw(Texts::zh_tw(), None, 100).join("\n");
        for expected in [
            "roh-cal · 按鍵",
            "移動",
            "o / Enter   打開當天清單",
            "編輯",
            "d           刪除（連子任務）",
        ] {
            assert!(text.contains(expected), "{expected}");
        }
    }

    #[test]
    fn help_from_the_day_list_keeps_the_list_underneath() {
        let list = DayList {
            date: date(2026, 10, 12),
            selected: 0,
            confirm_delete: None,
        };
        let text = draw(Texts::en(), Some(list), 100).join("\n");
        assert!(text.contains("2026-10-12 Mon"));
        assert!(text.contains("NAVIGATION"));
    }
}
