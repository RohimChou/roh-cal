//! Tasks page (`3`): the whole area right of the side menu, listing the entire task file by calendar, with
//! subtasks as a tree; the bottom row shows the chosen line's original text

use super::{
    Click,
    months::scrolled_top,
    planner::{date_text, day_color},
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

/// Draws the tasks page: first scroll so the chosen item is visible (stored in `app.tasks_top`), then
/// draw row by row
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let rows = app.tasks_rows();
    let [header, rule, body, bottom_rule, detail] = inner_areas(area);
    let height = i64::from(body.height);
    let cursor = app.tasks_cursor(&rows);
    if let Some(cursor) = cursor {
        let top = scrolled_top(
            i64::try_from(app.tasks_top).unwrap_or(0),
            i64::try_from(cursor).unwrap_or(0),
            height,
            SCROLLOFF,
        );
        // Do not overscroll: the last row just touches the bottom
        let max = i64::try_from(rows.len()).unwrap_or(0) - height;
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
        Line::styled(texts.header_tasks, Style::new().fg(theme.muted)),
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
    let columns = date_columns(app, &rows);
    let visible = rows.iter().enumerate().skip(app.tasks_top);
    for ((index, row), y) in visible.zip(body.y..body.bottom()) {
        let area = Rect {
            y,
            height: 1,
            ..body
        };
        match row {
            TaskRow::Heading { name, level } => {
                let name = name.as_deref().unwrap_or(texts.ungrouped);
                let indent = "  ".repeat(level.saturating_sub(1));
                frame.render_widget(
                    Line::styled(
                        format!(" {indent}{name}"),
                        Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
                    ),
                    area,
                );
            }
            TaskRow::Item {
                index: item_index,
                children,
                expanded,
            } => {
                let Some(item) = app.items.get(*item_index) else {
                    continue;
                };
                render_item(
                    frame,
                    area,
                    app,
                    item,
                    *item_index,
                    (*children, *expanded),
                    columns,
                );
                // The chosen item's whole row gets a background (when the keyboard is on the tasks page)
                if focused && cursor == Some(index) {
                    frame
                        .buffer_mut()
                        .set_style(area, Style::new().bg(theme.cursor_line));
                }
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

/// One item: 「 ▸ [ ] 實作退款 API  1/4            10/14       」. On the left: the indent (2 cells
/// per level), `▸` / `▾` (has subtasks: collapsed / expanded),
/// `[ ]` / `[x]` (task) or ` ∙ ` (event), the title, and how many subtasks are done. On the right:
/// a date column (right-aligned) and a time column, sized by `date_columns`; an item without a time
/// leaves that cell blank (without a date, both are empty)
///
/// Colors: items that are over (done tasks, past events) are gray, done titles struck through; a line
/// with a mistake has a red title; the date uses that day's calendar color;
/// `▸`, the subtask count and the time are muted
fn render_item(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    item: &Item,
    index: usize,
    (children, expanded): (bool, bool),
    (date_width, time_width): (usize, usize),
) {
    let theme = &app.theme;
    let open = item.is_open(app.today);
    let color = if open { theme.text } else { theme.muted };
    let marker = match (children, expanded) {
        (false, _) => " ",
        (true, false) => "▸",
        (true, true) => "▾",
    };
    let check = match (item.kind, item.done) {
        (Kind::Task, false) => "[ ]",
        (Kind::Task, true) => "[x]",
        (Kind::Event, _) => " ∙ ",
    };
    // That line has a mistake: red title
    let mistake = app.warnings.iter().any(|warning| warning.line == item.line);
    let mut title = Style::new().fg(if mistake { theme.error } else { color });
    if item.done {
        title = title.add_modifier(Modifier::CROSSED_OUT);
    }
    // Subtasks (only tasks count): (how many done, how many in total)
    let (done, total) = app
        .items
        .iter()
        .filter(|other| other.parent == Some(index) && other.kind == Kind::Task)
        .fold((0, 0), |(done, total), subtask| {
            (done + usize::from(subtask.done), total + 1)
        });
    let mut left = vec![
        Span::styled(
            format!(" {}{marker} ", "  ".repeat(item.depth)),
            Style::new().fg(theme.muted),
        ),
        Span::styled(format!("{check} "), Style::new().fg(color)),
        Span::styled(item.title.as_str(), title),
    ];
    if total > 0 {
        left.push(Span::styled(
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
    let right = Line::from(right);
    // Title too long: cut it off so the date is always visible (at least one blank cell between)
    let right_width = u16::try_from(right.width()).unwrap_or(u16::MAX);
    let left_area = Rect {
        width: area.width.saturating_sub(right_width + 1),
        ..area
    };
    frame.render_widget(Line::from(left), left_area);
    frame.render_widget(right.right_aligned(), area);
}

/// Which row a mouse click at (`column`, `row`) on the tasks page (`area`) hits; `on_marker` = the
/// click is on the `▸` / `▾` cell.
/// `None` on the header, the rules, the bottom row or the outer border
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let [_, _, body, _, _] = inner_areas(area);
    if !body.contains(Position::new(column, row)) {
        return None;
    }
    let index = app.tasks_top + usize::from(row - body.y);
    let rows = app.tasks_rows();
    let depth = match rows.get(index)? {
        TaskRow::Item { index, .. } => app.items.get(*index)?.depth,
        TaskRow::Heading { .. } => return None,
    };
    // `▸` sits at cell 1 + 2 × depth inside the box
    let marker = usize::from(body.x) + 1 + 2 * depth;
    Some(Click::TaskRow {
        row: index,
        on_marker: usize::from(column) == marker,
    })
}
