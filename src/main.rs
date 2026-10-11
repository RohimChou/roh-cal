mod app;
mod calendar;
mod i18n;
mod settings;
#[expect(
    dead_code,
    reason = "畫面目前只用到日期，其他欄位（分組、行號…）之後的任務欄、寫入才會用"
)]
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
use std::io::{ErrorKind, stdout};
use std::path::PathBuf;
use tasks::Item;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    let args = parse_args(std::env::args().skip(1))?;
    // 語言：--lang 參數 → 系統語言 → 英文
    let lang = args.lang.unwrap_or_else(Lang::from_system);
    // 進 TUI 之前先讀任務檔，讀不到才能正常印出錯誤
    let items = load_items(args.tasks)?;
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| {
        // 開著滑鼠的期間：_mouse 在 closure 結束時被 drop，滑鼠就自動關掉
        let _mouse = MouseCapture::enable()?;
        App::new(lang.texts(), items).run(terminal)
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

/// 任務檔的預設位置：`$XDG_DATA_HOME/roh-cal/tasks.md`，沒設就是 `~/.local/share/roh-cal/tasks.md`
fn default_tasks_path() -> Option<PathBuf> {
    let data_home = match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".local/share"),
    };
    Some(data_home.join("roh-cal").join("tasks.md"))
}

/// 讀任務檔：有給 `--tasks` 就一定要讀得到；用預設位置時，檔案不存在就當作沒有任務
fn load_items(tasks: Option<PathBuf>) -> Result<Vec<Item>> {
    let (path, required) = match tasks {
        Some(path) => (path, true),
        None => match default_tasks_path() {
            Some(path) => (path, false),
            None => return Ok(Vec::new()),
        },
    };
    match std::fs::read_to_string(&path) {
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
        assert!(load_items(Some(PathBuf::from("testdata/no-such-file.md"))).is_err());
        // 範例檔讀得到
        let items = load_items(Some(PathBuf::from("testdata/tasks.md"))).expect("sample file");
        assert_eq!(items.len(), 56);
    }
}
