//! Config file: read at startup (location: `paths::config_file`);
//! the settings page (`,`) saves its changes back when it closes
//!
//! Write only what you want to change; anything left out uses the default. A
//! misspelled name or a wrong type is an error (startup prints which line is wrong and
//! does not enter the TUI; a mistake in `[colors]` says which role and which value).
//! Saving changes only the lines that changed; other lines and comments stay word for word (a
//! hand-written config file is not overwritten). See `testdata/config.toml` for an example

use crate::files;
use crate::theme::{self, Theme};
use color_eyre::{
    Result,
    eyre::{WrapErr, bail},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, TableLike};

/// The settings that can change at runtime = the config file's content, with the same structure: `theme` =
/// the top-level `theme = "..."`, `calendar` = `[calendar]`, `month` = `[month]`, `colors` = `[colors]`
///
/// The names in the config file are the field names (e.g. `grid_lines = false` under `[month]`). serde's
/// `default`: fields left out use the `Default` value; `deny_unknown_fields`: unknown names are
/// errors, so a typo is not silently ignored. `Serialize`: turns it back into TOML when saving.
/// Has a `String` (allocates memory), so it is not `Copy`; a copy needs an explicit `.clone()`
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Which built-in theme to use (a name in `theme::BUILTIN`), e.g. `nord`
    pub theme: String,
    /// `[calendar]`: switches shared by the calendars of layouts 1 and 3
    pub calendar: CalendarSettings,
    /// `[month]`: switches for the big month calendar (layout 3)
    pub month: MonthSettings,
    /// `[colors]`: overrides some of the theme's roles (role → color,
    /// e.g. `today = "#a6e3a1"`); roles left out follow the theme.
    /// The settings page writes only the ones that differ from the theme (resetting one removes its line)
    pub colors: BTreeMap<String, String>,
}

impl Settings {
    /// Switches to the theme named `name`. Colors overridden in `[colors]` are first written as the actual
    /// color under the current theme (e.g. palette name `surface0` → `#313244`,
    /// terminal's `cyan` → `Cyan`): the new theme may not have that palette name, or
    /// its color of that name may differ, so after the switch the color stays the same
    pub fn set_theme(&mut self, name: &str) {
        if let Ok(current) = Theme::build(&self.theme, &self.colors) {
            for (role, color) in &mut self.colors {
                if let Some(resolved) = current.color(role) {
                    *color = theme::color_text(resolved);
                }
            }
        }
        self.theme = name.to_string();
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: theme::DEFAULT.to_string(),
            calendar: CalendarSettings::default(),
            month: MonthSettings::default(),
            colors: BTreeMap::new(),
        }
    }
}

/// Switches shared by the calendars (the config file's
/// `[calendar]`); all off by default (`bool` defaults to `false`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarSettings {
    /// Week-number column: the selected week shows its week number, other
    /// weeks show how many weeks they are from it (like vim's relativenumber)
    pub relative_weeks: bool,
}

/// Switches for the big month calendar (the config file's `[month]`); defaults are in `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MonthSettings {
    /// Draws faint grid lines between cells (turning it off gives content 1 more row and 1 more column);
    /// a new year's thick line is drawn either way
    pub grid_lines: bool,
    /// Puts a dot before each item in a cell (`∙ 14:00 Team sync`)
    pub item_icons: bool,
    /// Keeps the selected row fixed in the middle of the 6 rows (like vim's `scrolloff=999`);
    /// when off, it scrolls one row at a time, only when the selection moves out of the 6 rows
    pub center_cursor: bool,
}

impl Default for MonthSettings {
    fn default() -> Self {
        Self {
            grid_lines: true,
            item_icons: true,
            center_cursor: false,
        }
    }
}

/// Reads the config file; a missing file means all defaults; an unreadable file or bad content is an error
pub fn load(path: &Path) -> Result<Settings> {
    let text =
        read(path).wrap_err_with(|| format!("cannot read config file: {}", path.display()))?;
    parse(&text).wrap_err_with(|| format!("invalid config file: {}", path.display()))
}

/// Saves the settings page's changes to the config file: only values that changed from `before` (when the
/// settings page opened) to `after` (now) are written; the rest stays as in the
/// file (one-off values like `--theme`, or changes made elsewhere, are not
/// overwritten); nothing to change means no write; a missing folder is created.
/// If the written file would not load (e.g. the file was broken elsewhere, or the
/// theme and `[colors]` don't match), it is not saved and `InvalidData` is returned;
/// the original content stays so the user can fix it and open it next time
pub fn save(path: &Path, before: &Settings, after: &Settings) -> std::io::Result<()> {
    let invalid =
        |error: color_eyre::Report| std::io::Error::new(ErrorKind::InvalidData, error.to_string());
    let text = read(path)?;
    let new_text = updated(&text, before, after).map_err(invalid)?;
    // Only opened the settings page to look and changed nothing: don't touch the file
    if new_text == text {
        return Ok(());
    }
    parse(&new_text).map_err(invalid)?;
    files::write_atomically(path, &new_text)
}

/// Reads the config file's text; a missing file counts as empty (empty = all defaults)
fn read(path: &Path) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        result => result,
    }
}

/// Turns the config file's content into `Settings`: fields left out use defaults; unknown
/// names and wrong value types are errors (the message points to the line that is wrong);
/// a theme that does not exist, or a `[colors]` role that is misspelled or not a color, is also an error
fn parse(text: &str) -> Result<Settings> {
    let settings: Settings = toml::from_str(text)?;
    if let Err(error) = Theme::build(&settings.theme, &settings.colors) {
        bail!(error);
    }
    Ok(settings)
}

/// The config file's new content: values changed from `before` to `after` are written into
/// `text`. If the line exists, its value is replaced; if not, the line is added at the end
/// of the same table (e.g. `[month]`) (comments at the end of the file stay last);
/// a color reset to the theme's removes the whole line; other
/// lines, comments and whitespace stay exactly as they are
fn updated(text: &str, before: &Settings, after: &Settings) -> Result<String> {
    // Format-preserving TOML (toml_edit): change one value, leave everything else as it is
    let mut doc: DocumentMut = text.parse()?;
    // Turn both the before and after settings into TOML, then compare name by name
    let before = toml_edit::ser::to_document(before)?;
    let after = toml_edit::ser::to_document(after)?;
    merge(doc.as_table_mut(), before.as_table(), after.as_table());
    Ok(doc.to_string())
}

/// Writes into `doc` the values in `wanted` (after) that differ from `current` (before);
/// for a table (e.g. `[month]`), it goes one level in and does the same;
/// keys that `current` has but `wanted` doesn't (e.g. a color reset to the theme's) are removed from `doc`
///
/// `dyn TableLike`: accepts both a table like `[month]` and an inline table like `{ a = 1 }`
fn merge(doc: &mut dyn TableLike, current: &dyn TableLike, wanted: &dyn TableLike) {
    for (key, item) in wanted.iter() {
        // Both sides come from the same serializer, so the same printed text means the same
        // value (a whole table with no changes is skipped too, so no empty `[month]` appears)
        if current.get(key).map(ToString::to_string) == Some(item.to_string()) {
            continue;
        }
        if let Some(wanted_table) = item.as_table_like() {
            // Table: if the file doesn't have it yet, create an empty `[key]` first, then compare inside it
            if doc.get(key).and_then(Item::as_table_like).is_none() {
                doc.insert(key, Item::Table(Table::new()));
            }
            // current and wanted come from the same struct; if
            // wanted is a table here, current is a table here too
            if let (Some(doc_table), Some(current_table)) = (
                doc.get_mut(key).and_then(Item::as_table_like_mut),
                current.get(key).and_then(Item::as_table_like),
            ) {
                merge(doc_table, current_table, wanted_table);
            }
            // The whole table is empty now (e.g. every color reset) → also remove the `[colors]` line
            if wanted_table.is_empty() {
                doc.remove(key);
            }
            continue;
        }
        match (
            doc.get_mut(key).and_then(Item::as_value_mut),
            item.as_value(),
        ) {
            // The file has this line: replace only the value; the end-of-line
            // comment (kept in the old value's decor) moves to the new value
            (Some(old), Some(new)) => {
                let decor = old.decor().clone();
                *old = new.clone();
                *old.decor_mut() = decor;
            }
            // No such line: add it at the end of the same table, before the comments at the end of the file
            _ => {
                doc.insert(key, item.clone());
            }
        }
    }
    // The file has it but the settings to save don't (e.g. `r` resets a color to
    // the theme's) → remove the whole line (the comment above that line goes too)
    let stale: Vec<String> = current
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| wanted.get(key).is_none())
        .collect();
    for key in stale {
        doc.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Computes the new content against the file's current values (before = the file's values) (for tests)
    fn updated_file(text: &str, settings: &Settings) -> Result<String> {
        updated(text, &parse(text)?, settings)
    }

    #[test]
    fn save_writes_only_what_changed_on_the_page() {
        let dir = files::test_dir("config-only-changes");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config-v2.toml");
        // After the app opened, the grid lines were changed elsewhere in the file;
        // this run was opened with `--theme tokyo-night` (the file says gruvbox-dark)
        std::fs::write(
            &path,
            "theme = \"gruvbox-dark\"\n[month]\ngrid_lines = false\n",
        )
        .expect("write");
        let before = Settings {
            theme: "tokyo-night".to_string(),
            ..Settings::default()
        };
        // The settings page only turned on relative weeks: write
        // only that line; theme and grid lines stay as in the file
        let mut after = before.clone();
        after.calendar.relative_weeks = true;
        save(&path, &before, &after).expect("save");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "theme = \"gruvbox-dark\"\n[month]\ngrid_lines = false\n\n[calendar]\nrelative_weeks = true\n"
        );
        // Nothing changed: no write
        save(&path, &after, &after).expect("nothing changed");
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn save_refuses_to_write_a_file_that_would_not_load() {
        let dir = files::test_dir("config-would-break");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config-v2.toml");
        // A gruvbox palette name was added to the file elsewhere; the settings page switches the theme
        // to mocha: mocha has no light1, so the written file would not open next time → no write
        let text = "theme = \"gruvbox-dark\"\n[colors]\ntoday = \"light1\"\n";
        std::fs::write(&path, text).expect("write");
        let before = Settings {
            theme: "gruvbox-dark".to_string(),
            ..Settings::default()
        };
        let after = Settings::default();
        let error = save(&path, &before, &after).expect_err("would not load");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn missing_fields_use_the_defaults() {
        assert_eq!(parse("").expect("empty file"), Settings::default());
        assert_eq!(
            parse("[month]\n").expect("empty table"),
            Settings::default()
        );
        let settings = parse("[month]\ngrid_lines = false").expect("one field");
        assert_eq!(
            settings.month,
            MonthSettings {
                grid_lines: false,
                ..MonthSettings::default()
            }
        );
        assert_eq!(settings.calendar, CalendarSettings::default());
    }

    #[test]
    fn mistakes_are_errors() {
        for text in [
            // Misspelled name (missing an s)
            "[month]\ngrid_line = false",
            // Wrong type: needs true / false, not a string
            "[month]\ngrid_lines = \"no\"",
            // TOML syntax error: no value after the equals sign
            "[month]\ngrid_lines =",
            // The switch must go under [month], not at the top level
            "grid_lines = false",
            // Misspelled table name
            "[mnth]\ngrid_lines = false",
            // v1's name (v1's week_numbers = true meant relative weeks; v2 calls it relative_weeks)
            "[calendar]\nweek_numbers = true",
            // No such theme
            "theme = \"solarized\"",
            // Misspelled role name, not a color, terminal has no catppuccin palette names
            "[colors]\ntodya = \"red\"",
            "[colors]\ntoday = \"redd\"",
            "theme = \"terminal\"\n[colors]\ntoday = \"surface0\"",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn load_reads_the_sample_and_treats_a_missing_file_as_defaults() {
        // Sample file: every switch is set to the opposite of its default
        let settings = load(Path::new("testdata/config.toml")).expect("sample file");
        assert_eq!(
            settings,
            Settings {
                theme: "nord".to_string(),
                calendar: CalendarSettings {
                    relative_weeks: true
                },
                month: MonthSettings {
                    grid_lines: false,
                    item_icons: false,
                    center_cursor: true,
                },
                colors: BTreeMap::from([("weekend".to_string(), "nord11".to_string())]),
            }
        );
        // File does not exist → defaults
        let missing = load(Path::new("testdata/no-such-config.toml")).expect("missing file");
        assert_eq!(missing, Settings::default());
        // Exists but cannot be read (it is a folder) → error, not defaults
        assert!(load(Path::new("testdata")).is_err());
    }

    /// Defaults with only part of `[month]` changed (for tests),
    /// e.g. `with_month(|month| month.grid_lines = false)`
    fn with_month(change: impl FnOnce(&mut MonthSettings)) -> Settings {
        let mut settings = Settings::default();
        change(&mut settings.month);
        settings
    }

    #[test]
    fn updated_changes_only_what_differs_and_keeps_comments() {
        let text = "# My settings\n\n[month]\n# Grid lines\ngrid_lines = true   # too busy\ncenter_cursor = true\n";
        // grid_lines becomes false (the line exists: only the value changes, both comments stay);
        // item_icons becomes false (no such line: added at the end of [month]);
        // center_cursor unchanged (untouched)
        let settings = with_month(|month| {
            month.grid_lines = false;
            month.item_icons = false;
            month.center_cursor = true;
        });
        assert_eq!(
            updated_file(text, &settings).expect("valid file"),
            "# My settings\n\n[month]\n# Grid lines\ngrid_lines = false   # too busy\ncenter_cursor = true\nitem_icons = false\n"
        );
    }

    #[test]
    fn updated_creates_a_table_the_file_does_not_have() {
        let mut settings = Settings::default();
        settings.calendar.relative_weeks = true;
        assert_eq!(
            updated_file("[month]\ngrid_lines = true\n", &settings).expect("valid file"),
            "[month]\ngrid_lines = true\n\n[calendar]\nrelative_weeks = true\n"
        );
        assert_eq!(
            updated_file("", &settings).expect("empty file"),
            "[calendar]\nrelative_weeks = true\n"
        );
    }

    #[test]
    fn updated_adds_new_lines_before_the_comments_at_the_end() {
        // Comments at the end of the file (e.g. settings commented
        // out for now) stay last; the new line goes before them
        let text = "[month]\ngrid_lines = true\n# item_icons = true\n";
        let settings = with_month(|month| month.center_cursor = true);
        assert_eq!(
            updated_file(text, &settings).expect("valid file"),
            "[month]\ngrid_lines = true\ncenter_cursor = true\n# item_icons = true\n"
        );
    }

    #[test]
    fn updated_puts_the_theme_above_the_tables() {
        // theme is written at the top level: TOML requires it before
        // every `[table]`, or it would belong to the last table
        let settings = Settings {
            theme: "nord".to_string(),
            ..Settings::default()
        };
        let text = "[month]\ngrid_lines = true\n";
        let new_text = updated_file(text, &settings).expect("valid file");
        assert_eq!(new_text, "theme = \"nord\"\n[month]\ngrid_lines = true\n");
        assert_eq!(parse(&new_text).expect("reads back"), settings);
    }

    #[test]
    fn updated_writes_changed_colors_and_removes_the_reset_ones() {
        let text = "[month]\ngrid_lines = false\n\n[colors]\n# too faint\nmuted = \"#010203\"\nweekend = \"#f38ba8\"   # red\n";
        // Read in → muted reset (removed), today added, weekend unchanged
        let mut settings = parse(text).expect("valid");
        settings.colors.remove("muted");
        settings
            .colors
            .insert("today".to_string(), "#ffffff".to_string());
        assert_eq!(
            updated_file(text, &settings).expect("valid"),
            "[month]\ngrid_lines = false\n\n[colors]\nweekend = \"#f38ba8\"   # red\ntoday = \"#ffffff\"\n"
        );
    }

    #[test]
    fn updated_removes_the_colors_table_when_every_color_is_reset() {
        let text = "[month]\ngrid_lines = false\n\n[colors]\nweekend = \"#f38ba8\"\n";
        let mut settings = parse(text).expect("valid");
        settings.colors.clear();
        assert_eq!(
            updated_file(text, &settings).expect("valid"),
            "[month]\ngrid_lines = false\n"
        );
        // The file has no [colors] and no color was changed → no empty [colors] appears
        assert_eq!(
            updated_file("[month]\n", &Settings::default()).expect("valid"),
            "[month]\n"
        );
    }

    #[test]
    fn updated_leaves_the_text_alone_when_nothing_changed() {
        // Values in the file that equal the defaults are not removed either (grid_lines defaults to true)
        let text = "# A comment\n[month]\ngrid_lines = true\n";
        assert_eq!(
            updated_file(text, &Settings::default()).expect("valid"),
            text
        );
        // Empty file + all defaults → still empty (defaults are not written in, and no empty table appears)
        assert_eq!(updated_file("", &Settings::default()).expect("empty"), "");
    }

    #[test]
    fn save_creates_the_folder_and_load_reads_it_back() {
        let dir = files::test_dir("config-save");
        let path = dir.join("roh-cal").join("config-v2.toml");
        // File does not exist and all switches are defaults: nothing to change
        // → write nothing, not even the folder
        save(&path, &Settings::default(), &Settings::default()).expect("nothing to save");
        assert!(!dir.exists());
        let settings = with_month(|month| month.center_cursor = true);
        save(&path, &Settings::default(), &settings).expect("save");
        assert_eq!(load(&path).expect("load"), settings);
        // Only the one changed value is written; no temp file is left behind
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[month]\ncenter_cursor = true\n"
        );
        assert!(!dir.join("roh-cal").join("config-v2.toml.tmp").exists());
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn save_refuses_to_touch_a_broken_file() {
        let dir = files::test_dir("config-broken");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config-v2.toml");
        std::fs::write(&path, "[month]\ngrid_line = false\n").expect("write");
        // A file with a misspelled name (broken elsewhere): even when the settings page
        // changed something, it is not saved; the original content stays for the user to fix
        let mut changed = Settings::default();
        changed.month.item_icons = false;
        let error = save(&path, &Settings::default(), &changed).expect_err("broken file");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        // Nothing changed: the file is not touched, and it is not an error
        save(&path, &changed, &changed).expect("nothing changed");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "[month]\ngrid_line = false\n"
        );
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn the_error_names_the_file_and_the_line() {
        let dir = crate::files::test_dir("config-error");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("config.toml");
        std::fs::write(&path, "[month]\n\ngrid_lines = 1\n").expect("write");
        let error = format!("{:?}", load(&path).expect_err("wrong type"));
        assert!(error.contains("invalid config file"), "{error}");
        assert!(error.contains("config.toml"), "{error}");
        // toml's error message points to line 3
        assert!(error.contains("line 3"), "{error}");
        std::fs::remove_dir_all(&dir).expect("clean up");
    }
}
