# Changelog
All notable changes to roh-cal are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-19
The first release: v2, a rewrite from scratch. The first version (never released) lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

### Added
- Calendar + agenda view (`1`): the calendar scrolls through the months, with a dot right of each date for each of that day's items (up to 4) and a multi-day item underlined as one line across its days (in the `range_overlap` color where two or more overlap); the agenda lists each month's items next to it in one column (a busy month takes at most 2/3 of the height and 16 rows, and its items scroll inside it)
- Month grid (`2`): six rows of weeks with each day's items in its cell, a multi-day item as one bar across its days, a staircase line along each month's edge; `Enter` or a double click opens a popup with all of the day's items, each with its outermost calendar at the right, the chosen item's whole calendar path on its bottom border, and the day's tasks open / done on its top border (the open count colored by how much is left)
- Tasks page (`3`): the whole task file as a tree by calendar, nested like the calendar list, undated items and empty calendars too; a calendar's heading row can be chosen, and `a` there adds an item to it; expand and collapse (`l` `h`, a whole subtree `Shift+→` `Shift+←`, everything `=` `-`), indent and outdent (`L` `H`), reorder (`J` `K`), first / last (`gg` `G`); format mistakes are marked
- Writing back to the task file: check off, add (into any calendar: type `#` and part of its name in the editor to pick it from a fuzzy list), add a subtask, edit (`#` moves the item to another calendar, with its subtasks), reschedule with the calendar as the date picker, delete (`dd`, or `d` then `y`), and undo the last 50 writes; only the changed lines are rewritten, through a temp file, and a file changed elsewhere is reloaded instead of overwritten
- One-line editor in the middle of the view: as you type, it lists how the line will be read, one field per row (title, date, priority, calendar), a mistake on its own row in red, and a hint row with what can be typed (`@date`, `@10:00`, `!` `!!` `!!!`, `#name`); that syntax is in italics in the typed line; a priority or date typed after a block id is saved in front of it, so the id stays at the end of the line
- Live reload: changes saved to the task file elsewhere (another editor, a script) show up in every view within a second
- Task file format v0.2 and v0.3: events (items without a box), multi-day ranges `@2026-10-20..2026-10-25`, holiday calendars (`type:holiday`), Obsidian block ids at the end of a line, calendars nested by heading level (`# Work` → `## SFAP`)
- In progress (task file format v0.5): `- [/]`, as in Obsidian's Tasks plugin, switched with `/`; its title is bold with 🚧 in front in every view, and it comes first on its day in the agenda and the month grid; the tasks page shows `[/]` in the box
- Priorities (task file format v0.4): `!`, `!!` or `!!!` in an item; the editor lists it in its color as you type. An item not over yet is colored by it (yellow, orange, red; the roles `priority_low`, `priority_medium`, `priority_high`): its whole line in the agenda and the month grid, its title on the tasks page
- Calendar list on the left: show or hide each `#` group (checking or unchecking one does the same to the calendars inside it; each shows by its own check); add (`a`), add inside the chosen one (`A`), rename (`e`), delete (`dd`), move in / out (`L` `H`); `#` in its editor picks the calendar to put it in (a rename moves it there with everything inside); first / last (`gg` `G`); 「Hide all」 and 「Hide done」 switches at the top; on terminals narrower than 120 columns `s` calls it up over the screen; long lists scroll
- Help page (`?`) with the keys for where the keyboard is, settings page (`,`) and the config file `config.toml`; a color picker (`e` on a color row) for any color, not only the theme's palette
- Eight built-in themes (`nord` by default) as TOML files (`catppuccin-mocha`, `gruvbox-dark`, `tokyo-night`, `dracula`, `nord`, `one-dark`, `rose-pine`, `terminal`) and per-role color overrides
- English and Traditional Chinese
- Mouse support: the wheel and clicks work in every view; a double click opens a day or checks off a task
- `--sample` shows a fresh copy of the built-in sample task file, its dates moved to this week, so it can be tried right after `cargo install`
- `o` on the tasks page opens the task file in your editor (`$VISUAL` / `$EDITOR`, else `vi`) on the chosen line

[Unreleased]: https://github.com/RohimChou/roh-cal/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/RohimChou/roh-cal/releases/tag/v0.1.0
