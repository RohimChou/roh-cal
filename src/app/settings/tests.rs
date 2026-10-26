use super::*;
use crate::config::{CalendarSettings, DayPopupSettings, MonthSettings};
use crate::test_utils::press;
use crate::texts::Lang;
use crate::theme::Theme;
use jiff::civil::date;
use ratatui::layout::Rect;

/// An app with the settings page open (terminal 100 × 30)
fn app_with_settings() -> App {
    let mut app =
        App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new()).with_theme("catppuccin-mocha");
    app.screen = Rect::new(0, 0, 100, 30);
    press(&mut app, KeyCode::Char(','));
    app
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
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('j'));
    }
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
    // Row 9 is text's color: l switches to the palette's next color
    // (subtext0), records it in [colors], and the screen changes right away
    for _ in 0..8 {
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
    // h again: wraps around to the palette's last color (mid_blue); r resets
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(
        app.settings.colors.get("text").map(String::as_str),
        Some("#495a80")
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
    // The config file has a gruvbox palette name: r goes back to the default (nord-bright), the overridden color
    // is first written as the color itself, so the theme builds (and the saved file opens next time)
    let mut app = app_with_settings();
    app.settings.theme = "gruvbox-dark".to_string();
    app.settings
        .colors
        .insert("today".to_string(), "light1".to_string());
    app.apply_theme();
    press(&mut app, KeyCode::Char('r'));
    assert_eq!(app.settings.theme, "nord-bright");
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
fn colors_a_view_does_not_draw_are_for_the_other_views() {
    // Lit on `view`: it is used everywhere, or on that view
    let on_view = |row: Row, view: View| {
        row.places()
            .is_none_or(|places| places.contains(&Place::View(view)))
    };
    // The tasks page draws no week numbers, year line, selected brackets, grid lines or bars; layout 1
    // draws no grid lines or bars, and only layout 1 the selected brackets (the month grid gives the
    // selected day a background) and the overlap underline; the rest (text, today and day-off colors in
    // the date column…) are everywhere
    for role in ["year_line", "week_number", "week_number_dim"] {
        let row = Row::Color(role);
        assert!(!on_view(row, View::Tasks), "{role}");
        assert!(
            on_view(row, View::Planner) && on_view(row, View::Month),
            "{role}"
        );
    }
    for role in ["grid", "range"] {
        let row = Row::Color(role);
        assert!(
            !on_view(row, View::Tasks) && !on_view(row, View::Planner),
            "{role}"
        );
        assert!(on_view(row, View::Month), "{role}");
    }
    // Only layout 1 has the selected brackets and underlines overlapping multi-day items
    for role in ["selected", "range_overlap"] {
        let row = Row::Color(role);
        assert!(on_view(row, View::Planner), "{role}");
        assert!(
            !on_view(row, View::Month) && !on_view(row, View::Tasks),
            "{role}"
        );
    }
    for role in ["text", "today", "weekend", "selection", "priority_high"] {
        assert_eq!(Row::Color(role).places(), None, "{role}");
    }
    // Only the day popup's text can be dragged over
    assert_eq!(
        Row::Color("text_selection").places(),
        Some(&[Place::DayPopup][..])
    );
}

#[test]
fn rows_about_the_calendar_list_or_the_day_popup_are_lit_only_while_it_is_on_screen() {
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    let lit = |app: &App| {
        [Row::ListWidth, Row::ByCalendar, Row::Color("checked")].map(|row| row.applies_here(app))
    };
    // 130 wide: the calendar list is on the left; the day popup is closed
    app.screen = Rect::new(0, 0, 130, 30);
    assert_eq!(lit(&app), [true, false, true]);
    // Hidden with `s`: neither is on screen
    app.side_hidden = true;
    assert_eq!(lit(&app), [false, false, false]);
    // The day popup open
    app.view = View::Month;
    app.focus = super::super::Focus::Day;
    assert_eq!(lit(&app), [false, true, true]);
    // 100 wide (no room for the list) with it called up over the left by `s`
    app.screen = Rect::new(0, 0, 100, 30);
    app.focus = super::super::Focus::Side;
    assert_eq!(lit(&app), [true, false, true]);
    // A row for a view: only on that view
    assert!(Row::GridLines.applies_here(&app));
    app.view = View::Tasks;
    assert!(!Row::GridLines.applies_here(&app));
}

#[test]
fn every_switch_has_a_row() {
    // Destructure `Settings` into every field: if a setting is added and
    // not added here, it won't compile, a reminder to add a row in `rows()`
    let Settings {
        theme: _,
        colors: _,
        calendar: CalendarSettings {
            relative_weeks: _,
            dots: _,
        },
        month: MonthSettings {
            grid_lines: _,
            center_cursor: _,
        },
        calendar_list:
            CalendarListSettings {
                width: _,
                hide_done: _,
            },
        day_popup: DayPopupSettings { by_calendar: _ },
    } = Settings::default();
    assert_eq!(rows().len(), 8 + theme::ROLES.len());
}

#[test]
fn the_list_width_steps_by_one_cell_and_stops_at_the_ends() {
    let mut settings = Settings::default();
    let theme = Theme::catppuccin_mocha();
    // Auto by default; on its own, auto counts as 38: l = 39, h twice = 37, fixed from then on
    assert_eq!(Row::ListWidth.value(&settings, &theme), "auto");
    Row::ListWidth.change(&mut settings, 1);
    assert_eq!(settings.calendar_list.width, Some(39));
    assert_eq!(Row::ListWidth.value(&settings, &theme), "39");
    Row::ListWidth.change(&mut settings, -1);
    Row::ListWidth.change(&mut settings, -1);
    assert_eq!(settings.calendar_list.width, Some(37));
    // Stops at 22 and 60 instead of wrapping around
    settings.calendar_list.width = Some(22);
    Row::ListWidth.change(&mut settings, -1);
    assert_eq!(settings.calendar_list.width, Some(22));
    settings.calendar_list.width = Some(60);
    Row::ListWidth.change(&mut settings, 1);
    assert_eq!(settings.calendar_list.width, Some(60));
    // A hand-written 100 in the config file counts as 60: shown as 60, h goes to 59
    settings.calendar_list.width = Some(100);
    assert_eq!(Row::ListWidth.value(&settings, &theme), "60");
    Row::ListWidth.change(&mut settings, -1);
    assert_eq!(settings.calendar_list.width, Some(59));
    // r: back to auto; the other setting in `[calendar_list]` stays
    settings.calendar_list.hide_done = true;
    Row::ListWidth.reset(&mut settings);
    assert_eq!(settings.calendar_list.width, None);
    assert!(settings.calendar_list.hide_done);
}

#[test]
fn hl_on_the_auto_width_go_from_the_width_on_screen() {
    // 「 ☑ Tokyo trip 🗼🍣 1 」 makes the auto width 23 (see `ui::side`)
    let items = crate::tasks::parse("# Tokyo trip 🗼🍣\n- b @2026-12-24\n").items;
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), items).with_theme("catppuccin-mocha");
    app.screen = Rect::new(0, 0, 130, 30);
    press(&mut app, KeyCode::Char(','));
    let list_width = rows().iter().position(|&row| row == Row::ListWidth);
    while app.settings_page().map(|page| page.row) != list_width {
        press(&mut app, KeyCode::Char('j'));
    }
    // l: 24 (not 39); r: auto again; h: 22
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.settings.calendar_list.width, Some(24));
    press(&mut app, KeyCode::Char('r'));
    assert_eq!(app.settings.calendar_list.width, None);
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(app.settings.calendar_list.width, Some(22));
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
            Row::ListWidth => row.value(&changed, &Theme::catppuccin_mocha()),
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
    // In `theme::BUILTIN` order: gruvbox-dark → tokyo-night → dracula → nord-bright → one-dark → rose-pine →
    // terminal → back to mocha
    for expected in [
        "tokyo-night",
        "dracula",
        "nord-bright",
        "one-dark",
        "rose-pine",
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
    // j to dots: l turns it off (on by default)
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('l'));
    assert!(!app.settings.calendar.dots);
    // j to grid_lines, Space turns it off; k back to the first row; it does not go above the top
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char(' '));
    assert!(!app.settings.month.grid_lines);
    for _ in 0..4 {
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
    assert_eq!(app.settings_page().map(|page| page.row), Some(3));
    assert!(app.settings.calendar.relative_weeks);
    // R then y: resets everything, and the screen colors go back to the default theme
    press(&mut app, KeyCode::Char('R'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(app.settings, Settings::default());
    assert_eq!(app.theme, Theme::default());
    assert!(app.settings_page().is_some());
    // Resetting everything plays the intro again on the next start: its marker is deleted (only by R then y;
    // a cancelled R leaves it)
    let dir = crate::files::test_dir("reset-intro");
    let marker = dir.join("splash-seen");
    crate::files::write_atomically(&marker, "").expect("write the marker");
    app.splash_marker = Some(marker.clone());
    press(&mut app, KeyCode::Char('R'));
    press(&mut app, KeyCode::Char('n'));
    assert!(marker.exists());
    press(&mut app, KeyCode::Char('R'));
    press(&mut app, KeyCode::Char('y'));
    assert!(!marker.exists());
    // Already gone: resetting again is fine
    press(&mut app, KeyCode::Char('R'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(app.settings, Settings::default());
    std::fs::remove_dir_all(&dir).expect("clean up");
    // The theme row can also be reset alone with r
    for _ in 0..3 {
        press(&mut app, KeyCode::Char('k'));
    }
    press(&mut app, KeyCode::Char('h'));
    press(&mut app, KeyCode::Char('r'));
    assert_eq!(app.settings.theme, "nord-bright");
    assert_eq!(app.theme, Theme::default());
}

#[test]
fn clicking_chooses_a_row_then_changes_it_and_clicking_outside_closes() {
    // 100 × 30: the box (28 tall) starts at row 1; grid_lines is list line 5 (theme, [calendar],
    // relative_weeks, dots, [month], grid_lines): 1 + border 1 + 5 = 7
    let mut app = app_with_settings();
    let click = |app: &mut App, column, row| {
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    };
    click(&mut app, 50, 7);
    assert_eq!(app.settings_page().map(|page| page.row), Some(3));
    assert!(app.settings.month.grid_lines);
    // Click the chosen row again: change the value
    click(&mut app, 50, 7);
    assert!(!app.settings.month.grid_lines);
    // Wheel: choose the next row
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 50,
        row: 7,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(app.settings_page().map(|page| page.row), Some(4));
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
