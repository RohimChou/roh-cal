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
    /// 週數
    pub week_number: Color,
    /// 六、日的日期和表頭
    pub weekend: Color,
    /// 今天的日期
    pub today: Color,
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

        Self {
            text,
            muted: subtext0,
            line: surface2,
            year_line: lavender,
            label: lavender,
            week_number: yellow,
            weekend: blue,
            today: green,
        }
    }
}
