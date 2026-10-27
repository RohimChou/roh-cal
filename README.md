# roh-cal
A simple, keyboard-driven terminal calendar for plain-text tasks.

<!-- After adding the CI workflow (.github/workflows/ci.yml):
[![CI](https://github.com/RohimChou/roh-cal/actions/workflows/ci.yml/badge.svg)](https://github.com/RohimChou/roh-cal/actions/workflows/ci.yml)
-->
[![License: MIT](https://img.shields.io/github/license/RohimChou/roh-cal)](LICENSE)
![Platform: macOS | Linux](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey)
[![crates.io](https://img.shields.io/crates/v/roh-cal)](https://crates.io/crates/roh-cal)

<!-- Demo GIF: record it with `vhs docs/demo.tape` (see the comments in that file), then uncomment:
![roh-cal: calendar, month grid and tasks page](docs/demo.gif)
-->

Your tasks live in one Markdown file that you can edit in any editor (Obsidian included), keep in git and search with `grep`. roh-cal shows them on a calendar, and writes your changes back to that file without touching anything else.

> This is v2, a rewrite. The first version lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

## Features
- **Three views**: a calendar with the agenda next to it, a six-week month grid with each day's items in its cell, and a tasks page with the whole file as a tree
- **Plain text**: tasks (`- [ ]`, in progress `- [/]`) and events, dates, times, multi-day ranges, priorities (`!` `!!` `!!!`), subtasks, `#` headings as calendars; mistakes such as `@2026-13-01` are pointed out instead of silently dropped
- **Edit in place**: check off, add, edit, reschedule (the calendar is the date picker), delete, indent and reorder, and undo the last 50 writes
- **Safe writes**: only the changed lines are rewritten, through a temp file; if the file changed elsewhere since roh-cal read it, roh-cal reloads instead of overwriting
- **Live reload**: edit the task file in another editor and every view updates within a second
- **Calendars**: show or hide each `#` group, nested by heading level (`#` Work → `##` SFAP); a heading marked `type:holiday` makes its days holidays, colored like weekends; one marked `archived:<date>` (a finished project) moves to the bottom of the list, hidden until you check it
- **Mouse**: the wheel scrolls and clicks select everywhere; a double click opens a day or checks off a task
- **Themes**: eight built in, and any color can be overridden per role from a settings page
- **English and 繁體中文**

## Install
With Rust (stable) installed:

```sh
cargo install --locked roh-cal
```

Or build the latest code from source:

```sh
git clone https://github.com/RohimChou/roh-cal
cd roh-cal
cargo install --locked --path .
```

<!-- After the Homebrew tap and GitHub Releases are set up:
```sh
brew install rohimchou/tap/roh-cal
```
Prebuilt binaries: https://github.com/RohimChou/roh-cal/releases
-->

## Quick start
Try it with the built-in sample task file:

```sh
roh-cal --sample
```

The very first time (and with every `--sample`), it opens with a 5.5-second intro (any key skips it); `~/.local/state/roh-cal/splash-seen` (`$XDG_STATE_HOME` respected) remembers it has played; resetting all settings (`,` then `R` `y`) plays it again on the next start. Every `--sample` starts from a fresh copy with its dates moved to this week, so check off, edit and delete as you like. Press `?` for the keys, `2` and `3` for the other views, `q` to quit. Without `--tasks`, roh-cal reads `~/.local/share/roh-cal/tasks.md`. A task file looks like this:

```markdown
# Work
- [ ] Review the payment API !! @2026-10-14
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
| `1` | Calendar + agenda: the calendar on the left scrolls through the months, with a dot right of each date for each of that day's items (up to 4; colored like the day's most urgent open item: red / orange / yellow for `!!!` / `!!` / `!`, else blue for a holiday, else gray) and a multi-day item underlined as one line across its days (where two or more overlap, the underline turns the `range_overlap` color); the agenda on the right lists each month's items next to it in one column (a busy month takes at most 2/3 of the height and 16 rows, and its items scroll inside it) |
| `2` | Month grid: six rows of weeks, each day's items in its cell (`+N` when they don't fit); a multi-day item is one bar across its days, on the same line all the way, with its title written once (bars stacked on lines next to each other take turns between two shades, `range` and `range_alt`); an in-progress item leaves its time out of the cell (the popup shows it); a month's edge is a staircase line in the border color, a year's edge a thick one, and the week of a 1st has the month (and in January the year) under its week number; `Enter` opens a popup with all of that day's items, each after its check box (`∙` for an event; only a multi-day item writes its dates, e.g. `10/16~18`), a subtask under its parent and 2 cells further in (a parent not on that day is listed too, gray and without a check box, so you can tell whose subtasks they are), grouped under their calendars as a tree like the tasks page (e.g. `Work`, and under it `SFAP` 2 cells further in for `Work > SFAP`; with `[day_popup] by_calendar = false`, one list with the outermost calendar's name at each item's right end); in one list the bottom border shows the chosen item's whole calendar path, and the top border's right end counts the day's tasks, e.g. `3 open / 2 done` (the open count is red when more than half is left, orange from a third, yellow from a fifth; the done count is green) |
| `3` | Tasks page: the whole task file by calendar, undated items too, subtasks as a tree; a long title wraps onto the next lines; the bottom line shows the chosen line as written in the file, and lines with a format mistake are red and underlined |

The top row is a tab bar: the views and their keys, the shown one lit; click a tab to switch (`tab_bar = false` in the config file hides it).

Views 1 and 2 share the selected day, except that a view left with its popup open comes back on that day with the popup; a view left with the same day selected as now keeps its chosen item and scroll too.

A terminal at least 120 columns wide also shows the calendar list on the left; narrower, `s` calls it up.

## Command-line options
| Option | Meaning |
| :- | :- |
| `--lang en\|zh-TW` | Screen language. Default: the system language, else English |
| `--theme <name>` | Color theme: `catppuccin-mocha`, `gruvbox-dark`, `tokyo-night`, `dracula`, `nord-bright` (default), `one-dark`, `rose-pine` or `terminal` (the terminal's own 16 colors, for any background). Default: the config file's `theme` |
| `--tasks <path>` | Task file to show. Default: `~/.local/share/roh-cal/tasks.md` (`$XDG_DATA_HOME` respected); a missing default file just means no tasks |
| `--sample` | Show a fresh copy of the built-in sample task file, its dates moved to this week, written to `~/.cache/roh-cal/sample.md` (`$XDG_CACHE_HOME` respected) |
| `-h`, `--help` | Print the options |
| `-v`, `-V`, `--version` | Print the version |

## Keys
In roh-cal, `?` shows the most used keys and `Tab` there shows them all.

**Everywhere**

| Key | Action |
| :- | :- |
| `1` / `2` / `3` | Switch to the calendar + agenda / the month grid / the tasks page; the month grid opens with the selected week on the 2nd row. The tab bar at the top lists them, the shown one lit; clicking a tab switches too |
| `Tab` / `Shift+Tab` | Switch to the next / previous pane: the calendar, the agenda and the calendar list when it is on screen (the focused pane's border lights up); does nothing while layout 2's day popup is open |
| `s` | Hide / show the calendar list on the left (the task file's `#` groups) when the terminal is at least 120 columns wide; narrower, `s` calls it up over the left of the screen with the keyboard in it, and `s`, `Esc`, `q` or clicking outside put it away, back to the pane you were in |
| `.` | Hide / show done tasks everywhere (not their open subtasks; the tasks page keeps a done task with one under it), like the calendar list's `Hide done` switch; saved to the config file right away |
| `u` | Undo roh-cal's last write to the task file (up to 50; only if the file hasn't changed elsewhere since, otherwise it just reloads) |
| `?` | Help: where the task file and config file are (symlinks followed), then the keys for where the keyboard is (the calendar, the agenda, the tasks page…); `?` again or `Tab` shows all keys, the ones that don't work there in gray (`Tab` goes back, a third `?` closes it). `j` `k` or the wheel scroll it when the terminal is too short; `q`, `Esc` or clicking outside close it |
| `,` | Settings: the config file's settings (below) in a list; `j` `k` choose a row, `h` `l` (or `Space`, `Enter`, clicking the chosen row) change it (the theme row goes through the built-in themes, `width` one cell at a time, a color row through the theme's palette) and the screen follows right away; settings that change nothing on screen right now are dimmed (another view's, `width` while the calendar list is hidden, `by_calendar` while the day popup is closed), with a line saying where they are used; `[colors]` lists 10 colors at a time and scrolls to the chosen one, and a short terminal scrolls the whole list; `e` on a color row opens a color picker for any color (`j` `k` choose red, green or blue, `h` `l` change it by 1 and `H` `L` by 16, or type a color code such as `a6e3a1`; the screen follows as you go, `Enter` keeps it, `Esc` puts the old one back; with the mouse, click a bar or turn the wheel); `r` resets the row, `R` then `y` resets them all (and the intro plays again on the next start); `q`, `Esc`, `,` or clicking outside close it and save to the config file (only the changed lines; comments stay) |
| `Esc` | Back to the layout's main pane (the calendar, or the tasks page) from the agenda, layout 2's day popup or the calendar list; from the tasks page to layout 1; quit from the calendar |
| `q` / `Ctrl+C` | Quit (`q` in layout 2's day popup or the narrow calendar list only closes it) |

**On the calendar** (layouts 1 and 2)

| Key | Action |
| :- | :- |
| `h` `j` `k` `l` / arrows | Previous day / next week / previous week / next day |
| `[` / `]` | First day of the previous / next month |
| `t` | Jump to today; layout 2 shows its week on the 2nd row |
| `Enter` | Open the selected day's items in a popup over the calendar (both layouts): all of that day's items (long titles wrap), the ones hidden behind `+N` too, where the item keys below work, and `c` (or clicking the switch at the right end of the bottom border) lists them under their calendars or in one list, saved to the config file right away (`Esc`, `q` or clicking outside close it). With the mouse: double-click a day. `Tab` goes into layout 1's agenda instead |
| `a` | Add an item on the selected day, in the calendar of that day's first item (or the first calendar that is not a holiday calendar when the day has none); the editor shows the calendar under the line (Calendar); to pick another one, type `#` and part of its name (see `a` / `A` below). With no calendar in the file yet, it goes into a new `# Default` calendar (`# 預設` in Chinese) |

**On an item** (the agenda, layout 2's day popup, the tasks page)

| Key | Action |
| :- | :- |
| `j` `k` / `↓` `↑` | Choose the next / previous item |
| `Space` / `x` | Check off the chosen task, or uncheck it (written back to the task file; an event has no box) |
| `/` | Mark the chosen task in progress (`[/]`: its title turns bold with 🚧 in front, and it comes first on its day in the calendar views), or back to open; `Space` checks it off as usual |
| `e` / `Enter` | Edit the chosen item's line in a one-line editor that shows, as you type, how the line will be read field by field, only the fields it has: title, date, priority, calendar (a priority, `!` `!!` `!!!`, colors the line and its field right away; a mistake gets a red line of its own; an empty title is left blank and only called out once `Enter` cannot save it), and below that what can be typed: a date, a time, a priority, `#` and a calendar's name; `Enter` saves (not while the line has a mistake), `Esc` or clicking outside cancels. Inside: `←` `→` `Home` `End` move, `Ctrl+U` clears. On the calendar views an item on the selected day shows without its date (a time as `@10:00`), and the date is put back when saving; a multi-day item shows its whole range. The tasks page shows the whole line, so take a date away there. The fields under the line name the item's calendar; to move the item (with its subtasks) to another one, type `#` and part of its name, as when adding |
| `a` / `A` | Add an item to the chosen item's calendar in the same one-line editor, on the selected day (the date is added when saving; type your own `@2026-10-20` for another day, `@10:00` for a time; the tasks page adds no date) / add a subtask under the chosen item. To put the item in another calendar, type `#` and part of the calendar's name: a list of the calendars that match opens under it (fuzzy, like fzy: `#sf` finds `Work > SFAP`); `↑` `↓` (`Ctrl+P` `Ctrl+N`) or the wheel choose, `Enter`, `Tab` or a click picks one (the `#` word is taken out of the line), `Esc` closes the list and leaves the word as it is (e.g. an Obsidian `#tag`) |
| `r` | Reschedule the chosen item (the tasks page borrows layout 1's calendar; an undated item gets the day you pick). The calendar becomes the date picker (`hjkl`, `[` `]`, `t`, mouse); `Enter` (or clicking the same day again) moves the item by as many days as you moved, `Esc` cancels. A multi-day item moves as a whole |
| `d` | Delete the chosen item with its subtasks, after asking on the bottom line (`d` again or `y` deletes, so `dd` works like in vim; any other key cancels) |

**On the tasks page**

| Key | Action |
| :- | :- |
| `gg` / `G` | Choose the first / last item |
| `j` `k` on a heading | `j` `k` stop on each calendar's heading row too (an empty calendar is listed as one); `a` there adds an item at the end of that calendar |
| `l` / `h` (`→` / `←`) | Expand / collapse the chosen item's subtasks, or on a calendar's heading row that calendar (collapsed, it shows how many items are inside, its calendars included); `l` again goes to the first row inside, `h` again to the parent (from a top-level item, to its calendar's heading); click `▸` to do the same |
| `Shift+→` / `Shift+←` | Expand / collapse the chosen item's or calendar's whole subtree, every level under it; on an item without subtasks, `Shift+←` closes the one it is under. Right-click `▸` to do the same |
| `=` / `-` | Expand / collapse everything, calendars too: after `-` every calendar is listed and its items are not (a calendar with calendars inside stays open to show them, so its own items still show), and the chosen row becomes the calendar it was in; `+` works as `=` |
| `L` / `H` | Indent the chosen item with its subtasks under the item above it at the same level (2 more spaces than that item, like `A`) / outdent it to its parent's level, where the items below it at the same level become its subtasks (as in Obsidian); written back to the task file |
| `J` / `K` | Move the chosen item with its subtasks below / above the next item at the same level (it stays under the same parent and calendar); written back to the task file |
| `[` / `]` | Jump to the heading row of the previous / next calendar |
| `o` | Open the task file in your editor on the chosen line: `$VISUAL`, else `$EDITOR` (e.g. `export EDITOR=nvim`), else `vi`; vi, vim, nvim, nano, emacs, micro and kak start on that line. roh-cal comes back when the editor quits, with the changes shown |

**In the calendar list**

Archived calendars (`archived:<date>` in the heading) are listed last, under an 「Archived」 rule, and scroll on their own; the calendars above that rule take at most 30 rows.

| Key | Action |
| :- | :- |
| `j` `k` / `↓` `↑` | Choose the next / previous calendar |
| `gg` / `G` | Choose the 「Hide all」 switch at the top of the list / the last calendar |
| `[` / `]` | Choose the previous / next top-level calendar (in the archive part: one at its left), skipping the calendars inside them; `[` past the first one goes up to 「Hide all」 (here they do not change the month) |
| `Space` | Check / uncheck the chosen calendar together with the calendars inside it; an unchecked calendar's items are hidden. Each calendar follows only its own check, so one checked inside an unchecked one still shows |
| `Enter` | Show only the chosen calendar: it and the calendars inside it are checked (as with `Space`), every other one unchecked. `Enter` on it again puts every check back as it was before; `Enter` on another calendar meanwhile shows only that one instead |
| `k` past the first calendar | The two switches at the top of the list; `Space`, `Enter` or a click switches them. 「Hide all」 unchecks every calendar (it is on while they all are unchecked; switching it off checks them all again); 「Hide done」 hides done tasks everywhere (not their open subtasks; the tasks page keeps a done task with one under it), saved to the config file right away |
| `a` | Add a calendar: type its name in the one-line editor (`type:holiday` in it makes a holiday calendar, `archived:<date>` an archived one); it is written as a `# name` heading at the end of the task file. A name another top-level calendar already has cannot be saved. To put it inside another calendar, type `#` and part of that calendar's name and pick it from the list (as for items; 「Top level」 is first) |
| `A` | Add a calendar inside the chosen one: its heading gets one `#` more and goes at the end of the chosen calendar (after its items and the calendars inside it); the editor shows where it goes under the name (Inside). A name another calendar there already has cannot be saved |
| `e` | Rename the chosen calendar in the same editor: it starts with the heading's text (`type:holiday` included, so a holiday calendar stays one unless you take it out; typing or taking out `archived:<date>` archives it or brings it back, unchecked / checked like `z`); every heading of that calendar is rewritten, each keeping its `#` level. Ungrouped items have no heading to rename. Type `#` and part of another calendar's name to move it inside that one (「Top level」 to move it out): its headings, items and the calendars inside it go to the end of that calendar on save, the levels following along; `u` moves it back |
| `d` | Delete the chosen calendar: its headings and everything under them, the calendars inside it too, up to the next heading with as many `#` or fewer. The bottom row asks first and says how many items go with it; only `d` again (`dd`) or `y` deletes, `u` brings it back |
| `L` / `H` | Move the chosen calendar one level in (inside the calendar right above it at the same level) / out, with the calendars inside it: their headings get one `#` more / fewer |
| `z` | Archive the chosen calendar: ` archived:<today>` goes at the end of its headings, and it moves to the archive part at the bottom, unchecked. On an archived one: bring it back (the tag comes out; one archived together with the calendar it is in comes back with that one). `u` undoes it |

**Mouse**

| Action | Effect |
| :- | :- |
| Wheel | Move one week (in the agenda, the day popup and on the tasks page: one item, in the calendar list: one calendar); on the month grid (`2`) it only scrolls the weeks and the selected day stays, until a key moves it back into view |
| Click | Select a day, an item or a calendar, or a view in the tab bar at the top; double-click a day to go into its items, a task to check it off (a single click never checks one off); click a selected calendar again to check / uncheck it. Hold `Shift` to select text in the terminal |
| Right-click | On a tasks page item's `▸`: its whole subtree, every level under it. On a calendar in the calendar list: chooses it and does what `Enter` does there (only that calendar; on the one shown alone, every check back) |
| Drag | In the day popup: marks the item titles it goes over (color: `text_selection`); letting go copies that text to the clipboard, one item per line, and the bottom right says how many characters. The calendar list's right border: sets its width, which stays fixed (saved to the config file on letting go); double-click that border, or `r` on its settings row, to go back to auto |

## Config file
`~/.config/roh-cal/config.toml` (`$XDG_CONFIG_HOME` respected). It is optional: write only what you want to change, the rest keeps its default. A misspelt name or a wrong type stops roh-cal at start and says what is wrong (the line, or for `[colors]` the role and value). The settings page (`,`) edits it for you. Example: [testdata/config.toml](testdata/config.toml).

| Name | Default | Meaning |
| :- | :- | :- |
| `theme` | `"nord-bright"` | Color theme, same names as `--theme` (the built-in ones are in [themes/](themes/)) |
| `cross_out_done` | `true` | Strike through done tasks' titles; off, only their gray color sets them apart |
| `tab_bar` | `true` | The tab bar on the top row (the views and their keys); off, the views take that row too |
| `[calendar]` `relative_weeks` | `false` | Week column: the selected week shows its number, the other weeks how far away they are |
| `[calendar]` `dots` | `true` | Dots right of the dates in layout 1's calendar, one per item that day (up to 4), in the color of its most urgent open item |
| `[month]` `grid_lines` | `true` | Faint lines between the cells of the month grid (layout 2); without them only a month's edge is drawn |
| `[month]` `center_cursor` | `false` | Keep the selected row in the middle of the month grid's six rows (off: scroll a row at a time when the selection leaves them) |
| `[month]` `overflow` | `true` | A title too long for its cell runs into the empty cells to its right; the selected day's also into an empty line next to other items; off: cut at its own cell |
| `[calendar_list]` `width` | auto | The calendar list's width on the left, in cells (22 to 60); left out, it follows the longest calendar name from 22 up to 38, longer names are cut; `r` on its settings row puts it back to auto |
| `[calendar_list]` `hide_done` | `false` | Hide done tasks everywhere (not their open subtasks; the tasks page keeps a done task with one under it); `.` switches it too |
| `[day_popup]` `by_calendar` | `true` | The day popup lists the items under their calendars, as a tree like the tasks page; when off, one list with each item's outermost calendar at its right end; `c` in the popup switches it too |
| `[colors]` *role* | the theme's | Override one role's color, e.g. `today = "#a6e3a1"`: a color (`#rrggbb`, a terminal color name such as `light-blue` or `LightBlue`, or a 256-color number) or a name from the theme's palette. The roles are the names under `[roles]` in [themes/](themes/); `background` is the background the theme is made for (roh-cal does not paint it), which the tab bar's glow fades into: set it to your terminal's if they differ |

## Themes
Built in: `catppuccin-mocha`, `gruvbox-dark`, `tokyo-night`, `dracula`, `nord-bright` (default: Nord, lighter and more colorful), `one-dark`, `rose-pine`, and `terminal`, which only uses the terminal's own 16 colors so it follows your terminal's color scheme. Pick one with `--theme`, the `theme` setting or the settings page; override single colors with `[colors]`. Each theme is a small TOML file in [themes/](themes/): a `[palette]` of named colors and the `[roles]` that use them.

<!-- Screenshots of each theme: add them here once recorded -->

## Contributing
Bug reports and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build, test and propose changes, and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (in Traditional Chinese) for the modules, the layout model and how the screen tests work.

## License
[MIT](LICENSE)
