"""另外算出版型 1（side menu + 左月曆 + 右事項）的畫面，當 `src/ui.rs` 畫面 test 的預期值。

跟 Rust 的程式分開寫（週用 Python 的 `calendar` 算、週數用另一種算法、任務檔用簡單的 regex 讀），
兩邊算出來一樣才算對；不要看 Rust 畫出來的結果抄回來。

用法（在 repo 最上層）：python3 tools/expected_screen.py zh|en 寬 高 選取的日期 [最上面那塊的月份 YYYY-MM 跳過幾列] < 任務檔
例：python3 tools/expected_screen.py zh 70 23 2026-10-17 < testdata/agenda.md
環境變數：CHOSEN = 右邊選到排好之後的第幾件（鍵盤在右邊）；WINDOW = 選取的那個月右邊捲了幾列；
SIDE_HIDDEN = 按了 `s`、side menu 收起來（寬 120 格以上才有 side menu）
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
SIDE_WIDTH = 22  # side menu 的寬度（含框線）


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


def read_items(text):
    """簡單版：`- ` 開頭、有 `@YYYY-MM-DD` 的行；回傳 (日期, 時間, 標題, 第幾行)，排好"""
    items = []
    for i, line in enumerate(text.splitlines()):
        m = re.match(r"\s*- (?:\[[ xX]\] )?(.*?) @(\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2}))?$", line)
        if m:
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


def side_lines(t, h, groups):
    """side menu 那一欄的每一列（含上下框線，不含操作說明）：一個日曆一列「 ☑ 名字    件數 」"""
    iw = SIDE_WIDTH - 2
    rows = [ljust(t["side"], iw), "─" * iw]
    for name, count in groups:
        count = f" {count} "
        left = " ☑ " + (t["ungrouped"] if name is None else name)
        rows.append(ljust(cut(left, iw - len(count)), iw - len(count)) + count)
    rows = (rows + [" " * iw] * h)[: h - 3]
    return ["╭" + "─" * iw + "╮"] + ["│" + row + "│" for row in rows] + ["╰" + "─" * iw + "╯"]


def screen(lang, w, h, selected, first, skip, items, groups):
    t = TEXTS[lang]
    body_h = h - 5  # 框上下、表頭、表頭的線、操作說明
    sw = SIDE_WIDTH if w >= 120 and not SIDE_HIDDEN else 0  # side menu 的寬度
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


lang, w, h = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
selected = datetime.date.fromisoformat(sys.argv[4])
if len(sys.argv) > 6:
    first = tuple(int(part) for part in sys.argv[5].split("-"))
    skip = int(sys.argv[6])
else:
    first, skip = (selected.year, selected.month), 0
text = sys.stdin.read()
for line in screen(lang, w, h, selected, first, skip, read_items(text), read_groups(text)):
    assert width(line) == w, (width(line), line)
    print(f'            "{line.rstrip()}",')
