//! App state and the main loop: draw the screen → wait for input → update the state, until quit

mod day;
mod editor;
mod help;
mod moving;
mod picker;
mod settings;
mod tasks;
mod write;

pub use day::{TextDrag, TextSpot};
pub use editor::{Editor, Target};
pub use help::Help;
pub use moving::Moving;
pub use picker::Picker;
pub use settings::{Place, Row, SettingsPage, rows};
pub use tasks::TaskRow;
pub use write::Notice;

use crate::{
    calendar, clipboard,
    config::{self, CalendarListSettings, Settings},
    tasks::{Heading, HeadingLine, Item, Kind, Warning},
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
use std::time::{Duration, Instant, SystemTime};

/// Two clicks on the same day or item at most this far apart count as a double click
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// With no input, the main loop goes round this often anyway, to check whether the task file changed elsewhere
const REFRESH_EVERY: Duration = Duration::from_secs(1);

/// How long the intro plays at start (`Mode::Splash`) before the calendar shows
const SPLASH_FOR: Duration = Duration::from_millis(5_500);

/// How often the intro redraws while it plays (about 30 frames a second)
const FRAME_EVERY: Duration = Duration::from_millis(33);

/// How long a note stays at the bottom right (`App::toast`)
const TOAST_FOR: Duration = Duration::from_secs(2);

/// A short note at the bottom right, over the end of the bottom row; it goes by itself after `TOAST_FOR`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toast {
    /// Text dragged over in the day popup is on the clipboard: how many characters
    Copied(usize),
    /// Putting it on the clipboard failed (e.g. no clipboard command on this computer)
    CopyFailed,
}

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
    /// The tasks page (`3`): `j` / `k` choose items one by one, `l` / `h` expand / collapse subtasks
    Tasks,
    /// Layout 2's day popup (`Enter` on the calendar): `j` / `k` choose the selected day's items one by one
    Day,
}

/// What a click landed on, to tell a double click (two clicks on the same one within `DOUBLE_CLICK`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClickTarget {
    /// A day on the calendar (both layouts)
    Day(Date),
    /// An item in the agenda, the day popup or on the tasks page: its line in the task file
    Item(usize),
    /// The side menu's right border
    SideBorder,
}

/// The side menu's right border is held down with the mouse (see `App::drag_side_border`)
#[derive(Debug)]
struct SideDrag {
    /// The settings when it was pressed: letting go saves only what changed since (the width)
    before: Settings,
    /// Where it was pressed (terminal column)
    column: u16,
    /// The side menu's width then
    width: u16,
}

/// Which layout is shown (number keys switch)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Layout 1 (`1`): calendar on the left; on the right, each month's items line up with that month
    Planner,
    /// Layout 2 (`2`): the big month grid, 6 weeks at a time; each cell lists that day's items
    Month,
    /// Tasks page (`3`): the whole task file listed by calendar, subtasks as a tree, undated items included
    Tasks,
}

/// A row of the side menu the keyboard can be on, top to bottom: the two switches above the calendars, then
/// the calendars
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideRow {
    /// 「隱藏全部」: unchecks every calendar, or checks them all again
    HideAll,
    /// 「隱藏做完的」: `[calendar_list] hide_done`
    HideDone,
    /// A calendar: its index in `App::groups`
    Calendar(usize),
}

/// 「Only this one」 (`Enter` in the side menu): which calendar is shown alone, and every calendar's check from
/// before, to put back
#[derive(Debug)]
struct Solo {
    /// The path of the calendar shown alone (`Group::path`)
    path: Vec<String>,
    /// Every calendar's path and whether it was checked, before the first `Enter`
    checked: Vec<(Vec<String>, bool)>,
}

/// One calendar in the side menu (= one group in the task file)
#[derive(Debug)]
pub struct Group {
    /// Group name; `None` = items at the start of the file, before any heading
    pub name: Option<String>,
    /// The calendars it is inside, outermost first (see `Heading::parents`); empty at the top level
    pub parents: Vec<String>,
    /// Whether it is checked (all are checked at start, except archived ones); unchecking it hides the
    /// calendars inside it too
    pub shown: bool,
    /// Archived: one of its headings, or one of the calendars it is inside, has `archived:<date>`
    /// (`Heading::archived`)
    pub archived: bool,
}

impl Group {
    /// All groups in the task file: the groups of `items` and every heading in `headings` (so a heading
    /// with no items under it yet is a calendar too); headings with the same path count as one.
    /// In tree order: each calendar right after the one it is inside, calendars at the same level in
    /// order of first appearance; archived calendars come after all the others (in tree order too) and
    /// start unchecked
    fn list(items: &[Item], headings: &[HeadingLine]) -> Vec<Self> {
        let mut found: Vec<(usize, Option<&Heading>)> = items
            .iter()
            .map(|item| (item.line, item.group.as_ref()))
            .chain(
                headings
                    .iter()
                    .map(|heading| (heading.line, Some(&heading.heading))),
            )
            .collect();
        found.sort_by_key(|&(line, _)| line);
        let mut groups: Vec<Self> = Vec::new();
        for (_, heading) in found {
            let group = Self {
                name: heading.map(|heading| heading.name.clone()),
                parents: heading
                    .map(|heading| heading.parents.clone())
                    .unwrap_or_default(),
                shown: true,
                archived: heading.is_some_and(|heading| heading.archived.is_some()),
            };
            // A calendar with several headings is archived when any of them says so
            match groups.iter_mut().find(|other| other.same(&group)) {
                Some(other) => other.archived |= group.archived,
                None => groups.push(group),
            }
        }
        // Tree order: sort by where each calendar on its path first appeared, outermost first (a parent's
        // key is the start of its children's keys, so it comes right before them)
        let keys: Vec<Vec<usize>> = groups
            .iter()
            .map(|group| {
                (1..=group.parents.len())
                    .filter_map(|depth| {
                        groups.iter().position(|other| {
                            other.parents == group.parents[..depth - 1]
                                && other.name.as_deref() == Some(group.parents[depth - 1].as_str())
                        })
                    })
                    .chain(groups.iter().position(|other| other.same(group)))
                    .collect()
            })
            .collect();
        let mut order: Vec<usize> = (0..groups.len()).collect();
        order.sort_by(|&a, &b| keys[a].cmp(&keys[b]));
        let mut groups: Vec<Option<Self>> = groups.into_iter().map(Some).collect();
        let mut tree: Vec<Self> = order
            .into_iter()
            .filter_map(|index| groups[index].take())
            .collect();
        // A calendar inside an archived one is archived too
        let archived: Vec<Vec<String>> = tree
            .iter()
            .filter(|group| group.archived)
            .map(Self::path)
            .collect();
        for group in &mut tree {
            group.archived |= archived.iter().any(|outer| group.inside_path(outer));
            group.shown = !group.archived;
        }
        // The running calendars first, then the archived ones; each part keeps tree order
        let (mut running, archived): (Vec<Self>, Vec<Self>) =
            tree.into_iter().partition(|group| !group.archived);
        running.extend(archived);
        running
    }

    /// Its path: the calendars it is inside, then its own name (empty for 「Ungrouped」)
    pub fn path(&self) -> Vec<String> {
        self.parents
            .iter()
            .cloned()
            .chain(self.name.clone())
            .collect()
    }

    /// Whether it is inside `other` (any number of levels down)
    pub fn inside(&self, other: &Self) -> bool {
        self.inside_path(&other.path())
    }

    /// Whether it is inside the calendar with the path `outer` (any number of levels down)
    fn inside_path(&self, outer: &[String]) -> bool {
        let path = self.path();
        !outer.is_empty() && path.len() > outer.len() && path[..outer.len()] == *outer
    }

    /// Whether it is the same calendar as `other` (same path)
    pub fn same(&self, other: &Self) -> bool {
        self.name == other.name && self.parents == other.parents
    }

    /// Whether `heading` is one of this calendar's headings
    pub fn is(&self, heading: &Heading) -> bool {
        self.name.as_deref() == Some(heading.name.as_str()) && self.parents == heading.parents
    }

    /// Whether `item` belongs to this group (not to a calendar inside it)
    pub fn includes(&self, item: &Item) -> bool {
        match &item.group {
            Some(heading) => self.is(heading),
            None => self.name.is_none(),
        }
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
        /// The color picker (`e` on a color row), open over the settings page
        picker: Option<Picker>,
    },
    /// After `d`, asking whether to delete: the item to delete and how many items that removes (with subtasks)
    ConfirmDelete(Item, usize),
    /// After `d` in the calendar list, asking whether to delete the calendar: its name, its heading lines
    /// (as read when asking) and how many items that removes
    ConfirmDeleteCalendar {
        name: String,
        headings: Vec<HeadingLine>,
        count: usize,
    },
    /// Editor box (`e`, `a`, `A`)
    Editor(Editor),
    /// Rescheduling (`r`): which item to move
    Moving(Moving),
    /// The intro played at start (`ui::splash`), since `opened`: it closes after `SPLASH_FOR`, and any key, a
    /// click or the wheel skips it (that key or click does nothing else)
    Splash { opened: Instant },
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
    /// Which row is chosen in the side menu (read it with `chosen_side_row`); used only when the keyboard is
    /// there
    pub side_row: SideRow,
    /// Side menu scroll: which calendar is at the top (index in `groups`); it scrolls only when the calendars
    /// do not fit
    pub side_top: usize,
    /// The side menu's archive part scrolls on its own: which archived calendar is at its top (0 = the first
    /// archived one)
    pub side_archive_top: usize,
    /// Which pane had the keyboard before `s` called up the side menu when narrow (it goes back there on hide)
    side_back: Focus,
    /// A calendar shown alone with `Enter` in the side menu, and the checks to put back; `None` = not now
    solo: Option<Solo>,
    /// The last key was `g` in the side menu or on the tasks page: one more `g` right after it jumps to the
    /// first calendar / item (`gg`)
    pending_g: bool,
    /// Which pane has the keyboard
    pub focus: Focus,
    /// Which layout is shown
    pub view: View,
    /// Which item is chosen in the agenda (index in `agenda()`); used only when the keyboard is there
    pub agenda_index: usize,
    /// Which item is chosen in layout 2's popup (index in `day_items()`);
    /// used only when the keyboard is in the popup
    pub day_index: usize,
    /// Popup scroll: which line is at the top (a long title wraps onto more than one line; it scrolls only
    /// when the chosen item leaves the box)
    pub day_top: usize,
    /// Text being dragged over with the mouse in the popup (`App::day_selection`); `None` = none. A key, the
    /// next click, or reading the task file again clears it
    pub day_drag: Option<TextDrag>,
    /// The side menu's right border being dragged with the mouse; `None` = not dragging
    side_drag: Option<SideDrag>,
    /// What the last click on a day or an item hit and when, to tell a double click
    last_click: Option<(ClickTarget, Instant)>,
    /// Calendar scroll: which row is at the top of the screen (`None` = on the next draw, put the selected
    /// day's month at the top)
    pub calendar_top: Option<i64>,
    /// Agenda scroll: (the 1st of which month, which sorted row of that month is at the top); it scrolls only
    /// when a month has too many items to fit
    pub agenda_window: Option<(Date, usize)>,
    /// Layout 2 scroll: the Monday of the top week (`None` = on the next draw, put the week with the 1st of
    /// the selected month at the top)
    pub month_top: Option<Date>,
    /// Layout 2 was scrolled with the wheel while this day was selected: the 6 weeks stay where the wheel
    /// left them (the selected day may be off screen) until the selection changes
    pub month_free: Option<Date>,
    /// Screen colors
    pub theme: Theme,
    /// Settings from the config file (theme, colors, grid lines, relative week numbers…);
    /// the settings page changes them
    pub settings: Settings,
    /// Where the config file is (saved back when the settings page closes); `None` = do not save (tests, or
    /// no place for a config file)
    pub config_path: Option<PathBuf>,
    /// Where the marker that the intro has played is (`paths::splash_seen_file`): resetting all settings
    /// deletes it, so the intro plays again on the next start; `None` = no marker (tests, or no HOME)
    pub splash_marker: Option<PathBuf>,
    /// Terminal size (updated before each draw); needed to map mouse click positions
    pub screen: Rect,
    /// Where the task file is (checking and editing write back to it); `None` = no task file (tests), no writes
    pub tasks_path: Option<PathBuf>,
    /// The task file's content that the screen shows (as read at start, or as roh-cal last wrote or reloaded
    /// it); a file that differs from it was changed elsewhere
    tasks_text: String,
    /// The task file's modified time when last checked (`None` = not checked yet, or no file); a different
    /// time means the file may have changed, so it is read again
    tasks_modified: Option<SystemTime>,
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
    /// Calendars collapsed on the tasks page (their paths, e.g. `["Work", "SFAP"]`): the items and calendars
    /// inside them are not listed. All open at start
    pub collapsed: BTreeSet<Vec<String>>,
    /// Tasks page scroll: which line is at the top of the screen (counting every row's lines: a long title
    /// wraps onto more than one line)
    pub tasks_top: usize,
    /// `true` = leave the main loop after this round
    should_quit: bool,
    /// `true` = open the task file in the user's editor after this key (`o` on the tasks page); the main loop
    /// does it, since only it has the terminal
    open_editor: bool,
    /// Text to put on the clipboard after this event (letting go of a drag in the day popup); the main loop
    /// does it (`App::copy_out`)
    to_copy: Option<String>,
    /// The note at the bottom right and when it went up (`Toast`); `None` = none
    pub toast: Option<(Toast, Instant)>,
}

impl App {
    /// State at start: keyboard on the calendar, today selected, the default theme (nord-bright)
    pub fn new(lang: Lang, today: Date, items: Vec<Item>) -> Self {
        Self {
            lang,
            today,
            selected: today,
            groups: Group::list(&items, &[]),
            side_hidden: false,
            side_row: SideRow::Calendar(0),
            side_top: 0,
            side_archive_top: 0,
            side_back: Focus::Calendar,
            solo: None,
            pending_g: false,
            items,
            warnings: Vec::new(),
            headings: Vec::new(),
            focus: Focus::Calendar,
            view: View::Planner,
            agenda_index: 0,
            day_index: 0,
            day_top: 0,
            day_drag: None,
            side_drag: None,
            last_click: None,
            calendar_top: None,
            agenda_window: None,
            month_top: None,
            month_free: None,
            theme: Theme::default(),
            settings: Settings::default(),
            config_path: None,
            splash_marker: None,
            screen: Rect::default(),
            tasks_path: None,
            tasks_text: String::new(),
            tasks_modified: None,
            notice: None,
            undo: Vec::new(),
            mode: Mode::Normal,
            tasks_line: None,
            expanded: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            tasks_top: 0,
            should_quit: false,
            open_editor: false,
            to_copy: None,
            toast: None,
        }
    }

    /// The same app with built-in theme `name`, for tests that check one theme's palette names and colors
    #[cfg(test)]
    pub fn with_theme(mut self, name: &str) -> Self {
        self.settings.set_theme(name);
        self.apply_theme();
        self
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

    /// If asking whether to delete a calendar: its name and how many items that removes
    pub fn confirm_delete_calendar(&self) -> Option<(&str, usize)> {
        match &self.mode {
            Mode::ConfirmDeleteCalendar { name, count, .. } => Some((name, *count)),
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

    /// The heading lines of the calendar chosen in the side menu (every heading with its path, as read last);
    /// empty for 「Ungrouped」
    fn chosen_calendar_headings(&self) -> Vec<HeadingLine> {
        let Some(group) = self.chosen_group() else {
            return Vec::new();
        };
        self.headings
            .iter()
            .filter(|line| group.is(&line.heading))
            .cloned()
            .collect()
    }

    /// Every calendar's path, checked state and whether it is archived (taken before a write that changes a
    /// calendar's path)
    fn checked_states(&self) -> Vec<(Vec<String>, bool, bool)> {
        self.groups
            .iter()
            .map(|group| (group.path(), group.shown, group.archived))
            .collect()
    }

    /// After a write that renamed or moved a calendar (its path went from `old` to `new`): it and the
    /// calendars inside it get back the checked state they had (`checked` = from `checked_states` before
    /// the write), except the ones the write archived or brought back: they start over like with `z`
    /// (unchecked / checked, as `App::set_items` left them); it is chosen in the side menu
    fn carry_checked(
        &mut self,
        checked: &[(Vec<String>, bool, bool)],
        old: &[String],
        new: &[String],
    ) {
        for group in &mut self.groups {
            let path = group.path();
            if path.len() >= new.len() && path[..new.len()] == *new {
                let before: Vec<String> = old.iter().chain(&path[new.len()..]).cloned().collect();
                if let Some((_, shown, _)) = checked
                    .iter()
                    .find(|(path, _, archived)| *path == before && *archived == group.archived)
                {
                    group.shown = *shown;
                }
            }
        }
        if let Some(index) = self.groups.iter().position(|group| group.path() == new) {
            self.side_row = SideRow::Calendar(index);
        }
    }

    /// Whether this item is shown on the calendar, the agenda and the day popup (the tasks page uses
    /// `App::listed`): only if its own calendar (group) is checked (the calendars around it do not matter),
    /// and it is not a done task hidden by `[calendar_list] hide_done`
    pub fn is_shown(&self, item: &Item) -> bool {
        self.in_checked_calendar(item) && !self.hidden_as_done(item)
    }

    /// Whether `item`'s own calendar is checked in the side menu (not the ones it is inside: checking or
    /// unchecking one of those already does the same to it, `toggle_calendar`)
    fn in_checked_calendar(&self, item: &Item) -> bool {
        self.groups
            .iter()
            .filter(|group| group.includes(item))
            .all(|group| group.shown)
    }

    /// How many tasks on `day` are not done yet (in progress included) and how many are done: tasks in
    /// checked calendars that cover `day`. Done ones count even when 「Hide done」 hides them, like the
    /// numbers in the calendar list; events do not count
    pub fn day_counts(&self, day: Date) -> (usize, usize) {
        self.items
            .iter()
            .filter(|item| {
                item.kind == Kind::Task && item.covers(day) && self.in_checked_calendar(item)
            })
            .fold((0, 0), |(open, done), item| {
                if item.done {
                    (open, done + 1)
                } else {
                    (open + 1, done)
                }
            })
    }

    /// Hiding done tasks (`[calendar_list] hide_done`) and `item` is a done task: only the done task itself,
    /// what is under it stays (an open subtask is still to do). The tasks page has its own rule
    /// (`App::listed`)
    fn hidden_as_done(&self, item: &Item) -> bool {
        self.settings.calendar_list.hide_done && item.kind == Kind::Task && item.done
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
    /// (on the same day, in-progress `[/]` items come first, then items without a time; same date and time
    /// keep file order)
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
        // sort_by_key is a stable sort: equal keys keep their order. On the same day, in-progress items come
        // first (`false` sorts before `true`), then items carried over from the previous month, then ones
        // without a time (`None` sorts before `Some`)
        entries.sort_by_key(|entry| {
            let continued = Some(entry.date) != entry.item.date;
            (
                entry.date,
                !entry.item.in_progress,
                !continued,
                entry.item.time,
            )
        });
        entries
    }

    /// Items on `day` (calendar checked; a multi-day item counts if it covers that day): in-progress `[/]`
    /// items first, then earlier starts, items starting the same day by time
    /// (no time first; same time keeps file order). Layout 2's cells and popup, and `a` on the calendar (to
    /// find that day's group) use this
    pub fn items_on(&self, day: Date) -> Vec<&Item> {
        let mut items: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| item.covers(day) && self.is_shown(item))
            .collect();
        items.sort_by_key(|item| (!item.in_progress, item.date, item.time));
        items
    }

    /// The chosen row when the keyboard is in the agenda; `None` when it is on the calendar
    pub fn chosen(&self) -> Option<Entry<'_>> {
        match self.focus {
            Focus::Agenda => self.agenda().get(self.agenda_index).copied(),
            Focus::Calendar | Focus::Side | Focus::Tasks | Focus::Day => None,
        }
    }

    /// The item the keyboard has chosen: in the agenda, on the tasks page, or in layout 2's popup; `None` on
    /// the calendar or side menu (checking, editing, deleting, and rescheduling all act on this item)
    fn chosen_item(&self) -> Option<&Item> {
        match self.focus {
            Focus::Agenda => self.chosen().map(|entry| entry.item),
            Focus::Tasks => self.tasks_chosen(),
            Focus::Day => self.day_items().get(self.day_index).copied(),
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
    /// before (for the agenda and layout 2's popup, the choice stays;
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
            (Focus::Day, View::Planner | View::Month) => {
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

    /// Main loop: each round checks the task file, draws the screen once and waits for one input (at most
    /// `REFRESH_EVERY`), until quit
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            // Changes made to the task file elsewhere (e.g. saved in Obsidian) show up in every view
            self.refresh_tasks();
            // Record the terminal size first (it changes when the window is resized),
            // so drawing and mouse positions agree
            let size = terminal.size()?;
            self.resize(Rect::new(0, 0, size.width, size.height));
            self.end_finished_splash(Instant::now());
            self.end_finished_toast(Instant::now());
            terminal.draw(|frame| ui::render(frame, &mut self))?;
            // No input for a while: go round again to check the task file (while the intro plays, soon: it
            // moves; with a note at the bottom right, when it is due to go)
            let wait = match self.mode {
                Mode::Splash { .. } => FRAME_EVERY,
                _ => REFRESH_EVERY,
            };
            let wait = self.toast.map_or(wait, |(_, shown)| {
                wait.min(TOAST_FOR.saturating_sub(shown.elapsed()))
            });
            if !event::poll(wait)? {
                continue;
            }
            // Only key presses are handled (some terminals also send key releases)
            let event = event::read()?;
            if let Some(key) = event.as_key_press_event() {
                self.handle_key(key);
            } else if let Some(mouse) = event.as_mouse_event() {
                self.handle_mouse(mouse);
            }
            // Let go of a drag in the day popup: its text goes to the clipboard
            self.copy_out(clipboard::copy, Instant::now());
            // `o` on the tasks page: the editor has the terminal until it quits
            if std::mem::take(&mut self.open_editor) {
                self.open_in_editor(terminal)?;
            }
        }
        Ok(())
    }

    /// Keys: the open popup or action in progress (`mode`) gets them first; otherwise see `handle_normal_key`
    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        // The intro: any key skips it and does nothing else (a notice from the start, e.g. mistakes in the
        // task file, is still there after it)
        if let Mode::Splash { .. } = self.mode {
            self.mode = Mode::Normal;
            return;
        }
        // The last notice has been seen: any key clears it; so is text dragged over in the popup
        self.notice = None;
        self.day_drag = None;
        // A `g` waits for the very next key only (`gg`)
        let after_g = std::mem::take(&mut self.pending_g);
        match self.mode {
            // Help page: only scrolling, switching version, closing
            Mode::Help(_) => self.handle_help_key(key),
            // Settings page: only choosing a row, changing a value, resetting, closing
            Mode::Settings { .. } => self.handle_settings_key(key),
            // Asking whether to delete: `d` (`dd`) or `y` deletes, any other key just cancels
            Mode::ConfirmDelete(..) => self.answer_delete(key),
            Mode::ConfirmDeleteCalendar { .. } => self.answer_delete_calendar(key),
            // Editor box: keys are typing (Enter saves, Esc cancels)
            Mode::Editor(_) => self.handle_editor_key(key),
            // Rescheduling: only moving the date, confirming, cancelling
            Mode::Moving(_) => self.handle_moving_key(key),
            // The intro: skipped above
            Mode::Splash { .. } => {}
            Mode::Normal => self.handle_normal_key(key, after_g),
        }
    }

    /// Plays the intro (`Mode::Splash`) from now
    pub fn play_splash(&mut self) {
        self.mode = Mode::Splash {
            opened: Instant::now(),
        };
    }

    /// Takes the note at the bottom right down once it has been up for `TOAST_FOR` by `now`
    fn end_finished_toast(&mut self, now: Instant) {
        if self
            .toast
            .is_some_and(|(_, shown)| now.saturating_duration_since(shown) >= TOAST_FOR)
        {
            self.toast = None;
        }
    }

    /// Puts the text waiting in `to_copy` on the clipboard with `copy`, and says at the bottom right (from
    /// `now`) how many characters it was, or that it failed
    pub(crate) fn copy_out(&mut self, copy: impl FnOnce(&str) -> Result<()>, now: Instant) {
        let Some(text) = self.to_copy.take() else {
            return;
        };
        let toast = match copy(&text) {
            Ok(()) => Toast::Copied(text.chars().count()),
            Err(_) => Toast::CopyFailed,
        };
        self.toast = Some((toast, now));
    }

    /// Closes the intro once it has played for `SPLASH_FOR` by `now`
    fn end_finished_splash(&mut self, now: Instant) {
        if let Mode::Splash { opened } = self.mode
            && now.saturating_duration_since(opened) >= SPLASH_FOR
        {
            self.mode = Mode::Normal;
        }
    }

    /// Keys when no popup is open: `Tab` switches pane, `[` / `]` switch month, `t` back to today, `u` undo,
    /// `1` / `2` / `3` switch layout, `s` hides / shows the side menu, `?` help page, `,` settings page,
    /// `q` / `Ctrl+C` quit. `Esc` goes back to this layout's main pane;
    /// when already there: from the tasks page back to layout 1, from the calendar it quits. While layout 2's
    /// popup or the side menu called up when narrow is open, `Esc` / `q` only close it.
    /// Other keys depend on which pane has the keyboard (`after_g`: the key before this one was a `g` in the
    /// side menu or on the tasks page)
    fn handle_normal_key(&mut self, key: KeyEvent, after_g: bool) {
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
            // Back to today, scrolled like `go_today` says (the tasks page has no calendar, so ignored)
            KeyCode::Char('t') if self.view != View::Tasks => {
                self.go_today();
                self.focus = Focus::Calendar;
            }
            // Undo the last write (check, edit, …)
            KeyCode::Char('u') => self.undo(),
            // Hide / show done tasks everywhere (the calendar list's 「隱藏做完的」)
            KeyCode::Char('.') => self.toggle_hide_done(),
            KeyCode::Char('?') => self.mode = Mode::Help(Help::default()),
            KeyCode::Char(',') => self.open_settings(),
            KeyCode::Char('1') => self.set_view(View::Planner),
            KeyCode::Char('2') => self.set_view(View::Month),
            KeyCode::Char('3') => self.set_view(View::Tasks),
            // 1st of the previous / next month, scrolled to the top (ignored on the tasks page: no calendar;
            // in the side menu they choose a top-level calendar instead)
            KeyCode::Char('[') if self.view != View::Tasks && self.focus != Focus::Side => {
                self.jump_month(-1)
            }
            KeyCode::Char(']') if self.view != View::Tasks && self.focus != Focus::Side => {
                self.jump_month(1)
            }
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
                Focus::Side => self.handle_side_key(key.code, after_g),
                Focus::Tasks => self.handle_tasks_key(key, after_g),
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

    /// `Enter` on the calendar (or double-clicking a day), in both layouts: opens the day popup with that
    /// day's items (`Tab` still goes into layout 1's agenda); nothing happens if the day has no items
    fn open_day(&mut self) {
        self.open_day_popup();
    }

    /// Keyboard in the agenda: up / down one item, and the calendar's selected day follows the item; the
    /// rest are the keys on the chosen item (`handle_item_key`)
    fn handle_agenda_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_next(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_next(-1),
            _ => self.handle_item_key(code),
        }
    }

    /// Keys on the chosen item, the same in the agenda, the day popup and on the tasks page (all write back
    /// to the task file): `Space` / `x` check, `/` in progress, `r` reschedule, `e` / `Enter` edit, `a`
    /// add, `A` add a subtask, `d` delete; other keys do nothing
    fn handle_item_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(' ' | 'x') => self.toggle_chosen(),
            KeyCode::Char('/') => self.toggle_chosen_progress(),
            KeyCode::Char('r') => self.start_moving(),
            KeyCode::Char('e') | KeyCode::Enter => self.start_editing(),
            KeyCode::Char('a') => self.start_adding(),
            KeyCode::Char('A') => self.start_adding_subtask(),
            KeyCode::Char('d') => self.ask_delete(),
            _ => {}
        }
    }

    /// Keyboard in the side menu: up / down choose a row, `gg` / `G` the first / last row (like vim; the
    /// first row is the 「隱藏全部」 switch, the last one the last calendar), `[` / `]` the previous / next
    /// top-level calendar,
    /// `Space` checks or unchecks it (an unchecked calendar's items are hidden), `Enter` shows only it / puts
    /// the checks back (`toggle_solo`; on a switch row, `Enter` flips the switch like `Space`), `a` adds a calendar
    /// (`A` inside the chosen one),
    /// `e` renames the chosen one, `d` deletes it, `L` / `H` move it one level in / out, `z` archives it or
    /// brings it back
    /// (`after_g`: the key before this one was `g`)
    fn handle_side_key(&mut self, code: KeyCode, after_g: bool) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.choose_group(1),
            KeyCode::Char('k') | KeyCode::Up => self.choose_group(-1),
            // The first row: the 「隱藏全部」 switch above the calendars
            KeyCode::Char('g') if after_g => self.side_row = SideRow::HideAll,
            KeyCode::Char('g') => self.pending_g = true,
            // The last row: the last calendar (no calendars: the 「隱藏做完的」 switch)
            KeyCode::Char('G') => {
                self.side_row = match self.groups.len().checked_sub(1) {
                    Some(last) => SideRow::Calendar(last),
                    None => SideRow::HideDone,
                };
            }
            KeyCode::Char('[') => self.jump_top_calendar(-1),
            KeyCode::Char(']') => self.jump_top_calendar(1),
            KeyCode::Char(' ') => self.toggle_group(),
            KeyCode::Enter => match self.chosen_side_row() {
                SideRow::Calendar(_) => self.toggle_solo(),
                SideRow::HideAll | SideRow::HideDone => self.toggle_group(),
            },
            KeyCode::Char('a') => self.start_adding_calendar(),
            KeyCode::Char('A') => self.start_adding_subcalendar(),
            KeyCode::Char('e') => self.start_renaming_calendar(),
            KeyCode::Char('d') => self.ask_delete_calendar(),
            KeyCode::Char('L') => self.shift_calendar(true),
            KeyCode::Char('H') => self.shift_calendar(false),
            KeyCode::Char('z') => self.toggle_archived(),
            _ => {}
        }
    }

    /// The calendar chosen in the side menu: checked ⇄ unchecked (`toggle_calendar`); on a switch row above
    /// the calendars, that switch
    fn toggle_group(&mut self) {
        match self.chosen_side_row() {
            SideRow::HideAll => self.toggle_hide_all(),
            SideRow::HideDone => self.toggle_hide_done(),
            SideRow::Calendar(_) => self.toggle_calendar(),
        }
    }

    /// The chosen calendar: checked ⇄ unchecked, and every calendar inside it gets the same
    /// (`chosen_family`), e.g. checking `Work` checks `Work > SFAP` too
    fn toggle_calendar(&mut self) {
        let Some(shown) = self.chosen_group().map(|chosen| !chosen.shown) else {
            return;
        };
        for index in self.chosen_family() {
            self.groups[index].shown = shown;
        }
    }

    /// The chosen calendar and the calendars inside it that go along with it (`Space`, `Enter`), as indexes in
    /// `groups`: every one any number of levels down, except an archived one inside a running calendar (it
    /// keeps its own check); empty on a switch row
    fn chosen_family(&self) -> Vec<usize> {
        let Some(chosen) = self.chosen_group() else {
            return Vec::new();
        };
        self.groups
            .iter()
            .enumerate()
            .filter(|(_, group)| {
                group.same(chosen) || (group.inside(chosen) && (chosen.archived || !group.archived))
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// `Enter` on a calendar in the side menu: shows only it, i.e. it and the calendars inside it are checked
    /// (`chosen_family`, as `Space` would check them) and every other one is unchecked, after remembering
    /// every calendar's check; `Enter` on it again puts those checks back (`Space` meanwhile changes nothing
    /// that is put back). `Enter` on another calendar meanwhile shows only that one instead, still
    /// remembering the checks from before the first `Enter`
    fn toggle_solo(&mut self) {
        let Some(path) = self.chosen_group().map(Group::path) else {
            return;
        };
        match self.solo.take() {
            // On the calendar shown alone: every check back as it was (a calendar added since keeps its own)
            Some(solo) if solo.path == path => {
                for group in &mut self.groups {
                    let path = group.path();
                    if let Some(&(_, shown)) = solo.checked.iter().find(|(old, _)| *old == path) {
                        group.shown = shown;
                    }
                }
            }
            solo => {
                let checked = match solo {
                    Some(solo) => solo.checked,
                    None => self
                        .groups
                        .iter()
                        .map(|group| (group.path(), group.shown))
                        .collect(),
                };
                let family = self.chosen_family();
                for (index, group) in self.groups.iter_mut().enumerate() {
                    group.shown = family.contains(&index);
                }
                self.solo = Some(Solo { path, checked });
            }
        }
    }

    /// The row chosen in the side menu: `side_row`, except that with no calendars (nothing read yet, or an
    /// empty task file) a calendar row counts as 「隱藏做完的」, the last row then
    pub fn chosen_side_row(&self) -> SideRow {
        match self.side_row {
            SideRow::Calendar(_) if self.groups.is_empty() => SideRow::HideDone,
            row => row,
        }
    }

    /// How many levels a calendar is indented in its part of the side menu: a running one by how many
    /// calendars it is inside; an archived one only by the archived calendars it is inside, so one archived
    /// inside a running calendar (`CAIS > 2603_…`) starts at the left of the archive part
    pub fn side_level(&self, group: &Group) -> usize {
        if !group.archived {
            return group.parents.len();
        }
        (1..=group.parents.len())
            .filter(|&depth| {
                self.groups
                    .iter()
                    .any(|other| other.archived && other.path() == group.parents[..depth])
            })
            .count()
    }

    /// The calendar chosen in the side menu; `None` on a switch row
    fn chosen_group(&self) -> Option<&Group> {
        match self.side_row {
            SideRow::Calendar(index) => self.groups.get(index),
            SideRow::HideAll | SideRow::HideDone => None,
        }
    }

    /// Whether every calendar is unchecked (then 「隱藏全部」 shows as checked); `false` with no calendars
    pub fn all_hidden(&self) -> bool {
        !self.groups.is_empty() && self.groups.iter().all(|group| !group.shown)
    }

    /// 「隱藏全部」: unchecks every calendar; when they are all unchecked already, checks them all again
    /// except the archived ones (they stay unchecked, as at start)
    fn toggle_hide_all(&mut self) {
        let show = self.all_hidden();
        for group in &mut self.groups {
            group.shown = show && !group.archived;
        }
    }

    /// `[calendar_list] hide_done` on ⇄ off (the calendar list's switch, or `.` anywhere), saved to the
    /// config file right away (see `save_switch`). The chosen item stays chosen; hidden now, the choice stays
    /// at the same position (see `keep_chosen_line`, `keep_choice_in_range`)
    fn toggle_hide_done(&mut self) {
        let line = self.chosen_item().map(|item| item.line);
        let before = self.settings.clone();
        self.settings.calendar_list.hide_done = !before.calendar_list.hide_done;
        self.save_switch(&before);
        if let Some(line) = line {
            self.keep_chosen_line(line);
        }
        self.keep_choice_in_range();
    }

    /// Fewer items may be listed now (a reload, hiding done tasks): a choice past the end of the agenda or
    /// layout 2's popup moves to the last item; with none left, back to the calendar
    fn keep_choice_in_range(&mut self) {
        if self.focus == Focus::Day {
            self.keep_day_choice();
        }
        if self.focus == Focus::Agenda && self.agenda_index >= self.agenda().len() {
            match self.agenda().len().checked_sub(1) {
                Some(last) => self.choose(last),
                None => self.focus = Focus::Calendar,
            }
        }
    }

    /// A switch flipped or the side menu's width dragged outside the settings page (`before` = the settings
    /// before it): saved to the config file right away (only that line); if it cannot save, the bottom row
    /// says so
    fn save_switch(&mut self, before: &Settings) {
        if let Some(path) = &self.config_path
            && let Err(error) = config::save(path, before, &self.settings)
        {
            self.notice = Some(Notice::SettingsFailed(error.kind()));
        }
    }

    /// Mouse: the open popup or action in progress (`mode`) gets it first; otherwise see `handle_normal_mouse`
    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent) {
        let pressed = matches!(
            mouse.kind,
            MouseEventKind::Down(_) | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        );
        // The intro: a click or the wheel skips it and does nothing else (moving the mouse does not; a
        // notice from the start stays, like with a key)
        if let Mode::Splash { .. } = self.mode {
            if pressed {
                self.mode = Mode::Normal;
            }
            return;
        }
        // The last notice has been seen, but only a press or the wheel counts; release, move, and drag do not.
        // A click sends a press and then a release,
        // and if the release cleared it too, a notice from the click (e.g. an event cannot be checked) would
        // vanish right away
        if pressed {
            self.notice = None;
            // A click or the wheel between two `g`s: no longer `gg`
            self.pending_g = false;
        }
        match self.mode {
            // Help page: the wheel scrolls it, a click outside closes it
            Mode::Help(_) => self.handle_help_mouse(mouse),
            // Settings page: the wheel chooses rows, clicks on rows, a click outside closes it
            Mode::Settings { .. } => self.handle_settings_mouse(mouse),
            // Asking whether to delete: a click or the wheel just cancels (like any key other than `y`);
            // that event does nothing else
            Mode::ConfirmDelete(..) | Mode::ConfirmDeleteCalendar { .. } => {
                if matches!(
                    mouse.kind,
                    MouseEventKind::Down(_) | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) {
                    self.mode = Mode::Normal;
                }
            }
            // Editor box: the `#` list takes clicks and the wheel, a click outside = cancel (Esc)
            Mode::Editor(_) => self.handle_editor_mouse(mouse),
            // Rescheduling: the wheel moves the selected day as usual, clicking a date picks the new date,
            // other clicks are ignored
            Mode::Moving(_) => self.handle_moving_mouse(mouse),
            // The intro: skipped above
            Mode::Splash { .. } => {}
            Mode::Normal => self.handle_normal_mouse(mouse),
        }
    }

    /// Mouse when no popup is open: the wheel moves up / down (a week on the calendar, one item in the agenda
    /// or on the tasks page, one calendar in the side menu);
    /// a left click on a calendar date selects it (a double click = go into that day's items), on an agenda item
    /// chooses it (a double click = check it),
    /// on a tasks page row chooses it (a double click = check it; on `▸` = expand / collapse, a right click
    /// there = the whole subtree), on a side menu calendar chooses it (again = check / uncheck; a right click
    /// = only that one / the checks back, like `Enter`); dragging in
    /// the day popup goes over its text, and letting go copies it; dragging the side menu's right border
    /// changes its width
    fn handle_normal_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            // Scroll down (to see later ones), scroll up
            MouseEventKind::ScrollDown => self.scroll(1),
            MouseEventKind::ScrollUp => self.scroll(-1),
            // The side menu's right border: hold it down to drag it, double-click it for the auto width (not
            // while the day popup is open: a click outside the popup closes it)
            MouseEventKind::Down(MouseButton::Left)
                if self.focus != Focus::Day
                    && ui::on_side_border(self, mouse.column, mouse.row) =>
            {
                self.press_side_border(mouse.column, Instant::now());
            }
            MouseEventKind::Drag(MouseButton::Left) if self.side_drag.is_some() => {
                self.drag_side_border(mouse.column);
            }
            MouseEventKind::Up(MouseButton::Left) if self.side_drag.is_some() => {
                if let Some(drag) = self.side_drag.take() {
                    self.save_switch(&drag.before);
                }
            }
            // Narrow, side menu over the left side: a click outside hides it (like Esc) and does nothing else
            MouseEventKind::Down(MouseButton::Left)
                if ui::side_overlay(self)
                    .is_some_and(|area| !area.contains(Position::new(mouse.column, mouse.row))) =>
            {
                self.put_side_away();
            }
            // Layout 2's popup is open: click items in it, a click outside closes it
            MouseEventKind::Down(MouseButton::Left) if self.focus == Focus::Day => {
                self.click_day(mouse.column, mouse.row, Instant::now());
            }
            // Dragging with the left button held in the popup: over its text; letting go copies it
            MouseEventKind::Drag(MouseButton::Left) if self.focus == Focus::Day => {
                self.drag_day(mouse.column, mouse.row);
            }
            MouseEventKind::Up(MouseButton::Left) if self.focus == Focus::Day => self.release_day(),
            // Only a click on a date or an item selects; clicks on headers, lines, or blank cells are ignored
            MouseEventKind::Down(MouseButton::Left) => {
                match ui::click_at(self, mouse.column, mouse.row) {
                    Some(Click::Day(date)) => self.click_calendar_day(date, Instant::now()),
                    Some(Click::Item(index)) => self.click_item(index, Instant::now()),
                    Some(Click::Group(index)) => self.click_group(index),
                    Some(Click::HideAll) => {
                        self.focus = Focus::Side;
                        self.side_row = SideRow::HideAll;
                        self.toggle_hide_all();
                    }
                    Some(Click::HideDone) => {
                        self.focus = Focus::Side;
                        self.side_row = SideRow::HideDone;
                        self.toggle_hide_done();
                    }
                    Some(Click::TaskRow { row, on_marker }) => {
                        self.click_task(row, on_marker, Instant::now());
                    }
                    Some(Click::View(view)) => self.set_view(view),
                    None => {}
                }
            }
            // Right click on a tasks page item's `▸` / `▾`: its whole subtree; on a side menu calendar: like
            // `Enter` on it; anywhere else it is ignored
            MouseEventKind::Down(MouseButton::Right) => {
                match ui::click_at(self, mouse.column, mouse.row) {
                    Some(Click::TaskRow {
                        row,
                        on_marker: true,
                    }) => self.right_click_task(row),
                    Some(Click::Group(index)) => self.right_click_group(index),
                    _ => {}
                }
            }
            // Anything else (other drags, move) is ignored
            _ => {}
        }
    }

    /// The side menu's right border pressed at terminal column `column`, time `now`: starts dragging it; the
    /// second click of a double click puts the width back to auto (it follows the names again) and saves that
    fn press_side_border(&mut self, column: u16, now: Instant) {
        if self.is_double_click(ClickTarget::SideBorder, now) {
            let before = self.settings.clone();
            self.settings.calendar_list.width = None;
            self.save_switch(&before);
            return;
        }
        self.side_drag = Some(SideDrag {
            before: self.settings.clone(),
            column,
            width: ui::side_width(self),
        });
    }

    /// Dragging the side menu's right border to terminal column `column`: the width follows the mouse
    /// (moved 3 cells right = 3 cells wider) and is fixed from now on (`[calendar_list] width`), kept between
    /// `MIN_WIDTH` and `MAX_WIDTH`; letting go saves it to the config file
    fn drag_side_border(&mut self, column: u16) {
        let Some(drag) = &self.side_drag else {
            return;
        };
        // Dragged, so letting go and clicking it again is not a double click
        self.last_click = None;
        let moved = i32::from(column) - i32::from(drag.column);
        let width = (i32::from(drag.width) + moved).clamp(
            i32::from(CalendarListSettings::MIN_WIDTH),
            i32::from(CalendarListSettings::MAX_WIDTH),
        );
        self.settings.calendar_list.width = u16::try_from(width).ok();
    }

    /// A click on a calendar day at time `now` (both layouts): selects the day; a second click on the same
    /// day within `DOUBLE_CLICK` (a double click) goes into that day's items, like `Enter`. Clicking the
    /// selected day again later only selects it, so a stray click does not move the keyboard or open a popup
    fn click_calendar_day(&mut self, day: Date, now: Instant) {
        let double = self.is_double_click(ClickTarget::Day(day), now);
        self.selected = day;
        self.focus = Focus::Calendar;
        if double {
            self.open_day();
        }
    }

    /// A click on agenda row `index` at time `now`: chooses it; a double click checks it off. Clicking the
    /// chosen item again later only chooses it, so a stray click does not check it off by accident
    fn click_item(&mut self, index: usize, now: Instant) {
        self.choose(index);
        if let Some(line) = self.chosen_item().map(|item| item.line)
            && self.is_double_click(ClickTarget::Item(line), now)
        {
            self.toggle_chosen();
        }
    }

    /// Whether a click on `target` at time `now` is the second click of a double click (the click before it
    /// was on the same one, at most `DOUBLE_CLICK` earlier); remembers this click for the next one
    fn is_double_click(&mut self, target: ClickTarget, now: Instant) -> bool {
        let double = self
            .last_click
            .is_some_and(|(last, at)| last == target && now.duration_since(at) <= DOUBLE_CLICK);
        // A third quick click starts over instead of counting as another double click
        self.last_click = if double { None } else { Some((target, now)) };
        double
    }

    /// Wheel down (`step` = 1) / up (-1): moves a week when the keyboard is on layout 1's calendar (layout 2's
    /// only scrolls, `scroll_month`), one item in the agenda, the tasks page, or the popup, and one calendar
    /// in the side menu
    fn scroll(&mut self, step: i8) {
        match self.focus {
            Focus::Calendar if self.view == View::Month => self.scroll_month(step),
            Focus::Calendar => self.move_by(7 * i64::from(step)),
            Focus::Agenda => self.choose_next(isize::from(step)),
            Focus::Side => self.choose_group(isize::from(step)),
            Focus::Tasks => self.choose_task(isize::from(step)),
            Focus::Day => self.choose_day(isize::from(step)),
        }
    }

    /// Switches layout: layout 1's calendar scrolls the selected day's month to the top, layout 2 puts the
    /// selected week on row 2; layout 2 has no agenda, so a keyboard there goes back to the calendar
    fn set_view(&mut self, view: View) {
        self.view = view;
        self.scroll_to_month();
        self.selected_week_on_row_2();
        // The side menu stays; other panes go to this layout's main pane (layout 2 has no agenda, the tasks
        // page has no calendar)
        if self.focus != Focus::Side {
            self.focus = match (view, self.focus) {
                (View::Planner, Focus::Agenda) => Focus::Agenda,
                _ => self.home_focus(),
            };
        }
    }

    /// `t`: selects today and scrolls like switching layouts (`set_view`): layout 1's calendar puts today's
    /// month on top, layout 2 today's week on row 2
    fn go_today(&mut self) {
        self.selected = self.today;
        self.scroll_to_month();
        self.selected_week_on_row_2();
    }

    /// Layout 2: the selected week goes on row 2, the week before on top, so the days just past still show
    /// (with `[month] center_cursor` on, `ui::month::render` puts it on row 3 as usual)
    fn selected_week_on_row_2(&mut self) {
        self.month_top = Some(calendar::monday_of(self.selected).saturating_sub(1.week()));
    }

    /// Layout 2's wheel: the 6 weeks move a week down (`step` = 1) / up (-1) and the selected day stays, even
    /// off screen; the next key that moves the selection brings it back into view (`ui::month::render`)
    fn scroll_month(&mut self, step: i8) {
        let top = self
            .month_top
            .unwrap_or_else(|| calendar::monday_of(self.selected.first_of_month()));
        self.month_top = Some(top.saturating_add((7 * i64::from(step)).days()));
        self.month_free = Some(self.selected);
    }

    /// On the next draw, scroll the selected day's month to the top (in both layouts)
    fn scroll_to_month(&mut self) {
        self.calendar_top = None;
        self.month_top = None;
    }

    /// `Tab`: calendar → agenda (skipped if there are no items, or in layout 2) → side menu (only if drawn)
    /// → calendar;
    /// on the tasks page: tasks page ⇄ side menu (only if drawn); ignored while layout 2's popup is open
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
    /// items, or in layout 2) → calendar;
    /// on the tasks page: tasks page ⇄ side menu (only if drawn); ignored while layout 2's popup is open
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
        if self.focus == Focus::Side && self.chosen_side_row() == SideRow::Calendar(index) {
            self.toggle_group();
        } else {
            self.focus = Focus::Side;
            self.side_row = SideRow::Calendar(index);
        }
    }

    /// Right click on calendar number `index` in the side menu: chooses it (the keyboard moves to the side
    /// menu) and does what `Enter` does there: shows only it, or on the calendar shown alone puts the checks
    /// back (`toggle_solo`)
    fn right_click_group(&mut self, index: usize) {
        self.focus = Focus::Side;
        self.side_row = SideRow::Calendar(index);
        self.toggle_solo();
    }

    /// Moves the side menu choice `step` rows down (negative = up), in screen order: 「隱藏全部」, 「隱藏做完的」,
    /// then the calendars; stops at the top and bottom rows
    fn choose_group(&mut self, step: isize) {
        let count = self.groups.len();
        // The chosen row → its row on screen (0 = the top), move, and back
        let row = match self.chosen_side_row() {
            SideRow::HideAll => 0,
            SideRow::HideDone => 1,
            SideRow::Calendar(index) => index + 2,
        };
        self.side_row = match row.saturating_add_signed(step).min(count + 1) {
            0 => SideRow::HideAll,
            1 => SideRow::HideDone,
            row => SideRow::Calendar(row - 2),
        };
    }

    /// `[` / `]` in the side menu: chooses the previous (`step` = -1) / next (1) top-level calendar,
    /// skipping the calendars inside them, like `[` `]` on the tasks page go to the previous / next
    /// calendar; from a calendar inside one, `[` goes to the top-level calendar before the one it is in.
    /// In the archive part, top level = at the left of that part (`side_level`).
    /// With no top-level calendar before, `[` goes up to the 「隱藏全部」 switch (the first row, like `gg`);
    /// on a switch row, `]` goes to the first calendar; `]` stays put on the last one
    fn jump_top_calendar(&mut self, step: isize) {
        // Index in `groups` of each top-level calendar, in screen order
        let tops: Vec<usize> = self
            .groups
            .iter()
            .enumerate()
            .filter(|(_, group)| self.side_level(group) == 0)
            .map(|(index, _)| index)
            .collect();
        // The top-level calendar the chosen row is in: the last one at or above it (none on a switch row)
        let current = match self.chosen_side_row() {
            SideRow::Calendar(chosen) => tops.iter().rposition(|&index| index <= chosen),
            SideRow::HideAll | SideRow::HideDone => None,
        };
        let target = match current {
            Some(current) => current.checked_add_signed(step),
            None if step > 0 => Some(0),
            None => None,
        };
        match target.and_then(|target| tops.get(target)) {
            Some(&index) => self.side_row = SideRow::Calendar(index),
            // Past the first top-level calendar: the 「隱藏全部」 switch
            None if step < 0 => self.side_row = SideRow::HideAll,
            None => {}
        }
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
    /// If the agenda does not have it (e.g. its date was removed, or layout 2 has no agenda), back to the
    /// calendar. On the tasks page, choose that row; in layout 2's popup, choose that item in the popup
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
        match self.agenda_index_of(line) {
            Some(index) => self.choose(index),
            None => self.focus = Focus::Calendar,
        }
    }

    /// After the items change in a way that can move the chosen item within its day (`Space`, `/`:
    /// in-progress items come first; a change made elsewhere), keeps the item on file line `line` chosen in
    /// the agenda or layout 2's popup; if it is no longer listed (e.g. hidden once done), the choice stays
    /// at the same position, on the next item
    fn keep_chosen_line(&mut self, line: usize) {
        match self.focus {
            Focus::Agenda => {
                if let Some(index) = self.agenda_index_of(line) {
                    self.choose(index);
                }
            }
            Focus::Day => self.choose_day_line(line),
            // The tasks page remembers its choice by file line already
            Focus::Calendar | Focus::Side | Focus::Tasks => {}
        }
    }

    /// Where the item on file line `line` is in the agenda; for an item spanning months, the row in the
    /// selected day's month
    fn agenda_index_of(&self, line: usize) -> Option<usize> {
        let agenda = self.agenda();
        agenda
            .iter()
            .position(|entry| entry.item.line == line && entry.is_on(self.selected))
            .or_else(|| agenda.iter().position(|entry| entry.item.line == line))
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
mod tests;
