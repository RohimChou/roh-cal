//! Rescheduling (`r`): the calendar itself is the date picker. After `r` the keyboard moves to the calendar,
//! where the usual keys (`hjkl`, `[` `]`, `t`, the mouse) pick the new date;
//! `Enter` (or clicking the same day again) writes back to the task file, `Esc` cancels
//!
//! Like dragging: the item moves as many days later (earlier) as the selected day moved; a multi-day item
//! moves as a whole

use super::{App, Focus, Mode, View};
use crate::edit;
use crate::tasks::Item;
use crate::ui::{self, Click};
use jiff::{ToSpan, civil::Date};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// Rescheduling in progress: the item to move, and the layout and pane to return to when done (or cancelled)
#[derive(Debug)]
pub struct Moving {
    /// The item to move (as read when `r` was pressed)
    pub item: Item,
    /// The selected day when `r` was pressed: the item moves as many days as the selected day moves
    from: Date,
    /// Which layout and pane we were in before `r`
    back: (View, Focus),
}

impl App {
    /// `r`: starts moving the item chosen in the agenda (or tasks page). The keyboard moves to the calendar,
    /// and the selected day stays on the item's day; the tasks page borrows layout 1's calendar
    pub(super) fn start_moving(&mut self) {
        let Some(item) = self.chosen_item().cloned() else {
            return;
        };
        let back = (self.view, self.focus);
        // The tasks page has no calendar: borrow layout 1's, starting at the item's day (or today if undated)
        if self.view == View::Tasks {
            self.view = View::Planner;
            self.selected = item.date.unwrap_or(self.today);
            self.scroll_to_month();
        }
        self.mode = Mode::Moving(Moving {
            item,
            from: self.selected,
            back,
        });
        self.focus = Focus::Calendar;
    }

    /// Keys while rescheduling: moving the selected day works as usual; `Enter` confirms, `Esc` cancels,
    /// `Ctrl+C` quits, other keys are ignored
    pub(super) fn handle_moving_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Enter => self.finish_moving(),
            KeyCode::Esc => self.cancel_moving(),
            KeyCode::Char('t') => self.go_today(),
            KeyCode::Char('[') => self.jump_month(-1),
            KeyCode::Char(']') => self.jump_month(1),
            // Only keys that move the date (things like `a` to add on the calendar are ignored)
            code @ (KeyCode::Char('h' | 'j' | 'k' | 'l')
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down) => self.handle_calendar_key(code),
            _ => {}
        }
    }

    /// Mouse while rescheduling: the wheel moves the selected day as usual (a week); clicking a date picks the
    /// new date (clicking the same day again = confirm); clicks elsewhere are ignored
    pub(super) fn handle_moving_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll(1),
            MouseEventKind::ScrollUp => self.scroll(-1),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(Click::Day(date)) = ui::click_at(self, mouse.column, mouse.row) {
                    self.click_while_moving(date);
                }
            }
            _ => {}
        }
    }

    /// Click on a calendar day while rescheduling: select it; clicking the already selected day again = confirm
    pub(super) fn click_while_moving(&mut self, day: Date) {
        if day == self.selected {
            self.finish_moving();
        } else {
            self.selected = day;
        }
    }

    /// Confirm: move the item as many days as the selected day moved and write back to the task file;
    /// go back to the pane from before `r` (the agenda or layout 3's popup has the moved item chosen)
    fn finish_moving(&mut self) {
        let Moving { item, from, back } = match std::mem::take(&mut self.mode) {
            Mode::Moving(moving) => moving,
            // Not rescheduling: do nothing (put it back)
            other => {
                self.mode = other;
                return;
            }
        };
        let to = self.selected;
        // The new day: shift by as many days as the selected day moved (no write if it did not move); an item
        // with no date yet gets the selected day
        let new_first = match item.date {
            Some(first) if to != from => Some(first.saturating_add((to - from).get_days().days())),
            Some(_) => None,
            None => Some(to),
        };
        // Moved (or no move needed); `false` if the write failed or the file was changed elsewhere
        let mut moved = true;
        if let Some(new_first) = new_first {
            let Some((path, text)) = self.read_tasks() else {
                // Cannot read the task file: not moved; the selected day goes back to the one at `r`
                // (same as cancelling)
                self.selected = from;
                self.go_back(back);
                return;
            };
            moved = match edit::reschedule(&text, &item, new_first) {
                Ok(after) => self.write_tasks(&path, text, after, item.title),
                Err(edit::Changed) => {
                    self.reload(&text);
                    false
                }
            };
        }
        self.go_back(back);
        // The agenda chooses the moved item (only that line changes, so its line number is the same); if the
        // move failed or it is not found, stay on the calendar
        if back.1 == Focus::Agenda {
            let index = self
                .agenda()
                .iter()
                .position(|entry| entry.item.line == item.line && entry.is_on(to));
            match index {
                Some(index) if moved => self.agenda_index = index,
                _ => self.focus = Focus::Calendar,
            }
        }
        // Layout 3's popup: open on the new day with the moved item chosen; if the move failed, close it
        if back.1 == Focus::Day {
            if moved {
                self.choose_day_line(item.line);
            } else {
                self.focus = Focus::Calendar;
            }
        }
    }

    /// Cancel: the selected day goes back to the one at `r`, and the pane goes back to the one before `r`
    fn cancel_moving(&mut self) {
        match std::mem::take(&mut self.mode) {
            Mode::Moving(Moving { from, back, .. }) => {
                self.selected = from;
                self.go_back(back);
            }
            other => self.mode = other,
        }
    }

    /// Goes back to the layout and pane from before `r`
    fn go_back(&mut self, (view, focus): (View, Focus)) {
        self.view = view;
        self.focus = focus;
    }
}

#[cfg(test)]
mod tests {
    use super::super::write::Notice;
    use super::super::write::tests::{app_with_task_file, clean_up, press};
    use super::*;
    use jiff::civil::date;
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    #[test]
    fn r_then_hjkl_and_enter_moves_the_chosen_item() {
        let text = "- [ ] Weekly update @2026-10-16 17:30\n- [ ] Coffee chat @2026-10-17 08:00\n";
        let (mut app, path) = app_with_task_file("move", text);
        // Choose "Weekly update" and press r: the keyboard moves to the calendar, and the hint says
        // which item is moving
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(
            app.moving().map(|m| m.item.title.as_str()),
            Some("Weekly update")
        );
        // One week and one day later (10/24), Enter: only the date changes, the time stays;
        // back in the agenda with the moved item chosen
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] Weekly update @2026-10-24 17:30\n- [ ] Coffee chat @2026-10-17 08:00\n"
        );
        assert!(app.moving().is_none());
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(
            app.chosen()
                .map(|entry| (entry.item.title.as_str(), entry.date)),
            Some(("Weekly update", date(2026, 10, 24)))
        );
        // u undoes it
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn esc_cancels_the_move() {
        let text = "- [ ] Weekly update @2026-10-16 17:30\n";
        let (mut app, path) = app_with_task_file("move-cancel", text);
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.selected, date(2026, 11, 1));
        // Esc: the file is untouched, the selected day goes back to 10/16, back to the agenda
        press(&mut app, KeyCode::Esc);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        assert_eq!(app.selected, date(2026, 10, 16));
        assert!(app.moving().is_none());
        assert_eq!(app.focus, Focus::Agenda);
        // Enter without changing the date: no write, no undo step
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(app.notice, Some(Notice::NothingToUndo));
        clean_up(&path);
    }

    #[test]
    fn other_keys_do_nothing_while_moving() {
        let text = "- [ ] Weekly update @2026-10-16\n";
        let (mut app, path) = app_with_task_file("move-keys", text);
        press(&mut app, KeyCode::Char('r'));
        // q, Tab, Space, u are all ignored (no quitting, no pane switch, no checking)
        for code in ['q', ' ', 'u', 's', '3', 'a', 'e']
            .map(KeyCode::Char)
            .into_iter()
            .chain([KeyCode::Tab])
        {
            press(&mut app, code);
            assert!(app.moving().is_some(), "{code:?}");
            assert!(app.editor().is_none(), "{code:?}");
        }
        assert!(!app.should_quit);
        assert_eq!(app.view, View::Planner);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        clean_up(&path);
    }

    #[test]
    fn clicking_a_day_picks_it_and_clicking_it_again_moves_there() {
        // 70 × 23, October at the top: 10/20 (Tue) is at x 15-17, y 6
        let text = "- [ ] Weekly update @2026-10-16\n";
        let (mut app, path) = app_with_task_file("move-click", text);
        app.screen = ratatui::layout::Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        press(&mut app, KeyCode::Char('r'));
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 16,
            row: 6,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(click);
        assert_eq!(app.selected, date(2026, 10, 20));
        assert!(app.moving().is_some());
        app.handle_mouse(click);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- [ ] Weekly update @2026-10-20\n"
        );
        assert!(app.moving().is_none());
        clean_up(&path);
    }

    #[test]
    fn moving_a_multi_day_item_keeps_its_length() {
        // "Manager OOO" 10/28-11/3: press r on the November row (11/2 selected), one week later → the whole
        // span moves a week later, same number of days
        let text = "- Manager OOO @2026-10-28..2026-11-03\n";
        let (mut app, path) = app_with_task_file("move-range", text);
        app.agenda_index = 1;
        app.selected = date(2026, 11, 2);
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "- Manager OOO @2026-11-04..2026-11-10\n"
        );
        // Back in the agenda: the November row of the moved item is chosen (11/9 is also one of its days)
        assert_eq!(app.selected, date(2026, 11, 9));
        assert_eq!(
            app.chosen().map(|entry| entry.date),
            Some(date(2026, 11, 4))
        );
        clean_up(&path);
    }
}
