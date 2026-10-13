//! 當天清單：月曆上按 `Enter` / `o` 打開，疊在月曆框的正中間，列出那天的項目

use crate::app::{App, DayList};
use crate::tasks::Item;
use crate::ui::{calendar_area, items_on, notice_line};
use jiff::civil::Date;
use ratatui::{
    Frame,
    layout::{Constraint, Position, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, ListState, Padding, Paragraph},
};
use std::collections::BTreeSet;

/// 框的寬度（含框線）：月曆框左右各留 `MARGIN` 格，但最多 `MAX_WIDTH`（大月曆在很寬的 terminal 上才不會太長）
const MARGIN: u16 = 2;
const MAX_WIDTH: u16 = 60;
/// 框線和內容之間上下左右各空 1 格（選到那行的底色條也跟框線隔開）
const PADDING: Padding = Padding::uniform(1);

/// 畫當天清單：一個項目一行，選到的那行整行塗底色；日期在上框線、操作說明在下框線
/// （打勾沒寫成時，下框線暫時換成提示）。
/// `calendar` = 月曆框的位置（小月曆在左上角、大月曆是整個 terminal），清單疊在它的正中間
pub fn render(frame: &mut Frame, list: DayList, app: &App, calendar: Rect) {
    let theme = &app.theme;
    let rows = list_rows(&app.items, list.date, &app.expanded);
    let area = area(rows.len(), calendar);

    let title = format!(" {} ", (app.texts.day_title)(list.date));
    // 下框線：按了 d 就問要不要刪；有提示就顯示提示（下一個按鍵就恢復）；平常是操作說明
    let bottom = match list.confirm_delete {
        Some(count) => {
            // count 含它自己，問的時候講「連同幾個子任務」
            let question = (app.texts.delete_confirm)(count.saturating_sub(1));
            Line::styled(format!(" {question} "), theme.title)
        }
        None => notice_line(app)
            .unwrap_or_else(|| Line::styled(format!(" {} ", app.texts.day_hint), theme.hint)),
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .padding(PADDING)
        .title_top(Line::styled(title, theme.title))
        .title_bottom(bottom);
    // 先清掉底下的畫面，框才不會跟月曆的字疊在一起
    frame.render_widget(Clear, area);
    if rows.is_empty() {
        let empty = Line::styled(format!(" {}", app.texts.day_empty), theme.hint);
        frame.render_widget(Paragraph::new(empty).block(block), area);
        return;
    }
    let rows: Vec<ListItem> = rows
        .iter()
        .map(|row| ListItem::new(item_line(row, Some(list.date), &app.expanded, app)))
        .collect();
    // List 會自己捲動（選到的那行一定在框內），選到的那行整行塗上 highlight_style
    let mut state = ListState::default().with_selected(Some(list.selected));
    let widget = List::new(rows)
        .block(block)
        .highlight_style(theme.selected_bg);
    frame.render_stateful_widget(widget, area, &mut state);
}

/// 清單的框（含框線）放在哪：疊在月曆框正中間；`count` = 那天有幾個項目
fn area(count: usize, calendar: Rect) -> Rect {
    // 大小：框線 2 + 上下 padding + 項目（沒有項目時是 1 行說明）；月曆框不夠高就縮（Length 超過月曆框會被縮到月曆框的大小）
    let width = calendar.width.saturating_sub(2 * MARGIN).min(MAX_WIDTH);
    let height = 2 + PADDING.top + PADDING.bottom + count.max(1) as u16;
    calendar.centered(Constraint::Length(width), Constraint::Length(height))
}

/// 清單的框（含框線）在畫面上的範圍（點框外 = 關掉）
pub fn bounds(list: DayList, app: &App) -> Rect {
    let count = list_rows(&app.items, list.date, &app.expanded).len();
    area(count, calendar_area(app.screen, app))
}

/// 滑鼠點在 (`column`, `row`) 的是清單的第幾行（`list_rows` 的 index）；點在框線、padding、框外，或那天沒有項目，回傳 `None`。
/// 位置跟 `render` 用同一個 `area` 算。捲動照 ratatui `List` 的規則（ratatui-widgets 0.3 的 `get_items_bounds`）：
/// 從第一個開始畫，選到的超出框底才往下捲，捲到它剛好在最後一行
pub fn item_at(list: DayList, app: &App, column: u16, row: u16) -> Option<usize> {
    let count = list_rows(&app.items, list.date, &app.expanded).len();
    // 項目畫在框線和 padding 裡面
    let inner = Block::bordered().padding(PADDING).inner(bounds(list, app));
    if !inner.contains(Position::new(column, row)) {
        return None;
    }
    let first = list
        .selected
        .saturating_sub(usize::from(inner.height).saturating_sub(1));
    let index = first + usize::from(row - inner.y);
    (index < count).then_some(index)
}

/// 當天清單的一行：哪個項目、往右縮幾層（0 = 那天的項目，1 = 它的子任務…）
pub struct ListRow<'a> {
    pub item: &'a Item,
    pub level: usize,
}

/// 當天清單一行一行列出來：那天的項目照 `items_on` 的順序；
/// 展開的（`expanded` 記行號）底下接著列它的子任務，子任務也展開的再往下列
pub fn list_rows<'a>(
    items: &'a [Item],
    date: Date,
    expanded: &BTreeSet<usize>,
) -> Vec<ListRow<'a>> {
    let mut rows = Vec::new();
    for item in items_on(items, date) {
        push_tree(&mut rows, items, item, 0, expanded);
    }
    rows
}

/// 把 `item` 放進 `rows`；它展開了就接著放它的子任務（一路往下）
pub(super) fn push_tree<'a>(
    rows: &mut Vec<ListRow<'a>>,
    items: &'a [Item],
    item: &'a Item,
    level: usize,
    expanded: &BTreeSet<usize>,
) {
    rows.push(ListRow { item, level });
    if expanded.contains(&item.line) {
        for child in children(items, item) {
            push_tree(rows, items, child, level + 1, expanded);
        }
    }
}

/// `parent` 底下一層的子任務（不含子任務的子任務），照檔案順序
pub fn children<'a>(items: &'a [Item], parent: &Item) -> Vec<&'a Item> {
    // 父項目在 items 的第幾個：行號不會重複，用行號找
    let Some(index) = items.iter().position(|item| item.line == parent.line) else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|item| item.parent == Some(index))
        .collect()
}

/// 一個項目一行：` ▸ [ ] 14:00 標題 1/4`（沒寫 checkbox 的也顯示 `[ ]`，每個項目都能打勾）；
/// 完成的用 `item_done`（灰 + 劃線）。顏色掛在 span 上，劃線才不會延伸到行尾的空白
///
/// - 有子任務的前面是 `▸`（收起）/ `▾`（展開），後面淡色的 `1/4` = 子任務完成幾個 / 共幾個；沒有子任務的空一格，checkbox 才對齊
/// - 子任務單獨出現（它自己有這天的日期、父項目在別天）：標題前面用淡色帶上父項目，例：`實作退款 API › 寫 integration test`
/// - 項目有日期、又不是這天：後面用淡色標日期，例：`10/13`
///
/// `date` = 清單是哪一天的；任務 view 是 `None`：時間不放前面，跟日期一起放在後面，例：`10/14 10:30`。
/// `expanded` = 哪些項目展開了（當天清單、任務 view 各記各的）
pub(super) fn item_line(
    row: &ListRow,
    date: Option<Date>,
    expanded: &BTreeSet<usize>,
    app: &App,
) -> Line<'static> {
    let theme = &app.theme;
    let item = row.item;
    let children = children(&app.items, item);
    let marker = match (children.is_empty(), expanded.contains(&item.line)) {
        (true, _) => ' ',
        (false, false) => '▸',
        (false, true) => '▾',
    };
    let style = if item.done {
        theme.item_done
    } else {
        theme.item
    };
    // 補充資訊（`1/4`、父項目、別天的日期）用比標題暗的灰，才不會被當成標題的一部分
    let meta = theme.row_distance;
    let indent = "  ".repeat(row.level);
    let mark = if item.done { "[x]" } else { "[ ]" };
    let mut text = format!("{mark} ");
    if let Some(time) = item.time
        && date.is_some()
    {
        text.push_str(&format!("{:02}:{:02} ", time.hour(), time.minute()));
    }

    let mut spans = vec![
        Span::raw(format!(" {indent}")),
        Span::styled(format!("{marker} "), theme.hint),
        Span::styled(text, style),
    ];
    // 單獨出現的子任務：帶上父項目的標題
    if row.level == 0
        && let Some(parent) = item.parent.and_then(|index| app.items.get(index))
    {
        spans.push(Span::styled(format!("{} › ", parent.title), meta));
    }
    spans.push(Span::styled(item.title.clone(), style));
    if !children.is_empty() {
        let done = children.iter().filter(|child| child.done).count();
        spans.push(Span::styled(format!("  {done}/{}", children.len()), meta));
    }
    // 排在別天（任務 view 是每個有日期的）：標日期；任務 view 連時間一起標
    if let Some(day) = item.date
        && Some(day) != date
    {
        let mut when = format!("  {}/{}", day.month(), day.day());
        if let Some(time) = item.time
            && date.is_none()
        {
            when.push_str(&format!(" {:02}:{:02}", time.hour(), time.minute()));
        }
        spans.push(Span::styled(when, meta));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Notice, Overlay};
    use crate::i18n::Texts;
    use crate::tasks::parse;
    use crate::ui::rows;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    /// 測試用的任務：10/12 有三個（順序故意打亂）、10/13 有一個
    const TASKS: &str = "- [ ] 下午的會 @2026-10-12 14:00\n\
                         - [x] 早上的會 @2026-10-12 09:00\n\
                         - 沒時間的事 @2026-10-12\n\
                         - [ ] 別天 @2026-10-13";

    /// 打開 `day` 那天的清單、選第 `selected` 個，畫在 `width` × `height` 的畫面上
    fn draw(texts: Texts, day: i8, selected: usize, width: u16, height: u16) -> (Buffer, App) {
        draw_with_notice(texts, day, selected, None, width, height)
    }

    /// 同 `draw`，下框線帶著提示 `notice`
    fn draw_with_notice(
        texts: Texts,
        day: i8,
        selected: usize,
        notice: Option<Notice>,
        width: u16,
        height: u16,
    ) -> (Buffer, App) {
        let mut app = App::on_date(date(2026, 10, 10), texts, parse(TASKS).items);
        app.overlay = Some(Overlay::DayList(DayList {
            date: date(2026, 10, day),
            selected,
            confirm_delete: None,
        }));
        app.notice = notice;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        (terminal.backend().buffer().clone(), app)
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
        assert!(text.contains("a add · Space check · d delete · ? help"));
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
        assert!(text.contains("a 新增 · Space 打勾 · d 刪除 · ? 說明"));
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

    #[test]
    fn notice_takes_the_place_of_the_hint() {
        let (buffer, _) = draw_with_notice(Texts::en(), 12, 0, Some(Notice::Reloaded), 80, 24);
        let text = rows(&buffer).join("\n");
        assert!(text.contains("file changed elsewhere, reloaded"));
        assert!(!text.contains("Space check"));
        // 清單開著時提示只在清單上，月曆的下框線照常是操作說明
        assert!(text.contains("hjkl move"));
        let denied = Some(Notice::Failed(std::io::ErrorKind::PermissionDenied));
        let (buffer, _) = draw_with_notice(Texts::zh_tw(), 12, 0, denied, 80, 24);
        let text = rows(&buffer).join("\n");
        assert!(text.contains("任務檔存不了：permission denied"));
    }

    #[test]
    fn notice_goes_to_the_calendar_when_no_list_is_open() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::zh_tw(), parse(TASKS).items);
        app.notice = Some(Notice::Undone("早上的會".into()));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let text = rows(terminal.backend().buffer()).join("\n");
        assert!(text.contains("已復原：早上的會"));
        assert!(!text.contains("hjkl 移動"));
    }

    #[test]
    fn delete_question_fits_on_the_compact_list() {
        // 小月曆（80 × 24 的 terminal）裡的清單：整句問題都看得到，操作說明不見
        for (texts, question) in [
            (Texts::en(), "delete with 2 subtasks? y = yes, other = no"),
            (Texts::zh_tw(), "刪除（含 2 個子任務）？y = 是，其他鍵 = 否"),
        ] {
            let mut app = App::on_date(date(2026, 10, 10), texts, parse(TASKS).items);
            app.overlay = Some(Overlay::DayList(DayList {
                date: date(2026, 10, 12),
                selected: 0,
                confirm_delete: Some(3),
            }));
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| crate::ui::render(frame, &app))
                .unwrap();
            let text = rows(terminal.backend().buffer()).join("\n");
            assert!(text.contains(question), "{question}");
            assert!(!text.contains("d delete") && !text.contains("d 刪除"));
        }
    }

    #[test]
    fn item_at_finds_the_item_under_the_mouse_even_when_scrolled() {
        // 10/12 有 15 個項目：小月曆的框放不下，選第 13 個（index 12）時清單往下捲
        let text: String = (1..=15)
            .map(|n| format!("- [ ] item {n:02} @2026-10-12\n"))
            .collect();
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), parse(&text).items);
        app.screen = Rect::new(0, 0, 80, 24);
        let list = DayList {
            date: date(2026, 10, 12),
            selected: 12,
            confirm_delete: None,
        };
        app.overlay = Some(Overlay::DayList(list));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let screen = rows(terminal.backend().buffer());
        // 畫面上看得到的每一個項目（從畫面上的字認出是第幾個）：點它、點那一行右邊的空白，都是它
        let mut seen = Vec::new();
        for (y, line) in screen.iter().enumerate() {
            let Some(start) = line.find("[ ] item ") else {
                continue;
            };
            let number: usize = line[start + 9..start + 11].parse().unwrap();
            let x = u16::try_from(line[..start].chars().count()).unwrap();
            let y = u16::try_from(y).unwrap();
            assert_eq!(item_at(list, &app, x, y), Some(number - 1), "item {number}");
            assert_eq!(
                item_at(list, &app, x + 30, y),
                Some(number - 1),
                "item {number}"
            );
            seen.push(number);
        }
        // 真的有捲：第一個看不到，選到的第 13 個看得到
        assert!(!seen.contains(&1) && seen.contains(&13), "{seen:?}");
        // 上框線（有日期的那行）、上 padding、左框線、左 padding、框外：都不是項目
        let top = screen
            .iter()
            .position(|line| line.contains("2026-10-12 Mon"))
            .unwrap();
        let left = u16::try_from(
            screen[top][..screen[top].find('╭').unwrap()]
                .chars()
                .count(),
        )
        .unwrap();
        let (top, first) = (u16::try_from(top).unwrap(), u16::try_from(top + 2).unwrap());
        for (x, y) in [
            (left + 4, top),
            (left + 4, top + 1),
            (left, first),
            (left + 1, first),
            (0, 0),
        ] {
            assert_eq!(item_at(list, &app, x, y), None, "({x}, {y})");
        }
        // 沒有項目的那天：哪裡都不是項目
        let empty = DayList {
            date: date(2026, 10, 11),
            ..list
        };
        for (x, y) in (0..80).flat_map(|x| (0..24).map(move |y| (x, y))) {
            assert_eq!(item_at(empty, &app, x, y), None);
        }
    }
}
