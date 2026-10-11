# Task file format (spec v0.1)
One plain-text file, **one item per line**, written with Markdown list syntax. Every item can be checked off. The file is meant to be hand-edited in any editor, diffed in git and searched with `grep`; roh-cal reads it and writes it back (see [Writing](#writing)).

```markdown
# SFAP
- [ ] 估 2601RC 人天 @2026-10-12
- [ ] 追蹤紫儀：利率要從哪抓 @2026-10-12 14:00
- [x] 修正截斷問題 @2026-10-06
  - [x] 找到原因：SQL 少一個 DISTINCT
- 進 SIT @2026-10-19

# 個人
- [ ] 繳信用卡 @2026-10-25
- 看牙醫 @2026-10-14 10:30
- [ ] 整理書櫃

Notes, blank lines and anything else stay untouched.
```

A longer example: [`testdata/tasks.md`](../testdata/tasks.md).

## Rules
| # | Line | Meaning |
| :- | :- | :- |
| 1 | `- [ ] text` / `- [x] text` | **Item**, open / done (`[X]` also counts as done) |
| 2 | `- text` (no checkbox) | Also an **open item**, just written without the box (an appointment, a release date…). Checking it off writes `- [x] text` |
| 3 | `@YYYY-MM-DD` or `@YYYY-MM-DD HH:MM` anywhere in an item | The day (and 24-hour time) the item appears on the calendar. No date → the item is **unscheduled** |
| 4 | `# Name` (any heading level) | Starts a **group** (project); applies to the items below until the next heading. Groups do not nest in v0, but the parser keeps each heading's level so nesting (`# SFAP` → `## 11RC`) can be added later without changing files |
| 5 | An item indented deeper than the previous item | **Child** of that item (subtask) |

Everything else (blank lines, prose, code blocks, other Markdown) is ignored when reading and kept byte-for-byte when writing.

## Details
| Topic | Rule |
| :- | :- |
| Item marker | Only `- ` starts an item (not `*` / `+`), so there is one way to write it |
| Date token | `@` + a valid date, separated by whitespace. The **first** date token is used; it is removed from the displayed title. Convention: put it at the end of the line |
| Not a date | `@` followed by anything that is not `YYYY-MM-DD` stays plain text (e.g. `@Rukis`). A date-shaped but invalid token (`@2026-13-01`) also stays text, and the parser reports a warning (not shown on screen yet) |
| Items without a date | Unscheduled; not shown on the calendar (later: a list of their own) |
| Children | Shown on the calendar only if they have their own date; otherwise they belong to the parent |
| Indentation | Any amount of spaces or tabs; deeper than the parent = child |
| Encoding | UTF-8; LF or CRLF, preserved on write |
| Reserved syntax | `key:value` (lowercase key, no spaces) is reserved for future fields such as `due:2026-10-20`. Only keys defined by the spec are interpreted; v0 defines none, so URLs like `https://...` are just text |

## Where it is stored
Default `~/.local/share/roh-cal/tasks.md` (XDG data dir; `$XDG_DATA_HOME` respected). Point roh-cal at your own file — e.g. inside a notes vault or a git repo — with `--tasks <path>`.

## Writing
roh-cal writes the file when you check an item off, delete one or add one; editing items is planned. Every write follows these rules:

| Topic | Rule |
| :- | :- |
| What it writes | Toggle `[ ]` ↔ `[x]` (a plain `- text` becomes `- [x] text`). Delete an item together with the lines indented deeper below it (subtasks, notes, blank lines between them; not the blank lines after them). Add `- [ ] title @YYYY-MM-DD` after the last item (and its subtasks) of the selected item's group, or at the end of the file when the day has no items; a missing file is created. Edit an item: its line is rewritten as `- [ ] title @YYYY-MM-DD HH:MM` (indentation and checkbox kept, the date at the end, no time = no `HH:MM`); an edit that changes nothing leaves the line as it was |
| Identity | No IDs. roh-cal remembers each item's line number and original text; before writing it re-reads the file, and if that line changed it reloads instead of overwriting |
| Only those lines | Only the changed line (or the deleted block) is touched; every other byte stays the same |
| Safe write | Write to a temp file in the same folder, then rename, so a crash never leaves a half-written file |
| Undo | roh-cal keeps the whole file from before each of its writes (the last 50). `u` puts it back only if the file is still exactly what roh-cal wrote; otherwise it reloads and forgets that history |

## Not in v0
Recurring items, due vs scheduled dates, priorities, inline `#tags`, time ranges and multi-day events, reminders, multiple task files. Each can be added later as a `key:value` field or a new rule without breaking existing files.

Planned to revisit:
- **Nested groups** by heading level (`#` project → `##` sub-project)
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
