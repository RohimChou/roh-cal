//! 任務頁（`4`）：整份任務檔照日曆（分組）列出來，子任務排成樹（預設收起來），沒日期的也列；
//! `j` / `k` 上下、`l` / `h` 展開 / 收起（`+` / `-` 全部）、`L` / `H` 縮排 / 取消縮排、`J` / `K` 上下搬

use super::App;
use crate::{edit, tasks::Item};
use ratatui::crossterm::event::KeyCode;

/// 任務頁的一列
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRow {
    /// 日曆（分組）的標題
    Heading {
        /// 標題的名字；`None` = 檔案開頭、還沒有標題的「未分組」
        name: Option<String>,
        /// 幾個 `#`（縮排用）
        level: usize,
    },
    /// 一件事
    Item {
        /// `App::items` 裡的位置
        index: usize,
        /// 有沒有子任務
        children: bool,
        /// 子任務展開了沒（沒有子任務的一定是 `false`）
        expanded: bool,
    },
}

impl App {
    /// 任務頁的每一列，照檔案的順序：日曆換了就先列一列標題；子任務要上面每一層都展開了才列；
    /// 日曆沒勾（side menu）的整組不列
    pub fn tasks_rows(&self) -> Vec<TaskRow> {
        let mut rows = Vec::new();
        // 上一件列出來的事是哪一組（換組就列標題）
        let mut previous = None;
        for (index, item) in self.items.iter().enumerate() {
            if !self.is_shown(item) || !self.ancestors_expanded(item) {
                continue;
            }
            if previous != Some(&item.group) {
                rows.push(TaskRow::Heading {
                    name: item.group.as_ref().map(|heading| heading.name.clone()),
                    level: item.group.as_ref().map_or(1, |heading| heading.level),
                });
                previous = Some(&item.group);
            }
            let children = self.items.iter().any(|other| other.parent == Some(index));
            rows.push(TaskRow::Item {
                index,
                children,
                // 子任務都刪光了的，就算還記著展開也不算
                expanded: children && self.expanded.contains(&item.line),
            });
        }
        rows
    }

    /// `item` 上面每一層（父項目、父項目的父項目…）都展開了沒
    fn ancestors_expanded(&self, item: &Item) -> bool {
        let mut parent = item.parent;
        while let Some(index) = parent {
            let Some(ancestor) = self.items.get(index) else {
                return false;
            };
            if !self.expanded.contains(&ancestor.line) {
                return false;
            }
            parent = ancestor.parent;
        }
        true
    }

    /// 任務頁選到的那一列（`tasks_rows()` 裡的位置）：`tasks_line` 那一行的事；它不在畫面上（例：剛好被收起來、
    /// 日曆沒勾）或還沒選過，就是第一件
    pub fn tasks_cursor(&self, rows: &[TaskRow]) -> Option<usize> {
        let line_of = |row: &TaskRow| match row {
            TaskRow::Item { index, .. } => self.items.get(*index).map(|item| item.line),
            TaskRow::Heading { .. } => None,
        };
        rows.iter()
            .position(|row| self.tasks_line.is_some() && line_of(row) == self.tasks_line)
            .or_else(|| rows.iter().position(|row| line_of(row).is_some()))
    }

    /// 任務頁選到的那件事
    pub fn tasks_chosen(&self) -> Option<&Item> {
        let rows = self.tasks_rows();
        match rows.get(self.tasks_cursor(&rows)?)? {
            TaskRow::Item { index, .. } => self.items.get(*index),
            TaskRow::Heading { .. } => None,
        }
    }

    /// 鍵盤在任務頁：`j` / `k` 上下一件（標題跳過），`l` / `h` 展開 / 收起子任務，`+` / `-` 全部展開 / 全部收起，
    /// `L` / `H` 縮排 / 取消縮排，`J` / `K` 往下 / 往上搬；
    /// 寫檔的鍵跟右邊的事項框一樣：`Space` / `x` 打勾、`e` / `Enter` 編輯、`a` 新增、`A` 新增子任務、`d` 刪除、`r` 改期
    pub(super) fn handle_tasks_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_task(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_task(-1),
            KeyCode::Char('l') | KeyCode::Right => self.expand_task(),
            KeyCode::Char('h') | KeyCode::Left => self.collapse_task(),
            KeyCode::Char('+') => self.expand_all(),
            KeyCode::Char('-') => self.collapse_all(),
            KeyCode::Char('L') => self.indent_task(),
            KeyCode::Char('H') => self.outdent_task(),
            KeyCode::Char('J') => self.move_task(true),
            KeyCode::Char('K') => self.move_task(false),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            KeyCode::Char('r') => self.start_moving(),
            _ => {}
        }
    }

    /// 選往後（負的 = 往前）第 `step` 件（標題不算）；到第一件 / 最後一件就停住
    pub(super) fn choose_task(&mut self, step: isize) {
        let rows = self.tasks_rows();
        let lines: Vec<usize> = rows
            .iter()
            .filter_map(|row| match row {
                TaskRow::Item { index, .. } => self.items.get(*index).map(|item| item.line),
                TaskRow::Heading { .. } => None,
            })
            .collect();
        let Some(last) = lines.len().checked_sub(1) else {
            return;
        };
        let current = self
            .tasks_line
            .and_then(|line| lines.iter().position(|&other| other == line))
            .unwrap_or(0);
        self.tasks_line = Some(lines[current.saturating_add_signed(step).min(last)]);
    }

    /// 檔案第 `line` 行的那件事有沒有子任務
    fn has_children(&self, line: usize) -> bool {
        self.items
            .iter()
            .position(|item| item.line == line)
            .is_some_and(|index| self.items.iter().any(|other| other.parent == Some(index)))
    }

    /// `l`：選到的那件有子任務、還收著就展開；已經展開就跳到第一個子任務
    fn expand_task(&mut self) {
        let Some(line) = self.tasks_chosen().map(|item| item.line) else {
            return;
        };
        if !self.has_children(line) {
            return;
        }
        if self.expanded.insert(line) {
            return;
        }
        self.choose_task(1);
    }

    /// `h`：選到的那件展開著就收起來；不然跳到它的父項目（像 VS Code 的檔案樹）
    fn collapse_task(&mut self) {
        let Some((line, parent)) = self.tasks_chosen().map(|item| (item.line, item.parent)) else {
            return;
        };
        // 沒有子任務卻記著展開（子任務都刪光了）是舊的記錄：清掉，照樣跳到父項目
        if self.expanded.remove(&line) && self.has_children(line) {
            return;
        }
        if let Some(parent) = parent.and_then(|index| self.items.get(index)) {
            self.tasks_line = Some(parent.line);
        }
    }

    /// `+`：每一件有子任務的都展開
    fn expand_all(&mut self) {
        let parents: Vec<usize> = self
            .items
            .iter()
            .filter_map(|item| item.parent)
            .filter_map(|index| self.items.get(index))
            .map(|parent| parent.line)
            .collect();
        self.expanded.extend(parents);
    }

    /// `-`：全部收起來；選到的是子任務（收起來就看不到了）就改選它最上層的那件
    fn collapse_all(&mut self) {
        let top = self.tasks_chosen().map(|chosen| {
            let mut item = chosen;
            while let Some(parent) = item.parent.and_then(|index| self.items.get(index)) {
                item = parent;
            }
            item.line
        });
        if let Some(line) = top {
            self.tasks_line = Some(line);
        }
        self.expanded.clear();
    }

    /// `L`：選到的那件（連子任務）往裡縮一層，變成上面同一層那件的子任務（縮排 = 那件的縮排 + 2 格，跟 `A` 一樣）；
    /// 上面沒有同一層的（那一組、那一層的第一件）不動
    fn indent_task(&mut self) {
        let Some(item) = self.tasks_chosen().cloned() else {
            return;
        };
        let Some(above) = self.sibling_above(&item).cloned() else {
            return;
        };
        let to = format!("{}  ", edit::indent_of(&above.line_text));
        // 新的父項目展開，縮進去的那件才看得到
        self.reindent_task(&item, &to, above.line);
    }

    /// `H`：選到的那件（連子任務）往外一層，對齊它的父項目；它下面同一層的事變成它的子任務（跟 Obsidian 一樣）。
    /// 本來就在最外層的不動
    fn outdent_task(&mut self) {
        let Some(item) = self.tasks_chosen().cloned() else {
            return;
        };
        let Some(parent) = item.parent.and_then(|index| self.items.get(index)) else {
            return;
        };
        let to = edit::indent_of(&parent.line_text).to_string();
        // 它自己展開：下面變成它子任務的那幾件照樣看得到
        self.reindent_task(&item, &to, item.line);
    }

    /// `item` 上面同一層的那件（同一個父項目、同一段標題底下；中間隔著的是那件的子任務）；沒有就是 `None`
    fn sibling_above(&self, item: &Item) -> Option<&Item> {
        let index = self
            .items
            .iter()
            .position(|other| other.line == item.line)?;
        self.items[..index]
            .iter()
            .rev()
            .find(|other| other.depth <= item.depth)
            .filter(|other| {
                other.depth == item.depth
                    && other.parent == item.parent
                    && other.section == item.section
            })
    }

    /// `item` 下面同一層的那件（同一個父項目、同一段標題底下；中間隔著的是 `item` 自己的子任務）；沒有就是 `None`
    fn sibling_below(&self, item: &Item) -> Option<&Item> {
        let index = self
            .items
            .iter()
            .position(|other| other.line == item.line)?;
        self.items[index + 1..]
            .iter()
            .find(|other| other.depth <= item.depth)
            .filter(|other| {
                other.depth == item.depth
                    && other.parent == item.parent
                    && other.section == item.section
            })
    }

    /// `J` / `K`：選到的那件（連子任務）跟下面（`down`）/ 上面同一層的那件換位置，寫回任務檔；
    /// 那一層的最後一件 / 第一件不動（不會搬到別的父項目、別組）。搬完還選著它（行號照 `edit::Swap` 跟著搬）
    fn move_task(&mut self, down: bool) {
        let Some(item) = self.tasks_chosen().cloned() else {
            return;
        };
        let other = if down {
            self.sibling_below(&item)
        } else {
            self.sibling_above(&item)
        };
        let Some(other) = other.cloned() else {
            return;
        };
        let (upper, lower) = if down {
            (&item, &other)
        } else {
            (&other, &item)
        };
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        // 剛打開還沒選過（預設選第一件）也先記下來，搬完選取才跟得過去
        self.tasks_line = Some(item.line);
        match edit::swap(&text, upper, lower) {
            Ok((after, swap)) => {
                self.write_tasks_swapped(&path, text, after, item.title.clone(), Some(swap));
            }
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// `item` 那一塊的縮排換成 `to`、寫回任務檔；寫成了就展開第 `expand` 行那件。在別處改過就重新載入
    fn reindent_task(&mut self, item: &Item, to: &str, expand: usize) {
        let Some((path, text)) = self.read_tasks() else {
            return;
        };
        match edit::reindent(&text, item, to) {
            Ok(after) => {
                if self.write_tasks(&path, text, after, item.title.clone()) {
                    self.expanded.insert(expand);
                }
            }
            Err(edit::Changed) => self.reload(&text),
        }
    }

    /// 點任務頁的第 `row` 列（`tasks_rows()` 裡的位置）：點一下選它，點的是 `▸` / `▾` 就展開 / 收起；
    /// 點標題不理
    pub(super) fn click_task(&mut self, row: usize, on_marker: bool) {
        let Some(TaskRow::Item {
            index, children, ..
        }) = self.tasks_rows().get(row).cloned()
        else {
            return;
        };
        let Some(line) = self.items.get(index).map(|item| item.line) else {
            return;
        };
        if on_marker && children {
            if !self.expanded.remove(&line) {
                self.expanded.insert(line);
            }
        } else if self.focus == super::Focus::Tasks && self.tasks_line == Some(line) {
            // 再點一次已經選到的那件 = 打勾（跟右邊的事項框一樣）
            self.toggle_chosen();
            return;
        }
        self.tasks_line = Some(line);
        self.focus = super::Focus::Tasks;
    }

    /// 寫檔之後，任務頁選著第 `line` 行那件事；它在收起來的父項目底下就把上面每一層都展開
    pub(super) fn choose_task_line(&mut self, line: usize) {
        self.tasks_line = Some(line);
        let mut parent = self
            .items
            .iter()
            .find(|item| item.line == line)
            .and_then(|item| item.parent);
        while let Some(index) = parent {
            let Some(ancestor) = self.items.get(index) else {
                break;
            };
            self.expanded.insert(ancestor.line);
            parent = ancestor.parent;
        }
    }

    /// 刪除之後，任務頁選第 `row` 列（`tasks_rows()` 裡的位置）那件或它後面第一件；後面沒有就選最後一件
    pub(super) fn choose_task_row_near(&mut self, row: usize) {
        let rows = self.tasks_rows();
        let line_of = |row: &TaskRow| match row {
            TaskRow::Item { index, .. } => self.items.get(*index).map(|item| item.line),
            TaskRow::Heading { .. } => None,
        };
        self.tasks_line = rows
            .iter()
            .skip(row)
            .find_map(line_of)
            .or_else(|| rows.iter().rev().find_map(line_of));
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Focus, View};
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::crossterm::event::KeyEvent;

    /// 任務頁的 app（今天 2026/10/17）、任務檔是 `text`
    fn tasks_app(text: &str) -> App {
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        );
        app.handle_key(KeyEvent::from(KeyCode::Char('4')));
        app
    }

    /// 按一個鍵
    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    /// 每一列的樣子：標題 `#名字`、事 `標題`（前面每一層兩格空白，有子任務的收著 `+`、展開 `-`）
    fn shape(app: &App) -> Vec<String> {
        app.tasks_rows()
            .iter()
            .map(|row| match row {
                TaskRow::Heading { name, .. } => format!("#{}", name.as_deref().unwrap_or("")),
                TaskRow::Item {
                    index,
                    children,
                    expanded,
                } => {
                    let item = &app.items[*index];
                    let mark = match (children, expanded) {
                        (false, _) => "",
                        (true, false) => "+",
                        (true, true) => "-",
                    };
                    format!("{}{mark}{}", "  ".repeat(item.depth), item.title)
                }
            })
            .collect()
    }

    /// 任務頁的 test 用的檔案：沒分組的一件、「工作」有三層子任務、「生活」
    const TREE: &str = "- 沒分組\n# 工作\n- [ ] 寫報告 @2026-10-16\n  - [ ] 找資料\n    - [ ] 查文獻\n  - [x] 列大綱\n- [ ] 整理書櫃\n# 生活\n- 爬山 @2026-10-18\n";

    #[test]
    fn rows_list_the_whole_file_by_calendar_with_subtasks_collapsed() {
        let app = tasks_app(TREE);
        assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
        // 沒日期的也列；子任務預設收著
        assert_eq!(
            shape(&app),
            [
                "#",
                "沒分組",
                "#工作",
                "+寫報告",
                "整理書櫃",
                "#生活",
                "爬山"
            ]
        );
    }

    #[test]
    fn l_and_h_expand_and_collapse_like_a_file_tree() {
        let mut app = tasks_app(TREE);
        // 第一件是「沒分組」；往下到寫報告，l 展開
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("寫報告")
        );
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(
            shape(&app),
            [
                "#",
                "沒分組",
                "#工作",
                "-寫報告",
                "  +找資料",
                "  列大綱",
                "整理書櫃",
                "#生活",
                "爬山"
            ]
        );
        // 已經展開再按 l：跳到第一個子任務；再 l 展開它
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("找資料")
        );
        press(&mut app, KeyCode::Right);
        assert!(shape(&app).contains(&"    查文獻".to_string()));
        // h：展開著的先收起來；再 h 跳回父項目；再 h 收起父項目
        press(&mut app, KeyCode::Char('h'));
        assert!(!shape(&app).contains(&"    查文獻".to_string()));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("寫報告")
        );
        press(&mut app, KeyCode::Left);
        assert_eq!(shape(&app)[3], "+寫報告");
        // 沒有子任務的按 l：不動
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("整理書櫃")
        );
    }

    #[test]
    fn plus_and_minus_expand_and_collapse_everything() {
        let mut app = tasks_app(TREE);
        // +：每一層都展開，還選著原本那件（沒分組）
        press(&mut app, KeyCode::Char('+'));
        assert_eq!(
            shape(&app),
            [
                "#",
                "沒分組",
                "#工作",
                "-寫報告",
                "  -找資料",
                "    查文獻",
                "  列大綱",
                "整理書櫃",
                "#生活",
                "爬山"
            ]
        );
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("沒分組")
        );
        // 往下選到最深的查文獻；-：全部收起，改選它最上層的寫報告
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("查文獻")
        );
        press(&mut app, KeyCode::Char('-'));
        assert_eq!(
            shape(&app),
            [
                "#",
                "沒分組",
                "#工作",
                "+寫報告",
                "整理書櫃",
                "#生活",
                "爬山"
            ]
        );
        assert_eq!(app.tasks_line, Some(3));
    }

    #[test]
    fn j_and_k_skip_headings_and_stop_at_the_ends() {
        let mut app = tasks_app(TREE);
        for _ in 0..10 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("爬山")
        );
        press(&mut app, KeyCode::Up);
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("整理書櫃")
        );
        for _ in 0..10 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("沒分組")
        );
    }

    #[test]
    fn unchecked_calendars_leave_the_tasks_page() {
        let mut app = tasks_app(TREE);
        app.groups[1].shown = false;
        assert_eq!(shape(&app), ["#", "沒分組", "#生活", "爬山"]);
    }

    /// 暫存資料夾裡的任務檔（內容 `text`）、開著任務頁的 app；用完要 `clean_up`
    fn tasks_app_with_file(name: &str, text: &str) -> (App, std::path::PathBuf) {
        let (mut app, path) = super::super::write::tests::app_with_task_file(name, text);
        press(&mut app, KeyCode::Char('4'));
        (app, path)
    }

    /// 一個字一個字打進編輯框
    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn the_writing_keys_work_on_the_tasks_page() {
        use super::super::write::tests::clean_up;
        let text = "# 工作\n- [ ] 寫報告\n  - [ ] 找資料\n- [ ] 交週報 @2026-10-16\n# 生活\n- 爬山 @2026-10-18\n";
        let (mut app, path) = tasks_app_with_file("tasks-keys", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        // Space：沒日期的也能打勾；還選著同一件
        press(&mut app, KeyCode::Char(' '));
        assert!(read().contains("- [x] 寫報告\n"));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("寫報告")
        );
        // A：子任務加在它的子任務下面；父項目展開、選著新的那件
        press(&mut app, KeyCode::Char('A'));
        type_text(&mut app, "列大綱");
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("  - [ ] 找資料\n  - [ ] 列大綱\n- [ ] 交週報"));
        assert!(app.expanded.contains(&2));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("列大綱")
        );
        // d、y：刪掉；選著下一件（交週報，現在是第 4 行）
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert!(!read().contains("列大綱"));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("交週報")
        );
        // u：列大綱回來；交週報跟著往下一行，還是選著它
        press(&mut app, KeyCode::Char('u'));
        assert!(read().contains("列大綱"));
        assert_eq!(app.tasks_line, Some(5));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("交週報")
        );
        // a：任務頁不填日期；加在「工作」最後面，選著新的那件
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(
            app.editor().map(|editor| editor.input.value()),
            Some("- [ ] ")
        );
        type_text(&mut app, "開會 @2026-10-20");
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("- [ ] 交週報 @2026-10-16\n- [ ] 開會 @2026-10-20\n# 生活"));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("開會")
        );
        // e：編輯選到的那件
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " 10:00");
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("- [ ] 開會 @2026-10-20 10:00\n"));
        assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
        clean_up(&path);
    }

    #[test]
    fn shift_l_and_h_indent_and_outdent_with_the_subtasks() {
        use super::super::write::tests::clean_up;
        let text = "# 工作\n- [ ] 寫報告\n  - [ ] 找資料\n  - [ ] 列大綱\n- [ ] 交週報\n  - [ ] 填表\n# 生活\n- 爬山\n";
        let (mut app, path) = tasks_app_with_file("tasks-indent", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
        // 寫報告是第一件，上面沒有同一層的：L 不動
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(read(), text);
        // 交週報 L：連填表一起縮進去，變成寫報告的子任務；寫報告展開，還選著交週報
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(
            read(),
            "# 工作\n- [ ] 寫報告\n  - [ ] 找資料\n  - [ ] 列大綱\n  - [ ] 交週報\n    - [ ] 填表\n# 生活\n- 爬山\n"
        );
        assert_eq!(
            shape(&app),
            [
                "#工作",
                "-寫報告",
                "  找資料",
                "  列大綱",
                "  +交週報",
                "#生活",
                "爬山"
            ]
        );
        assert_eq!(chosen(&app).as_deref(), Some("交週報"));
        // u 復原；往上選到找資料，H：對齊寫報告，下面的列大綱變成它的子任務、它展開
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), text);
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(chosen(&app).as_deref(), Some("找資料"));
        press(&mut app, KeyCode::Char('H'));
        assert_eq!(
            read(),
            "# 工作\n- [ ] 寫報告\n- [ ] 找資料\n  - [ ] 列大綱\n- [ ] 交週報\n  - [ ] 填表\n# 生活\n- 爬山\n"
        );
        assert_eq!(
            shape(&app),
            [
                "#工作",
                "寫報告",
                "-找資料",
                "  列大綱",
                "+交週報",
                "#生活",
                "爬山"
            ]
        );
        // 爬山：最外層（H 不動）、也是「生活」的第一件（上面的交週報是別組，L 不動）
        let before = read();
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(chosen(&app).as_deref(), Some("爬山"));
        press(&mut app, KeyCode::Char('H'));
        press(&mut app, KeyCode::Char('L'));
        assert_eq!(read(), before);
        clean_up(&path);
    }

    #[test]
    fn shift_j_and_k_move_the_item_with_its_subtasks() {
        use super::super::write::tests::clean_up;
        let text =
            "# 工作\n- [ ] 寫報告\n  - [ ] 找資料\n  - [ ] 列大綱\n- [ ] 交週報\n# 生活\n- 爬山\n";
        let (mut app, path) = tasks_app_with_file("tasks-swap", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        let chosen = |app: &App| app.tasks_chosen().map(|item| item.title.clone());
        // 寫報告（第 2 行，展開著）J：連子任務搬到交週報下面；還選著它、還展開著（現在是第 3 行）
        app.expanded.insert(2);
        press(&mut app, KeyCode::Char('J'));
        let moved =
            "# 工作\n- [ ] 交週報\n- [ ] 寫報告\n  - [ ] 找資料\n  - [ ] 列大綱\n# 生活\n- 爬山\n";
        assert_eq!(read(), moved);
        assert_eq!(chosen(&app).as_deref(), Some("寫報告"));
        assert_eq!(app.expanded.iter().copied().collect::<Vec<_>>(), [3]);
        assert_eq!(
            shape(&app),
            [
                "#工作",
                "交週報",
                "-寫報告",
                "  找資料",
                "  列大綱",
                "#生活",
                "爬山"
            ]
        );
        // 已經是「工作」最後一件（下面的爬山是別組）：J 不動
        press(&mut app, KeyCode::Char('J'));
        assert_eq!(read(), moved);
        // 列大綱 K：跟找資料換；再 K：已經是第一個子任務，不動
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('K'));
        let swapped =
            "# 工作\n- [ ] 交週報\n- [ ] 寫報告\n  - [ ] 列大綱\n  - [ ] 找資料\n# 生活\n- 爬山\n";
        assert_eq!(read(), swapped);
        assert_eq!(app.tasks_line, Some(4));
        press(&mut app, KeyCode::Char('K'));
        assert_eq!(read(), swapped);
        // u：換回來，還選著列大綱（回到第 5 行）
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), moved);
        assert_eq!(app.tasks_line, Some(5));
        assert_eq!(chosen(&app).as_deref(), Some("列大綱"));
        clean_up(&path);
    }

    #[test]
    fn indent_and_move_stay_inside_their_heading_section() {
        use super::super::write::tests::clean_up;
        // 兩段同名的「工作」中間夾著沒有事的「生活」：第二段的交週報上面沒有同一段的事，L、K 都不動（不白寫一次檔）
        let text = "# 工作\n- [ ] 寫報告\n# 生活\n# 工作\n- [ ] 交週報\n";
        let (mut app, path) = tasks_app_with_file("tasks-sections", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        app.tasks_line = Some(5);
        for key in ['L', 'K'] {
            press(&mut app, KeyCode::Char(key));
            assert_eq!(read(), text, "{key}");
        }
        assert!(app.expanded.is_empty());
        // 第一段的寫報告：下面也沒有同一段的事，J 不動
        app.tasks_line = Some(2);
        press(&mut app, KeyCode::Char('J'));
        assert_eq!(read(), text);
        clean_up(&path);
    }

    #[test]
    fn moving_one_of_two_same_named_items_keeps_the_choice_on_it() {
        use super::super::write::tests::clean_up;
        // 兩件都叫運動：第一件（第 1 行，展開著）J → 搬到第 3 行，選取、展開都跟著它
        let text = "- [ ] 運動\n  - [ ] 跑步\n- [ ] 運動\n  - [ ] 游泳\n";
        let (mut app, path) = tasks_app_with_file("tasks-same-name", text);
        let read = || std::fs::read_to_string(&path).expect("read");
        app.expanded.insert(1);
        press(&mut app, KeyCode::Char('J'));
        let moved = "- [ ] 運動\n  - [ ] 游泳\n- [ ] 運動\n  - [ ] 跑步\n";
        assert_eq!(read(), moved);
        assert_eq!(app.tasks_line, Some(3));
        assert_eq!(app.expanded.iter().copied().collect::<Vec<_>>(), [3]);
        // 已經是最後一件：再 J 不動（不會換回去）
        press(&mut app, KeyCode::Char('J'));
        assert_eq!(read(), moved);
        // u：搬回來，選取、展開也回到第 1 行
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), text);
        assert_eq!(app.tasks_line, Some(1));
        assert_eq!(app.expanded.iter().copied().collect::<Vec<_>>(), [1]);
        clean_up(&path);
    }

    #[test]
    fn deleting_every_subtask_does_not_leave_the_parent_expanded() {
        use super::super::write::tests::clean_up;
        // 寫報告（第 3 行）、找資料（第 4 行）都展開；刪掉找資料唯一的子任務查文獻（第 5 行）
        let (mut app, path) = tasks_app_with_file("tasks-stale-expanded", TREE);
        app.expanded.extend([3, 4]);
        app.tasks_line = Some(5);
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        // 找資料沒有子任務了：不算展開；按一次 h 就跳到父項目寫報告（不用按兩次）
        app.tasks_line = Some(4);
        let row = app.tasks_cursor(&app.tasks_rows()).expect("chosen");
        assert!(matches!(
            app.tasks_rows()[row],
            TaskRow::Item {
                children: false,
                expanded: false,
                ..
            }
        ));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.tasks_line, Some(3));
        clean_up(&path);
    }

    #[test]
    fn a_conflict_on_the_tasks_page_keeps_the_keyboard_there() {
        use super::super::write::{Notice, tests::clean_up};
        // 編輯、新增、刪除的時候檔案在別處改過（最前面多了一行）：重新載入，鍵盤還是在任務頁，不會跑到看不到的月曆
        let text = "- [ ] 整理書櫃\n- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = tasks_app_with_file("tasks-conflict", text);
        // 每次在最前面加不一樣的一行：選到的那件（第 1 行）每次都對不上
        let mut extra = String::new();
        let mut change_elsewhere = || {
            extra = format!("- 別處加的 {}\n{extra}", extra.lines().count() + 1);
            std::fs::write(&path, format!("{extra}{text}")).expect("write");
        };
        for keys in [&['e', '!'][..], &['a', 'x'], &['d']] {
            for &key in keys {
                press(&mut app, KeyCode::Char(key));
            }
            change_elsewhere();
            let confirm = if keys[0] == 'd' {
                KeyCode::Char('y')
            } else {
                KeyCode::Enter
            };
            press(&mut app, confirm);
            assert_eq!(app.notice, Some(Notice::Reloaded), "{keys:?}");
            assert_eq!(
                (app.view, app.focus),
                (View::Tasks, Focus::Tasks),
                "{keys:?}"
            );
        }
        clean_up(&path);
    }

    #[test]
    fn r_on_the_tasks_page_borrows_the_calendar() {
        use super::super::write::tests::clean_up;
        // 還沒排日期的整理書櫃：r 換到版型 1 的月曆，從今天開始選；往後一天、Enter → 排在 10/18，回到任務頁
        let text = "- [ ] 整理書櫃\n- [ ] 交週報 @2026-10-16\n";
        let (mut app, path) = tasks_app_with_file("tasks-move", text);
        press(&mut app, KeyCode::Char('r'));
        assert_eq!((app.view, app.focus), (View::Planner, Focus::Calendar));
        assert_eq!(app.selected, date(2026, 10, 17));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] 整理書櫃 @2026-10-18\n- [ ] 交週報 @2026-10-16\n"
        );
        assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
        assert_eq!(
            app.tasks_chosen().map(|item| item.title.as_str()),
            Some("整理書櫃")
        );
        // Esc 取消：也回到任務頁
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.view, app.focus), (View::Tasks, Focus::Tasks));
        clean_up(&path);
    }

    #[test]
    fn expanded_items_follow_their_lines_after_a_delete_above() {
        use super::super::write::tests::clean_up;
        // b（第 2 行）展開著；刪掉上面的 a 之後 b 變成第 1 行，還是展開著
        let text = "- [ ] a\n- [ ] b\n  - [ ] c\n";
        let (mut app, path) = tasks_app_with_file("tasks-remap", text);
        app.expanded.insert(2);
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.expanded.iter().copied().collect::<Vec<_>>(), [1]);
        assert_eq!(shape(&app), ["#", "-b", "  c"]);
        clean_up(&path);
    }

    #[test]
    fn esc_goes_back_to_the_calendar_and_tab_reaches_the_side_menu() {
        let mut app = tasks_app(TREE);
        // 夠寬：Tab 換到 side menu、再 Tab 回任務頁；Esc 從 side menu 回任務頁
        app.screen = ratatui::layout::Rect::new(0, 0, 120, 30);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Side);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Tasks);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.focus, Focus::Tasks);
        // 在任務頁按 Esc：回到版型 1 的月曆（不是離開）
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.view, app.focus), (View::Planner, Focus::Calendar));
        assert!(!app.should_quit);
    }
}
