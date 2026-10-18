"""Recompute the expected screens of every screen test in `src/ui.rs` with `tools/expected_screen.py`,
and rewrite `src/ui.rs` in place

Usage (from the repo root): python3 tools/regen_ui_screens.py
Run it once after changing the screen, then use `git diff src/ui.rs` to see what changed in the expected
screens and whether that is what you want.
When you add a screen test, add an entry to `PLAN` below (test name → arguments of each expected screen,
in the order they appear in the test).
"""
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GENERATOR = ROOT / "tools" / "expected_screen.py"
AGENDA = (ROOT / "testdata" / "agenda.md").read_text()
UI = ROOT / "src" / "ui.rs"

# test name → [(expected_screen.py arguments,
#   task file (True = testdata/agenda.md, a file name = that file in testdata/, False = none), env vars)]
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
    "side_menu_lists_the_calendars_when_wide": [(["zh", "121", "14", "2026-10-17"], True, {"WINDOW": "1"}), (["zh", "121", "14", "2026-10-17"], True, {"SIDE_HIDDEN": "1"})],
    "unchecked_calendars_hide_their_items": [(["en", "121", "14", "2026-10-17"], True, {"HIDDEN": "1"})],
    "multi_day_items_are_listed_once_per_month": [(["zh", "70", "30", "2026-10-17"], "ranges.md", {}), (["zh", "70", "30", "2026-11-02", "2026-10", "5"], "ranges.md", {"CHOSEN": "4"})],
    "the_agenda_header_keeps_the_whole_date_when_narrow": [(["zh", "56", "10", "2026-10-17"], False, {}), (["zh", "57", "10", "2026-10-17"], False, {})],
    "month_view_fills_the_space_with_six_equal_weeks": [(["zh", "80", "30", "2026-10-17"], False, {"LAYOUT": "3"}), (["en", "80", "28", "2026-10-17"], False, {"LAYOUT": "3"})],
    "month_view_lists_each_days_items_in_its_cell": [(["zh", "80", "30", "2026-10-17"], "month.md", {"LAYOUT": "3"}), (["en", "80", "24", "2026-10-17"], "month.md", {"LAYOUT": "3"}), (["zh", "80", "16", "2026-10-17"], "month.md", {"LAYOUT": "3"})],
    "month_view_with_the_side_menu_hides_unchecked_calendars": [(["zh", "121", "22", "2026-10-17"], "month.md", {"LAYOUT": "3"}), (["zh", "121", "22", "2026-10-17"], "month.md", {"LAYOUT": "3", "HIDDEN": "1"})],
    "month_view_without_grid_lines_gives_the_cells_more_room": [(["zh", "80", "24", "2026-10-17"], "month.md", {"LAYOUT": "3", "NO_GRID": "1"})],
    "month_view_without_item_icons": [(["en", "80", "24", "2026-10-17"], "month.md", {"LAYOUT": "3", "NO_ICONS": "1"})],
    "month_view_keeps_the_selected_week_in_the_middle": [(["zh", "80", "16", "2026-10-17"], False, {"LAYOUT": "3", "CENTER": "1"}), (["zh", "80", "16", "2026-11-02"], False, {"LAYOUT": "3", "CENTER": "1"})],
    "relative_week_numbers_in_both_layouts": [(["zh", "35", "14", "2026-10-17"], False, {"RELATIVE": "1"}), (["zh", "80", "16", "2026-10-17"], False, {"LAYOUT": "3", "RELATIVE": "1"})],
    "month_view_lists_a_multi_day_item_on_each_of_its_days": [(["zh", "80", "30", "2026-10-17"], "ranges.md", {"LAYOUT": "3"})],
    "tasks_page_lists_the_file_as_a_tree": [(["zh", "70", "30", "2026-10-17"], "tasks.md", {"LAYOUT": "4"}), (["en", "70", "16", "2026-10-17"], "tasks.md", {"LAYOUT": "4", "EXPANDED": "6,59,60", "CHOSEN_LINE": "61"})],
    "tasks_page_points_at_format_mistakes": [(["zh", "70", "16", "2026-10-17"], "tasks.md", {"LAYOUT": "4", "CHOSEN_LINE": "43"})],
    "tasks_page_with_the_side_menu": [(["zh", "121", "14", "2026-10-17"], "tasks.md", {"LAYOUT": "4", "HIDDEN": "0,1"})],
    "month_view_scrolls_a_week_at_a_time": [(["en", "80", "16", "2026-11-02", "2026-09-21"], False, {"LAYOUT": "3"}), (["zh", "80", "16", "2026-09-20", "2026-09-28"], False, {"LAYOUT": "3"})],
}


def generate(args, data, extra):
    """Run expected_screen.py once and return each row (written as Rust strings, with outer whitespace stripped)"""
    # Pass only PATH and this screen's env vars, so anything set outside (e.g. a LAYOUT left over from
    # the last run) does not leak in
    env = {"PATH": os.environ["PATH"], **extra}
    if isinstance(data, str):
        text = (ROOT / "testdata" / data).read_text()
    else:
        text = AGENDA if data else ""
    out = subprocess.run(["python3", str(GENERATOR), *args], input=text,
                         capture_output=True, text=True, check=True, env=env).stdout
    return [line.strip() for line in out.splitlines()]


def main():
    src = UI.read_text()
    for name, screens in PLAN.items():
        start = src.index(f"    fn {name}() {{")
        end = src.find("\n    #[test]", start)
        end = len(src) if end < 0 else end
        lines = src[start:end].split("\n")
        # A run of string lines (`"...",`) = one expected screen
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
        for (i, j), (args, data, extra) in reversed(list(zip(blocks, screens))):
            indent = lines[i][: len(lines[i]) - len(lines[i].lstrip())]
            lines[i:j] = [indent + line for line in generate(args, data, extra)]
        src = src[:start] + "\n".join(lines) + src[end:]
    UI.write_text(src)
    print(f"重算了 {sum(len(v) for v in PLAN.values())} 個畫面：{UI.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
