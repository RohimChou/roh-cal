//! 設定頁（`,`）：蓋在畫面正中間的框，設定照設定檔的樣子列出來：最上層的（`theme`）在前，
//! 再來照表（`[calendar]`、`[month]`、`[colors]`）分段；最下面一行說明選到的那一列在做什麼
//!
//! ```text
//! ╭ roh-cal ∙ 設定 ─────────────────────────╮
//! │                                         │
//! │  theme              catppuccin-mocha    │  ← 選到的那列墊底色
//! │                                         │
//! │  [calendar]                             │
//! │    relative_weeks   off                 │
//! │  ...                                    │
//! │  [colors]                               │
//! │    text             ██ text             │  ← 色塊 = 那個角色現在的顏色，後面是色票裡的名字
//! │  ...                                    ┃  ← 放不下時可以捲（選到的那列一定看得到），右框線上有捲軸
//! │                                         │
//! │  顏色：l 換下一套 theme，h 換上一套     │  ← 說明固定在最下面，不跟著捲
//! │                                         │
//! ╰ jk 選  hl 改  r 還原  R 全部還原  Esc 關閉 ╯
//! ```

use crate::app::{App, SettingsPage, rows};
use crate::theme;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Padding, Paragraph},
};

/// 名字和值之間至少空幾格（名字那欄的寬度 = 最長的名字 + 這麼多格）
const NAME_GAP: usize = 3;
/// 框線和內容之間：上下各空 1 行（左邊靠段落名的縮排留空）
const PADDING: Padding = Padding::vertical(1);
/// 內容和右框線之間空幾格（跟左邊段落名的縮排一樣）
const RIGHT_MARGIN: usize = 2;
/// 顏色那幾列的值前面的色塊：2 格空白墊那個顏色的底色，再空 1 格
const SWATCH: &str = "  ";
/// 清單下面固定的幾行：空一行、說明
const FOOTER: u16 = 2;
/// 捲動時選到的那列離上下邊緣至少留幾行
const SCROLLOFF: i64 = 1;

/// 清單裡的一行：段落名、一列設定（`rows()` 的第幾列）、空行
enum Entry {
    /// `[calendar]` 這種段落名
    Section(&'static str),
    /// 一列設定：`rows()` 的第幾列
    Row(usize),
    /// 空行
    Blank,
}

/// 清單由上到下每一行是什麼：最上層的列在前（沒有段落名）；每一段是段落名 + 那一段的列，段和段之間空一行
fn entries() -> Vec<Entry> {
    let rows = rows();
    let mut entries = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let new_section = index == 0 || rows[index - 1].section() != row.section();
        if new_section {
            if index > 0 {
                entries.push(Entry::Blank);
            }
            if let Some(section) = row.section() {
                entries.push(Entry::Section(section));
            }
        }
        entries.push(Entry::Row(index));
    }
    entries
}

/// 第 `row` 列（`rows()` 的位置）是清單的第幾行
fn line_of(row: usize) -> Option<usize> {
    entries()
        .iter()
        .position(|entry| matches!(entry, Entry::Row(index) if *index == row))
}

/// 名字那欄的寬度：最長的名字 + `NAME_GAP`
fn name_width() -> usize {
    rows().iter().map(|row| row.name().len()).max().unwrap_or(0) + NAME_GAP
}

/// 值那欄最寬幾格：開關的 `off`、最長的 theme 名字、色塊 + 最長的顏色名字（每一套 theme 的色票都算）
fn value_width() -> usize {
    let names = theme::names();
    let colors = names
        .iter()
        .flat_map(|name| theme::choices(name))
        .map(|(name, _)| SWATCH.len() + 1 + name.len());
    names
        .iter()
        .map(|name| name.len())
        .chain(colors)
        .chain(["off".len()])
        .max()
        .unwrap_or(0)
}

/// 框的位置：大小剛好包住內容（說明取最長的那句，選別列時框不會變大變小），放在 `screen` 正中間；
/// 比畫面高就縮到畫面那麼高（清單可以捲）
pub fn area(app: &App, screen: Rect) -> Rect {
    let texts = app.lang.texts();
    // 一列：縮排 4 + 名字那欄 + 值
    let row_width = 4 + name_width() + value_width();
    let description_width = rows()
        .iter()
        .map(|&row| Span::raw(texts.setting_description(row)).width() + 2)
        .max()
        .unwrap_or(0);
    // 下框線放得下操作說明，也放得下按了 `R` 之後的問句
    let hint_width = [texts.settings_hint, texts.settings_reset_all]
        .map(|hint| Span::raw(hint).width() + 2)
        .into_iter()
        .max()
        .unwrap_or(0);
    let inside = (row_width.max(description_width) + RIGHT_MARGIN)
        .max(title(app).width())
        .max(hint_width);
    let width = u16::try_from(inside + 2).unwrap_or(u16::MAX);
    let height = u16::try_from(entries().len())
        .unwrap_or(u16::MAX)
        .saturating_add(FOOTER + PADDING.top + PADDING.bottom + 2);
    screen.centered(
        Constraint::Length(width.min(screen.width)),
        Constraint::Length(height.min(screen.height)),
    )
}

/// 框裡面（扣掉框線、padding）
fn inner(area: Rect) -> Rect {
    Block::bordered().padding(PADDING).inner(area)
}

/// 清單那一塊：框裡面扣掉最下面固定的空行和說明
fn list_area(area: Rect) -> Rect {
    let inner = inner(area);
    Rect {
        height: inner.height.saturating_sub(FOOTER),
        ..inner
    }
}

/// 捲到哪：從 `page.top` 開始，選到的那列離上下邊緣至少留 `SCROLLOFF` 行，不夠才捲；不捲過頭
pub fn scrolled(app: &App, screen: Rect, page: SettingsPage) -> usize {
    let height = i64::from(list_area(area(app, screen)).height);
    let line = line_of(page.row).unwrap_or(0);
    let top = super::months::scrolled_top(
        i64::try_from(page.top).unwrap_or(0),
        i64::try_from(line).unwrap_or(0),
        height,
        SCROLLOFF,
    );
    let max = i64::try_from(entries().len()).unwrap_or(0) - height;
    usize::try_from(top.min(max).max(0)).unwrap_or(0)
}

/// 上框線：`roh-cal ∙ 設定`（app 名稱醒目，後面淡淡的）
fn title(app: &App) -> Line<'static> {
    let theme = &app.theme;
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "roh-cal",
            Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ∙ {} ", app.lang.texts().settings_title),
            Style::new().fg(theme.muted),
        ),
    ])
}

/// 下框線的字：平常是操作說明；正在問要不要全部還原（`page.confirm_reset`）就換成問句
fn hint_text(app: &App, page: SettingsPage) -> &'static str {
    let texts = app.lang.texts();
    if page.confirm_reset {
        texts.settings_reset_all
    } else {
        texts.settings_hint
    }
}

/// 畫設定頁：每一列「名字　值」（最上層的縮排 2、表裡的縮排 4，值對齊在同一欄；顏色前面有色塊），
/// 選到的那列墊底色；從 `page.top` 那行開始畫（先用 `scrolled` 捲好）；最下面寫選到的那一列在做什麼
pub fn render(frame: &mut Frame, app: &App, page: SettingsPage) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let area = area(app, frame.area());
    let hint_style = if page.confirm_reset {
        Style::new().fg(theme.notice)
    } else {
        Style::new().fg(theme.muted)
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .padding(PADDING)
        .title_top(title(app))
        .title_bottom(Line::styled(
            format!(" {} ", hint_text(app, page)),
            hint_style,
        ));
    let rows = rows();
    let chosen = rows[page.row.min(rows.len() - 1)];
    let lines: Vec<Line> = entries()
        .iter()
        .map(|entry| match *entry {
            Entry::Section(name) => Line::styled(
                format!("  [{name}]"),
                Style::new()
                    .fg(theme.help_section)
                    .add_modifier(Modifier::BOLD),
            ),
            Entry::Row(index) => {
                let row = rows[index];
                let mut value_style = Style::new().fg(theme.text);
                if index == page.row {
                    value_style = value_style.add_modifier(Modifier::BOLD);
                }
                // 表裡的多縮排 2 格，名字那欄少 2 格，值才對齊
                let (indent, width) = match row.section() {
                    Some(_) => ("    ", name_width()),
                    None => ("  ", name_width() + 2),
                };
                let mut spans = vec![Span::styled(
                    format!("{indent}{:<width$}", row.name()),
                    Style::new().fg(theme.help_key),
                )];
                // 顏色：先畫色塊（底色 = 那個角色現在的顏色；`Reset` = terminal 預設的顏色，看起來就是空的）
                if let Some(color) = row.color(&app.theme) {
                    spans.push(Span::styled(SWATCH, Style::new().bg(color)));
                    spans.push(Span::raw(" "));
                }
                spans.push(Span::styled(
                    row.value(&app.settings, &app.theme),
                    value_style,
                ));
                Line::from(spans)
            }
            Entry::Blank => Line::default(),
        })
        .collect();
    let list = list_area(area);
    let scroll = u16::try_from(page.top).unwrap_or(u16::MAX);
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    super::clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), list);
    // 說明固定在框裡最下面那行
    frame.render_widget(
        Line::styled(
            format!("  {}", texts.setting_description(chosen)),
            Style::new().fg(theme.muted),
        ),
        Rect {
            y: inner(area).bottom().saturating_sub(1),
            height: 1,
            ..list
        },
    );
    // 選到的那列整列墊底色（框裡面那麼寬；色塊自己的底色不蓋掉）
    if let Some(y) = row_y(list, page.top, page.row) {
        for x in list.left()..list.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            if cell.bg == ratatui::style::Color::Reset {
                cell.set_bg(theme.cursor_line);
            }
        }
    }
    // 放不下：右框線上畫捲軸
    let max = u16::try_from(entries().len())
        .unwrap_or(u16::MAX)
        .saturating_sub(list.height);
    if max > 0 {
        super::render_scrollbar(frame, area, scroll.min(max), max, list.height, theme);
    }
}

/// 第 `row` 列（`rows()` 的位置）畫在清單那塊（`list`，從第 `top` 行開始畫）的哪一列；捲出去了是 `None`
fn row_y(list: Rect, top: usize, row: usize) -> Option<u16> {
    let line = line_of(row)?.checked_sub(top)?;
    let y = list.y.checked_add(u16::try_from(line).ok()?)?;
    (y < list.bottom()).then_some(y)
}

/// 滑鼠點在 (`column`, `row`) 點到設定頁的哪一列（`rows()` 的位置）；點在段落名、空行、說明、框線上回傳 `None`
pub fn row_at(app: &App, screen: Rect, column: u16, row: u16) -> Option<usize> {
    let page = app.settings_page()?;
    let list = list_area(area(app, screen));
    if column < list.x || column >= list.right() {
        return None;
    }
    (0..rows().len()).find(|&index| row_y(list, page.top, index) == Some(row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Mode;
    use crate::texts::Lang;
    use crate::theme::Theme;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};

    /// 打開設定頁（`page`）畫一次 70 × `height` 的畫面，回傳 app（`settings_page.top` 已經捲好）、框的位置、
    /// 框裡每一列的字（右框線上的捲軸 `┃` 換回 `│`）、整個畫面
    fn draw(page: SettingsPage, height: u16) -> (App, Rect, Vec<String>, Buffer) {
        draw_app(
            App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new()),
            page,
            height,
        )
    }

    /// 跟 `draw` 一樣，但用傳進來的 `app`（例：先換了 theme）
    fn draw_app(mut app: App, page: SettingsPage, height: u16) -> (App, Rect, Vec<String>, Buffer) {
        app.screen = Rect::new(0, 0, 70, height);
        app.mode = Mode::Settings {
            page,
            opened: app.settings.clone(),
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

    #[test]
    fn settings_list_theme_switches_and_colors_like_the_config_file() {
        // 預期的框用另一支 Python 算（照 themes/catppuccin-mocha.toml 的 [roles] 寫每個角色的色票名字）：
        // 名字那欄 = 最長的名字（week_number_dim，15 格）+ 3，最上層的 theme 縮排少 2 格、名字那欄多 2 格，值都在同一欄；
        // 寬度看最長的說明、下框線的操作說明（42 格 + 前後空白），取大的
        let (_, area, rows, buffer) = draw(SettingsPage::default(), 40);
        assert_eq!(area, Rect::new(12, 1, 46, 38));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 設定 ────────────────────────────╮",
                "│                                            │",
                "│  theme               catppuccin-mocha      │",
                "│                                            │",
                "│  [calendar]                                │",
                "│    relative_weeks    off                   │",
                "│                                            │",
                "│  [month]                                   │",
                "│    grid_lines        on                    │",
                "│    item_icons        on                    │",
                "│    center_cursor     off                   │",
                "│                                            │",
                "│  [colors]                                  │",
                "│    text                 text               │",
                "│    muted                subtext0           │",
                "│    line                 surface2           │",
                "│    year_line            lavender           │",
                "│    label                lavender           │",
                "│    week_number          yellow             │",
                "│    week_number_dim      dim_yellow         │",
                "│    weekend              blue               │",
                "│    has_item             peach              │",
                "│    holiday              red                │",
                "│    today                green              │",
                "│    selected             red                │",
                "│    cursor_line          surface0           │",
                "│    focus                mauve              │",
                "│    checked              mauve              │",
                "│    notice               yellow             │",
                "│    error                red                │",
                "│    grid                 surface0           │",
                "│    other_month          overlay0           │",
                "│    help_section         blue               │",
                "│    help_key             peach              │",
                "│                                            │",
                "│  顏色：l 換下一套 theme，h 換上一套        │",
                "│                                            │",
                "╰ jk 選  hl 改  r 還原  R 全部還原  Esc 關閉 ╯",
            ]
        );
        // 選到的那列（第一列 theme，y = 1 + 2）整列墊底色、值加粗；其他列沒有底色
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(13, 3)].bg, theme.cursor_line);
        assert_eq!(buffer[(56, 3)].bg, theme.cursor_line);
        // 值那欄：框裡面 13 + 縮排 2 + 名字那欄 20 = 35（表裡的列：13 + 4 + 18，一樣是 35）
        assert!(buffer[(35, 3)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(35, 6)].symbol(), "o");
        assert_eq!(buffer[(13, 6)].bg, Color::Reset);
        assert_eq!(buffer[(15, 3)].fg, theme.help_key);
        assert_eq!(buffer[(15, 5)].fg, theme.help_section);
        // 色塊：today 那列（清單第 21 行，y = 3 + 21）值那欄的 2 格墊 today 的顏色，後面是名字
        assert_eq!(buffer[(35, 24)].bg, theme.today);
        assert_eq!(buffer[(36, 24)].bg, theme.today);
        assert_eq!(buffer[(37, 24)].bg, Color::Reset);
    }

    #[test]
    fn the_chosen_color_row_keeps_its_swatch() {
        // 選到 text 那列（第 6 列，清單第 11 行，y 14）：整列墊底色，但色塊還是 text 的顏色
        let (_, _, rows, buffer) = draw(
            SettingsPage {
                row: 5,
                ..SettingsPage::default()
            },
            40,
        );
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(14, 14)].bg, theme.cursor_line);
        assert_eq!(buffer[(35, 14)].bg, theme.text);
        assert_eq!(buffer[(38, 14)].bg, theme.cursor_line);
        assert_eq!(rows[35], "│  顏色：l 換色票的下一個，r 回到 theme 的   │");
    }

    #[test]
    fn a_reset_color_gets_no_swatch() {
        // terminal 的 text 是 terminal 預設的字色（Reset）：色塊那 2 格看起來是空的，後面寫 Reset
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.settings.set_theme("terminal");
        app.apply_theme();
        let (_, _, rows, buffer) = draw_app(app, SettingsPage::default(), 40);
        assert_eq!(rows[13], "│    text                 Reset              │");
        assert_eq!(buffer[(35, 14)].bg, Color::Reset);
        assert_eq!(buffer[(36, 14)].bg, Color::Reset);
        // 其他角色照樣有色塊（muted = dark-gray）
        assert_eq!(buffer[(35, 15)].bg, Color::DarkGray);
    }

    #[test]
    fn a_short_screen_scrolls_to_the_chosen_row() {
        // 70 × 23：框最高 23、清單放得下 17 行（32 行要捲）；選最後一列（help_key，清單第 31 行）→ 捲到底（最上面第 15 行），
        // 說明還是在最下面
        let (app, area, rows, buffer) = draw(
            SettingsPage {
                row: 25,
                ..SettingsPage::default()
            },
            23,
        );
        assert_eq!(app.settings_page().map(|page| page.top), Some(15));
        assert_eq!(area, Rect::new(12, 0, 46, 23));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 設定 ────────────────────────────╮",
                "│                                            │",
                "│    label                lavender           │",
                "│    week_number          yellow             │",
                "│    week_number_dim      dim_yellow         │",
                "│    weekend              blue               │",
                "│    has_item             peach              │",
                "│    holiday              red                │",
                "│    today                green              │",
                "│    selected             red                │",
                "│    cursor_line          surface0           │",
                "│    focus                mauve              │",
                "│    checked              mauve              │",
                "│    notice               yellow             │",
                "│    error                red                │",
                "│    grid                 surface0           │",
                "│    other_month          overlay0           │",
                "│    help_section         blue               │",
                "│    help_key             peach              │",
                "│                                            │",
                "│  顏色：l 換色票的下一個，r 回到 theme 的   │",
                "│                                            │",
                "╰ jk 選  hl 改  r 還原  R 全部還原  Esc 關閉 ╯",
            ]
        );
        // 捲軸在右框線的最下面那段
        assert_eq!(buffer[(57, 21)].symbol(), "┃");
        assert_eq!(buffer[(57, 1)].symbol(), "│");
        // 點最下面那列：還是 help_key；最上面那列是 label（第 10 列）：照捲過的位置換算
        assert_eq!(row_at(&app, app.screen, 20, 18), Some(25));
        assert_eq!(row_at(&app, app.screen, 20, 2), Some(9));
    }

    #[test]
    fn r_asks_on_the_bottom_border() {
        let (_, _, rows, _) = draw(
            SettingsPage {
                row: 4,
                confirm_reset: true,
                ..SettingsPage::default()
            },
            40,
        );
        assert_eq!(rows[35], "│  大月曆：選取的那週固定在正中間            │");
        assert_eq!(rows[37], "╰ 全部還原成預設？y 還原，其他鍵取消 ────────╯");
    }

    #[test]
    fn clicks_map_to_the_rows() {
        let (app, _, _, _) = draw(SettingsPage::default(), 40);
        // theme 在 y 3、relative_weeks 在 y 6、grid_lines 在 y 9、最後的 help_key 在 y 34；
        // 段落名、空行、說明、框線上都不算
        assert_eq!(row_at(&app, app.screen, 20, 3), Some(0));
        assert_eq!(row_at(&app, app.screen, 20, 6), Some(1));
        assert_eq!(row_at(&app, app.screen, 13, 9), Some(2));
        assert_eq!(row_at(&app, app.screen, 56, 34), Some(25));
        for (x, y) in [
            (20, 4),
            (20, 5),
            (20, 13),
            (20, 36),
            (12, 3),
            (57, 3),
            (20, 1),
        ] {
            assert_eq!(row_at(&app, app.screen, x, y), None, "({x}, {y})");
        }
    }
}
