//! month view：填滿整個 terminal 的大月曆，每天一個大格子
//!
//! 捲動方式跟 compact view 一樣：同一個 `top`、一週一週捲，前後月份的日子灰色；
//! 但只畫 `WEEKS`（6）週，格子才有高度放任務。
//! 格子大小不是固定的：每次畫面重畫都從 terminal 現在的大小算（terminal 拉大 / 縮小，格子跟著變）

use crate::app::{App, View};
use crate::ui::{WEEKDAYS, weekend_style};
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

    // 日期格子：從 `top` 那週開始，一週一行
    for (row, cells) in weeks.into_iter().enumerate() {
        let Ok(monday) = app.top.checked_add((row as i64).weeks()) else {
            break;
        };
        // 這一週的 7 天（只有 9999-12-31 這種日期上限才會不滿 7 天）
        for (cell, date) in cells.into_iter().zip(monday.series(1.day()).take(7)) {
            frame.render_widget(day_cell(date, month, app), cell);
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

/// 一天的格子：第一行是日期（每個月 1 號後面標月份，例：` 1 Oct`），下面的行留給任務
///
/// 只有選取的那格塗底色（其他格子露出 terminal 的底色）；數字再疊週末 / 其他月份 / 今天 / 選取的顏色
fn day_cell(date: Date, month: Date, app: &App) -> Paragraph<'static> {
    let theme = &app.theme;
    let other_month = date.first_of_month() != month;
    let selected = date == app.selected;
    // 整格底色：只有選取的那格
    let cell_style = if selected {
        theme.cell_selected
    } else {
        Style::new()
    };
    // 數字依序疊上 style：一般 → 週末 → 其他月份 → 今天 → 選取（順序見 `Theme`）
    let mut number_style = theme.day.patch(weekend_style(date.weekday(), theme));
    let layers = [
        (other_month, theme.other_month),
        (date == app.today, theme.today),
        (selected, theme.cell_selected_day),
    ];
    for (applies, layer) in layers {
        if applies {
            number_style = number_style.patch(layer);
        }
    }
    let mut line = Line::from(Span::styled(format!(" {:>2} ", date.day()), number_style));
    if date.day() == 1 {
        let marker = (app.texts.month_marker)(date.month());
        // 其他月份的月份標記跟數字一樣變淡
        let mut marker_style = theme.month_marker;
        if other_month {
            marker_style = marker_style.patch(theme.other_month);
        }
        line.push_span(Span::styled(marker, marker_style));
    }
    // `Paragraph::style` 會先把整格塗上底色，文字再畫在上面
    Paragraph::new(line).style(cell_style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Texts;
    use jiff::civil::date;
    use ratatui::{Terminal, backend::TestBackend, style::Color};

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
