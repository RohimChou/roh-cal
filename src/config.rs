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
/// the top-level `theme = "..."`, `calendar` = `[calendar]`, `month` = `[month]`,
/// `calendar_list` = `[calendar_list]`, `day_popup` = `[day_popup]`, `colors` = `[colors]`
///
/// The names in the config file are the field names (e.g. `grid_lines = false` under `[month]`). serde's
/// `default`: fields left out use the `Default` value; `deny_unknown_fields`: unknown names are
/// errors, so a typo is not silently ignored. `Serialize`: turns it back into TOML when saving.
/// Has a `String` (allocates memory), so it is not `Copy`; a copy needs an explicit `.clone()`
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Which built-in theme to use (a name in `theme::BUILTIN`), e.g. `nord-bright`
    pub theme: String,
    /// `[calendar]`: switches for the calendars of layouts 1 and 2
    pub calendar: CalendarSettings,
    /// `[month]`: switches for the big month calendar (layout 2)
    pub month: MonthSettings,
    /// `[calendar_list]`: the calendar list on the far left (the side menu)
    pub calendar_list: CalendarListSettings,
    /// `[day_popup]`: the popup with one day's items (layouts 1 and 2)
    pub day_popup: DayPopupSettings,
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
            calendar_list: CalendarListSettings::default(),
            day_popup: DayPopupSettings::default(),
            colors: BTreeMap::new(),
        }
    }
}

/// Switches for the calendars of layouts 1 and 2 (the config file's `[calendar]`); defaults are in
/// `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarSettings {
    /// Week-number column: the selected week shows its week number, other
    /// weeks show how many weeks they are from it (like vim's relativenumber)
    pub relative_weeks: bool,
    /// Layout 1's calendar: a dot right of each date for each of that day's items; when off, that cell is
    /// blank (the calendar keeps its width)
    pub dots: bool,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        Self {
            relative_weeks: false,
            dots: true,
        }
    }
}

/// Switches for the big month calendar (the config file's `[month]`); defaults are in `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MonthSettings {
    /// Draws faint grid lines between cells (turning it off gives content more room; a month's edge is
    /// still drawn)
    pub grid_lines: bool,
    /// Keeps the selected row fixed in the middle of the 6 rows (like vim's `scrolloff=999`);
    /// when off, it scrolls one row at a time, only when the selection moves out of the 6 rows
    pub center_cursor: bool,
}

impl Default for MonthSettings {
    fn default() -> Self {
        Self {
            grid_lines: true,
            center_cursor: false,
        }
    }
}

/// The calendar list (the config file's `[calendar_list]`); defaults are in `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarListSettings {
    /// The widest the list gets, in cells (incl. its border): it follows the longest calendar name up to
    /// this, and longer names get cut off. Only `MIN_WIDTH` to `MAX_WIDTH` counts (see `max_width()`)
    pub max_width: u16,
    /// Hide done tasks (and their subtasks) everywhere; switched at the bottom of the calendar list too
    pub hide_done: bool,
}

impl CalendarListSettings {
    /// The narrowest the list gets: its header always fits
    pub const MIN_WIDTH: u16 = 22;
    /// The largest `max_width` that counts: wider would squeeze the calendar next to it
    pub const MAX_WIDTH: u16 = 60;

    /// `max_width` kept between `MIN_WIDTH` and `MAX_WIDTH` (the config file may say anything)
    pub fn max_width(self) -> u16 {
        self.max_width.clamp(Self::MIN_WIDTH, Self::MAX_WIDTH)
    }
}

impl Default for CalendarListSettings {
    fn default() -> Self {
        Self {
            max_width: 38,
            hide_done: false,
        }
    }
}

/// The day popup (the config file's `[day_popup]`); defaults are in `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DayPopupSettings {
    /// Lists the items under their outermost calendar's name, the calendars in the calendar list's order;
    /// when off, one list with each item's outermost calendar at its right end
    pub by_calendar: bool,
}

impl Default for DayPopupSettings {
    fn default() -> Self {
        Self { by_calendar: true }
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
mod tests;
