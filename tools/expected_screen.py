"""另外算出版型 1（side menu + 左月曆 + 右事項）、版型 3（side menu + 大月曆）的畫面，當 `src/ui.rs` 畫面 test 的預期值。

跟 Rust 的程式分開寫（週用 Python 的 `calendar` 算、週數用另一種算法、任務檔用簡單的 regex 讀），
兩邊算出來一樣才算對；不要看 Rust 畫出來的結果抄回來。

用法（在 repo 最上層）：python3 tools/expected_screen.py zh|en 寬 高 選取的日期 [最上面那塊的月份 YYYY-MM 跳過幾列] < 任務檔
例：python3 tools/expected_screen.py zh 70 23 2026-10-17 < testdata/agenda.md
版型 3（環境變數 LAYOUT=3）：python3 tools/expected_screen.py zh|en 寬 高 選取的日期 [最上面那週的星期一 YYYY-MM-DD] < 任務檔
（沒給最上面那週 = 剛打開：選取那個月 1 號的前一週，選取的那週不在 6 週裡再捲）
環境變數：CHOSEN = 右邊選到排好之後的第幾件（鍵盤在右邊）；WINDOW = 選取的那個月右邊捲了幾列；
SIDE_HIDDEN = 按了 `s`、side menu 收起來（寬 120 格以上才有 side menu）；
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
    "en": dict(hints=" hjkl move  Tab switch  [] month  t today  q quit",
               hints_side=" hjkl move  Tab switch  [] month  t today  s calendars  q quit",
               side=" Calendars", ungrouped="Ungrouped", hw=" Wk ",
               wd=["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"], items=" Agenda", more=lambda n: f"{n} more",
               year=lambda y: f"{y}", month=lambda m: calendar.month_abbr[m],
               date=lambda d: f"{d:%a %b} {d.day}, {d.year}"),
    "zh": dict(hints=" hjkl 移動  Tab 切換  [] 換月  t 今天  q 離開",
               hints_side=" hjkl 移動  Tab 切換  [] 換月  t 今天  s 日曆  q 離開",
               side=" 日曆", ungrouped="未分組", hw=" 週 ",
               wd=list("一二三四五六日"), items=" 重要事項", more=lambda n: f"還有 {n} 件",
               year=lambda y: f"{y}年", month=lambda m: f"{m}月",
               date=lambda d: f"{d.year}/{d.month}/{d.day}（{'一二三四五六日'[d.weekday()]}）"),
}
CHOSEN = int(os.environ["CHOSEN"]) if "CHOSEN" in os.environ else None
WINDOW = int(os.environ.get("WINDOW", "0"))
SIDE_HIDDEN = "SIDE_HIDDEN" in os.environ
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


def week_number(d):
    # 跟 Rust 不同的算法：這天那週的星期一 − 1/1 那週的星期一，除以 7
    monday = lambda x: x - datetime.timedelta(days=x.weekday())
    return (monday(d) - monday(datetime.date(d.year, 1, 1))).days // 7 + 1


def read_items(text, hidden_names):
    """簡單版：`- ` 開頭、有 `@YYYY-MM-DD` 的行，日曆（上面最近的 `#` 標題）沒勾的不算；
    回傳 (日期, 時間, 標題, 第幾行)，排好"""
    items = []
    name = None
    for i, line in enumerate(text.splitlines()):
        heading = re.match(r"#+ (.*)", line)
        if heading:
            name = heading.group(1).strip()
        m = re.match(r"\s*- (?:\[[ xX]\] )?(.*?) @(\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2}))?$", line)
        if m and name not in hidden_names:
            items.append((datetime.date.fromisoformat(m.group(2)), m.group(3), m.group(1), i))
    # 日期、時間（沒時間的在前）、檔案順序
    items.sort(key=lambda it: (it[0], it[1] is not None, it[1] or "", it[3]))
    return items


def read_groups(text):
    """分組（side menu 的日曆），照第一次出現的順序；回傳 [(名稱（標題之前的是 None）, 有日期的有幾件)]"""
    groups = {}  # dict 會照放進去的順序
    name = None
    for line in text.splitlines():
        heading = re.match(r"#+ (.*)", line)
        if heading:
            name = heading.group(1).strip()
        elif re.match(r"\s*- ", line):
            groups.setdefault(name, 0)
            groups[name] += bool(re.search(r" @\d{4}-\d{2}-\d{2}", line))
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

    def item_text(it):
        date, time, title, _ = it
        d = f"{date.month}/{date.day}"
        rest = (time + " " if time else "") + title
        if CHOSEN is not None and items.index(it) == CHOSEN:
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
        month_items = [it for it in items if (it[0].year, it[0].month) == (y, m)]
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
                left = rjust(label, 6) + f"{week_number(datetime.date(y, m, day)):>3} " + "│" + "".join(days)
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
    # 右邊的表頭：左邊「重要事項」、右邊選取的日期（後面一格空白）
    date = t["date"](selected) + " "
    if iw:
        pad = iw - width(t["items"]) - width(date)
        assert pad >= 0, "表頭放不下日期"
        header_right = t["items"] + " " * pad + date
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
    # 欄：週數欄 5 格；天平分剩下的寬度（扣掉 7 條直線），零頭從星期一開始一欄多 1 格
    days_w = iw - 5 - 7
    widths = [5] + [days_w // 7 + (1 if k < days_w % 7 else 0) for k in range(7)]
    xs = [0]
    for k in range(1, 8):
        xs.append(xs[k - 1] + widths[k - 1] + 1)
    lines_x = [xs[k] + widths[k] for k in range(7)]  # 直線：每一欄的右邊（星期日右邊是外框）
    wh = (ih - 2 - 5) // 6  # 每週幾列（除不盡的零頭留在最下面，直線照樣畫到下框）
    week_y = lambda i: 2 + i * (wh + 1)
    c = Canvas(iw, ih)
    left, right = ["│"] * ih, ["│"] * ih  # 每一列的左右外框
    for x in lines_x:
        for y in range(1, ih):
            c.rows[y][x] = "│"
    rules = [(1, "┬")] + [(week_y(i) + wh, "┼") for i in range(5)]
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
        c.put(0, week_y(i), f" {week_number(numbered):>3} ", 5)
        week_cells = []  # 這週 7 格各自的每一行
        for k, d in enumerate(days):
            mark = ("[", "]") if d == selected else (" ", " ")
            first = mark[0] + str(d.day) + mark[1] + (t["month"](d.month) if d.day == 1 else "")
            day_items = [it for it in items if it[0] == d]  # items 已經照日期、時間排好
            if wh <= 1:
                # 格子只有 1 行：日期後面畫點點（幾件就幾個，最多 4 個）
                cell = [first + ["", "⠂", "⠒", "⠖", "⠶"][min(len(day_items), 4)]]
            else:
                # 放得下就全列；放不下就少列一件，最後一行 +N
                shown = len(day_items) if len(day_items) <= wh - 1 else wh - 2
                cell = [first] + [" ∙ " + (time + " " if time else "") + title for _, time, title, _ in day_items[:shown]]
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


def month_top(selected, given):
    """最上面那週的星期一：有給就用；沒給 = 選取那個月 1 號那週的前一週。選取的那週不在這 6 週裡就捲到剛好看得到"""
    monday = lambda x: x - datetime.timedelta(days=x.weekday())
    top = datetime.date.fromisoformat(given) if given else monday(selected.replace(day=1)) - datetime.timedelta(weeks=1)
    week = monday(selected)
    if week < top:
        top = week
    elif week > top + datetime.timedelta(weeks=5):
        top = week - datetime.timedelta(weeks=5)
    return top


lang, w, h = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
selected = datetime.date.fromisoformat(sys.argv[4])
text = sys.stdin.read()
groups = read_groups(text)
items = read_items(text, {groups[i][0] for i in HIDDEN})
if os.environ.get("LAYOUT") == "3":
    top = month_top(selected, sys.argv[5] if len(sys.argv) > 5 else None)
    out = month_screen(lang, w, h, selected, top, items, groups)
else:
    if len(sys.argv) > 6:
        first = tuple(int(part) for part in sys.argv[5].split("-"))
        skip = int(sys.argv[6])
    else:
        first, skip = (selected.year, selected.month), 0
    out = screen(lang, w, h, selected, first, skip, items, groups)
for line in out:
    assert width(line) == w, (width(line), line)
    print(f'            "{line.rstrip()}",')
