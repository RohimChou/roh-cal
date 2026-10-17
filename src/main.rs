//! roh-cal：terminal 月曆，讀純文字的任務檔

mod app;

use app::App;
use color_eyre::Result;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| App::default().run(terminal))
}
