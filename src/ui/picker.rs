//! Color picker (`e` on a color row of the settings page): a small box over the middle of the screen
//!
//! ```text
//! ╭ 自訂顏色 ∙ today ─────────────────────╮
//! │                                       │
//! │  ████████  #a6e3a1                    │  ← the color (blank cells with it as background), its code
//! │                                       │
//! │  > R  166  ░░░░░░░░░░░░|░░░░░░░░░░░   │  ← one bar per channel: each cell is the color with that
//! │    G  227  ░░░░░░░░░░░░░░░░░░░░|░░░   │     channel at that point (blank cells with a background),
//! │    B  161  ░░░░░░░░░░░░░░░░░░░░|░░░   │     `|` = where it is now; `>` = the chosen channel
//! │                                       │
//! │  也可以直接打色碼：a6e3a1              │
//! ╰ jk 選  hl HL 調  Enter 存  Esc 取消 ──╯
//! ```

use crate::app::{App, Picker};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// How many cells a channel's bar has
const BAR: u16 = 24;
/// How many cells wide the color itself is drawn
const SWATCH: usize = 8;
/// The box's height: border, blank, the color, blank, 3 channels, blank, the typing hint, border
const HEIGHT: u16 = 10;
/// Where the first channel is, counted from the box's top
const CHANNEL_Y: u16 = 4;
/// Where the bars start, counted from the box's left: border 1 + `  > R  166  ` 12
const BAR_X: u16 = 13;
/// The channels' names
const CHANNELS: [&str; 3] = ["R", "G", "B"];

/// The box's area: wide enough for the bars and the texts, in the middle of `screen`
pub fn area(app: &App, screen: Rect, picker: &Picker) -> Rect {
    let texts = app.lang.texts();
    // Inside: indent 2 + label 10 + bar + 2 blank cells; the texts get 2 cells before them (the hint on
    // the bottom border 1 on each side)
    let width = [
        usize::from(BAR_X - 1 + BAR) + 2,
        super::border_hints(app, texts.picker_hint).width(),
        Span::raw(texts.picker_typing).width() + 4,
        title(app, picker).width(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
        + 2;
    screen.centered(
        Constraint::Length(u16::try_from(width).unwrap_or(u16::MAX).min(screen.width)),
        Constraint::Length(HEIGHT.min(screen.height)),
    )
}

/// The top border: 「 自訂顏色 ∙ today 」
fn title(app: &App, picker: &Picker) -> Line<'static> {
    Line::styled(
        format!(" {} ∙ {} ", app.lang.texts().picker_title, picker.role),
        Style::new().fg(app.theme.label),
    )
}

/// The value at the `index`-th cell of a bar (0 at the left end, 255 at the right end)
fn bar_value(index: u16) -> u8 {
    let last = u32::from(BAR - 1);
    u8::try_from((u32::from(index) * 255 + last / 2) / last).unwrap_or(u8::MAX)
}

/// Which cell of a bar marks `value`
fn bar_index(value: u8) -> u16 {
    let last = u32::from(BAR - 1);
    u16::try_from((u32::from(value) * last + 127) / 255).unwrap_or(0)
}

/// Draws the picker over the settings page
pub fn render(frame: &mut Frame, app: &App, picker: &Picker) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let area = area(app, frame.area(), picker);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(title(app, picker))
        .title_bottom(super::border_hints(app, texts.picker_hint));
    super::clear_with_ring(frame, area, frame.area());
    frame.render_widget(block, area);
    let [red, green, blue] = picker.rgb;
    let row = |dy: u16| Rect::new(area.x + 1, area.y + dy, area.width.saturating_sub(2), 1);
    // The color and its code; while typing, the digits so far (the rest as `_`) in the notice color
    let code = if picker.typed.is_empty() {
        Span::styled(
            picker.hex(),
            Style::new().fg(theme.text).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            format!("#{:_<6}", picker.typed),
            Style::new().fg(theme.notice),
        )
    };
    frame.render_widget(
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                " ".repeat(SWATCH),
                Style::new().bg(Color::Rgb(red, green, blue)),
            ),
            Span::raw("  "),
            code,
        ]),
        row(2),
    );
    for (channel, name) in CHANNELS.iter().enumerate() {
        let chosen = channel == picker.channel;
        let mut label = Style::new().fg(theme.help_key);
        if chosen {
            label = label.add_modifier(Modifier::BOLD);
        }
        let mut spans = vec![
            Span::styled(
                if chosen { "  > " } else { "    " },
                Style::new().fg(theme.focus),
            ),
            Span::styled(*name, label),
            Span::styled(
                format!("  {:>3}  ", picker.rgb[channel]),
                Style::new().fg(theme.text),
            ),
        ];
        // The bar: the color with this channel at each cell's value
        let knob = bar_index(picker.rgb[channel]);
        for index in 0..BAR {
            let mut rgb = picker.rgb;
            rgb[channel] = bar_value(index);
            let [r, g, b] = rgb;
            let mut style = Style::new().bg(Color::Rgb(r, g, b));
            let symbol = if index == knob {
                // Black on a light cell, white on a dark one
                let light = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 140_000;
                style = style.fg(if light {
                    Color::Rgb(0, 0, 0)
                } else {
                    Color::Rgb(255, 255, 255)
                });
                "|"
            } else {
                " "
            };
            spans.push(Span::styled(symbol, style));
        }
        let y = CHANNEL_Y + u16::try_from(channel).unwrap_or(0);
        frame.render_widget(Line::from(spans), row(y));
    }
    frame.render_widget(
        Line::styled(
            format!("  {}", texts.picker_typing),
            Style::new().fg(theme.muted),
        ),
        row(HEIGHT - 2),
    );
}

/// What a mouse click at (`column`, `row`) hits: a channel's row (`None` = its name or value; `Some` =
/// the value of the bar cell clicked); `None` elsewhere
pub fn at(app: &App, screen: Rect, column: u16, row: u16) -> Option<(usize, Option<u8>)> {
    let picker = app.picker()?;
    let area = area(app, screen, picker);
    let channel = usize::from(row.checked_sub(area.y + CHANNEL_Y)?);
    if channel >= CHANNELS.len() || column <= area.x || column >= area.right() - 1 {
        return None;
    }
    let index = column.checked_sub(area.x + BAR_X);
    Some((channel, index.filter(|&index| index < BAR).map(bar_value)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, crossterm::event::KeyCode};

    /// An app (70 × 24, zh) with the settings page open and the picker open on today (mocha green
    /// #a6e3a1 = 166, 227, 161), drawn once; returns it, the picker's area and the screen
    fn picker_on_today() -> (App, Rect, ratatui::buffer::Buffer) {
        use ratatui::crossterm::event::KeyEvent;
        let mut app =
            App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new()).with_theme("catppuccin-mocha");
        app.screen = Rect::new(0, 0, 70, 24);
        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        let today = crate::app::rows()
            .iter()
            .position(|&row| row == crate::app::Row::Color("today"))
            .expect("today");
        for _ in 0..today {
            app.handle_key(KeyEvent::from(KeyCode::Char('j')));
        }
        app.handle_key(KeyEvent::from(KeyCode::Char('e')));
        let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &mut app))
            .unwrap();
        let picker = app.picker().expect("open").clone();
        let area = area(&app, app.screen, &picker);
        (app, area, terminal.backend().buffer().clone())
    }

    #[test]
    fn bars_go_from_0_to_255() {
        assert_eq!((bar_value(0), bar_value(BAR - 1)), (0, 255));
        // 23 steps of about 11: cell 12 = 133
        assert_eq!(bar_value(12), 133);
        assert_eq!((bar_index(0), bar_index(255), bar_index(133)), (0, 23, 12));
        // Every cell's value leads back to that cell
        for index in 0..BAR {
            assert_eq!(bar_index(bar_value(index)), index);
        }
    }

    #[test]
    fn the_picker_shows_the_color_its_code_and_a_bar_per_channel() {
        // Width: the key hints on the bottom border, 4 groups (29) + 3 ` | ` + a blank at each end = 40
        // inside (wider than indent 2 + `> R  166  ` 10 + bar 24 + 2 = 38), 42 with the border; in the
        // middle of 70 × 24 → x 14, y 7. Expected rows computed by a separate Python script (knob = the bar
        // cell nearest the value: round(value × 23 / 255))
        let (app, area, buffer) = picker_on_today();
        assert_eq!(area, Rect::new(14, 7, 42, 10));
        let rows = crate::ui::tests::area_rows(&buffer, area);
        assert_eq!(
            rows,
            [
                "╭ 自訂顏色 ∙ today ──────────────────────╮",
                "│                                        │",
                "│            #a6e3a1                     │",
                "│                                        │",
                "│  > R  166                 |            │",
                "│    G  227                      |       │",
                "│    B  161                 |            │",
                "│                                        │",
                "│  也可以直接打色碼：a6e3a1              │",
                "╰ jk 選 | hl HL 調 | Enter 存 | Esc 取消 ╯",
            ]
        );
        // The color: 8 cells with today's green as background
        let at = |dx: u16, dy: u16| &buffer[(area.x + dx, area.y + dy)];
        assert_eq!(at(3, 2).bg, Color::Rgb(166, 227, 161));
        assert_eq!(at(10, 2).bg, Color::Rgb(166, 227, 161));
        assert_eq!(at(11, 2).bg, Color::Reset);
        // Red's bar: from 0 at the left end to 255 at the right end, green and blue as they are; the knob
        // (cell 15 = 166) is black on that light cell
        assert_eq!(at(BAR_X, 4).bg, Color::Rgb(0, 227, 161));
        assert_eq!(at(BAR_X + BAR - 1, 4).bg, Color::Rgb(255, 227, 161));
        assert_eq!(at(BAR_X + 15, 4).symbol(), "|");
        assert_eq!(at(BAR_X + 15, 4).fg, Color::Rgb(0, 0, 0));
        // Clicks: a bar cell = that channel and value; the name = only the channel; the border = nothing
        let click = |dx: u16, dy: u16| at_cell(&app, area, dx, dy);
        assert_eq!(click(BAR_X, 5), Some((1, Some(0))));
        assert_eq!(click(BAR_X + BAR - 1, 6), Some((2, Some(255))));
        assert_eq!(click(4, 6), Some((2, None)));
        assert_eq!(click(BAR_X, 3), None);
        assert_eq!(click(BAR_X, 7), None);
        assert_eq!(click(area.width - 1, 4), None);
    }

    /// `at` for the cell `dx` right and `dy` down from the box's top-left corner
    fn at_cell(app: &App, area: Rect, dx: u16, dy: u16) -> Option<(usize, Option<u8>)> {
        at(app, app.screen, area.x + dx, area.y + dy)
    }
}
