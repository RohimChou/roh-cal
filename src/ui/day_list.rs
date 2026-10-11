//! 當天清單：月曆上按 `Enter` / `o` 打開，疊在月曆框的正中間，列出那天的項目

use crate::app::{App, DayList};
use crate::tasks::Item;
use crate::ui::items_on;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, ListState, Padding, Paragraph},
};

/// 框的寬度（含框線）：月曆框左右各留 `MARGIN` 格，但最多 `MAX_WIDTH`（大月曆在很寬的 terminal 上才不會太長）
const MARGIN: u16 = 2;
const MAX_WIDTH: u16 = 60;
/// 框線和內容之間上下左右各空 1 格（選到那行的底色條也跟框線隔開）
const PADDING: Padding = Padding::uniform(1);

/// 畫當天清單：一個項目一行，選到的那行整行塗底色；日期在上框線、操作說明在下框線。
/// `calendar` = 月曆框的位置（小月曆在左上角、大月曆是整個 terminal），清單疊在它的正中間
pub fn render(frame: &mut Frame, list: DayList, app: &App, calendar: Rect) {
    let theme = &app.theme;
    let items = items_on(&app.items, list.date);
    // 大小：框線 2 + 上下 padding + 項目（沒有項目時是 1 行說明）；月曆框不夠高就縮（Length 超過月曆框會被縮到月曆框的大小）
    let width = calendar.width.saturating_sub(2 * MARGIN).min(MAX_WIDTH);
    let height = 2 + PADDING.top + PADDING.bottom + items.len().max(1) as u16;
    let area = calendar.centered(Constraint::Length(width), Constraint::Length(height));

    let title = format!(" {} ", (app.texts.day_title)(list.date));
    let hint = format!(" {} ", app.texts.day_hint);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .padding(PADDING)
        .title_top(Line::styled(title, theme.title))
        .title_bottom(Line::styled(hint, theme.hint));
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    frame.render_widget(Clear, area);
    if items.is_empty() {
        let empty = Line::styled(format!(" {}", app.texts.day_empty), theme.hint);
        frame.render_widget(Paragraph::new(empty).block(block), area);
        return;
    }
    let rows: Vec<ListItem> = items
        .iter()
        .map(|item| ListItem::new(item_line(item, app)))
        .collect();
    // List 會自己捲動（選到的那行一定在框內），選到的那行整行塗上 highlight_style
    let mut state = ListState::default().with_selected(Some(list.selected));
    let widget = List::new(rows)
        .block(block)
        .highlight_style(theme.selected_bg);
    frame.render_stateful_widget(widget, area, &mut state);
}

/// 一個項目一行：` [ ] 14:00 標題`（沒寫 checkbox 的也顯示 `[ ]`，每個項目都能打勾）；
/// 完成的用 `item_done`（灰 + 劃線）。顏色掛在 span 上，劃線才不會延伸到行尾的空白
fn item_line(item: &Item, app: &App) -> Line<'static> {
    let mark = if item.done { "[x]" } else { "[ ]" };
    let mut text = format!(" {mark} ");
    if let Some(time) = item.time {
        text.push_str(&format!("{:02}:{:02} ", time.hour(), time.minute()));
    }
    text.push_str(&item.title);
    let style = if item.done {
        app.theme.item_done
    } else {
        app.theme.item
    };
    Line::from(Span::styled(text, style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Overlay;
    use crate::i18n::Texts;
    use crate::tasks::parse;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    /// 測試用的任務：10/12 有三個（順序故意打亂）、10/13 有一個
    const TASKS: &str = "- [ ] 下午的會 @2026-10-12 14:00\n\
                         - [x] 早上的會 @2026-10-12 09:00\n\
                         - 沒時間的事 @2026-10-12\n\
                         - [ ] 別天 @2026-10-13";

    /// 打開 `day` 那天的清單、選第 `selected` 個，畫在 `width` × `height` 的畫面上
    fn draw(texts: Texts, day: i8, selected: usize, width: u16, height: u16) -> (Buffer, App) {
        let mut app = App::on_date(date(2026, 10, 10), texts, parse(TASKS).items);
        app.overlay = Some(Overlay::DayList(DayList {
            date: date(2026, 10, day),
            selected,
        }));
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        (terminal.backend().buffer().clone(), app)
    }

    /// 每一行的文字，去掉尾端空白；中文字佔 2 格，後面那格（buffer 裡是空白）跳過，接起來才是原本的字
    fn rows(buffer: &Buffer) -> Vec<String> {
        let area = buffer.area;
        (0..area.height)
            .map(|y| {
                let mut row = String::new();
                let mut x = 0;
                while x < area.width {
                    let symbol = buffer[(x, y)].symbol();
                    row.push_str(symbol);
                    x += u16::try_from(Span::raw(symbol).width().max(1)).unwrap();
                }
                row.trim_end().to_string()
            })
            .collect()
    }

    /// 找出含有 `text` 的那一行是第幾行
    fn row_of(rows: &[String], text: &str) -> u16 {
        let index = rows.iter().position(|row| row.contains(text));
        u16::try_from(index.expect(text)).unwrap()
    }

    #[test]
    fn list_shows_the_days_items_in_time_order() {
        let (buffer, _) = draw(Texts::en(), 12, 0, 80, 24);
        let rows = rows(&buffer);
        let text = rows.join("\n");
        assert!(text.contains("2026-10-12 Mon"));
        assert!(text.contains("j/k select · q/Esc close"));
        // 有時間的照時間排、沒時間的在後面；沒 checkbox 的也顯示 [ ]；別天的不列
        let early = row_of(&rows, "[x] 09:00 早上的會");
        let late = row_of(&rows, "[ ] 14:00 下午的會");
        let untimed = row_of(&rows, "[ ] 沒時間的事");
        assert_eq!((late, untimed), (early + 1, early + 2));
        assert!(!text.contains("別天"));
    }

    #[test]
    fn selected_row_has_the_background_and_done_items_are_dimmed() {
        let (buffer, app) = draw(Texts::en(), 12, 1, 80, 24);
        let rows = rows(&buffer);
        let selected = row_of(&rows, "14:00 下午的會");
        let done = row_of(&rows, "09:00 早上的會");
        // 框正中間的那一格（字已經結束、是行尾的空白）也有選取的底色：整行都塗
        let middle = buffer.area.width / 2;
        assert_eq!(
            buffer[(middle, selected)].bg,
            app.theme.selected_bg.bg.unwrap()
        );
        assert_ne!(buffer[(middle, done)].bg, app.theme.selected_bg.bg.unwrap());
        // 完成的項目用 item_done 的顏色；字找第一個 `[`
        let x = (0..buffer.area.width)
            .find(|&x| buffer[(x, done)].symbol() == "[")
            .unwrap();
        assert_eq!(buffer[(x, done)].fg, app.theme.item_done.fg.unwrap());
        assert_eq!(buffer[(x, selected)].fg, app.theme.item.fg.unwrap());
    }

    #[test]
    fn empty_day_says_so_and_zh_tw_title() {
        let (buffer, _) = draw(Texts::zh_tw(), 14, 0, 80, 24);
        let text = rows(&buffer).join("\n");
        assert!(text.contains("2026-10-14（三）"));
        assert!(text.contains("這天沒有項目"));
        assert!(text.contains("j/k 選擇 · q/Esc 關閉"));
    }

    #[test]
    fn short_screen_scrolls_to_keep_the_selected_item_visible() {
        // 6 行高：框線 2 + 上下 padding 2 → 只放得下 2 個項目；選第 3 個時，第 1 個捲出去
        let (buffer, _) = draw(Texts::en(), 12, 2, 80, 6);
        let text = rows(&buffer).join("\n");
        assert!(text.contains("沒時間的事"));
        assert!(text.contains("下午的會"));
        assert!(!text.contains("早上的會"));
    }
}
