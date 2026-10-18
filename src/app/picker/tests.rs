use super::*;
use crate::app::Row;
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::layout::Rect;

/// Presses `code` (no modifier)
fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::from(code));
}

/// An app with the settings page open on the `role` color row (theme catppuccin-mocha)
fn on_color_row(role: &'static str) -> App {
    let mut app =
        App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new()).with_theme("catppuccin-mocha");
    app.screen = Rect::new(0, 0, 80, 30);
    press(&mut app, KeyCode::Char(','));
    let row = crate::app::rows()
        .iter()
        .position(|&row| row == Row::Color(role))
        .expect("role");
    for _ in 0..row {
        press(&mut app, KeyCode::Char('j'));
    }
    app
}

#[test]
fn e_opens_the_picker_on_a_color_row_only() {
    // today is mocha's green #a6e3a1 = 166, 227, 161
    let mut app = on_color_row("today");
    press(&mut app, KeyCode::Char('e'));
    let picker = app.picker().expect("open");
    assert_eq!(
        (picker.role, picker.rgb, picker.channel),
        ("today", [166, 227, 161], 0)
    );
    // Not on the theme row
    let mut app =
        App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new()).with_theme("catppuccin-mocha");
    press(&mut app, KeyCode::Char(','));
    press(&mut app, KeyCode::Char('e'));
    assert_eq!(app.picker(), None);
}

#[test]
fn keys_change_the_color_and_the_screen_follows() {
    let mut app = on_color_row("today");
    press(&mut app, KeyCode::Char('e'));
    // l +1 red, j to green, L +16, H −16, h −1 twice; clamped at 255
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('L'));
    assert_eq!(app.picker().map(|picker| picker.rgb), Some([167, 243, 161]));
    press(&mut app, KeyCode::Char('L'));
    assert_eq!(app.picker().map(|picker| picker.rgb), Some([167, 255, 161]));
    press(&mut app, KeyCode::Char('H'));
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(app.picker().map(|picker| picker.rgb), Some([167, 238, 161]));
    // The role already has it, in `[colors]` and on screen
    assert_eq!(
        app.settings.colors.get("today").map(String::as_str),
        Some("#a7eea1")
    );
    assert_eq!(app.theme.today, Color::Rgb(167, 238, 161));
    // Enter keeps it and closes the picker (the settings page stays)
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.picker(), None);
    assert!(app.settings_page().is_some());
    assert_eq!(app.theme.today, Color::Rgb(167, 238, 161));
}

#[test]
fn typing_six_hex_digits_sets_the_color() {
    let mut app = on_color_row("weekend");
    press(&mut app, KeyCode::Char('e'));
    // 5 digits: nothing yet; Backspace takes one back; a 6th sets it (upper case works too)
    for digit in "12345".chars() {
        press(&mut app, KeyCode::Char(digit));
    }
    assert_eq!(
        app.picker().map(|picker| picker.typed.as_str()),
        Some("12345")
    );
    assert_eq!(app.settings.colors.get("weekend"), None);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Char('b'));
    assert_eq!(
        app.picker().map(|picker| picker.rgb),
        Some([0x12, 0x34, 0xab])
    );
    assert_eq!(app.theme.weekend, Color::Rgb(0x12, 0x34, 0xab));
    // Typing mocha's own blue back: no override left in `[colors]`
    for digit in "89b4fa".chars() {
        press(&mut app, KeyCode::Char(digit));
    }
    assert_eq!(app.settings.colors.get("weekend"), None);
}

#[test]
fn esc_puts_back_the_color_from_before() {
    // An override from before (#010203) comes back; with none, the theme's own color does
    let mut app = on_color_row("today");
    app.settings.colors.insert("today".into(), "#010203".into());
    app.apply_theme();
    press(&mut app, KeyCode::Char('e'));
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.theme.today, Color::Rgb(2, 2, 3));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.picker(), None);
    assert_eq!(app.theme.today, Color::Rgb(1, 2, 3));
    let mut app = on_color_row("today");
    press(&mut app, KeyCode::Char('e'));
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Char('q'));
    assert_eq!(app.settings.colors.get("today"), None);
    assert_eq!(app.theme.today, Theme::catppuccin_mocha().today);
    // q closed only the picker, not the settings page
    assert!(app.settings_page().is_some());
}

#[test]
fn the_mouse_sets_a_channel_and_a_click_outside_puts_the_color_back() {
    let mut app = on_color_row("today");
    press(&mut app, KeyCode::Char('e'));
    let area = ui::picker_area(&app);
    let mouse = |app: &mut App, kind, column, row| {
        app.handle_mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    };
    let click = MouseEventKind::Down(MouseButton::Left);
    // The left end of green's bar (the bars start 13 cells in, green is the 6th row): green 0, chosen
    mouse(&mut app, click, area.x + 13, area.y + 5);
    let picker = app.picker().expect("open");
    assert_eq!((picker.rgb, picker.channel), ([166, 0, 161], 1));
    assert_eq!(app.theme.today, Color::Rgb(166, 0, 161));
    // The wheel: up +1, down −1 on the chosen channel
    mouse(&mut app, MouseEventKind::ScrollUp, area.x + 13, area.y + 5);
    mouse(&mut app, MouseEventKind::ScrollUp, area.x + 13, area.y + 5);
    mouse(
        &mut app,
        MouseEventKind::ScrollDown,
        area.x + 13,
        area.y + 5,
    );
    assert_eq!(app.picker().map(|picker| picker.rgb), Some([166, 1, 161]));
    // Outside the box: the color from before, the picker closed, the settings page still open
    mouse(&mut app, click, area.x, area.bottom());
    assert_eq!(app.picker(), None);
    assert_eq!(app.theme.today, Theme::catppuccin_mocha().today);
    assert!(app.settings_page().is_some());
}

#[test]
fn terminal_colors_start_from_their_usual_values() {
    assert_eq!(rgb(Color::Rgb(1, 2, 3)), [1, 2, 3]);
    assert_eq!(rgb(Color::Blue), [0, 0, 238]);
    assert_eq!(rgb(Color::DarkGray), [127, 127, 127]);
    assert_eq!(rgb(Color::Indexed(9)), [255, 0, 0]);
    // 256 colors: the cube (196 = 5, 0, 0) and the grays (244 = 8 + 12 × 10)
    assert_eq!(rgb(Color::Indexed(196)), [255, 0, 0]);
    assert_eq!(rgb(Color::Indexed(16)), [0, 0, 0]);
    assert_eq!(rgb(Color::Indexed(110)), [135, 175, 215]);
    assert_eq!(rgb(Color::Indexed(244)), [128, 128, 128]);
    assert_eq!(rgb(Color::Reset), [128, 128, 128]);
}
