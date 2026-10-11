//! 新增 / 編輯表單：月曆或當天清單按 `a` 新增、清單按 `Enter` / `o` / `e` 編輯，疊在月曆框的正中間（跟當天清單一樣），兩欄：標題、日期；游標在日期欄時下面出現小月曆

use crate::app::{App, Form, FormField, FormProblem};
use crate::calendar::monday_of;
use crate::ui::{WEEKDAYS, notice_line, weekend_style};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
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
/// 日期欄下面的小月曆：星期列 1 行 + 3 週
const PICKER_ROWS: u16 = 4;

/// 畫新增表單：一欄一行（欄位名 + 打的字），游標那欄的欄位名醒目、terminal 的游標停在打字的位置；
/// 游標在日期欄時，日期下面出現小月曆。
/// 下框線：欄位寫錯的原因 > 提示（例：檔案在別處改過）> 操作說明
pub fn render(frame: &mut Frame, form: &Form, app: &App, calendar: Rect) {
    let theme = &app.theme;
    let width = calendar.width.saturating_sub(2 * MARGIN).min(MAX_WIDTH);
    // 小月曆只在游標在日期欄時出現，框跟著變高（一樣置中，跟底下的清單對齊）
    let picker = form.focus == FormField::Date;
    let picker_rows = if picker { PICKER_ROWS } else { 0 };
    let height = 2 + PADDING.top + PADDING.bottom + 2 + picker_rows;
    let area = calendar.centered(Constraint::Length(width), Constraint::Length(height));

    let bottom = match form.problem {
        Some(FormProblem::EmptyTitle) => Some(app.texts.form_empty_title),
        Some(FormProblem::DateInTitle) => Some(app.texts.form_date_in_title),
        Some(FormProblem::BadDate) => Some(app.texts.form_bad_date),
        None => None,
    }
    .map(|problem| Line::styled(format!(" {problem} "), theme.error))
    .or_else(|| notice_line(app))
    .unwrap_or_else(|| Line::styled(format!(" {} ", app.texts.form_hint), theme.hint));
    let heading = if form.editing.is_some() {
        app.texts.form_edit_title
    } else {
        app.texts.form_title
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .padding(PADDING)
        .title_top(Line::styled(format!(" {heading} "), theme.title))
        .title_bottom(bottom);
    let inner = block.inner(area);
    // 先清掉底下的畫面，框才不會跟清單的字疊在一起
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let [title_row, date_row, picker_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(picker_rows),
    ])
    .areas(inner);
    let [title_label, date_label] = app.texts.form_labels;
    let fields = [
        (FormField::Title, title_row, title_label, &form.title),
        (FormField::Date, date_row, date_label, &form.date),
    ];
    for (field, row, label, input) in fields {
        let focused = form.focus == field;
        let [label_area, mut value_area] =
            Layout::horizontal([Constraint::Length(LABEL_WIDTH), Constraint::Fill(1)]).areas(row);
        let label_style = if focused { theme.title } else { theme.hint };
        frame.render_widget(Paragraph::new(Line::styled(label, label_style)), label_area);
        // 游標在日期欄：最右邊提示可以用 hjkl 換日期（打的字放在左邊剩下的位置，不會被蓋住）
        if focused && field == FormField::Date {
            let hint = Line::styled(app.texts.form_date_hint, theme.hint);
            let hint_width = u16::try_from(hint.width()).unwrap_or(u16::MAX);
            let [input_area, hint_area] =
                Layout::horizontal([Constraint::Fill(1), Constraint::Length(hint_width)])
                    .areas(value_area);
            frame.render_widget(Paragraph::new(hint), hint_area);
            value_area = input_area;
        }
        render_input(frame, input, value_area, focused);
    }
    if picker {
        // 小月曆跟日期欄打的字對齊（欄位名那欄空著）
        let [_, picker_area] =
            Layout::horizontal([Constraint::Length(LABEL_WIDTH), Constraint::Fill(1)])
                .areas(picker_area);
        frame.render_widget(Paragraph::new(picker_lines(form.picked, app)), picker_area);
    }
}

/// 小月曆的每一行：星期列 + 3 週（`picked` 那週在中間）；顏色跟月曆一樣：
/// 週末上色，再依序疊上「別的月份變淡 → 今天 → 選到的那天」
fn picker_lines(picked: Date, app: &App) -> Vec<Line<'static>> {
    let theme = &app.theme;
    // 星期列：每格 4 格寬 = 名稱靠右 3 格 + 1 個空白，跟下面日期的位置對齊
    let mut header = Line::default();
    for (name, weekday) in app.texts.weekdays.into_iter().zip(WEEKDAYS) {
        let style = theme.weekday_header.patch(weekend_style(weekday, theme));
        let padding = " ".repeat(3_usize.saturating_sub(Span::raw(name).width()));
        header.push_span(Span::styled(format!("{padding}{name} "), style));
    }
    let mut lines = vec![header];
    let middle = monday_of(picked);
    for weeks in [-1, 0, 1] {
        // 超出 jiff 能表示的範圍（日期上下限附近）那週就不畫
        let Ok(monday) = middle.checked_add(weeks.weeks()) else {
            continue;
        };
        let mut week = Line::default();
        for date in monday.series(1.day()).take(7) {
            let mut style = theme.day.patch(weekend_style(date.weekday(), theme));
            let layers = [
                (
                    date.first_of_month() != picked.first_of_month(),
                    theme.other_month,
                ),
                (date == app.today, theme.today),
                (date == picked, theme.selected),
            ];
            for (applies, layer) in layers {
                if applies {
                    style = style.patch(layer);
                }
            }
            // 一格 4 格寬：「空白 + 2 位數日期 + 空白」整格上色，選到那天的底色才是完整的一顆
            week.push_span(Span::styled(format!(" {:>2} ", date.day()), style));
        }
        lines.push(week);
    }
    lines
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
    use crate::theme::Theme;
    use crate::ui::rows;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend};

    /// 畫一個表單（標題打了 `title`、日期欄是 `picked`、游標在 `focus`、`problem` 是沒通過的檢查）
    /// 在 80 × 24 的畫面上（今天是 10/10），回傳畫面每一行的文字和 terminal
    fn draw(
        texts: Texts,
        title: &str,
        picked: Date,
        focus: FormField,
        problem: Option<FormProblem>,
    ) -> (Vec<String>, Terminal<TestBackend>) {
        let mut app = App::on_date(date(2026, 10, 10), texts, Vec::new());
        app.overlay = Some(Overlay::Form(Box::new(Form {
            title: Input::new(title.to_string()),
            date: Input::new(picked.to_string()),
            picked,
            focus,
            back: Some(DayList {
                date: date(2026, 10, 12),
                selected: 0,
                confirm_delete: None,
            }),
            editing: None,
            problem,
        })));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        (rows(terminal.backend().buffer()), terminal)
    }

    /// 大部分 test 的日期欄：2026-10-12（星期一）
    const OCT_12: Date = date(2026, 10, 12);

    /// 含有 `text` 的那一行是第幾行，和它在那一行從第幾格開始（這些字都佔 1 格）
    fn position_of(rows: &[String], text: &str) -> (u16, u16) {
        let y = rows.iter().position(|row| row.contains(text)).expect(text);
        let x = rows[y][..rows[y].find(text).unwrap()].chars().count();
        (u16::try_from(x).unwrap(), u16::try_from(y).unwrap())
    }

    #[test]
    fn form_shows_both_fields_and_the_cursor_in_the_title() {
        let (rows, mut terminal) = draw(Texts::en(), "abc", OCT_12, FormField::Title, None);
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
        let (rows, mut terminal) = draw(Texts::en(), "abc", OCT_12, FormField::Date, None);
        let (x, y) = position_of(&rows, "2026-10-12");
        terminal.backend_mut().assert_cursor_position((x + 10, y));
    }

    #[test]
    fn problems_take_the_place_of_the_hint() {
        let (rows, terminal) = draw(
            Texts::en(),
            "",
            OCT_12,
            FormField::Title,
            Some(FormProblem::EmptyTitle),
        );
        let text = rows.join("\n");
        assert!(text.contains("the title is empty"));
        assert!(!text.contains("Tab next"));
        // 用出錯的顏色（醒目的紅），不是標題的顏色
        let (x, y) = position_of(&rows, "the title is empty");
        let cell = &terminal.backend().buffer()[(x, y)];
        assert_eq!(cell.fg, Theme::default().error.fg.unwrap());
        assert_ne!(cell.fg, Theme::default().title.fg.unwrap());
        let (rows, _) = draw(
            Texts::zh_tw(),
            "x",
            OCT_12,
            FormField::Date,
            Some(FormProblem::BadDate),
        );
        let text = rows.join("\n");
        assert!(text.contains("新增項目"));
        assert!(text.contains("標題   x"));
        assert!(text.contains("日期要像 2026-10-12"));
        // 標題裡有 @日期：中英文都整句看得到（下框線放得下）
        for (texts, expected) in [
            (Texts::en(), "the title can't contain an @date"),
            (Texts::zh_tw(), "標題不能有 @日期，請用日期欄"),
        ] {
            let problem = Some(FormProblem::DateInTitle);
            let (rows, _) = draw(texts, "x", OCT_12, FormField::Title, problem);
            assert!(rows.join("\n").contains(expected), "{expected}");
        }
    }

    #[test]
    fn the_date_field_shows_a_small_calendar_under_the_date() {
        // 游標在標題：沒有小月曆、沒有 hjkl 的提示
        let (rows, _) = draw(Texts::en(), "abc", OCT_12, FormField::Title, None);
        let text = rows.join("\n");
        assert!(!text.contains("MON TUE WED THU FRI SAT SUN"));
        assert!(!text.contains("hjkl change date"));
        // 游標在日期：日期那行最右邊是提示；下面是星期列 + 前一週、10/12 那週、下一週，跟日期打的字對齊
        let (rows, terminal) = draw(Texts::en(), "abc", OCT_12, FormField::Date, None);
        let (x, y) = position_of(&rows, "2026-10-12");
        assert!(rows[usize::from(y)].contains("hjkl change date"));
        let expected = [
            "MON TUE WED THU FRI SAT SUN",
            "  5   6   7   8   9  10  11",
            " 12  13  14  15  16  17  18",
            " 19  20  21  22  23  24  25",
        ];
        for (row, line) in (1..).zip(expected) {
            assert_eq!(position_of(&rows, line), (x, y + row), "{line}");
        }
        // 選到的 12 號：整格（4 格）是選取的底色，13 號不是
        let theme = Theme::default();
        let buffer = terminal.backend().buffer();
        for column in x..x + 4 {
            assert_eq!(buffer[(column, y + 3)].bg, theme.selected.bg.unwrap());
        }
        assert_ne!(buffer[(x + 4, y + 3)].bg, theme.selected.bg.unwrap());
        // 今天（10/10，星期六）：今天的顏色蓋過週末的顏色
        assert_eq!(buffer[(x + 5 * 4 + 1, y + 2)].symbol(), "1");
        assert_eq!(buffer[(x + 5 * 4 + 1, y + 2)].fg, theme.today.fg.unwrap());
    }

    #[test]
    fn days_of_other_months_are_dimmed_in_the_small_calendar() {
        // 10/30（星期五）：那週的星期日是 11/1 → 變淡（蓋過週末的顏色）；10/31 星期六 → 週末的顏色
        let (rows, terminal) = draw(
            Texts::en(),
            "abc",
            date(2026, 10, 30),
            FormField::Date,
            None,
        );
        let (x, y) = position_of(&rows, " 26  27  28  29  30  31   1");
        let theme = Theme::default();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(x + 6 * 4 + 2, y)].symbol(), "1");
        assert_eq!(buffer[(x + 6 * 4 + 2, y)].fg, theme.other_month.fg.unwrap());
        assert_eq!(buffer[(x + 5 * 4 + 1, y)].symbol(), "3");
        assert_eq!(buffer[(x + 5 * 4 + 1, y)].fg, theme.saturday.fg.unwrap());
        // 上一週、下一週：10/19～25、11/2～8
        assert_eq!(position_of(&rows, " 19  20  21  22  23  24  25").1, y - 1);
        assert_eq!(position_of(&rows, "  2   3   4   5   6   7   8").1, y + 1);
    }

    #[test]
    fn the_form_never_has_the_day_list_under_it() {
        // 10/12 有 5 個項目：清單比表單高，畫在底下的話清單的標題（日期 + 星期）會露在表單上面；
        // 從清單、從月曆打開都不畫
        let text: String = (1..=5)
            .map(|n| format!("- 項目{n} @2026-10-12\n"))
            .collect();
        let list = DayList {
            date: OCT_12,
            selected: 0,
            confirm_delete: None,
        };
        for back in [Some(list), None] {
            let items = crate::tasks::parse(&text).items;
            let mut app = App::on_date(OCT_12, Texts::en(), items);
            app.overlay = Some(Overlay::Form(Box::new(Form {
                title: Input::default(),
                date: Input::new(OCT_12.to_string()),
                picked: OCT_12,
                focus: FormField::Title,
                back,
                editing: None,
                problem: None,
            })));
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| crate::ui::render(frame, &app))
                .unwrap();
            let text = rows(terminal.backend().buffer()).join("\n");
            assert!(text.contains(" new item "), "{back:?}");
            assert!(!text.contains("2026-10-12 Mon"), "{back:?}");
        }
    }

    #[test]
    fn the_edit_form_says_edit_in_its_title() {
        let list = DayList {
            date: OCT_12,
            selected: 0,
            confirm_delete: None,
        };
        for (texts, heading) in [(Texts::en(), " edit item "), (Texts::zh_tw(), " 編輯項目 ")] {
            let item = crate::tasks::parse("- [ ] 週會 @2026-10-12")
                .items
                .remove(0);
            let mut app = App::on_date(OCT_12, texts, vec![item.clone()]);
            app.overlay = Some(Overlay::Form(Box::new(Form {
                title: Input::new(item.title.clone()),
                date: Input::new(OCT_12.to_string()),
                picked: OCT_12,
                focus: FormField::Title,
                back: Some(list),
                editing: Some(item),
                problem: None,
            })));
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| crate::ui::render(frame, &app))
                .unwrap();
            let text = rows(terminal.backend().buffer()).join("\n");
            assert!(text.contains(heading), "{heading}");
            assert!(!text.contains(" new item ") && !text.contains(" 新增項目 "));
        }
    }
}
