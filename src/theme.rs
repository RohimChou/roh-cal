//! Colors: the screen only names "roles" (today, weekend, week number…), never color names;
//! switching theme = swapping the whole set
//!
//! A theme is one TOML file (the built-in ones are in the repo's `themes/`, compiled into the binary)

use crate::tasks::Priority;
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
#[cfg(test)]
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;
use toml_edit::DocumentMut;

/// A theme: one color per role; read from `[roles]` in the theme file (TOML) (see `Theme::parse`),
/// field name = role name.
/// `Serialize`: tests use it to confirm the role names (`ROLES`, `Theme::color`) match the fields exactly
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(Serialize))]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Normal text: weekday dates, the headers
    pub text: Color,
    /// Secondary text: the key hints at the bottom
    pub muted: Color,
    /// Lines: outer borders, between columns, between months
    pub line: Color,
    /// The thick line where the year changes
    pub year_line: Color,
    /// Titles, the year / month on the left
    pub label: Color,
    /// The week number of the selected week
    pub week_number: Color,
    /// The week numbers of other weeks (dimmer)
    pub week_number_dim: Color,
    /// Days off: Saturday and Sunday (dates and headers) and holidays (the days of items in a holiday
    /// calendar)
    pub weekend: Color,
    /// Today's date
    pub today: Color,
    /// The selected date in layout 1's calendar: the brackets around it `[17]`
    pub selected: Color,
    /// Background of where you are (like vim's cursorline): the selected week's row in the calendar, the
    /// selected day's cell in the month grid, the chosen row in the agenda, the tasks page, the calendar list
    /// and the settings page
    pub selection: Color,
    /// Background of text dragged over with the mouse in the day popup (its characters turn the normal text
    /// color): the theme's blue mixed with the background, lighter than `selection`, so it shows on the chosen
    /// row too
    pub text_selection: Color,
    /// The border of the pane that has keyboard focus; the matching characters in the editor's `#` list
    pub focus: Color,
    /// The `☑` of checked calendars in the side menu
    pub checked: Color,
    /// The notice that briefly replaces the bottom row (write failed, reloaded)
    pub notice: Color,
    /// Mistakes (why the editor cannot save, a badly formatted line on the tasks page)
    pub error: Color,
    /// The faint grid lines between cells in the month grid (layout 2)
    pub grid: Color,
    /// The settings and help sections that do nothing where the keyboard is (a step darker than `muted`). The
    /// name is from when the month grid grayed out the days outside the selected month (it no longer does)
    pub other_month: Color,
    /// Background of a multi-day item's bar in the month grid (layout 2)
    pub range: Color,
    /// The underline under the days where two or more multi-day items overlap, in layout 1's calendar (one
    /// underline cannot be drawn twice, so it changes color instead)
    pub range_overlap: Color,
    /// An item marked `!!!` (high priority) that is not over yet: its whole line (date, time, title)
    pub priority_high: Color,
    /// An item marked `!!` (medium priority) that is not over yet
    pub priority_medium: Color,
    /// An item marked `!` (low priority) that is not over yet
    pub priority_low: Color,
    /// Section names on the help page (`Move`, `Edit`…)
    pub help_section: Color,
    /// Keys on the help page (`hjkl`, `Space`…)
    pub help_key: Color,
}

/// Built-in themes: (name, theme file content); the files are in the repo's `themes/`,
/// compiled into the binary
pub const BUILTIN: [(&str, &str); 8] = [
    (
        "catppuccin-mocha",
        include_str!("../themes/catppuccin-mocha.toml"),
    ),
    ("gruvbox-dark", include_str!("../themes/gruvbox-dark.toml")),
    ("tokyo-night", include_str!("../themes/tokyo-night.toml")),
    ("dracula", include_str!("../themes/dracula.toml")),
    ("nord-bright", include_str!("../themes/nord-bright.toml")),
    ("one-dark", include_str!("../themes/one-dark.toml")),
    ("rose-pine", include_str!("../themes/rose-pine.toml")),
    ("terminal", include_str!("../themes/terminal.toml")),
];

/// The theme used when none is given
pub const DEFAULT: &str = "nord-bright";

/// Names of the built-in themes, in `BUILTIN` order
pub fn names() -> Vec<&'static str> {
    BUILTIN.iter().map(|&(name, _)| name).collect()
}

/// Every role's name, in the order of the `Theme` fields (= the names in a theme file's `[roles]` and the
/// config file's `[colors]`; pinned by a test)
pub const ROLES: [&str; 25] = [
    "text",
    "muted",
    "line",
    "year_line",
    "label",
    "week_number",
    "week_number_dim",
    "weekend",
    "today",
    "selected",
    "selection",
    "text_selection",
    "focus",
    "checked",
    "notice",
    "error",
    "grid",
    "other_month",
    "range",
    "range_overlap",
    "priority_high",
    "priority_medium",
    "priority_low",
    "help_section",
    "help_key",
];

/// The terminal's 16 colors: when a theme has no palette (e.g. `terminal`), the settings page picks
/// colors from these
const ANSI: [Color; 16] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

/// The colors each built-in theme can pick from (in `BUILTIN` order): the theme files are read once on
/// first use, and this copy is used after that
static CHOICES: LazyLock<Vec<Vec<(String, Color)>>> = LazyLock::new(|| {
    BUILTIN
        .iter()
        .map(|&(_, text)| read_choices(text))
        .collect()
});

/// The colors the built-in theme `name` can pick from: its palette (name, color), in theme file order;
/// without a palette (e.g. `terminal`), the terminal's 16 colors (name = how the config file writes it);
/// empty if there is no such theme
pub fn choices(name: &str) -> &'static [(String, Color)] {
    BUILTIN
        .iter()
        .position(|(builtin, _)| *builtin == name)
        .map_or(&[], |index| CHOICES[index].as_slice())
}

/// Read the colors to pick from out of a theme file (`text`) (see `choices`)
fn read_choices(text: &str) -> Vec<(String, Color)> {
    let palette: Vec<(String, Color)> = text
        .parse::<DocumentMut>()
        .ok()
        .and_then(|doc| {
            let table = doc.get("palette")?.as_table()?;
            Some(
                table
                    .iter()
                    .filter_map(|(key, value)| {
                        Some((key.to_string(), value.as_str()?.parse().ok()?))
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();
    if palette.is_empty() {
        ANSI.iter()
            .map(|&color| (color_text(color), color))
            .collect()
    } else {
        palette
    }
}

/// A color written as in the config file: `#cdd6f4`, `236` (a 256-color index); terminal colors use
/// ratatui's spelling `LightBlue`
/// (capitalized: palette names are all lower case, so even if a theme's palette also has `cyan`, it reads
/// back as the terminal's cyan)
pub fn color_text(color: Color) -> String {
    match color {
        Color::Rgb(red, green, blue) => format!("#{red:02x}{green:02x}{blue:02x}"),
        named => named.to_string(),
    }
}

/// The shape of a theme file (TOML): `[palette]` the palette (names are up to you, optional), `[roles]`
/// which color each role uses
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    /// Palette: name → color (`"#rrggbb"`); `[roles]` can use these names
    #[serde(default)]
    palette: BTreeMap<String, String>,
    /// Which color each role uses (a palette name or a color); `Theme::parse` then turns them into colors
    /// one by one
    roles: toml::Table,
}

/// The default theme (`DEFAULT`)
impl Default for Theme {
    fn default() -> Self {
        Self::builtin(DEFAULT).unwrap_or_else(|| unreachable!("built-in theme"))
    }
}

impl Theme {
    /// [Catppuccin Mocha](https://catppuccin.com/palette): soft colors for dark backgrounds
    /// (v1's default; tests check its exact colors)
    #[cfg(test)]
    pub fn catppuccin_mocha() -> Self {
        Self::builtin("catppuccin-mocha").unwrap_or_else(|| unreachable!("built-in theme"))
    }

    /// The built-in theme called `name` (with no colors overridden); `None` if there is no such name
    pub fn builtin(name: &str) -> Option<Self> {
        let (_, text) = BUILTIN.iter().find(|(builtin, _)| *builtin == name)?;
        // The built-in theme files are compiled into the binary and a test checks that each one reads;
        // failing to read one is a programming bug, not the user's fault
        Some(
            Self::parse(text, &BTreeMap::new())
                .unwrap_or_else(|error| unreachable!("built-in theme {name}: {error}")),
        )
    }

    /// The built-in theme `name`, with some roles overridden by `colors` (the config file's `[colors]`:
    /// role → color, or a palette name of that theme);
    /// no such theme, an unknown role, or something that is not a color is an error (the error message says
    /// what is wrong)
    pub fn build(name: &str, colors: &BTreeMap<String, String>) -> Result<Self, String> {
        let (_, text) = BUILTIN
            .iter()
            .find(|(builtin, _)| *builtin == name)
            .ok_or_else(|| {
                format!(
                    "no theme named \"{name}\" (built-in: {})",
                    names().join(", ")
                )
            })?;
        // Check the overriding colors one by one first, so the error message can say which one is wrong
        let file: ThemeFile = toml::from_str(text).map_err(|error| error.to_string())?;
        for (role, color) in colors {
            if !ROLES.contains(&role.as_str()) {
                return Err(format!(
                    "[colors] {role}: no such role (the roles are the names under [roles] in themes/)"
                ));
            }
            if !file.palette.contains_key(color) && color.parse::<Color>().is_err() {
                return Err(format!(
                    "[colors] {role} = \"{color}\": not a color, nor a palette name of {name}"
                ));
            }
        }
        Self::parse(text, colors).map_err(|error| format!("[colors]: {}", error.message()))
    }

    /// The current color of role `role` (a name in `ROLES`); `None` if there is no such role.
    /// Names match the fields one to one, pinned by a test
    pub fn color(&self, role: &str) -> Option<Color> {
        let color = match role {
            "text" => self.text,
            "muted" => self.muted,
            "line" => self.line,
            "year_line" => self.year_line,
            "label" => self.label,
            "week_number" => self.week_number,
            "week_number_dim" => self.week_number_dim,
            "weekend" => self.weekend,
            "today" => self.today,
            "selected" => self.selected,
            "selection" => self.selection,
            "text_selection" => self.text_selection,
            "focus" => self.focus,
            "checked" => self.checked,
            "notice" => self.notice,
            "error" => self.error,
            "grid" => self.grid,
            "other_month" => self.other_month,
            "range" => self.range,
            "range_overlap" => self.range_overlap,
            "priority_high" => self.priority_high,
            "priority_medium" => self.priority_medium,
            "priority_low" => self.priority_low,
            "help_section" => self.help_section,
            "help_key" => self.help_key,
            _ => return None,
        };
        Some(color)
    }

    /// The style for what does not work here (settings rows, help sections): `other_month`'s color;
    /// `reset` there asks the terminal for its faint text instead (the text color dimmed; the terminal
    /// theme's 16 colors cannot mix the text with the background)
    pub fn dimmed(&self) -> Style {
        match self.other_month {
            Color::Reset => Style::new().add_modifier(Modifier::DIM),
            color => Style::new().fg(color),
        }
    }

    /// The color of a priority (`!!!` → `priority_high`…)
    pub fn priority(&self, priority: Priority) -> Color {
        match priority {
            Priority::High => self.priority_high,
            Priority::Medium => self.priority_medium,
            Priority::Low => self.priority_low,
        }
    }

    /// Turn a theme file (TOML) into a `Theme`: first override some roles with `colors`, then replace
    /// palette names in `[roles]` with the colors from `[palette]`,
    /// and finally read each role as a color (`"#rrggbb"`, terminal color names like `"blue"`, and 256-color
    /// indexes all work).
    /// A missing role, an unknown role, or something that is not a color is an error
    fn parse(text: &str, colors: &BTreeMap<String, String>) -> Result<Self, toml::de::Error> {
        let ThemeFile { palette, mut roles } = toml::from_str(text)?;
        for (role, color) in colors {
            roles.insert(role.clone(), toml::Value::String(color.clone()));
        }
        for (_, value) in roles.iter_mut() {
            if let Some(color) = value.as_str().and_then(|name| palette.get(name)) {
                *value = toml::Value::String(color.clone());
            }
        }
        toml::Value::Table(roles).try_into()
    }
}

#[cfg(test)]
mod tests;
