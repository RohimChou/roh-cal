//! Layout 2's day popup: `Enter` on the calendar (or double-clicking a day) opens it. It lists all
//! items on the selected day (also ones a cell cannot fit and shows as `+N`). The keys are the same as in
//! layout 1's agenda (drawing is in `ui::day`)

use super::{App, Focus};
use crate::ui;
use ratatui::{crossterm::event::KeyCode, layout::Position};

impl App {
    /// Opens the popup with the first item chosen; does not open if the day has no items
    pub(super) fn open_day_popup(&mut self) {
        if self.items_on(self.selected).is_empty() {
            return;
        }
        self.day_index = 0;
        self.day_clicked = false;
        self.day_top = 0;
        self.focus = Focus::Day;
    }

    /// Keyboard in the popup: `j` / `k` next / previous item. Keys that write match the agenda:
    /// `Space` / `x` check, `r` reschedule, `e` / `Enter` edit,
    /// `a` add, `A` add a subtask, `d` delete (closing with `Esc` / `q` is in `handle_normal_key`)
    pub(super) fn handle_day_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_day(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_day(-1),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('r') => self.start_moving(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            _ => {}
        }
    }

    /// Chooses the item `step` items further down in the popup (negative = up); stops at the first / last item
    pub(super) fn choose_day(&mut self, step: isize) {
        let last = self.items_on(self.selected).len().saturating_sub(1);
        self.day_index = self.day_index.saturating_add_signed(step).min(last);
    }

    /// Left click while the popup is open: clicking an item chooses it; clicking the chosen item again
    /// checks it (like the agenda; the first click after opening only chooses, see `day_clicked`).
    /// A click outside the box closes the popup (like `Esc`); clicks on the border or empty space are ignored
    pub(super) fn click_day(&mut self, column: u16, row: u16) {
        if !ui::day_area(self).contains(Position::new(column, row)) {
            self.focus = Focus::Calendar;
            return;
        }
        match ui::day_item_at(self, column, row) {
            Some(index) if index == self.day_index && self.day_clicked => self.toggle_chosen(),
            Some(index) => {
                self.day_index = index;
                self.day_clicked = true;
            }
            None => {}
        }
    }

    /// After a write, chooses the item on file line `line` in the popup. If it is no longer on the selected
    /// day (e.g. its date changed), the choice stays at the same position
    pub(super) fn choose_day_line(&mut self, line: usize) {
        if let Some(index) = self
            .items_on(self.selected)
            .iter()
            .position(|item| item.line == line)
        {
            self.day_index = index;
        }
        self.keep_day_choice();
    }

    /// The day has fewer items (after a delete, an undo, or a date change): if the choice is now out of range,
    /// choose the last item; if no items are left, close the popup
    pub(super) fn keep_day_choice(&mut self) {
        match self.items_on(self.selected).len().checked_sub(1) {
            Some(last) => self.day_index = self.day_index.min(last),
            None => self.focus = Focus::Calendar,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::View;
    use super::super::write::tests::{app_with_task_file, clean_up, press};
    use super::*;
    use crate::texts::Lang;
    use jiff::civil::date;
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;
    use std::time::{Duration, Instant};

    /// 10/17 has 4 items. Popup order: "Trip to NYC" (starts 10/16), "Triage bugs" (no time),
    /// "Team sync" 10:00, "Weekly update" 17:30; 10/20 has one more
    const DAY: &str = "# Work\n- [ ] Weekly update @2026-10-17 17:30\n- Team sync @2026-10-17 10:00\n\
                       - [x] Triage bugs @2026-10-17\n- Trip to NYC @2026-10-16..2026-10-18\n\
                       - [ ] Deploy to prod @2026-10-20\n";

    /// An app in layout 2 with 10/17 selected and the keyboard on the calendar (no task file)
    fn month_app() -> App {
        let mut app = App::new(
            Lang::ZhTw,
            date(2026, 10, 17),
            crate::tasks::parse(DAY).items,
        );
        app.view = View::Month;
        app.screen = Rect::new(0, 0, 60, 20);
        app
    }

    /// Title of the item chosen in the popup
    fn chosen(app: &App) -> Option<String> {
        app.chosen_item().map(|item| item.title.clone())
    }

    /// One left click / one wheel step at (`column`, `row`)
    fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) {
        app.handle_mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    }

    #[test]
    fn enter_opens_the_days_items_and_esc_or_q_closes_them() {
        let mut app = month_app();
        press(&mut app, KeyCode::Enter);
        assert_eq!((app.view, app.focus), (View::Month, Focus::Day));
        assert_eq!(chosen(&app).as_deref(), Some("Trip to NYC"));
        // j stops at the last item; k goes back one; Tab is ignored
        for _ in 0..10 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(chosen(&app).as_deref(), Some("Weekly update"));
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("Team sync"))
        );
        // Esc closes it, back to the calendar; reopening chooses the first item; q also just closes, no quit
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.view, app.focus), (View::Month, Focus::Calendar));
        press(&mut app, KeyCode::Enter);
        assert_eq!(chosen(&app).as_deref(), Some("Trip to NYC"));
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(app.focus, Focus::Calendar);
        assert!(!app.should_quit);
        // A day with no items (10/19): does not open
        app.selected = date(2026, 10, 19);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn only_a_double_click_opens_the_day_from_the_month_grid() {
        let mut app = month_app();
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let (oct16, oct17, oct18) = (date(2026, 10, 16), date(2026, 10, 17), date(2026, 10, 18));
        // The selected day clicked again 0.9 s later: only selected, no popup
        app.click_calendar_day(oct17, at(0));
        app.click_calendar_day(oct17, at(900));
        assert_eq!(app.focus, Focus::Calendar);
        // Another click 0.3 s after that one: a double click, the popup opens
        app.click_calendar_day(oct17, at(1200));
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("Trip to NYC"))
        );
        // Two quick clicks on different days: the second day is selected, no popup
        press(&mut app, KeyCode::Esc);
        app.click_calendar_day(oct16, at(2000));
        app.click_calendar_day(oct17, at(2200));
        assert_eq!((app.selected, app.focus), (oct17, Focus::Calendar));
        // A day that was not selected, double-clicked: the first click selects it, the second opens it
        app.click_calendar_day(oct18, at(3000));
        app.click_calendar_day(oct18, at(3300));
        assert_eq!((app.selected, app.focus), (oct18, Focus::Day));
    }

    #[test]
    fn the_mouse_needs_two_quick_presses_to_open_the_day() {
        // October on top of the grid; find the cell that maps to 10/17
        let mut app = month_app();
        app.month_top = Some(date(2026, 9, 28));
        let (column, row) = (0..60)
            .flat_map(|x| (0..20).map(move |y| (x, y)))
            .find(|&(x, y)| ui::click_at(&app, x, y) == Some(ui::Click::Day(date(2026, 10, 17))))
            .expect("10/17 is on screen");
        // One press on the selected day: nothing opens
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            column,
            row,
        );
        assert_eq!(app.focus, Focus::Calendar);
        // A second press right away: a double click
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            column,
            row,
        );
        assert_eq!(app.focus, Focus::Day);
    }

    #[test]
    fn clicks_and_the_wheel_choose_in_the_popup() {
        let mut app = month_app();
        press(&mut app, KeyCode::Enter);
        let area = ui::day_area(&app);
        // Click the 3rd item (3rd row below the border): chooses "Team sync"; a click on the border is ignored
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            area.x + 3,
            area.y + 3,
        );
        assert_eq!(chosen(&app).as_deref(), Some("Team sync"));
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            area.x,
            area.y + 3,
        );
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("Team sync"))
        );
        // The wheel moves down one item and stops at the last one
        mouse(&mut app, MouseEventKind::ScrollDown, 0, 0);
        mouse(&mut app, MouseEventKind::ScrollDown, 0, 0);
        assert_eq!(chosen(&app).as_deref(), Some("Weekly update"));
        // Click outside the box: closes it
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 0, 0);
        assert_eq!(app.focus, Focus::Calendar);
    }

    /// An app with a task file, in layout 2, 10/17 selected, the popup open
    fn popup_with_file(name: &str) -> (App, std::path::PathBuf) {
        let (mut app, path) = app_with_task_file(name, DAY);
        app.view = View::Month;
        app.focus = Focus::Calendar;
        app.selected = date(2026, 10, 17);
        app.screen = Rect::new(0, 0, 60, 20);
        press(&mut app, KeyCode::Enter);
        (app, path)
    }

    /// Types text into the editor box one character at a time
    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn checking_and_editing_keep_the_popup_on_the_same_item() {
        let (mut app, path) = popup_with_file("day-popup-edit");
        let read = || std::fs::read_to_string(&path).expect("read");
        // Space on "Triage bugs" (2nd item): unchecks it
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(' '));
        assert!(read().contains("- [ ] Triage bugs @2026-10-17\n"));
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("Triage bugs"))
        );
        // Click the chosen item: the first click after opening only chooses it (so the second click of a
        // double click on the selected day does not check it by accident). Clicking again checks it, and the
        // file is back as it was
        let area = ui::day_area(&app);
        let click = |app: &mut App| {
            mouse(
                app,
                MouseEventKind::Down(MouseButton::Left),
                area.x + 3,
                area.y + 2,
            );
        };
        click(&mut app);
        assert!(read().contains("- [ ] Triage bugs @2026-10-17\n"));
        click(&mut app);
        assert_eq!(read(), DAY);
        // e, add 18:00: it moves to the end, and the popup still has it chosen
        press(&mut app, KeyCode::Char('e'));
        type_text(&mut app, " 18:00");
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("- [x] Triage bugs @2026-10-17 18:00\n"));
        assert_eq!(
            (app.focus, app.day_index, chosen(&app).as_deref()),
            (Focus::Day, 3, Some("Triage bugs"))
        );
        clean_up(&path);
    }

    #[test]
    fn a_reschedule_that_does_not_happen_goes_back_to_the_same_day() {
        use super::super::write::Notice;
        let (mut app, path) = popup_with_file("day-popup-not-moved");
        let chosen_day = |app: &App| (app.selected, app.focus, chosen(app));
        let before = (
            date(2026, 10, 17),
            Focus::Day,
            Some("Triage bugs".to_string()),
        );
        // "Triage bugs": r → 3 days later → Esc cancels: back to the 10/17 popup, still chosen
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('r'));
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('l'));
        }
        press(&mut app, KeyCode::Esc);
        assert_eq!(chosen_day(&app), before);
        // r again → 3 days later → the task file is gone, Enter: not moved, back to the 10/17 popup as well,
        // and the bottom row says it could not write
        press(&mut app, KeyCode::Char('r'));
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('l'));
        }
        std::fs::remove_file(&path).expect("remove");
        press(&mut app, KeyCode::Enter);
        assert_eq!(chosen_day(&app), before);
        assert!(matches!(app.notice, Some(Notice::Failed(_))));
        clean_up(&path);
    }

    #[test]
    fn deleting_moving_and_adding_from_the_popup() {
        let (mut app, path) = popup_with_file("day-popup-write");
        let read = || std::fs::read_to_string(&path).expect("read");
        // Last item ("Weekly update"): r → l → Enter: moved to 10/18; the popup switches to 10/18
        // with it chosen ("Trip to NYC", "Weekly update")
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('j'));
        }
        press(&mut app, KeyCode::Char('r'));
        assert_eq!(app.focus, Focus::Calendar);
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Enter);
        assert!(read().contains("- [ ] Weekly update @2026-10-18 17:30\n"));
        assert_eq!(app.selected, date(2026, 10, 18));
        assert_eq!(
            (app.focus, app.day_index, chosen(&app).as_deref()),
            (Focus::Day, 1, Some("Weekly update"))
        );
        // u: "Weekly update" goes back to 10/17; 10/18 only has "Trip to NYC" left, and the choice
        // shrinks with it
        press(&mut app, KeyCode::Char('u'));
        assert_eq!(read(), DAY);
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("Trip to NYC"))
        );
        // a: prefilled with 10/18, added at the end of "Work"; the popup has the new item chosen
        press(&mut app, KeyCode::Char('a'));
        type_text(&mut app, "File expenses");
        press(&mut app, KeyCode::Enter);
        assert!(
            read().ends_with("- [ ] Deploy to prod @2026-10-20\n- [ ] File expenses @2026-10-18\n")
        );
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("File expenses"))
        );
        // d, y deletes "File expenses": chooses the last item ("Trip to NYC"); deleting "Trip to NYC" too
        // leaves the day empty, so the popup closes, still in layout 2
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(
            (app.focus, chosen(&app).as_deref()),
            (Focus::Day, Some("Trip to NYC"))
        );
        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert!(!read().contains("Trip to NYC"));
        assert_eq!((app.view, app.focus), (View::Month, Focus::Calendar));
        clean_up(&path);
    }
}
