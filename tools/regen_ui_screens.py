"""用 `tools/expected_screen.py` 重算 `src/ui.rs` 裡所有畫面 test 的預期畫面，直接改寫 `src/ui.rs`

用法（在 repo 最上層）：python3 tools/regen_ui_screens.py
改了畫面之後跑一次，再用 `git diff src/ui.rs` 看預期畫面變了哪裡、是不是你要的。
新增畫面 test 時，要在下面的 `PLAN` 加一筆（test 名字 → 每個預期畫面的參數，照在 test 裡出現的順序）。
"""
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GENERATOR = ROOT / "tools" / "expected_screen.py"
AGENDA = (ROOT / "testdata" / "agenda.md").read_text()
UI = ROOT / "src" / "ui.rs"

# test 名字 → [(expected_screen.py 的參數, 用不用 testdata/agenda.md, 環境變數)]
PLAN = {
    "calendar_from_this_month_down_with_a_thick_line_between_years": [(["zh", "35", "26", "2026-10-17"], False, {})],
    "calendar_in_english": [(["en", "35", "11", "2026-10-17"], False, {})],
    "selection_brackets_and_the_month_on_top": [(["en", "35", "8", "2026-11-02"], False, {}), (["zh", "35", "8", "2027-01-03", "2026-12", "5"], False, {})],
    "calendar_scrolls_down_only_when_the_selection_gets_close_to_the_bottom": [(["en", "35", "11", "2026-11-07", "2026-10", "4"], False, {}), (["en", "35", "11", "2026-10-31", "2026-10", "2"], False, {})],
    "calendar_scrolls_up_into_earlier_months": [(["zh", "35", "11", "2026-10-10", "2026-09", "5"], False, {})],
    "agenda_lists_each_months_items_next_to_it": [(["zh", "70", "23", "2026-10-17"], True, {})],
    "agenda_flows_into_columns_when_wide": [(["en", "95", "15", "2026-10-17"], True, {})],
    "agenda_focus_brackets_the_chosen_item": [(["zh", "70", "23", "2026-10-17"], True, {"CHOSEN": "5"})],
    "a_capped_month_scrolls_inside_while_choosing_items": [(["zh", "70", "11", "2026-10-20"], True, {"CHOSEN": "7", "WINDOW": "3"})],
    "the_agenda_follows_the_calendar": [(["zh", "70", "11", "2026-10-20"], True, {"WINDOW": "3"})],
    "a_hint_tells_how_many_more_items_a_month_has": [(["zh", "70", "11", "2026-10-17"], True, {"WINDOW": "2"})],
}


def generate(args, with_agenda, extra):
    """跑一次 expected_screen.py，回傳每一列（Rust 字串的寫法，前後空白去掉）"""
    env = {k: v for k, v in os.environ.items() if k not in ("CHOSEN", "WINDOW")}
    env.update(extra)
    out = subprocess.run(["python3", str(GENERATOR), *args], input=AGENDA if with_agenda else "",
                         capture_output=True, text=True, check=True, env=env).stdout
    return [line.strip() for line in out.splitlines()]


def main():
    src = UI.read_text()
    for name, screens in PLAN.items():
        start = src.index(f"    fn {name}() {{")
        end = src.find("\n    #[test]", start)
        end = len(src) if end < 0 else end
        lines = src[start:end].split("\n")
        # 連續的字串列（`"...",`）= 一個預期畫面
        blocks, i = [], 0
        while i < len(lines):
            text = lines[i].strip()
            if text.startswith('"') and text.endswith('",'):
                j = i
                while j < len(lines) and lines[j].strip().startswith('"') and lines[j].strip().endswith('",'):
                    j += 1
                blocks.append((i, j))
                i = j
            else:
                i += 1
        assert len(blocks) == len(screens), f"{name}：test 裡有 {len(blocks)} 個畫面，PLAN 寫了 {len(screens)} 個"
        for (i, j), (args, with_agenda, extra) in reversed(list(zip(blocks, screens))):
            indent = lines[i][: len(lines[i]) - len(lines[i].lstrip())]
            lines[i:j] = [indent + line for line in generate(args, with_agenda, extra)]
        src = src[:start] + "\n".join(lines) + src[end:]
    UI.write_text(src)
    print(f"重算了 {sum(len(v) for v in PLAN.values())} 個畫面：{UI.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
