# roh-cal
A simple, keyboard-driven terminal calendar for plain-text tasks.

> v2 is a rewrite in progress. The first version lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

## Run
```sh
cargo run --release -- --tasks testdata/tasks.md    # try it with the sample task file
```

| Option | Meaning |
| :- | :- |
| `--lang en\|zh-TW` | Screen language. Default: the system language, else English |
| `--tasks <path>` | Task file to show. Default: `~/.local/share/roh-cal/tasks.md` (`$XDG_DATA_HOME` respected); a missing default file just means no tasks |
| `-h`, `--help` | Print the options |
| `-v`, `-V`, `--version` | Print the version |

The task file format is in [docs/task-file-format.md](docs/task-file-format.md).

For developers: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (in Traditional Chinese) explains the modules, the layout model and how the screen tests work.

## Keys
| Key | Action |
| :- | :- |
| `h` `j` `k` `l` / arrows | Previous day / next week / previous week / next day; in the agenda `j` `k` / ↓ ↑ choose the next / previous item, in the calendar list the next / previous calendar |
| `Tab` | Switch between the calendar, the agenda and the calendar list (the focused pane's border lights up) |
| `Space` / `Enter` | In the calendar list: check / uncheck a calendar; an unchecked calendar's items are hidden |
| `Space` / `x` | In the agenda or on the tasks page: check off the chosen task, or uncheck it (written back to the task file; an event has no box) |
| `[` / `]` | First day of the previous / next month |
| `t` | Jump to today |
| `r` | In the agenda or on the tasks page: reschedule the chosen item (the tasks page borrows layout 1's calendar; an undated item gets the day you pick). The calendar becomes the date picker (`hjkl`, `[` `]`, `t`, mouse); `Enter` (or clicking the same day again) moves the item by as many days as you moved, `Esc` cancels. A multi-day item moves as a whole |
| `Enter` | On the calendar: go into the selected day's items (the agenda's first item that day; layout 3 switches to layout 1). Clicking the selected day again does the same |
| `e` / `Enter` | In the agenda or on the tasks page: edit the chosen item's line in a one-line editor that shows, as you type, how the line will be read; `Enter` saves (not while the line has a mistake), `Esc` or clicking outside cancels. Inside: `←` `→` `Home` `End` move, `Ctrl+U` clears |
| `a` / `A` | Add an item in the same one-line editor, the selected day already filled in: in the agenda or on the tasks page to the chosen item's calendar (the tasks page fills in no date), on the calendar to the selected day / add a subtask under the chosen item |
| `d` | In the agenda or on the tasks page: delete the chosen item with its subtasks, after asking on the bottom line (`y` deletes, any other key cancels) |
| `u` | Undo roh-cal's last write to the task file (up to 50; only if the file hasn't changed elsewhere since, otherwise it just reloads) |
| `1` / `3` / `4` | Layout 1 (calendar + agenda) / layout 3 (a big month grid, six weeks with each day's items in its cell) / the tasks page (the whole task file by calendar, undated items too, subtasks as a tree; the bottom line shows the chosen line as written in the file) |
| `l` / `h` | On the tasks page: expand / collapse the chosen item's subtasks (`l` again goes to the first subtask, `h` again to the parent); click `▸` to do the same |
| `s` | Hide / show the calendar list on the left (the task file's `#` groups); only when the terminal is at least 120 columns wide |
| Mouse wheel / click | Move one week (in the agenda: one item, in the calendar list: one calendar) / select a day, an item or a calendar (click a selected calendar again to check / uncheck it, a chosen task again to check it off; hold `Shift` to select text in the terminal) |
| `Esc` | Back to the calendar from the agenda or the calendar list, or from the tasks page to layout 1; quit from the calendar |
| `q` / `Ctrl+C` | Quit |
