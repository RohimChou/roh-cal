//! Color picker: `e` on a color row of the settings page opens it over the settings page, to make any color
//! for that role (not only the theme's palette)
//!
//! `j` / `k` choose red, green or blue; `h` / `l` change it by 1, `H` / `L` by 16; typing 6 hex digits
//! (`a6e3a1`) sets the whole color. The screen follows every change; `Enter` keeps the color, `Esc` / `q`
//! puts back the one from before. Mouse: clicking a bar sets that channel, the wheel changes the chosen
//! channel by 1, clicking outside the box = `Esc`

use super::{App, Mode};
use crate::theme::{self, Theme};
use crate::ui;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    layout::Position,
    style::Color,
};

/// The color picker's state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    /// Which role it makes a color for (a name in `theme::ROLES`)
    pub role: &'static str,
    /// The color being made: red, green, blue
    pub rgb: [u8; 3],
    /// The chosen channel: 0 red, 1 green, 2 blue
    pub channel: usize,
    /// Hex digits typed so far (fewer than 6; the 6th sets the color and empties this)
    pub typed: String,
    /// The role's `[colors]` entry when the picker opened (`None` = the theme's own color): `Esc` puts it
    /// back
    pub before: Option<String>,
}

impl Picker {
    /// A picker for `role`, starting from the color it has now in `theme`
    fn new(role: &'static str, theme: &Theme, before: Option<String>) -> Self {
        Self {
            role,
            rgb: theme.color(role).map_or([128, 128, 128], rgb),
            channel: 0,
            typed: String::new(),
            before,
        }
    }

    /// The color as the config file writes it, e.g. `#a6e3a1`
    pub fn hex(&self) -> String {
        let [red, green, blue] = self.rgb;
        theme::color_text(Color::Rgb(red, green, blue))
    }

    /// Changes the chosen channel by `delta`, stopping at 0 and 255
    fn adjust(&mut self, delta: i16) {
        let value = &mut self.rgb[self.channel];
        *value = u8::try_from((i16::from(*value) + delta).clamp(0, 255)).unwrap_or(*value);
        self.typed.clear();
    }

    /// Takes one typed character: a hex digit is added; the 6th sets the whole color. Returns whether the
    /// color changed
    fn type_digit(&mut self, digit: char) -> bool {
        self.typed.push(digit.to_ascii_lowercase());
        if self.typed.len() < 6 {
            return false;
        }
        let typed = std::mem::take(&mut self.typed);
        match format!("#{typed}").parse::<Color>() {
            Ok(Color::Rgb(red, green, blue)) => {
                self.rgb = [red, green, blue];
                true
            }
            _ => false,
        }
    }
}

/// The red, green, blue of a color: as it is for `#rrggbb`; the usual xterm values for the terminal's 16
/// colors and the 256-color numbers; mid gray for the terminal's default color (`Reset`), which has none
fn rgb(color: Color) -> [u8; 3] {
    // xterm's 16 colors, in `Color::Indexed` order (0 black … 15 white)
    const ANSI: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 0, 0],
        [0, 205, 0],
        [205, 205, 0],
        [0, 0, 238],
        [205, 0, 205],
        [0, 205, 205],
        [229, 229, 229],
        [127, 127, 127],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [92, 92, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    let index = match color {
        Color::Rgb(red, green, blue) => return [red, green, blue],
        Color::Indexed(index) => index,
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        Color::Reset => return [128, 128, 128],
    };
    match index {
        0..16 => ANSI[usize::from(index)],
        // 16-231: a 6 × 6 × 6 color cube
        16..232 => {
            let level = |step: u8| if step == 0 { 0 } else { 55 + step * 40 };
            let cube = index - 16;
            [level(cube / 36), level(cube / 6 % 6), level(cube % 6)]
        }
        // 232-255: 24 grays
        _ => {
            let gray = 8 + (index - 232) * 10;
            [gray, gray, gray]
        }
    }
}

impl App {
    /// The open color picker, if any
    pub fn picker(&self) -> Option<&Picker> {
        match &self.mode {
            Mode::Settings { picker, .. } => picker.as_ref(),
            _ => None,
        }
    }

    /// `e` on a color row of the settings page: opens the picker for `role`
    pub(super) fn open_picker(&mut self, role: &'static str) {
        let before = self.settings.colors.get(role).cloned();
        let new = Picker::new(role, &self.theme, before);
        if let Mode::Settings { picker, .. } = &mut self.mode {
            *picker = Some(new);
        }
    }

    /// Changes the open picker with `change` (which returns whether the color changed); if it did, the
    /// role gets the new color right away (written as `[colors]` only when it differs from the theme's)
    fn change_picker(&mut self, change: impl FnOnce(&mut Picker) -> bool) {
        let Mode::Settings {
            picker: Some(picker),
            ..
        } = &mut self.mode
        else {
            return;
        };
        if !change(picker) {
            return;
        }
        let (role, hex) = (picker.role, picker.hex());
        let base = Theme::builtin(&self.settings.theme).and_then(|theme| theme.color(role));
        if hex.parse::<Color>().ok() == base {
            self.settings.colors.remove(role);
        } else {
            self.settings.colors.insert(role.to_string(), hex);
        }
        self.apply_theme();
    }

    /// Closes the picker: `keep` = the color stays; otherwise the role's color from before comes back
    fn close_picker(&mut self, keep: bool) {
        let Mode::Settings { picker, .. } = &mut self.mode else {
            return;
        };
        let Some(closed) = picker.take() else {
            return;
        };
        if !keep {
            match closed.before {
                Some(color) => self.settings.colors.insert(closed.role.to_string(), color),
                None => self.settings.colors.remove(closed.role),
            };
            self.apply_theme();
        }
    }

    /// Keys while the picker is open (see this module's top comment); other keys are ignored
    pub(super) fn handle_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            // Quit from the picker: the color from before comes back, then the settings page saves and
            // closes
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.close_picker(false);
                self.close_settings();
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => self.change_picker(|picker| {
                picker.channel = (picker.channel + 1).min(2);
                picker.typed.clear();
                false
            }),
            KeyCode::Char('k') | KeyCode::Up => self.change_picker(|picker| {
                picker.channel = picker.channel.saturating_sub(1);
                picker.typed.clear();
                false
            }),
            KeyCode::Char('h') | KeyCode::Left => self.change_picker(|picker| {
                picker.adjust(-1);
                true
            }),
            KeyCode::Char('l') | KeyCode::Right => self.change_picker(|picker| {
                picker.adjust(1);
                true
            }),
            KeyCode::Char('H') => self.change_picker(|picker| {
                picker.adjust(-16);
                true
            }),
            KeyCode::Char('L') => self.change_picker(|picker| {
                picker.adjust(16);
                true
            }),
            KeyCode::Char(digit) if digit.is_ascii_hexdigit() => {
                self.change_picker(|picker| picker.type_digit(digit));
            }
            KeyCode::Backspace => self.change_picker(|picker| {
                picker.typed.pop();
                false
            }),
            KeyCode::Enter => self.close_picker(true),
            KeyCode::Esc | KeyCode::Char('q') => self.close_picker(false),
            _ => {}
        }
    }

    /// Mouse while the picker is open: clicking a channel's bar sets that channel (and chooses it), the
    /// wheel changes the chosen channel by 1, clicking outside the box puts the color from before back
    pub(super) fn handle_picker_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.change_picker(|picker| {
                picker.adjust(1);
                true
            }),
            MouseEventKind::ScrollDown => self.change_picker(|picker| {
                picker.adjust(-1);
                true
            }),
            MouseEventKind::Down(MouseButton::Left) => {
                let position = Position::new(mouse.column, mouse.row);
                if !ui::picker_area(self).contains(position) {
                    self.close_picker(false);
                    return;
                }
                if let Some((channel, value)) = ui::picker_at(self, mouse.column, mouse.row) {
                    self.change_picker(|picker| {
                        picker.channel = channel;
                        picker.typed.clear();
                        match value {
                            Some(value) => {
                                picker.rgb[channel] = value;
                                true
                            }
                            None => false,
                        }
                    });
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
