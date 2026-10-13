//! 任務 view：整份任務檔照分組列出來，含沒日期的；子任務跟當天清單一樣可以展開 / 收起（按鍵在 `app::tasks_view`）

use crate::app::App;
use crate::tasks::{Heading, Item};
use crate::ui::day_list::{ListRow, item_line, push_tree};
use crate::ui::notice_line;
use ratatui::{
    Frame,
    text::Line,
    widgets::{Block, BorderType, List, ListItem, ListState, Padding},
};
use std::collections::BTreeSet;

/// 任務 view 的一行：空行（分組之間）、分組標題、或一個項目
pub enum TreeRow<'a> {
    Blank,
    Heading(&'a Heading),
    Item(ListRow<'a>),
}

/// 任務 view 一行一行列出來：分組標題，底下是那組最外層的項目（照檔案順序）；展開的接著列子任務。
/// 檔案開頭、第一個標題之前的項目沒有標題
pub fn tree_rows<'a>(items: &'a [Item], expanded: &BTreeSet<usize>) -> Vec<TreeRow<'a>> {
    let mut rows = Vec::new();
    let mut group: Option<&Heading> = None;
    for item in items.iter().filter(|item| item.parent.is_none()) {
        // 換了分組：先放標題（不是第一行就先空一行）
        if let Some(heading) = &item.group
            && group != Some(heading)
        {
            if !rows.is_empty() {
                rows.push(TreeRow::Blank);
            }
            rows.push(TreeRow::Heading(heading));
        }
        group = item.group.as_ref();
        let mut tree = Vec::new();
        push_tree(&mut tree, items, item, 0, expanded);
        rows.extend(tree.into_iter().map(TreeRow::Item));
    }
    rows
}

/// 只有項目的那幾行（選取用：`TasksView.selected` 是這裡的 index）
pub fn item_rows<'a>(items: &'a [Item], expanded: &BTreeSet<usize>) -> Vec<ListRow<'a>> {
    tree_rows(items, expanded)
        .into_iter()
        .filter_map(|row| match row {
            TreeRow::Item(row) => Some(row),
            _ => None,
        })
        .collect()
}

/// 畫任務 view：佔滿整個 terminal；標題在上框線、操作說明在下框線（按了 `V`、有提示、正在問刪除時換掉）
pub fn render(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let state = &app.tasks_view;
    let rows = tree_rows(&app.items, &state.expanded);

    let bottom = if app.view_prefix {
        Line::styled(format!(" {} ", app.texts.view_prefix_hint), theme.title)
    } else if let Some(count) = state.confirm_delete {
        let question = (app.texts.delete_confirm)(count.saturating_sub(1));
        Line::styled(format!(" {question} "), theme.title)
    } else if let Some(notice) = notice_line(app).filter(|_| app.overlay.is_none()) {
        notice
    } else {
        Line::styled(format!(" {} ", app.texts.tasks_hint), theme.hint)
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border)
        .padding(Padding::uniform(1))
        .title_top(Line::styled(format!(" {} ", app.texts.tasks_title), theme.title).centered())
        .title_bottom(bottom.centered());

    // 選到的項目在 rows 裡是第幾行（分組標題、空行也算一行）
    let selected = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| matches!(row, TreeRow::Item(_)))
        .nth(state.selected)
        .map(|(index, _)| index);
    let lines: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            TreeRow::Blank => ListItem::new(""),
            // 分組標題：`##` 以下每多一層往右縮 2 格
            TreeRow::Heading(heading) => {
                let indent = "  ".repeat(heading.level.saturating_sub(1));
                ListItem::new(Line::styled(
                    format!(" {indent}{}", heading.name),
                    theme.title,
                ))
            }
            TreeRow::Item(row) => ListItem::new(item_line(row, None, &state.expanded, app)),
        })
        .collect();
    // List 會自己捲動（選到的那行一定在框內）
    let mut list_state = ListState::default().with_selected(selected);
    let widget = List::new(lines)
        .block(block)
        .highlight_style(theme.selected_bg);
    frame.render_stateful_widget(widget, frame.area(), &mut list_state);
}
