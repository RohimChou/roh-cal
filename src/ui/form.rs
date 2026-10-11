//! 新增表單：清單裡按 `a` 打開，疊在月曆框的正中間（跟當天清單一樣），兩欄：標題、日期

use crate::app::{App, Form, FormField, FormProblem};
use crate::ui::notice_line;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, BorderType, Clear, Padding, Paragraph},
};
use tui_input::Input;

/// 框的寬度（含框線）：跟當天清單一樣，月曆框左右各留 `MARGIN` 格，最多 `MAX_WIDTH`
const MARGIN: u16 = 2;
const MAX_WIDTH: u16 = 60;
/// 框線和內容之間上下左右各空 1 格（跟當天清單一樣）
const PADDING: Padding = Padding::uniform(1);
/// 欄位名那欄的寬度（`title` 5 格、`標題` 4 格，後面再空 2 格）
const LABEL_WIDTH: u16 = 7;

/// 畫新增表單：一欄一行（欄位名 + 打的字），游標那欄的欄位名醒目、terminal 的游標停在打字的位置。
/// 下框線：欄位寫錯的原因 > 提示（例：檔案在別處改過）> 操作說明
pub fn render(frame: &mut Frame, form: &Form, app: &App, calendar: Rect) {
    let theme = &app.theme;
    let width = calendar.width.saturating_sub(2 * MARGIN).min(MAX_WIDTH);
    let height = 2 + PADDING.top + PADDING.bottom + 2;
    let area = calendar.centered(Constraint::Length(width), Constraint::Length(height));

    let bottom = match form.problem {
        Some(FormProblem::EmptyTitle) => Some(app.texts.form_empty_title),
        Some(FormProblem::BadDate) => Some(app.texts.form_bad_date),
        None => None,
    }
    .map(|problem| Line::styled(format!(" {problem} "), theme.title))
    .or_else(|| notice_line(app))
    .unwrap_or_else(|| Line::styled(format!(" {} ", app.texts.form_hint), theme.hint));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .padding(PADDING)
        .title_top(Line::styled(
            format!(" {} ", app.texts.form_title),
            theme.title,
        ))
        .title_bottom(bottom);
    let inner = block.inner(area);
    // 先清掉底下的畫面，框才不會跟清單的字疊在一起
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let [title_row, date_row] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(inner);
    let [title_label, date_label] = app.texts.form_labels;
    let fields = [
        (FormField::Title, title_row, title_label, &form.title),
        (FormField::Date, date_row, date_label, &form.date),
    ];
    for (field, row, label, input) in fields {
        let focused = form.focus == field;
        let [label_area, value_area] =
            Layout::horizontal([Constraint::Length(LABEL_WIDTH), Constraint::Fill(1)]).areas(row);
        let label_style = if focused { theme.title } else { theme.hint };
        frame.render_widget(Paragraph::new(Line::styled(label, label_style)), label_area);
        render_input(frame, input, value_area, focused);
    }
}

/// 畫一欄打的字：字比欄位長時往左捲，游標一定看得到；`focused` 的那欄把 terminal 的游標放在打字的位置
fn render_input(frame: &mut Frame, input: &Input, area: Rect, focused: bool) {
    // 留 1 格給游標（游標在最後面時也看得到）
    let scroll = input.visual_scroll(usize::from(area.width.saturating_sub(1)));
    let scroll_cells = u16::try_from(scroll).unwrap_or(u16::MAX);
    frame.render_widget(
        Paragraph::new(input.value()).scroll((0, scroll_cells)),
        area,
    );
    if focused {
        let cursor = u16::try_from(input.visual_cursor() - scroll).unwrap_or(u16::MAX);
        frame.set_cursor_position((area.x + cursor, area.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{DayList, Overlay};
    use crate::i18n::Texts;
    use crate::ui::rows;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend};

    /// 畫一個表單（標題打了 `title`、游標在 `focus`、`problem` 是沒通過的檢查）在 80 × 24 的畫面上，
    /// 回傳畫面每一行的文字和 terminal
    fn draw(
        texts: Texts,
        title: &str,
        focus: FormField,
        problem: Option<FormProblem>,
    ) -> (Vec<String>, Terminal<TestBackend>) {
        let mut app = App::on_date(date(2026, 10, 10), texts, Vec::new());
        app.overlay = Some(Overlay::Form(Form {
            title: Input::new(title.to_string()),
            date: Input::new("2026-10-12".to_string()),
            focus,
            back: DayList {
                date: date(2026, 10, 12),
                selected: 0,
                confirm_delete: None,
            },
            problem,
        }));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        (rows(terminal.backend().buffer()), terminal)
    }

    /// 含有 `text` 的那一行是第幾行，和它在那一行從第幾格開始（這些字都佔 1 格）
    fn position_of(rows: &[String], text: &str) -> (u16, u16) {
        let y = rows.iter().position(|row| row.contains(text)).expect(text);
        let x = rows[y][..rows[y].find(text).unwrap()].chars().count();
        (u16::try_from(x).unwrap(), u16::try_from(y).unwrap())
    }

    #[test]
    fn form_shows_both_fields_and_the_cursor_in_the_title() {
        let (rows, mut terminal) = draw(Texts::en(), "abc", FormField::Title, None);
        let text = rows.join("\n");
        for expected in [
            " new item ",
            "title  abc",
            "date   2026-10-12",
            "Tab next · Enter save · Esc cancel",
        ] {
            assert!(text.contains(expected), "{expected}");
        }
        // 游標停在標題最後一個字的後面
        let (x, y) = position_of(&rows, "abc");
        terminal.backend_mut().assert_cursor_position((x + 3, y));
    }

    #[test]
    fn cursor_follows_the_focused_field() {
        let (rows, mut terminal) = draw(Texts::en(), "abc", FormField::Date, None);
        let (x, y) = position_of(&rows, "2026-10-12");
        terminal.backend_mut().assert_cursor_position((x + 10, y));
    }

    #[test]
    fn problems_take_the_place_of_the_hint() {
        let (rows, _) = draw(
            Texts::en(),
            "",
            FormField::Title,
            Some(FormProblem::EmptyTitle),
        );
        let text = rows.join("\n");
        assert!(text.contains("the title is empty"));
        assert!(!text.contains("Tab next"));
        let (rows, _) = draw(
            Texts::zh_tw(),
            "x",
            FormField::Date,
            Some(FormProblem::BadDate),
        );
        let text = rows.join("\n");
        assert!(text.contains("新增項目"));
        assert!(text.contains("標題   x"));
        assert!(text.contains("日期要像 2026-10-12"));
    }
}
