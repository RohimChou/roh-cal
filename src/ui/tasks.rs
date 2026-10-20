//! Tasks page (`3`): the whole area right of the side menu, listing the entire task file by calendar, with
//! subtasks as a tree (a title too long for the row wraps onto the next lines); the bottom row shows the
//! chosen line's original text

use super::{
    Click,
    months::scrolled_to_range,
    planner::{date_text, day_color},
    title_span,
};
use crate::{
    app::{App, Focus, TaskRow},
    tasks::{Item, Kind},
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// How many rows to keep between the chosen item and the top and bottom edges; scroll only when there
/// are fewer (same as layout 1)
const SCROLLOFF: i64 = 2;

/// How the inside of the tasks page box is split: the header (a line of text + a rule), one item per row
/// in the middle, and at the bottom a rule + the chosen line's original text;
/// used both for drawing and for mapping mouse positions. Returns (header, header rule, middle, bottom
/// rule, bottom row); when too short, the middle is empty
fn inner_areas(area: Rect) -> [Rect; 5] {
    let inner = Block::bordered().inner(area);
    let row = |y: u16| Rect {
        y,
        height: 1,
        ..inner
    };
    if inner.height < 5 {
        return [
            row(inner.y),
            Rect::default(),
            Rect::default(),
            Rect::default(),
            Rect::default(),
        ];
    }
    let body = Rect {
        y: inner.y + 2,
        height: inner.height - 4,
        ..inner
    };
    [
        row(inner.y),
        row(inner.y + 1),
        body,
        row(inner.bottom() - 2),
        row(inner.bottom() - 1),
    ]
}

/// One row of the tasks page laid out at the page's width (see `layout_rows`)
enum RowLines {
    /// A calendar's heading: one line
    Heading,
    /// An item: its left part line by line (the title wrapped), and its right part (date and time columns)
    /// for the first line
    Item {
        left: Vec<Line<'static>>,
        right: Line<'static>,
    },
}

impl RowLines {
    /// How many lines it takes
    fn height(&self) -> usize {
        match self {
            Self::Heading => 1,
            Self::Item { left, .. } => left.len().max(1),
        }
    }
}

/// Every row of `rows` laid out at `width` cells, and the line each one starts on (counting from the first
/// row's first line)
fn layout_rows(app: &App, rows: &[TaskRow], width: u16) -> (Vec<RowLines>, Vec<usize>) {
    let columns = date_columns(app, rows);
    let layouts: Vec<RowLines> = rows
        .iter()
        .map(|row| match row {
            TaskRow::Heading { .. } => RowLines::Heading,
            TaskRow::Item {
                index,
                children,
                expanded,
            } => app.items.get(*index).map_or(
                RowLines::Item {
                    left: Vec::new(),
                    right: Line::default(),
                },
                |item| item_lines(app, item, *index, (*children, *expanded), columns, width),
            ),
        })
        .collect();
    let starts = layouts
        .iter()
        .scan(0, |line, layout| {
            let start = *line;
            *line += layout.height();
            Some(start)
        })
        .collect();
    (layouts, starts)
}

/// Draws the tasks page: first scroll so the chosen item's lines are all visible (stored in
/// `app.tasks_top`, counted in lines), then draw line by line
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let rows = app.tasks_rows();
    let [header, rule, body, bottom_rule, detail] = inner_areas(area);
    let height = i64::from(body.height);
    let cursor = app.tasks_cursor(&rows);
    let (layouts, starts) = layout_rows(app, &rows, body.width);
    let to_i64 = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    if let Some(cursor) = cursor {
        let first = to_i64(starts[cursor]);
        let last = first + to_i64(layouts[cursor].height()) - 1;
        let top = scrolled_to_range(to_i64(app.tasks_top), first, last, height, SCROLLOFF);
        // Do not overscroll: the last line just touches the bottom
        let total = starts
            .last()
            .zip(layouts.last())
            .map_or(0, |(start, layout)| start + layout.height());
        let max = to_i64(total) - height;
        app.tasks_top = usize::try_from(top.min(max).max(0)).unwrap_or(0);
    }
    // Read-only from here on
    let app = &*app;
    let theme = &app.theme;
    let texts = app.lang.texts();
    let focused = app.focus == Focus::Tasks;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { theme.focus } else { theme.line }));
    frame.render_widget(block, area);
    frame.render_widget(
        Line::styled(texts.header_tasks, Style::new().fg(theme.text)),
        header,
    );
    // Right side of the header: if the task file has mistakes, say how many (red text)
    if !app.warnings.is_empty() {
        frame.render_widget(
            Line::from(Span::styled(
                texts.mistakes(app.warnings.len()),
                Style::new().fg(theme.error),
            ))
            .right_aligned(),
            header,
        );
    }
    for area in [rule, bottom_rule] {
        frame.render_widget(
            Line::styled(
                line::HORIZONTAL.repeat(usize::from(area.width)),
                Style::new().fg(theme.line),
            ),
            area,
        );
    }
    for (index, (row, (layout, start))) in rows.iter().zip(layouts.iter().zip(&starts)).enumerate()
    {
        for part in 0..layout.height() {
            // Above the top of the box: skip; below its bottom: done
            let Some(offset) = (start + part).checked_sub(app.tasks_top) else {
                continue;
            };
            let Some(y) = u16::try_from(offset)
                .ok()
                .map(|offset| body.y + offset)
                .filter(|&y| y < body.bottom())
            else {
                break;
            };
            let area = Rect {
                y,
                height: 1,
                ..body
            };
            render_line(frame, area, app, (row, layout), part);
            // The chosen item's lines get a background (when the keyboard is on the tasks page)
            if focused && cursor == Some(index) {
                frame
                    .buffer_mut()
                    .set_style(area, Style::new().bg(theme.selection));
            }
        }
    }
    // Bottom: the chosen line's original text (shows how it is written in the file); if that line has a
    // mistake, say what is wrong instead (red text)
    if let Some(item) = app.tasks_chosen() {
        let line = match app
            .warnings
            .iter()
            .find(|warning| warning.line == item.line)
        {
            Some(warning) => Line::styled(
                texts.tasks_mistake(item.line, &warning.kind),
                Style::new().fg(theme.error),
            ),
            None => Line::styled(
                texts.tasks_detail(item.line, item.line_text.trim()),
                Style::new().fg(theme.muted),
            ),
        };
        frame.render_widget(line, detail);
    } else if let Some(heading) = cursor
        .and_then(|cursor| match &rows[cursor] {
            TaskRow::Heading { line, .. } => *line,
            TaskRow::Item { .. } => None,
        })
        .and_then(|line| app.headings.iter().find(|heading| heading.line == line))
    {
        // A calendar's heading row: its heading line as written
        frame.render_widget(
            Line::styled(
                texts.tasks_detail(heading.line, heading.text.trim()),
                Style::new().fg(theme.muted),
            ),
            detail,
        );
    }
}

/// Text for the date column on the right: dates not in this year get the year in front (e.g. `10/14`,
/// `10/20~25`, `2027/1/2`); `None` without a date
fn date_label(item: &Item, app: &App) -> Option<String> {
    let first = item.date?;
    let year = if first.year() == app.today.year() {
        String::new()
    } else {
        format!("{}/", first.year())
    };
    Some(format!("{year}{}", date_text(item)))
}

/// Widths of the two columns on the right (date column, time column); every row uses them so they line
/// up: date column = the longest date among the listed items;
/// time column = 6 cells (` 08:00`) if any item has a time, otherwise 0
fn date_columns(app: &App, rows: &[TaskRow]) -> (usize, usize) {
    rows.iter()
        .filter_map(|row| match row {
            TaskRow::Item { index, .. } => app.items.get(*index),
            TaskRow::Heading { .. } => None,
        })
        .fold((0, 0), |(date, time), item| {
            let label = date_label(item, app).map_or(0, |label| label.chars().count());
            (date.max(label), if item.time.is_some() { 6 } else { time })
        })
}

/// Draws line `part` of one row (`row` laid out as `layout`) in `area`: a heading is `▾` / `▸` (open /
/// collapsed; a blank when nothing is inside it, and for 「未分組」; muted) and its name (the label color,
/// not bold), indented 2 cells per calendar it is inside; a collapsed one ends with how many items are inside
/// (`App::calendar_item_count`, muted), e.g. 「▸ Recurring  14」. An item's line is that line of its left
/// part, and on the first line its date and time columns on the right
fn render_line(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    (row, layout): (&TaskRow, &RowLines),
    part: usize,
) {
    let theme = &app.theme;
    match (row, layout) {
        (
            TaskRow::Heading {
                name,
                depth,
                path,
                children,
                collapsed,
                ..
            },
            _,
        ) => {
            let marker = match (children, collapsed) {
                (false, _) => " ",
                (true, false) => "▾",
                (true, true) => "▸",
            };
            let name = name.as_deref().unwrap_or(app.lang.texts().ungrouped);
            let indent = "  ".repeat(*depth);
            let mut spans = vec![
                Span::styled(format!(" {indent}{marker} "), Style::new().fg(theme.muted)),
                Span::styled(name.to_string(), Style::new().fg(theme.label)),
            ];
            if *collapsed {
                spans.push(Span::styled(
                    format!("  {}", app.calendar_item_count(path)),
                    Style::new().fg(theme.muted),
                ));
            }
            frame.render_widget(Line::from(spans), area);
        }
        (TaskRow::Item { .. }, RowLines::Item { left, right }) => {
            // The left part stops one blank cell before the right part (on every line, so the wrapped lines
            // line up)
            let right_width = u16::try_from(right.width()).unwrap_or(u16::MAX);
            let left_area = Rect {
                width: area.width.saturating_sub(right_width + 1),
                ..area
            };
            if let Some(line) = left.get(part) {
                frame.render_widget(line, left_area);
            }
            if part == 0 {
                frame.render_widget(right, area);
            }
        }
        (TaskRow::Item { .. }, RowLines::Heading) => {}
    }
}

/// Splits styled text into lines at most `width` cells wide (at least one character each). A line breaks at a
/// space (which is dropped), after a wide character (so Chinese breaks between any two characters; a wide
/// character that does not fit simply starts the next line), and inside a word only when the word alone is
/// wider than a line; continuation lines start without spaces.
/// Each character keeps its span's style
///
/// E.g. width 10: `Send the weekly update` → `Send the`, `weekly`, `update`
pub(super) fn wrap(spans: &[Span<'static>], width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let wide = |c: char| Span::raw(c.to_string()).width() > 1;
    let mut lines: Vec<Vec<(char, Style)>> = vec![Vec::new()];
    let mut used = 0;
    // Where the current line may break: the index in it where the next line would start
    let mut breakable: Option<usize> = None;
    let chars = spans
        .iter()
        .flat_map(|span| span.content.chars().map(move |c| (c, span.style)));
    for (c, style) in chars {
        let cells = if wide(c) { 2 } else { 1 };
        let continuing = lines.len() > 1;
        let Some(line) = lines.last_mut() else {
            break;
        };
        if c == ' ' {
            if line.is_empty() && continuing {
                continue;
            }
            if used + 1 > width {
                // Break at this space; it is not drawn
                lines.push(Vec::new());
                used = 0;
                breakable = None;
            } else {
                line.push((c, style));
                used += 1;
                breakable = Some(line.len());
            }
            continue;
        }
        if used + cells > width && !line.is_empty() {
            // Move what comes after the last place to break onto a new line (nothing to move: break here)
            let rest = breakable.map_or_else(Vec::new, |at| line.split_off(at));
            while line.last().is_some_and(|&(c, _)| c == ' ') {
                line.pop();
            }
            used = rest.iter().map(|&(c, _)| if wide(c) { 2 } else { 1 }).sum();
            lines.push(rest);
            breakable = None;
        }
        let Some(line) = lines.last_mut() else {
            break;
        };
        line.push((c, style));
        used += cells;
        if cells > 1 {
            breakable = Some(line.len());
        }
    }
    lines
        .into_iter()
        .map(|mut line| {
            while line.last().is_some_and(|&(c, _)| c == ' ') {
                line.pop();
            }
            // Back to spans: consecutive characters with the same style make one span
            let mut spans: Vec<Span<'static>> = Vec::new();
            for (c, style) in line {
                match spans.last_mut() {
                    Some(span) if span.style == style => span.content.to_mut().push(c),
                    _ => spans.push(Span::styled(c.to_string(), style)),
                }
            }
            spans
        })
        .collect()
}

/// How many levels an item's line is indented (2 cells each): one under its calendar's heading (so its `▸`
/// lines up with the heading's name), the calendars its calendar is inside (an item in `Work > SFAP` 1 more),
/// plus its own subtask depth
fn levels(item: &Item) -> usize {
    1 + item
        .group
        .as_ref()
        .map_or(0, |heading| heading.parents.len())
        + item.depth
}

/// One item laid out at `width` cells: 「 ▸ [ ] 實作退款 API  1/4            10/14       」. On the left: the indent
/// (2 cells per level, see `levels`), `▸` / `▾` (has subtasks: collapsed / expanded), `[ ]` / `[x]` (task) or ` ∙ ` (event),
/// the title, and how many subtasks are done; when that does not fit, the title (with the count) wraps onto
/// the next lines, which start under the title. On the right of the first line: a date column
/// (right-aligned) and a time column, sized by `date_columns`; an item without a time leaves that cell blank
/// (without a date, both are empty)
///
/// Colors: items that are over (done tasks, past events) are gray, done titles struck through; a line
/// with a mistake has a red, underlined title (a `!!!` title is red too, but not underlined); an item with a
/// priority not over yet has its title in that priority's color; the date uses that day's calendar color;
/// `▸`, the subtask count and the time are muted
fn item_lines(
    app: &App,
    item: &Item,
    index: usize,
    (children, expanded): (bool, bool),
    (date_width, time_width): (usize, usize),
    width: u16,
) -> RowLines {
    let theme = &app.theme;
    let open = item.is_open(app.today);
    let color = if open { theme.text } else { theme.muted };
    let marker = match (children, expanded) {
        (false, _) => " ",
        (true, false) => "▸",
        (true, true) => "▾",
    };
    let check = match (item.kind, item.done, item.in_progress) {
        (Kind::Task, true, _) => "[x]",
        (Kind::Task, false, true) => "[/]",
        (Kind::Task, false, false) => "[ ]",
        (Kind::Event, ..) => " ∙ ",
    };
    // That line has a mistake: red, underlined title
    let mistake = app.warnings.iter().any(|warning| warning.line == item.line);
    let title_color = match item.priority {
        _ if mistake => theme.error,
        Some(priority) if open => theme.priority(priority),
        _ => color,
    };
    let mut title = title_span(item, title_color);
    if mistake {
        title = title.patch_style(Style::new().add_modifier(Modifier::UNDERLINED));
    }
    // Subtasks (only tasks count): (how many done, how many in total)
    let (done, total) = app
        .items
        .iter()
        .filter(|other| other.parent == Some(index) && other.kind == Kind::Task)
        .fold((0, 0), |(done, total), subtask| {
            (done + usize::from(subtask.done), total + 1)
        });
    let prefix = vec![
        Span::styled(
            format!(" {}{marker} ", "  ".repeat(levels(item))),
            Style::new().fg(theme.muted),
        ),
        Span::styled(format!("{check} "), Style::new().fg(color)),
    ];
    let mut text = vec![title];
    if total > 0 {
        text.push(Span::styled(
            format!("  {done}/{total}"),
            Style::new().fg(theme.muted),
        ));
    }
    // Right side: a date column (right-aligned), a time column (blank without a time), then one blank cell
    let mut right = Vec::new();
    if let (Some(first), Some(label)) = (item.date, date_label(item, app)) {
        let date_color = if open {
            day_color(first, app)
        } else {
            theme.muted
        };
        right.push(Span::styled(
            format!("{label:>date_width$}"),
            Style::new().fg(date_color),
        ));
        let time = item
            .time
            .map_or(String::new(), |time| format!(" {}", time.strftime("%H:%M")));
        right.push(Span::styled(
            format!("{time:<time_width$}"),
            Style::new().fg(theme.muted),
        ));
        right.push(Span::raw(" "));
    }
    let right = Line::from(right).right_aligned();
    // The title wraps in what is left between the prefix and the right part (one blank cell before it)
    let prefix_width = Line::from(prefix.clone()).width();
    let room = usize::from(width).saturating_sub(right.width() + 1 + prefix_width);
    let mut wrapped = wrap(&text, room).into_iter();
    let mut left = vec![Line::from(
        prefix
            .into_iter()
            .chain(wrapped.next().unwrap_or_default())
            .collect::<Vec<_>>(),
    )];
    left.extend(wrapped.map(|line| {
        Line::from(
            std::iter::once(Span::raw(" ".repeat(prefix_width)))
                .chain(line)
                .collect::<Vec<_>>(),
        )
    }));
    RowLines::Item { left, right }
}

/// Which row a mouse click at (`column`, `row`) on the tasks page (`area`) hits (any line of a wrapped
/// item); `on_marker` = the click is on the `▸` / `▾` cell (an item's, or a calendar heading's).
/// `None` on the header, the rules, the bottom row, the outer border, and on a heading row that cannot be
/// chosen anywhere but its `▸` / `▾`
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let [_, _, body, _, _] = inner_areas(area);
    if !body.contains(Position::new(column, row)) {
        return None;
    }
    // Which line of the list, then which row has that line (a wrapped item has several)
    let line = app.tasks_top + usize::from(row - body.y);
    let rows = app.tasks_rows();
    let (layouts, starts) = layout_rows(app, &rows, body.width);
    let index = starts
        .iter()
        .zip(&layouts)
        .position(|(&start, layout)| (start..start + layout.height()).contains(&line))?;
    let levels = match rows.get(index)? {
        TaskRow::Item { index, .. } => levels(app.items.get(*index)?),
        // A calendar's heading row: its `▸` / `▾` sits at cell 1 + 2 × depth inside the box
        TaskRow::Heading {
            depth,
            line,
            children,
            ..
        } => {
            let on_marker = *children && usize::from(column) == usize::from(body.x) + 1 + 2 * depth;
            return (line.is_some() || on_marker).then_some(Click::TaskRow {
                row: index,
                on_marker,
            });
        }
    };
    // `▸` sits at cell 1 + 2 × levels inside the box, on the item's first line
    let marker = usize::from(body.x) + 1 + 2 * levels;
    Some(Click::TaskRow {
        row: index,
        on_marker: line == starts[index] && usize::from(column) == marker,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    /// The text of each wrapped line
    fn texts(lines: &[Vec<Span<'static>>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| line.iter().map(|span| span.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn wrap_breaks_at_spaces_and_between_chinese_characters() {
        // Expected lines from the separate Python version in tools/expected_screen.py
        let wrapped = |text: &str, width| texts(&wrap(&[Span::raw(text.to_string())], width));
        assert_eq!(
            wrapped("Send the weekly update", 10),
            ["Send the", "weekly", "update"]
        );
        assert_eq!(
            wrapped("批次 audit log 調整（10/21 下班前交）", 12),
            ["批次 audit", "log 調整（", "10/21 下班前", "交）"]
        );
        // A word wider than a line: it starts on a new line, then breaks where the line is full
        assert_eq!(
            wrapped("see https://wiki.example.com/payments/runbook", 20),
            ["see", "https://wiki.example", ".com/payments/runboo", "k"]
        );
        // The spaces before the subtask count do not start the next line; width 1 still makes progress
        assert_eq!(wrapped("Pay  1/4", 6), ["Pay", "1/4"]);
        assert_eq!(wrapped("ab", 1), ["a", "b"]);
    }

    #[test]
    fn wrap_keeps_each_characters_style() {
        // The title in red, the count muted: a line that holds both has two spans
        let red = Style::new().fg(Color::Red);
        let muted = Style::new().fg(Color::DarkGray);
        let lines = wrap(
            &[
                Span::styled("Pay the fine", red),
                Span::styled("  1/4", muted),
            ],
            9,
        );
        assert_eq!(texts(&lines), ["Pay the", "fine  1/4"]);
        assert_eq!(lines[1][0], Span::styled("fine", red));
        assert_eq!(lines[1][1], Span::styled("  1/4", muted));
    }
}
