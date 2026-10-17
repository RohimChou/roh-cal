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
| `[` / `]` | First day of the previous / next month |
| `t` | Jump to today |
| `1` / `3` | Layout 1 (calendar + agenda) / layout 3 (a big month grid, six weeks with each day's items in its cell) |
| `s` | Hide / show the calendar list on the left (the task file's `#` groups); only when the terminal is at least 120 columns wide |
| Mouse wheel / click | Move one week (in the agenda: one item, in the calendar list: one calendar) / select a day, an item or a calendar (click a selected calendar again to check / uncheck it; hold `Shift` to select text in the terminal) |
| `Esc` | Back to the calendar from the agenda or the calendar list; quit from the calendar |
| `q` / `Ctrl+C` | Quit |
