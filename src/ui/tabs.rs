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

/// Draws the tab bar on `area` (the top row): the shown view's tab has a faint `focus` bar at its left
/// (`Theme::tab_bar`) on the `selection` background, and its text in bold `focus` on a background that
/// fades from `selection` into the theme's background toward its right end, like a glow from the bar
/// (`Theme::tab_glow`); the other tabs are `muted`
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let mut spans = vec![Span::raw(" ")];
    for (view, text, _) in tabs(app) {
        if view == app.view {
            spans.push(Span::styled(
                BAR,
                Style::new().fg(theme.tab_bar()).bg(theme.selection),
            ));
            // One character at a time, each a step further along the fade (the first on `selection`, the
            // last on the background)
            let characters: Vec<char> = text.chars().collect();
            let steps = characters.len().saturating_sub(1).max(1) as f32;
            for (step, character) in characters.into_iter().enumerate() {
                let style = Style::new()
                    .fg(theme.focus)
                    .bg(theme.tab_glow(step as f32 / steps))
                    .add_modifier(Modifier::BOLD);
                spans.push(Span::styled(character.to_string(), style));
            }
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
