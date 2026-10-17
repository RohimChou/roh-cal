# roh-cal
A simple, keyboard-driven terminal calendar for plain-text tasks.

> v2 is a rewrite in progress. The first version lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

## Run
```sh
cargo run --release -- --tasks testdata/sample.md
```

| Option | Meaning |
| :- | :- |
| `--tasks <PATH>` | Task file to show (default: `~/.local/share/roh-cal/tasks.md`; a missing default file means no tasks) |
| `-h`, `--help` | Print help |
| `-V`, `--version` | Print version |

The task file format is in [docs/task-file-format.md](docs/task-file-format.md).

## Keys
| Key | Action |
| :- | :- |
| `h` `j` `k` `l` / arrows | Move a day / a week (in the agenda or the side menu: up and down) |
| `Tab` | Next pane: calendar → agenda → side menu |
| `Space` / `Enter` | In the side menu: show or hide a calendar (a group in the task file) |
| `s` | Hide or show the side menu (only drawn when the terminal is at least 120 columns wide) |
| `1` / `2` | Layout: the month's items next to each month / the focused day and the days after it |
| `t` | Today |
| `L` | Switch between Chinese and English |
| `q` / `Esc` | Quit (`Esc` in another pane goes back to the calendar) |
