//! 執行時可以改的設定（開關、顏色）和設定頁（`,`）的狀態
//!
//! 設定頁是一張清單，跟設定檔一樣分成 `[calendar]`、`[month]`、`[colors]` 三段、名字一模一樣：
//! `j` / `k` 選一列，`h` / `l` 改那列的值（開關 on ↔ off、顏色換色票），
//! 畫面馬上跟著變；`r` 把那一列還原成預設；`R`（Shift+r）全部還原，要再按 `y` 確認；`q` / `Esc` 關閉。
//! 開關和改過的顏色啟動時從設定檔讀、設定頁關掉時存回去（`config.rs`）。
//! 畫面用的 `Theme` 一律從 `Settings` 算：預設 theme + `Settings.colors`（`Theme::with_colors`），兩邊不會不一致

use crate::theme::{ColorField, HexColor, PALETTE, Theme};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 執行時可以改的設定 = 設定檔的內容，結構一模一樣：`calendar` = `[calendar]`、`month` = `[month]`、`colors` = `[colors]`
///
/// 設定檔裡的名字就是欄位名（例：`[month]` 底下寫 `grid_lines = false`）。`serde` 的 `default`：沒寫的欄位用 `Default` 的值；
/// `deny_unknown_fields`：不認識的名字算錯誤，打錯字才不會被默默忽略。`Serialize`：存檔時反過來轉成 TOML。
/// 有 `BTreeMap`（會配置記憶體），所以不是 `Copy`，要複製一份得明確寫 `.clone()`
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// `[calendar]`：大小月曆共用的開關
    pub calendar: CalendarSettings,
    /// `[month]`：大月曆的開關
    pub month: MonthSettings,
    /// `[colors]`：改過的顏色（例：`sunday = "#f38ba8"`）；沒寫的用預設 theme 的。
    /// 只記跟預設不一樣的，之後換預設 theme，沒改過的顏色會跟著新的走
    pub colors: BTreeMap<ColorField, HexColor>,
}

impl Settings {
    /// 改一個顏色：跟預設 theme 一樣就從 `colors` 拿掉（設定檔只記改過的）；
    /// 只有 RGB 顏色存得進設定檔，其他的也當作沒改
    pub fn set_color(&mut self, field: ColorField, color: Color) {
        let is_default = field.color(&Theme::default()) == Some(color);
        match HexColor::from_color(color) {
            Some(hex) if !is_default => {
                self.colors.insert(field, hex);
            }
            _ => {
                self.colors.remove(&field);
            }
        }
    }
}

/// 大小月曆共用的開關（設定檔的 `[calendar]`）；預設值在 `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarSettings {
    /// 左邊欄：選取那週顯示週數、其他週顯示跟選取那週差幾週（像 vim 的 relativenumber）
    pub week_numbers: bool,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        Self { week_numbers: true }
    }
}

/// 大月曆的開關（設定檔的 `[month]`）；預設值在 `Default`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MonthSettings {
    /// 格子之間畫不畫淡淡的格線（關掉可以多 1 行 1 格放內容）
    pub grid_lines: bool,
    /// 每個項目前面要不要加一個點（`∙ 14:00 標題`）；之後想擴成假日 🌴、使用者自訂 emoji
    pub item_icons: bool,
    /// 移動時選取的那週固定在畫面正中間（像 vim 的 `scrolloff=999`）；關掉就自由移動，掉出畫面才捲。
    /// 小月曆不受影響（上下固定保留 1 週）
    pub center_cursor: bool,
}

impl Default for MonthSettings {
    fn default() -> Self {
        Self {
            grid_lines: false,
            item_icons: true,
            center_cursor: true,
        }
    }
}

/// 設定頁的一列：一個開關，或 `Theme` 的一個顏色欄位
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// `Settings::calendar.week_numbers`
    WeekNumbers,
    /// `Settings::month.grid_lines`
    GridLines,
    /// `Settings::month.item_icons`
    ItemIcons,
    /// `Settings::month.center_cursor`
    CenterCursor,
    /// `Theme` 的一個顏色欄位，`h` / `l` 在 Catppuccin 色票裡換色
    Color(ColorField),
}

/// 設定頁的列，照顯示順序：開關在上面，顏色在下面
pub const ROWS: [Row; 17] = [
    Row::WeekNumbers,
    Row::GridLines,
    Row::ItemIcons,
    Row::CenterCursor,
    Row::Color(ColorField::Item),
    Row::Color(ColorField::ItemDone),
    Row::Color(ColorField::MoreItems),
    Row::Color(ColorField::GridLines),
    Row::Color(ColorField::SelectedBg),
    Row::Color(ColorField::Day),
    Row::Color(ColorField::OtherMonth),
    Row::Color(ColorField::Today),
    Row::Color(ColorField::Title),
    Row::Color(ColorField::MonthLabel),
    Row::Color(ColorField::WeekdayHeader),
    Row::Color(ColorField::Saturday),
    Row::Color(ColorField::Sunday),
];

impl Row {
    /// 顯示在左邊的名字 = 設定檔裡的名字（開關的名字 = `MonthSettings` 的欄位名，有 test 釘住）
    pub fn label(self) -> &'static str {
        match self {
            Self::WeekNumbers => "week_numbers",
            Self::GridLines => "grid_lines",
            Self::ItemIcons => "item_icons",
            Self::CenterCursor => "center_cursor",
            Self::Color(field) => field.name(),
        }
    }

    /// 這一列在設定檔的哪個表（`[calendar]`、`[month]`、`[colors]`）；設定頁也照這個分段
    pub fn section(self) -> &'static str {
        match self {
            Self::WeekNumbers => "calendar",
            Self::GridLines | Self::ItemIcons | Self::CenterCursor => "month",
            Self::Color(_) => "colors",
        }
    }

    /// 目前的值：開關回傳 `on` / `off`；顏色回傳色票名字和顏色本身（畫色塊用）
    pub fn value(self, settings: &Settings, theme: &Theme) -> (String, Option<Color>) {
        match self {
            Self::WeekNumbers => (on_off(settings.calendar.week_numbers).to_string(), None),
            Self::GridLines => (on_off(settings.month.grid_lines).to_string(), None),
            Self::ItemIcons => (on_off(settings.month.item_icons).to_string(), None),
            Self::CenterCursor => (on_off(settings.month.center_cursor).to_string(), None),
            Self::Color(field) => {
                let current = field.color(theme);
                // 色票裡有就顯示色票名；沒有（例：設定檔手寫的色碼）就顯示色碼
                let label = PALETTE
                    .iter()
                    .find(|&&(_, color)| Some(color) == current)
                    .map(|&(name, _)| name.to_string())
                    .or_else(|| current.and_then(HexColor::from_color).map(String::from))
                    .unwrap_or_else(|| "custom".to_string());
                (label, current)
            }
        }
    }

    /// 改這列的值：開關直接切換；顏色往色票的前（`step` = −1）/ 後（+1）一格，頭尾相接
    pub fn adjust(self, step: isize, settings: &mut Settings, theme: &mut Theme) {
        match self {
            Self::WeekNumbers => {
                settings.calendar.week_numbers = !settings.calendar.week_numbers;
            }
            Self::GridLines => settings.month.grid_lines = !settings.month.grid_lines,
            Self::ItemIcons => settings.month.item_icons = !settings.month.item_icons,
            Self::CenterCursor => settings.month.center_cursor = !settings.month.center_cursor,
            Self::Color(field) => {
                let current = field.color(theme);
                let index = PALETTE
                    .iter()
                    .position(|&(_, color)| Some(color) == current)
                    .unwrap_or(0);
                settings.set_color(field, PALETTE[wrap(index, step, PALETTE.len())].1);
                // 畫面用的 theme 一律從 settings 重算
                *theme = Theme::with_colors(&settings.colors);
            }
        }
    }

    /// 把這一列還原成預設值（`Settings::default()` / `Theme::default()` 裡的那個）
    pub fn reset(self, settings: &mut Settings, theme: &mut Theme) {
        let defaults = Settings::default();
        match self {
            Self::WeekNumbers => settings.calendar.week_numbers = defaults.calendar.week_numbers,
            Self::GridLines => settings.month.grid_lines = defaults.month.grid_lines,
            Self::ItemIcons => settings.month.item_icons = defaults.month.item_icons,
            Self::CenterCursor => settings.month.center_cursor = defaults.month.center_cursor,
            Self::Color(field) => {
                settings.colors.remove(&field);
                *theme = Theme::with_colors(&settings.colors);
            }
        }
    }
}

/// `true` → `on`、`false` → `off`
fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

/// `index + step`，超出 0..len 就從另一頭接回來
fn wrap(index: usize, step: isize, len: usize) -> usize {
    (index as isize + step).rem_euclid(len as isize) as usize
}

/// 設定頁開著時的狀態：現在選到第幾列、是不是正在問「全部還原嗎？」
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Panel {
    /// `ROWS` 的 index
    pub selected: usize,
    /// 按了 `R`，正在等確認：下一個鍵是 `y` 才全部還原，其他鍵都是取消
    pub confirm_reset_all: bool,
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
        // 預設關
        assert_eq!(Row::GridLines.value(&settings, &theme).0, "off");
        Row::GridLines.adjust(1, &mut settings, &mut theme);
        assert!(settings.month.grid_lines);
        assert_eq!(Row::GridLines.value(&settings, &theme).0, "on");
        // 往左也是切換
        Row::GridLines.adjust(-1, &mut settings, &mut theme);
        assert!(!settings.month.grid_lines);
    }

    #[test]
    fn color_row_cycles_through_the_palette_and_writes_the_theme() {
        let mut settings = Settings::default();
        let mut theme = Theme::default();
        let row = Row::Color(ColorField::Item);
        assert_eq!(row.value(&settings, &theme).0, "subtext1");
        row.adjust(1, &mut settings, &mut theme);
        assert_eq!(row.value(&settings, &theme).0, "subtext0");
        assert_eq!(theme.item.fg, Some(PALETTE[16].1));
        // 往前兩格：subtext1 → text（頭尾相接在第一個色 rosewater 之前才會發生）
        row.adjust(-2, &mut settings, &mut theme);
        assert_eq!(row.value(&settings, &theme).0, "text");
        // 底色欄位改的是 bg，不是 fg：surface1（第 21 個）往後一格是 surface0
        let cell = Row::Color(ColorField::SelectedBg);
        cell.adjust(1, &mut settings, &mut theme);
        assert_eq!(theme.selected_bg.bg, Some(PALETTE[22].1));
        assert_eq!(theme.selected_bg.fg, None);
    }

    #[test]
    fn each_toggle_row_is_named_after_its_config_key() {
        // 存成設定檔的樣子（`[month]` 底下一個開關一行），跟預設比
        let default_file = toml::Table::try_from(Settings::default()).expect("serialize");
        let value = |file: &toml::Table, row: Row| {
            file.get(row.section())
                .and_then(|table| table.get(row.label()))
                .and_then(toml::Value::as_bool)
        };
        for row in ROWS {
            if let Row::Color(_) = row {
                continue;
            }
            let mut settings = Settings::default();
            row.adjust(1, &mut settings, &mut Theme::default());
            // 切換這一列之後，設定檔裡「[section] 底下名字 = label」的那個值就是變了的那個
            let file = toml::Table::try_from(settings).expect("serialize");
            assert!(value(&file, row).is_some(), "{row:?}: 設定檔裡沒有這個名字");
            assert_ne!(value(&file, row), value(&default_file, row), "{row:?}");
        }
    }

    #[test]
    fn each_color_field_uses_its_name_in_the_config_file() {
        for row in ROWS {
            let Row::Color(field) = row else { continue };
            // serde 轉出來的名字（設定檔 [colors] 的 key）= 設定頁顯示的名字
            let config_name = toml::Value::try_from(field).expect("serialize");
            assert_eq!(config_name.as_str(), Some(field.name()), "{field:?}");
        }
    }

    #[test]
    fn color_rows_write_settings_and_only_remember_changed_colors() {
        let mut settings = Settings::default();
        let mut theme = Theme::default();
        let row = Row::Color(ColorField::Item);
        // 往後一格：subtext1 → subtext0，記進 settings.colors，theme 跟著變
        row.adjust(1, &mut settings, &mut theme);
        assert_eq!(
            settings.colors.get(&ColorField::Item),
            HexColor::from_color(PALETTE[16].1).as_ref()
        );
        assert_eq!(theme, Theme::with_colors(&settings.colors));
        // 再往前一格回到預設的 subtext1 → 從 settings.colors 拿掉（設定檔只記改過的）
        row.adjust(-1, &mut settings, &mut theme);
        assert!(settings.colors.is_empty());
        // r：改過的拿掉，theme 回到預設
        row.adjust(1, &mut settings, &mut theme);
        row.reset(&mut settings, &mut theme);
        assert!(settings.colors.is_empty());
        assert_eq!(theme.item, Theme::default().item);
    }

    #[test]
    fn each_color_field_changes_the_theme_field_with_its_name() {
        // 一個特別的顏色，預設 theme 裡沒有
        let marker = Color::Rgb(1, 2, 3);
        let before = format!("{:#?}", Theme::default());
        for row in ROWS {
            let Row::Color(field) = row else { continue };
            let mut theme = Theme::default();
            field.set_color(&mut theme, marker);
            // `{:#?}` 一個欄位印一行（例：`saturday: Style::new().fg(...),`）；
            // 跟預設比，只有名字 = `field.name()` 的那一行會變
            let after = format!("{theme:#?}");
            let changed: Vec<&str> = after
                .lines()
                .zip(before.lines())
                .filter(|(after, before)| after != before)
                .map(|(after, _)| after.trim())
                .collect();
            assert_eq!(changed.len(), 1, "{field:?}: {changed:?}");
            assert!(
                changed[0].starts_with(&format!("{}: ", field.name())),
                "{field:?}: {changed:?}"
            );
        }
    }

    #[test]
    fn reset_restores_only_that_row() {
        let mut settings = Settings::default();
        let mut theme = Theme::default();
        Row::GridLines.adjust(1, &mut settings, &mut theme);
        Row::Color(ColorField::Item).adjust(1, &mut settings, &mut theme);
        Row::Color(ColorField::ItemDone).adjust(1, &mut settings, &mut theme);
        // 只還原 item：item_done 和 grid_lines 保持改過的值
        Row::Color(ColorField::Item).reset(&mut settings, &mut theme);
        assert_eq!(theme.item, Theme::default().item);
        assert_ne!(theme.item_done, Theme::default().item_done);
        assert!(settings.month.grid_lines);
        Row::GridLines.reset(&mut settings, &mut theme);
        assert!(!settings.month.grid_lines);
    }

    #[test]
    fn panel_selection_wraps_around() {
        let mut panel = Panel::default();
        assert_eq!(panel.row(), Row::WeekNumbers);
        panel.select(-1);
        assert_eq!(panel.selected, ROWS.len() - 1);
        panel.select(1);
        assert_eq!(panel.selected, 0);
    }
}
