use super::*;
use crate::app::Mode;
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use std::path::PathBuf;

/// Opens the help page (`help`) and draws the whole `width` × `height` screen once (layout 1 underneath,
/// the keyboard on the calendar); returns the box's area and the text of each row in the box
fn draw(lang: Lang, width: u16, height: u16, help: Help) -> (Rect, Vec<String>, Buffer) {
    draw_in(Focus::Calendar, lang, width, height, help)
}

/// Same as `draw`, with the keyboard in `focus`
fn draw_in(
    focus: Focus,
    lang: Lang,
    width: u16,
    height: u16,
    help: Help,
) -> (Rect, Vec<String>, Buffer) {
    let mut app = App::new(lang, date(2026, 10, 17), Vec::new());
    app.focus = focus;
    draw_app(app, width, height, help)
}

/// Draws `app` with the help page (`help`) open on a `width` × `height` screen; returns the same as `draw`
fn draw_app(mut app: App, width: u16, height: u16, help: Help) -> (Rect, Vec<String>, Buffer) {
    app.screen = Rect::new(0, 0, width, height);
    app.mode = Mode::Help(help);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| crate::ui::render(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let area = area(&app, app.screen, help.full);
    (area, crate::ui::tests::area_rows(&buffer, area), buffer)
}

#[test]
fn short_page_puts_two_columns_in_the_middle_of_the_screen() {
    // Expected box computed by a separate Python script: the keyboard is on the calendar, so the
    // calendar's keys on the left, the views and system keys (they work anywhere) on the right, each
    // column running down on its own; key column = longest key (Enter, [ / ], 5 cells) + 3; the right
    // column starts at the left column's widest line + 1; the top border says where the keyboard is
    let (area, rows, buffer) = draw(Lang::ZhTw, 70, 23, Help::default());
    // When centering does not divide evenly, the extra cell goes on the left (ratatui's `Flex::Center`):
    // (70 − 56) / 2 = 7, (23 − 16) / 2: the extra row goes on top
    assert_eq!(area, Rect::new(7, 4, 56, 16));
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 按鍵 ∙ 月曆 ───────────────────────────────╮",
            "│                                                      │",
            "│  月曆                        畫面                    │",
            "│    hjkl    移動                1       月曆 + 事項   │",
            "│    [ / ]   上個月 / 下個月     2       大月曆        │",
            "│    t       今天                3       任務頁        │",
            "│    Enter   那天的事            s       日曆清單      │",
            "│    a       在那天新增          .       隱藏做完的    │",
            "│                                Tab     換框          │",
            "│                                                      │",
            "│                              系統                    │",
            "│                                ?       說明          │",
            "│                                ,       設定          │",
            "│                                q       離開          │",
            "│                                                      │",
            "╰ ?/Tab 全部按鍵 | q/Esc 關閉 ─────────────────────────╯",
        ]
    );
    // Colors: the border is lit (keyboard focus is here); section names, keys and descriptions each have
    // their own color
    let theme = Theme::default();
    assert_eq!(buffer[(7, 4)].fg, theme.focus);
    assert_eq!(buffer[(10, 6)].fg, theme.help_section);
    assert_eq!(buffer[(12, 7)].fg, theme.help_key);
    assert_eq!(buffer[(20, 7)].fg, theme.text);
    // Outside the box is still the screen underneath: the tab bar on top (layout 1's tab lit), layout 1's
    // top-left corner below it
    assert_eq!(buffer[(1, 0)].symbol(), "┃");
    assert_eq!(buffer[(0, 1)].symbol(), "╭");
}

#[test]
fn short_page_lists_the_keys_for_where_the_keyboard_is() {
    // Expected boxes computed by the same separate Python script: the tasks page, the calendar list, the
    // agenda (the day popup shares the agenda's keys)
    let (area, rows, _) = draw_in(Focus::Tasks, Lang::ZhTw, 70, 23, Help::default());
    // (23 − 16) / 2: the extra row goes on top
    assert_eq!(area, Rect::new(5, 4, 60, 16));
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 按鍵 ∙ 任務頁 ─────────────────────────────────╮",
            "│                                                          │",
            "│  任務頁                         畫面                     │",
            "│    j / k    選                    1        月曆 + 事項   │",
            "│    gg / G   第一件 / 最後一件     2        大月曆        │",
            "│    l / h    展開 / 收起           3        任務頁        │",
            "│    Space    打勾                  s        日曆清單      │",
            "│    /        進行中                .        隱藏做完的    │",
            "│    e        編輯                  Tab      換框          │",
            "│    a / A    新增 / 新增子任務                            │",
            "│    r        改期                系統                     │",
            "│    d        刪除                  ?        說明          │",
            "│    u        復原                  ,        設定          │",
            "│    o        編輯器                q        離開          │",
            "│                                                          │",
            "╰ ?/Tab 全部按鍵 | q/Esc 關閉 ─────────────────────────────╯",
        ]
    );
    let (_, rows, _) = draw_in(Focus::Side, Lang::ZhTw, 70, 23, Help::default());
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 按鍵 ∙ 日曆清單 ───────────────────────────────╮",
            "│                                                          │",
            "│  日曆清單                       畫面                     │",
            "│    j / k    選                    1        月曆 + 事項   │",
            "│    gg / G   第一個 / 最後一個     2        大月曆        │",
            "│    [ / ]    上 / 下一個最外層     3        任務頁        │",
            "│    Space    勾選                  s        日曆清單      │",
            "│    Enter    只看這個 / 還原       .        隱藏做完的    │",
            "│    a / A    新增 / 新增子日曆     Tab      換框          │",
            "│    e        改名                                         │",
            "│    d        刪除                系統                     │",
            "│    L / H    往內 / 往外一層       ?        說明          │",
            "│    z        封存 / 取消封存       ,        設定          │",
            "│                                   q        離開          │",
            "│                                                          │",
            "╰ ?/Tab 全部按鍵 | q/Esc 關閉 ─────────────────────────────╯",
        ]
    );
    let (_, rows, _) = draw_in(Focus::Agenda, Lang::ZhTw, 70, 23, Help::default());
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 按鍵 ∙ 事項 ─────────────────────────────────╮",
            "│                                                        │",
            "│  事項                          畫面                    │",
            "│    j / k   選                    1       月曆 + 事項   │",
            "│    Space   打勾                  2       大月曆        │",
            "│    /       進行中                3       任務頁        │",
            "│    e       編輯                  s       日曆清單      │",
            "│    a / A   新增 / 新增子任務     .       隱藏做完的    │",
            "│    r       改期                  Tab     換框          │",
            "│    d       刪除                                        │",
            "│    u       復原                系統                    │",
            "│                                  ?       說明          │",
            "│                                  ,       設定          │",
            "│                                  q       離開          │",
            "│                                                        │",
            "╰ ?/Tab 全部按鍵 | q/Esc 關閉 ───────────────────────────╯",
        ]
    );
    let (_, rows, _) = draw_in(Focus::Day, Lang::ZhTw, 70, 23, Help::default());
    assert!(
        rows[0].starts_with("╭ roh-cal ∙ 按鍵 ∙ 那天的事 "),
        "{}",
        rows[0]
    );
    assert!(rows[2].starts_with("│  事項 "), "{}", rows[2]);
}

#[test]
fn a_narrow_screen_stacks_the_columns() {
    // 44 cells cannot fit two columns (left 27 + 1 + right 23 + border 2 = 53): the right column goes below
    // the left one, with a blank line between
    let (area, rows, _) = draw(Lang::ZhTw, 44, 30, Help::default());
    assert_eq!(area, Rect::new(6, 4, 32, 23));
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 按鍵 ∙ 月曆 ───────╮",
            "│                              │",
            "│  月曆                        │",
            "│    hjkl    移動              │",
            "│    [ / ]   上個月 / 下個月   │",
            "│    t       今天              │",
            "│    Enter   那天的事          │",
            "│    a       在那天新增        │",
            "│                              │",
            "│  畫面                        │",
            "│    1       月曆 + 事項       │",
            "│    2       大月曆            │",
            "│    3       任務頁            │",
            "│    s       日曆清單          │",
            "│    .       隱藏做完的        │",
            "│    Tab     換框              │",
            "│                              │",
            "│  系統                        │",
            "│    ?       說明              │",
            "│    ,       設定              │",
            "│    q       離開              │",
            "│                              │",
            "╰ ?/Tab 全部按鍵 | q/Esc 關閉 ─╯",
        ]
    );
}

/// The app on the calendar in Chinese, with the task file at `tasks` and the config file at `config`
/// (under `/nonexistent`: not there, and not inside anyone's home, so the paths are the same on every machine)
fn app_with_files(tasks: Option<&str>, config: Option<&str>) -> App {
    let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
    app.tasks_path = tasks.map(PathBuf::from);
    app.config_path = config.map(PathBuf::from);
    app
}

#[test]
fn files_are_listed_on_top_of_the_keys() {
    // Expected box computed by a separate Python script: the files block, a blank line, then the same two
    // columns as `short_page_puts_two_columns_in_the_middle_of_the_screen`, and the box just as wide (the
    // keys set the width); label column = 任務檔 (6) + 3, wider than the key column (8); neither file is
    // there, so each says so after its name. A path does not fit the 54 − 3 − 4 − 9 = 38 cells after the
    // label: it wraps after a `/`, the next line under its first character
    let app = app_with_files(
        Some("/nonexistent/roh-cal/tasks.md"),
        Some("/nonexistent/roh-cal/config.toml"),
    );
    let (area, rows, buffer) = draw_app(app, 70, 23, Help::default());
    assert_eq!(area, Rect::new(7, 1, 56, 22));
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 按鍵 ∙ 月曆 ───────────────────────────────╮",
            "│                                                      │",
            "│  檔案                                                │",
            "│    任務檔   /nonexistent/roh-cal/                    │",
            "│             tasks.md（還沒建立）                     │",
            "│    設定檔   /nonexistent/roh-cal/                    │",
            "│             config.toml（還沒建立）                  │",
            "│                                                      │",
            "│  月曆                        畫面                    │",
            "│    hjkl    移動                1       月曆 + 事項   │",
            "│    [ / ]   上個月 / 下個月     2       大月曆        │",
            "│    t       今天                3       任務頁        │",
            "│    Enter   那天的事            s       日曆清單      │",
            "│    a       在那天新增          .       隱藏做完的    │",
            "│                                Tab     換框          │",
            "│                                                      │",
            "│                              系統                    │",
            "│                                ?       說明          │",
            "│                                ,       設定          │",
            "│                                q       離開          │",
            "│                                                      │",
            "╰ ?/Tab 全部按鍵 | q/Esc 關閉 ─────────────────────────╯",
        ]
    );
    // Colors like a section: block name, labels like keys, the path in text color (its next line too), the
    // note muted
    let theme = Theme::default();
    assert_eq!(buffer[(10, 3)].fg, theme.help_section);
    assert_eq!(buffer[(12, 4)].fg, theme.help_key);
    assert_eq!(buffer[(21, 4)].fg, theme.text);
    assert_eq!(buffer[(21, 5)].fg, theme.text);
    assert_eq!(buffer[(28, 5)].fg, theme.text);
    assert_eq!(buffer[(29, 5)].fg, theme.muted);
    // The full version has the same block on top; its keys are wide enough for the whole path
    let app = app_with_files(Some("/nonexistent/roh-cal/tasks.md"), None);
    let full = Help {
        full: true,
        scroll: 0,
    };
    let (_, rows, _) = draw_app(app, 100, 60, full);
    assert!(rows[2].starts_with("│  檔案 "), "{rows:?}");
    assert!(
        rows[3].contains("/nonexistent/roh-cal/tasks.md（還沒建立）"),
        "{rows:?}"
    );
    assert!(rows[5].starts_with("│  月曆 "), "{rows:?}");
}

#[test]
fn a_part_of_a_path_wider_than_the_keys_widens_the_box_just_enough() {
    // Expected box computed by the same separate Python script: 50 cells is too narrow for two columns, so
    // the keys stack as in `a_narrow_screen_stacks_the_columns` (30 cells inside); after the label there is
    // room for 30 − 3 − 13 = 14 cells, but the last part 「tasks.md（還沒建立）」 is 20 (the note stays with
    // the file name), so the lines take 20 and the box widens to 13 + 20 + 3 = 36 inside. No config file
    // path (no HOME): no line for it
    let app = app_with_files(Some("/nonexistent/roh-cal/tasks.md"), None);
    let (area, rows, _) = draw_app(app, 50, 30, Help::default());
    assert_eq!(area, Rect::new(6, 1, 38, 28));
    assert_eq!(
        rows[..8],
        [
            "╭ roh-cal ∙ 按鍵 ∙ 月曆 ─────────────╮",
            "│                                    │",
            "│  檔案                              │",
            "│    任務檔   /nonexistent/          │",
            "│             roh-cal/               │",
            "│             tasks.md（還沒建立）   │",
            "│                                    │",
            "│  月曆                              │",
        ]
    );
}

#[test]
fn wrap_path_breaks_after_a_slash_or_anywhere_in_a_part_too_wide() {
    // Lines put together are the path again
    assert_eq!(
        wrap_path("~/notes/52.51 roh-cal/tasks.md", 22),
        ["~/notes/52.51 roh-cal/", "tasks.md"]
    );
    assert_eq!(
        wrap_path("~/notes/52.51 roh-cal/tasks.md", 30),
        ["~/notes/52.51 roh-cal/tasks.md"]
    );
    // 「52.51 roh-cal/」 is 14 cells, wider than 10: broken inside
    assert_eq!(
        wrap_path("~/52.51 roh-cal/a.md", 10),
        ["~/", "52.51 roh-", "cal/a.md"]
    );
    // Wide characters count 2 cells each
    assert_eq!(wrap_path("~/日曆/任務.md", 7), ["~/日曆/", "任務.md"]);
    assert_eq!(
        wrap_path("~/日曆/任務.md", 6),
        ["~/", "日曆/", "任務.m", "d"]
    );
}

#[test]
fn a_short_screen_scrolls_with_a_bar_on_the_right_border() {
    // 44 × 20: one column of 18 lines; the box is at most 20 tall and holds 16 lines → can scroll 2 lines;
    // the scrollbar is drawn on the right border
    let (area, rows, _) = draw(Lang::ZhTw, 44, 20, Help::default());
    assert_eq!(area, Rect::new(6, 0, 32, 20));
    let bar = |rows: &[String]| -> Vec<usize> {
        rows.iter()
            .enumerate()
            .filter(|(_, row)| row.ends_with('┃'))
            .map(|(y, _)| y)
            .collect()
    };
    // At the top: the first section name is visible and the scrollbar is at the top
    // (starting on the row below the rounded corner)
    assert_eq!(rows[2], "│  月曆                        ┃");
    assert_eq!(bar(&rows).first(), Some(&1));
    // Scrolled to the bottom (overscrolling also stops at the last page): the last key is visible and
    // the scrollbar is at the bottom
    let scrolled = Help {
        full: false,
        scroll: 99,
    };
    let (_, rows, _) = draw(Lang::ZhTw, 44, 20, scrolled);
    assert_eq!(
        rows[17].trim_end_matches(['│', '┃', ' ']),
        "│    q       離開"
    );
    assert_eq!(bar(&rows).last(), Some(&18));
}

#[test]
fn full_page_lists_every_section_side_by_side_without_aligning() {
    for lang in [Lang::En, Lang::ZhTw] {
        let page = lang.texts().help;
        let (area, rows, _) = draw(
            lang,
            100,
            46,
            Help {
                full: true,
                scroll: 0,
            },
        );
        // The longer column (right) is 42 lines + padding 2 + border 2: it just fits, no scrolling
        assert_eq!(area.height, 46, "{lang:?}");
        let text = rows.join("\n");
        for section in page.left.iter().chain(page.right) {
            assert!(text.contains(section.title), "{}", section.title);
        }
        // Side by side: the first section of both columns is on the same row
        let row_of = |text: &str| rows.iter().position(|row| row.contains(text));
        assert_eq!(row_of(page.left[0].title), row_of(page.right[0].title));
        // Section names not aligned: the left column's second section follows right after its first
        // (one blank line between), without waiting for the right column's longer first section
        assert_eq!(
            row_of(page.left[1].title),
            row_of(page.left[0].title).map(|row| row + page.left[0].keys.len() + 2),
            "{lang:?}"
        );
        // The full version leaves only 1 cell on the right: between the right column's widest line
        // (a description) and the right border
        let widest = page
            .right
            .iter()
            .flat_map(|section| section.keys)
            .max_by_key(|(_, action)| Span::raw(*action).width())
            .map(|(_, action)| *action)
            .unwrap();
        let row = &rows[row_of(widest).unwrap()];
        assert_eq!(
            &row[row.find(widest).unwrap() + widest.len()..],
            " │",
            "{lang:?}"
        );
    }
}

#[test]
fn full_page_grays_out_the_sections_that_do_not_work_here() {
    let theme = Theme::default();
    // The color of the piece of text that is exactly `text` (a section name with its indent, a key, a
    // description) on the full page, with the keyboard in `focus`
    let color_of = |focus: Focus, text: &str| {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.focus = focus;
        let (lines, _) = layout(&app, Rect::new(0, 0, 100, 38), true);
        lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.trim_end() == text)
            .and_then(|span| span.style.fg)
            .expect(text)
    };
    // Keyboard on the calendar: the calendar's keys and the ones that work anywhere are lit; the tasks
    // page's and the calendar list's are dimmed, keys and descriptions too
    let calendar = Focus::Calendar;
    assert_eq!(color_of(calendar, "  月曆"), theme.help_section);
    assert_eq!(color_of(calendar, "  畫面"), theme.help_section);
    assert_eq!(color_of(calendar, "  編輯框"), theme.help_section);
    assert_eq!(color_of(calendar, "  任務頁"), theme.other_month);
    assert_eq!(color_of(calendar, "  日曆清單"), theme.other_month);
    assert_eq!(color_of(calendar, "= / -"), theme.other_month);
    assert_eq!(color_of(calendar, "全部展開 / 全部收起"), theme.other_month);
    // On the tasks page: the other way round for the calendar and the tasks page
    assert_eq!(color_of(Focus::Tasks, "  任務頁"), theme.help_section);
    assert_eq!(color_of(Focus::Tasks, "= / -"), theme.help_key);
    assert_eq!(color_of(Focus::Tasks, "  月曆"), theme.other_month);
    assert_eq!(
        color_of(Focus::Tasks, "  事項、跳窗、任務頁"),
        theme.help_section
    );
    // The terminal theme (16 colors) grays out with the terminal's faint text instead
    let app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new()).with_theme("terminal");
    let (lines, _) = layout(&app, Rect::new(0, 0, 100, 38), true);
    let style = lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .find(|span| span.content.trim_end() == "  任務頁")
        .map(|span| span.style)
        .expect("tasks page section");
    assert!(style.add_modifier.contains(Modifier::DIM));
    // The top border says what gray means
    let (_, rows, _) = draw(
        Lang::ZhTw,
        100,
        38,
        Help {
            full: true,
            scroll: 0,
        },
    );
    assert!(
        rows[0].starts_with("╭ roh-cal ∙ 全部按鍵，灰的這裡用不到 "),
        "{}",
        rows[0]
    );
}
