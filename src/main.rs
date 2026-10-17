//! roh-cal：terminal 月曆，讀純文字的任務檔

mod app;
mod texts;
mod ui;

use app::App;
use color_eyre::Result;
use texts::Lang;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 語言：跟著系統，讀不到或不支援就用英文
    let lang = Lang::from_system();
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| App::new(lang).run(terminal))
}
