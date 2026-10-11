mod app;
mod calendar;
mod config;
mod files;
mod i18n;
mod paths;
mod settings;
mod tasks;
mod theme;
mod ui;

use app::App;
use color_eyre::{
    Result,
    eyre::{WrapErr, bail},
};
use i18n::Lang;
use ratatui::crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use settings::Settings;
use std::io::{ErrorKind, stdout};
use std::path::{Path, PathBuf};
use tasks::Item;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    let args = parse_args(std::env::args().skip(1))?;
    // 語言：--lang 參數 → 系統語言 → 英文
    let lang = args.lang.unwrap_or_else(Lang::from_system);
    // 進 TUI 之前先讀設定檔和任務檔，讀不到才能正常印出錯誤
    // 設定檔：連 HOME 都沒有（找不到放設定檔的地方）就全部用預設值，也不存檔
    let config_path = paths::config_file();
    let settings = match &config_path {
        Some(path) => config::load(path)?,
        None => Settings::default(),
    };
    // 任務檔：有給 --tasks 就一定要讀得到；用預設位置時，檔案不存在就當作沒有任務
    let (tasks_path, required) = match args.tasks {
        Some(path) => (Some(path), true),
        None => (paths::tasks_file(), false),
    };
    let items = match &tasks_path {
        Some(path) => load_items(path, required)?,
        None => Vec::new(),
    };
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| {
        // 開著滑鼠的期間：_mouse 在 closure 結束時被 drop，滑鼠就自動關掉
        let _mouse = MouseCapture::enable()?;
        App::new(lang.texts(), items, tasks_path, settings, config_path).run(terminal)
    })
}

/// 開著滑鼠事件的期間；drop 時自動關掉（正常結束、出錯、panic 都一樣）
///
/// `ratatui::run` 只會還原 raw mode 和 alternate screen，滑鼠要自己關，
/// 不然離開後 terminal 還會一直收到滑鼠的控制碼
struct MouseCapture;

impl MouseCapture {
    /// 開啟滑鼠事件（滾輪、點擊）
    fn enable() -> Result<Self> {
        execute!(stdout(), EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for MouseCapture {
    fn drop(&mut self) {
        // drop 不能回傳錯誤；關不掉也沒辦法再做什麼，所以忽略
        let _ = execute!(stdout(), DisableMouseCapture);
    }
}

/// 命令列參數，例：`roh-cal --lang en --tasks ~/notes/tasks.md`
#[derive(Debug, Default, PartialEq)]
struct Args {
    /// `--lang`：沒給就跟著系統語言
    lang: Option<Lang>,
    /// `--tasks`：任務檔的路徑；沒給就用預設位置
    tasks: Option<PathBuf>,
}

/// 讀命令列參數（不含第 0 個的程式路徑）
///
/// 在進入 TUI 之前讀，參數錯了才能正常印出錯誤訊息
fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args> {
    const USAGE: &str = "usage: roh-cal [--lang en|zh-TW] [--tasks <path>]";
    let mut parsed = Args::default();
    while let Some(arg) = args.next() {
        // 每個參數後面都要接一個值
        let Some(value) = args.next() else {
            bail!("{arg} needs a value ({USAGE})");
        };
        match arg.as_str() {
            "--lang" => {
                let Some(lang) = Lang::from_code(&value) else {
                    bail!("unsupported language: {value} (supported: en, zh-TW)");
                };
                parsed.lang = Some(lang);
            }
            "--tasks" => parsed.tasks = Some(PathBuf::from(value)),
            _ => bail!("unknown argument: {arg} ({USAGE})"),
        }
    }
    Ok(parsed)
}

/// 讀任務檔；`required` = 使用者自己指定的檔案（`--tasks`），不存在要報錯；預設位置的檔案不存在就當作沒有任務
fn load_items(path: &Path, required: bool) -> Result<Vec<Item>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(tasks::parse(&text).items),
        Err(error) if !required && error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => {
            Err(error).wrap_err_with(|| format!("cannot read task file: {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把 `"--lang en"` 這樣的字串切成參數（test 用）
    fn args(line: &str) -> Result<Args> {
        parse_args(line.split_whitespace().map(String::from))
    }

    #[test]
    fn parse_args_reads_lang_and_tasks() {
        let parsed = args("--tasks testdata/tasks.md --lang zh-TW").expect("valid args");
        assert_eq!(parsed.lang, Some(Lang::ZhTw));
        assert_eq!(parsed.tasks, Some(PathBuf::from("testdata/tasks.md")));
        assert_eq!(args("").expect("no args"), Args::default());
    }

    #[test]
    fn parse_args_rejects_mistakes() {
        for line in ["--lang", "--tasks", "--lang ja", "--foo bar"] {
            assert!(args(line).is_err(), "{line}");
        }
    }

    #[test]
    fn missing_task_file_is_an_error_only_when_asked_for() {
        // 自己用 --tasks 指定的檔案不存在 → 錯誤
        let missing = Path::new("testdata/no-such-file.md");
        assert!(load_items(missing, true).is_err());
        // 預設位置的檔案不存在 → 沒有任務
        assert!(load_items(missing, false).expect("no tasks").is_empty());
        // 範例檔讀得到
        let items = load_items(Path::new("testdata/tasks.md"), true).expect("sample file");
        assert_eq!(items.len(), 56);
    }
}
