//! 說明頁（`?`）開著時的按鍵和滑鼠：`Tab` 換精簡版 / 完整版、放不下時上下捲、`?` / `q` / `Esc` 或點框外關掉

use super::{App, Mode};
use crate::ui;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    layout::Position,
};

/// 說明頁開著：哪一版、往下捲了幾行
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Help {
    /// `true` = 完整版（所有的鍵）；`false` = 精簡版（最常用的鍵）
    pub full: bool,
    /// 畫面放不下時往下捲了幾行
    pub scroll: u16,
}

impl App {
    /// 說明頁開著時的按鍵：`j` / `k`（`↓` / `↑`）放不下時上下捲；`Tab` 換精簡版 / 完整版（回到最上面）；
    /// `?` / `q` / `Esc` 關掉；`Ctrl+C` 離開；其他鍵不理
    pub(super) fn handle_help_key(&mut self, key: KeyEvent) {
        let Some(help) = self.help() else {
            return;
        };
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => self.scroll_help(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_help(-1),
            KeyCode::Tab | KeyCode::BackTab => {
                self.mode = Mode::Help(Help {
                    full: !help.full,
                    scroll: 0,
                });
            }
            KeyCode::Char('?' | 'q') | KeyCode::Esc => self.mode = Mode::Normal,
            _ => {}
        }
    }

    /// 說明頁開著時的滑鼠：滾輪上下捲（底下的月曆不動）、點框外關掉，點框裡不理
    pub(super) fn handle_help_mouse(&mut self, mouse: MouseEvent) {
        let Some(help) = self.help() else {
            return;
        };
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll_help(1),
            MouseEventKind::ScrollUp => self.scroll_help(-1),
            MouseEventKind::Down(MouseButton::Left)
                if !ui::help_area(self, help.full)
                    .contains(Position::new(mouse.column, mouse.row)) =>
            {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    /// 說明頁往下（`step` = 1）/ 往上（-1）捲一行：放得下就不動，捲到底就停
    fn scroll_help(&mut self, step: i16) {
        let Some(help) = self.help() else {
            return;
        };
        // 最多能捲幾行（照 terminal 現在的大小算）；terminal 拉高之後原本捲的可能超過，先夾回來
        let max = ui::help_max_scroll(self, help.full);
        let scroll = help.scroll.min(max).saturating_add_signed(step).min(max);
        self.mode = Mode::Help(Help { scroll, ..help });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::layout::Rect;

    /// 打開說明頁的 app：今天 2026/10/17、terminal `width` × `height`
    fn app_with_help(width: u16, height: u16) -> App {
        let mut app = App::new(Lang::ZhTw, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, width, height);
        app.handle_key(KeyEvent::from(KeyCode::Char('?')));
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn question_mark_opens_the_short_page_and_tab_switches() {
        let mut app = app_with_help(100, 40);
        assert_eq!(app.help(), Some(Help::default()));
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            app.help(),
            Some(Help {
                full: true,
                scroll: 0
            })
        );
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.help(), Some(Help::default()));
    }

    #[test]
    fn question_mark_q_and_esc_close_it_without_quitting() {
        for code in [KeyCode::Char('?'), KeyCode::Char('q'), KeyCode::Esc] {
            let mut app = app_with_help(100, 40);
            press(&mut app, code);
            assert_eq!(app.help(), None, "{code:?}");
            assert!(!app.should_quit, "{code:?}");
        }
    }

    #[test]
    fn other_keys_do_nothing_while_it_is_open() {
        let mut app = app_with_help(100, 40);
        // 移動、換版型、換框、新增都不理：選取的日期、版型、框都不變
        for code in ['l', 'j', '3', 'a', 't', ']', 's', 'u'].map(KeyCode::Char) {
            press(&mut app, code);
        }
        assert_eq!(app.help(), Some(Help::default()));
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.view, super::super::View::Planner);
        assert!(app.editor().is_none());
        // Ctrl+C 還是能離開
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn j_and_k_scroll_only_when_it_does_not_fit() {
        // 100 × 40：完整版放得下，j 不動
        let mut app = app_with_help(100, 40);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.help().map(|help| help.scroll), Some(0));
        // 100 × 20：完整版 28 行 + padding 2 + 框線 2 = 32 行，框最高 20、裡面放得下 16 行 → 能捲 12 行
        let mut app = app_with_help(100, 20);
        press(&mut app, KeyCode::Tab);
        for _ in 0..15 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(app.help().map(|help| help.scroll), Some(12));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.help().map(|help| help.scroll), Some(11));
        // 滾輪也一樣，底下的月曆不動
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 50, 10));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 50, 10));
        assert_eq!(app.help().map(|help| help.scroll), Some(9));
        assert_eq!(app.selected, date(2026, 10, 17));
        // Tab 換回精簡版：回到最上面
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.help(), Some(Help::default()));
    }

    #[test]
    fn clicking_outside_closes_it() {
        let mut app = app_with_help(100, 40);
        // 點框裡（正中間）不理，底下的月曆也不會選到
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 50, 20));
        assert!(app.help().is_some());
        assert_eq!(app.selected, date(2026, 10, 17));
        // 點左上角（框外）：關掉，那一下不會選到底下的東西
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0));
        assert_eq!(app.help(), None);
        assert_eq!(app.selected, date(2026, 10, 17));
    }
}
