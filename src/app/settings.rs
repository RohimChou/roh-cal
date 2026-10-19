//! Settings page (`,`): the config file's settings as one list (theme,
//! switches, each role's color), drawn over the middle of the screen;
//! changes show on screen right away, and are saved to the config file on close
//!
//! `j` / `k` choose a row, `h` / `l` (`Space`, `Enter`) change that row's
//! value, `e` on a color row opens the color picker (`picker.rs`), `r` resets that row, `R` resets
//! everything (press `y` to confirm), `q` / `Esc` / `,` or a click outside the box closes it

use super::{App, Mode, Notice, View};
use crate::config::{self, CalendarListSettings, Settings};
use crate::theme::{self, Theme};
use crate::ui;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    layout::Position,
};

/// The settings page is open: which row is chosen, where it
/// is scrolled, and whether it is asking to reset everything
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SettingsPage {
    /// Which row of `rows()` is chosen
    pub row: usize,
    /// When it does not fit on screen, which line is at the top of the box
    /// (each draw scrolls so the chosen row is visible, see `ui::settings`)
    pub top: usize,
    /// The first color shown in `[colors]` (0 = `text`): the section shows a few colors at a time and
    /// scrolls inside to the chosen one (see `ui::settings`)
    pub colors_top: usize,
    /// `R` was pressed and it is asking whether to reset everything (the bottom border becomes the question)
    pub confirm_reset: bool,
}

/// One row of the settings page = one setting in the config file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The top-level `theme`: which built-in theme to use
    Theme,
    /// `[calendar] relative_weeks`
    RelativeWeeks,
    /// `[month] grid_lines`
    GridLines,
    /// `[month] center_cursor`
    CenterCursor,
    /// `[calendar_list] max_width`: a number, not a switch
    ListWidth,
    /// `[calendar_list] hide_done`
    HideDone,
    /// One role in `[colors]` (a name in `theme::ROLES`): overrides that color of the theme
    Color(&'static str),
}

/// The settings page's rows, in display order (same as the config file:
/// top-level first, rows of the same table together; each role's color last)
pub fn rows() -> Vec<Row> {
    let mut rows = vec![
        Row::Theme,
        Row::RelativeWeeks,
        Row::GridLines,
        Row::CenterCursor,
        Row::ListWidth,
        Row::HideDone,
    ];
    rows.extend(theme::ROLES.map(Row::Color));
    rows
}

impl Row {
    /// Which table of the config file this row is in (`[calendar]`, `[month]`, `[colors]`);
    /// `None` for the top level (`theme`). The settings page is split into sections by this too
    pub fn section(self) -> Option<&'static str> {
        match self {
            Self::Theme => None,
            Self::RelativeWeeks => Some("calendar"),
            Self::GridLines | Self::CenterCursor => Some("month"),
            Self::ListWidth | Self::HideDone => Some("calendar_list"),
            Self::Color(_) => Some("colors"),
        }
    }

    /// The views this setting changes something on (the settings page dims it on the others);
    /// `None` = every view
    pub fn views(self) -> Option<&'static [View]> {
        match self {
            Self::RelativeWeeks => Some(&[View::Planner, View::Month]),
            Self::GridLines | Self::CenterCursor => Some(&[View::Month]),
            // Colors only the calendars draw: the year line, week numbers, the selected brackets; the month
            // grid's own: its grid lines and multi-day bars
            Self::Color("year_line" | "week_number" | "week_number_dim" | "selected") => {
                Some(&[View::Planner, View::Month])
            }
            Self::Color("grid" | "range") => Some(&[View::Month]),
            // Only layout 1's calendar underlines overlapping multi-day items
            Self::Color("range_overlap") => Some(&[View::Planner]),
            Self::Theme | Self::ListWidth | Self::HideDone | Self::Color(_) => None,
        }
    }

    /// Whether this setting changes something on `view`
    pub fn applies_to(self, view: View) -> bool {
        self.views().is_none_or(|views| views.contains(&view))
    }

    /// The name shown on the left = the name in the config file (pinned by a test)
    pub fn name(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::RelativeWeeks => "relative_weeks",
            Self::GridLines => "grid_lines",
            Self::CenterCursor => "center_cursor",
            Self::ListWidth => "max_width",
            Self::HideDone => "hide_done",
            Self::Color(role) => role,
        }
    }

    /// For switch rows, that `bool` in `settings` (`&mut`: for changing
    /// and resetting); `theme`, the width and colors are not switches, so `None`
    fn switch(self, settings: &mut Settings) -> Option<&mut bool> {
        match self {
            Self::Theme | Self::ListWidth | Self::Color(_) => None,
            Self::RelativeWeeks => Some(&mut settings.calendar.relative_weeks),
            Self::GridLines => Some(&mut settings.month.grid_lines),
            Self::CenterCursor => Some(&mut settings.month.center_cursor),
            Self::HideDone => Some(&mut settings.calendar_list.hide_done),
        }
    }

    /// The current value (text drawn on the right): switches are `on` / `off`, `theme` is the theme's name,
    /// the width is a number (the one that counts, see `CalendarListSettings::max_width`);
    /// colors are their palette name (a color not in the palette is written as
    /// itself, e.g. `#010203`). `theme` = the screen colors built from `settings`
    pub fn value(self, settings: &Settings, theme: &Theme) -> String {
        let on_off = |on: bool| if on { "on" } else { "off" }.to_string();
        match self {
            Self::Theme => settings.theme.clone(),
            Self::Color(_) => {
                let Some(color) = self.color(theme) else {
                    return String::new();
                };
                theme::choices(&settings.theme)
                    .iter()
                    .find(|&&(_, choice)| choice == color)
                    .map_or_else(|| theme::color_text(color), |(name, _)| name.clone())
            }
            Self::RelativeWeeks => on_off(settings.calendar.relative_weeks),
            Self::GridLines => on_off(settings.month.grid_lines),
            Self::CenterCursor => on_off(settings.month.center_cursor),
            Self::HideDone => on_off(settings.calendar_list.hide_done),
            Self::ListWidth => settings.calendar_list.max_width().to_string(),
        }
    }

    /// For color rows, the current color in `theme` (the screen
    /// colors built from the settings); `None` for other rows
    pub fn color(self, theme: &Theme) -> Option<ratatui::style::Color> {
        let Self::Color(role) = self else {
            return None;
        };
        theme.color(role)
    }

    /// Changes this row's value: switches on ⇄ off; `theme` goes to
    /// the next (`step` = 1) / previous (-1) theme in built-in order;
    /// colors go to the next / previous one in the theme's palette
    /// (terminal's 16 colors if there is no palette). Both wrap around.
    /// The width gets 1 cell wider / narrower, stopping at `MIN_WIDTH` / `MAX_WIDTH` (no wrapping)
    fn change(self, settings: &mut Settings, step: isize) {
        if let Some(switch) = self.switch(settings) {
            *switch = !*switch;
            return;
        }
        match self {
            Self::ListWidth => {
                let list = &mut settings.calendar_list;
                list.max_width = list
                    .max_width()
                    .saturating_add_signed(i16::try_from(step).unwrap_or(0))
                    .clamp(
                        CalendarListSettings::MIN_WIDTH,
                        CalendarListSettings::MAX_WIDTH,
                    );
            }
            Self::Color(role) => {
                let choices = theme::choices(&settings.theme);
                let current = Theme::build(&settings.theme, &settings.colors)
                    .ok()
                    .and_then(|theme| theme.color(role));
                // If the current color is not in the palette (e.g. a
                // hand-written color code), start from the first (or last)
                let next = match choices
                    .iter()
                    .position(|&(_, color)| Some(color) == current)
                {
                    Some(index) => wrap(index, step, choices.len()),
                    None if step > 0 => 0,
                    None => choices.len() - 1,
                };
                let color = choices[next].1;
                // Same as the theme's own color: don't record it (the config file only records changes)
                let base = Theme::builtin(&settings.theme).and_then(|theme| theme.color(role));
                if Some(color) == base {
                    settings.colors.remove(role);
                } else {
                    settings
                        .colors
                        .insert(role.to_string(), theme::color_text(color));
                }
            }
            _ => {
                let names = theme::names();
                let index = names
                    .iter()
                    .position(|&name| name == settings.theme)
                    .unwrap_or(0);
                settings.set_theme(names[wrap(index, step, names.len())]);
            }
        }
    }

    /// Resets this row to its default (color = remove the override, back to the theme's color)
    fn reset(self, settings: &mut Settings) {
        let mut defaults = Settings::default();
        if let Self::Color(role) = self {
            settings.colors.remove(role);
            return;
        }
        if self == Self::ListWidth {
            settings.calendar_list = defaults.calendar_list;
            return;
        }
        // Switch: use the same switch's value from the defaults; the only non-switch left is theme
        if let Some(default) = self.switch(&mut defaults).copied() {
            if let Some(switch) = self.switch(settings) {
                *switch = default;
            }
            return;
        }
        settings.set_theme(&defaults.theme);
    }
}

/// Steps from the `index`-th one backward (`step` = −1) / forward (+1), wrapping around;
/// returns which of the `len` items it lands on
fn wrap(index: usize, step: isize, len: usize) -> usize {
    index
        .cast_signed()
        .saturating_add(step)
        .rem_euclid(len.cast_signed())
        .cast_unsigned()
}

impl App {
    /// `,`: opens the settings page with the first row chosen
    pub(super) fn open_settings(&mut self) {
        self.mode = Mode::Settings {
            page: SettingsPage::default(),
            opened: self.settings.clone(),
            picker: None,
        };
    }

    /// Replaces the settings page's state (which row is chosen,
    /// whether it is asking); ignored if the settings page is not open
    fn set_settings_page(&mut self, new: SettingsPage) {
        if let Mode::Settings { page, .. } = &mut self.mode {
            *page = new;
        }
    }

    /// Keys while the settings page is open: see this module's top comment; other keys are ignored
    pub(super) fn handle_settings_key(&mut self, key: KeyEvent) {
        let Some(page) = self.settings_page() else {
            return;
        };
        // The color picker is open over it: the keys are the picker's
        if self.picker().is_some() {
            self.handle_picker_key(key);
            return;
        }
        // Asking whether to reset everything: `y` resets, any other key just cancels
        if page.confirm_reset {
            if key.code == KeyCode::Char('y') {
                self.settings = Settings::default();
                self.apply_theme();
            }
            self.set_settings_page(SettingsPage {
                confirm_reset: false,
                ..page
            });
            return;
        }
        let rows = rows();
        let row = rows[page.row.min(rows.len() - 1)];
        match key.code {
            // Save to the config file before quitting too
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.close_settings();
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => self.choose_setting(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_setting(-1),
            KeyCode::Char('h') | KeyCode::Left => self.change_setting(row, -1),
            KeyCode::Char('l' | ' ') | KeyCode::Right | KeyCode::Enter => {
                self.change_setting(row, 1);
            }
            KeyCode::Char('e') => {
                if let Row::Color(role) = row {
                    self.open_picker(role);
                }
            }
            KeyCode::Char('r') => {
                row.reset(&mut self.settings);
                self.apply_theme();
            }
            KeyCode::Char('R') => {
                self.set_settings_page(SettingsPage {
                    confirm_reset: true,
                    ..page
                });
            }
            KeyCode::Char('q' | ',') | KeyCode::Esc => self.close_settings(),
            _ => {}
        }
    }

    /// Changes `row`'s value (`step`: see `Row::change`); when
    /// the theme changes, the screen colors change right away
    fn change_setting(&mut self, row: Row, step: isize) {
        row.change(&mut self.settings, step);
        self.apply_theme();
    }

    /// Mouse while the settings page is open: the wheel chooses the row above / below; clicking a row =
    /// choose it, clicking the chosen row again = change its value; clicking outside the box closes it
    pub(super) fn handle_settings_mouse(&mut self, mouse: MouseEvent) {
        let Some(page) = self.settings_page() else {
            return;
        };
        if self.picker().is_some() {
            self.handle_picker_mouse(mouse);
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollDown => self.choose_setting(1),
            MouseEventKind::ScrollUp => self.choose_setting(-1),
            MouseEventKind::Down(MouseButton::Left) => {
                let position = Position::new(mouse.column, mouse.row);
                if !ui::settings_area(self).contains(position) {
                    self.close_settings();
                    return;
                }
                match ui::settings_row_at(self, mouse.column, mouse.row) {
                    Some(row) if row == page.row => self.change_setting(rows()[row], 1),
                    Some(row) => self.set_settings_page(SettingsPage { row, ..page }),
                    None => {}
                }
            }
            _ => {}
        }
    }

    /// Chooses the next row (`step` = 1) / previous row (-1); stops at either end
    fn choose_setting(&mut self, step: isize) {
        if let Mode::Settings { page, .. } = &mut self.mode {
            page.row = page.row.saturating_add_signed(step).min(rows().len() - 1);
        }
    }

    /// Closes the settings page and saves the settings changed since it opened to the config file (only
    /// those lines; nothing changed means no write); if it cannot save, says so in the bottom row
    pub(super) fn close_settings(&mut self) {
        let before = match std::mem::take(&mut self.mode) {
            Mode::Settings { opened, .. } => opened,
            // The settings page is not open: do nothing (put it back)
            other => {
                self.mode = other;
                return;
            }
        };
        if let Some(path) = &self.config_path
            && let Err(error) = config::save(path, &before, &self.settings)
        {
            self.notice = Some(Notice::SettingsFailed(error.kind()));
        }
    }
}

#[cfg(test)]
mod tests;
