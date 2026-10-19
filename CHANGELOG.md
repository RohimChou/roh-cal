# Changelog
All notable changes to roh-cal are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
v2, a rewrite from scratch. The first version (never released) lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

### Added
- Calendar + agenda view (`1`): the calendar scrolls through the months, the agenda lists each month's items next to it
- Month grid (`2`): six rows of weeks with each day's items in its cell, a multi-day item as one bar across its days, a staircase line along each month's edge; `Enter` or a double click opens a popup with all of the day's items
- Tasks page (`3`): the whole task file as a tree by calendar, undated items too; expand and collapse (`l` `h`, `+` `-`), indent and outdent (`L` `H`), reorder (`J` `K`); format mistakes are marked
- Writing back to the task file: check off, add (into any calendar: type `#` and part of its name in the editor to pick it from a fuzzy list), add a subtask, edit in a one-line editor (`#` moves the item to another calendar, with its subtasks), reschedule with the calendar as the date picker, delete, and undo the last 50 writes; only the changed lines are rewritten, through a temp file, and a file changed elsewhere is reloaded instead of overwritten
- Live reload: changes saved to the task file elsewhere (another editor, a script) show up in every view within a second
- Task file format v0.2: events (items without a box), multi-day ranges `@2026-10-20..2026-10-25`, holiday calendars (`type:holiday`), Obsidian block ids at the end of a line
- In progress (task file format v0.5): `- [/]`, as in Obsidian's Tasks plugin; its title is bold in every view, and the tasks page shows `[/]` in the box
- Priorities (task file format v0.4): `!`, `!!` or `!!!` in an item; the editor's check row says which one it read. An item not over yet is colored by it (yellow, orange, red; the roles `priority_low`, `priority_medium`, `priority_high`): its whole line in the agenda and the month grid, its title on the tasks page
- Calendar list on the left: show or hide each `#` group; on terminals narrower than 120 columns `s` calls it up over the screen; long lists scroll
- Help page (`?`) with the keys for where the keyboard is, settings page (`,`) and the config file `config.toml`; a color picker (`e` on a color row) for any color, not only the theme's palette
- Eight built-in themes as TOML files (`catppuccin-mocha`, `gruvbox-dark`, `tokyo-night`, `dracula`, `nord`, `one-dark`, `rose-pine`, `terminal`) and per-role color overrides
- English and Traditional Chinese
- Mouse support: the wheel and clicks work in every view

[Unreleased]: https://github.com/RohimChou/roh-cal/commits/v2
