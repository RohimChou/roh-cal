//! 任務頁（`4`）：side menu 右邊整塊，整份任務檔照日曆列出來，子任務排成樹；最下面一列是選到的那一行原本的文字

use super::{
    Click,
    months::scrolled_top,
    planner::{date_text, day_color},
};
use crate::{
    app::{App, Focus, TaskRow},
    tasks::{Item, Kind},
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Modifier, Style},
    symbols::line,
    text::{Line, Span},
    widgets::{Block, BorderType},
};

/// 選到的那件離上下邊緣至少留幾列，不夠才捲（跟版型 1 一樣）
const SCROLLOFF: i64 = 2;

/// 任務頁框裡面怎麼切：表頭（一列字 + 一條線）、中間一列一件事、下面一條線 + 選到的那一行原本的文字；
/// 畫畫面和滑鼠換算位置都用這個。回傳（表頭, 表頭的線, 中間, 下面的線, 最下面那列）；太矮時中間是空的
fn inner_areas(area: Rect) -> [Rect; 5] {
    let inner = Block::bordered().inner(area);
    let row = |y: u16| Rect {
        y,
        height: 1,
        ..inner
    };
    if inner.height < 5 {
        return [
            row(inner.y),
            Rect::default(),
            Rect::default(),
            Rect::default(),
            Rect::default(),
        ];
    }
    let body = Rect {
        y: inner.y + 2,
        height: inner.height - 4,
        ..inner
    };
    [
        row(inner.y),
        row(inner.y + 1),
        body,
        row(inner.bottom() - 2),
        row(inner.bottom() - 1),
    ]
}

/// 畫任務頁：先捲到選到的那件看得到（記在 `app.tasks_top`），再一列一列畫
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let rows = app.tasks_rows();
    let [header, rule, body, bottom_rule, detail] = inner_areas(area);
    let height = i64::from(body.height);
    let cursor = app.tasks_cursor(&rows);
    if let Some(cursor) = cursor {
        let top = scrolled_top(
            i64::try_from(app.tasks_top).unwrap_or(0),
            i64::try_from(cursor).unwrap_or(0),
            height,
            SCROLLOFF,
        );
        // 不捲過頭：最後一列貼著底就好
        let max = i64::try_from(rows.len()).unwrap_or(0) - height;
        app.tasks_top = usize::try_from(top.min(max).max(0)).unwrap_or(0);
    }
    // 接下來只讀
    let app = &*app;
    let theme = &app.theme;
    let texts = app.lang.texts();
    let focused = app.focus == Focus::Tasks;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { theme.focus } else { theme.line }));
    frame.render_widget(block, area);
    frame.render_widget(
        Line::styled(texts.header_tasks, Style::new().fg(theme.muted)),
        header,
    );
    // 表頭右邊：任務檔有寫錯的地方就寫幾處（紅字）
    if !app.warnings.is_empty() {
        frame.render_widget(
            Line::from(Span::styled(
                texts.mistakes(app.warnings.len()),
                Style::new().fg(theme.error),
            ))
            .right_aligned(),
            header,
        );
    }
    for area in [rule, bottom_rule] {
        frame.render_widget(
            Line::styled(
                line::HORIZONTAL.repeat(usize::from(area.width)),
                Style::new().fg(theme.line),
            ),
            area,
        );
    }
    let visible = rows.iter().enumerate().skip(app.tasks_top);
    for ((index, row), y) in visible.zip(body.y..body.bottom()) {
        let area = Rect {
            y,
            height: 1,
            ..body
        };
        match row {
            TaskRow::Heading { name, level } => {
                let name = name.as_deref().unwrap_or(texts.ungrouped);
                let indent = "  ".repeat(level.saturating_sub(1));
                frame.render_widget(
                    Line::styled(
                        format!(" {indent}{name}"),
                        Style::new().fg(theme.label).add_modifier(Modifier::BOLD),
                    ),
                    area,
                );
            }
            TaskRow::Item {
                index: item_index,
                children,
                expanded,
            } => {
                let Some(item) = app.items.get(*item_index) else {
                    continue;
                };
                render_item(frame, area, app, item, *item_index, (*children, *expanded));
                // 選到的那件整列墊底色（鍵盤在任務頁時）
                if focused && cursor == Some(index) {
                    frame
                        .buffer_mut()
                        .set_style(area, Style::new().bg(theme.cursor_line));
                }
            }
        }
    }
    // 最下面：選到的那一行原本的文字（看得到檔案裡怎麼寫）；那一行寫錯了就改說哪裡錯（紅字）
    if let Some(item) = app.tasks_chosen() {
        let line = match app
            .warnings
            .iter()
            .find(|warning| warning.line == item.line)
        {
            Some(warning) => Line::styled(
                texts.tasks_mistake(item.line, &warning.kind),
                Style::new().fg(theme.error),
            ),
            None => Line::styled(
                texts.tasks_detail(item.line, item.line_text.trim()),
                Style::new().fg(theme.muted),
            ),
        };
        frame.render_widget(line, detail);
    }
}

/// 一件事：「 ▸ [ ] 實作退款 API  1/4            10/14 」：左邊是縮排（一層 2 格）、`▸` / `▾`（有子任務：收著 / 展開）、
/// `[ ]` / `[x]`（任務）或 ` ∙ `（行程）、標題、子任務做完幾件；右邊靠右是日期和時間（沒日期就空著）
///
/// 顏色：結束了的（做完的任務、過去的行程）灰、做完的標題劃線；寫錯的那一行標題紅字；日期用那天在月曆上的顏色；
/// `▸`、子任務的件數、時間淡色
fn render_item(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    item: &Item,
    index: usize,
    (children, expanded): (bool, bool),
) {
    let theme = &app.theme;
    let open = item.is_open(app.today);
    let color = if open { theme.text } else { theme.muted };
    let marker = match (children, expanded) {
        (false, _) => " ",
        (true, false) => "▸",
        (true, true) => "▾",
    };
    let check = match (item.kind, item.done) {
        (Kind::Task, false) => "[ ]",
        (Kind::Task, true) => "[x]",
        (Kind::Event, _) => " ∙ ",
    };
    // 那一行有寫錯的地方：標題紅字
    let mistake = app.warnings.iter().any(|warning| warning.line == item.line);
    let mut title = Style::new().fg(if mistake { theme.error } else { color });
    if item.done {
        title = title.add_modifier(Modifier::CROSSED_OUT);
    }
    // 子任務（任務才算）：(做完幾件, 共幾件)
    let (done, total) = app
        .items
        .iter()
        .filter(|other| other.parent == Some(index) && other.kind == Kind::Task)
        .fold((0, 0), |(done, total), subtask| {
            (done + usize::from(subtask.done), total + 1)
        });
    let mut left = vec![
        Span::styled(
            format!(" {}{marker} ", "  ".repeat(item.depth)),
            Style::new().fg(theme.muted),
        ),
        Span::styled(format!("{check} "), Style::new().fg(color)),
        Span::styled(item.title.as_str(), title),
    ];
    if total > 0 {
        left.push(Span::styled(
            format!("  {done}/{total}"),
            Style::new().fg(theme.muted),
        ));
    }
    // 右邊：日期（不是今年的前面加年份）、時間，後面留一格
    let mut right = Vec::new();
    if let Some(first) = item.date {
        let year = if first.year() == app.today.year() {
            String::new()
        } else {
            format!("{}/", first.year())
        };
        let date_color = if open {
            day_color(first, app)
        } else {
            theme.muted
        };
        right.push(Span::styled(
            format!("{year}{}", date_text(item)),
            Style::new().fg(date_color),
        ));
        if let Some(time) = item.time {
            right.push(Span::styled(
                format!(" {}", time.strftime("%H:%M")),
                Style::new().fg(theme.muted),
            ));
        }
        right.push(Span::raw(" "));
    }
    let right = Line::from(right);
    // 標題太長：切掉，日期一定看得到（中間至少留一格）
    let right_width = u16::try_from(right.width()).unwrap_or(u16::MAX);
    let left_area = Rect {
        width: area.width.saturating_sub(right_width + 1),
        ..area
    };
    frame.render_widget(Line::from(left), left_area);
    frame.render_widget(right.right_aligned(), area);
}

/// 滑鼠點在任務頁（`area`）的 (`column`, `row`) 點到哪一列；`on_marker` = 點在 `▸` / `▾` 那一格上。
/// 點在表頭、線、最下面那列、外框上回傳 `None`
pub fn click_at(area: Rect, app: &App, column: u16, row: u16) -> Option<Click> {
    let [_, _, body, _, _] = inner_areas(area);
    if !body.contains(Position::new(column, row)) {
        return None;
    }
    let index = app.tasks_top + usize::from(row - body.y);
    let rows = app.tasks_rows();
    let depth = match rows.get(index)? {
        TaskRow::Item { index, .. } => app.items.get(*index)?.depth,
        TaskRow::Heading { .. } => return None,
    };
    // `▸` 在框裡面第 1 + 2 × 層數格
    let marker = usize::from(body.x) + 1 + 2 * depth;
    Some(Click::TaskRow {
        row: index,
        on_marker: usize::from(column) == marker,
    })
}
