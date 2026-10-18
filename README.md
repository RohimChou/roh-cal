# roh-cal
A simple, keyboard-driven terminal calendar for plain-text tasks.

<!-- After adding the CI workflow (.github/workflows/ci.yml):
[![CI](https://github.com/RohimChou/roh-cal/actions/workflows/ci.yml/badge.svg)](https://github.com/RohimChou/roh-cal/actions/workflows/ci.yml)
-->
[![License: MIT](https://img.shields.io/github/license/RohimChou/roh-cal)](LICENSE)
![Platform: macOS | Linux](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey)
<!-- After the first release on crates.io:
[![crates.io](https://img.shields.io/crates/v/roh-cal)](https://crates.io/crates/roh-cal)
-->

<!-- Demo GIF: record it with `vhs docs/demo.tape` (see the comments in that file), then uncomment:
![roh-cal: calendar, month grid and tasks page](docs/demo.gif)
-->

Your tasks live in one Markdown file that you can edit in any editor (Obsidian included), keep in git and search with `grep`. roh-cal shows them on a calendar, and writes your changes back to that file without touching anything else.

> This is v2, a rewrite. The first version lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

## Features
- **Three views**: a calendar with the agenda next to it, a six-week month grid with each day's items in its cell, and a tasks page with the whole file as a tree
- **Plain text**: tasks (`- [ ]`) and events, dates, times, multi-day ranges, subtasks, `#` headings as calendars; mistakes such as `@2026-13-01` are pointed out instead of silently dropped
- **Edit in place**: check off, add, edit, reschedule (the calendar is the date picker), delete, indent and reorder, and undo the last 50 writes
- **Safe writes**: only the changed lines are rewritten, through a temp file; if the file changed elsewhere since roh-cal read it, roh-cal reloads instead of overwriting
- **Calendars**: show or hide each `#` group; a heading marked `type:holiday` colors its days as holidays
- **Mouse**: the wheel scrolls and clicks select everywhere; a double click opens a day, a second click checks off
- **Themes**: six built in, and any color can be overridden per role from a settings page
- **English and 繁體中文**

## Install
roh-cal is not on crates.io yet. Build it from source with Rust (stable):

```sh
git clone https://github.com/RohimChou/roh-cal
cd roh-cal
cargo install --locked --path .
```

<!-- After the first release:
```sh
cargo install --locked roh-cal
brew install rohimchou/tap/roh-cal
```
Prebuilt binaries: https://github.com/RohimChou/roh-cal/releases
-->

## Quick start
Try it with the sample task file. Copy it first, because roh-cal writes your changes back to the file it shows:

```sh
cp testdata/tasks.md /tmp/tasks.md
roh-cal --tasks /tmp/tasks.md
```

Press `?` for the keys, `3` and `4` for the other views, `q` to quit. Without `--tasks`, roh-cal reads `~/.local/share/roh-cal/tasks.md`. A task file looks like this:

```markdown
# Work
- [ ] Review the payment API @2026-10-14
  - [ ] Write the integration tests @2026-10-13
- Weekly meeting @2026-10-19 10:00
- Business trip @2026-10-28..2026-11-03

# Home
- [ ] Pay the credit card bill @2026-10-25
- [ ] Tidy up the bookshelf
```

The full format is in [docs/task-file-format.md](docs/task-file-format.md).

## Views
| Key | View |
| :- | :- |
| `1` | Calendar + agenda: the calendar on the left scrolls through the months; the agenda on the right lists each month's items next to it |
| `3` | Month grid: six rows of weeks, each day's items in its cell (`+N` when they don't fit); every month starts a new row, and a thick line with the year marks a new year; `Enter` opens a popup with all of that day's items |
| `4` | Tasks page: the whole task file by calendar, undated items too, subtasks as a tree; the bottom line shows the chosen line as written in the file, and lines with a format mistake are red |

A terminal at least 120 columns wide also shows the calendar list on the left; narrower, `s` calls it up.

## Command-line options
| Option | Meaning |
| :- | :- |
| `--lang en\|zh-TW` | Screen language. Default: the system language, else English |
| `--theme <name>` | Color theme: `catppuccin-mocha` (default), `gruvbox-dark`, `tokyo-night`, `dracula`, `nord` or `terminal` (the terminal's own 16 colors, for any background). Default: the config file's `theme` |
| `--tasks <path>` | Task file to show. Default: `~/.local/share/roh-cal/tasks.md` (`$XDG_DATA_HOME` respected); a missing default file just means no tasks |
| `-h`, `--help` | Print the options |
| `-v`, `-V`, `--version` | Print the version |

## Keys
In roh-cal, `?` shows the most used keys and `Tab` there shows them all.

**Everywhere**

| Key | Action |
| :- | :- |
| `1` / `3` / `4` | Switch to the calendar + agenda / the month grid / the tasks page |
| `Tab` | Switch between the calendar, the agenda and the calendar list when it is on screen (the focused pane's border lights up); does nothing while layout 3's day popup is open |
| `s` | Hide / show the calendar list on the left (the task file's `#` groups) when the terminal is at least 120 columns wide; narrower, `s` calls it up over the left of the screen with the keyboard in it, and `s`, `Esc`, `q` or clicking outside put it away, back to the pane you were in |
| `u` | Undo roh-cal's last write to the task file (up to 50; only if the file hasn't changed elsewhere since, otherwise it just reloads) |
| `?` | Help: the most used keys; `Tab` shows all keys. `j` `k` or the wheel scroll it when the terminal is too short; `?`, `q`, `Esc` or clicking outside close it |
| `,` | Settings: the config file's settings (below) in a list; `j` `k` choose a row, `h` `l` (or `Space`, `Enter`, clicking the chosen row) change it (the theme row goes through the built-in themes, a color row through the theme's palette) and the screen follows right away; a short terminal scrolls the list; `r` resets the row, `R` then `y` resets them all; `q`, `Esc`, `,` or clicking outside close it and save to the config file (only the changed lines; comments stay) |
| `Esc` | Back to the layout's main pane (the calendar, or the tasks page) from the agenda, layout 3's day popup or the calendar list; from the tasks page to layout 1; quit from the calendar |
| `q` / `Ctrl+C` | Quit (`q` in layout 3's day popup or the narrow calendar list only closes it) |

**On the calendar** (layouts 1 and 3)

| Key | Action |
| :- | :- |
| `h` `j` `k` `l` / arrows | Previous day / next week / previous week / next day |
| `[` / `]` | First day of the previous / next month |
| `t` | Jump to today |
| `Enter` | Go into the selected day's items: in layout 1 the agenda's first item that day; layout 3 opens a popup over the month grid with all of that day's items, the ones hidden behind `+N` too, where the item keys below work (`Esc`, `q` or clicking outside close it). With the mouse: double-click a day |
| `a` | Add an item on the selected day, in the calendar of that day's first item (or at the end of the file when the day has none) |

**On an item** (the agenda, layout 3's day popup, the tasks page)

| Key | Action |
| :- | :- |
| `j` `k` / `↓` `↑` | Choose the next / previous item |
| `Space` / `x` | Check off the chosen task, or uncheck it (written back to the task file; an event has no box) |
| `e` / `Enter` | Edit the chosen item's line in a one-line editor that shows, as you type, how the line will be read; `Enter` saves (not while the line has a mistake), `Esc` or clicking outside cancels. Inside: `←` `→` `Home` `End` move, `Ctrl+U` clears |
| `a` / `A` | Add an item to the chosen item's calendar in the same one-line editor, the selected day already filled in (the tasks page fills in no date) / add a subtask under the chosen item |
| `r` | Reschedule the chosen item (the tasks page borrows layout 1's calendar; an undated item gets the day you pick). The calendar becomes the date picker (`hjkl`, `[` `]`, `t`, mouse); `Enter` (or clicking the same day again) moves the item by as many days as you moved, `Esc` cancels. A multi-day item moves as a whole |
| `d` | Delete the chosen item with its subtasks, after asking on the bottom line (`y` deletes, any other key cancels) |

**On the tasks page**

| Key | Action |
| :- | :- |
| `l` / `h` | Expand / collapse the chosen item's subtasks (`l` again goes to the first subtask, `h` again to the parent); click `▸` to do the same |
| `+` / `-` | Expand / collapse every item's subtasks (when the chosen item gets hidden, its top-level item is chosen instead) |
| `L` / `H` | Indent the chosen item with its subtasks under the item above it at the same level (2 more spaces than that item, like `A`) / outdent it to its parent's level, where the items below it at the same level become its subtasks (as in Obsidian); written back to the task file |
| `J` / `K` | Move the chosen item with its subtasks below / above the next item at the same level (it stays under the same parent and calendar); written back to the task file |

**In the calendar list**

| Key | Action |
| :- | :- |
| `j` `k` / `↓` `↑` | Choose the next / previous calendar |
| `Space` / `Enter` | Check / uncheck the chosen calendar; an unchecked calendar's items are hidden |

**Mouse**

| Action | Effect |
| :- | :- |
| Wheel | Move one week (in the agenda, the day popup and on the tasks page: one item, in the calendar list: one calendar) |
| Click | Select a day, an item or a calendar; double-click a day to go into its items; click a chosen task again to check it off, a selected calendar again to check / uncheck it. Hold `Shift` to select text in the terminal |

## Config file
`~/.config/roh-cal/config-v2.toml` (`$XDG_CONFIG_HOME` respected; v1's `config.toml` next to it is left alone). It is optional: write only what you want to change, the rest keeps its default. A misspelt name or a wrong type stops roh-cal at start and says what is wrong (the line, or for `[colors]` the role and value). The settings page (`,`) edits it for you. Example: [testdata/config.toml](testdata/config.toml).

| Name | Default | Meaning |
| :- | :- | :- |
| `theme` | `"catppuccin-mocha"` | Color theme, same names as `--theme` (the built-in ones are in [themes/](themes/)) |
| `[calendar]` `relative_weeks` | `false` | Week column: the selected week shows its number, the other weeks how far away they are |
| `[month]` `grid_lines` | `true` | Faint lines between the cells of the month grid (layout 3); without them a new year still gets its thick line |
| `[month]` `item_icons` | `true` | A dot before each item in the month grid's cells |
| `[month]` `center_cursor` | `false` | Keep the selected row in the middle of the month grid's six rows (off: scroll a row at a time when the selection leaves them) |
| `[colors]` *role* | the theme's | Override one role's color, e.g. `today = "#a6e3a1"`: a color (`#rrggbb`, a terminal color name such as `light-blue` or `LightBlue`, or a 256-color number) or a name from the theme's palette. The roles are the names under `[roles]` in [themes/](themes/) |

## Themes
Built in: `catppuccin-mocha` (default), `gruvbox-dark`, `tokyo-night`, `dracula`, `nord`, and `terminal`, which only uses the terminal's own 16 colors so it follows your terminal's color scheme. Pick one with `--theme`, the `theme` setting or the settings page; override single colors with `[colors]`. Each theme is a small TOML file in [themes/](themes/): a `[palette]` of named colors and the `[roles]` that use them.

<!-- Screenshots of each theme: add them here once recorded -->

## Contributing
Bug reports and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build, test and propose changes, and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (in Traditional Chinese) for the modules, the layout model and how the screen tests work.

## License
[MIT](LICENSE)
