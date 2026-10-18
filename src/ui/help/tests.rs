use super::*;
use crate::app::Mode;
use crate::texts::Lang;
use jiff::civil::date;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

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
    // (70 − 56) / 2 = 7, (23 − 15) / 2 = 4
    assert_eq!(area, Rect::new(7, 4, 56, 15));
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
            "│    a       在那天新增          Tab     換框          │",
            "│                                                      │",
            "│                              系統                    │",
            "│                                ?       說明          │",
            "│                                ,       設定          │",
            "│                                q       離開          │",
            "│                                                      │",
            "╰ ?/Tab 全部按鍵  q/Esc 關閉 ──────────────────────────╯",
        ]
    );
    // Colors: the border is lit (keyboard focus is here); section names, keys and descriptions each have
    // their own color
    let theme = Theme::catppuccin_mocha();
    assert_eq!(buffer[(7, 4)].fg, theme.focus);
    assert_eq!(buffer[(10, 6)].fg, theme.help_section);
    assert_eq!(buffer[(12, 7)].fg, theme.help_key);
    assert_eq!(buffer[(20, 7)].fg, theme.text);
    // Outside the box is still the calendar underneath (layout 1's top-left corner)
    assert_eq!(buffer[(0, 0)].symbol(), "╭");
}

#[test]
fn short_page_lists_the_keys_for_where_the_keyboard_is() {
    // Expected boxes computed by the same separate Python script: the tasks page, the calendar list, the
    // agenda (the day popup shares the agenda's keys)
    let (area, rows, _) = draw_in(Focus::Tasks, Lang::ZhTw, 70, 23, Help::default());
    assert_eq!(area, Rect::new(6, 4, 58, 15));
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 按鍵 ∙ 任務頁 ───────────────────────────────╮",
            "│                                                        │",
            "│  任務頁                        畫面                    │",
            "│    j / k   選                    1       月曆 + 事項   │",
            "│    l / h   展開 / 收起           2       大月曆        │",
            "│    Space   打勾                  3       任務頁        │",
            "│    e       編輯                  s       日曆清單      │",
            "│    a / A   新增 / 新增子任務     Tab     換框          │",
            "│    r       改期                                        │",
            "│    d       刪除                系統                    │",
            "│    u       復原                  ?       說明          │",
            "│                                  ,       設定          │",
            "│                                  q       離開          │",
            "│                                                        │",
            "╰ ?/Tab 全部按鍵  q/Esc 關閉 ────────────────────────────╯",
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
            "│    Space    勾選                  3        任務頁        │",
            "│    a        新增日曆              s        日曆清單      │",
            "│    e        改名                  Tab      換框          │",
            "│    d        刪除                                         │",
            "│    L / H    往內 / 往外一層     系統                     │",
            "│                                   ?        說明          │",
            "│                                   ,        設定          │",
            "│                                   q        離開          │",
            "│                                                          │",
            "╰ ?/Tab 全部按鍵  q/Esc 關閉 ──────────────────────────────╯",
        ]
    );
    let (_, rows, _) = draw_in(Focus::Agenda, Lang::ZhTw, 70, 23, Help::default());
    assert_eq!(
        rows,
        [
            "╭ roh-cal ∙ 按鍵 ∙ 事項 ──────────────────────────────────╮",
            "│                                                         │",
            "│  事項                           畫面                    │",
            "│    j / k   選                     1       月曆 + 事項   │",
            "│    h / l   左右換欄（分欄時）     2       大月曆        │",
            "│    Space   打勾                   3       任務頁        │",
            "│    e       編輯                   s       日曆清單      │",
            "│    a / A   新增 / 新增子任務      Tab     換框          │",
            "│    r       改期                                         │",
            "│    d       刪除                 系統                    │",
            "│    u       復原                   ?       說明          │",
            "│                                   ,       設定          │",
            "│                                   q       離開          │",
            "│                                                         │",
            "╰ ?/Tab 全部按鍵  q/Esc 關閉 ─────────────────────────────╯",
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
    assert_eq!(area, Rect::new(6, 4, 32, 22));
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
            "│    Tab     換框              │",
            "│                              │",
            "│  系統                        │",
            "│    ?       說明              │",
            "│    ,       設定              │",
            "│    q       離開              │",
            "│                              │",
            "╰ ?/Tab 全部按鍵  q/Esc 關閉 ──╯",
        ]
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
            38,
            Help {
                full: true,
                scroll: 0,
            },
        );
        // The longer column (right) is 34 lines + padding 2 + border 2: it just fits, no scrolling
        assert_eq!(area.height, 38, "{lang:?}");
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
    let theme = Theme::catppuccin_mocha();
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
    assert_eq!(color_of(calendar, "+ / -"), theme.other_month);
    assert_eq!(color_of(calendar, "全部展開 / 全部收起"), theme.other_month);
    // On the tasks page: the other way round for the calendar and the tasks page
    assert_eq!(color_of(Focus::Tasks, "  任務頁"), theme.help_section);
    assert_eq!(color_of(Focus::Tasks, "+ / -"), theme.help_key);
    assert_eq!(color_of(Focus::Tasks, "  月曆"), theme.other_month);
    assert_eq!(
        color_of(Focus::Tasks, "  事項、跳窗、任務頁"),
        theme.help_section
    );
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
