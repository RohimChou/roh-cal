//! Tab bar: the top row lists the views the keys `1` `2` `3` switch to, the shown one lit, so a first look
//! tells there are other views and which one this is. Clicking a tab = pressing its number key

use super::VIEW_KEYS;
use crate::app::{App, View};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

/// Drawn at the left of the shown view's tab
const BAR: &str = "┃";

/// Each tab: its view, its text (` 1 calendar `) and its first column from the left of the bar (the bar's
/// cell). One space before the first tab; every tab takes the same room shown or not (the bar's cell, then
/// the text), so switching never moves the names
fn tabs(app: &App) -> Vec<(View, String, u16)> {
    let mut x = 1;
    VIEW_KEYS
        .iter()
        .zip(app.lang.texts().tabs)
        .enumerate()
        .map(|(index, (&view, name))| {
            let text = format!(" {} {name} ", index + 1);
            let tab = (view, text, x);
            x += 1 + width(&tab.1);
            tab
        })
        .collect()
}

/// How many cells `text` takes (a CJK character takes 2)
fn width(text: &str) -> u16 {
    u16::try_from(Span::raw(text).width()).unwrap_or(u16::MAX)
}

/// Draws the tab bar on `area` (the top row): the shown view's tab has a `focus` bar at its left, its text
/// in bold `focus`, both on the `selection` background; the other tabs are `muted`
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let shown = Style::new().fg(theme.focus).bg(theme.selection);
    let mut spans = vec![Span::raw(" ")];
    for (view, text, _) in tabs(app) {
        if view == app.view {
            spans.push(Span::styled(BAR, shown));
            spans.push(Span::styled(text, shown.add_modifier(Modifier::BOLD)));
        } else {
            spans.push(Span::styled(
                format!(" {text}"),
                Style::new().fg(theme.muted),
            ));
        }
    }
    frame.render_widget(Line::from(spans), area);
}

/// Which view's tab terminal cell (`column`, `row`) is on, in the tab bar on `area` (the bar's cell counts);
/// `None` = not on a tab
pub fn view_at(area: Rect, app: &App, column: u16, row: u16) -> Option<View> {
    if row != area.y {
        return None;
    }
    tabs(app)
        .into_iter()
        .find(|(_, text, x)| {
            let left = area.x + x;
            (left..left + 1 + width(text)).contains(&column)
        })
        .map(|(view, ..)| view)
}
