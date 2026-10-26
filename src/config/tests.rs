use super::*;

/// Computes the new content against the file's current values (before = the file's values) (for tests)
fn updated_file(text: &str, settings: &Settings) -> Result<String> {
    updated(text, &parse(text)?, settings)
}

#[test]
fn save_writes_only_what_changed_on_the_page() {
    let dir = files::test_dir("config-only-changes");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("config.toml");
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
    let path = dir.join("config.toml");
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
        // Names from v1, which v2 does not have
        "[calendar]\nweek_numbers = true",
        "[month]\nitem_icons = false",
        // The width must be a whole number from 0 up, not text or negative (left out = auto)
        "[calendar_list]\nwidth = \"wide\"",
        "[calendar_list]\nwidth = -1",
        // The width's old name
        "[calendar_list]\nmax_width = 38",
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
    // Sample file: every setting differs from its default (switches are the opposite)
    let settings = load(Path::new("testdata/config.toml")).expect("sample file");
    assert_eq!(
        settings,
        Settings {
            theme: "gruvbox-dark".to_string(),
            calendar: CalendarSettings {
                relative_weeks: true,
                dots: false,
            },
            month: MonthSettings {
                grid_lines: false,
                center_cursor: true,
            },
            calendar_list: CalendarListSettings {
                width: Some(30),
                hide_done: true,
            },
            day_popup: DayPopupSettings { by_calendar: false },
            colors: BTreeMap::from([("weekend".to_string(), "red".to_string())]),
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
    let text = "# My settings\n\n[calendar]\nrelative_weeks = true\n\n[month]\n# Grid lines\ngrid_lines = true   # too busy\n";
    // grid_lines becomes false (the line exists: only the value changes, both comments stay);
    // center_cursor becomes true (no such line: added at the end of [month]);
    // relative_weeks unchanged (untouched)
    let settings = Settings {
        calendar: CalendarSettings {
            relative_weeks: true,
            ..CalendarSettings::default()
        },
        ..with_month(|month| {
            month.grid_lines = false;
            month.center_cursor = true;
        })
    };
    assert_eq!(
        updated_file(text, &settings).expect("valid file"),
        "# My settings\n\n[calendar]\nrelative_weeks = true\n\n[month]\n# Grid lines\ngrid_lines = false   # too busy\ncenter_cursor = true\n"
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
    let text = "[month]\ngrid_lines = true\n# more later\n";
    let settings = with_month(|month| month.center_cursor = true);
    assert_eq!(
        updated_file(text, &settings).expect("valid file"),
        "[month]\ngrid_lines = true\ncenter_cursor = true\n# more later\n"
    );
}

#[test]
fn updated_puts_the_theme_above_the_tables() {
    // theme is written at the top level: TOML requires it before
    // every `[table]`, or it would belong to the last table
    let settings = Settings {
        theme: "dracula".to_string(),
        ..Settings::default()
    };
    let text = "[month]\ngrid_lines = true\n";
    let new_text = updated_file(text, &settings).expect("valid file");
    assert_eq!(
        new_text,
        "theme = \"dracula\"\n[month]\ngrid_lines = true\n"
    );
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
fn updated_writes_a_fixed_width_and_removes_it_for_auto() {
    // Fixed at 30: a line under [calendar_list]
    let text = "[calendar_list]\nhide_done = true\n";
    let mut settings = parse(text).expect("valid");
    settings.calendar_list.width = Some(30);
    let fixed = updated_file(text, &settings).expect("valid");
    assert_eq!(fixed, "[calendar_list]\nhide_done = true\nwidth = 30\n");
    // Back to auto: the line goes (TOML has no null)
    settings.calendar_list.width = None;
    assert_eq!(updated_file(&fixed, &settings).expect("valid"), text);
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
    let path = dir.join("roh-cal").join("config.toml");
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
    assert!(!dir.join("roh-cal").join("config.toml.tmp").exists());
    std::fs::remove_dir_all(&dir).expect("clean up");
}

#[test]
fn save_refuses_to_touch_a_broken_file() {
    let dir = files::test_dir("config-broken");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[month]\ngrid_line = false\n").expect("write");
    // A file with a misspelled name (broken elsewhere): even when the settings page
    // changed something, it is not saved; the original content stays for the user to fix
    let mut changed = Settings::default();
    changed.month.center_cursor = true;
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
