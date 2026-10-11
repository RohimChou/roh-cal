//! month view：填滿整個 terminal 的大月曆，每天一個大格子，格子裡列當天的項目
//!
//! 捲動方式跟 compact view 一樣：同一個 `top`、一週一週捲，前後月份的日子灰色；
//! 但只畫 `WEEKS`（6）週，格子才有高度放任務。
//! 格子大小不是固定的：每次畫面重畫都從 terminal 現在的大小算（terminal 拉大 / 縮小，格子跟著變）

use crate::app::{App, View};
use crate::tasks::Item;
use crate::theme::Theme;
use crate::ui::{WEEKDAYS, item_dots, items_by_day, weekend_style};
use jiff::{ToSpan, civil::Date};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
};

/// month view 固定顯示幾週
const WEEKS: i64 = View::Month.weeks();

/// 畫出 month view：圓角框填滿 terminal，裡面是星期列 + `WEEKS` 週，每週 7 個等寬等高的格子
pub fn render(frame: &mut Frame, app: &App) {
    let month = app.selected.first_of_month();
    let title = format!(" {} ", (app.texts.title)(month.year(), month.month()));
    let hint = format!(" {} ", app.texts.hint);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(app.theme.border)
        .title_top(Line::styled(title, app.theme.title).centered())
        .title_bottom(Line::styled(hint, app.theme.hint).centered());

    // 框佔整個 terminal；`inner` 是框線裡面的區域，格子都切在這裡面
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let (header, weeks) = grid(inner);

    // 星期列：每個星期名稱放在自己那欄的最左邊，跟下面的日期對齊
    for ((cell, name), weekday) in header.into_iter().zip(app.texts.weekdays).zip(WEEKDAYS) {
        let style = app
            .theme
            .weekday_header
            .patch(weekend_style(weekday, &app.theme));
        frame.render_widget(Paragraph::new(format!(" {name}")).style(style), cell);
    }

    // 日期格子：從 `top` 那週開始，一週一行；項目先依日期分好組，每格直接拿
    let by_day = items_by_day(&app.items);
    for (row, cells) in weeks.into_iter().enumerate() {
        let Ok(monday) = app.top.checked_add((row as i64).weeks()) else {
            break;
        };
        // 這一週的 7 天（只有 9999-12-31 這種日期上限才會不滿 7 天）
        let dates: Vec<Date> = monday.series(1.day()).take(7).collect();
        let lines: Vec<Vec<Line>> = dates
            .iter()
            .map(|&date| {
                let items = by_day.get(&date).map_or(&[][..], Vec::as_slice);
                cell_lines(date, month, items, cells[0].height, app)
            })
            .collect();
        render_week(frame, &cells, &dates, &lines, app);
    }
}

/// 畫一週的 7 格：先塗底色，再一行一行畫文字
///
/// 文字太長時可以**往右延伸**到隔壁格（像試算表的溢出）：隔壁格同一行沒有東西就借用它的寬度，
/// 一路借到遇見有內容的格子、選取的格子、或這週最右邊為止；再長就截斷
fn render_week(
    frame: &mut Frame,
    cells: &[Rect; 7],
    dates: &[Date],
    lines: &[Vec<Line>],
    app: &App,
) {
    // 底色：只有選取的那格
    for (&cell, &date) in cells.iter().zip(dates) {
        if date == app.selected {
            frame.render_widget(Paragraph::new("").style(app.theme.cell_selected), cell);
        }
    }
    for (column, (&cell, cell_lines)) in cells.iter().zip(lines).enumerate() {
        for (row, line) in cell_lines.iter().enumerate() {
            // 第一行是日期，不延伸；其他行往右看隔壁格這一行是不是空的
            let mut width = cell.width;
            if row > 0 {
                for next in column + 1..cells.len() {
                    let free = lines[next].len() <= row && dates[next] != app.selected;
                    if !free {
                        break;
                    }
                    width += cells[next].width;
                }
            }
            let area = Rect {
                y: cell.y + row as u16,
                width,
                height: 1,
                ..cell
            };
            frame.render_widget(Paragraph::new(line.clone()), area);
        }
    }
}

/// 把框裡面切成格子：最上面 1 行是星期列，下面 `WEEKS` 週**每週一樣高**；每一行再平分成 7 欄
///
/// 每週的高度 = (框裡面的高度 − 星期列) ÷ 週數，取整數；除不盡的零頭留在最底下當空白，
/// 不用 `Fill` 平分（`Fill` 會讓有些週多 1 行，看起來高低不齊）。
/// 回傳（星期列的 7 格, 每週的 7 格）。terminal 太小時格子可能是 0 高 / 0 寬，畫不出東西但不會壞
fn grid(inner: Rect) -> ([Rect; 7], Vec<[Rect; 7]>) {
    let week_height = inner.height.saturating_sub(1) / WEEKS as u16;
    // 直的切：星期列 `Length(1)`，每週 `Length(week_height)`，零頭 `Fill(1)` 吃掉
    let mut constraints = vec![Constraint::Length(1)];
    constraints.extend(std::iter::repeat_n(
        Constraint::Length(week_height),
        WEEKS as usize,
    ));
    constraints.push(Constraint::Fill(1));
    let rows = Layout::vertical(constraints).split(inner);
    // 橫的切：7 欄平分（寬度差 1 格看不出來）
    let columns = |row: Rect| Layout::horizontal([Constraint::Fill(1); 7]).areas::<7>(row);
    let header = columns(rows[0]);
    let weeks = rows[1..=WEEKS as usize]
        .iter()
        .map(|&row| columns(row))
        .collect();
    (header, weeks)
}

/// 一格裡的每一行：第一行日期（每個月 1 號後面標月份，例：` 1 Oct`），接著當天的項目
///
/// 空間規則（`rows` = 格子有幾行）：
/// - 項目放得下就全列；放不下就少列一個，最後一行改成 `+N`（N = 沒列出來的數量）
/// - 格子只有 1 行時不列項目，改在日期旁邊畫點點（跟 compact view 一樣，橘 = 有沒完成的任務）
/// - 標題太長：先往右延伸（見 `render_week`），還是放不下就截斷，不加 `…`（那個字寬度不固定，會把月曆弄歪）
///
/// 顏色：其他月份整格（數字、標記、項目）都變灰；數字再疊今天 / 選取。
/// 項目不設底色，畫在選取的格子上時自然露出格子的底色
fn cell_lines(
    date: Date,
    month: Date,
    items: &[&Item],
    rows: u16,
    app: &App,
) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let other_month = date.first_of_month() != month;
    // 其他月份：每個 span 都疊這個；本月：空的 Style，疊了沒影響
    let dim = if other_month {
        theme.other_month
    } else {
        Style::new()
    };

    // 第一行：數字依序疊上 style：一般 → 週末 → 其他月份 → 今天 → 選取（順序見 `Theme`）
    let mut number_style = theme
        .day
        .patch(weekend_style(date.weekday(), theme))
        .patch(dim);
    let mut dots_style = if items.iter().any(|item| !item.done) {
        theme.dots_open
    } else {
        theme.dots_quiet
    }
    .patch(dim);
    let layers = [
        (date == app.today, theme.today),
        (date == app.selected, theme.cell_selected_day),
    ];
    for (applies, layer) in layers {
        if applies {
            number_style = number_style.patch(layer);
            dots_style = dots_style.patch(layer);
        }
    }
    let mut first = Line::from(Span::styled(format!(" {:>2}", date.day()), number_style));
    let rows = usize::from(rows);
    if rows <= 1 {
        // 只有 1 行（視窗很矮）：項目列不下，用點點表示有幾個
        first.push_span(Span::styled(item_dots(items.len()), dots_style));
        return vec![first];
    }
    first.push_span(Span::styled(" ", number_style));
    if date.day() == 1 {
        let marker = (app.texts.month_marker)(date.month());
        first.push_span(Span::styled(marker, theme.month_marker.patch(dim)));
    }

    // 接下來的行：放得下就全列，放不下就少列一個、最後一行 `+N`
    let available = rows - 1;
    let shown = if items.len() <= available {
        items.len()
    } else {
        available - 1
    };
    let mut lines = vec![first];
    for item in &items[..shown] {
        lines.push(item_line(item, dim, theme));
    }
    if shown < items.len() {
        let more = format!(" +{}", items.len() - shown);
        lines.push(Line::styled(more, theme.item_more.patch(dim)));
    }
    lines
}

/// 一個項目一行：`14:00 標題`；不加任何符號（格子的寬度很寶貴），完成的用顏色分：灰 + 劃線。時間跟標題同色
fn item_line(item: &Item, dim: Style, theme: &Theme) -> Line<'static> {
    let style = if item.done {
        theme.item_done
    } else {
        theme.item
    }
    .patch(dim);
    let mut text = String::from(" ");
    if let Some(time) = item.time {
        text.push_str(&format!("{:02}:{:02} ", time.hour(), time.minute()));
    }
    text.push_str(&item.title);
    // style 掛在 span 上（不是 Line 上），跟其他行一致，test 也從 span 讀
    Line::from(Span::styled(text, style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Texts;
    use jiff::civil::date;
    use ratatui::{
        Terminal,
        backend::TestBackend,
        style::{Color, Modifier},
    };

    #[test]
    fn grid_has_one_header_row_and_equal_cells() {
        // 70 寬 × 25 高：星期列 1 行，剩下 24 行平分 6 週 = 每週 4 行；70 平分 7 欄 = 每欄 10 格
        let (header, weeks) = grid(Rect::new(1, 1, 70, 25));
        assert_eq!(weeks.len(), 6);
        assert!(
            header
                .iter()
                .all(|cell| cell.height == 1 && cell.width == 10)
        );
        for cells in &weeks {
            assert!(
                cells
                    .iter()
                    .all(|cell| cell.height == 4 && cell.width == 10)
            );
        }
        // 第 1 週星期一的格子從星期列下面開始；第 2 週接在第 1 週下面
        assert_eq!(weeks[0][0], Rect::new(1, 2, 10, 4));
        assert_eq!(weeks[1][0], Rect::new(1, 6, 10, 4));
        // 星期日那欄貼到最右邊
        assert_eq!(weeks[0][6].right(), 71);
    }

    #[test]
    fn leftover_rows_stay_blank_at_the_bottom() {
        // 27 高：26 ÷ 6 = 4 餘 2 → 每週還是 4 行，最後 2 行空著（不會有某週變 5 行）
        let (_, weeks) = grid(Rect::new(1, 1, 70, 27));
        assert!(weeks.iter().all(|cells| cells[0].height == 4));
        assert_eq!(weeks[5][0].bottom(), 1 + 1 + 6 * 4);
    }

    /// 把一行的 span 接成字串（方便比對）
    fn text(line: &Line) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    /// 2026-10-12 的 4 個項目：早上（完成）、下午、兩個沒時間也沒 checkbox 的
    fn sample() -> Vec<Item> {
        let text = "- [ ] 下午 @2026-10-12 14:00\n- 沒時間 A @2026-10-12\n- [x] 早上 @2026-10-12 09:00\n\
                    - 沒時間 B @2026-10-12\n- [ ] 隔天 @2026-10-13";
        crate::tasks::parse(text).items
    }

    #[test]
    fn cell_lists_items_or_shows_how_many_are_hidden() {
        let app = App::on_date(date(2026, 10, 10), Texts::en(), sample());
        let by_day = items_by_day(&app.items);
        let items = &by_day[&date(2026, 10, 12)];
        let lines = |rows| cell_lines(date(2026, 10, 12), date(2026, 10, 1), items, rows, &app);
        // 5 行：日期 + 4 個項目，剛好放滿；順序照時間、沒時間的在後
        let full = lines(5);
        assert_eq!(full.len(), 5);
        assert_eq!(text(&full[0]), " 12 ");
        assert_eq!(text(&full[1]), " 09:00 早上");
        assert_eq!(text(&full[2]), " 14:00 下午");
        assert_eq!(text(&full[3]), " 沒時間 A");
        assert_eq!(text(&full[4]), " 沒時間 B");
        // 6 行：放得下，不會多一行
        assert_eq!(lines(6).len(), 5);
        // 4 行：少列一個，最後一行 +2
        let short = lines(4);
        assert_eq!(short.len(), 4);
        assert_eq!(text(&short[2]), " 14:00 下午");
        assert_eq!(text(&short[3]), " +2");
        // 2 行：一個都列不下，只有 +4
        let tiny = lines(2);
        assert_eq!(tiny.len(), 2);
        assert_eq!(text(&tiny[1]), " +4");
        // 1 行：日期旁邊畫點點（4 個項目 = ⠶），有沒完成的任務 → 橘色
        let one = lines(1);
        assert_eq!(one.len(), 1);
        assert_eq!(text(&one[0]), " 12⠶");
        assert_eq!(one[0].spans[1].style.fg, app.theme.dots_open.fg);
        // 沒項目：只有日期
        let empty = cell_lines(date(2026, 10, 14), date(2026, 10, 1), &[], 4, &app);
        assert_eq!(empty.len(), 1);
        assert_eq!(text(&empty[0]), " 14 ");
    }

    #[test]
    fn item_colors_follow_done_and_other_month() {
        let app = App::on_date(date(2026, 10, 10), Texts::en(), sample());
        let by_day = items_by_day(&app.items);
        let items = &by_day[&date(2026, 10, 12)];
        // 本月：完成的灰 + 劃線、其他一般色（沒 checkbox 的也是）；整行（含時間）一個 span 同色
        let lines = cell_lines(date(2026, 10, 12), date(2026, 10, 1), items, 5, &app);
        let style = |line: &Line| line.spans[0].style;
        assert_eq!(style(&lines[1]).fg, app.theme.item_done.fg);
        assert!(
            style(&lines[1])
                .add_modifier
                .contains(Modifier::CROSSED_OUT)
        );
        assert_eq!(style(&lines[2]).fg, app.theme.item.fg);
        assert!(
            !style(&lines[2])
                .add_modifier
                .contains(Modifier::CROSSED_OUT)
        );
        assert_eq!(style(&lines[3]).fg, app.theme.item.fg);
        assert!(lines[1..].iter().all(|line| line.spans.len() == 1));
        // 其他月份（選取的是 9 月）：每個 span 都變灰
        let dimmed = cell_lines(date(2026, 10, 12), date(2026, 9, 1), items, 5, &app);
        for line in &dimmed {
            for span in &line.spans {
                assert_eq!(
                    span.style.fg, app.theme.other_month.fg,
                    "{:?}",
                    span.content
                );
            }
        }
    }

    /// 畫出 80 × 24 的大月曆，回傳畫面的 buffer
    fn draw(app: &App) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    /// 從 `cell` 的第 `row` 行開始往右讀 `width` 格（可以超過格子的寬度，用來看溢出）
    fn text_from(buffer: &ratatui::buffer::Buffer, cell: Rect, row: u16, width: u16) -> String {
        (cell.x..cell.x + width)
            .map(|x| buffer[(x, cell.y + row)].symbol())
            .collect()
    }

    #[test]
    fn long_titles_stop_at_a_neighbour_with_content() {
        // 中文標題比格子寬，隔壁 10/13 同一行有東西：截在格子邊界，不會畫到隔壁格
        let text = "- [ ] 很長很長的中文標題會超過格子的寬度 @2026-10-12\n- 短 @2026-10-13";
        let mut app = App::on_date(
            date(2026, 10, 10),
            Texts::en(),
            crate::tasks::parse(text).items,
        );
        app.view = View::Month;
        let buffer = draw(&app);
        let (_, weeks) = grid(Rect::new(1, 1, 78, 22));
        // 10/12 是第 4 週星期一、10/13 星期二；項目在格子的第 2 行
        let monday = weeks[3][0];
        let tuesday = weeks[3][1];
        // 格子 11 格寬：開頭空白 1 格 + 5 個中文字各 2 格 = 11，第 6 個字放不下就不畫
        // （buffer 裡中文字佔 2 格，第 2 格是空白，所以讀出來是「很 長 很 長 的」）
        assert_eq!(
            text_from(&buffer, monday, 1, monday.width),
            " 很 長 很 長 的 "
        );
        assert_eq!(text_from(&buffer, tuesday, 1, 3), " 短 ");
    }

    #[test]
    fn long_titles_spill_into_empty_neighbours() {
        // 10/13 這一行沒東西、10/14 有：10/12 的標題延伸過 10/13，停在 10/14 前面
        let text = "- [ ] 很長很長的中文標題會超過格子的寬度 @2026-10-12\n- 短 @2026-10-14\n\
                    - [ ] 第二行只有自己 @2026-10-12";
        let mut app = App::on_date(
            date(2026, 10, 10),
            Texts::en(),
            crate::tasks::parse(text).items,
        );
        app.view = View::Month;
        let buffer = draw(&app);
        let (_, weeks) = grid(Rect::new(1, 1, 78, 22));
        let monday = weeks[3][0];
        let wednesday = weeks[3][2];
        // 兩格 22 格寬：空白 1 + 10 個中文字 20 = 21，第 11 個字放不下（剩 1 格空白）
        assert_eq!(
            text_from(&buffer, monday, 1, 22),
            " 很 長 很 長 的 中 文 標 題 會  "
        );
        assert_eq!(text_from(&buffer, wednesday, 1, 3), " 短 ");
        // 第二行也延伸：10/13、10/14 的第 2 行都沒東西，整句（15 格）寫得完
        assert_eq!(text_from(&buffer, monday, 2, 16), " 第 二 行 只 有 自 己  ");
        // 選取的格子不借：選 10/13 之後第一行就截在 10/12 的邊界
        app.selected = date(2026, 10, 13);
        let buffer = draw(&app);
        assert_eq!(
            text_from(&buffer, monday, 1, monday.width + 1),
            " 很 長 很 長 的  "
        );
    }

    #[test]
    fn sample_file_fills_cells_in_time_order_with_overflow_count() {
        // 預期值用另一支 Python 腳本數 testdata/tasks.md 得到：
        // 10/12 有 2 個（10:00 週會、14:00 Code review）；10/16 有 5 個（17:30 交週報、18:30 團隊聚餐、3 個沒時間的）
        let items =
            crate::tasks::parse(&std::fs::read_to_string("testdata/tasks.md").unwrap()).items;
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), items);
        app.view = View::Month;
        // 80 × 30：框內 28 行 − 星期列 = 27 ÷ 6 = 每格 4 行（日期 + 3 行）
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let (_, weeks) = grid(Rect::new(1, 1, 78, 28));
        let line_at = |cell: Rect, row: u16| -> String {
            (cell.x..cell.right())
                .map(|x| buffer[(x, cell.y + row)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        };
        // 10/12：第 4 週星期一，2 個都放得下，照時間排
        let mon = weeks[3][0];
        assert_eq!(line_at(mon, 1), " 10:00 週 會");
        assert_eq!(line_at(mon, 2), " 14:00 Code");
        assert_eq!(line_at(mon, 3), "");
        // 10/16：星期五，5 個放不下：列 2 個 + "+3"
        let fri = weeks[3][4];
        assert_eq!(line_at(fri, 1), " 17:30 交 週");
        assert_eq!(line_at(fri, 2), " 18:30 團 隊");
        assert_eq!(line_at(fri, 3), " +3");
    }

    #[test]
    fn month_view_draws_dates_and_cell_backgrounds() {
        let mut app = App::on_date(date(2026, 10, 10), Texts::en(), Vec::new());
        app.view = View::Month;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        // 格子的位置跟 render 一樣算：框線各佔 1
        let (header, weeks) = grid(Rect::new(1, 1, 78, 22));
        let text_at = |cell: Rect, width: u16| -> String {
            (cell.x..cell.x + width)
                .map(|x| buffer[(x, cell.y)].symbol())
                .collect()
        };
        assert_eq!(text_at(header[0], 4), " MON");
        assert_eq!(text_at(header[6], 4), " SUN");
        // 一打開最上面那週從 9/21 開始（跟 compact 一樣），10/1 是第 2 週星期四，後面標月份
        for (row, cells) in weeks.iter().enumerate() {
            for (column, &cell) in cells.iter().enumerate() {
                let expected = date(2026, 9, 21) + ((row * 7 + column) as i32).days();
                assert_eq!(text_at(cell, 3), format!(" {:>2}", expected.day()));
            }
        }
        assert_eq!(text_at(weeks[1][3], 7), "  1 Oct");
        // 11/1 在其他月份：月份標記跟數字一樣變淡
        let nov = weeks[5][6];
        assert_eq!(text_at(nov, 7), "  1 Nov");
        assert_eq!(
            buffer[(nov.x + 4, nov.y)].fg,
            app.theme.other_month.fg.unwrap()
        );
        assert_eq!(
            buffer[(weeks[1][3].x + 4, weeks[1][3].y)].fg,
            app.theme.month_marker.fg.unwrap()
        );
        // 選取的 10/10（第 3 週星期六）整格都是 `cell_selected` 的底色：左上角到右下角
        let cell = weeks[2][5];
        let selected_bg = app.theme.cell_selected.bg.unwrap();
        assert_eq!(buffer[(cell.x, cell.y)].bg, selected_bg);
        assert_eq!(
            buffer[(cell.right() - 1, cell.bottom() - 1)].bg,
            selected_bg
        );
        // 其他格子沒有底色：10/5（本月）、9/21（其他月份）
        assert_eq!(buffer[(weeks[2][0].x, weeks[2][0].y)].bg, Color::Reset);
        assert_eq!(buffer[(weeks[0][0].x, weeks[0][0].y)].bg, Color::Reset);
    }
}
