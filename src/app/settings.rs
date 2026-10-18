//! Settings page (`,`): the config file's settings as one list (theme,
//! switches, each role's color), drawn over the middle of the screen;
//! changes show on screen right away, and are saved to the config file on close
//!
//! `j` / `k` choose a row, `h` / `l` (`Space`, `Enter`) change that row's
//! value, `r` resets that row, `R` resets everything (press `y` to confirm),
//! `q` / `Esc` / `,` or a click outside the box closes it

use super::{App, Mode, Notice};
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
            Self::ListWidth => Some("calendar_list"),
            Self::Color(_) => Some("colors"),
        }
    }

    /// The name shown on the left = the name in the config file (pinned by a test)
    pub fn name(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::RelativeWeeks => "relative_weeks",
            Self::GridLines => "grid_lines",
            Self::CenterCursor => "center_cursor",
            Self::ListWidth => "max_width",
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
    fn close_settings(&mut self) {
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
mod tests {
    use super::*;
    use crate::config::{CalendarSettings, MonthSettings};
    use crate::texts::Lang;
    use crate::theme::Theme;
    use jiff::civil::date;
    use ratatui::layout::Rect;

    /// An app with the settings page open (terminal 100 × 30)
    fn app_with_settings() -> App {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 100, 30);
        press(&mut app, KeyCode::Char(','));
        app
    }

    /// Presses one key
    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn closing_saves_only_what_changed_and_says_when_it_cannot() {
        let dir = crate::files::test_dir("settings-save");
        let path = dir.join("config.toml");
        // Open it to look and close without changes: no file is created
        let mut app = app_with_settings();
        app.config_path = Some(path.clone());
        press(&mut app, KeyCode::Esc);
        assert!(!path.exists());
        // Change relative_weeks, then close: only that line is written
        press(&mut app, KeyCode::Char(','));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[calendar]\nrelative_weeks = true\n"
        );
        assert_eq!(app.notice, None);
        // Ctrl+C saves before quitting too
        press(&mut app, KeyCode::Char(','));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
        assert!(
            std::fs::read_to_string(&path)
                .expect("read")
                .contains("grid_lines = false")
        );
        // The config file was broken elsewhere: not saved, the
        // bottom row says it cannot save, the file is left untouched
        std::fs::write(&path, "[month]\ngrid_line = false\n").expect("write");
        let mut app = app_with_settings();
        app.config_path = Some(path.clone());
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(
            app.notice,
            Some(Notice::SettingsFailed(std::io::ErrorKind::InvalidData))
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[month]\ngrid_line = false\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn color_rows_go_through_the_palette_and_keep_only_overrides() {
        let mut app = app_with_settings();
        // Row 6 is text's color: l switches to the palette's next color
        // (subtext0), records it in [colors], and the screen changes right away
        for _ in 0..5 {
            press(&mut app, KeyCode::Char('j'));
        }
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(
            app.settings.colors.get("text").map(String::as_str),
            Some("#a6adc8")
        );
        assert_eq!(app.theme.text, ratatui::style::Color::Rgb(0xa6, 0xad, 0xc8));
        assert_eq!(
            Row::Color("text").value(&app.settings, &app.theme),
            "subtext0"
        );
        // h goes back to the theme's own color: not recorded (the config file only records changes)
        press(&mut app, KeyCode::Char('h'));
        assert!(app.settings.colors.is_empty());
        // h again: wraps around to the palette's last color (mauve); r resets
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(
            app.settings.colors.get("text").map(String::as_str),
            Some("#cba6f7")
        );
        press(&mut app, KeyCode::Char('r'));
        assert!(app.settings.colors.is_empty());
        assert_eq!(app.theme, Theme::catppuccin_mocha());
    }

    #[test]
    fn switching_the_theme_keeps_the_overridden_colors() {
        // The config file has a hand-written mocha palette name; switching to terminal, which has no palette:
        // it is first written as the color itself, so it stays the same color and the theme can be built
        let mut app = app_with_settings();
        app.settings
            .colors
            .insert("today".to_string(), "surface0".to_string());
        app.apply_theme();
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.settings.theme, "terminal");
        assert_eq!(
            app.settings.colors.get("today").map(String::as_str),
            Some("#313244")
        );
        assert_eq!(
            app.theme.today,
            ratatui::style::Color::Rgb(0x31, 0x32, 0x44)
        );
        assert_eq!(app.theme.text, ratatui::style::Color::Reset);
        // terminal has no palette: colors change within the 16 colors
        assert_eq!(
            Row::Color("today").value(&app.settings, &app.theme),
            "#313244"
        );
    }

    #[test]
    fn r_on_the_theme_row_keeps_the_overridden_colors_too() {
        // The config file has a gruvbox palette name: r goes back to the default mocha, the overridden color
        // is first written as the color itself, so the theme builds (and the saved file opens next time)
        let mut app = app_with_settings();
        app.settings.theme = "gruvbox-dark".to_string();
        app.settings
            .colors
            .insert("today".to_string(), "light1".to_string());
        app.apply_theme();
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.settings.theme, "catppuccin-mocha");
        assert_eq!(
            app.settings.colors.get("today").map(String::as_str),
            Some("#ebdbb2")
        );
        assert_eq!(
            app.theme.today,
            ratatui::style::Color::Rgb(0xeb, 0xdb, 0xb2)
        );
    }

    #[test]
    fn a_terminal_color_stays_itself_in_a_theme_with_the_same_palette_name() {
        // terminal's cyan switched to tokyo-night (whose palette also has cyan = #7dcfff):
        // stays terminal's cyan, not replaced by the palette color of the same name
        let mut app = app_with_settings();
        app.settings.theme = "terminal".to_string();
        app.settings
            .colors
            .insert("weekend".to_string(), "cyan".to_string());
        app.settings.set_theme("tokyo-night");
        app.apply_theme();
        assert_eq!(app.settings.theme, "tokyo-night");
        assert_eq!(app.theme.weekend, ratatui::style::Color::Cyan);
    }

    #[test]
    fn every_switch_has_a_row() {
        // Destructure `Settings` into every field: if a setting is added and
        // not added here, it won't compile, a reminder to add a row in `rows()`
        let Settings {
            theme: _,
            colors: _,
            calendar: CalendarSettings { relative_weeks: _ },
            month:
                MonthSettings {
                    grid_lines: _,
                    center_cursor: _,
                    // No longer a setting (only read so older files open): no row
                    removed_item_icons: _,
                },
            calendar_list: CalendarListSettings { max_width: _ },
        } = Settings::default();
        assert_eq!(rows().len(), 5 + theme::ROLES.len());
    }

    #[test]
    fn the_list_width_steps_by_one_cell_and_stops_at_the_ends() {
        let mut settings = Settings::default();
        // Default 36: l = 37, h twice = 35
        Row::ListWidth.change(&mut settings, 1);
        assert_eq!(settings.calendar_list.max_width, 37);
        Row::ListWidth.change(&mut settings, -1);
        Row::ListWidth.change(&mut settings, -1);
        assert_eq!(settings.calendar_list.max_width, 35);
        // Stops at 22 and 60 instead of wrapping around
        settings.calendar_list.max_width = 22;
        Row::ListWidth.change(&mut settings, -1);
        assert_eq!(settings.calendar_list.max_width, 22);
        settings.calendar_list.max_width = 60;
        Row::ListWidth.change(&mut settings, 1);
        assert_eq!(settings.calendar_list.max_width, 60);
        // A hand-written 100 in the config file counts as 60: shown as 60, h goes to 59
        settings.calendar_list.max_width = 100;
        let theme = Theme::catppuccin_mocha();
        assert_eq!(Row::ListWidth.value(&settings, &theme), "60");
        Row::ListWidth.change(&mut settings, -1);
        assert_eq!(settings.calendar_list.max_width, 59);
        // r: back to 36
        Row::ListWidth.reset(&mut settings);
        assert_eq!(settings.calendar_list.max_width, 36);
    }

    #[test]
    fn row_names_are_the_config_file_names() {
        // Change each row's value once, write it as a config file line "[table]
        // name = new value", and reading it back gives exactly the changed settings
        for row in rows() {
            let mut changed = Settings::default();
            row.change(&mut changed, 1);
            let value = match row {
                Row::Theme => format!("\"{}\"", changed.theme),
                Row::Color(role) => format!("\"{}\"", changed.colors[role]),
                Row::ListWidth => changed.calendar_list.max_width.to_string(),
                _ => (row.value(&changed, &Theme::catppuccin_mocha()) == "on").to_string(),
            };
            let table = row
                .section()
                .map(|section| format!("[{section}]\n"))
                .unwrap_or_default();
            let text = format!("{table}{} = {value}\n", row.name());
            let read: Settings = toml::from_str(&text).expect(&text);
            assert_eq!(read, changed, "{text}");
        }
    }

    #[test]
    fn comma_opens_it_and_hl_change_the_chosen_row() {
        let mut app = app_with_settings();
        assert_eq!(app.settings_page(), Some(SettingsPage::default()));
        // First row, theme: l goes to the next theme (the screen colors change
        // right away), l on the last theme goes back to the first, h goes back
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.settings.theme, "gruvbox-dark");
        assert_eq!(Some(&app.theme), Theme::builtin("gruvbox-dark").as_ref());
        // In `theme::BUILTIN` order: gruvbox-dark → tokyo-night → dracula → nord → terminal → back to mocha
        for expected in [
            "tokyo-night",
            "dracula",
            "nord",
            "terminal",
            "catppuccin-mocha",
        ] {
            press(&mut app, KeyCode::Char('l'));
            assert_eq!(app.settings.theme, expected);
        }
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.settings.theme, "terminal");
        assert_eq!(app.theme.text, ratatui::style::Color::Reset);
        // j to relative_weeks: l turns it on, h turns it off again
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        assert!(app.settings.calendar.relative_weeks);
        press(&mut app, KeyCode::Char('h'));
        assert!(!app.settings.calendar.relative_weeks);
        // j to grid_lines, Space turns it off; k back to the first row; it does not go above the top
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.settings.month.grid_lines);
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(app.settings_page().map(|page| page.row), Some(0));
        // At the end it also stops on the last row
        for _ in 0..40 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(
            app.settings_page().map(|page| page.row),
            Some(rows().len() - 1)
        );
    }

    #[test]
    fn r_resets_a_row_and_capital_r_then_y_resets_everything() {
        let mut app = app_with_settings();
        // theme switched to gruvbox-dark, relative_weeks on, grid_lines off
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        // r: resets only grid_lines, the rest stays
        press(&mut app, KeyCode::Char('r'));
        assert!(app.settings.month.grid_lines);
        assert!(app.settings.calendar.relative_weeks);
        assert_eq!(app.settings.theme, "gruvbox-dark");
        // Another key after R: it only cancels, and that key does nothing else (j does not move)
        press(&mut app, KeyCode::Char('R'));
        assert_eq!(
            app.settings_page().map(|page| page.confirm_reset),
            Some(true)
        );
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.settings_page().map(|page| page.confirm_reset),
            Some(false)
        );
        assert_eq!(app.settings_page().map(|page| page.row), Some(2));
        assert!(app.settings.calendar.relative_weeks);
        // R then y: resets everything, and the screen colors go back to the default theme
        press(&mut app, KeyCode::Char('R'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.settings, Settings::default());
        assert_eq!(app.theme, Theme::catppuccin_mocha());
        assert!(app.settings_page().is_some());
        // The theme row can also be reset alone with r
        for _ in 0..2 {
            press(&mut app, KeyCode::Char('k'));
        }
        press(&mut app, KeyCode::Char('h'));
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.settings.theme, "catppuccin-mocha");
        assert_eq!(app.theme, Theme::catppuccin_mocha());
    }

    #[test]
    fn clicking_chooses_a_row_then_changes_it_and_clicking_outside_closes() {
        // 100 × 30: the box (46 × 38) is taller than the screen, so it shrinks to 30 high,
        // at (27, 0); grid_lines is line 6 of the list: 0 + border 1 + padding 1 + 6 = 8
        let mut app = app_with_settings();
        let click = |app: &mut App, column, row| {
            app.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            });
        };
        click(&mut app, 50, 8);
        assert_eq!(app.settings_page().map(|page| page.row), Some(2));
        assert!(app.settings.month.grid_lines);
        // Click the chosen row again: change the value
        click(&mut app, 50, 8);
        assert!(!app.settings.month.grid_lines);
        // Wheel: choose the next row
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 50,
            row: 8,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.settings_page().map(|page| page.row), Some(3));
        // Click outside the box: closes it, and the calendar underneath is not clicked
        click(&mut app, 0, 0);
        assert_eq!(app.settings_page(), None);
        assert_eq!(app.selected, date(2026, 10, 17));
    }

    #[test]
    fn q_esc_and_comma_close_it_and_other_keys_do_nothing() {
        for code in [KeyCode::Char('q'), KeyCode::Esc, KeyCode::Char(',')] {
            let mut app = app_with_settings();
            press(&mut app, code);
            assert_eq!(app.settings_page(), None, "{code:?}");
            assert!(!app.should_quit, "{code:?}");
        }
        // Moving the date, switching layouts and the help page are all ignored
        let mut app = app_with_settings();
        for code in ['t', ']', '3', '?', 'a'].map(KeyCode::Char) {
            press(&mut app, code);
        }
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.view, super::super::View::Planner);
        assert_eq!(app.help(), None);
        assert_eq!(app.settings_page(), Some(SettingsPage::default()));
    }
}
