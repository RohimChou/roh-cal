//! 說明頁（`?`）：蓋在畫面正中間，大小剛好包住內容（排法照 v1、tuxedo）；打開時是精簡版，`Tab` 換完整版
//!
//! 左右兩欄列按鍵；畫面不夠寬放兩欄時，改成一欄由上往下排；畫面不夠高時可以上下捲，右框線上有捲軸。
//! 內容在 `Texts::help_short`、`Texts::help_full`

use crate::app::{App, Help};
use crate::texts::{HelpPage, HelpSection};
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::{Constraint, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
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

/// 畫說明頁：大小剛好包住內容，放在畫面正中間；terminal 太矮放不下時，從往下捲了 `help.scroll` 行的地方開始畫，
/// 右框線上畫捲軸
pub fn render(frame: &mut Frame, app: &App, help: Help) {
    let theme = &app.theme;
    let (lines, area) = layout(app, frame.area(), help.full);
    // 捲過頭（例：捲到底之後把 terminal 拉高）就停在最後一頁
    let max = overflow(lines.len(), area);
    let scroll = help.scroll.min(max);
    let (title, hint) = border_lines(page(app, help.full), theme);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .padding(PADDING)
        .title_top(title)
        .title_bottom(hint);
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    super::clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block).scroll((scroll, 0)), area);
    if max > 0 {
        render_scrollbar(frame, area, scroll, max, theme);
    }
}

/// 放不下時，右框線上畫捲軸：軌道就是原本的框線（不另外畫），粗線那段 = 現在看得到的部分在整頁的哪裡
fn render_scrollbar(frame: &mut Frame, area: Rect, scroll: u16, max: u16, theme: &Theme) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .track_symbol(None)
        .begin_symbol(None)
        .end_symbol(None)
        .thumb_symbol("┃")
        .thumb_style(Style::new().fg(theme.label));
    // 能停的位置有 max + 1 個（0..=max），一次看得到 visible_rows 行
    let mut state = ScrollbarState::new(usize::from(max) + 1)
        .position(usize::from(scroll))
        .viewport_content_length(usize::from(visible_rows(area)));
    // 上下的圓角不算：捲軸只畫在右框線的直線那段
    frame.render_stateful_widget(scrollbar, area.inner(Margin::new(0, 1)), &mut state);
}

/// 說明頁的框（含框線）在 `screen` 上的範圍（滑鼠點框外 = 關掉）
pub fn area(app: &App, screen: Rect, full: bool) -> Rect {
    layout(app, screen, full).1
}

/// 說明頁最多能往下捲幾行（照 `screen` 的大小算）；放得下 = 0
pub fn max_scroll(app: &App, screen: Rect, full: bool) -> u16 {
    let (lines, area) = layout(app, screen, full);
    overflow(lines.len(), area)
}

/// 框裡放得下幾行內容：扣掉上下框線和 padding
fn visible_rows(area: Rect) -> u16 {
    area.height.saturating_sub(PADDING.top + PADDING.bottom + 2)
}

/// `lines` 行的內容放進 `area` 的框，有幾行放不下（= 最多能往下捲幾行）
fn overflow(lines: usize, area: Rect) -> u16 {
    u16::try_from(lines)
        .unwrap_or(u16::MAX)
        .saturating_sub(visible_rows(area))
}

/// 現在顯示的那一版：精簡版，或 `full` 時的完整版
fn page(app: &App, full: bool) -> HelpPage {
    let texts = app.lang.texts();
    if full {
        texts.help_full
    } else {
        texts.help_short
    }
}

/// 說明頁的內容（一行一行）和框放在哪（`render`、`area`、`max_scroll` 共用）
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
    // 說明頁不屬於哪一天、哪一個框：放在整個畫面的正中間
    let area = screen.centered(
        Constraint::Length(box_width.min(screen.width)),
        Constraint::Length(box_height.min(screen.height)),
    );
    (lines, area)
}

/// 上下框線的字：上框線 `roh-cal ∙ 按鍵`（app 名稱醒目，後面淡淡的）、下框線是操作說明
fn border_lines(help: HelpPage, theme: &Theme) -> (Line<'static>, Line<'static>) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "roh-cal",
            Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" ∙ {} ", help.title), Style::new().fg(theme.muted)),
    ]);
    let hint = Line::styled(format!(" {} ", help.hint), Style::new().fg(theme.muted));
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
    let bold = Modifier::BOLD;
    sections
        .iter()
        .map(|section| {
            // 顏色放在字上，不放在整行：兩欄並排時這一行會接上另一欄的字，整行的顏色會染到另一欄
            let title = Span::styled(
                format!("  {}", section.title),
                Style::new().fg(theme.help_section).add_modifier(bold),
            );
            let mut lines = vec![Line::from(title)];
            for &(key, action) in section.keys {
                let padding = " ".repeat(key_width - Span::raw(key).width());
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("{key}{padding}"),
                        Style::new().fg(theme.help_key).add_modifier(bold),
                    ),
                    Span::styled(action, Style::new().fg(theme.text)),
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
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    /// 打開說明頁（`help`）畫一次 `width` × `height` 的整個畫面（底下是版型 1），回傳框的位置和框裡每一列的字
    fn draw(lang: Lang, width: u16, height: u16, help: Help) -> (Rect, Vec<String>, Buffer) {
        let mut app = App::new(lang, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, width, height);
        app.help = Some(help);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let area = area(&app, app.screen, help.full);
        (area, box_rows(&buffer, area), buffer)
    }

    /// `area` 裡每一列的字；中文字佔 2 格，後面那格跳過
    fn box_rows(buffer: &Buffer, area: Rect) -> Vec<String> {
        (area.top()..area.bottom())
            .map(|y| {
                let mut row = String::new();
                let mut x = area.left();
                while x < area.right() {
                    let symbol = buffer[(x, y)].symbol();
                    row.push_str(symbol);
                    x += u16::try_from(Span::raw(symbol).width().max(1)).unwrap();
                }
                row
            })
            .collect()
    }

    #[test]
    fn short_page_puts_two_columns_in_the_middle_of_the_screen() {
        // 預期的框用另一支 Python 算：按鍵那欄 = 最長的鍵（Space、[ / ]，5 格）+ 3；右欄從左欄最寬那行 + 1 開始；
        // 段落名對齊（編輯 6 個鍵，移動那段下面補 2 行空白）
        let (area, rows, buffer) = draw(Lang::ZhTw, 70, 23, Help::default());
        // 置中除不盡的那格放在左邊（ratatui 的 `Flex::Center`）：(70 − 49) / 2 = 10.5 → 11
        assert_eq!(area, Rect::new(11, 3, 49, 17));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ───────────────────────────────╮",
                "│                                               │",
                "│  移動                        編輯             │",
                "│    hjkl    移動                Space   打勾   │",
                "│    [ / ]   上個月 / 下個月     a       新增   │",
                "│    t       今天                e       編輯   │",
                "│    Tab     換框                r       改期   │",
                "│                                d       刪除   │",
                "│                                u       復原   │",
                "│                                               │",
                "│  畫面                        系統             │",
                "│    1       月曆 + 事項         ?       說明   │",
                "│    3       大月曆              ,       設定   │",
                "│    4       任務頁              q       離開   │",
                "│    s       日曆清單                           │",
                "│                                               │",
                "╰ Tab 全部按鍵  ?/q/Esc 關閉 ───────────────────╯",
            ]
        );
        // 顏色：框線亮起來（鍵盤在這裡）、段落名、按鍵、說明各一種顏色
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(11, 3)].fg, theme.focus);
        assert_eq!(buffer[(14, 5)].fg, theme.help_section);
        assert_eq!(buffer[(16, 6)].fg, theme.help_key);
        assert_eq!(buffer[(24, 6)].fg, theme.text);
        // 框外面還是底下的月曆（版型 1 的左上角）
        assert_eq!(buffer[(0, 0)].symbol(), "╭");
    }

    #[test]
    fn a_narrow_screen_stacks_the_columns() {
        // 44 格放不下兩欄（左欄 28 + 右欄 16 + 框線 2 = 46）：右欄接在左欄下面，中間空一行
        let (area, rows, _) = draw(Lang::ZhTw, 44, 30, Help::default());
        assert_eq!(area, Rect::new(6, 1, 32, 28));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 按鍵 ──────────────╮",
                "│                              │",
                "│  移動                        │",
                "│    hjkl    移動              │",
                "│    [ / ]   上個月 / 下個月   │",
                "│    t       今天              │",
                "│    Tab     換框              │",
                "│                              │",
                "│  畫面                        │",
                "│    1       月曆 + 事項       │",
                "│    3       大月曆            │",
                "│    4       任務頁            │",
                "│    s       日曆清單          │",
                "│                              │",
                "│  編輯                        │",
                "│    Space   打勾              │",
                "│    a       新增              │",
                "│    e       編輯              │",
                "│    r       改期              │",
                "│    d       刪除              │",
                "│    u       復原              │",
                "│                              │",
                "│  系統                        │",
                "│    ?       說明              │",
                "│    ,       設定              │",
                "│    q       離開              │",
                "│                              │",
                "╰ Tab 全部按鍵  ?/q/Esc 關閉 ──╯",
            ]
        );
    }

    #[test]
    fn a_short_screen_scrolls_with_a_bar_on_the_right_border() {
        // 44 × 20：一欄 24 行，框最高 20、裡面放得下 16 行 → 能捲 8 行；捲軸畫在右框線上
        let (area, rows, _) = draw(Lang::ZhTw, 44, 20, Help::default());
        assert_eq!(area, Rect::new(6, 0, 32, 20));
        let bar = |rows: &[String]| -> Vec<usize> {
            rows.iter()
                .enumerate()
                .filter(|(_, row)| row.ends_with('┃'))
                .map(|(y, _)| y)
                .collect()
        };
        // 最上面：第一段的段落名看得到，捲軸在最上面（圓角下面那一列開始）
        assert_eq!(rows[2], "│  移動                        ┃");
        assert_eq!(bar(&rows).first(), Some(&1));
        // 捲到底（捲過頭也停在最後一頁）：最後一個鍵看得到，捲軸在最下面
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
            let page = lang.texts().help_full;
            let (area, rows, _) = draw(
                lang,
                100,
                34,
                Help {
                    full: true,
                    scroll: 0,
                },
            );
            // 右欄 28 行 + padding 2 + 框線 2，放得下不用捲
            assert_eq!(area.height, 32, "{lang:?}");
            let text = rows.join("\n");
            for section in page.left.iter().chain(page.right) {
                assert!(text.contains(section.title), "{}", section.title);
            }
            // 兩欄並排：兩欄的第一段在同一列
            let row_of = |text: &str| rows.iter().position(|row| row.contains(text));
            assert_eq!(row_of(page.left[0].title), row_of(page.right[0].title));
            // 不對齊段落名：左欄第二段緊接在第一段下面（中間空一行），不等右欄比較長的第一段
            assert_eq!(
                row_of(page.left[1].title),
                row_of(page.left[0].title).map(|row| row + page.left[0].keys.len() + 2),
                "{lang:?}"
            );
            // 完整版右邊只留 1 格：右欄最寬那行（說明）和右框線之間
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
}
