//! 執行時可以改的設定（開關、顏色）和設定頁（Option+q）的狀態
//!
//! 設定頁是一張清單：`j` / `k` 選一列，`h` / `l` 改那列的值（開關 on ↔ off、顏色換色票），
//! 畫面馬上跟著變。之後要存設定檔，存的就是 `Settings`（顏色在 `Theme`）

use crate::theme::{PALETTE, Theme};
use ratatui::style::Color;

/// 執行時可以改的開關；預設值在 `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// 大月曆的格子之間畫不畫淡淡的格線（關掉可以多 1 行 1 格放內容）
    pub grid_lines: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { grid_lines: true }
    }
}

/// 設定頁的一列：一個開關，或 `Theme` 的一個顏色欄位
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// `Settings::grid_lines`
    GridLines,
    /// `Theme` 的某個欄位（名字照欄位名），`h` / `l` 在 Catppuccin 色票裡換色
    Color(&'static str),
}

/// 設定頁的列，照顯示順序：開關在上面，顏色在下面
pub const ROWS: [Row; 13] = [
    Row::GridLines,
    Row::Color("item"),
    Row::Color("item_done"),
    Row::Color("item_more"),
    Row::Color("grid"),
    Row::Color("cell_selected"),
    Row::Color("cell_selected_day"),
    Row::Color("day"),
    Row::Color("other_month"),
    Row::Color("today"),
    Row::Color("title"),
    Row::Color("month_marker"),
    Row::Color("weekday_header"),
];

impl Row {
    /// 顯示在左邊的名字
    pub fn label(self) -> &'static str {
        match self {
            Self::GridLines => "grid lines",
            Self::Color(name) => name,
        }
    }

    /// 目前的值：開關回傳 `on` / `off`；顏色回傳色票名字和顏色本身（畫色塊用）
    pub fn value(self, settings: &Settings, theme: &Theme) -> (String, Option<Color>) {
        match self {
            Self::GridLines => (on_off(settings.grid_lines).to_string(), None),
            Self::Color(name) => {
                let current = current_color(name, theme);
                let label = PALETTE
                    .iter()
                    .find(|&&(_, color)| Some(color) == current)
                    .map_or("custom".to_string(), |&(name, _)| name.to_string());
                (label, current)
            }
        }
    }

    /// 改這列的值：開關直接切換；顏色往色票的前（`step` = −1）/ 後（+1）一格，頭尾相接
    pub fn adjust(self, step: isize, settings: &mut Settings, theme: &mut Theme) {
        match self {
            Self::GridLines => settings.grid_lines = !settings.grid_lines,
            Self::Color(name) => {
                let current = current_color(name, theme);
                let index = PALETTE
                    .iter()
                    .position(|&(_, color)| Some(color) == current)
                    .unwrap_or(0);
                let color = PALETTE[wrap(index, step, PALETTE.len())].1;
                let style = theme.field_mut(name);
                *style = if uses_bg(name) {
                    style.bg(color)
                } else {
                    style.fg(color)
                };
            }
        }
    }
}

/// 這個欄位調的是底色（整格塗色的那種）還是文字色
fn uses_bg(name: &str) -> bool {
    name == "cell_selected"
}

/// 欄位現在的顏色（底色或文字色，看 `uses_bg`）
fn current_color(name: &str, theme: &Theme) -> Option<Color> {
    // `field_mut` 要 `&mut`；Theme 是 Copy，複製一份來讀就好
    let mut theme = *theme;
    let style = theme.field_mut(name);
    if uses_bg(name) { style.bg } else { style.fg }
}

/// `true` → `on`、`false` → `off`
fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

/// `index + step`，超出 0..len 就從另一頭接回來
fn wrap(index: usize, step: isize, len: usize) -> usize {
    (index as isize + step).rem_euclid(len as isize) as usize
}

/// 設定頁開著時的狀態：現在選到第幾列
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Panel {
    /// `ROWS` 的 index
    pub selected: usize,
}

impl Panel {
    /// 現在選到的列
    pub fn row(self) -> Row {
        ROWS[self.selected]
    }

    /// 往下（`step` = +1）/ 往上（−1）選一列，頭尾相接
    pub fn select(&mut self, step: isize) {
        self.selected = wrap(self.selected, step, ROWS.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_lines_row_toggles() {
        let mut settings = Settings::default();
        let mut theme = Theme::default();
        assert_eq!(Row::GridLines.value(&settings, &theme).0, "on");
        Row::GridLines.adjust(1, &mut settings, &mut theme);
        assert!(!settings.grid_lines);
        assert_eq!(Row::GridLines.value(&settings, &theme).0, "off");
        // 往左也是切換
        Row::GridLines.adjust(-1, &mut settings, &mut theme);
        assert!(settings.grid_lines);
    }

    #[test]
    fn color_row_cycles_through_the_palette_and_writes_the_theme() {
        let mut settings = Settings::default();
        let mut theme = Theme::default();
        let row = Row::Color("item");
        assert_eq!(row.value(&settings, &theme).0, "subtext1");
        row.adjust(1, &mut settings, &mut theme);
        assert_eq!(row.value(&settings, &theme).0, "subtext0");
        assert_eq!(theme.item.fg, Some(PALETTE[16].1));
        // 往前兩格：subtext1 → text（頭尾相接在第一個色 rosewater 之前才會發生）
        row.adjust(-2, &mut settings, &mut theme);
        assert_eq!(row.value(&settings, &theme).0, "text");
        // 底色欄位改的是 bg，不是 fg
        let cell = Row::Color("cell_selected");
        cell.adjust(1, &mut settings, &mut theme);
        assert_eq!(theme.cell_selected.bg, Some(PALETTE[1].1));
        assert_eq!(theme.cell_selected.fg, None);
    }

    #[test]
    fn panel_selection_wraps_around() {
        let mut panel = Panel::default();
        assert_eq!(panel.row(), Row::GridLines);
        panel.select(-1);
        assert_eq!(panel.selected, ROWS.len() - 1);
        panel.select(1);
        assert_eq!(panel.selected, 0);
    }
}
