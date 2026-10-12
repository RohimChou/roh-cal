//! 說明頁：按 `?` 打開，疊在畫面正中間（排法照 tuxedo）；打開時是精簡版，`Tab` 換完整版
//!
//! 左右兩欄列按鍵；畫面不夠寬放兩欄時，改成一欄由上往下排。內容在 `Texts::help`

use crate::app::App;
use crate::i18n::{HelpPage, HelpSection};
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Padding, Paragraph},
};

/// 按鍵和它的說明之間至少空幾格（按鍵那欄的寬度 = 兩欄裡最長的按鍵 + 這麼多格）
const KEY_GAP: usize = 3;
/// 兩欄之間至少空幾格（右欄自己還有縮排：段落名 2 格、按鍵 4 格，所以 1 格就分得開）
const GAP: usize = 1;
/// 精簡版的內容和右框線之間空幾格：跟兩欄之間看起來一樣寬（`GAP` + 段落名的縮排 2），左、中、右的留白才平均；
/// 完整版只留 1 格
const SHORT_RIGHT_MARGIN: usize = GAP + 2;
/// 框線和內容之間：上下各空 1 行（左邊靠段落名的縮排留空）
const PADDING: Padding = Padding::vertical(1);

/// 畫說明頁：大小剛好包住內容，放在整個 terminal 的正中間（說明頁不屬於哪一天，不用對齊月曆框）；
/// terminal 太矮時下面的會被切掉
pub fn render(frame: &mut Frame, app: &App, full: bool) {
    let theme = &app.theme;
    let (lines, area) = layout(app, frame.area(), full);
    let (title, hint) = border_lines(page(app, full), theme);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .padding(PADDING)
        .title_top(title)
        .title_bottom(hint);
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// 說明頁的框（含框線）在畫面上的範圍（點框外 = 關掉）
pub fn bounds(app: &App, full: bool) -> Rect {
    layout(app, app.screen, full).1
}

/// 現在顯示的那一版：精簡版，或 `full` 時的完整版
fn page(app: &App, full: bool) -> HelpPage {
    if full {
        app.texts.help.full
    } else {
        app.texts.help.short
    }
}

/// 說明頁的內容（一行一行）和框放在哪（`render` 和 `bounds` 共用）
fn layout(app: &App, screen: Rect, full: bool) -> (Vec<Line<'static>>, Rect) {
    let help = page(app, full);
    // 精簡版的段落名水平對齊；完整版段落多、長短差很多，兩欄各自往下排（對齊會多出很多空行）
    let lines = arrange(help.left, help.right, &app.theme, screen.width, !full);

    // 大小：最寬那行 + 右邊的留白 + 框線 2（也要放得下上下框線的字，不然會被切掉）；
    // 行數 + 上下 padding + 框線 2（超過畫面會被縮到畫面大小）
    let (title, hint) = border_lines(help, &app.theme);
    let right_margin = if full { 1 } else { SHORT_RIGHT_MARGIN };
    let inside = (width(&lines) + right_margin)
        .max(title.width())
        .max(hint.width());
    let box_width = u16::try_from(inside + 2).unwrap_or(u16::MAX);
    let box_height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(PADDING.top + PADDING.bottom + 2);
    let area = screen.centered(
        Constraint::Length(box_width),
        Constraint::Length(box_height),
    );
    (lines, area)
}

/// 上下框線的字：上框線 `roh-cal · keys`（app 名稱醒目，後面淡淡的）、下框線是操作說明（`render` 和 `layout` 共用）
fn border_lines(help: HelpPage, theme: &Theme) -> (Line<'static>, Line<'static>) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("roh-cal", theme.title),
        Span::styled(format!(" · {} ", help.title), theme.hint),
    ]);
    let hint = Line::styled(format!(" {} ", help.hint), theme.hint);
    (title, hint)
}

/// 兩欄的段落排成一行一行：寬度（扣掉框線）放得下就左右並排（`align` 時同一個位置的兩段段落名在同一行），
/// 放不下就改成一欄，右欄接在左欄下面
fn arrange(
    left: &[HelpSection],
    right: &[HelpSection],
    theme: &Theme,
    screen_width: u16,
    align: bool,
) -> Vec<Line<'static>> {
    // 按鍵那欄兩欄一樣寬：兩欄所有按鍵裡最長的 + `KEY_GAP`
    let key_width = left
        .iter()
        .chain(right)
        .flat_map(|section| section.keys)
        .map(|&(key, _)| Span::raw(key).width())
        .max()
        .unwrap_or(0)
        + KEY_GAP;
    let left = section_blocks(left, key_width, theme);
    let right = section_blocks(right, key_width, theme);
    let block_width = |blocks: &[Vec<Line>]| blocks.iter().map(|block| width(block)).max();
    // 右欄從 `split` 那格開始
    let split = block_width(&left).unwrap_or(0) + GAP;
    if split + block_width(&right).unwrap_or(0) + 2 <= usize::from(screen_width) {
        let (left, right) = if align {
            aligned(left, right)
        } else {
            (stacked(left), stacked(right))
        };
        side_by_side(left, right, split)
    } else {
        // 一欄：右欄接在左欄下面，中間空一行
        let mut lines = stacked(left);
        lines.push(Line::default());
        lines.extend(stacked(right));
        lines
    }
}

/// 一欄的每一段排成幾行：段落名（縮排 2）、每個按鍵一行（縮排 4，按鍵那欄 `key_width` 格寬）
fn section_blocks(
    sections: &[HelpSection],
    key_width: usize,
    theme: &Theme,
) -> Vec<Vec<Line<'static>>> {
    sections
        .iter()
        .map(|section| {
            // 顏色放在字上，不放在整行：兩欄並排時這一行會接上另一欄的字，整行的顏色會染到另一欄
            let title = Span::styled(format!("  {}", section.title), theme.help_section);
            let mut lines = vec![Line::from(title)];
            for &(key, action) in section.keys {
                let padding = " ".repeat(key_width - Span::raw(key).width());
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(format!("{key}{padding}"), theme.help_key),
                    Span::raw(action),
                ]));
            }
            lines
        })
        .collect()
}

/// 一欄的段落由上往下接起來，段落之間空一行
fn stacked(blocks: Vec<Vec<Line<'static>>>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, block) in blocks.into_iter().enumerate() {
        if index > 0 {
            lines.push(Line::default());
        }
        lines.extend(block);
    }
    lines
}

/// 兩欄的段落一段一段對齊：同一個位置的兩段，比較短的那段下面補空行，下一段的段落名才會在同一行；段落之間空一行
fn aligned(
    left: Vec<Vec<Line<'static>>>,
    right: Vec<Vec<Line<'static>>>,
) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    let count = left.len().max(right.len());
    let (mut left, mut right) = (left.into_iter(), right.into_iter());
    let (mut left_lines, mut right_lines) = (Vec::new(), Vec::new());
    for index in 0..count {
        let mut left_block = left.next().unwrap_or_default();
        let mut right_block = right.next().unwrap_or_default();
        let height = left_block.len().max(right_block.len());
        left_block.resize(height, Line::default());
        right_block.resize(height, Line::default());
        if index > 0 {
            left_lines.push(Line::default());
            right_lines.push(Line::default());
        }
        left_lines.extend(left_block);
        right_lines.extend(right_block);
    }
    (left_lines, right_lines)
}

/// 最寬那行佔幾格（中文字算 2 格）
fn width(lines: &[Line]) -> usize {
    lines.iter().map(Line::width).max().unwrap_or(0)
}

/// 左右兩欄並排：左欄每一行補空白到 `split` 格，再接上右欄同一行；比較短的那欄下面留空
fn side_by_side(
    left: Vec<Line<'static>>,
    right: Vec<Line<'static>>,
    split: usize,
) -> Vec<Line<'static>> {
    let rows = left.len().max(right.len());
    let mut left = left.into_iter();
    let mut right = right.into_iter();
    (0..rows)
        .map(|_| {
            let mut line = left.next().unwrap_or_default();
            let padding = split.saturating_sub(line.width());
            line.push_span(Span::raw(" ".repeat(padding)));
            for span in right.next().unwrap_or_default() {
                line.push_span(span);
            }
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{arrange, bounds};
    use crate::app::{App, DayList, Overlay};
    use crate::i18n::{HelpSection, Texts};
    use crate::theme::Theme;
    use crate::ui::rows;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend};

    /// 打開說明頁（`back` = 從哪個清單打開的、`full` = 完整版），畫在 `width` × `height` 的畫面上，回傳每一行的文字
    fn draw(
        texts: Texts,
        back: Option<DayList>,
        full: bool,
        width: u16,
        height: u16,
    ) -> Vec<String> {
        let mut app = App::on_date(date(2026, 10, 10), texts, Vec::new());
        app.overlay = Some(Overlay::Help { back, full });
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        rows(terminal.backend().buffer())
    }

    /// 含有 `text` 的那一行是第幾行
    fn row_of(rows: &[String], text: &str) -> usize {
        rows.iter().position(|row| row.contains(text)).expect(text)
    }

    /// `text` 在它那一行從第幾格開始（這裡的字都佔 1 格）
    fn column_of(rows: &[String], text: &str) -> usize {
        let row = &rows[row_of(rows, text)];
        row[..row.find(text).expect(text)].chars().count()
    }

    #[test]
    fn short_help_has_four_sections_in_two_columns() {
        let rows = draw(Texts::en(), None, false, 100, 40);
        let text = rows.join("\n");
        for expected in [
            "roh-cal · keys",
            // 按鍵那欄兩欄一樣寬 = 最長的鍵 + 3 格：[ / ] 5 格 → 8 格
            "hjkl    move",
            "[ / ]   prev/next month",
            "t       jump to today",
            "o       open",
            "V       cycle view",
            "v       view picker",
            "a       add item",
            "?       help",
            ",       settings",
            "q       quit",
            "Tab all keys · ?/q/Esc close",
        ] {
            assert!(text.contains(expected), "{expected}");
        }
        // 只在完整版的：看不到
        for hidden in ["Ctrl+C", "DAY LIST", "MOUSE"] {
            assert!(!text.contains(hidden), "{hidden}");
        }
        // 兩欄：NAVIGATION 和 EDITING 在同一行，VIEW、SYSTEM 在它們下面；上框線和段落名之間空一行（padding）
        let top = row_of(&rows, "roh-cal · keys");
        assert_eq!(row_of(&rows, "NAVIGATION"), top + 2);
        assert_eq!(row_of(&rows, "EDITING"), top + 2);
        // 段落名水平對齊：NAVIGATION、EDITING 都是 4 個鍵，VIEW、SYSTEM 在同一行
        assert_eq!(row_of(&rows, "VIEW"), row_of(&rows, "SYSTEM"));
        assert_eq!(row_of(&rows, "VIEW"), row_of(&rows, "u       undo") + 2);
        // 右欄從哪裡開始：左欄最長的一行（"    [ / ]   prev/next month"，27 格）+ 兩欄之間 1 格
        assert_eq!(
            column_of(&rows, "EDITING") - column_of(&rows, "NAVIGATION"),
            27 + 1
        );
        // 右欄對齊：段落名縮排 2、按鍵縮排 4
        assert_eq!(
            column_of(&rows, "EDITING") + 2,
            column_of(&rows, "a       add item")
        );
        // 右邊的留白跟兩欄之間一樣：最寬的右欄那行（settings）和右框線之間空 3 格
        let row = &rows[row_of(&rows, "settings")];
        let after = &row[row.find("settings").unwrap() + "settings".len()..];
        assert_eq!(after, "   │");
        // 下框線和最後一行（右欄的 q）之間也空一行
        assert_eq!(
            row_of(&rows, "?/q/Esc close"),
            row_of(&rows, "q       quit") + 2
        );
    }

    #[test]
    fn full_help_shows_every_section_in_two_columns() {
        // (上框線, 左欄第一段, 右欄第一段, 左欄最後一行, 下框線, 右欄最寬的說明)
        for (texts, (title, first, right, last, hint, widest)) in [
            (
                Texts::en(),
                (
                    "roh-cal · all keys",
                    "NAVIGATION",
                    "DAY LIST",
                    "Ctrl+C          quit from anywhere",
                    "Tab fewer keys",
                    "delete + subtasks",
                ),
            ),
            (
                Texts::zh_tw(),
                (
                    "roh-cal · 全部按鍵",
                    "移動",
                    "當天清單",
                    "Ctrl+C          哪裡都能直接離開",
                    "Tab 精簡",
                    "刪除（連子任務）",
                ),
            ),
        ] {
            let rows = draw(texts, None, true, 100, 30);
            // 上下框線都在畫面裡，右欄跟左欄並排（沒有退回一欄）
            assert!(row_of(&rows, title) < row_of(&rows, hint), "{title}");
            assert_eq!(row_of(&rows, first), row_of(&rows, right), "{right}");
            // 完整版右邊只留 1 格：最寬的右欄那行和右框線之間
            let row = &rows[row_of(&rows, widest)];
            assert_eq!(
                &row[row.find(widest).unwrap() + widest.len()..],
                " │",
                "{widest}"
            );
            // 最長的鍵是 click outside、e / o / Enter（13 格）→ 兩欄的按鍵那欄都是 16 格
            let text = rows.join("\n");
            assert!(text.contains(last), "{last}");
            for section in [texts.help.full.left, texts.help.full.right].concat() {
                assert!(text.contains(section.title), "{}", section.title);
            }
            // 完整版不對齊段落名：左欄第二段緊接在第一段下面（中間空一行），不會等右欄比較長的 DAY LIST
            let (nav, view) = (texts.help.full.left[0], texts.help.full.left[1]);
            let nav_last = nav.keys.len();
            assert_eq!(
                row_of(&rows, view.title),
                row_of(&rows, nav.title) + nav_last + 2,
                "{}",
                view.title
            );
        }
    }

    #[test]
    fn section_names_and_keys_use_the_theme_colors() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.overlay = Some(Overlay::Help {
            back: None,
            full: false,
        });
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rows = rows(buffer);
        // 找某段字第一個字那格的顏色
        let fg = |text: &str| {
            let y = row_of(&rows, text);
            let x = column_of(&rows, text);
            buffer[(u16::try_from(x).unwrap(), u16::try_from(y).unwrap())].fg
        };
        assert_eq!(Some(fg("NAVIGATION")), app.theme.help_section.fg);
        assert_eq!(Some(fg("hjkl    move")), app.theme.help_key.fg);
    }

    #[test]
    fn narrow_screen_stacks_the_columns() {
        let rows = draw(Texts::en(), None, false, 40, 40);
        // 一欄：EDITING 在左欄最後一列（v view picker）下面，中間空一行
        assert_eq!(row_of(&rows, "EDITING"), row_of(&rows, "view picker") + 2);
    }

    #[test]
    fn help_in_traditional_chinese() {
        let text = draw(Texts::zh_tw(), None, false, 100, 40).join("\n");
        for expected in [
            "roh-cal · 按鍵",
            "移動",
            "o       打開",
            "畫面",
            "編輯",
            "a       新增",
            "系統",
            "Tab 全部按鍵 · ?/q/Esc 關閉",
        ] {
            assert!(text.contains(expected), "{expected}");
        }
    }

    #[test]
    fn help_from_the_day_list_keeps_the_list_underneath() {
        let list = DayList {
            date: date(2026, 10, 12),
            selected: 0,
            confirm_delete: None,
        };
        let text = draw(Texts::en(), Some(list), false, 100, 40).join("\n");
        assert!(text.contains("2026-10-12 Mon"));
        assert!(text.contains("NAVIGATION"));
    }

    #[test]
    fn bounds_is_the_box_on_screen() {
        // 精簡版兩欄（寬）、一欄（窄）、完整版：框的四個角從畫面上找
        for (full, width) in [(false, 100), (false, 40), (true, 100)] {
            let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
            app.screen = ratatui::layout::Rect::new(0, 0, width, 40);
            let rows = draw(Texts::en(), None, full, width, 40);
            let top = row_of(&rows, "roh-cal · ");
            let bottom = row_of(&rows, "?/q/Esc close");
            let column = |row: usize, corner: char| {
                rows[row][..rows[row].find(corner).unwrap()].chars().count()
            };
            let (left, right) = (column(top, '╭'), column(top, '╮'));
            assert_eq!(column(bottom, '╰'), left);
            let expected = ratatui::layout::Rect::new(
                u16::try_from(left).unwrap(),
                u16::try_from(top).unwrap(),
                u16::try_from(right - left + 1).unwrap(),
                u16::try_from(bottom - top + 1).unwrap(),
            );
            assert_eq!(bounds(&app, full), expected, "{full} {width}");
        }
    }

    #[test]
    fn section_titles_keep_their_color_next_to_any_line() {
        // 右欄比左欄多一段：MORE 那一行的左欄是空行，段落名一樣要上色（顏色在字上，不是靠整行）
        let theme = Theme::default();
        let left = [HelpSection {
            title: "LEFT",
            keys: &[("a", "alpha")],
        }];
        let right = [
            HelpSection {
                title: "RIGHT",
                keys: &[("b", "beta"), ("c", "gamma")],
            },
            HelpSection {
                title: "MORE",
                keys: &[("d", "delta")],
            },
        ];
        // 對齊（精簡版）、不對齊（完整版）都一樣
        for align in [true, false] {
            let lines = arrange(&left, &right, &theme, 100, align);
            // 畫出來的顏色 = 整行的樣式再疊上那段字的樣式
            let fg = |text: &str| {
                lines
                    .iter()
                    .find_map(|line| {
                        let span = line.spans.iter().find(|span| span.content.contains(text))?;
                        Some(line.style.patch(span.style).fg)
                    })
                    .expect(text)
            };
            for title in ["LEFT", "RIGHT", "MORE"] {
                assert_eq!(fg(title), theme.help_section.fg, "{title} {align}");
            }
            // 說明不會被旁邊的段落名染色
            for action in ["alpha", "beta", "gamma", "delta"] {
                assert_eq!(fg(action), None, "{action} {align}");
            }
        }
    }
}
