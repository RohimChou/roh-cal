# Changelog
All notable changes to roh-cal are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
v2, a rewrite from scratch. The first version (never released) lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

### Added
- Calendar + agenda view (`1`): the calendar scrolls through the months, the agenda lists each month's items next to it
- Month grid (`2`): six rows of weeks with each day's items in its cell, a multi-day item as one bar across its days, a staircase line along each month's edge; `Enter` or a double click opens a popup with all of the day's items
- Tasks page (`3`): the whole task file as a tree by calendar, undated items too; expand and collapse (`l` `h`, `+` `-`), indent and outdent (`L` `H`), reorder (`J` `K`); format mistakes are marked
- Writing back to the task file: check off, add, add a subtask, edit in a one-line editor, reschedule with the calendar as the date picker, delete, and undo the last 50 writes; only the changed lines are rewritten, through a temp file, and a file changed elsewhere is reloaded instead of overwritten
- Task file format v0.2: events (items without a box), multi-day ranges `@2026-10-20..2026-10-25`, holiday calendars (`type:holiday`), Obsidian block ids at the end of a line
- Priorities (task file format v0.4): `!`, `!!` or `!!!` in an item; the editor's check row says which one it read. An item not over yet is colored by it (yellow, orange, red; the roles `priority_low`, `priority_medium`, `priority_high`): its whole line in the agenda and the month grid, its title on the tasks page
- Overdue: an unfinished task past its day shows how late it is before its title (`3d late` / `過期3天`, role `overdue`) in the agenda, the day popup and the tasks page
- Calendar list on the left: show or hide each `#` group; on terminals narrower than 120 columns `s` calls it up over the screen; long lists scroll
- Help page (`?`) with the keys for where the keyboard is, settings page (`,`) and the config file `config.toml`; a color picker (`e` on a color row) for any color, not only the theme's palette
- Six built-in themes as TOML files (`catppuccin-mocha`, `gruvbox-dark`, `tokyo-night`, `dracula`, `nord`, `terminal`) and per-role color overrides
- English and Traditional Chinese
- Mouse support: the wheel and clicks work in every view

[Unreleased]: https://github.com/RohimChou/roh-cal/commits/v2
