"""Compute the screens of layout 1 (side menu + calendar on the left + agenda on the right),
layout 3 (side menu + month grid) and the tasks page on their own, as the expected values
of the screen tests in `src/ui.rs`.

Written apart from the Rust code (weeks come from Python's `calendar`, week numbers use a different formula,
the task file is read with a simple regex); a result only counts as right when both sides agree. Do not copy
what Rust draws back in here.

Usage (from the repo root):
python3 tools/expected_screen.py zh|en WIDTH HEIGHT SELECTED_DAY [TOP_MONTH SKIP_ROWS] < TASK_FILE
(TOP_MONTH = YYYY-MM of the month block at the top; SKIP_ROWS = how many rows to skip)
Example: python3 tools/expected_screen.py zh 70 23 2026-10-17 < testdata/agenda.md
Layout 3 (env var LAYOUT=3):
python3 tools/expected_screen.py zh|en WIDTH HEIGHT SELECTED_DAY [TOP_ROW] < TASK_FILE
(A row is one week, but every month starts a new row. TOP_ROW = YYYY-MM-DD of the top row's first day.
Not given = just opened: the last row of the month before the selected one; scroll if the selected row is
not within the 6 rows)
Tasks page (env var LAYOUT=4):
python3 tools/expected_screen.py zh|en WIDTH HEIGHT SELECTED_DAY < TASK_FILE
(the tasks page does not use SELECTED_DAY, but it must still be given);
EXPANDED = line numbers of the items whose subtasks are expanded, comma-separated;
CHOSEN_LINE = line number of the chosen item (not given = the first item)
Env vars: CHOSEN = index of the chosen item in the sorted agenda (keyboard in the agenda);
WINDOW = how many rows the selected month's agenda is scrolled;
SIDE_HIDDEN = `s` was pressed and the side menu is hidden (the side menu only exists at 120+ cells wide);
NO_GRID = layout 3 draws no grid lines (config `[month] grid_lines = false`);
NO_ICONS = no dot before items in layout 3 (`item_icons = false`);
CENTER = layout 3 keeps the selected row on row 3 (`center_cursor = true`; the top-row argument is not needed);
RELATIVE = week-number column in layouts 1 and 3: the selected week shows its week number, other weeks show
how many weeks away they are (`[calendar] relative_weeks = true`);
HIDDEN = indexes of the unchecked calendars in the side menu, comma-separated (e.g. `1,3`); their items
are not shown; LAYOUT = 3 is layout 3, 4 is the tasks page
Today is always 2026-10-17.
"""
import calendar
import datetime
import os
import re
import sys
import unicodedata

TEXTS = {
    "en": dict(hints_tasks=" jk move  hl fold  Space check  e edit  a add  s calendars  Esc calendar  ? help  , settings  q quit",
               tasks=" Tasks", detail=lambda n, t: f" Line {n}  {t}", mistakes=lambda n: f"{n} mistake(s) ",
               mistake=lambda n, kind, token: f" Line {n}: {token}: " + {"date": "no such day", "time": "no such time", "range": "ends before it starts"}[kind],
               hints=" hjkl move  Tab pane  [] month  t today  s calendars  ? help  , settings  q quit",
               side=" Calendars", ungrouped="Ungrouped", hw=" Wk ",
               wd=["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"], items=" Agenda", more=lambda n: f"{n} more",
               year=lambda y: f"{y}", month=lambda m: calendar.month_abbr[m],
               date=lambda d: f"{d:%a %b} {d.day}, {d.year}"),
    "zh": dict(hints_tasks=" jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說明  , 設定  q 離開",
               tasks=" 任務", detail=lambda n, t: f" 第 {n} 行  {t}", mistakes=lambda n: f"{n} 處寫錯 ",
               mistake=lambda n, kind, token: f" 第 {n} 行：{token}：" + {"date": "沒有這一天", "time": "沒有這個時間", "range": "結束在開始之前"}[kind],
               hints=" hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  , 設定  q 離開",
               side=" 日曆", ungrouped="未分組", hw=" 週 ",
               wd=list("一二三四五六日"), items=" 重要事項", more=lambda n: f"還有 {n} 件",
               year=lambda y: f"{y}年", month=lambda m: f"{m}月",
               date=lambda d: f"{d.year}/{d.month}/{d.day}（{'一二三四五六日'[d.weekday()]}）"),
}
CHOSEN = int(os.environ["CHOSEN"]) if "CHOSEN" in os.environ else None
WINDOW = int(os.environ.get("WINDOW", "0"))
SIDE_HIDDEN = "SIDE_HIDDEN" in os.environ
NO_GRID = "NO_GRID" in os.environ
NO_ICONS = "NO_ICONS" in os.environ
CENTER = "CENTER" in os.environ
RELATIVE = "RELATIVE" in os.environ
HIDDEN = {int(i) for i in os.environ.get("HIDDEN", "").split(",") if i}
SIDE_MIN, SIDE_MAX = 22, 36  # side menu min / max width (borders included; the max is the config's default
                             # [calendar_list] max_width); width follows the longest calendar


def width(s):
    return sum(2 if unicodedata.east_asian_width(c) in "WF" else 1 for c in s)


def rjust(s, n):
    return " " * (n - width(s)) + s


def ljust(s, n):
    return s + " " * (n - width(s))


def cut(s, n):
    """At most n cells; a wide character that does not fit is left out entirely"""
    out = ""
    for c in s:
        if width(out + c) > n:
            break
        out += c
    return out


def week_number(d, selected=None):
    # A different formula from the Rust code: (Monday of this day's week − Monday of 1/1's week) divided by 7
    monday = lambda x: x - datetime.timedelta(days=x.weekday())
    if RELATIVE and selected and monday(d) != monday(selected):
        # Relative: how many weeks away from the selected week (before or after alike)
        return abs((monday(d) - monday(selected)).days) // 7
    return (monday(d) - monday(datetime.date(d.year, 1, 1))).days // 7 + 1


def read_items(text, hidden_names):
    """Simple version: lines that start with `- ` and end with `@YYYY-MM-DD` or `@YYYY-MM-DD..YYYY-MM-DD`
    (a time may follow). Lines whose calendar (the nearest `#` heading above) is unchecked are skipped.
    Returns (date, time, title, line index, last day of a multi-day item or None), sorted"""
    items = []
    name = None
    for i, line in enumerate(text.splitlines()):
        heading = re.match(r"#+ (.*)", line)
        if heading:
            # `type:holiday` is not part of the name
            name = " ".join(w for w in heading.group(1).split() if w != "type:holiday")
        m = re.match(r"\s*- (?:\[[ xX]\] )?(.*?) @(\d{4}-\d{2}-\d{2})(?:\.\.(\d{4}-\d{2}-\d{2}))?(?: (\d{2}:\d{2}))?$", line)
        if m and name not in hidden_names:
            end = datetime.date.fromisoformat(m.group(3)) if m.group(3) else None
            items.append((datetime.date.fromisoformat(m.group(2)), m.group(4), m.group(1), i, end))
    # By date, time (items without a time first), then file order
    items.sort(key=lambda it: (it[0], it[1] is not None, it[1] or "", it[3]))
    return items


def agenda_entries(items):
    """Each line of the agenda: (day it is listed on, whether it carries over from the previous month, the item).
    An item that crosses months gets one line in every month it touches; in the later months it is listed on
    the 1st, first in that day"""
    entries = []
    for it in items:
        entries.append((it[0], False, it))
        if it[4]:
            y, m = it[0].year, it[0].month
            while True:
                y, m = (y + 1, 1) if m == 12 else (y, m + 1)
                first_day = datetime.date(y, m, 1)
                if first_day > it[4]:
                    break
                entries.append((first_day, True, it))
    entries.sort(key=lambda e: (e[0], not e[1], e[2][1] is not None, e[2][1] or "", e[2][3]))
    return entries


def range_text(it):
    """The agenda's date: `10/5`, multi-day `10/20~25`, across months `10/28~11/3`"""
    start, end = it[0], it[4]
    d = f"{start.month}/{start.day}"
    if end and (end.year, end.month) == (start.year, start.month):
        d += f"~{end.day}"
    elif end:
        d += f"~{end.month}/{end.day}"
    return d


def read_groups(text):
    """Groups (the side menu's calendars) in order of first appearance, a heading with no items under it
    included; returns [(name (None for items before any heading), number of items with a date)]
    (items read with read_tree: items in code blocks do not count, and a day that does not exist is not
    a date)"""
    groups = {}  # dict keeps insertion order
    items = {it["line"]: it for it in read_tree(text)[0]}
    in_code = False
    for n, raw in enumerate(text.splitlines(), start=1):
        stripped = raw.lstrip()
        if stripped.startswith("```") or stripped.startswith("~~~"):
            in_code = not in_code
            continue
        heading = None if in_code else re.match(r"(#{1,6}) (.*)", raw)
        if heading:
            groups.setdefault(" ".join(w for w in heading.group(2).split() if w != "type:holiday"), 0)
        if n in items:
            it = items[n]
            name = it["group"][0] if it["group"] else None
            groups.setdefault(name, 0)
            groups[name] += it["date"] is not None
    return list(groups.items())


def side_width(t, groups):
    """side menu width (borders included): the longest row 「 ☑ name count 」 plus borders,
    between SIDE_MIN and SIDE_MAX"""
    longest = max((3 + width(t["ungrouped"] if name is None else name) + len(f" {count} ")
                   for name, count in groups), default=0)
    return min(max(longest + 2, SIDE_MIN), SIDE_MAX)


def side_lines(t, h, groups):
    """Each row of the side menu column (top and bottom borders included, the hints line not):
    one row per calendar 「 ☑ name    count 」"""
    iw = side_width(t, groups) - 2
    rows = [ljust(t["side"], iw), "─" * iw]
    for i, (name, count) in enumerate(groups):
        count = f" {count} "
        left = (" ☐ " if i in HIDDEN else " ☑ ") + (t["ungrouped"] if name is None else name)
        rows.append(ljust(cut(left, iw - len(count)), iw - len(count)) + count)
    rows = (rows + [" " * iw] * h)[: h - 3]
    return ["╭" + "─" * iw + "╮"] + ["│" + row + "│" for row in rows] + ["╰" + "─" * iw + "╯"]


def screen(lang, w, h, selected, first, skip, items, groups):
    t = TEXTS[lang]
    body_h = h - 5  # box top and bottom, header, header line, hints line
    sw = side_width(t, groups) if w >= 120 and not SIDE_HIDDEN else 0  # side menu width
    aw = w - sw - 35  # agenda box width (borders included)
    iw = max(aw - 2, 0)  # width inside the agenda box
    n = min(max((iw + 2) // 30, 1), 4)  # number of columns
    colw = (iw - 2 * (n - 1)) // n if iw else 0
    assert iw == 0 or colw * n + 2 * (n - 1) == iw, "pick a width that divides evenly"

    entries = agenda_entries(items)

    def item_text(e):
        _, time, title, _, _ = e[2]
        d = range_text(e[2])
        rest = (time + " " if time else "") + title
        if CHOSEN is not None and entries.index(e) == CHOSEN:
            s = " " * (5 - len(d)) + "[" + d + "]" + rest
        else:
            s = " " + rjust(d, 5) + " " + rest
        return ljust(cut(s, colw), colw)

    rule = lambda thick: ("━" * 10 + "┿" + "━" * 22) if thick else ("─" * 10 + "┼" + "─" * 22)
    # Each row: (calendar half, agenda half, year of this row's week (None when there is no date),
    # whether the right border is the scrollbar thumb)
    rows = []
    y, m = first
    hidden = 0  # how many of the previous month's items are not shown below it
    while len(rows) < skip + body_h:
        if rows:
            sym = "━" if m == 1 else "─"
            right = sym * iw
            # The previous month's items do not fit: the right end of the line says how many more
            # (muted, with one cell of line left at the end)
            if hidden:
                label = " " + t["more"](hidden) + " "
                if width(label) + 2 <= iw:
                    right = sym * (iw - width(label) - 1) + label + sym
            rows.append((rule(m == 1), right, None, False))
        weeks = calendar.monthcalendar(y, m)
        month_items = [e for e in entries if (e[0].year, e[0].month) == (y, m)]
        # Rows per column (no height limit), and the rows this block really gets
        # (at most 2/3 of the calendar height, at least the number of weeks)
        layout = max(len(weeks), -(-len(month_items) // n))
        height = min(layout, max(len(weeks), body_h * 2 // 3))
        # How many rows the selected month's agenda is scrolled; other months start from the top
        offset = min(WINDOW, layout - height) if (y, m) == (selected.year, selected.month) else 0
        # After scrolling, how many items below are still out of view: count every column
        end = offset + height
        hidden = sum(max(0, min(max(len(month_items) - c * layout, 0), layout) - end) for c in range(n))
        # Scrollbar: only when things do not fit; thumb length = height × visible share (at least 1);
        # the thumb touches the bottom when scrolled to the end
        if layout > height:
            length = max(1, height * height // layout)
            bar_start = offset * (height - length) // (layout - height)
            bar = range(bar_start, bar_start + length)
        else:
            bar = range(0)
        for i in range(height):
            label = ""
            if m == 1 and i == 0:
                label = t["year"](y)
            elif (m == 1 and i == 1) or (m != 1 and i == 0):
                label = t["month"](m)
            if i < len(weeks):
                week = weeks[i]
                days = list("".join(f"{d:>3}" if d else "   " for d in week) + " ")
                for k, d in enumerate(week):
                    if d and datetime.date(y, m, d) == selected:
                        start = 3 * k + 3 - len(str(d))
                        days[start - 1] = "["
                        days[3 * k + 3] = "]"
                day = next(d for d in week if d)
                left = rjust(label, 6) + f"{week_number(datetime.date(y, m, day), selected):>3} " + "│" + "".join(days)
                year = y
            else:
                left = " " * 10 + "│" + " " * 22
                year = None
            cols = []
            for c in range(n):
                k = c * layout + offset + i
                cols.append(item_text(month_items[k]) if k < len(month_items) else " " * colw)
            right = "  ".join(cols) if iw else ""
            rows.append((left, right, year, i in bar))
        y, m = (y + 1, 1) if m == 12 else (y, m + 1)
    visible = rows[skip:skip + body_h]
    # The top-left corner always shows the year of the top week on screen
    month_header = t["year"](next(year for _, _, year, _ in visible if year is not None))
    header_left = rjust(month_header, 6) + rjust(t["hw"], 4) + "│" + ljust("".join(" " + d for d in t["wd"]), 22)
    # Agenda header: 「重要事項」 on the left, the selected day on the right (one space after it);
    # when narrow the date stays whole and 「重要事項」 is cut, with at least one space between them
    date = t["date"](selected) + " "
    if iw:
        label = cut(t["items"], max(iw - width(date) - 1, 0))
        header_right = label + " " * (iw - width(label) - width(date)) + date
    else:
        header_right = ""
    inner = [(header_left, header_right, False), (rule(False), "─" * iw, False)]
    inner += [(l, r, bar) for l, r, _, bar in visible]

    def framed(left, right, thumb):
        line = "│" + left + "│"
        if aw:
            line += "│" + right + ("┃" if thumb else "│")
        return line

    lines = ["╭" + "─" * 33 + "╮" + ("╭" + "─" * iw + "╮" if aw else "")]
    lines += [framed(l, r, b) for l, r, b in inner]
    lines.append("╰" + "─" * 33 + "╯" + ("╰" + "─" * iw + "╯" if aw else ""))
    if sw:
        lines = [side + line for side, line in zip(side_lines(t, h, groups), lines)]
    # Hints line (has `s` whether wide or narrow)
    lines.append(t["hints"])
    # Lines wider than the screen (e.g. the hints line on a narrow screen) get cut
    return [ljust(cut(line, w), w) for line in lines]


class Canvas:
    """A cell-by-cell canvas: a wide character takes 2 cells and the second one holds None
    (skipped when printed, like `text_rows` in the Rust tests)"""

    def __init__(self, w, h):
        self.w = w
        self.rows = [[" "] * w for _ in range(h)]

    def put(self, x, y, s, limit=None):
        """Write s rightward from (x, y), up to cell x + limit
        (a wide character that does not fit is not written at all)"""
        end = min(self.w, x + (limit if limit is not None else self.w))
        for c in s:
            cw = width(c)
            if x + cw > end:
                break
            self.rows[y][x] = c
            if cw == 2:
                self.rows[y][x + 1] = None
            x += cw

    def line(self, y):
        return "".join(c for c in self.rows[y] if c is not None)


def month_screen(lang, w, h, selected, top, items, groups):
    """Layout 3: side menu (only when wide enough) + month grid (6 rows: a week, split in two where a new
    month starts) + hints line"""
    t = TEXTS[lang]
    sw = side_width(t, groups) if w >= 120 and not SIDE_HIDDEN else 0
    iw, ih = w - sw - 2, h - 3  # width and height inside the month grid box (minus the box and hints line)
    # Room taken by grid lines: with grid lines, 1 cell each between columns, under the header and
    # between weeks; without them 0 (the cells touch)
    gap = 0 if NO_GRID else 1
    # Columns: the week-number column is 5 cells; the days share the rest (minus the 7 vertical lines)
    # evenly, and the leftover cells go one per column starting from Monday
    days_w = iw - 5 - 7 * gap
    widths = [5] + [days_w // 7 + (1 if k < days_w % 7 else 0) for k in range(7)]
    xs = [0]
    for k in range(1, 8):
        xs.append(xs[k - 1] + widths[k - 1] + gap)
    lines_x = [xs[k] + widths[k] for k in range(7)] if gap else []  # vertical lines: right of each column
                                                                    # (right of Sunday is the outer border)
    starts = [top]
    for _ in range(5):
        starts.append(next_row(starts[-1]))
    # The row a new year starts after (k: between row k and row k + 1), if one does on screen
    new_year = next((k for k in range(5) if (starts[k + 1].month, starts[k + 1].day) == (1, 1)), None)
    # Without grid lines a new year still gets its own line, which takes one row of height
    year_row = 1 if not gap and new_year is not None else 0
    wh = (ih - 1 - gap - 5 * gap - year_row) // 6  # rows per week (leftover rows stay at the bottom;
                                                   # vertical lines still run down to the bottom border)
    week_y = lambda i: 1 + gap + i * (wh + gap) + (1 if year_row and i > new_year else 0)
    c = Canvas(iw, ih)
    left, right = ["│"] * ih, ["│"] * ih  # left and right outer border of each row
    for x in lines_x:
        for y in range(1, ih):
            c.rows[y][x] = "│"
    # The line below the header and above each row after the first; above a new year it is thick
    rules = [(1, "─", "┬", "├", "┤")]
    for i in range(5):
        thick = (starts[i + 1].month, starts[i + 1].day) == (1, 1)
        rules.append((week_y(i) + wh,) + (("━", "┿", "┝", "┥") if thick else ("─", "┼", "├", "┤")))
    for y, horizontal, cross, left_end, right_end in rules if gap else []:
        c.rows[y] = [horizontal] * iw
        for x in lines_x:
            c.rows[y][x] = cross
        left[y], right[y] = left_end, right_end
    # Without grid lines: only the new year's line, thick, on its own row
    if year_row:
        y = week_y(new_year) + wh
        c.rows[y] = ["━"] * iw
        left[y], right[y] = "┝", "┥"
    # A thick line carries the year, one cell in from Monday's left edge, if Monday's cell leaves a line
    # cell on both sides
    if new_year is not None and (gap or year_row):
        label = " " + t["year"](starts[new_year + 1].year) + " "
        if widths[1] >= width(label) + 2:
            c.put(xs[1] + 1, week_y(new_year) + wh, label)
    # Header: 「  週 」, and each day's name at the left edge of its column
    c.put(0, 0, " " + t["hw"], 5)
    for k, name in enumerate(t["wd"]):
        c.put(xs[k + 1], 0, " " + name, widths[k + 1])
    for i, start in enumerate(starts):
        end = row_end(start)
        monday = start - datetime.timedelta(days=start.weekday())
        days = [monday + datetime.timedelta(days=k) for k in range(7)]
        # Week number: from the row's first day
        c.put(0, week_y(i), f" {week_number(start, selected):>3} ", 5)
        week_cells = []  # the lines of each of this row's 7 cells (empty for days outside the row)
        for k, d in enumerate(days):
            if not start <= d <= end:
                week_cells.append([])
                continue
            # The selected day has no brackets; its cell gets a background instead (colors are not checked here)
            first = " " + str(d.day) + " " + (t["month"](d.month) if d.day == 1 else "")
            # Items that cover this day (a multi-day item is listed on each of its days);
            # items are already sorted by start day and time
            day_items = [it for it in items if it[0] <= d <= (it[4] or it[0])]
            if wh <= 1:
                # The cell has only 1 line: draw dots after the date (one per item, at most 4)
                cell = [first + ["", "⠂", "⠒", "⠖", "⠶"][min(len(day_items), 4)]]
            else:
                # List them all if they fit; if not, list one fewer and put +N on the last line
                shown = len(day_items) if len(day_items) <= wh - 1 else wh - 2
                icon = " " if NO_ICONS else " ∙ "
                cell = [first] + [icon + (time + " " if time else "") + title for _, time, title, _, _ in day_items[:shown]]
                if shown < len(day_items):
                    cell.append(f" +{len(day_items) - shown}")
            week_cells.append(cell)
        # Except on the first line, text may run right into the next cell as long as that cell's line
        # is empty (all the way to a cell with something in it, the selected day's cell, or Sunday)
        for k, cell in enumerate(week_cells):
            for r, text in enumerate(cell):
                end = k
                while (r > 0 and end + 1 < 7 and len(week_cells[end + 1]) <= r
                       and not (days[end + 1] == selected and start <= selected <= row_end(start))):
                    end += 1
                c.put(xs[k + 1], week_y(i) + r, text, xs[end + 1] + widths[end + 1] - xs[k + 1])
    # Outer border: the selected day in the middle of the top border (an odd leftover cell goes to the right);
    # vertical lines meet the bottom border as ┴
    title = " " + t["date"](selected) + " "
    top_border = Canvas(iw, 1)
    top_border.rows[0] = ["─"] * iw
    top_border.put((iw - width(title)) // 2, 0, title)
    bottom_border = ["─"] * iw
    for x in lines_x:
        bottom_border[x] = "┴"
    lines = ["╭" + top_border.line(0) + "╮"]
    lines += [left[y] + c.line(y) + right[y] for y in range(ih)]
    lines.append("╰" + "".join(bottom_border) + "╯")
    if sw:
        lines = [side + line for side, line in zip(side_lines(t, h, groups), lines)]
    lines.append(t["hints"])
    return [ljust(cut(line, w), w) for line in lines]


def read_tree(text):
    """For the tasks page: read every item in the whole file (items without a date too); a separate,
    simple parser. Returns ([dict(line, text, indent, depth, parent, task, done, title, date, end, time, group)]
    (in file order), mistakes [(line number, "date" / "time" / "range", the word)])"""
    items, stack, group, in_code, mistakes = [], [], None, False, []
    for n, raw in enumerate(text.splitlines(), start=1):
        stripped = raw.lstrip()
        if stripped.startswith("```") or stripped.startswith("~~~"):
            in_code = not in_code
            continue
        if in_code:
            continue
        heading = re.match(r"(#{1,6}) (.*)", raw)
        if heading:
            name = " ".join(w for w in heading.group(2).split() if w != "type:holiday")
            group, stack = (name, len(heading.group(1))), []
            continue
        if not stripped.startswith("- "):
            continue
        indent = sum(4 if c == "\t" else 1 for c in raw[: len(raw) - len(stripped)])
        rest = stripped[2:]
        box = re.match(r"\[([ xX])\](?: |$)", rest)
        task = box is not None
        done = task and box.group(1) in "xX"
        if task:
            rest = rest[3:]
        # Take out the first valid date (or range) and the time right after it; the rest is the title.
        # A ^id at the end of the line does not count either
        words, date, end, time = rest.split(), None, None, None
        out, i = [], 0
        while i < len(words):
            w = words[i]
            m = re.fullmatch(r"@(\d{4}-\d{2}-\d{2})(?:\.\.(\d{4}-\d{2}-\d{2}))?", w)
            if m and date is None:
                try:
                    first = datetime.date.fromisoformat(m.group(1))
                    last = datetime.date.fromisoformat(m.group(2)) if m.group(2) else first
                    ok = last >= first
                    if not ok:
                        mistakes.append((n, "range", w))
                except ValueError:
                    ok = False
                    mistakes.append((n, "date", w))
                if ok:
                    date, end = first, (last if last > first else None)
                    # A word right after the date that looks like a time: if that time exists it is the time;
                    # if not, it stays as text and a mistake is recorded
                    if i + 1 < len(words) and re.fullmatch(r"\d\d:\d\d", words[i + 1]):
                        if int(words[i + 1][:2]) < 24 and int(words[i + 1][3:]) < 60:
                            time = words[i + 1]
                            i += 1
                        else:
                            mistakes.append((n, "time", words[i + 1]))
                    i += 1
                    continue
            out.append(w)
            i += 1
        if out and re.fullmatch(r"\^[A-Za-z0-9-]+", out[-1]):
            out.pop()
        while stack and stack[-1][0] >= indent:
            stack.pop()
        parent = stack[-1][1] if stack else None
        items.append(dict(line=n, text=raw.strip(), indent=indent, depth=len(stack), parent=parent, task=task,
                          done=done, title=" ".join(out), date=date, end=end, time=time, group=group))
        stack.append((indent, len(items) - 1))
    return items, mistakes


def tasks_screen(lang, w, h, text, groups):
    """Tasks page: side menu (only when wide enough) + tasks box (header, each row of the tree, and at the
    bottom the original text of the chosen line)"""
    t = TEXTS[lang]
    items, mistakes = read_tree(text)
    hidden_names = {groups[i][0] for i in HIDDEN}
    expanded = {int(n) for n in os.environ.get("EXPANDED", "").split(",") if n}
    sw = side_width(t, groups) if w >= 120 and not SIDE_HIDDEN else 0
    iw, ih = w - sw - 2, h - 3
    body_h = ih - 4

    def visible(k):
        # Visible only when every level above it is expanded
        p = items[k]["parent"]
        while p is not None:
            if items[p]["line"] not in expanded:
                return False
            p = items[p]["parent"]
        return True

    rows, previous = [], object()
    for k, it in enumerate(items):
        name = it["group"][0] if it["group"] else None
        if name in hidden_names or not visible(k):
            continue
        if it["group"] != previous:
            rows.append(("heading", it["group"]))
            previous = it["group"]
        rows.append(("item", k))
    item_rows = [r for r, row in enumerate(rows) if row[0] == "item"]
    chosen_line = int(os.environ["CHOSEN_LINE"]) if "CHOSEN_LINE" in os.environ else None
    cursor = next((r for r in item_rows if items[rows[r][1]]["line"] == chosen_line), item_rows[0] if item_rows else None)
    # Scrolling: start from the top; keep at least 2 rows between the chosen item and the top or bottom
    # edge; do not scroll past the end
    top = 0
    if cursor is not None:
        margin = min(2, (body_h - 1) // 2)
        if cursor >= top + body_h - margin:
            top = cursor + margin + 1 - body_h
        top = max(0, min(top, len(rows) - body_h))

    def date_label(it):
        # Text of the date column on the right; dates not in 2026 get the year in front
        if not it["date"]:
            return ""
        d = range_text((it["date"], None, None, None, it["end"]))
        return d if it["date"].year == 2026 else f"{it['date'].year}/" + d

    # Widths of the two right columns: date column = the longest date among the listed items;
    # time column = 6 if any item has a time, else 0
    listed = [items[row[1]] for row in rows if row[0] == "item"]
    date_w = max((width(date_label(it)) for it in listed), default=0)
    time_w = 6 if any(it["time"] for it in listed) else 0

    def item_line(k):
        it = items[k]
        children = [c for c in items if c["parent"] == k]
        marker = " " if not children else ("▾" if it["line"] in expanded else "▸")
        check = ("[x]" if it["done"] else "[ ]") if it["task"] else " ∙ "
        left = " " + "  " * it["depth"] + marker + " " + check + " " + it["title"]
        subtasks = [c for c in children if c["task"]]
        if subtasks:
            left += f"  {sum(c['done'] for c in subtasks)}/{len(subtasks)}"
        right = ""
        if it["date"]:
            time = " " + it["time"] if it["time"] else ""
            right = rjust(date_label(it), date_w) + ljust(time, time_w) + " "
        left = cut(left, iw - width(right) - 1)
        return left + " " * (iw - width(left) - width(right)) + right

    body = []
    for row in rows[top:top + body_h]:
        if row[0] == "heading":
            name, level = row[1] if row[1] else (None, 1)
            body.append(ljust(cut(" " + "  " * (level - 1) + (t["ungrouped"] if name is None else name), iw), iw))
        else:
            body.append(item_line(row[1]))
    body += [" " * iw] * (body_h - len(body))
    detail = ""
    if cursor is not None:
        it = items[rows[cursor][1]]
        wrong = [m for m in mistakes if m[0] == it["line"]]
        detail = t["mistake"](*wrong[0]) if wrong else t["detail"](it["line"], it["text"])
    # Header: 「任務」 on the left, the number of mistakes on the right (if there are any)
    header = t["tasks"]
    if mistakes:
        count = t["mistakes"](len(mistakes))
        header = header + " " * (iw - width(header) - width(count)) + count
    inner = [ljust(header, iw), "─" * iw] + body + ["─" * iw, ljust(cut(detail, iw), iw)]
    lines = ["╭" + "─" * iw + "╮"] + ["│" + row + "│" for row in inner] + ["╰" + "─" * iw + "╯"]
    if sw:
        lines = [side + line for side, line in zip(side_lines(t, h, groups), lines)]
    lines.append(t["hints_tasks"])
    return [ljust(cut(line, w), w) for line in lines]


def row_start(d):
    """First day of the month grid row holding d: its Monday, unless its month starts later that week"""
    return max(d - datetime.timedelta(days=d.weekday()), d.replace(day=1))


def row_end(start):
    """Last day of the row starting on start: its Sunday, unless the month ends first"""
    sunday = start - datetime.timedelta(days=start.weekday()) + datetime.timedelta(days=6)
    next_month = (start.replace(day=28) + datetime.timedelta(days=4)).replace(day=1)
    return min(sunday, next_month - datetime.timedelta(days=1))


def next_row(start):
    return row_end(start) + datetime.timedelta(days=1)


def previous_row(start):
    return row_start(start - datetime.timedelta(days=1))


def month_top(selected, given):
    """First day of the top row: the given one if any; if not given, the last row of the month before the
    selected one. If the selected row is not within these 6 rows, scroll just enough to show it"""
    top = row_start(datetime.date.fromisoformat(given)) if given else previous_row(selected.replace(day=1))
    row = row_start(selected)
    if CENTER:
        # Centered: the selected row is row 3 of the 6 (2 rows above it)
        return previous_row(previous_row(row))
    last = top
    for _ in range(5):
        last = next_row(last)
    if row < top:
        top = row
    elif row > last:
        top = row
        for _ in range(5):
            top = previous_row(top)
    return top


lang, w, h = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
selected = datetime.date.fromisoformat(sys.argv[4])
text = sys.stdin.read()
groups = read_groups(text)
if os.environ.get("LAYOUT") == "4":
    out = tasks_screen(lang, w, h, text, groups)
elif os.environ.get("LAYOUT") == "3":
    items = read_items(text, {groups[i][0] for i in HIDDEN})
    top = month_top(selected, sys.argv[5] if len(sys.argv) > 5 else None)
    out = month_screen(lang, w, h, selected, top, items, groups)
else:
    items = read_items(text, {groups[i][0] for i in HIDDEN})
    if len(sys.argv) > 6:
        first = tuple(int(part) for part in sys.argv[5].split("-"))
        skip = int(sys.argv[6])
    else:
        first, skip = (selected.year, selected.month), 0
    out = screen(lang, w, h, selected, first, skip, items, groups)
for line in out:
    assert width(line) == w, (width(line), line)
    print(f'            "{line.rstrip()}",')
