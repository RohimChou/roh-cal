# Task file format (spec v0.3)
One plain-text file, **one item per line**, written with Markdown list syntax. An item with a checkbox is a **task** you check off; one without is an **event** (a meeting, a holiday, a release date) that is simply over once its day has passed. The file is meant to be hand-edited in any editor, diffed in git and searched with `grep`; roh-cal reads it and writes it back (see [Writing](#writing)).

```markdown
# Payments API
- [ ] Estimate the v2.3 tickets @2026-10-12
- [ ] Follow up with Sarah on the retry policy @2026-10-12 14:00
- [x] Fix duplicate rows in the CSV export @2026-10-06
  - [x] Found the cause: the query was missing a DISTINCT
- Code freeze @2026-10-19

# Personal
- [ ] Pay the credit card bill @2026-10-25
- Dentist appointment @2026-10-14 10:30
- [ ] Organize the bookshelf

# US holidays type:holiday
- Thanksgiving break @2026-11-26..2026-11-29

Notes, blank lines and anything else stay untouched.
```

A longer example: [`testdata/tasks.md`](../testdata/tasks.md).

## Rules
| # | Line | Meaning |
| :- | :- | :- |
| 1 | `- [ ] text` / `- [x] text` | **Task**, open / done (`[X]` also counts as done) |
| 2 | `- text` (no checkbox) | **Event**: an appointment, a holiday, a milestone. It cannot be checked off; once its day has passed it is over (shown grayed out, not crossed out) |
| 3 | `@YYYY-MM-DD` or `@YYYY-MM-DD HH:MM` anywhere in an item | The day (and 24-hour time) the item appears on the calendar. No date → the item is **unscheduled** |
| 3b | `@YYYY-MM-DD..YYYY-MM-DD` (optionally followed by `HH:MM`) | A **multi-day** item, from the first date to the last, both included (a trip, a training course). The time is the start time on the first day |
| 4 | `# Name` (any heading level) | Starts a **group** (project, calendar); applies to the items below until the next heading. A heading with no items under it is still a group (an empty calendar). Groups **nest by heading level**: a heading is inside the nearest heading above it with fewer `#` (`# Payments API` → `## Refunds`). A group is its whole path: `Work > SFAP` and `Personal > SFAP` are two groups, while a heading that repeats the same path (`# Work` again further down) continues the same group. Hiding a group hides the groups inside it |
| 4b | `type:holiday` anywhere in a heading | The group is a **holiday calendar**: the days of its items (every day of a multi-day item) are holidays, drawn like a weekend (a day off). The token is not part of the group's name. Write holidays as events (no box), e.g. `- Thanksgiving break @2026-11-26..2026-11-29` |
| 5 | An item indented deeper than the previous item | **Child** of that item (subtask) |

Everything else (blank lines, prose, code blocks, other Markdown) is ignored when reading and kept byte-for-byte when writing. So is front matter: when the first line is `---`, everything up to the next `---` (an Obsidian note's properties, e.g. a `tags:` list) is skipped.

## Details
| Topic | Rule |
| :- | :- |
| Item marker | Only `- ` starts an item (not `*` / `+`), so there is one way to write it |
| Date token | `@` + a valid date or range, separated by whitespace. The **first** date token is used; it is removed from the displayed title. Convention: put it at the end of the line |
| Not a date | `@` followed by anything that is not `YYYY-MM-DD` stays plain text (e.g. `@Kevin`). A date-shaped but invalid token (`@2026-13-01`) also stays text, and the parser reports a warning: roh-cal says how many there are when it opens, and the tasks page (`3`) marks those lines in red and says what is wrong. So does a range whose end is before its start (`@2026-10-25..2026-10-20`); a range from a day to the same day is just that day |
| Items without a date | Unscheduled; not shown on the calendar, but listed on the tasks page (`3`) |
| Children | Shown on the calendar only if they have their own date; otherwise they belong to the parent |
| Indentation | Any amount of spaces or tabs; deeper than the parent = child |
| Encoding | UTF-8; LF or CRLF, preserved on write |
| Block id | An Obsidian block id at the very end of an item (`^refund-01`: `^` + letters, digits, `-`) is not part of the title; it stays in the file so links to the line keep working |
| Reserved syntax | `key:value` (lowercase key, no spaces) is reserved for future fields such as `due:2026-10-20`. Only keys defined by the spec are interpreted; so far that is only `type:holiday` in a heading (rule 4b), so URLs like `https://...` are just text |

## Where it is stored
Default `~/.local/share/roh-cal/tasks.md` (XDG data dir; `$XDG_DATA_HOME` respected). Point roh-cal at your own file — e.g. inside a notes vault or a git repo — with `--tasks <path>`.

## Writing

roh-cal writes the file when you check an item off, reschedule, edit, add or delete one, indent, outdent or move one on the tasks page, and when you undo one of those. Every write follows these rules:

| Topic | Rule |
| :- | :- |
| What it writes | Toggle a task `[ ]` ↔ `[x]` (events have no box and are never toggled). Delete an item together with its block: the lines indented deeper below it (subtasks, notes, blank lines between them; not the blank lines after them), and further down any subtask the parser still counts as its own after a note line that is not indented. Add an item: the typed line goes after the last item (and its subtasks) of the chosen item's group; from the calendar, the group of the selected day's first item, or the end of the file when that day has none. A subtask goes right after its parent and the parent's subtasks, indented two spaces deeper. A missing file is created. When the file has no heading yet, a new item (not a subtask) goes into a new `# Default` calendar (`# 預設` with the Chinese interface) at the end, written together with it. Add a calendar: a `# name` heading becomes the file's last line, after a blank line. Rename a calendar: each of its heading lines (headings with the same name are one calendar) becomes its own `#`s plus the typed text. Delete a calendar: each of its headings goes together with every line under it, the calendars inside it too, up to the next heading with as many `#` or fewer (a `#` line inside a code block is not a heading) or the end of the file. Move a calendar in / out: its headings and those of the calendars inside it get one `#` more / fewer (in only below a calendar at the same level, at most six `#`; out only from two `#` or more). Edit an item: the line becomes exactly what was typed in the one-line editor (its indentation kept); an edit that changes nothing leaves the line as it was. Reschedule an item: only its date token changes (the time stays; a multi-day range keeps its length; an unscheduled item gets ` @YYYY-MM-DD` at the end, before a trailing block id). Indent an item (`L`): its block moves under the item above it at the same level, two spaces deeper than that item; outdent (`H`): its block lines up with its parent, and the items below it at the same level become its subtasks (as in Obsidian); deeper lines in the block keep their extra indent. Move an item (`J` / `K`): its block swaps places with the next / previous block at the same level under the same heading; blank lines between them stay where they are, and each place keeps its indent |
| Checked before saving | The editor shows, while typing, how the line will be read (task or event, date, title). It refuses to save a line that is not one item (`- ` first), that has no title, or that has a mistake such as `@2026-13-01`, so nothing silently loses its date |
| Identity | No IDs. roh-cal remembers each item's line number and original text; before writing it re-reads the file, and if that line changed it reloads instead of overwriting |
| Only those lines | Only the changed line (or the deleted, re-indented or moved blocks) is touched; every other byte stays the same |
| Safe write | Write to a temp file in the same folder, then rename, so a crash never leaves a half-written file |
| Undo | roh-cal keeps the whole file from before each of its writes (the last 50). `u` puts it back only if the file is still exactly what roh-cal wrote; otherwise it reloads and forgets that history |

## Not in v0
Recurring items, due vs scheduled dates, priorities, inline `#tags`, time ranges within a day, reminders, multiple task files. Each can be added later as a `key:value` field or a new rule without breaking existing files.

Planned to revisit:
- **Completion dates** when checking a task off (e.g. `done:2026-10-11`), useful for showing what was finished on each day

## Borrowed from
| Idea | From |
| :- | :- |
| One item per line, tiny spec, `key:value` extensions | [todo.txt](https://github.com/todotxt/todo.txt) |
| Projects as headings + indentation instead of repeating `+project` on every line; `@` marks metadata | [TaskPaper](https://www.taskpaper.com) |
| `- [ ]` / `- [x]` checkboxes | GitHub-flavored Markdown (renders in GitHub, Obsidian, most editors) |

## History
- v0 (2026-10-11): first version
- v0.1 (same day): no separate "event" kind; every item can be checked off
- v0.2 (2026-10-18): an item without a checkbox is an event again (rule 2), but without an extra color: an event still to come is shown like an open task, a past one is grayed out
- v0.2 (same day): multi-day items `@first..last` (rule 3b)
- v0.2 (same day): holiday calendars, `type:holiday` in a heading (rule 4b)
- v0.2 (same day): a trailing Obsidian block id is not part of the title
- v0.3 (same day): groups nest by heading level, and a group is its whole path (rule 4)
