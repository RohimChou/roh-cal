//! 設定頁（`,`）：蓋在畫面正中間的框，設定檔的開關照表（`[calendar]`、`[month]`）分段列出來；
//! 最下面一行說明選到的那一列在做什麼
//!
//! ```text
//! ╭ roh-cal ∙ 設定 ─────────────────────────╮
//! │                                         │
//! │  [calendar]                             │
//! │    relative_weeks   off                 │  ← 選到的那列墊底色
//! │                                         │
//! │  [month]                                │
//! │    grid_lines       on                  │
//! │  ...                                    │
//! │  週數欄：選取那週寫週數，其他週寫差幾週 │
//! │                                         │
//! ╰ jk 選  hl 改  r 還原  R 全部還原  Esc 關閉 ╯
//! ```

use crate::app::{App, ROWS, SettingsPage};
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

/// 框裡的一行：段落名、一列開關（`ROWS` 的第幾列）、空行、說明
enum Entry {
    /// `[calendar]` 這種段落名
    Section(&'static str),
    /// 一列開關：`ROWS` 的第幾列
    Row(usize),
    /// 空行
    Blank,
    /// 最下面：選到的那一列的說明
    Description,
}

/// 框裡由上到下每一行是什麼：每一段是段落名 + 那一段的列，段和段之間空一行；最後空一行再寫說明
fn entries() -> Vec<Entry> {
    let mut entries = Vec::new();
    for (index, row) in ROWS.iter().enumerate() {
        let new_section = index == 0 || ROWS[index - 1].section() != row.section();
        if new_section {
            if index > 0 {
                entries.push(Entry::Blank);
            }
            entries.push(Entry::Section(row.section()));
        }
        entries.push(Entry::Row(index));
    }
    entries.push(Entry::Blank);
    entries.push(Entry::Description);
    entries
}

/// 名字那欄的寬度：最長的名字 + `NAME_GAP`
fn name_width() -> usize {
    ROWS.iter().map(|row| row.name().len()).max().unwrap_or(0) + NAME_GAP
}

/// 框的位置：大小剛好包住內容（說明取最長的那句，選別列時框不會變大變小），放在 `screen` 正中間
pub fn area(app: &App, screen: Rect) -> Rect {
    let texts = app.lang.texts();
    // 一列：縮排 4 + 名字那欄 + 值（最長 `off` 3 格）
    let row_width = 4 + name_width() + 3;
    let description_width = ROWS
        .iter()
        .map(|&row| Span::raw(texts.setting_description(row)).width() + 2)
        .max()
        .unwrap_or(0);
    let (title, hint) = (title(app), hint_text(app, None));
    let inside = (row_width.max(description_width) + RIGHT_MARGIN)
        .max(title.width())
        .max(Span::raw(hint).width() + 2);
    let width = u16::try_from(inside + 2).unwrap_or(u16::MAX);
    let height = u16::try_from(entries().len())
        .unwrap_or(u16::MAX)
        .saturating_add(PADDING.top + PADDING.bottom + 2);
    screen.centered(
        Constraint::Length(width.min(screen.width)),
        Constraint::Length(height.min(screen.height)),
    )
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
fn hint_text(app: &App, page: Option<SettingsPage>) -> &'static str {
    let texts = app.lang.texts();
    if page.is_some_and(|page| page.confirm_reset) {
        texts.settings_reset_all
    } else {
        texts.settings_hint
    }
}

/// 畫設定頁：每一列「名字　值」，選到的那列墊底色；最下面寫選到的那一列在做什麼
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
            format!(" {} ", hint_text(app, Some(page))),
            hint_style,
        ));
    let inner = block.inner(area);
    let chosen = ROWS[page.row.min(ROWS.len() - 1)];
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
                let row = ROWS[index];
                let value = if row.value(&app.settings) {
                    "on"
                } else {
                    "off"
                };
                let mut value_style = Style::new().fg(theme.text);
                if index == page.row {
                    value_style = value_style.add_modifier(Modifier::BOLD);
                }
                Line::from(vec![
                    Span::styled(
                        format!("    {:<width$}", row.name(), width = name_width()),
                        Style::new().fg(theme.help_key),
                    ),
                    Span::styled(value, value_style),
                ])
            }
            Entry::Blank => Line::default(),
            Entry::Description => Line::styled(
                format!("  {}", texts.setting_description(chosen)),
                Style::new().fg(theme.muted),
            ),
        })
        .collect();
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    super::clear_wide_left_of(frame.buffer_mut(), area);
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
    // 選到的那列整列墊底色（框裡面那麼寬）
    if let Some(y) = row_y(inner, page.row) {
        frame.buffer_mut().set_style(
            Rect {
                y,
                height: 1,
                ..inner
            },
            Style::new().bg(theme.cursor_line),
        );
    }
}

/// 第 `row` 列（`ROWS` 的位置）畫在框裡面（`inner`）的哪一列；畫不下就是 `None`
fn row_y(inner: Rect, row: usize) -> Option<u16> {
    let line = entries()
        .iter()
        .position(|entry| matches!(entry, Entry::Row(index) if *index == row))?;
    let y = inner.y.checked_add(u16::try_from(line).ok()?)?;
    (y < inner.bottom()).then_some(y)
}

/// 滑鼠點在 (`column`, `row`) 點到設定頁的哪一列（`ROWS` 的位置）；點在段落名、空行、說明、框線上回傳 `None`
pub fn row_at(app: &App, screen: Rect, column: u16, row: u16) -> Option<usize> {
    let inner = Block::bordered().padding(PADDING).inner(area(app, screen));
    if column < inner.x || column >= inner.right() {
        return None;
    }
    (0..ROWS.len()).find(|&index| row_y(inner, index) == Some(row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use crate::theme::Theme;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};

    /// 打開設定頁（`page`）畫一次 70 × 23 的畫面，回傳 app、框的位置、框裡每一列的字、整個畫面
    fn draw(page: SettingsPage) -> (App, Rect, Vec<String>, Buffer) {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 70, 23);
        app.settings_page = Some(page);
        let mut terminal = Terminal::new(TestBackend::new(70, 23)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &mut app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let area = area(&app, app.screen);
        let rows = (area.top()..area.bottom())
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
            .collect();
        (app, area, rows, buffer)
    }

    #[test]
    fn settings_list_the_switches_by_table_with_a_description() {
        // 預期的框用另一支 Python 算：名字那欄 = 最長的名字（relative_weeks，14 格）+ 3；
        // 寬度看最長的說明（38 格 + 縮排 2 + 右邊留 2）和下框線的操作說明（42 格 + 前後空白），取大的
        let (_, area, rows, buffer) = draw(SettingsPage::default());
        assert_eq!(area, Rect::new(12, 5, 46, 13));
        assert_eq!(
            rows,
            [
                "╭ roh-cal ∙ 設定 ────────────────────────────╮",
                "│                                            │",
                "│  [calendar]                                │",
                "│    relative_weeks   off                    │",
                "│                                            │",
                "│  [month]                                   │",
                "│    grid_lines       on                     │",
                "│    item_icons       on                     │",
                "│    center_cursor    off                    │",
                "│                                            │",
                "│  週數欄：選取那週寫週數，其他週寫差幾週    │",
                "│                                            │",
                "╰ jk 選  hl 改  r 還原  R 全部還原  Esc 關閉 ╯",
            ]
        );
        // 選到的那列（第一列，y = 5 + 3）整列墊底色、值加粗；其他列沒有底色
        let theme = Theme::catppuccin_mocha();
        assert_eq!(buffer[(13, 8)].bg, theme.cursor_line);
        assert_eq!(buffer[(56, 8)].bg, theme.cursor_line);
        // 值在框裡面縮排 4 + 名字那欄 17 格之後：13 + 21 = 34
        assert!(buffer[(34, 8)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(13, 11)].bg, Color::Reset);
        assert_eq!(buffer[(17, 8)].fg, theme.help_key);
        assert_eq!(buffer[(15, 7)].fg, theme.help_section);
    }

    #[test]
    fn the_description_follows_the_chosen_row_and_r_asks_on_the_bottom_border() {
        let (_, _, rows, _) = draw(SettingsPage {
            row: 3,
            confirm_reset: true,
        });
        assert_eq!(rows[10], "│  大月曆：選取的那週固定在正中間            │");
        assert_eq!(rows[12], "╰ 全部還原成預設？y 還原，其他鍵取消 ────────╯");
    }

    #[test]
    fn clicks_map_to_the_rows() {
        let (app, _, _, _) = draw(SettingsPage::default());
        // 第一列在 y 8、month 的三列在 y 11～13；段落名、空行、說明、框線上都不算
        assert_eq!(row_at(&app, app.screen, 20, 8), Some(0));
        assert_eq!(row_at(&app, app.screen, 13, 11), Some(1));
        assert_eq!(row_at(&app, app.screen, 56, 13), Some(3));
        for (x, y) in [(20, 7), (20, 10), (20, 15), (12, 8), (57, 8), (20, 5)] {
            assert_eq!(row_at(&app, app.screen, x, y), None, "({x}, {y})");
        }
    }
}
