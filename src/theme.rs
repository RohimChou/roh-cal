//! 顏色：畫面只講「角色」（今天、週末、週數…），不講顏色名；換 theme = 整組換掉

use ratatui::style::Color;

/// 一套 theme：每個角色各配一個顏色
#[derive(Debug)]
pub struct Theme {
    /// 一般的字：平日的日期
    pub text: Color,
    /// 次要的字：表頭、最下面的操作說明
    pub muted: Color,
    /// 外框、欄和欄之間、月和月之間的線
    pub line: Color,
    /// 跨年的粗線
    pub year_line: Color,
    /// 標題、左邊的年 / 月
    pub label: Color,
    /// 選取那一週的週數
    pub week_number: Color,
    /// 其他週的週數（比較暗）
    pub week_number_dim: Color,
    /// 六、日的日期和表頭
    pub weekend: Color,
    /// 有還沒結束的事的日期
    pub has_item: Color,
    /// 假日（假日日曆裡的事的日子）的日期
    pub holiday: Color,
    /// 今天的日期
    pub today: Color,
    /// 選取的日期：前後的括號 `[17]`
    pub selected: Color,
    /// 月曆上選取那一週那一列、右邊選到的那件事的底色（像 vim 的 cursorline）
    pub cursor_line: Color,
    /// 鍵盤在的那個框的框線
    pub focus: Color,
    /// side menu 上勾選的日曆的 `☑`
    pub checked: Color,
    /// 最下面一列暫時換成的提示（寫檔失敗、重新載入）
    pub notice: Color,
    /// 大月曆（版型 3）格子之間的淡格線
    pub grid: Color,
    /// 大月曆（版型 3）不是選取那個月的日子：日期和事都變灰
    pub other_month: Color,
}

impl Theme {
    /// [Catppuccin Mocha](https://catppuccin.com/palette)：暗色背景用的柔和配色（跟 v1 的預設一樣）
    pub fn catppuccin_mocha() -> Self {
        // 色票：https://github.com/catppuccin/palette 的 mocha
        let text = Color::Rgb(205, 214, 244);
        let subtext0 = Color::Rgb(166, 173, 200);
        let surface2 = Color::Rgb(88, 91, 112);
        let lavender = Color::Rgb(180, 190, 254);
        let blue = Color::Rgb(137, 180, 250);
        let green = Color::Rgb(166, 227, 161);
        let yellow = Color::Rgb(249, 226, 175);
        // 黃色和底色（base 30, 30, 46）7:3 混出來的暗黃：看得出是黃色，但比選取那週暗
        let dim_yellow = Color::Rgb(183, 167, 136);
        let red = Color::Rgb(243, 139, 168);
        let surface0 = Color::Rgb(49, 50, 68);
        let overlay0 = Color::Rgb(108, 112, 134);
        let mauve = Color::Rgb(203, 166, 247);
        let peach = Color::Rgb(250, 179, 135);

        Self {
            text,
            muted: subtext0,
            line: surface2,
            year_line: lavender,
            label: lavender,
            week_number: yellow,
            week_number_dim: dim_yellow,
            weekend: blue,
            has_item: peach,
            holiday: red,
            today: green,
            selected: red,
            cursor_line: surface0,
            focus: mauve,
            checked: mauve,
            notice: yellow,
            // 格線、其他月份：跟 v1 一樣
            grid: surface0,
            other_month: overlay0,
        }
    }
}
