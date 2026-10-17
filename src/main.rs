//! roh-cal v2：先做 mock，試各種 UI 的樣子（資料是寫死的假資料）

mod app;
mod calendar;
mod mock;
mod planner;
mod theme;

use app::App;
use color_eyre::Result;
use jiff::Zoned;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| App::new(Zoned::now().date()).run(terminal))
}
