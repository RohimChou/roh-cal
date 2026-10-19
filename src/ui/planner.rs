//! Layout 1: the calendar box on the left and the agenda on the right, with their rows lined up. The header
//! stays at the top; below it, one big block per month runs downward, and it scrolls up and down
//!
//! How many rows each month takes, the row numbers and the scroll position: see `months.rs`

use super::{
    Click,
    months::{Label, Months, Row},
    title_span,
};
use crate::{
    app::{App, Focus},
    calendar::{self, WeekRow},
    tasks::Item,
    texts::Texts,
    theme::Theme,
};
use jiff::civil::Date;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};
use std::rc::Rc;

/// Cells per day: a blank, the date (right-aligned in 2 cells), and one cell after it
const DAY_WIDTH: u16 = 4;
/// The 7 days, plus 1 extra cell at the end for Sunday's `]` when the cell after its date is taken
const DAYS_WIDTH: u16 = 7 * DAY_WIDTH + 1;
/// The four columns in the calendar box: year/month, week number, │, 7 days
const COLUMNS: [Constraint; 4] = [
    Constraint::Length(6),
    Constraint::Length(4),
    Constraint::Length(1),
    Constraint::Length(DAYS_WIDTH),
];
/// Width of the calendar box: the four columns + left and right border
pub const CALENDAR_WIDTH: u16 = 6 + 4 + 1 + DAYS_WIDTH + 2;
/// How many rows to keep between the selected row and the screen's top and bottom edges; scroll only when
/// there are fewer (like vim's scrolloff)
const SCROLLOFF: i64 = 2;
/// When the agenda is split into columns, the minimum width of each column
const AGENDA_COLUMN_WIDTH: u16 = 28;
/// The most columns the agenda can split into
const MAX_AGENDA_COLUMNS: u16 = 4;
/// Gap between agenda columns
const AGENDA_GAP: u16 = 2;

/// How many columns an agenda `width` cells wide gets: each at least `AGENDA_COLUMN_WIDTH` cells (with
/// `AGENDA_GAP` cells between columns), at most `MAX_AGENDA_COLUMNS` columns
fn agenda_columns(width: u16) -> usize {
    usize::from(
        ((width + AGENDA_GAP) / (AGENDA_COLUMN_WIDTH + AGENDA_GAP)).clamp(1, MAX_AGENDA_COLUMNS),
    )
}

/// How many columns the agenda box (`agenda`, with its border) is split into
pub(super) fn columns(agenda: Rect) -> usize {
    let [_, _, body] = inner_areas(agenda);
    agenda_columns(body.width)
}

/// Splits one agenda row into `count` columns, like a newspaper
fn split_columns(area: Rect, count: usize) -> Rc<[Rect]> {
    Layout::horizontal(vec![Constraint::Fill(1); count])
        .spacing(AGENDA_GAP)
        .split(area)
}

/// This week's first day that belongs to this month; the week number and year / month come from it
fn first_day(days: &WeekRow) -> Option<Date> {
    days.iter().flatten().next().copied()
}

/// The largest share of the calendar height one month may take (at least its week count), so the months
/// below stay visible: (numerator, denominator) = 2/3
const MAX_MONTH_SHARE: (u16, u16) = (2, 3);

/// The most rows one month takes: `MAX_MONTH_SHARE` of the calendar height
fn max_month_height(body: Rect) -> usize {
    let (numerator, denominator) = MAX_MONTH_SHARE;
    usize::from(body.height) * usize::from(numerator) / usize::from(denominator)
}

/// Draws the calendar box on the left (`calendar`) and the agenda on the right (`agenda`)
///
/// First scroll the agenda within the selected month (stored in `app.agenda_window`), then scroll the
/// whole screen (stored in `app.calendar_top`), so the selected week and the items to show are both visible
pub fn render(frame: &mut Frame, calendar: Rect, agenda: Rect, app: &mut App) {
    let [calendar_header, calendar_rule, calendar_body] = inner_areas(calendar);
    let [agenda_header, agenda_rule, agenda_body] = inner_areas(agenda);
    let columns = agenda_columns(agenda_body.width);
    let max_height = max_month_height(calendar_body);
    // `app` gets changed, so compute these first (`Months` borrows `app.items`, and Rust does not allow
    // changing something while it is borrowed for reading)
    let (top, window) = {
        let mut months = Months::new(app, columns, max_height);
        let month = app.selected.first_of_month();
        // Agenda items that must be visible: keyboard on the agenda = the chosen item; on the calendar =
        // the selected day's items
        let targets = match app.chosen() {
            Some(item) => vec![item],
            None => months.day_items(app.selected),
        };
        // The agenda scrolls within that month (a new month starts from its first row)
        let previous = app
            .agenda_window
            .filter(|(window_month, _)| *window_month == month)
            .map_or(0, |(_, offset)| offset);
        let window = (month, months.window_for(month, previous, &targets));
        months.window = Some(window);
        // Then scroll the whole screen: if not scrolled yet (just opened, or after `t`), put the selected
        // day's month at the top first
        let top = app
            .calendar_top
            .unwrap_or_else(|| months.block_start(app.selected));
        let top = months.scrolled_to(
            top,
            app.selected,
            &targets,
            i64::from(calendar_body.height),
            SCROLLOFF,
        );
        (top, window)
    };
    app.calendar_top = Some(top);
    app.agenda_window = Some(window);
    // Read-only from here on
    let app = &*app;
    let mut months = Months::new(app, columns, max_height);
    months.window = app.agenda_window;
    let texts = app.lang.texts();
    let theme = &app.theme;
    // The box that has the keyboard gets its border in the `focus` color
    let border_color = |focus: Focus| {
        if app.focus == focus {
            theme.focus
        } else {
            theme.line
        }
    };
    for (area, focus) in [(calendar, Focus::Calendar), (agenda, Focus::Agenda)] {
        let color = border_color(focus);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(color));
        frame.render_widget(block, area);
    }
    let chosen = app.chosen();

    // Rows visible on screen
    let visible = months.visible_rows(top, usize::from(calendar_body.height));

    // The header's top-left always shows the year of the top week on screen (the selected day's year
    // if no week is on screen)
    let year = visible
        .iter()
        .find_map(|row| match row {
            Row::Week {
                days: Some(days), ..
            } => first_day(days),
            _ => None,
        })
        .unwrap_or(app.selected);
    render_header(frame, calendar_header, texts, theme, year);
    // Agenda header: 「重要事項」 on the left, the selected day on the right (one blank cell after it).
    // Each is drawn in its own area: the date is always complete,
    // 「重要事項」 is cut off if it does not fit (never half a Chinese character), with at least one blank
    // cell between. Drawn in one area, a date starting on the second half of a Chinese character would
    // get eaten
    let date = format!("{} ", texts.date_title(app.selected));
    let date_width = u16::try_from(Span::raw(&date).width())
        .unwrap_or(u16::MAX)
        .min(agenda_header.width);
    let label_area = Rect {
        width: agenda_header.width.saturating_sub(date_width + 1),
        ..agenda_header
    };
    let date_area = Rect {
        x: agenda_header.right() - date_width,
        width: date_width,
        ..agenda_header
    };
    frame.render_widget(
        Line::styled(texts.header_items, Style::new().fg(theme.muted)),
        label_area,
    );
    frame.render_widget(Line::styled(date, Style::new().fg(theme.text)), date_area);
    render_rule(frame, calendar_rule, agenda_rule, false, None, theme);

    // Draw row by row; the same row of the left and right box sits at the same height
    for (y, row) in (calendar_body.y..calendar_body.bottom()).zip(&visible) {
        let calendar_row = Rect::new(calendar_body.x, y, calendar_body.width, 1);
        let agenda_row = Rect::new(agenda_body.x, y, agenda_body.width, 1);
        match row {
            Row::Separator { year, hidden } => {
                // The month above has items that do not fit: the right end of the line says how many more
                let hint = (*hidden > 0).then(|| texts.more_items(*hidden));
                render_rule(frame, calendar_row, agenda_row, *year, hint, theme);
            }
            Row::Week {
                label,
                days,
                items,
                thumb,
            } => {
                render_week(frame, calendar_row, *label, days.as_ref(), app);
                for (entry, area) in items
                    .iter()
                    .zip(split_columns(agenda_row, items.len()).iter())
                {
                    if let Some(entry) = entry {
                        // Chosen line (keyboard on the agenda): date in brackets, background color
                        let is_chosen = chosen == Some(*entry);
                        // Keyboard on the calendar: the selected day's items get a background color (same as
                        // that week on the calendar; a multi-day item counts if it covers that day)
                        let on_selected_day =
                            app.focus == Focus::Calendar && entry.is_on(app.selected);
                        frame.render_widget(item_line(entry.item, app, is_chosen), *area);
                        if is_chosen || on_selected_day {
                            frame
                                .buffer_mut()
                                .set_style(*area, Style::new().bg(theme.selection));
                        }
                    }
                }
                // This month's items do not fit: draw the scrollbar thumb on the right border (that
                // border cell becomes a thick line)
                if *thumb == Some(true) && agenda.width > 0 {
                    frame.render_widget(
                        Line::styled(
                            line::THICK_VERTICAL,
                            Style::new().fg(border_color(Focus::Agenda)),
                        ),
                        Rect::new(agenda.right() - 1, y, 1, 1),
                    );
                }
            }
        }
    }
}

/// How the inside of a box is split: the header (a line of text + a rule) stays at the top, months fill
/// the rows below; used both for drawing and for mapping mouse positions
///
/// The side menu is split the same way, so the headers of all three boxes line up
pub(super) fn inner_areas(area: Rect) -> [Rect; 3] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(Block::bordered().inner(area))
}

/// What a mouse click at terminal (`column`, `row`) hits: a day in the calendar box on the left
/// (`calendar`) or an item in the agenda on the right (`agenda`);
/// `None` when it is not on a date or an item
///
/// The scroll position is `app.calendar_top`, stored at the last draw; each calendar day takes `DAY_WIDTH`
/// cells (the blanks before and after the date count as that day)
pub fn click_at(calendar: Rect, agenda: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let [_, _, calendar_body] = inner_areas(calendar);
    let [_, _, agenda_body] = inner_areas(agenda);
    let mut months = Months::new(
        app,
        agenda_columns(agenda_body.width),
        max_month_height(calendar_body),
    );
    months.window = app.agenda_window;
    let position = Position::new(column, row);
    // Every row on screen, same as when drawing (including the agenda's scroll); both boxes' rows line up
    let rows = months.visible_rows(app.calendar_top?, usize::from(calendar_body.height));
    if calendar_body.contains(position) {
        // Which row was clicked (lines between months and item-only rows have no dates)
        let Row::Week {
            days: Some(days), ..
        } = rows.get(usize::from(row - calendar_body.y))?
        else {
            return None;
        };
        // Which day: which group of `DAY_WIDTH` cells in the 7-day column; the extra cell at the end
        // is the 8th and has no date
        let [_, _, _, days_area] = Layout::horizontal(COLUMNS).areas(calendar_body);
        let weekday = usize::from(column.checked_sub(days_area.x)? / DAY_WIDTH);
        days.get(weekday).copied().flatten().map(Click::Day)
    } else if agenda_body.contains(position) {
        let Row::Week { items, .. } = rows.get(usize::from(row - agenda_body.y))? else {
            return None;
        };
        // Which column was clicked (the gap between columns does not count)
        let agenda_row = Rect::new(agenda_body.x, row, agenda_body.width, 1);
        let index = split_columns(agenda_row, items.len())
            .iter()
            .position(|area| area.contains(position))?;
        let entry = items.get(index).copied().flatten()?;
        // That line's index in `App::agenda()`
        app.agenda()
            .iter()
            .position(|other| *other == entry)
            .map(Click::Item)
    } else {
        None
    }
}

/// Header: 「2026年  週 │ 一 二 … 日」; Saturday and Sunday use the weekend color
///
/// The year/month column shows the year of `year`, e.g. 「2026年  週」
fn render_header(frame: &mut Frame, area: Rect, texts: &Texts, theme: &Theme, year: Date) {
    let muted = Style::new().fg(theme.muted);
    let month_header = texts.year_label(year);
    let weekdays: Vec<Span> = texts
        .weekdays
        .iter()
        .enumerate()
        .map(|(i, name)| {
            // The 6th and 7th are Saturday and Sunday
            let style = if i >= 5 {
                Style::new().fg(theme.weekend)
            } else {
                muted
            };
            // Over its date: a blank before, the cell after the date after
            Span::styled(format!(" {name} "), style)
        })
        .collect();
    render_cells(
        frame,
        area,
        [
            Line::styled(month_header, muted).right_aligned(),
            Line::styled(texts.header_week, muted).right_aligned(),
            Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
            Line::from(weekdays),
        ],
    );
}

/// One row of the calendar box: year / month on the left, week number, │, 7 days (`DAY_WIDTH` cells per day,
/// dates right-aligned); item-only rows draw only │
///
/// If the selected day is in this row: the week number is bright and the whole row gets the
/// `selection` background; other rows have a dimmer week number
fn render_week(
    frame: &mut Frame,
    area: Rect,
    label: Option<Label>,
    days: Option<&WeekRow>,
    app: &App,
) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let day = days.and_then(first_day);
    let label = match (label, day) {
        (Some(Label::Year), Some(day)) => texts.year_label(day),
        (Some(Label::Month), Some(day)) => texts.month_label(day),
        _ => String::new(),
    };
    // The selected day is in this row: brighter week number, background on the whole row
    let selected_week = days.is_some_and(|days| days.contains(&Some(app.selected)));
    // Week number (or weeks away from the selected week): any day of this row works
    // (a row is always one week)
    let relative = app.settings.calendar.relative_weeks;
    let week = day
        .map(|day| format!("{:>3} ", calendar::week_label(day, app.selected, relative)))
        .unwrap_or_default();
    let week_color = if selected_week {
        theme.week_number
    } else {
        theme.week_number_dim
    };
    render_cells(
        frame,
        area,
        [
            Line::styled(label, Style::new().fg(theme.label)).right_aligned(),
            Line::styled(week, Style::new().fg(week_color)),
            Line::styled(line::VERTICAL, Style::new().fg(theme.line)),
            days.map(|days| day_cells(days, app)).unwrap_or_default(),
        ],
    );
    if selected_week {
        // Only change the background; leave the text color alone
        frame
            .buffer_mut()
            .set_style(area, Style::new().bg(theme.selection));
    }
}

/// The dates of the 7 days, `DAY_WIDTH` cells per day: a blank, the date right-aligned in 2 cells, and its
/// dots after it (one per item that day, `dots`); the selected day gets brackets: `[17]`, or `[17⠒]` around
/// its dots
///
/// A date has at most 2 digits, so there is always a blank before it: `[` takes the last blank before the
/// date; `]` takes the cell after the date when it has no dots, else the next day's first blank (for
/// Sunday, the extra cell at the end)
///
/// A multi-day item underlines its dates, and the cells between two of its days too, so its days read as
/// one line (`ranges_on`); two items back to back stay apart; where two or more overlap, the underline is
/// in `range_overlap`
fn day_cells(days: &WeekRow, app: &App) -> Line<'static> {
    let bracket = Style::new()
        .fg(app.theme.selected)
        .add_modifier(Modifier::BOLD);
    // The cells between `day` and the next day: underlined when an item runs on, like `day`'s date
    let between = |day: Option<Date>, next: Option<Date>| match (day, next) {
        (Some(day), Some(next)) => {
            range_underline(ranges_on(day, next, app), day_color(day, app), app)
        }
        _ => Style::new(),
    };
    let mut spans = Vec::new();
    // The previous day is the selected one and has dots: its `]` goes in this day's first blank
    let mut close = false;
    for (i, day) in days.iter().enumerate() {
        let number = day.map(|day| day.day().to_string()).unwrap_or_default();
        let selected = *day == Some(app.selected);
        // The blanks and brackets before the date sit between it and the previous day's
        let before = between(i.checked_sub(1).and_then(|previous| days[previous]), *day);
        // How many blanks to pad before the date, e.g. `17` gets 1, `5` gets 2; minus the cells brackets take
        let mut gap = 3 - number.len() - usize::from(selected);
        if close {
            spans.push(Span::styled("]", bracket.patch(before)));
            gap -= 1;
        }
        spans.push(Span::styled(" ".repeat(gap), before));
        if selected {
            spans.push(Span::styled("[", bracket.patch(before)));
        }
        if let Some(day) = day {
            // The selected day and today are bold; days covered by a multi-day item are underlined
            let mut style = Style::new().fg(day_color(*day, app));
            if selected || *day == app.today {
                style = style.add_modifier(Modifier::BOLD);
            }
            let ranges = ranges_on(*day, *day, app);
            style = style.patch(range_underline(ranges, day_color(*day, app), app));
            spans.push(Span::styled(number, style));
        }
        // The cell after the date (between it and the next day's): its dots; when it has none, the
        // selected day's `]`
        let after = between(*day, days.get(i + 1).copied().flatten());
        let count = day.map_or(0, |day| app.items_on(day).len());
        close = selected && count > 0;
        if selected && count == 0 {
            spans.push(Span::styled("]", bracket.patch(after)));
        } else {
            let style = Style::new().fg(app.theme.muted).patch(after);
            spans.push(Span::styled(dots(count), style));
        }
    }
    if close {
        spans.push(Span::styled("]", bracket));
    }
    Line::from(spans)
}

/// The dots right of a date: one per item that day (`count`), at most 4, stacked in one cell; a blank for
/// none. Braille dots are always 1 cell wide (`•` is 2 cells in terminals with wide CJK characters on)
fn dots(count: usize) -> &'static str {
    match count {
        0 => " ",
        1 => "⠂",
        2 => "⠒",
        3 => "⠖",
        _ => "⠶",
    }
}

/// Whether the item is in a holiday calendar (its heading has `type:holiday`)
pub(super) fn is_holiday_item(item: &Item) -> bool {
    item.group.as_ref().is_some_and(|group| group.holiday)
}

/// The color of an item's line while it is not over yet: its priority's (`!!!` red, `!!` orange, `!`
/// yellow), else a holiday's day-off blue, else the normal text color; `true` with it = the whole line
/// (the time too) has this color, `false` = the time stays muted
pub(super) fn open_item_color(item: &Item, theme: &Theme) -> (Color, bool) {
    match item.priority {
        Some(priority) => (theme.priority(priority), true),
        None if is_holiday_item(item) => (theme.weekend, true),
        None => (theme.text, false),
    }
}

/// A day's date color: today > a day off (a weekend or a holiday) > normal; items do not change it
/// (items that are all over, or in unchecked calendars, count as no items)
pub(super) fn day_color(day: Date, app: &App) -> Color {
    let theme = &app.theme;
    if day == app.today {
        theme.today
    } else if app.is_holiday(day) || day.weekday().to_monday_zero_offset() >= 5 {
        theme.weekend
    } else {
        theme.text
    }
}

/// Whether `day` is inside a multi-day item (in a checked calendar, not a done task): if so, that date
/// is underlined on the calendar
pub(super) fn in_multi_day(day: Date, app: &App) -> bool {
    ranges_on(day, day, app) > 0
}

/// How many multi-day items (in a checked calendar, not a done task) cover both `day` and `next`; the
/// same day twice = how many `day` is in
fn ranges_on(day: Date, next: Date, app: &App) -> usize {
    app.items
        .iter()
        .filter(|item| {
            item.end.is_some()
                && !item.done
                && app.is_shown(item)
                && item.covers(day)
                && item.covers(next)
        })
        .count()
}

/// The underline for `count` multi-day items on a date or between two dates: none for 0, in `color` for 1,
/// in `range_overlap` for two or more
fn range_underline(count: usize, color: Color, app: &App) -> Style {
    let underline = Style::new().add_modifier(Modifier::UNDERLINED);
    match count {
        0 => Style::new(),
        1 => underline.underline_color(color),
        _ => underline.underline_color(app.theme.range_overlap),
    }
}

/// The agenda date: `10/5`; multi-day items show a range: `10/20~25`, across months `10/28~11/3`,
/// across years `12/30~1/2`
pub(super) fn date_text(item: &Item) -> String {
    let Some(first) = item.date else {
        return String::new();
    };
    let start = format!("{}/{}", first.month(), first.day());
    match item.end {
        Some(last) if last.first_of_month() == first.first_of_month() => {
            format!("{start}~{}", last.day())
        }
        Some(last) => format!("{start}~{}/{}", last.month(), last.day()),
        None => start,
    }
}

/// The color of an item's time (and of its calendar in the day popup): the line's own color when a priority
/// or a holiday colors the whole line, otherwise muted (gray once over too)
pub(super) fn secondary_color(item: &Item, app: &App) -> Color {
    let theme = &app.theme;
    match item
        .is_open(app.today)
        .then(|| open_item_color(item, theme))
    {
        Some((color, true)) => color,
        _ => theme.muted,
    }
}

/// One agenda item: 「 10/16 17:30 交週報」 (date at least 5 cells, right-aligned; a multi-day date is
/// longer and pushes the title right; time only if it has one)
///
/// Not over yet: see `open_item_color` (a priority or a holiday colors the whole line, otherwise the
/// normal color with the time muted). Over: the whole line turns gray; done tasks also get a
/// struck-through title, past events do not.
/// `chosen` = the chosen item while the keyboard is on the agenda: date in brackets `[10/17]`, bold
pub(super) fn item_line(item: &Item, app: &App, chosen: bool) -> Line<'static> {
    let theme = &app.theme;
    // The line follows the item itself, not its day; gray once over
    let color = if item.is_open(app.today) {
        open_item_color(item, theme).0
    } else {
        theme.muted
    };
    let date = date_text(item);
    // The date part is at least 7 cells wide: 「 10/17 」; the chosen one is 「[10/17]」 (a shorter date
    // gets blanks before the bracket; a multi-day range is longer)
    let mut spans = if chosen {
        let bracket = Style::new().fg(theme.selected).add_modifier(Modifier::BOLD);
        vec![
            Span::raw(" ".repeat(5_usize.saturating_sub(date.len()))),
            Span::styled("[", bracket),
            Span::styled(date, Style::new().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled("]", bracket),
        ]
    } else {
        vec![Span::styled(format!(" {date:>5} "), Style::new().fg(color))]
    };
    if let Some(time) = item.time {
        spans.push(Span::styled(
            format!("{} ", time.strftime("%H:%M")),
            Style::new().fg(secondary_color(item, app)),
        ));
    }
    spans.push(title_span(item, color));
    Line::from(spans)
}

/// The same horizontal line across both boxes (`thick` = the thick new-year line); in the calendar
/// box, a cross where it meets the vertical line. With a `hint`: written at the right end of the
/// agenda part of the line (muted, leaving one line cell at the end), e.g. `───── 還有 23 件 ─`
fn render_rule(
    frame: &mut Frame,
    calendar: Rect,
    agenda: Rect,
    thick: bool,
    hint: Option<String>,
    theme: &Theme,
) {
    let (horizontal, cross, color) = if thick {
        (line::THICK_HORIZONTAL, "┿", theme.year_line)
    } else {
        (line::HORIZONTAL, line::CROSS, theme.line)
    };
    let style = Style::new().fg(color);
    let columns = Layout::horizontal(COLUMNS).areas::<4>(calendar);
    for (i, column) in columns.into_iter().enumerate() {
        // The 3rd column is the vertical-line column
        let symbol = if i == 2 { cross } else { horizontal };
        let text = symbol.repeat(usize::from(column.width));
        frame.render_widget(Line::styled(text, style), column);
    }
    let text = horizontal.repeat(usize::from(agenda.width));
    frame.render_widget(Line::styled(text, style), agenda);
    if let Some(hint) = hint {
        // Color goes on the `Span`: `Line::styled` would make the whole row (including the line on
        // the left) muted
        let hint = Line::from(Span::styled(
            format!(" {hint} "),
            Style::new().fg(theme.muted),
        ));
        // Only write it if it fits (at least one line cell left on each side)
        if hint.width() + 2 <= usize::from(agenda.width) {
            let area = Rect {
                width: agenda.width - 1,
                ..agenda
            };
            frame.render_widget(hint.right_aligned(), area);
        }
    }
}

/// Draws one piece of text in each of the four columns of a calendar box row
fn render_cells(frame: &mut Frame, area: Rect, cells: [Line; 4]) {
    let columns = Layout::horizontal(COLUMNS).areas::<4>(area);
    for (cell, column) in cells.into_iter().zip(columns) {
        frame.render_widget(cell, column);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agenda_columns_are_at_least_28_wide() {
        // 2 cells between columns: 2 columns need 28 + 2 + 28 = 58 cells
        assert_eq!(agenda_columns(0), 1);
        assert_eq!(agenda_columns(57), 1);
        assert_eq!(agenda_columns(58), 2);
        assert_eq!(agenda_columns(88), 3);
        assert_eq!(agenda_columns(118), 4);
        assert_eq!(agenda_columns(500), 4);
    }

    #[test]
    fn dots_stack_up_to_four() {
        let shown: Vec<&str> = (0..=6).map(dots).collect();
        assert_eq!(shown, [" ", "⠂", "⠒", "⠖", "⠶", "⠶", "⠶"]);
    }
}
