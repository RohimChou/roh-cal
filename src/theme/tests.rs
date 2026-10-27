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
    let error = Theme::build("catppuccin-mocha", &colors(&[("todya", "red")])).expect_err("role");
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
    assert_eq!(mocha.len(), 18);
    assert_eq!(mocha[0], ("text".to_string(), Color::Rgb(0xcd, 0xd6, 0xf4)));
    assert_eq!(mocha[16].0, "mid_blue");
    assert_eq!(
        mocha[17],
        ("base".to_string(), Color::Rgb(0x1e, 0x1e, 0x2e))
    );
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
        assert_ne!(theme.range, theme.selection, "{}", names()[index]);
        // Bars stacked on lines next to each other take turns between two different backgrounds
        assert_ne!(theme.range_alt, theme.range, "{}", names()[index]);
        assert_ne!(theme.range_alt, theme.selection, "{}", names()[index]);
    }
    assert_eq!(
        Theme::catppuccin_mocha().range,
        Color::Rgb(0x38, 0x51, 0x67)
    );
    // Every other line of bars: sapphire and the background mixed 4:6
    assert_eq!(
        Theme::catppuccin_mocha().range_alt,
        Color::Rgb(0x40, 0x62, 0x7a)
    );
    // The selection background: mauve and the background mixed 3:7
    assert_eq!(
        Theme::catppuccin_mocha().selection,
        Color::Rgb(0x52, 0x47, 0x6a)
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
        Theme::builtin("dracula").map(|theme| theme.grid),
        Some(Color::Rgb(0x44, 0x47, 0x5a))
    );
    assert_eq!(
        Theme::builtin("nord-bright").map(|theme| theme.weekend),
        Some(Color::Rgb(0x8c, 0xb0, 0xd4))
    );
    assert_eq!(
        Theme::builtin("one-dark").map(|theme| theme.focus),
        Some(Color::Rgb(0xc6, 0x78, 0xdd))
    );
    assert_eq!(
        Theme::builtin("rose-pine").map(|theme| theme.today),
        Some(Color::Rgb(0xeb, 0xbc, 0xba))
    );
    assert_eq!(
        Theme::builtin("terminal").map(|theme| theme.text),
        Some(Color::Reset)
    );
    // High priority (!!!) is a red vivid enough to stand out among the other colors: nord-bright's red
    // with much more color; its yellow a touch softer, so it does not outshine that red; the orange (!!)
    // in between
    assert_eq!(
        Theme::builtin("nord-bright").map(|theme| theme.priority_high),
        Some(Color::Rgb(0xf6, 0x5e, 0x6c))
    );
    assert_eq!(
        Theme::builtin("nord-bright").map(|theme| theme.priority_medium),
        Some(Color::Rgb(0xf5, 0x96, 0x78))
    );
    // Its green (today) leans toward a leaf green
    assert_eq!(
        Theme::builtin("nord-bright").map(|theme| theme.today),
        Some(Color::Rgb(0xa2, 0xd3, 0x86))
    );
    assert_eq!(
        Theme::builtin("nord-bright").map(|theme| theme.priority_low),
        Some(Color::Rgb(0xf4, 0xd3, 0x92))
    );
    // Dimmed text (what does not work here): text and the background half and half, e.g. nord6 #eceff4
    // and nord0 #2e3440 → #8d929a; fainter than muted (secondary) text in every theme with RGB colors
    assert_eq!(
        Theme::builtin("nord-bright").map(|theme| theme.other_month),
        Some(Color::Rgb(0x8d, 0x92, 0x9a))
    );
    let brightness = |color: Color| match color {
        Color::Rgb(r, g, b) => Some(u32::from(r) + u32::from(g) + u32::from(b)),
        _ => None,
    };
    for theme in &themes {
        if let (Some(dim), Some(muted)) = (brightness(theme.other_month), brightness(theme.muted)) {
            assert!(dim < muted, "{theme:?}");
        }
    }
    // nord-bright's year line: nord8 #94d1e3 and the background nord0 #2e3440 mixed 55:45 → #668a9a, fainter
    // than the month names (label, nord8 itself)
    let bright = Theme::builtin("nord-bright").expect("built in");
    assert_eq!(bright.year_line, Color::Rgb(0x66, 0x8a, 0x9a));
    assert!(brightness(bright.year_line) < brightness(bright.label));
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

#[test]
fn marked_text_shows_on_the_chosen_row_in_every_theme() {
    // Text dragged over in the day popup is mostly on the chosen item's row: its background must differ
    // from that row's
    for name in names() {
        let theme = Theme::builtin(name).expect("built-in");
        assert_ne!(theme.text_selection, theme.selection, "{name}");
    }
}

#[test]
fn mixing_moves_part_of_the_way_and_only_mixes_rgb() {
    let (black, white) = (Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255));
    assert_eq!(mix(black, white, 0.0), black);
    assert_eq!(mix(black, white, 1.0), white);
    assert_eq!(mix(black, white, 0.25), Color::Rgb(64, 64, 64));
    assert_eq!(mix(black, white, 0.5), Color::Rgb(128, 128, 128));
    // Past either end: stays at that end (not further on toward 255 or 0)
    let (dark, light) = (Color::Rgb(100, 100, 100), Color::Rgb(200, 200, 200));
    assert_eq!(mix(dark, light, 2.0), light);
    assert_eq!(mix(dark, light, -1.0), dark);
    // A terminal color or `reset` on either side cannot be mixed: the first color before halfway, the second
    // from halfway on
    assert_eq!(mix(black, Color::Reset, 0.25), black);
    assert_eq!(mix(black, Color::Reset, 0.5), Color::Reset);
    assert_eq!(mix(Color::Blue, white, 0.4), Color::Blue);
    assert_eq!(mix(Color::Blue, white, 0.6), white);
}

#[test]
fn the_tab_bar_colors_come_from_focus_selection_and_background() {
    // nord-bright (computed apart): focus #c69bbe, selection #5c5366, background nord0 #2e3440. The bar is
    // the focus color 35% of the way to the selection color; the shown tab's background is the selection
    // color a quarter of the way to the background
    let theme = Theme::default();
    assert_eq!(theme.background, Color::Rgb(0x2e, 0x34, 0x40));
    assert_eq!(theme.tab_bar(), Color::Rgb(161, 130, 159));
    assert_eq!(theme.tab_background(), Color::Rgb(81, 75, 93));
    // Every built-in theme names its background, except the terminal one, which cannot know it (`reset`):
    // its tab background is plain selection
    for name in names().into_iter().filter(|&name| name != "terminal") {
        let theme = Theme::builtin(name).expect(name);
        assert!(matches!(theme.background, Color::Rgb(..)), "{name}");
    }
    let terminal = Theme::builtin("terminal").expect("terminal");
    assert_eq!(terminal.background, Color::Reset);
    assert_eq!(terminal.tab_background(), terminal.selection);
}
