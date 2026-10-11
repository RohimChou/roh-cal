//! 執行時可以改的設定（開關、顏色）和設定頁（Option+q）的狀態
//!
//! 設定頁是一張清單：`j` / `k` 選一列，`h` / `l` 改那列的值（開關 on ↔ off、顏色換色票），
//! 畫面馬上跟著變；`r` 把那一列還原成預設；`q` / `Esc` 關閉。
//! 開關啟動時從設定檔讀、設定頁關掉時存回去（`config.rs`）；顏色在 `Theme`，還沒進設定檔

use crate::theme::{PALETTE, Theme};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// 執行時可以改的開關；預設值在 `Default`
///
/// 設定檔裡的名字就是欄位名（例：`grid_lines = false`）。`serde` 的 `default`：沒寫的欄位用 `Default` 的值；
/// `deny_unknown_fields`：不認識的名字算錯誤，打錯字才不會被默默忽略。`Serialize`：存檔時反過來轉成 TOML
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// 大月曆的格子之間畫不畫淡淡的格線（關掉可以多 1 行 1 格放內容）
    pub grid_lines: bool,
    /// 大月曆的每個項目前面要不要加一個點（`∙ 14:00 標題`）；之後想擴成假日 🌴、使用者自訂 emoji
    pub item_icons: bool,
    /// 大月曆移動時選取的那週固定在畫面正中間（像 vim 的 `scrolloff=999`）；關掉就自由移動，掉出畫面才捲。
    /// 小月曆不受影響（上下固定保留 1 週）
    pub center_cursor: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            grid_lines: true,
            item_icons: true,
            center_cursor: true,
        }
    }
}

/// 設定頁的一列：一個開關，或 `Theme` 的一個顏色欄位
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// `Settings::grid_lines`
    GridLines,
    /// `Settings::item_icons`
    ItemIcons,
    /// `Settings::center_cursor`
    CenterCursor,
    /// `Theme` 的某個欄位（名字照欄位名），`h` / `l` 在 Catppuccin 色票裡換色
    Color(&'static str),
}

/// 設定頁的列，照顯示順序：開關在上面，顏色在下面
pub const ROWS: [Row; 16] = [
    Row::GridLines,
    Row::ItemIcons,
    Row::CenterCursor,
    Row::Color("item"),
    Row::Color("item_done"),
    Row::Color("item_more"),
    Row::Color("grid"),
    Row::Color("cell_selected"),
    Row::Color("day"),
    Row::Color("other_month"),
    Row::Color("today"),
    Row::Color("title"),
    Row::Color("month_marker"),
    Row::Color("weekday_header"),
    Row::Color("saturday"),
    Row::Color("sunday"),
];

impl Row {
    /// 顯示在左邊的名字
    pub fn label(self) -> &'static str {
        match self {
            Self::GridLines => "grid lines",
            Self::ItemIcons => "item icons",
            Self::CenterCursor => "center cursor",
            Self::Color(name) => name,
        }
    }

    /// 目前的值：開關回傳 `on` / `off`；顏色回傳色票名字和顏色本身（畫色塊用）
    pub fn value(self, settings: &Settings, theme: &Theme) -> (String, Option<Color>) {
        match self {
            Self::GridLines => (on_off(settings.grid_lines).to_string(), None),
            Self::ItemIcons => (on_off(settings.item_icons).to_string(), None),
            Self::CenterCursor => (on_off(settings.center_cursor).to_string(), None),
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
            Self::ItemIcons => settings.item_icons = !settings.item_icons,
            Self::CenterCursor => settings.center_cursor = !settings.center_cursor,
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

    /// 把這一列還原成預設值（`Settings::default()` / `Theme::default()` 裡的那個）
    pub fn reset(self, settings: &mut Settings, theme: &mut Theme) {
        let defaults = Settings::default();
        let mut default_theme = Theme::default();
        match self {
            Self::GridLines => settings.grid_lines = defaults.grid_lines,
            Self::ItemIcons => settings.item_icons = defaults.item_icons,
            Self::CenterCursor => settings.center_cursor = defaults.center_cursor,
            Self::Color(name) => *theme.field_mut(name) = *default_theme.field_mut(name),
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
        // 底色欄位改的是 bg，不是 fg：surface1（第 21 個）往後一格是 surface0
        let cell = Row::Color("cell_selected");
        cell.adjust(1, &mut settings, &mut theme);
        assert_eq!(theme.cell_selected.bg, Some(PALETTE[22].1));
        assert_eq!(theme.cell_selected.fg, None);
    }

    #[test]
    fn reset_restores_only_that_row() {
        let mut settings = Settings::default();
        let mut theme = Theme::default();
        Row::GridLines.adjust(1, &mut settings, &mut theme);
        Row::Color("item").adjust(1, &mut settings, &mut theme);
        Row::Color("item_done").adjust(1, &mut settings, &mut theme);
        // 只還原 item：item_done 和 grid lines 保持改過的值
        Row::Color("item").reset(&mut settings, &mut theme);
        assert_eq!(theme.item, Theme::default().item);
        assert_ne!(theme.item_done, Theme::default().item_done);
        assert!(!settings.grid_lines);
        Row::GridLines.reset(&mut settings, &mut theme);
        assert!(settings.grid_lines);
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
