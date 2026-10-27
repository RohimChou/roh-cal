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

/// Draws the tab bar on `area` (the top row): the shown view's tab is one block on a background a little
/// fainter than `selection` (`Theme::tab_background`), with a faint `focus` bar at its left
/// (`Theme::tab_bar`) and its text in bold `focus`; the other tabs are `muted`
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let mut spans = vec![Span::raw(" ")];
    for (view, text, _) in tabs(app) {
        if view == app.view {
            let background = theme.tab_background();
            spans.push(Span::styled(
                BAR,
                Style::new().fg(theme.tab_bar()).bg(background),
            ));
            spans.push(Span::styled(
                text,
                Style::new()
                    .fg(theme.focus)
                    .bg(background)
                    .add_modifier(Modifier::BOLD),
            ));
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
/// `None` = not on a tab (or no tab bar: `area` is 0 rows tall)
pub fn view_at(area: Rect, app: &App, column: u16, row: u16) -> Option<View> {
    if !(area.top()..area.bottom()).contains(&row) {
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
