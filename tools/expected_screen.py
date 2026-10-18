"""另外算出版型 1（side menu + 左月曆 + 右事項）、版型 3（side menu + 大月曆）的畫面，當 `src/ui.rs` 畫面 test 的預期值。

跟 Rust 的程式分開寫（週用 Python 的 `calendar` 算、週數用另一種算法、任務檔用簡單的 regex 讀），
兩邊算出來一樣才算對；不要看 Rust 畫出來的結果抄回來。

用法（在 repo 最上層）：python3 tools/expected_screen.py zh|en 寬 高 選取的日期 [最上面那塊的月份 YYYY-MM 跳過幾列] < 任務檔
例：python3 tools/expected_screen.py zh 70 23 2026-10-17 < testdata/agenda.md
版型 3（環境變數 LAYOUT=3）：python3 tools/expected_screen.py zh|en 寬 高 選取的日期 [最上面那週的星期一 YYYY-MM-DD] < 任務檔
（沒給最上面那週 = 剛打開：選取那個月 1 號的前一週，選取的那週不在 6 週裡再捲）
任務頁（環境變數 LAYOUT=4）：python3 tools/expected_screen.py zh|en 寬 高 今天以外沒用到的日期 < 任務檔；
EXPANDED = 展開了子任務的事在第幾行，逗號隔開；CHOSEN_LINE = 選到的那件在第幾行（沒給 = 第一件）
環境變數：CHOSEN = 右邊選到排好之後的第幾件（鍵盤在右邊）；WINDOW = 選取的那個月右邊捲了幾列；
SIDE_HIDDEN = 按了 `s`、side menu 收起來（寬 120 格以上才有 side menu）；
NO_GRID = 版型 3 不畫格線（設定檔 `[month] grid_lines = false`）；NO_ICONS = 版型 3 的事前面不加點（`item_icons = false`）；
CENTER = 版型 3 選取的那週固定在第 3 列（`center_cursor = true`，最上面那週的參數不用給）；
RELATIVE = 版型 1、3 的週數欄：選取那週寫週數，其他週寫差幾週（`[calendar] relative_weeks = true`）；
HIDDEN = 沒勾的日曆是 side menu 的第幾個，逗號隔開（例：`1,3`），它們的事不顯示；LAYOUT = 3 是版型 3
今天固定是 2026-10-17。
"""
import calendar
import datetime
import os
import re
import sys
import unicodedata

TEXTS = {
    "en": dict(hints=" hjkl move  Tab switch  [] month  t today  ? help  q quit",
               hints_tasks=" jk move  hl fold  Space check  e edit  a add  Esc calendar  ? help  q quit",
               hints_tasks_side=" jk move  hl fold  Space check  e edit  a add  s calendars  Esc calendar  ? help  q quit",
               tasks=" Tasks", detail=lambda n, t: f" Line {n}  {t}", mistakes=lambda n: f"{n} mistake(s) ",
               mistake=lambda n, kind, token: f" Line {n}: {token}: " + {"date": "no such day", "time": "no such time", "range": "ends before it starts"}[kind],
               hints_side=" hjkl move  Tab switch  [] month  t today  s calendars  ? help  q quit",
               side=" Calendars", ungrouped="Ungrouped", hw=" Wk ",
               wd=["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"], items=" Agenda", more=lambda n: f"{n} more",
               year=lambda y: f"{y}", month=lambda m: calendar.month_abbr[m],
               date=lambda d: f"{d:%a %b} {d.day}, {d.year}"),
    "zh": dict(hints=" hjkl 移動  Tab 切換  [] 換月  t 今天  ? 說明  q 離開",
               hints_tasks=" jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  Esc 月曆  ? 說明  q 離開",
               hints_tasks_side=" jk 移動  hl 收合  Space 打勾  e 編輯  a 新增  s 日曆  Esc 月曆  ? 說明  q 離開",
               tasks=" 任務", detail=lambda n, t: f" 第 {n} 行  {t}", mistakes=lambda n: f"{n} 處寫錯 ",
               mistake=lambda n, kind, token: f" 第 {n} 行：{token}：" + {"date": "沒有這一天", "time": "沒有這個時間", "range": "結束在開始之前"}[kind],
               hints_side=" hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  ? 說明  q 離開",
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
SIDE_MIN, SIDE_MAX = 22, 30  # side menu 最窄、最寬幾格（含框線）；寬度跟著最長的那個日曆


def width(s):
    return sum(2 if unicodedata.east_asian_width(c) in "WF" else 1 for c in s)


def rjust(s, n):
    return " " * (n - width(s)) + s


def ljust(s, n):
    return s + " " * (n - width(s))


def cut(s, n):
    """最多 n 格；放不下的寬字元整個不放"""
    out = ""
    for c in s:
        if width(out + c) > n:
            break
        out += c
    return out


def week_number(d, selected=None):
    # 跟 Rust 不同的算法：這天那週的星期一 − 1/1 那週的星期一，除以 7
    monday = lambda x: x - datetime.timedelta(days=x.weekday())
    if RELATIVE and selected and monday(d) != monday(selected):
        # 相對：跟選取那週差幾週（不分前後）
        return abs((monday(d) - monday(selected)).days) // 7
    return (monday(d) - monday(datetime.date(d.year, 1, 1))).days // 7 + 1


def read_items(text, hidden_names):
    """簡單版：`- ` 開頭、行尾是 `@YYYY-MM-DD` 或 `@YYYY-MM-DD..YYYY-MM-DD`（可以再接時間）的行，
    日曆（上面最近的 `#` 標題）沒勾的不算；回傳 (日期, 時間, 標題, 第幾行, 跨多天的最後一天或 None)，排好"""
    items = []
    name = None
    for i, line in enumerate(text.splitlines()):
        heading = re.match(r"#+ (.*)", line)
        if heading:
            # `type:holiday` 不算名字
            name = " ".join(w for w in heading.group(1).split() if w != "type:holiday")
        m = re.match(r"\s*- (?:\[[ xX]\] )?(.*?) @(\d{4}-\d{2}-\d{2})(?:\.\.(\d{4}-\d{2}-\d{2}))?(?: (\d{2}:\d{2}))?$", line)
        if m and name not in hidden_names:
            end = datetime.date.fromisoformat(m.group(3)) if m.group(3) else None
            items.append((datetime.date.fromisoformat(m.group(2)), m.group(4), m.group(1), i, end))
    # 日期、時間（沒時間的在前）、檔案順序
    items.sort(key=lambda it: (it[0], it[1] is not None, it[1] or "", it[3]))
    return items


def agenda_entries(items):
    """右邊的每一行：(列在哪天, 是不是從上個月接過來的, 那件事)。跨月的事碰到的每個月各一行，
    之後的月列在 1 號、排在那天最前面"""
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
    """右邊的日期：`10/5`、跨多天 `10/20~25`、跨月 `10/28~11/3`"""
    start, end = it[0], it[4]
    d = f"{start.month}/{start.day}"
    if end and (end.year, end.month) == (start.year, start.month):
        d += f"~{end.day}"
    elif end:
        d += f"~{end.month}/{end.day}"
    return d


def read_groups(text):
    """分組（side menu 的日曆），照第一次出現的順序；回傳 [(名稱（標題之前的是 None）, 有日期的有幾件)]
    （用 read_tree 讀：code block 裡的不算、不存在的日期不算有日期）"""
    groups = {}  # dict 會照放進去的順序
    for it in read_tree(text)[0]:
        name = it["group"][0] if it["group"] else None
        groups.setdefault(name, 0)
        groups[name] += it["date"] is not None
    return list(groups.items())


def side_width(t, groups):
    """side menu 的寬度（含框線）：最長的那列「 ☑ 名字 件數 」加框線，在 SIDE_MIN ～ SIDE_MAX 之間"""
    longest = max((3 + width(t["ungrouped"] if name is None else name) + len(f" {count} ")
                   for name, count in groups), default=0)
    return min(max(longest + 2, SIDE_MIN), SIDE_MAX)


def side_lines(t, h, groups):
    """side menu 那一欄的每一列（含上下框線，不含操作說明）：一個日曆一列「 ☑ 名字    件數 」"""
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
    body_h = h - 5  # 框上下、表頭、表頭的線、操作說明
    sw = side_width(t, groups) if w >= 120 and not SIDE_HIDDEN else 0  # side menu 的寬度
    aw = w - sw - 35  # 右邊框的寬度（含框線）
    iw = max(aw - 2, 0)  # 右邊框裡面的寬度
    n = min(max((iw + 2) // 30, 1), 4)  # 分幾欄
    colw = (iw - 2 * (n - 1)) // n if iw else 0
    assert iw == 0 or colw * n + 2 * (n - 1) == iw, "挑一個整除的寬度"

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
    # 每一列：(月曆那半, 事項那半, 這列那週的年份（沒有日期是 None）, 右框線是不是捲軸的亮塊)
    rows = []
    y, m = first
    hidden = 0  # 上一個月下面還有幾件沒顯示出來
    while len(rows) < skip + body_h:
        if rows:
            sym = "━" if m == 1 else "─"
            right = sym * iw
            # 上一個月的事放不下：線的右邊寫還有幾件（淡色，最後留一格線）
            if hidden:
                label = " " + t["more"](hidden) + " "
                if width(label) + 2 <= iw:
                    right = sym * (iw - width(label) - 1) + label + sym
            rows.append((rule(m == 1), right, None, False))
        weeks = calendar.monthcalendar(y, m)
        month_items = [e for e in entries if (e[0].year, e[0].month) == (y, m)]
        # 每欄排幾列（不限高）、這一塊實際幾列（最多月曆高度的 2/3，至少是週數）
        layout = max(len(weeks), -(-len(month_items) // n))
        height = min(layout, max(len(weeks), body_h * 2 // 3))
        # 選取的那個月右邊捲了幾列；其他月從頭
        offset = min(WINDOW, layout - height) if (y, m) == (selected.year, selected.month) else 0
        # 捲過之後下面還有幾件看不到：每一欄都算
        end = offset + height
        hidden = sum(max(0, min(max(len(month_items) - c * layout, 0), layout) - end) for c in range(n))
        # 捲軸：放不下時才有；亮塊長度 = 高度 × 看得到的比例（至少 1），捲到底時貼底
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
    # 左上角一直寫畫面最上面那週的年份
    month_header = t["year"](next(year for _, _, year, _ in visible if year is not None))
    header_left = rjust(month_header, 6) + rjust(t["hw"], 4) + "│" + ljust("".join(" " + d for d in t["wd"]), 22)
    # 右邊的表頭：左邊「重要事項」、右邊選取的日期（後面一格空白）；窄的時候日期照寫，「重要事項」切掉，中間至少空一格
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
    # 夠寬時操作說明多一個 `s`（收起來也一樣）
    lines.append(t["hints_side"] if w >= 120 else t["hints"])
    # 比畫面寬的（例：窄畫面的操作說明）會被切掉
    return [ljust(cut(line, w), w) for line in lines]


class Canvas:
    """一格一格的畫布：寬字元佔 2 格，後面那格放 None（印出來時跳過，跟 Rust test 的 `text_rows` 一樣）"""

    def __init__(self, w, h):
        self.w = w
        self.rows = [[" "] * w for _ in range(h)]

    def put(self, x, y, s, limit=None):
        """從 (x, y) 往右寫 s，最多寫到 x + limit 格（寬字元放不下就整個不寫）"""
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
    """版型 3：side menu（夠寬才有）+ 大月曆（6 週）+ 操作說明"""
    t = TEXTS[lang]
    sw = side_width(t, groups) if w >= 120 and not SIDE_HIDDEN else 0
    iw, ih = w - sw - 2, h - 3  # 大月曆框裡面的寬、高（扣掉框、操作說明）
    # 格線佔的寬度：有格線 = 欄和欄之間、表頭下面、週和週之間各 1 格；沒有就 0（格子緊貼著）
    gap = 0 if NO_GRID else 1
    # 欄：週數欄 5 格；天平分剩下的寬度（扣掉 7 條直線），零頭從星期一開始一欄多 1 格
    days_w = iw - 5 - 7 * gap
    widths = [5] + [days_w // 7 + (1 if k < days_w % 7 else 0) for k in range(7)]
    xs = [0]
    for k in range(1, 8):
        xs.append(xs[k - 1] + widths[k - 1] + gap)
    lines_x = [xs[k] + widths[k] for k in range(7)] if gap else []  # 直線：每一欄的右邊（星期日右邊是外框）
    wh = (ih - 1 - gap - 5 * gap) // 6  # 每週幾列（除不盡的零頭留在最下面，直線照樣畫到下框）
    week_y = lambda i: 1 + gap + i * (wh + gap)
    c = Canvas(iw, ih)
    left, right = ["│"] * ih, ["│"] * ih  # 每一列的左右外框
    for x in lines_x:
        for y in range(1, ih):
            c.rows[y][x] = "│"
    rules = [(1, "┬")] + [(week_y(i) + wh, "┼") for i in range(5)] if gap else []
    for y, cross in rules:
        c.rows[y] = ["─"] * iw
        for x in lines_x:
            c.rows[y][x] = cross
        left[y], right[y] = "├", "┤"
    # 表頭：「  週 」，每天的名字放在那欄最左邊
    c.put(0, 0, " " + t["hw"], 5)
    for k, name in enumerate(t["wd"]):
        c.put(xs[k + 1], 0, " " + name, widths[k + 1])
    month = (selected.year, selected.month)
    for i in range(6):
        monday = top + datetime.timedelta(weeks=i)
        days = [monday + datetime.timedelta(days=k) for k in range(7)]
        # 週數：那週裡屬於選取那個月的日子，沒有就用星期一
        numbered = next((d for d in days if (d.year, d.month) == month), monday)
        c.put(0, week_y(i), f" {week_number(numbered, selected):>3} ", 5)
        week_cells = []  # 這週 7 格各自的每一行
        for k, d in enumerate(days):
            mark = ("[", "]") if d == selected else (" ", " ")
            first = mark[0] + str(d.day) + mark[1] + (t["month"](d.month) if d.day == 1 else "")
            # 蓋到這天的事（跨多天的每一天都列）；items 已經照開始那天、時間排好
            day_items = [it for it in items if it[0] <= d <= (it[4] or it[0])]
            if wh <= 1:
                # 格子只有 1 行：日期後面畫點點（幾件就幾個，最多 4 個）
                cell = [first + ["", "⠂", "⠒", "⠖", "⠶"][min(len(day_items), 4)]]
            else:
                # 放得下就全列；放不下就少列一件，最後一行 +N
                shown = len(day_items) if len(day_items) <= wh - 1 else wh - 2
                icon = " " if NO_ICONS else " ∙ "
                cell = [first] + [icon + (time + " " if time else "") + title for _, time, title, _, _ in day_items[:shown]]
                if shown < len(day_items):
                    cell.append(f" +{len(day_items) - shown}")
            week_cells.append(cell)
        # 第一行以外：字可以往右寫進隔壁格，只要隔壁格那一行是空的（一路到有東西的格子或星期日）
        for k, cell in enumerate(week_cells):
            for r, text in enumerate(cell):
                end = k
                while r > 0 and end + 1 < 7 and len(week_cells[end + 1]) <= r:
                    end += 1
                c.put(xs[k + 1], week_y(i) + r, text, xs[end + 1] + widths[end + 1] - xs[k + 1])
    # 外框：上面右邊寫選取的日期（貼著右上角）；直線碰到下框接成 ┴
    title = " " + t["date"](selected) + " "
    top_border = Canvas(iw, 1)
    top_border.rows[0] = ["─"] * iw
    top_border.put(iw - width(title), 0, title)
    bottom_border = ["─"] * iw
    for x in lines_x:
        bottom_border[x] = "┴"
    lines = ["╭" + top_border.line(0) + "╮"]
    lines += [left[y] + c.line(y) + right[y] for y in range(ih)]
    lines.append("╰" + "".join(bottom_border) + "╯")
    if sw:
        lines = [side + line for side, line in zip(side_lines(t, h, groups), lines)]
    lines.append(t["hints_side"] if w >= 120 else t["hints"])
    return [ljust(cut(line, w), w) for line in lines]


def read_tree(text):
    """任務頁用：讀整份檔案的每一件事（沒日期的也算），另外寫的簡單版 parser。
    回傳 ([dict(line, text, indent, depth, parent, task, done, title, date, end, time, group)]（照檔案的順序）,
    寫錯的地方 [(第幾行, "date" / "time" / "range", 那個字)])"""
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
        # 第一個合法的日期（或範圍）和緊接的時間拿掉，剩下的是標題；行尾的 ^id 也不算
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
                    # 日期後面緊接長得像時間的字：存在的就是時間，不存在的當文字、記一個錯
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
    """任務頁：side menu（夠寬才有）+ 任務框（表頭、樹狀的每一列、最下面選到的那一行原本的文字）"""
    t = TEXTS[lang]
    items, mistakes = read_tree(text)
    hidden_names = {groups[i][0] for i in HIDDEN}
    expanded = {int(n) for n in os.environ.get("EXPANDED", "").split(",") if n}
    sw = side_width(t, groups) if w >= 120 and not SIDE_HIDDEN else 0
    iw, ih = w - sw - 2, h - 3
    body_h = ih - 4

    def visible(k):
        # 上面每一層都展開了才看得到
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
    # 捲動：從最上面開始，選到的那件離上下緣至少留 2 列；不捲過頭
    top = 0
    if cursor is not None:
        margin = min(2, (body_h - 1) // 2)
        if cursor >= top + body_h - margin:
            top = cursor + margin + 1 - body_h
        top = max(0, min(top, len(rows) - body_h))

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
            d = range_text((it["date"], None, None, None, it["end"]))
            if it["date"].year != 2026:
                d = f"{it['date'].year}/" + d
            right = d + (" " + it["time"] if it["time"] else "") + " "
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
    # 表頭：左邊「任務」、右邊有幾處寫錯（有的話）
    header = t["tasks"]
    if mistakes:
        count = t["mistakes"](len(mistakes))
        header = header + " " * (iw - width(header) - width(count)) + count
    inner = [ljust(header, iw), "─" * iw] + body + ["─" * iw, ljust(cut(detail, iw), iw)]
    lines = ["╭" + "─" * iw + "╮"] + ["│" + row + "│" for row in inner] + ["╰" + "─" * iw + "╯"]
    if sw:
        lines = [side + line for side, line in zip(side_lines(t, h, groups), lines)]
    lines.append(t["hints_tasks_side"] if w >= 120 else t["hints_tasks"])
    return [ljust(cut(line, w), w) for line in lines]


def month_top(selected, given):
    """最上面那週的星期一：有給就用；沒給 = 選取那個月 1 號那週的前一週。選取的那週不在這 6 週裡就捲到剛好看得到"""
    monday = lambda x: x - datetime.timedelta(days=x.weekday())
    top = datetime.date.fromisoformat(given) if given else monday(selected.replace(day=1)) - datetime.timedelta(weeks=1)
    week = monday(selected)
    if CENTER:
        # 置中：選取的那週在 6 週的第 3 列（前面 2 週）
        return week - datetime.timedelta(weeks=2)
    if week < top:
        top = week
    elif week > top + datetime.timedelta(weeks=5):
        top = week - datetime.timedelta(weeks=5)
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
