# roh-cal
A terminal calendar for a plain-text task file. Vim-style keys, a compact 8-week view and a big month view that lists each day's items. Built with Rust + [Ratatui](https://ratatui.rs/).

## Run
Needs a recent stable Rust (`rustup update`).

```sh
git clone https://github.com/RohimChou/roh-cal.git
cd roh-cal
cargo run -- --tasks testdata/tasks.md    # try it with the sample task file
cargo install --path .                    # then run `roh-cal` from anywhere
```

| Option | Meaning |
| :- | :- |
| `--tasks <path>` | Task file to show. Default: `~/.local/share/roh-cal/tasks.md` (`$XDG_DATA_HOME` respected); a missing default file just means no tasks |
| `--lang en\|zh-TW` | Screen language. Default: the system language, else English |

## Keys
| Key | Action |
| :- | :- |
| `h` `j` `k` `l` / arrows | Previous day / next week / previous week / next day |
| `t` | Jump to today |
| `Enter` / `o` | Open the selected day's list |
| `a` | Add an item on the selected day (opens the add form) |
| `u` | Undo roh-cal's last change to the task file (up to 50 steps) |
| `v` | Switch between the compact view and the month view |
| `Option+q` | Open / close the settings panel |
| `?` | Help: the main keys on one page (`?` / `q` / `Esc` closes it) |
| `q` / `Esc` | Quit |
| `Ctrl+C` | Quit from anywhere (a list, a form, the settings panel); unsaved form input is dropped, settings are saved |
| Mouse wheel / click | Move one week / select a day (hold `Shift` to select text in the terminal) |

In the day list:

| Key | Action |
| :- | :- |
| `j` / `k` | Pick an item |
| `Enter` / `o` / `e` | Edit the item (opens the form) |
| `a` | Add an item (opens a form) |
| `Space` / `x` | Check off / uncheck, saved to the task file right away |
| `d` | Delete the item and its subtasks (asks first: `y` = yes, any other key = no) |
| `u` | Undo |
| `?` | Help |
| `q` / `Esc` | Close |

In the add / edit form:

| Key | Action |
| :- | :- |
| `Tab` / `Shift+Tab` | Next / previous field (title, date, time) |
| `h` `j` `k` `l` / arrows (date field) | Previous day / next week / previous week / next day |
| `Enter` | Save and go back; the day list / calendar selects the new item |
| `Esc` / `q` | Cancel (in the title field `q` is just a letter) |

The date field shows a small calendar and takes digits and `-` only, e.g. `2026-10-12`. The time field is optional and takes digits and `:` only: `14:00`, `9:00`, `1400` or `930` (saved as `14:00`, `09:00`, `14:00`, `09:30`).

In the settings panel:

| Key | Action |
| :- | :- |
| `j` / `k` | Pick a row |
| `h` / `l`, `Space`, `Enter` | Change the value (toggle, or the previous / next color) |
| `r` | Reset this row to its default |
| `R` | Reset everything; press `y` to confirm, any other key cancels |
| `q` / `Esc` / `Option+q` | Close and save |

## Settings file
`~/.config/roh-cal/config.toml` (`$XDG_CONFIG_HOME` respected). Every key is optional; edit it by hand or with the settings panel. Saving from the panel rewrites only the lines whose value changed, so your comments and layout stay. A typo or an unknown key is an error shown before the calendar starts.

```toml
[calendar]               # both views
week_numbers = true      # left column: week number of the selected week, distance in weeks for the others

[month]                  # month view
grid_lines = false       # faint lines between the day cells
item_icons = true        # a dot before each item
center_cursor = true     # keep the selected week in the middle of the screen

[colors]                 # only the colors you changed, as #rrggbb
sunday = "#f38ba8"
```

Color names: `item`, `item_done`, `more_items`, `grid_lines`, `selected_bg`, `day`, `other_month`, `today`, `title`, `month_label`, `weekday_header`, `saturday`, `sunday`. The default palette is [Catppuccin Mocha](https://catppuccin.com/palette).

## Task file
A Markdown file, one item per line. It reads well in any editor, Obsidian or GitHub. Full rules: [docs/task-file-format.md](docs/task-file-format.md).

```markdown
# Work
- [ ] Code review @2026-10-12 14:00
- [x] Fix the migration @2026-10-08
  - [x] Find the cause

# Personal
- Dentist @2026-10-14 10:30
- [ ] Tidy the bookshelf
```

- `- [ ]` / `- [x]` = open / done; a plain `- text` is an open item too
- `@YYYY-MM-DD` (optionally `HH:MM`) puts the item on the calendar; no date = unscheduled
- `# Heading` starts a group; deeper indentation = a subtask
- Everything else is left alone

Checking an item off rewrites only that line; deleting one removes it together with the lines indented below it; adding one writes `- [ ] title @date time` after the last item of the selected item's group (or at the end of the file). If the file was changed elsewhere since roh-cal read it (e.g. in Obsidian), roh-cal reloads it instead of writing. `u` undoes roh-cal's changes one at a time, but never over changes made elsewhere.
