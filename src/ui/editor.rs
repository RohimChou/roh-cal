//! One-line editor: a box in the middle of the main view (like the day popup), with some blank space inside
//! the border when there is room; the `#` list of calendars opens right under the typed text

use crate::app::{App, Editor, Target};
use crate::edit::{Place, Problem};
use crate::tasks::Priority;
use ratatui::{
    Frame,
    layout::{Margin, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
};

/// The widest the editor gets: a usual task line fits with room to spare; a longer one scrolls
const MAX_WIDTH: u16 = 80;

/// Below this terminal width the editor takes the whole width (no margins, no blank cells inside)
const NARROW: u16 = 30;

/// Blank space between the border and the text when there is room: one row above and below (when the
/// terminal is tall enough), one cell left and right (when it is at least `NARROW` wide)
const PADDING: Margin = Margin::new(1, 1);

/// The most calendars the `#` list shows at once (more: it scrolls)
const LIST_ROWS: usize = 8;

/// The editor's area on the main view (`main`: right of the side menu, above the key-hint row): in the
/// middle both ways, like the day popup; 2 cells from each side, at most `MAX_WIDTH` wide, as tall as what
/// is in it (see `layout`). Used for drawing and for the mouse (a click outside the box = cancel)
pub fn area(app: &App, main: Rect, editor: &Editor) -> Rect {
    layout(app, main, editor).0
}

/// The editor's area (see `area`), where the text goes inside it (the border taken off, and the padding too
/// when there is room for it), and the lines under the typed text (`body`). Inside: the typed text, a
/// blank row, then the body; when the main view is too short, no padding rows, and the body is cut
fn layout(app: &App, main: Rect, editor: &Editor) -> (Rect, Rect, Vec<Line<'static>>) {
    let wide = main.width >= NARROW;
    let width = if wide {
        (main.width - 4).min(MAX_WIDTH)
    } else {
        main.width
    };
    let horizontal = if wide { PADDING.horizontal } else { 0 };
    let body = body(editor, app, width.saturating_sub(2 + 2 * horizontal));
    let rows = 2 + u16::try_from(body.len()).unwrap_or(u16::MAX);
    let vertical = if main.height >= rows + 2 + 2 * PADDING.vertical {
        PADDING.vertical
    } else {
        0
    };
    let height = (rows + 2 + 2 * vertical).min(main.height);
    let area = Rect::new(
        main.x + (main.width - width) / 2,
        main.y + (main.height - height) / 2,
        width,
        height,
    );
    let inner = Block::bordered()
        .inner(area)
        .inner(Margin::new(horizontal, vertical));
    (area, inner, body)
}

/// Where the typed text goes: the first row inside the box, after one blank cell
fn input_area(inner: Rect) -> Rect {
    Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(1),
        height: inner.height.min(1),
        ..inner
    }
}

/// How many cells the typed text is scrolled left (so the cursor stays visible, with one cell to spare)
fn input_scroll(editor: &Editor, input_area: Rect) -> usize {
    editor
        .input
        .visual_scroll(usize::from(input_area.width).saturating_sub(1))
}

/// The `#` list's box and which calendar is on its first row; `None` when the list is closed (or there is
/// no room for even one row). The box is right under the typed text with its left border under the `#`
/// (moved left when it would go past the right edge), as wide as the longest calendar or the keys on its
/// bottom border, at most `LIST_ROWS` calendars tall (it may cover the key hints at the bottom); it
/// scrolls so the chosen calendar shows
pub fn list_area(
    app: &App,
    screen: Rect,
    editor: &Editor,
    keys: &'static str,
) -> Option<(Rect, usize)> {
    let found = editor.suggestions()?;
    let (start, _) = editor.hash_word()?;
    let input_area = input_area(layout(app, super::areas(screen, app).main, editor).1);
    // Where the `#` is on screen: the cells before it, less what the typed text is scrolled
    let before: String = editor.input.value().chars().take(start).collect();
    let column = Span::raw(before)
        .width()
        .saturating_sub(input_scroll(editor, input_area));
    let column = input_area.x + u16::try_from(column).unwrap_or(u16::MAX);
    let names = editor.calendar_names();
    let widest = found
        .iter()
        .map(|found| Span::raw(&names[found.index]).width() + 4)
        .chain([super::border_hints(app, keys).width() + 2])
        .max()
        .unwrap_or(0);
    let width = u16::try_from(widest).unwrap_or(u16::MAX).min(screen.width);
    let y = input_area.y + 1;
    let rows = u16::try_from(found.len().min(LIST_ROWS)).unwrap_or(0);
    let height = (rows + 2).min(screen.bottom().saturating_sub(y));
    if height < 3 {
        return None;
    }
    let x = column
        .saturating_sub(1)
        .min(screen.right().saturating_sub(width));
    let index = editor.hash_list.index.min(found.len() - 1);
    let top = (index + 1).saturating_sub(usize::from(height - 2));
    Some((Rect::new(x, y, width, height), top))
}

/// Draws the editor: the top border says what it does (「新增事項」, 「編輯事項」, 「新增日曆」…); row 1 is the typed
/// text (cursor at the typing position, the text in its priority's color as soon as it has one); after a
/// blank row, the body (what it reads as, then what can be typed); the bottom border says how to save
pub fn render(frame: &mut Frame, editor: &Editor, app: &App) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let (area, inner, body) = layout(app, super::areas(frame.area(), app).main, editor);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(Line::styled(
            match &editor.target {
                Target::Edit { .. } => texts.edit_title(),
                Target::Add(place) => texts.add_title(matches!(place, Place::ChildOf(_))),
                Target::AddCalendar { .. } => texts.add_calendar_title,
                Target::RenameCalendar { .. } => texts.rename_calendar_title,
            },
            Style::new().fg(theme.label),
        ))
        .title_bottom(super::border_hints(app, texts.editor_keys()));
    // Clear the screen underneath and one blank cell around the box first (not the key-hint row at the
    // bottom), then draw the box
    let screen = frame.area();
    let bounds = Rect {
        height: screen.height.saturating_sub(1),
        ..screen
    };
    super::clear_with_ring(frame, area, bounds);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    // Typed text: scrolls left when too long so the cursor stays visible (one blank cell in front); in its
    // priority's color as soon as it has one (`!` `!!` `!!!`), like the item on screen
    let input_area = input_area(inner);
    let scroll = input_scroll(editor, input_area);
    let color = editor
        .priority()
        .map_or(theme.text, |priority| theme.priority(priority));
    frame.render_widget(
        Paragraph::new(Line::from(typed_spans(
            editor.input.value(),
            Style::new().fg(color),
        )))
        .scroll((0, u16::try_from(scroll).unwrap_or(u16::MAX))),
        input_area,
    );
    let cursor = editor.input.visual_cursor().saturating_sub(scroll);
    frame.set_cursor_position(Position::new(
        input_area.x + u16::try_from(cursor).unwrap_or(0),
        input_area.y,
    ));
    // The body, after a blank row (cut at the bottom when the terminal is too short)
    if inner.height > 2 {
        frame.render_widget(
            Paragraph::new(body),
            Rect {
                y: inner.y + 2,
                height: inner.height - 2,
                ..inner
            },
        );
    }
    render_list(frame, editor, app);
}

/// The typed text in `style`, with what is not the title in italics so it stands out: an `@date` or
/// `@date..date`, a time (`@10:00` on the selected day, or right after a date), `!` `!!` `!!!`, and a `#`
/// word (a calendar being picked)
fn typed_spans(text: &str, style: Style) -> Vec<Span<'static>> {
    let italic = style.add_modifier(Modifier::ITALIC);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut after_date = false;
    let mut rest = text;
    while !rest.is_empty() {
        // The next run of blanks, or of other characters (a word)
        let blank = rest.starts_with(char::is_whitespace);
        let end = rest
            .find(|c: char| c.is_whitespace() != blank)
            .unwrap_or(rest.len());
        let (run, next) = rest.split_at(end);
        rest = next;
        if blank {
            spans.push(Span::styled(run.to_string(), style));
            continue;
        }
        let date = run.strip_prefix('@').is_some_and(|word| {
            crate::tasks::looks_like_date(word) || crate::tasks::looks_like_range(word)
        });
        let time = run
            .strip_prefix('@')
            .is_some_and(crate::tasks::looks_like_time)
            || (after_date && crate::tasks::looks_like_time(run));
        let syntax = date
            || time
            || Priority::from_word(run).is_some()
            || (run.starts_with('#') && run.len() > 1);
        spans.push(Span::styled(
            run.to_string(),
            if syntax { italic } else { style },
        ));
        after_date = date;
    }
    spans
}

/// The lines under the typed text: what it reads as, field by field (`fields`), then after a blank line
/// what can be typed (`hints`, wrapped to `width` cells)
fn body(editor: &Editor, app: &App, width: u16) -> Vec<Line<'static>> {
    let mut lines = fields(editor, app);
    lines.push(Line::default());
    lines.extend(hints(editor, app, width));
    lines
}

/// What the typed text reads as, one field per line with its label in front (only the fields it has), e.g.
/// `標題  Pay rent`, `日期  2026/10/21（三） 10:00`, `優先度  高` (in its color), `日曆  Work > SFAP`; then what
/// is wrong, in red on its own line. An item is read leniently (a wrong date stays in the title as text), so
/// the fields show while it is being fixed; an empty title is left blank, an empty name is a gray hint, and
/// neither is called wrong until `Enter` could not save it (`Editor::tried`). A calendar: its name,
/// `類型  假日日曆` for a holiday calendar, `封存  2026/10/26（一）` for an archived one, the calendar it is
/// inside (`上層日曆  最外層` at the top level)
fn fields(editor: &Editor, app: &App) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let words = &texts.editor_words;
    let text = Style::new().fg(theme.text);
    let muted = Style::new().fg(theme.muted);
    let mut fields: Vec<(&str, Span<'static>)> = Vec::new();
    match &editor.target {
        Target::Edit { .. } | Target::Add(_) => {
            if let Some(item) = crate::tasks::parse(&editor.line()).items.into_iter().next() {
                fields.push((words.title, Span::styled(item.title.clone(), text)));
                if item.date.is_some() {
                    fields.push((words.date, Span::styled(texts.when(&item), text)));
                }
                if let Some(priority) = item.priority {
                    let level = match priority {
                        Priority::High => words.levels[0],
                        Priority::Medium => words.levels[1],
                        Priority::Low => words.levels[2],
                    };
                    let color = Style::new().fg(theme.priority(priority));
                    fields.push((words.priority, Span::styled(level, color)));
                }
            }
            // Where it goes: the item a subtask goes under, or the calendar
            match (&editor.target, editor.calendar()) {
                (Target::Add(Place::ChildOf(parent)), _) => {
                    fields.push((words.under, Span::styled(parent.title.clone(), text)));
                }
                (_, Some(path)) => {
                    fields.push((words.calendar, Span::styled(path.join(" > "), text)))
                }
                _ => {}
            }
        }
        Target::AddCalendar { parent, .. } | Target::RenameCalendar { parent, .. } => {
            let heading = crate::tasks::parse(&format!("# {}", editor.input.value().trim()))
                .headings
                .into_iter()
                .next()
                .map(|line| line.heading)
                .filter(|heading| !heading.name.is_empty());
            fields.push(match &heading {
                Some(heading) => (words.name, Span::styled(heading.name.clone(), text)),
                None => (words.name, Span::styled(words.name_placeholder, muted)),
            });
            if heading.as_ref().is_some_and(|heading| heading.holiday) {
                fields.push((words.kind, Span::styled(words.holiday, text)));
            }
            if let Some(day) = heading.as_ref().and_then(|heading| heading.archived) {
                fields.push((words.archived, Span::styled(texts.date_title(day), text)));
            }
            let inside = match parent.is_empty() {
                true => texts.top_level.to_string(),
                false => parent.join(" > "),
            };
            fields.push((words.inside, Span::styled(inside, text)));
        }
    }
    // Labels padded to the widest one (+ 2 cells), so the values line up
    let label_width = fields
        .iter()
        .map(|(label, _)| Span::raw(*label).width())
        .max()
        .unwrap_or(0)
        + 2;
    let mut lines: Vec<Line> = fields
        .into_iter()
        .map(|(label, value)| {
            let padding = " ".repeat(label_width - Span::raw(label).width());
            Line::from(vec![
                Span::raw(" "),
                Span::styled(format!("{label}{padding}"), muted),
                value,
            ])
        })
        .collect();
    let empty = matches!(
        editor.check(),
        Err(Problem::EmptyTitle | Problem::EmptyName)
    );
    if let Err(problem) = editor.check()
        && (!empty || editor.tried)
    {
        lines.push(Line::styled(
            texts.problem(&problem),
            Style::new().fg(theme.error),
        ));
    }
    lines
}

/// What can be typed, after `Syntax:` / 「語法：」: each thing as typed, in the help page's section color
/// (blue; the key color is orange in some themes, like a `!!` priority), then its label (as in the fields,
/// lower case), a `|` in the dimmer line color between two (like the bottom row), e.g.
/// `語法：@2026-10-19 日期 | @10:00 時間 | ! !! !!! 優先度 | #名字 日曆`
/// (today's date; `!` `!!` `!!!` each in its priority's color; `#` only when it can pick a calendar, not
/// for a subtask). A calendar: `語法：type:holiday 假日日曆 | archived:2026-10-19 封存 | #名字 上層日曆`
/// (today's date). Wrapped to `width` cells between the things, the next lines lined up under the first one
fn hints(editor: &Editor, app: &App, width: u16) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let words = &app.lang.texts().editor_words;
    let muted = Style::new().fg(theme.muted);
    let key = Style::new().fg(theme.help_section);
    // Each thing that can be typed: its spans (what to type, a blank, its label)
    let thing = |typed: String, label: &str| {
        vec![
            Span::styled(typed, key),
            Span::styled(format!(" {}", label.to_lowercase()), muted),
        ]
    };
    let mut things = match editor.target {
        Target::AddCalendar { .. } | Target::RenameCalendar { .. } => {
            vec![
                thing("type:holiday".to_string(), words.holiday),
                thing(format!("archived:{}", app.today), words.archived),
            ]
        }
        Target::Edit { .. } | Target::Add(_) => {
            let mut priority = Vec::new();
            for (marks, level) in [
                ("!", Priority::Low),
                (" !!", Priority::Medium),
                (" !!!", Priority::High),
            ] {
                priority.push(Span::styled(marks, Style::new().fg(theme.priority(level))));
            }
            priority.push(Span::styled(
                format!(" {}", words.priority.to_lowercase()),
                muted,
            ));
            vec![
                thing(format!("@{}", app.today), words.date),
                thing("@10:00".to_string(), words.time),
                priority,
            ]
        }
    };
    if !editor.hash_list.calendars.is_empty() {
        let label = match editor.target {
            Target::AddCalendar { .. } | Target::RenameCalendar { .. } => words.inside,
            Target::Edit { .. } | Target::Add(_) => words.calendar,
        };
        things.push(thing(words.name_key.to_string(), label));
    }
    let prefix = format!(" {}", words.syntax);
    let indent = Span::raw(prefix.clone()).width();
    let separator = super::hint_separator(app);
    let gap = separator.width();
    let mut lines = vec![vec![Span::styled(prefix, muted)]];
    let mut used = indent;
    for (index, spans) in things.into_iter().enumerate() {
        let size = Line::from(spans.clone()).width();
        // A `|` between things; a thing that does not fit goes on a new line under the first one
        if index > 0 && used + gap + size > usize::from(width) {
            lines.push(vec![Span::raw(" ".repeat(indent))]);
            used = indent;
        } else if index > 0 {
            if let Some(line) = lines.last_mut() {
                line.push(separator.clone());
            }
            used += gap;
        }
        if let Some(line) = lines.last_mut() {
            line.extend(spans);
        }
        used += size;
    }
    lines.into_iter().map(Line::from).collect()
}

/// Draws the `#` list over the editor (see `list_area`): one calendar per row, its full path; the characters
/// that match what was typed in the focus color and bold, the chosen row on the selection background; the
/// keys on the bottom border
fn render_list(frame: &mut Frame, editor: &Editor, app: &App) {
    let theme = &app.theme;
    let keys = app.lang.texts().hash_list_keys;
    let (Some((area, top)), Some(found)) = (
        list_area(app, frame.area(), editor, keys),
        editor.suggestions(),
    ) else {
        return;
    };
    let names = editor.calendar_names();
    let index = editor.hash_list.index.min(found.len() - 1);
    super::clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_bottom(super::border_hints(app, keys));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    for (row, (at, found)) in found
        .iter()
        .enumerate()
        .skip(top)
        .take(usize::from(inner.height))
        .enumerate()
    {
        let mut spans = vec![Span::raw(" ")];
        for (position, character) in names[found.index].chars().enumerate() {
            let style = if found.positions.contains(&position) {
                Style::new().fg(theme.focus).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme.text)
            };
            spans.push(Span::styled(character.to_string(), style));
        }
        // The chosen row: the background across the whole row, not only under the name
        let row_style = if at == index {
            Style::new().bg(theme.selection)
        } else {
            Style::new()
        };
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(row_style),
            Rect {
                y: inner.y + u16::try_from(row).unwrap_or(0),
                height: 1,
                ..inner
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words of `text` that `typed_spans` puts in italics
    fn italic_words(text: &str) -> Vec<String> {
        typed_spans(text, Style::new())
            .into_iter()
            .filter(|span| span.style.add_modifier.contains(Modifier::ITALIC))
            .map(|span| span.content.to_string())
            .collect()
    }

    #[test]
    fn the_syntax_in_the_typed_text_is_italic() {
        // A date and the time right after it, a priority, a `#` word; the title and `- [ ]` are not
        assert_eq!(
            italic_words("- [ ] Pay rent !!! @2026-10-21 10:00 #sf"),
            ["!!!", "@2026-10-21", "10:00", "#sf"]
        );
        // A range, a time on the selected day
        assert_eq!(
            italic_words("- Trip @2026-10-20..2026-10-25 and @10:00"),
            ["@2026-10-20..2026-10-25", "@10:00"]
        );
        // Not syntax: a time with no date before it, four `!`, a lone `#`, an e-mail-like `@`
        assert!(italic_words("- [ ] Call at 10:00 !!!! # me@example.com").is_empty());
    }
}
