# Changelog
All notable changes to roh-cal are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
v2, a rewrite from scratch. The first version (never released) lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

### Added
- Calendar + agenda view (`1`): the calendar scrolls through the months, the agenda lists each month's items next to it
- Month grid (`3`): six rows of weeks with each day's items in its cell; every month starts a new row and a thick line marks a new year, like the calendar in `1`; `Enter` or a double click opens a popup with all of the day's items
- Tasks page (`4`): the whole task file as a tree by calendar, undated items too; expand and collapse (`l` `h`, `+` `-`), indent and outdent (`L` `H`), reorder (`J` `K`); format mistakes are marked
- Writing back to the task file: check off, add, add a subtask, edit in a one-line editor, reschedule with the calendar as the date picker, delete, and undo the last 50 writes; only the changed lines are rewritten, through a temp file, and a file changed elsewhere is reloaded instead of overwritten
- Task file format v0.2: events (items without a box), multi-day ranges `@2026-10-20..2026-10-25`, holiday calendars (`type:holiday`), Obsidian block ids at the end of a line
- Calendar list on the left: show or hide each `#` group; on terminals narrower than 120 columns `s` calls it up over the screen; long lists scroll
- Help page (`?`), settings page (`,`) and the config file `config-v2.toml`
- Six built-in themes as TOML files (`catppuccin-mocha`, `gruvbox-dark`, `tokyo-night`, `dracula`, `nord`, `terminal`) and per-role color overrides
- English and Traditional Chinese
- Mouse support: the wheel and clicks work in every view

[Unreleased]: https://github.com/RohimChou/roh-cal/commits/v2
