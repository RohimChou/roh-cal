//! roh-cal v2：terminal 月曆，讀純文字的任務檔

mod app;
mod calendar;
mod items;
mod paths;
mod planner;
mod tasks;
mod texts;
mod theme;

use app::App;
use clap::Parser;
use color_eyre::{Result, eyre::WrapErr};
use jiff::Zoned;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 讀命令列參數：參數錯了、--help、--version 都在這裡印出來就結束（還沒進 TUI）
    let args = Args::parse();
    // 任務檔：有給 --tasks 就一定要讀得到；用預設位置時，檔案不存在就當作沒有任務
    let (path, required) = match args.tasks {
        Some(path) => (Some(path), true),
        None => (paths::tasks_file(), false),
    };
    let parsed = match &path {
        Some(path) => load_tasks(path, required)?,
        None => Vec::new(),
    };
    let groups = items::groups(&parsed);
    let items = items::from_tasks(&parsed);
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| App::new(Zoned::now().date(), items, groups).run(terminal))
}

/// 命令列參數，例：`roh-cal --tasks ~/notes/tasks.md`；`--help` 由 clap 自動產生（第一行是 `Cargo.toml` 的 `description`）
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// `--tasks`：任務檔的路徑；沒給就用預設位置
    #[arg(
        long,
        value_name = "PATH",
        help = "Task file to show [default: ~/.local/share/roh-cal/tasks.md]"
    )]
    tasks: Option<PathBuf>,
}

/// 讀任務檔；`required` = 使用者自己指定的檔案（`--tasks`），不存在要報錯；預設位置的檔案不存在就當作沒有任務
fn load_tasks(path: &Path, required: bool) -> Result<Vec<tasks::Item>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(tasks::parse(&text).items),
        Err(error) if !required && error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => {
            Err(error).wrap_err_with(|| format!("cannot read task file: {}", path.display()))
        }
    }
}
