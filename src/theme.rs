//! Colors: the screen only names "roles" (today, weekend, week number…), never color names;
//! switching theme = swapping the whole set
//!
//! A theme is one TOML file (the built-in ones are in the repo's `themes/`, compiled into the binary)

use crate::tasks::Priority;
use ratatui::style::Color;
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
    /// Normal text: weekday dates
    pub text: Color,
    /// Secondary text: headers, the key hints at the bottom
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
    /// Background of the selected week's row in the calendar, the chosen item in the agenda and the selected
    /// day's cell in the month grid (like vim's cursorline)
    pub cursor_line: Color,
    /// The border of the pane that has keyboard focus
    pub focus: Color,
    /// The `☑` of checked calendars in the side menu
    pub checked: Color,
    /// The notice that briefly replaces the bottom row (write failed, reloaded)
    pub notice: Color,
    /// Mistakes (why the editor cannot save, a badly formatted line on the tasks page)
    pub error: Color,
    /// The faint grid lines between cells in the month grid (layout 2)
    pub grid: Color,
    /// Days in the month grid (layout 2) outside the selected month: dates and items turn gray; also the
    /// settings and help sections that do nothing where the keyboard is (a step darker than `muted`)
    pub other_month: Color,
    /// Background of a multi-day item's bar in the month grid (layout 2)
    pub range: Color,
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
pub const BUILTIN: [(&str, &str); 6] = [
    (
        "catppuccin-mocha",
        include_str!("../themes/catppuccin-mocha.toml"),
    ),
    ("gruvbox-dark", include_str!("../themes/gruvbox-dark.toml")),
    ("tokyo-night", include_str!("../themes/tokyo-night.toml")),
    ("dracula", include_str!("../themes/dracula.toml")),
    ("nord", include_str!("../themes/nord.toml")),
    ("terminal", include_str!("../themes/terminal.toml")),
];

/// The theme used when none is given
pub const DEFAULT: &str = "catppuccin-mocha";

/// Names of the built-in themes, in `BUILTIN` order
pub fn names() -> Vec<&'static str> {
    BUILTIN.iter().map(|&(name, _)| name).collect()
}

/// Every role's name, in the order of the `Theme` fields (= the names in a theme file's `[roles]` and the
/// config file's `[colors]`; pinned by a test)
pub const ROLES: [&str; 23] = [
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
    "cursor_line",
    "focus",
    "checked",
    "notice",
    "error",
    "grid",
    "other_month",
    "range",
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

impl Theme {
    /// [Catppuccin Mocha](https://catppuccin.com/palette): soft colors for dark backgrounds
    /// (same as v1's default)
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
            "cursor_line" => self.cursor_line,
            "focus" => self.focus,
            "checked" => self.checked,
            "notice" => self.notice,
            "error" => self.error,
            "grid" => self.grid,
            "other_month" => self.other_month,
            "range" => self.range,
            "priority_high" => self.priority_high,
            "priority_medium" => self.priority_medium,
            "priority_low" => self.priority_low,
            "help_section" => self.help_section,
            "help_key" => self.help_key,
            _ => return None,
        };
        Some(color)
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
mod tests {
    use super::*;

    impl Theme {
        /// Read only the theme file, with no colors overridden (for tests)
        fn parse_text(text: &str) -> Result<Self, toml::de::Error> {
            Self::parse(text, &BTreeMap::new())
        }
    }

    #[test]
    fn roles_list_every_field_in_order() {
        // `ROLES` matches the fields of `Theme` exactly (the names and order of Theme turned into TOML)
        let doc = toml_edit::ser::to_document(&Theme::catppuccin_mocha()).expect("serialize");
        let fields: Vec<&str> = doc.iter().map(|(key, _)| key).collect();
        assert_eq!(fields, ROLES);
        // `Theme::color` finds every role and returns that field's color: override each role with a different
        // color (256-color indexes 0-20), then look them up
        let colors: BTreeMap<String, String> = (0..)
            .zip(ROLES)
            .map(|(index, role): (u8, _)| (role.to_string(), index.to_string()))
            .collect();
        let theme = Theme::build("terminal", &colors).expect("terminal");
        let doc = toml_edit::ser::to_document(&theme).expect("serialize");
        for (index, (role, value)) in (0..).zip(doc.iter()) {
            assert_eq!(value.as_str(), Some(index.to_string().as_str()), "{role}");
            assert_eq!(theme.color(role), Some(Color::Indexed(index)), "{role}");
        }
        assert_eq!(
            Theme::catppuccin_mocha().color("today"),
            Some(Color::Rgb(0xa6, 0xe3, 0xa1))
        );
        assert_eq!(Theme::catppuccin_mocha().color("todya"), None);
    }

    #[test]
    fn colors_override_roles_and_must_be_real_roles_and_colors() {
        let colors = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|&(role, color)| (role.to_string(), color.to_string()))
                .collect()
        };
        // Both colors and palette names work; roles not given keep the theme's color
        let theme = Theme::build(
            "catppuccin-mocha",
            &colors(&[("today", "#010203"), ("weekend", "red")]),
        )
        .expect("valid");
        assert_eq!(theme.today, Color::Rgb(1, 2, 3));
        assert_eq!(theme.weekend, Color::Rgb(0xf3, 0x8b, 0xa8));
        assert_eq!(theme.text, Theme::catppuccin_mocha().text);
        // terminal has no palette: red is the terminal's red
        let terminal = Theme::build("terminal", &colors(&[("weekend", "red")])).expect("valid");
        assert_eq!(terminal.weekend, Color::Red);
        // Errors: no such theme, a misspelled role name, not a color; the message says which one
        assert!(Theme::build("solarized", &BTreeMap::new()).is_err());
        let error =
            Theme::build("catppuccin-mocha", &colors(&[("todya", "red")])).expect_err("role");
        assert!(error.contains("todya: no such role"), "{error}");
        let error = Theme::build("terminal", &colors(&[("today", "surface0")])).expect_err("color");
        assert!(
            error.contains("today = \"surface0\": not a color, nor a palette name of terminal"),
            "{error}"
        );
    }

    #[test]
    fn choices_are_the_palette_in_file_order_or_the_16_colors() {
        assert!(choices("no-such-theme").is_empty());
        let mocha = choices("catppuccin-mocha");
        assert_eq!(mocha.len(), 14);
        assert_eq!(mocha[0], ("text".to_string(), Color::Rgb(0xcd, 0xd6, 0xf4)));
        assert_eq!(mocha[13].0, "dim_sapphire");
        let terminal = choices("terminal");
        assert_eq!(terminal.len(), 16);
        assert_eq!(terminal[12], ("LightBlue".to_string(), Color::LightBlue));
    }

    #[test]
    fn color_text_reads_back_as_the_same_color() {
        for color in [
            Color::Rgb(0xcd, 0xd6, 0xf4),
            Color::LightBlue,
            Color::DarkGray,
            Color::Indexed(236),
        ] {
            assert_eq!(color_text(color).parse::<Color>(), Ok(color), "{color:?}");
        }
        assert_eq!(color_text(Color::Rgb(0xcd, 0xd6, 0xf4)), "#cdd6f4");
        assert_eq!(color_text(Color::DarkGray), "DarkGray");
    }

    #[test]
    fn every_builtin_theme_reads() {
        for (name, text) in BUILTIN {
            assert!(Theme::parse(text, &BTreeMap::new()).is_ok(), "{name}");
        }
        assert_eq!(Theme::builtin("no-such-theme"), None);
        // Every theme differs (catches a copy-paste that was not edited)
        let themes: Vec<Theme> = names()
            .iter()
            .filter_map(|name| Theme::builtin(name))
            .collect();
        assert_eq!(themes.len(), BUILTIN.len());
        for (index, theme) in themes.iter().enumerate() {
            for other in &themes[index + 1..] {
                assert_ne!(theme, other);
            }
            // A multi-day bar must not look like the selected cell's background
            assert_ne!(theme.range, theme.cursor_line, "{}", names()[index]);
        }
        assert_eq!(
            Theme::catppuccin_mocha().range,
            Color::Rgb(0x38, 0x51, 0x67)
        );
        // Every palette color is used by some role (no leftovers from roles that are gone)
        for (name, text) in BUILTIN {
            let doc: DocumentMut = text.parse().expect("toml");
            let Some(palette) = doc.get("palette").and_then(|item| item.as_table()) else {
                continue;
            };
            let roles = doc["roles"].as_table().expect("roles");
            for (color, _) in palette {
                assert!(
                    roles.iter().any(|(_, role)| role.as_str() == Some(color)),
                    "{name}: {color} is used by no role"
                );
            }
        }
        // Each theme's palette was copied from the original project; check one role of each against it
        assert_eq!(
            Theme::builtin("gruvbox-dark").map(|theme| theme.today),
            Some(Color::Rgb(0xb8, 0xbb, 0x26))
        );
        assert_eq!(
            Theme::builtin("tokyo-night").map(|theme| theme.focus),
            Some(Color::Rgb(0xbb, 0x9a, 0xf7))
        );
        assert_eq!(
            Theme::builtin("dracula").map(|theme| theme.cursor_line),
            Some(Color::Rgb(0x44, 0x47, 0x5a))
        );
        assert_eq!(
            Theme::builtin("nord").map(|theme| theme.weekend),
            Some(Color::Rgb(0x81, 0xa1, 0xc1))
        );
        assert_eq!(
            Theme::builtin("terminal").map(|theme| theme.text),
            Some(Color::Reset)
        );
    }

    #[test]
    fn roles_take_palette_names_or_colors() {
        // Test by changing one line of mocha: palette name, #rrggbb, terminal color name, 256-color index
        let mocha = BUILTIN[0].1;
        let with = |role: &str| mocha.replace("today = \"green\"", &format!("today = {role}"));
        assert_eq!(
            Theme::parse_text(&with("\"red\"")).map(|theme| theme.today),
            Ok(Color::Rgb(243, 139, 168))
        );
        assert_eq!(
            Theme::parse_text(&with("\"#010203\"")).map(|theme| theme.today),
            Ok(Color::Rgb(1, 2, 3))
        );
        assert_eq!(
            Theme::parse_text(&with("\"light-blue\"")).map(|theme| theme.today),
            Ok(Color::LightBlue)
        );
        assert_eq!(
            Theme::parse_text(&with("\"236\"")).map(|theme| theme.today),
            Ok(Color::Indexed(236))
        );
    }

    #[test]
    fn mistakes_in_a_theme_file_are_errors() {
        let mocha = BUILTIN[0].1;
        for text in [
            // A role missing
            mocha.replace("today = \"green\"\n", ""),
            // A misspelled role name
            mocha.replace("today = ", "todya = "),
            // Neither a color nor a palette name
            mocha.replace("today = \"green\"", "today = \"greeen\""),
            // An unknown table
            format!("{mocha}\n[colours]\n"),
        ] {
            assert!(Theme::parse(&text, &BTreeMap::new()).is_err(), "{text}");
        }
    }
}
