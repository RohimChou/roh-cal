use super::*;
use crate::app::{Mode, Row};
use crate::texts::Lang;
use crate::theme::Theme;
use jiff::civil::date;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::{Buffer, Cell},
    style::Color,
};

// Where things are, counted from the box's top-left corner: list line L is on row 1 + L (below the
// border); names of table rows start at cell 5 (border + indent 4), the top-level theme and the
// section names at cell 3 (border + indent 2), values at cell 23 (border + indent 2 + name column 20);
// the last rows of the box are the blank line, the description, the line below it and the bottom
// border
const NAME: u16 = 5;
const TOP_NAME: u16 = 3;
const VALUE: u16 = 23;

/// The row (from the box's top) of the settings row `row` (`[colors]` showing its first colors)
fn y_of(row: Row) -> u16 {
    let index = rows().iter().position(|&each| each == row).expect("row");
    1 + u16::try_from(line_of(index, 0).expect("line")).expect("fits")
}

/// The index of `row` in `rows()`
fn index_of(row: Row) -> usize {
    rows().iter().position(|&each| each == row).expect("row")
}

/// The cell `dx` right and `dy` down from the box's top-left corner
fn cell(buffer: &Buffer, area: Rect, dx: u16, dy: u16) -> &Cell {
    &buffer[(area.x + dx, area.y + dy)]
}

/// Opens the settings page (`page`) and draws a 70 × `height` screen once; returns the app (with
/// `settings_page.top` already scrolled), the box's area,
/// the text of each row in the box (the scrollbar `┃` on the right border turned back into `│`),
/// and the whole screen
fn draw(page: SettingsPage, height: u16) -> (App, Rect, Vec<String>, Buffer) {
    draw_app(
        App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new()),
        page,
        height,
    )
}

/// Same as `draw`, but with the given `app` (e.g. with the theme already changed)
fn draw_app(mut app: App, page: SettingsPage, height: u16) -> (App, Rect, Vec<String>, Buffer) {
    app.screen = Rect::new(0, 0, 70, height);
    app.mode = Mode::Settings {
        page,
        opened: app.settings.clone(),
        picker: None,
    };
    let mut terminal = Terminal::new(TestBackend::new(70, height)).unwrap();
    terminal
        .draw(|frame| crate::ui::render(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let area = area(&app, app.screen);
    let rows = crate::ui::tests::area_rows(&buffer, area)
        .iter()
        .map(|row| row.replace('┃', "│"))
        .collect();
    (app, area, rows, buffer)
}

/// The row a page is opened on, choosing `row`
fn choosing(row: usize) -> SettingsPage {
    SettingsPage {
        row,
        ..SettingsPage::default()
    }
}

#[test]
fn settings_list_theme_switches_and_colors_like_the_config_file() {
    // Expected box computed by a separate Python script (each role's palette name taken from
    // [roles] in themes/catppuccin-mocha.toml):
    // name column = longest name (week_number_dim, 15 cells) + 3; the top-level theme has 2 less
    // indent and a name column 2 wider, so all values share one column;
    // the inside is as wide as a row (indent 4 + name column 18 + the longest value, catppuccin-mocha,
    // 16; tokyo-night's terminal_black with its swatch, 17) + 2 blank cells, 41 cells: the descriptions
    // and the key hints on the bottom border fit in it;
    // no blank lines; `[colors]` shows its first 10 colors (13 more below): 25 rows tall
    let (_, area, rows, buffer) = draw(SettingsPage::default(), 44);
    assert_eq!(area, Rect::new(14, 10, 43, 25));
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 設定 ─────────────────────────╮",
            "│  theme               catppuccin-mocha   │",
            "│  [calendar]                             │",
            "│    relative_weeks    off                │",
            "│  [month]                                │",
            "│    grid_lines        on                 │",
            "│    center_cursor     off                │",
            "│  [calendar_list]                        │",
            "│    max_width         36                 │",
            "│    hide_done         off                │",
            "│  [colors]                               │",
            "│    text                 text            │",
            "│    muted                subtext0        │",
            "│    line                 surface2        │",
            "│    year_line            lavender        │",
            "│    label                lavender        │",
            "│    week_number          yellow          │",
            "│    week_number_dim      dim_yellow      │",
            "│    weekend              blue            │",
            "│    today                green           │",
            "│    selected             red             │",
            "│                                         │",
            "│  顏色：l 換下一套 theme，h 換上一套     │",
            "│                                         │",
            "╰ jk 選  hl 改  r 還原  R 全部  Esc 關閉 ─╯",
        ]
    );
    // The chosen row (the first row, theme) has a background from the left to the right inside edge
    // and a bold value; other rows have no background
    let theme = Theme::catppuccin_mocha();
    let at = |dx, dy| cell(&buffer, area, dx, dy);
    let right = area.width - 2;
    assert_eq!(at(1, y_of(Row::Theme)).bg, theme.selection);
    assert_eq!(at(right, y_of(Row::Theme)).bg, theme.selection);
    assert!(
        at(VALUE, y_of(Row::Theme))
            .modifier
            .contains(Modifier::BOLD)
    );
    assert_eq!(at(VALUE, y_of(Row::RelativeWeeks)).symbol(), "o");
    assert_eq!(at(1, y_of(Row::RelativeWeeks)).bg, Color::Reset);
    assert_eq!(at(TOP_NAME, y_of(Row::Theme)).fg, theme.help_key);
    assert_eq!(
        at(TOP_NAME, y_of(Row::RelativeWeeks) - 1).fg,
        theme.help_section
    );
    // The width row's value is in the same column
    assert_eq!(at(VALUE, y_of(Row::ListWidth)).symbol(), "3");
    // Swatch: on the today row, the 2 cells of the value column have today's color, followed by the
    // name
    let today = y_of(Row::Color("today"));
    assert_eq!(at(VALUE, today).bg, theme.today);
    assert_eq!(at(VALUE + 1, today).bg, theme.today);
    assert_eq!(at(VALUE + 2, today).bg, Color::Reset);
}

#[test]
fn the_chosen_color_row_keeps_its_swatch() {
    // Choosing the text row: the whole row has a background, but the swatch keeps the text color
    let text = Row::Color("text");
    let (_, area, rows, buffer) = draw(choosing(index_of(text)), 44);
    let theme = Theme::catppuccin_mocha();
    let at = |dx, dy| cell(&buffer, area, dx, dy);
    assert_eq!(at(2, y_of(text)).bg, theme.selection);
    assert_eq!(at(VALUE, y_of(text)).bg, theme.text);
    assert_eq!(at(VALUE + 3, y_of(text)).bg, theme.selection);
    assert_eq!(
        rows[rows.len() - 3],
        "│  顏色：l 換下一個，e 自訂，r 還原       │"
    );
}

#[test]
fn a_reset_color_gets_no_swatch() {
    // The terminal theme's text is the terminal's default text color (Reset): the 2 swatch cells
    // look empty, followed by Reset
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.settings.set_theme("terminal");
    app.apply_theme();
    let (_, area, rows, buffer) = draw_app(app, SettingsPage::default(), 44);
    let at = |dx, dy| cell(&buffer, area, dx, dy);
    let text = y_of(Row::Color("text"));
    assert_eq!(
        rows[usize::from(text)],
        "│    text                 Reset           │"
    );
    assert_eq!(at(VALUE, text).bg, Color::Reset);
    assert_eq!(at(VALUE + 1, text).bg, Color::Reset);
    // Other roles still have a swatch (muted = dark-gray)
    assert_eq!(at(VALUE, y_of(Row::Color("muted"))).bg, Color::DarkGray);
}

#[test]
fn a_short_screen_scrolls_to_the_chosen_row() {
    // 70 × 16: the box is at most 16 tall and the list holds 11 lines (16 − border 2 − the 3 lines
    // below the list), so it scrolls; choosing the last row (help_key) → `[colors]` shows its last 10
    // colors and the list is scrolled to the end (its 11 last lines), and the description is still at
    // the bottom
    let last = rows().len() - 1;
    let (app, area, rows, buffer) = draw(choosing(last), 16);
    assert_eq!(
        app.settings_page().map(|page| (page.top, page.colors_top)),
        Some((entries(0).len() - 11, theme::ROLES.len() - COLOR_ROWS))
    );
    assert_eq!(area, Rect::new(14, 0, 43, 16));
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 設定 ─────────────────────────╮",
            "│  [colors]                               │",
            "│    notice               yellow          │",
            "│    error                red             │",
            "│    grid                 surface0        │",
            "│    other_month          faint           │",
            "│    range                dim_sapphire    │",
            "│    priority_high        red             │",
            "│    priority_medium      peach           │",
            "│    priority_low         yellow          │",
            "│    help_section         blue            │",
            "│    help_key             peach           │",
            "│                                         │",
            "│  顏色：l 換下一個，e 自訂，r 還原       │",
            "│                                         │",
            "╰ jk 選  hl 改  r 還原  R 全部  Esc 關閉 ─╯",
        ]
    );
    // The scrollbar is on the bottom part of the right border
    assert_eq!(cell(&buffer, area, 42, 14).symbol(), "┃");
    assert_eq!(cell(&buffer, area, 42, 1).symbol(), "│");
    // Clicking the bottom list row: still help_key; the top one is `[colors]` (not a row), the one
    // below it notice, the first color shown: mapped using both scroll positions
    assert_eq!(row_at(&app, app.screen, 20, 11), Some(last));
    assert_eq!(row_at(&app, app.screen, 20, 1), None);
    assert_eq!(
        row_at(&app, app.screen, 20, 2),
        Some(index_of(Row::Color("notice")))
    );
}

#[test]
fn colors_scroll_inside_their_section() {
    // Expected rows computed by the same Python script as above. 70 × 44 holds the whole box, so the
    // list itself does not scroll; choosing the 12th color (focus) moves `[colors]` down so it shows one
    // more color below it: the 4th to the 13th color (year_line … focus, then checked)
    let focus = index_of(Row::Color("focus"));
    let (app, area, rows, buffer) = draw(choosing(focus), 44);
    assert_eq!(
        app.settings_page().map(|page| (page.top, page.colors_top)),
        Some((0, 3))
    );
    assert_eq!(rows[11], "│    year_line            lavender        │");
    assert_eq!(rows[20], "│    checked              mauve           │");
    // The scrollbar beside the colors (x 42, rows 11-20): its thumb moved down from the top, and the
    // rows above `[colors]` keep a plain border
    let border = |dy: u16| cell(&buffer, area, 42, dy).symbol().to_string();
    assert_eq!(border(11), "│");
    assert!((12..=20).any(|dy| border(dy) == "┃"));
    assert_eq!(border(5), "│");
    // Back up to text: `[colors]` scrolls back to its first color; a row above `[colors]` keeps it
    let page = SettingsPage {
        row: index_of(Row::Color("text")),
        colors_top: 3,
        ..SettingsPage::default()
    };
    let (app, _, _, _) = draw_app(app, page, 44);
    assert_eq!(app.settings_page().map(|page| page.colors_top), Some(0));
    let mut page = choosing(index_of(Row::Theme));
    page.colors_top = 5;
    let (app, _, _, _) = draw(page, 44);
    assert_eq!(app.settings_page().map(|page| page.colors_top), Some(5));
}

#[test]
fn r_asks_on_the_bottom_border() {
    let (_, _, rows, _) = draw(
        SettingsPage {
            row: index_of(Row::CenterCursor),
            confirm_reset: true,
            ..SettingsPage::default()
        },
        44,
    );
    assert_eq!(
        rows[rows.len() - 3],
        "│  大月曆：選取的那週固定在正中間         │"
    );
    assert_eq!(
        rows[rows.len() - 1],
        "╰ 全部還原成預設？y 還原，其他鍵取消 ─────╯"
    );
}

#[test]
fn clicks_map_to_the_rows() {
    let (app, area, _, _) = draw(SettingsPage::default(), 44);
    let click = |dx: u16, dy: u16| row_at(&app, app.screen, area.x + dx, area.y + dy);
    // Any cell of a row inside the box, the left and right ends too; the last row shown is the 10th
    // color (`[colors]` shows 10 at a time)
    let last = rows()[colors().0 + COLOR_ROWS - 1];
    assert_eq!(click(8, y_of(Row::Theme)), Some(0));
    assert_eq!(click(8, y_of(Row::RelativeWeeks)), Some(1));
    assert_eq!(click(1, y_of(Row::GridLines)), Some(2));
    assert_eq!(
        click(8, y_of(Row::ListWidth)),
        Some(index_of(Row::ListWidth))
    );
    assert_eq!(click(area.width - 2, y_of(last)), Some(index_of(last)));
    // Section names, the blank line, the description, the line below it and the border do not count
    let height = area.height;
    for (dx, dy) in [
        (8, height - 2),
        (8, y_of(Row::Theme) + 1),
        (8, y_of(Row::RelativeWeeks) - 1),
        (8, y_of(Row::ListWidth) - 1),
        (8, y_of(Row::Color("text")) - 1),
        (8, height - 4),
        (8, height - 3),
        (0, y_of(Row::Theme)),
        (area.width - 1, y_of(Row::Theme)),
        (8, 0),
    ] {
        assert_eq!(click(dx, dy), None, "({dx}, {dy})");
    }
}

#[test]
fn settings_that_do_nothing_here_are_dimmed_and_say_where_they_apply() {
    // Layout 1: the month grid's grid_lines and center_cursor are dimmed (the other-month gray,
    // darker than muted text), relative_weeks is not; choosing grid_lines says below the
    // description which view it is for
    let theme = Theme::catppuccin_mocha();
    let grid_lines = index_of(Row::GridLines);
    let (_, area, rows, buffer) = draw(choosing(grid_lines), 44);
    let at = |dx, dy| cell(&buffer, area, dx, dy);
    assert_eq!(at(NAME, y_of(Row::GridLines)).fg, theme.other_month);
    assert_eq!(at(VALUE, y_of(Row::GridLines)).fg, theme.other_month);
    assert_eq!(at(NAME, y_of(Row::CenterCursor)).fg, theme.other_month);
    assert_eq!(at(NAME, y_of(Row::RelativeWeeks)).fg, theme.help_key);
    let below = rows.len() - 2;
    assert_eq!(
        rows[below - 1].trim_end_matches([' ', '│']),
        "│  大月曆：格子之間畫淡淡的格線"
    );
    assert_eq!(
        rows[below].trim_end_matches([' ', '│']),
        "│  這個畫面用不到：2 大月曆"
    );
    assert_eq!(at(TOP_NAME, area.height - 2).fg, theme.notice);
    // Layout 2: the same row is a normal one, nothing below the description
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.view = crate::app::View::Month;
    let (_, area, rows, buffer) = draw_app(app, choosing(grid_lines), 44);
    assert_eq!(
        cell(&buffer, area, NAME, y_of(Row::GridLines)).fg,
        theme.help_key
    );
    assert_eq!(rows[rows.len() - 2].trim_end_matches([' ', '│']), "");
    // Tasks page: relative_weeks is for layouts 1 and 2; theme is for every view
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.view = crate::app::View::Tasks;
    let (_, area, rows, buffer) = draw_app(app, choosing(index_of(Row::RelativeWeeks)), 44);
    let at = |dx, dy| cell(&buffer, area, dx, dy);
    assert_eq!(at(NAME, y_of(Row::RelativeWeeks)).fg, theme.other_month);
    assert_eq!(at(TOP_NAME, y_of(Row::Theme)).fg, theme.help_key);
    assert_eq!(
        rows[rows.len() - 2].trim_end_matches([' ', '│']),
        "│  這個畫面用不到：1 月曆、2 大月曆"
    );
}
