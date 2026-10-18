//! App state and the main loop: draw the screen → wait for input → update the state, until quit

mod day;
mod editor;
mod help;
mod moving;
mod settings;
mod tasks;
mod write;

pub use editor::{Editor, Reading, Target};
pub use help::Help;
pub use moving::Moving;
pub use settings::{Row, SettingsPage, rows};
pub use tasks::TaskRow;
pub use write::Notice;

use crate::{
    config::Settings,
    tasks::{HeadingLine, Item, Warning},
    texts::Lang,
    theme::Theme,
    ui::{self, Click},
};
use color_eyre::Result;
use jiff::{ToSpan, civil::Date};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{
        self, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    layout::{Position, Rect},
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Two clicks on the same day at most this far apart count as a double click
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// Which pane has the keyboard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The calendar on the left: `hjkl` move the selected day
    Calendar,
    /// The agenda on the right: `j` / `k` choose items one by one
    Agenda,
    /// The side menu at the far left (always shown when wide enough; otherwise `s` calls it up over the left):
    /// `j` / `k` choose a calendar, `Space` checks it
    Side,
    /// The tasks page (`4`): `j` / `k` choose items one by one, `l` / `h` expand / collapse subtasks
    Tasks,
    /// Layout 3's day popup (`Enter` on the calendar): `j` / `k` choose the selected day's items one by one
    Day,
}

/// Which layout is shown (number keys switch)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Layout 1 (`1`): calendar on the left; on the right, each month's items line up with that month
    Planner,
    /// Layout 3 (`3`): the big month grid, 6 weeks at a time; each cell lists that day's items
    Month,
    /// Tasks page (`4`): the whole task file listed by calendar, subtasks as a tree, undated items included
    Tasks,
}

/// One calendar in the side menu (= one group in the task file)
#[derive(Debug)]
pub struct Group {
    /// Group name; `None` = items at the start of the file, before any heading
    pub name: Option<String>,
    /// Whether it is checked (all are checked at start)
    pub shown: bool,
}

impl Group {
    /// All groups in the task file, in order of first appearance: the groups of `items` and every heading
    /// in `headings` (so a heading with no items under it yet is a calendar too); headings with the same
    /// name count as one
    fn list(items: &[Item], headings: &[HeadingLine]) -> Vec<Self> {
        let mut names: Vec<(usize, Option<String>)> = items
            .iter()
            .map(|item| {
                let name = item.group.as_ref().map(|heading| heading.name.clone());
                (item.line, name)
            })
            .chain(
                headings
                    .iter()
                    .map(|heading| (heading.line, Some(heading.heading.name.clone()))),
            )
            .collect();
        names.sort_by_key(|&(line, _)| line);
        let mut groups: Vec<Self> = Vec::new();
        for (_, name) in names {
            if !groups.iter().any(|group| group.name == name) {
                groups.push(Self { name, shown: true });
            }
        }
        groups
    }

    /// Whether `item` belongs to this group
    pub fn includes(&self, item: &Item) -> bool {
        self.name.as_deref() == item.group.as_ref().map(|heading| heading.name.as_str())
    }
}

/// One row in the agenda: which item, and on which day it is listed (the order from `App::agenda()` is the
/// agenda's top-to-bottom order)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    /// Which item
    pub item: &'a Item,
    /// The day it is listed on: usually the item's day; a multi-day item that spans months is listed on the
    /// 1st in each later month
    pub date: Date,
}

impl Entry<'_> {
    /// Whether this row is an item on `day`: the item covers `day` (every day of a multi-day item counts),
    /// and this row is listed in `day`'s month
    /// (an item spanning months has one row per month; only that month's row counts)
    pub fn is_on(&self, day: Date) -> bool {
        self.item.covers(day) && self.date.first_of_month() == day.first_of_month()
    }
}

/// Things drawn over the screen that get keys and the mouse first; only one at a time
///
/// Kept in one enum (not several `Option`s): keys, mouse, and drawing all `match` on it, so when a new
/// one is added, the compiler catches every place that does not handle it
#[derive(Debug, Default)]
pub enum Mode {
    /// Nothing open: keys go to the pane that has the keyboard
    #[default]
    Normal,
    /// Help page (`?`)
    Help(Help),
    /// Settings page (`,`): the page's state, and the settings when it opened (closing saves only what
    /// changed since then)
    Settings {
        page: SettingsPage,
        opened: Settings,
    },
    /// After `d`, asking whether to delete: the item to delete and how many items that removes (with subtasks)
    ConfirmDelete(Item, usize),
    /// Editor box (`e`, `a`, `A`)
    Editor(Editor),
    /// Rescheduling (`r`): which item to move
    Moving(Moving),
}

/// State of the whole app
#[derive(Debug)]
pub struct App {
    /// Screen language
    pub lang: Lang,
    /// Today (the day the app was opened; tests can set any day)
    pub today: Date,
    /// The selected day: the one in brackets `[17]` on the calendar
    pub selected: Date,
    /// All items in the task file
    pub items: Vec<Item>,
    /// Mistakes in the task file (e.g. `@2026-13-01`), marked on the tasks page
    pub warnings: Vec<Warning>,
    /// Every heading in the task file (the calendars' lines, for renaming and deleting a calendar)
    pub headings: Vec<HeadingLine>,
    /// Calendars in the side menu, in task file order
    pub groups: Vec<Group>,
    /// The side menu was hidden with `s`
    pub side_hidden: bool,
    /// Which calendar is chosen in the side menu (index in `groups`); used only when the keyboard is there
    pub group_index: usize,
    /// Side menu scroll: which calendar is at the top (index in `groups`); it scrolls only when the calendars
    /// do not fit
    pub side_top: usize,
    /// Which pane had the keyboard before `s` called up the side menu when narrow (it goes back there on hide)
    side_back: Focus,
    /// Which pane has the keyboard
    pub focus: Focus,
    /// Which layout is shown
    pub view: View,
    /// Which item is chosen in the agenda (index in `agenda()`); used only when the keyboard is there
    pub agenda_index: usize,
    /// Which item is chosen in layout 3's popup (index in `items_on(selected)`);
    /// used only when the keyboard is in the popup
    pub day_index: usize,
    /// Whether the mouse has clicked in the popup since it opened; before that, clicking the chosen item only
    /// chooses it and does not check it
    /// (double-clicking the selected day: the first click opens the popup, the second lands on its first item)
    pub day_clicked: bool,
    /// Popup scroll: which item is at the top (it scrolls only when the chosen item leaves the box)
    pub day_top: usize,
    /// The day and time of the last click on a calendar day (both layouts), to tell a double click
    last_click: Option<(Date, Instant)>,
    /// Calendar scroll: which row is at the top of the screen (`None` = on the next draw, put the selected
    /// day's month at the top)
    pub calendar_top: Option<i64>,
    /// Agenda scroll: (the 1st of which month, which sorted row of that month is at the top); it scrolls only
    /// when a month has too many items to fit
    pub agenda_window: Option<(Date, usize)>,
    /// Layout 3 scroll: the Monday of the top week (`None` = on the next draw, put the week before the 1st of
    /// the selected month at the top)
    pub month_top: Option<Date>,
    /// Screen colors
    pub theme: Theme,
    /// Settings from the config file (theme, colors, grid lines, relative week numbers…);
    /// the settings page changes them
    pub settings: Settings,
    /// Where the config file is (saved back when the settings page closes); `None` = do not save (tests, or
    /// no place for a config file)
    pub config_path: Option<PathBuf>,
    /// Terminal size (updated before each draw); needed to map mouse click positions
    pub screen: Rect,
    /// Where the task file is (checking and editing write back to it); `None` = no task file (tests), no writes
    pub tasks_path: Option<PathBuf>,
    /// A notice that temporarily replaces the bottom row (write failed, reloaded, undo); gone on the next
    /// key or click
    pub notice: Option<Notice>,
    /// Writes roh-cal has made (for `u` to undo), newest last
    undo: Vec<write::UndoStep>,
    /// The popup that is open or the action in progress (help page, settings page, delete question, editor
    /// box, rescheduling); only one at a time
    pub mode: Mode,
    /// File line of the item chosen on the tasks page; `None` = nothing chosen yet (choose the first)
    pub tasks_line: Option<usize>,
    /// Items on the tasks page with their subtasks expanded (file line numbers)
    pub expanded: BTreeSet<usize>,
    /// Tasks page scroll: which row is at the top of the screen (index in `tasks_rows()`)
    pub tasks_top: usize,
    /// `true` = leave the main loop after this round
    should_quit: bool,
}

impl App {
    /// State at start: keyboard on the calendar, today selected, theme is Catppuccin Mocha
    pub fn new(lang: Lang, today: Date, items: Vec<Item>) -> Self {
        Self {
            lang,
            today,
            selected: today,
            groups: Group::list(&items, &[]),
            side_hidden: false,
            group_index: 0,
            side_top: 0,
            side_back: Focus::Calendar,
            items,
            warnings: Vec::new(),
            headings: Vec::new(),
            focus: Focus::Calendar,
            view: View::Planner,
            agenda_index: 0,
            day_index: 0,
            day_clicked: false,
            day_top: 0,
            last_click: None,
            calendar_top: None,
            agenda_window: None,
            month_top: None,
            theme: Theme::catppuccin_mocha(),
            settings: Settings::default(),
            config_path: None,
            screen: Rect::default(),
            tasks_path: None,
            notice: None,
            undo: Vec::new(),
            mode: Mode::Normal,
            tasks_line: None,
            expanded: BTreeSet::new(),
            tasks_top: 0,
            should_quit: false,
        }
    }

    /// The help page's state, if it is open
    pub fn help(&self) -> Option<Help> {
        match self.mode {
            Mode::Help(help) => Some(help),
            _ => None,
        }
    }

    /// The settings page's state, if it is open
    pub fn settings_page(&self) -> Option<SettingsPage> {
        match self.mode {
            Mode::Settings { page, .. } => Some(page),
            _ => None,
        }
    }

    /// If asking whether to delete: the item to delete and how many items that removes
    pub fn confirm_delete(&self) -> Option<(&Item, usize)> {
        match &self.mode {
            Mode::ConfirmDelete(item, count) => Some((item, *count)),
            _ => None,
        }
    }

    /// The editor box's state, if it is open
    pub fn editor(&self) -> Option<&Editor> {
        match &self.mode {
            Mode::Editor(editor) => Some(editor),
            _ => None,
        }
    }

    /// If rescheduling: which item is moving
    pub fn moving(&self) -> Option<&Moving> {
        match &self.mode {
            Mode::Moving(moving) => Some(moving),
            _ => None,
        }
    }

    /// Applies the settings to the screen colors (`settings.theme` plus overrides in `settings.colors`). If
    /// they cannot be built into a theme, nothing changes (the config file is checked on load and theme changes
    /// go through `Settings::set_theme`, so normally it always builds)
    pub fn apply_theme(&mut self) {
        if let Ok(theme) = Theme::build(&self.settings.theme, &self.settings.colors) {
            self.theme = theme;
        }
    }

    /// Whether this item is shown: only if its calendar (group) is checked
    pub fn is_shown(&self, item: &Item) -> bool {
        self.groups
            .iter()
            .find(|group| group.includes(item))
            .is_none_or(|group| group.shown)
    }

    /// Whether `day` is a holiday: an item in a checked holiday calendar (its heading has `type:holiday`)
    /// covers that day
    pub fn is_holiday(&self, day: Date) -> bool {
        self.items.iter().any(|item| {
            item.group.as_ref().is_some_and(|group| group.holiday)
                && self.is_shown(item)
                && item.covers(day)
        })
    }

    /// Items listed in the agenda: ones with a date whose calendar is checked, sorted by listed day and time
    /// (on the same day, items without a time come first; same date and time keep file order)
    ///
    /// A multi-day item that spans months is listed once in each month it touches: in the first month on its
    /// start day, in later months on the 1st, at the top of that month.
    /// Items without a date are not listed for now (they have no place on the calendar)
    pub fn agenda(&self) -> Vec<Entry<'_>> {
        let mut entries = Vec::new();
        for item in self.items.iter().filter(|item| self.is_shown(item)) {
            let (Some(first), Some(last)) = (item.date, item.last_day()) else {
                continue;
            };
            entries.push(Entry { item, date: first });
            // Each later month it touches (on the 1st)
            let months = first
                .first_of_month()
                .series(1.month())
                .skip(1)
                .take_while(|&month| month <= last);
            entries.extend(months.map(|date| Entry { item, date }));
        }
        // sort_by_key is a stable sort: equal keys keep their order. On the same day, items carried over from
        // the previous month come first, then ones without a time (`None` sorts before `Some`)
        entries.sort_by_key(|entry| {
            let continued = Some(entry.date) != entry.item.date;
            (entry.date, !continued, entry.item.time)
        });
        entries
    }

    /// Items on `day` (calendar checked; a multi-day item counts if it covers that day): earlier starts
    /// first, items starting the same day by time
    /// (no time first; same time keeps file order). Layout 3's cells and `a` on the calendar (to find that
    /// day's group) both use this
    pub fn items_on(&self, day: Date) -> Vec<&Item> {
        let mut items: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| item.covers(day) && self.is_shown(item))
            .collect();
        items.sort_by_key(|item| (item.date, item.time));
        items
    }

    /// The chosen row when the keyboard is in the agenda; `None` when it is on the calendar
    pub fn chosen(&self) -> Option<Entry<'_>> {
        match self.focus {
            Focus::Agenda => self.agenda().get(self.agenda_index).copied(),
            Focus::Calendar | Focus::Side | Focus::Tasks | Focus::Day => None,
        }
    }

    /// The item the keyboard has chosen: in the agenda, on the tasks page, or in layout 3's popup; `None` on
    /// the calendar or side menu (checking, editing, deleting, and rescheduling all act on this item)
    fn chosen_item(&self) -> Option<&Item> {
        match self.focus {
            Focus::Agenda => self.chosen().map(|entry| entry.item),
            Focus::Tasks => self.tasks_chosen(),
            Focus::Day => self.items_on(self.selected).get(self.day_index).copied(),
            Focus::Calendar | Focus::Side => None,
        }
    }

    /// Whether the side menu takes up space at the far left: the terminal is wide enough and `s` has not
    /// hidden it (the one called up over the left when narrow does not count)
    fn side_shown(&self) -> bool {
        ui::side_fits(self.screen) && !self.side_hidden
    }

    /// Records the terminal size. If the terminal gets wide enough while the side menu is hidden and has the
    /// keyboard, the keyboard goes back to the main pane
    /// (when narrow, the keyboard in the side menu = it is shown over the left side and still works)
    fn resize(&mut self, screen: Rect) {
        self.screen = screen;
        if self.focus == Focus::Side && ui::side_fits(screen) && self.side_hidden {
            self.focus = self.home_focus();
        }
    }

    /// Hides the side menu that was called up when narrow: the keyboard goes back to the pane it was in
    /// before (for the agenda and layout 3's popup, the choice stays;
    /// if unchecked calendars left fewer items, the last one is chosen). If that pane is gone now (the layout
    /// changed, no items left), it goes back to the main pane
    fn put_side_away(&mut self) {
        self.focus = self.home_focus();
        match (self.side_back, self.view) {
            (Focus::Agenda, View::Planner) => {
                if let Some(last) = self.agenda().len().checked_sub(1) {
                    self.choose(self.agenda_index.min(last));
                }
            }
            (Focus::Day, View::Month) => {
                self.focus = Focus::Day;
                self.keep_day_choice();
            }
            _ => {}
        }
    }

    /// This layout's main pane: the calendar (layouts 1, 3) or the tasks page; `Esc` and hiding the side menu
    /// bring the keyboard back here
    fn home_focus(&self) -> Focus {
        match self.view {
            View::Planner | View::Month => Focus::Calendar,
            View::Tasks => Focus::Tasks,
        }
    }

    /// Main loop: each round draws the screen once and waits for one input, until quit
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            // Record the terminal size first (it changes when the window is resized),
            // so drawing and mouse positions agree
            let size = terminal.size()?;
            self.resize(Rect::new(0, 0, size.width, size.height));
            terminal.draw(|frame| ui::render(frame, &mut self))?;
            // event::read() blocks until there is input; only key presses are handled (some terminals also send
            // key releases)
            let event = event::read()?;
            if let Some(key) = event.as_key_press_event() {
                self.handle_key(key);
            } else if let Some(mouse) = event.as_mouse_event() {
                self.handle_mouse(mouse);
            }
        }
        Ok(())
    }

    /// Keys: the open popup or action in progress (`mode`) gets them first; otherwise see `handle_normal_key`
    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        // The last notice has been seen: any key clears it
        self.notice = None;
        match self.mode {
            // Help page: only scrolling, switching version, closing
            Mode::Help(_) => self.handle_help_key(key),
            // Settings page: only choosing a row, changing a value, resetting, closing
            Mode::Settings { .. } => self.handle_settings_key(key),
            // Asking whether to delete: `y` deletes, any other key just cancels
            Mode::ConfirmDelete(..) => self.answer_delete(key),
            // Editor box: keys are typing (Enter saves, Esc cancels)
            Mode::Editor(_) => self.handle_editor_key(key),
            // Rescheduling: only moving the date, confirming, cancelling
            Mode::Moving(_) => self.handle_moving_key(key),
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    /// Keys when no popup is open: `Tab` switches pane, `[` / `]` switch month, `t` back to today, `u` undo,
    /// `1` / `3` / `4` switch layout, `s` hides / shows the side menu, `?` help page, `,` settings page,
    /// `q` / `Ctrl+C` quit. `Esc` goes back to this layout's main pane;
    /// when already there: from the tasks page back to layout 1, from the calendar it quits. While layout 3's
    /// popup or the side menu called up when narrow is open, `Esc` / `q` only close it.
    /// Other keys depend on which pane has the keyboard
    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            // While the popup or the side menu called up when narrow is open, q only closes it
            // (like the help page) and does not quit
            KeyCode::Char('q') if self.focus == Focus::Day => self.focus = Focus::Calendar,
            KeyCode::Char('q') if ui::side_overlay(self).is_some() => self.put_side_away(),
            KeyCode::Char('q') => self.should_quit = true,
            // In raw mode Ctrl+C does not send SIGINT to end the program, so handle it here
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            // The side menu called up when narrow: hide it; the keyboard goes back to the pane from before
            KeyCode::Esc if ui::side_overlay(self).is_some() => self.put_side_away(),
            // Keyboard not in the main pane (agenda, side menu): back to the main pane
            KeyCode::Esc if self.focus != self.home_focus() => self.focus = self.home_focus(),
            // Tasks page: back to layout 1's calendar; calendar: quit
            KeyCode::Esc if self.view == View::Tasks => self.set_view(View::Planner),
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.next_focus(),
            KeyCode::BackTab => self.previous_focus(),
            // Back to today and scroll its month to the top (the tasks page has no calendar, so ignored)
            KeyCode::Char('t') if self.view != View::Tasks => {
                self.go_today();
                self.focus = Focus::Calendar;
            }
            // Undo the last write (check, edit, …)
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('?') => self.mode = Mode::Help(Help::default()),
            KeyCode::Char(',') => self.open_settings(),
            KeyCode::Char('1') => self.set_view(View::Planner),
            KeyCode::Char('3') => self.set_view(View::Month),
            KeyCode::Char('4') => self.set_view(View::Tasks),
            // 1st of the previous / next month, scrolled to the top (ignored on the tasks page: no calendar)
            KeyCode::Char('[') if self.view != View::Tasks => self.jump_month(-1),
            KeyCode::Char(']') if self.view != View::Tasks => self.jump_month(1),
            // Terminal wide enough: hide / show the side menu; when hiding it while it has the keyboard,
            // go back to the main pane
            KeyCode::Char('s') if ui::side_fits(self.screen) => {
                self.side_hidden = !self.side_hidden;
                if self.focus == Focus::Side {
                    self.focus = self.home_focus();
                }
            }
            // Not wide enough: call up / hide the side menu over the left side (keyboard in the side menu
            // = it is called up); hiding it returns the keyboard to the pane from before
            KeyCode::Char('s') if self.focus == Focus::Side => self.put_side_away(),
            KeyCode::Char('s') => {
                self.side_back = self.focus;
                self.focus = Focus::Side;
            }
            _ => match self.focus {
                Focus::Calendar => self.handle_calendar_key(key.code),
                Focus::Agenda => self.handle_agenda_key(key.code),
                Focus::Side => self.handle_side_key(key.code),
                Focus::Tasks => self.handle_tasks_key(key.code),
                Focus::Day => self.handle_day_key(key.code),
            },
        }
    }

    /// Keyboard on the calendar: left / right = previous / next day, up / down = previous / next week
    /// (like vim); `a` adds an item on the selected day,
    /// `Enter` goes into that day's items
    fn handle_calendar_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('h') | KeyCode::Left => self.move_by(-1),
            KeyCode::Char('l') | KeyCode::Right => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-7),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(7),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Enter => self.open_day(),
            _ => {}
        }
    }

    /// `Enter` on the calendar (or double-clicking a day): go into that day's items; the keyboard
    /// moves to the agenda and the day's first item is chosen;
    /// layout 3 has no agenda, so it opens the day popup instead; nothing happens if the day has no items
    fn open_day(&mut self) {
        if self.view == View::Month {
            self.open_day_popup();
            return;
        }
        if !self.agenda().iter().any(|entry| entry.is_on(self.selected)) {
            return;
        }
        self.enter_agenda();
    }

    /// Keyboard in the agenda: up / down one item, and the calendar's selected day follows the item;
    /// `Space` / `x` check, `r` reschedule, `e` / `Enter` edit,
    /// `a` add, `A` add a subtask, `d` delete (all write back to the task file)
    fn handle_agenda_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_next(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_next(-1),
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('r') => self.start_moving(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            _ => {}
        }
    }

    /// Keyboard in the side menu: up / down choose a calendar, `Space` / `Enter` check or uncheck it (an
    /// unchecked calendar's items are hidden), `a` adds a calendar
    fn handle_side_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_group(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_group(-1),
            KeyCode::Char(' ') | KeyCode::Enter => self.toggle_group(),
            KeyCode::Char('a') => self.start_adding_calendar(),
            _ => {}
        }
    }

    /// The calendar chosen in the side menu: checked ⇄ unchecked
    fn toggle_group(&mut self) {
        if let Some(group) = self.groups.get_mut(self.group_index) {
            group.shown = !group.shown;
        }
    }

    /// Mouse: the open popup or action in progress (`mode`) gets it first; otherwise see `handle_normal_mouse`
    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent) {
        // The last notice has been seen, but only a press or the wheel counts; release, move, and drag do not.
        // A click sends a press and then a release,
        // and if the release cleared it too, a notice from the click (e.g. an event cannot be checked) would
        // vanish right away
        if matches!(
            mouse.kind,
            MouseEventKind::Down(_) | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            self.notice = None;
        }
        match self.mode {
            // Help page: the wheel scrolls it, a click outside closes it
            Mode::Help(_) => self.handle_help_mouse(mouse),
            // Settings page: the wheel chooses rows, clicks on rows, a click outside closes it
            Mode::Settings { .. } => self.handle_settings_mouse(mouse),
            // Asking whether to delete: a click or the wheel just cancels (like any key other than `y`);
            // that event does nothing else
            Mode::ConfirmDelete(..) => {
                if matches!(
                    mouse.kind,
                    MouseEventKind::Down(_) | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) {
                    self.mode = Mode::Normal;
                }
            }
            // Editor box: a click outside = cancel (Esc); everything else is ignored
            Mode::Editor(_) => {
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && !ui::editor_area(self.screen)
                        .contains(Position::new(mouse.column, mouse.row))
                {
                    self.mode = Mode::Normal;
                }
            }
            // Rescheduling: the wheel moves the selected day as usual, clicking a date picks the new date,
            // other clicks are ignored
            Mode::Moving(_) => self.handle_moving_mouse(mouse),
            Mode::Normal => self.handle_normal_mouse(mouse),
        }
    }

    /// Mouse when no popup is open: the wheel moves up / down (a week on the calendar, one item in the agenda
    /// or on the tasks page, one calendar in the side menu);
    /// a left click on a calendar date selects it (a double click = go into that day's items), on an agenda item
    /// chooses it (again = check it),
    /// on a tasks page row chooses it (again = check it; on `▸` = expand / collapse), on a side menu calendar
    /// chooses it (again = check / uncheck)
    fn handle_normal_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            // Scroll down (to see later ones), scroll up
            MouseEventKind::ScrollDown => self.scroll(1),
            MouseEventKind::ScrollUp => self.scroll(-1),
            // Narrow, side menu over the left side: a click outside hides it (like Esc) and does nothing else
            MouseEventKind::Down(MouseButton::Left)
                if ui::side_overlay(self)
                    .is_some_and(|area| !area.contains(Position::new(mouse.column, mouse.row))) =>
            {
                self.put_side_away();
            }
            // Layout 3's popup is open: click items in it, a click outside closes it
            MouseEventKind::Down(MouseButton::Left) if self.focus == Focus::Day => {
                self.click_day(mouse.column, mouse.row);
            }
            // Only a click on a date or an item selects; clicks on headers, lines, or blank cells are ignored
            MouseEventKind::Down(MouseButton::Left) => {
                match ui::click_at(self, mouse.column, mouse.row) {
                    Some(Click::Day(date)) => self.click_calendar_day(date, Instant::now()),
                    // Clicking the chosen item again = check it
                    Some(Click::Item(index))
                        if self.focus == Focus::Agenda && self.agenda_index == index =>
                    {
                        self.toggle_chosen();
                    }
                    Some(Click::Item(index)) => self.choose(index),
                    Some(Click::Group(index)) => self.click_group(index),
                    Some(Click::TaskRow { row, on_marker }) => self.click_task(row, on_marker),
                    None => {}
                }
            }
            // Anything else (right click, drag, move) is ignored
            _ => {}
        }
    }

    /// A click on a calendar day at time `now` (both layouts): selects the day; a second click on the same
    /// day within `DOUBLE_CLICK` (a double click) goes into that day's items, like `Enter`. Clicking the
    /// selected day again later only selects it, so a stray click does not move the keyboard or open a popup
    fn click_calendar_day(&mut self, day: Date, now: Instant) {
        let double = self
            .last_click
            .is_some_and(|(last, at)| last == day && now.duration_since(at) <= DOUBLE_CLICK);
        self.selected = day;
        self.focus = Focus::Calendar;
        if double {
            // A third quick click starts over instead of counting as another double click
            self.last_click = None;
            self.open_day();
        } else {
            self.last_click = Some((day, now));
        }
    }

    /// Wheel down (`step` = 1) / up (-1): moves a week when the keyboard is on the calendar, one item in the
    /// agenda, the tasks page, or the popup, and one calendar in the side menu
    fn scroll(&mut self, step: i8) {
        match self.focus {
            Focus::Calendar => self.move_by(7 * i64::from(step)),
            Focus::Agenda => self.choose_next(isize::from(step)),
            Focus::Side => self.choose_group(isize::from(step)),
            Focus::Tasks => self.choose_task(isize::from(step)),
            Focus::Day => self.choose_day(isize::from(step)),
        }
    }

    /// Switches layout: the selected day's month scrolls to the top; layout 3 has no agenda, so a keyboard
    /// there goes back to the calendar
    fn set_view(&mut self, view: View) {
        self.view = view;
        self.scroll_to_month();
        // The side menu stays; other panes go to this layout's main pane (layout 3 has no agenda, the tasks
        // page has no calendar)
        if self.focus != Focus::Side {
            self.focus = match (view, self.focus) {
                (View::Planner, Focus::Agenda) => Focus::Agenda,
                _ => self.home_focus(),
            };
        }
    }

    /// `t`: selects today and scrolls today's month to the top
    fn go_today(&mut self) {
        self.selected = self.today;
        self.scroll_to_month();
    }

    /// On the next draw, scroll the selected day's month to the top (in both layouts)
    fn scroll_to_month(&mut self) {
        self.calendar_top = None;
        self.month_top = None;
    }

    /// `Tab`: calendar → agenda (skipped if there are no items, or in layout 3) → side menu (only if drawn)
    /// → calendar;
    /// on the tasks page: tasks page ⇄ side menu (only if drawn); ignored while layout 3's popup is open
    fn next_focus(&mut self) {
        match self.focus {
            Focus::Day => {}
            Focus::Calendar => {
                if self.view == View::Planner {
                    self.enter_agenda();
                }
                if self.focus == Focus::Calendar && self.side_shown() {
                    self.focus = Focus::Side;
                }
            }
            Focus::Agenda | Focus::Tasks if self.side_shown() => self.focus = Focus::Side,
            Focus::Agenda | Focus::Side | Focus::Tasks => self.focus = self.home_focus(),
        }
    }

    /// `Shift+Tab`: `Tab` backwards: calendar → side menu (only if drawn) → agenda (skipped if there are no
    /// items, or in layout 3) → calendar;
    /// on the tasks page: tasks page ⇄ side menu (only if drawn); ignored while layout 3's popup is open
    fn previous_focus(&mut self) {
        match self.focus {
            Focus::Day => {}
            Focus::Calendar | Focus::Tasks if self.side_shown() => self.focus = Focus::Side,
            Focus::Calendar => {
                if self.view == View::Planner {
                    self.enter_agenda();
                }
            }
            Focus::Side if self.view == View::Planner => {
                self.enter_agenda();
                if self.focus == Focus::Side {
                    self.focus = Focus::Calendar;
                }
            }
            Focus::Agenda | Focus::Side | Focus::Tasks => self.focus = self.home_focus(),
        }
    }

    /// Click on calendar number `index` in the side menu: one click chooses it (the keyboard moves to the
    /// side menu); clicking the chosen one again = check / uncheck
    fn click_group(&mut self, index: usize) {
        if self.focus == Focus::Side && self.group_index == index {
            self.toggle_group();
        } else {
            self.focus = Focus::Side;
            self.group_index = index;
        }
    }

    /// Moves the side menu choice `step` calendars down (negative = up); stops at the first / last one
    fn choose_group(&mut self, step: isize) {
        let last = self.groups.len().saturating_sub(1);
        self.group_index = self.group_index.saturating_add_signed(step).min(last);
    }

    /// Moves the keyboard to the agenda: chooses the first item on or after the selected day (a multi-day item
    /// that started earlier but covers the selected day sorts first, so it is chosen first);
    /// if there is nothing on or after it, choose the last item; if there are no items at all, do not move
    fn enter_agenda(&mut self) {
        let index = {
            let agenda = self.agenda();
            let Some(last) = agenda.len().checked_sub(1) else {
                return;
            };
            agenda
                .iter()
                .position(|entry| entry.date >= self.selected || entry.is_on(self.selected))
                .unwrap_or(last)
        };
        self.choose(index);
    }

    /// Chooses agenda item `index`: the keyboard moves to the agenda, and the calendar selects the day the
    /// item is listed on;
    /// if the selected day is already one of the item's days (a day inside a multi-day span), it stays
    fn choose(&mut self, index: usize) {
        let Some((date, on_selected)) = self
            .agenda()
            .get(index)
            .map(|entry| (entry.date, entry.is_on(self.selected)))
        else {
            return;
        };
        self.agenda_index = index;
        if !on_selected {
            self.selected = date;
        }
        self.focus = Focus::Agenda;
    }

    /// After a write, chooses the item on file line `line` in the agenda; for an item spanning months, the row
    /// in the selected day's month.
    /// If the agenda does not have it (e.g. its date was removed, or layout 3 has no agenda), back to the
    /// calendar. On the tasks page, choose that row; in layout 3's popup, choose that item in the popup
    fn choose_line(&mut self, line: usize) {
        if self.view == View::Tasks {
            self.choose_task_line(line);
            return;
        }
        if self.focus == Focus::Day {
            self.choose_day_line(line);
            return;
        }
        if self.view != View::Planner {
            self.focus = Focus::Calendar;
            return;
        }
        let index = {
            let agenda = self.agenda();
            agenda
                .iter()
                .position(|entry| entry.item.line == line && entry.is_on(self.selected))
                .or_else(|| agenda.iter().position(|entry| entry.item.line == line))
        };
        match index {
            Some(index) => self.choose(index),
            None => self.focus = Focus::Calendar,
        }
    }

    /// Chooses the item `step` items further down (negative = up); stops at the first / last item
    fn choose_next(&mut self, step: isize) {
        let last = self.agenda().len().saturating_sub(1);
        self.choose(self.agenda_index.saturating_add_signed(step).min(last));
    }

    /// Jumps to the 1st of the month `months` months later (negative = earlier), scrolled to the top;
    /// the keyboard goes back to the calendar
    fn jump_month(&mut self, months: i64) {
        self.selected = self
            .selected
            .first_of_month()
            .saturating_add(months.months());
        self.scroll_to_month();
        self.focus = Focus::Calendar;
    }

    /// Moves the selected day this many days later (negative = earlier)
    fn move_by(&mut self, days: i64) {
        self.selected = self.selected.saturating_add(days.days());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    /// Presses one key (no modifier such as Ctrl)
    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    #[test]
    fn hjkl_and_arrows_move_by_a_day_or_a_week() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2026, 10, 24));
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 25));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected, date(2026, 10, 18));
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.selected, date(2026, 10, 17));
        // Arrow keys do the same; moving across months and years works as usual
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.selected, date(2026, 10, 31));
        press(&mut app, KeyCode::Right);
        assert_eq!(app.selected, date(2026, 11, 1));
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.selected, date(2026, 10, 24));
        app.selected = date(2026, 12, 29);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2027, 1, 5));
    }

    /// One mouse action at (`column`, `row`)
    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn wheel_moves_a_week() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(app.selected, date(2026, 10, 24));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.selected, date(2026, 10, 10));
    }

    #[test]
    fn left_click_on_a_date_selects_it() {
        // 40 × 26 screen, October at the top: the 16th is at (24-26, 5); see the screen tests in ui.rs
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.screen = Rect::new(0, 0, 40, 26);
        app.calendar_top = Some(0);
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 25, 5));
        assert_eq!(app.selected, date(2026, 10, 16));
        // Right click, click on the header: ignored
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 28, 5));
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 28, 1));
        assert_eq!(app.selected, date(2026, 10, 16));
    }

    #[test]
    fn brackets_jump_to_the_first_of_the_previous_or_next_month() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.calendar_top = Some(3);
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.selected, date(2026, 11, 1));
        // After the jump, that month must scroll to the top (on the next draw)
        assert_eq!(app.calendar_top, None);
        press(&mut app, KeyCode::Char('['));
        press(&mut app, KeyCode::Char('['));
        assert_eq!(app.selected, date(2026, 9, 1));
        // Across a year
        app.selected = date(2026, 12, 15);
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.selected, date(2027, 1, 1));
        app.selected = date(2027, 1, 20);
        press(&mut app, KeyCode::Char('['));
        assert_eq!(app.selected, date(2026, 12, 1));
    }

    /// An app with today = 2026/10/17 and `text` as the task file
    fn app_with(text: &str) -> App {
        App::new(
            Lang::En,
            date(2026, 10, 17),
            crate::tasks::parse(text).items,
        )
    }

    /// The agenda in order: 10/10 a, 10/17 b, 10/17 c, 10/20 d (the undated one is not listed)
    const ITEMS: &str =
        "- d @2026-10-20\n- c @2026-10-17 09:00\n- No date\n- b @2026-10-17\n- a @2026-10-10\n";

    #[test]
    fn tab_moves_into_the_agenda_and_back() {
        // 10/17 selected → moving to the agenda chooses the first item on 10/17 (b, with no time, comes first)
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("b")
        );
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(app.chosen(), None);
        // Nothing on or after the selected day: choose the last item, and the calendar jumps to its day
        app.selected = date(2026, 12, 1);
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("d")
        );
        assert_eq!(app.selected, date(2026, 10, 20));
    }

    #[test]
    fn shift_tab_goes_through_the_panes_backwards() {
        // Wide enough for the side menu: calendar → side menu → agenda (10/17's first item, b) → calendar
        let mut app = app_with(ITEMS);
        app.screen = Rect::new(0, 0, 140, 30);
        let mut order = Vec::new();
        for _ in 0..3 {
            press(&mut app, KeyCode::BackTab);
            order.push(app.focus);
        }
        assert_eq!(order, [Focus::Side, Focus::Agenda, Focus::Calendar]);
        // Narrow (no side menu): calendar ⇄ agenda; with no items it stays on the calendar
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(
            (
                app.focus,
                app.chosen().map(|entry| entry.item.title.as_str())
            ),
            (Focus::Agenda, Some("b"))
        );
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.focus, Focus::Calendar);
        let mut app = app_with("- No date\n");
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.focus, Focus::Calendar);
        // Side menu with no items: side menu → calendar (the agenda is skipped)
        app.screen = Rect::new(0, 0, 140, 30);
        press(&mut app, KeyCode::BackTab);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.focus, Focus::Calendar);
        // Layout 3: calendar ⇄ side menu; the day popup ignores it
        let mut app = app_with(ITEMS);
        app.screen = Rect::new(0, 0, 140, 30);
        app.view = View::Month;
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.focus, Focus::Side);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.focus, Focus::Calendar);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.focus, Focus::Day);
        // Tasks page: tasks page ⇄ side menu
        let mut app = app_with(ITEMS);
        app.screen = Rect::new(0, 0, 140, 30);
        press(&mut app, KeyCode::Char('4'));
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.focus, Focus::Side);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.focus, Focus::Tasks);
    }

    #[test]
    fn tab_stays_on_the_calendar_without_items() {
        let mut app = app_with("- No date\n");
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn jk_in_the_agenda_choose_items_and_the_calendar_follows() {
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("c")
        );
        press(&mut app, KeyCode::Down);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("d")
        );
        assert_eq!(app.selected, date(2026, 10, 20));
        // Down from the last item: stays
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("d")
        );
        // Up to the first item, then up again: stays
        for _ in 0..5 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("a")
        );
        assert_eq!(app.selected, date(2026, 10, 10));
        // h / l do nothing in the agenda
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 10));
    }

    #[test]
    fn esc_in_the_agenda_goes_back_to_the_calendar() {
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.focus, Focus::Calendar);
        assert!(!app.should_quit);
        // Already on the calendar: Esc quits
        press(&mut app, KeyCode::Esc);
        assert!(app.should_quit);
    }

    #[test]
    fn t_and_brackets_go_back_to_the_calendar() {
        for code in [KeyCode::Char('t'), KeyCode::Char('['), KeyCode::Char(']')] {
            let mut app = app_with(ITEMS);
            press(&mut app, KeyCode::Tab);
            press(&mut app, code);
            assert_eq!(app.focus, Focus::Calendar, "{code:?}");
        }
    }

    #[test]
    fn wheel_in_the_agenda_moves_an_item() {
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Tab);
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("c")
        );
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("a")
        );
    }

    #[test]
    fn clicking_an_item_moves_the_keyboard_there() {
        // 70 × 23, October at the top: the agenda's first row is 10/10 a (the inside of the agenda box starts
        // at x 36, y 3)
        let mut app = app_with(ITEMS);
        app.screen = Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 40, 3));
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("a")
        );
        assert_eq!(app.selected, date(2026, 10, 10));
        // Then click a date on the calendar: the keyboard goes back to the calendar
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 25, 5));
        assert_eq!(app.focus, Focus::Calendar);
        assert_eq!(app.selected, date(2026, 10, 16));
    }

    #[test]
    fn t_goes_back_to_today() {
        let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
        app.selected = date(2027, 3, 1);
        app.calendar_top = Some(30);
        press(&mut app, KeyCode::Char('t'));
        assert_eq!(app.selected, date(2026, 10, 17));
        assert_eq!(app.calendar_top, None);
    }

    #[test]
    fn groups_keep_file_order() {
        // Items before any heading are `None`; headings with the same name are one group; `##` is a group
        // too; a heading with no items under it yet is a calendar too (e.g. one just added)
        let mut app = app_with("");
        app.set_items(crate::tasks::parse(
            "- a\n# Work\n- b\n# No items\n## Personal\n- c\n# Work\n- d\n",
        ));
        let names: Vec<Option<&str>> = app
            .groups
            .iter()
            .map(|group| group.name.as_deref())
            .collect();
        assert_eq!(
            names,
            [None, Some("Work"), Some("No items"), Some("Personal")]
        );
        assert!(app.groups.iter().all(|group| group.shown));
        // d is under the second "# Work" and also counts as "Work"
        assert!(app.groups[1].includes(&app.items[3]));
        assert!(!app.groups[0].includes(&app.items[3]));
    }

    #[test]
    fn unchecked_groups_leave_the_agenda() {
        // "Personal" unchecked: its b and c are not listed, and Tab into the agenda skips them too
        let mut app = app_with(
            "- a @2026-10-10\n# Personal\n- b @2026-10-17\n- c @2026-10-20\n# Work\n- d @2026-10-25\n",
        );
        app.groups[1].shown = false;
        let titles: Vec<&str> = app
            .agenda()
            .iter()
            .map(|entry| entry.item.title.as_str())
            .collect();
        assert_eq!(titles, ["a", "d"]);
        assert!(!app.is_shown(&app.items[1]));
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("d")
        );
        // Checked again: listed again
        app.groups[1].shown = true;
        assert_eq!(app.agenda().len(), 4);
    }

    /// An app 120 cells wide (with a side menu) and `text` as the task file
    fn wide_app_with(text: &str) -> App {
        let mut app = app_with(text);
        app.screen = Rect::new(0, 0, 120, 30);
        app
    }

    /// Three groups: ungrouped (a), Personal (b), Work (c)
    const GROUPS: &str = "- a @2026-10-10\n# Personal\n- b @2026-10-17\n# Work\n- c @2026-10-20\n";

    #[test]
    fn tab_goes_through_the_side_menu_when_wide() {
        // Calendar → agenda → side menu → calendar
        let mut app = wide_app_with(GROUPS);
        for focus in [Focus::Agenda, Focus::Side, Focus::Calendar] {
            press(&mut app, KeyCode::Tab);
            assert_eq!(app.focus, focus);
        }
        // No items in the agenda: the calendar goes straight to the side menu
        let mut app = wide_app_with("- No date\n");
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Side);
        // Hidden: same as when narrow, calendar ⇄ agenda
        let mut app = wide_app_with(GROUPS);
        app.side_hidden = true;
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn side_menu_keys_choose_and_check_calendars() {
        let mut app = wide_app_with(GROUPS);
        app.focus = Focus::Side;
        // j to "Personal", Space unchecks it: b is no longer listed
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.group_index, 1);
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.groups[1].shown);
        assert_eq!(app.agenda().len(), 2);
        // Enter checks it again
        press(&mut app, KeyCode::Enter);
        assert!(app.groups[1].shown);
        // Down past the last one, up past the first one: stays; arrow keys and the wheel do the same
        for _ in 0..3 {
            press(&mut app, KeyCode::Down);
        }
        assert_eq!(app.group_index, 2);
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.group_index, 1);
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('k'));
        }
        assert_eq!(app.group_index, 0);
        // h / l do not move the selected day; Esc goes back to the calendar without quitting
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.selected, date(2026, 10, 17));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.focus, Focus::Calendar);
        assert!(!app.should_quit);
    }

    #[test]
    fn the_keyboard_leaves_the_side_menu_when_it_is_hidden_but_not_when_narrowed() {
        // Hide it with `s`
        let mut app = wide_app_with(GROUPS);
        app.focus = Focus::Side;
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.focus, Focus::Calendar);
        // The terminal gets narrow: the keyboard stays in the side menu, which now shows over the left side
        let mut app = wide_app_with(GROUPS);
        app.focus = Focus::Side;
        app.resize(Rect::new(0, 0, 120, 20));
        assert_eq!(app.focus, Focus::Side);
        app.resize(Rect::new(0, 0, 119, 20));
        assert_eq!(app.focus, Focus::Side);
        assert!(ui::side_overlay(&app).is_some());
    }

    #[test]
    fn clicking_a_calendar_selects_it_then_toggles_it() {
        // 120 × 30: inside the side menu, one calendar per row from y 3 (screen in ui.rs's
        // `side_menu_lists_the_calendars_when_wide`)
        let mut app = wide_app_with(GROUPS);
        let click = |app: &mut App, row| {
            app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 5, row));
        };
        // First click: chooses "Personal", the keyboard moves to the side menu, not toggled yet
        click(&mut app, 4);
        assert_eq!((app.focus, app.group_index), (Focus::Side, 1));
        assert!(app.groups[1].shown);
        // Click again: unchecks it; once more: checks it again
        click(&mut app, 4);
        assert!(!app.groups[1].shown);
        click(&mut app, 4);
        assert!(app.groups[1].shown);
        // Click another calendar: only chooses, does not toggle
        click(&mut app, 5);
        assert_eq!(app.group_index, 2);
        assert!(app.groups[2].shown);
        // When the keyboard is not in the side menu, clicking the chosen one also just chooses it
        // (back to the side menu)
        app.focus = Focus::Calendar;
        click(&mut app, 5);
        assert_eq!(app.focus, Focus::Side);
        assert!(app.groups[2].shown);
    }

    #[test]
    fn s_hides_the_side_menu_when_wide_and_calls_it_up_when_narrow() {
        let mut app = app_with(GROUPS);
        // Narrow: s calls it up (the keyboard moves to the side menu), and it works like when wide; s again
        // hides it, and so does Esc
        app.screen = Rect::new(0, 0, 119, 30);
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.focus, Focus::Side);
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char(' '));
        assert!(!app.groups[1].shown);
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.focus, Focus::Calendar);
        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.focus, Focus::Calendar);
        assert!(!app.side_hidden);
        // Wide enough: hides it, pressing again shows it
        app.screen = Rect::new(0, 0, 120, 30);
        press(&mut app, KeyCode::Char('s'));
        assert!(app.side_hidden);
        press(&mut app, KeyCode::Char('s'));
        assert!(!app.side_hidden);
    }

    #[test]
    fn putting_the_narrow_side_menu_away_goes_back_to_the_pane_before() {
        // 60 cells wide, keyboard in the agenda with c (3rd item) chosen: s calls up the side menu; hiding it
        // with s / Esc / a click outside / q always goes back to the agenda with c still chosen
        // (q only hides it, does not quit)
        let mut app = app_with(ITEMS);
        app.screen = Rect::new(0, 0, 60, 20);
        app.choose(2);
        let left = MouseEventKind::Down(MouseButton::Left);
        for close in 0..4 {
            press(&mut app, KeyCode::Char('s'));
            assert_eq!(app.focus, Focus::Side);
            match close {
                0 => press(&mut app, KeyCode::Char('s')),
                1 => press(&mut app, KeyCode::Esc),
                2 => app.handle_mouse(mouse(left, 50, 10)),
                _ => press(&mut app, KeyCode::Char('q')),
            }
            assert_eq!((app.focus, app.agenda_index), (Focus::Agenda, 2), "{close}");
            assert!(!app.should_quit);
        }
        // Layout 3, the 10/17 popup with the 2nd item (c) chosen: call it up and hide it; the popup comes back
        // with c still chosen
        press(&mut app, KeyCode::Char('3'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Char('s'));
        assert_eq!((app.focus, app.day_index), (Focus::Day, 1));
        // Uncheck the only calendar while it is up: the day has no items left, so hiding it goes back to
        // the calendar
        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn the_narrow_side_menu_takes_clicks_and_a_click_outside_puts_it_away() {
        // 60 × 20: s calls it up over the left side (after the border, header, and line, one calendar per row:
        // ungrouped y 3, Personal y 4)
        let mut app = app_with(GROUPS);
        app.screen = Rect::new(0, 0, 60, 20);
        press(&mut app, KeyCode::Char('s'));
        let area = ui::side_overlay(&app).expect("overlay");
        assert_eq!(area, Rect::new(0, 0, 22, 19));
        // Click Personal: chooses it; click again: unchecks it
        let left = MouseEventKind::Down(MouseButton::Left);
        app.handle_mouse(mouse(left, 3, 4));
        assert_eq!((app.focus, app.group_index), (Focus::Side, 1));
        app.handle_mouse(mouse(left, 3, 4));
        assert!(!app.groups[1].shown);
        // Click outside the box (10/16 on the calendar; with October at the top, 10/17 is at x 27-29, y 5):
        // only hides it, the selected day stays
        app.calendar_top = Some(0);
        let selected = app.selected;
        assert_eq!(
            ui::click_at(&app, 25, 5),
            Some(Click::Day(date(2026, 10, 16)))
        );
        app.handle_mouse(mouse(left, 25, 5));
        assert_eq!((app.focus, app.selected), (Focus::Calendar, selected));
        assert_eq!(ui::side_overlay(&app), None);
    }

    #[test]
    fn widening_keeps_the_side_menu_unless_it_was_hidden() {
        // Called up when narrow, then the terminal is widened: side menu at the far left, keyboard still on it
        let mut app = app_with(GROUPS);
        app.resize(Rect::new(0, 0, 60, 20));
        press(&mut app, KeyCode::Char('s'));
        app.resize(Rect::new(0, 0, 120, 20));
        assert_eq!(app.focus, Focus::Side);
        // Hidden earlier while wide (side_hidden): after widening it is not shown, and the keyboard goes back
        // to the calendar
        let mut app = app_with(GROUPS);
        app.side_hidden = true;
        app.resize(Rect::new(0, 0, 60, 20));
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.focus, Focus::Side);
        app.resize(Rect::new(0, 0, 120, 20));
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn a_multi_day_item_is_listed_in_each_month_and_chosen_from_any_of_its_days() {
        // Manager OOO 10/28-11/3: one row in October and one in November; the November row is listed on
        // 11/1, before 11/1's "Write the monthly report"
        let mut app = app_with(
            "- [ ] Write the monthly report @2026-11-01\n- Manager OOO @2026-10-28..2026-11-03\n- [ ] Send the weekly update @2026-10-30\n",
        );
        let rows: Vec<(&str, Date)> = app
            .agenda()
            .iter()
            .map(|entry| (entry.item.title.as_str(), entry.date))
            .collect();
        assert_eq!(
            rows,
            [
                ("Manager OOO", date(2026, 10, 28)),
                ("Send the weekly update", date(2026, 10, 30)),
                ("Manager OOO", date(2026, 11, 1)),
                ("Write the monthly report", date(2026, 11, 1)),
            ]
        );
        // Select 11/2 (inside the OOO) and press Tab: chooses the November row; the selected day stays
        app.selected = date(2026, 11, 2);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.agenda_index, 2);
        assert_eq!(app.selected, date(2026, 11, 2));
        // j: "Write the monthly report", the calendar jumps to 11/1; k: back to the OOO; 11/1 is also
        // one of its days, so the calendar stays
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, date(2026, 11, 1));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!((app.agenda_index, app.selected), (2, date(2026, 11, 1)));
        // Up again: "Send the weekly update" (10/30) → the OOO's October row (10/30 is also one of its
        // days, so the calendar stays)
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!((app.agenda_index, app.selected), (0, date(2026, 10, 30)));
        // Tab on 10/29 (inside the OOO, in October): chooses the October row, ahead of 10/30's
        // "Send the weekly update"
        let mut app = app_with(
            "- [ ] Send the weekly update @2026-10-30\n- Manager OOO @2026-10-28..2026-11-03\n",
        );
        app.selected = date(2026, 10, 29);
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            app.chosen().map(|entry| entry.date),
            Some(date(2026, 10, 28))
        );
        assert_eq!(app.selected, date(2026, 10, 29));
    }

    #[test]
    fn enter_on_the_calendar_goes_into_that_days_items() {
        // 10/17 has b and c: Enter → the keyboard moves to the agenda and chooses b
        let mut app = app_with(ITEMS);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.focus, Focus::Agenda);
        assert_eq!(
            app.chosen().map(|entry| entry.item.title.as_str()),
            Some("b")
        );
        // A day with no items (10/18): nothing happens
        let mut app = app_with(ITEMS);
        app.selected = date(2026, 10, 18);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.focus, Focus::Calendar);
        // Layout 3: the layout stays; the day popup opens with the first item (b) chosen
        let mut app = app_with(ITEMS);
        app.view = View::Month;
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            (app.view, app.focus, app.day_index),
            (View::Month, Focus::Day, 0)
        );
        // With the mouse it takes a double click (70 × 23, October at the top: 10/17 is at x 27-29, y 5):
        // one click on the selected day only selects it, a second one right away goes in
        let mut app = app_with(ITEMS);
        app.screen = Rect::new(0, 0, 70, 23);
        app.calendar_top = Some(0);
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 28, 5));
        assert_eq!(app.focus, Focus::Calendar);
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 28, 5));
        assert_eq!(app.focus, Focus::Agenda);
    }

    #[test]
    fn only_a_double_click_goes_into_the_day_in_layout_1() {
        let mut app = app_with(ITEMS);
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let (oct16, oct17) = (date(2026, 10, 16), date(2026, 10, 17));
        // The selected day clicked again 0.9 s later: only selected, the keyboard stays on the calendar
        app.click_calendar_day(oct17, at(0));
        app.click_calendar_day(oct17, at(900));
        assert_eq!(app.focus, Focus::Calendar);
        // Another click 0.3 s after that one: a double click, the agenda's first item that day (b)
        app.click_calendar_day(oct17, at(1200));
        assert_eq!(
            (
                app.focus,
                app.chosen().map(|entry| entry.item.title.as_str())
            ),
            (Focus::Agenda, Some("b"))
        );
        // Two quick clicks on different days: the second day is selected, the keyboard back on the calendar
        app.click_calendar_day(oct16, at(2000));
        app.click_calendar_day(oct17, at(2200));
        assert_eq!((app.selected, app.focus), (oct17, Focus::Calendar));
    }

    #[test]
    fn number_keys_switch_layouts() {
        // 3 switches to the month grid, 1 switches back; switching scrolls the selected month to the top
        let mut app = app_with(ITEMS);
        app.calendar_top = Some(5);
        app.month_top = Some(date(2026, 1, 5));
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.view, View::Month);
        assert_eq!((app.calendar_top, app.month_top), (None, None));
        press(&mut app, KeyCode::Char('1'));
        assert_eq!(app.view, View::Planner);
        // Switching to the month grid (no agenda) with the keyboard in the agenda: back to the calendar;
        // the side menu is not affected
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Agenda);
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.focus, Focus::Calendar);
        let mut app = wide_app_with(GROUPS);
        app.focus = Focus::Side;
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.focus, Focus::Side);
    }

    #[test]
    fn tab_skips_the_agenda_in_the_month_view() {
        // Month grid: calendar → side menu → calendar; when narrow (no side menu), Tab does nothing
        let mut app = wide_app_with(GROUPS);
        app.view = View::Month;
        for focus in [Focus::Side, Focus::Calendar] {
            press(&mut app, KeyCode::Tab);
            assert_eq!(app.focus, focus);
        }
        let mut app = app_with(ITEMS);
        app.view = View::Month;
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.focus, Focus::Calendar);
    }

    #[test]
    fn the_mouse_works_in_the_month_view() {
        // 80 × 30, 9/21 at the top (for cell positions see ui.rs's
        // `month_view_click_positions_map_to_the_cells`)
        let mut app = app_with(ITEMS);
        app.view = View::Month;
        app.screen = Rect::new(0, 0, 80, 30);
        app.month_top = Some(date(2026, 9, 21));
        // Click the Tuesday cell of week 2 (not necessarily on the date): selects that day
        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 20, 9));
        assert_eq!(app.selected, date(2026, 9, 29));
        // The wheel moves a week
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 20, 9));
        assert_eq!(app.selected, date(2026, 10, 6));
    }

    #[test]
    fn t_and_brackets_put_the_month_on_top_in_the_month_view_too() {
        for code in [KeyCode::Char('t'), KeyCode::Char('['), KeyCode::Char(']')] {
            let mut app = app_with(ITEMS);
            app.view = View::Month;
            app.month_top = Some(date(2026, 1, 5));
            press(&mut app, code);
            assert_eq!(app.month_top, None, "{code:?}");
        }
    }

    #[test]
    fn quit_keys() {
        let keys = [
            KeyEvent::from(KeyCode::Char('q')),
            KeyEvent::from(KeyCode::Esc),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ];
        for key in keys {
            let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
            app.handle_key(key);
            assert!(app.should_quit, "{key:?} should quit");
        }
    }

    #[test]
    fn other_keys_do_not_quit() {
        // Plain c (no Ctrl), capital Q, and Enter do not quit
        let keys = [
            KeyEvent::from(KeyCode::Char('c')),
            KeyEvent::from(KeyCode::Char('Q')),
            KeyEvent::from(KeyCode::Enter),
        ];
        for key in keys {
            let mut app = App::new(Lang::En, date(2026, 10, 17), Vec::new());
            app.handle_key(key);
            assert!(!app.should_quit, "{key:?} should not quit");
        }
    }
}
