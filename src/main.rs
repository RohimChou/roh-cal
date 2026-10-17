//! roh-cal：terminal 月曆，讀純文字的任務檔

mod app;
mod calendar;
mod paths;
mod tasks;
mod texts;
mod theme;
mod ui;

use app::App;
use clap::Parser;
use color_eyre::{Result, eyre::WrapErr};
use jiff::Zoned;
use ratatui::crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use std::io::{ErrorKind, stdout};
use std::path::{Path, PathBuf};
use tasks::Item;
use texts::Lang;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 讀命令列參數：參數錯了、--help、--version 都在這裡印出來就結束（還沒進 TUI）
    let args = Args::parse();
    // 語言：--lang 參數 → 系統語言 → 英文
    let lang = args.lang.unwrap_or_else(Lang::from_system);
    // 任務檔：有給 --tasks 就一定要讀得到；用預設位置時，檔案不存在就當作沒有任務
    let (path, required) = match args.tasks {
        Some(path) => (Some(path), true),
        None => (paths::tasks_file(), false),
    };
    let items = match &path {
        Some(path) => load_items(path, required)?,
        None => Vec::new(),
    };
    // 今天：系統時區的日期
    let today = Zoned::now().date();
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| {
        // 開著滑鼠的期間：_mouse 在 closure 結束時被 drop，滑鼠就自動關掉
        let _mouse = MouseCapture::enable()?;
        App::new(lang, today, items).run(terminal)
    })
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

/// 命令列參數，例：`roh-cal --lang zh-TW --tasks ~/notes/tasks.md`；`--help` 由 clap 自動產生。
/// `--help` 印出的英文：第一行是 `Cargo.toml` 的 `description`（`about` 沒給值），參數的說明寫在 `help` 裡；
/// clap 預設拿 `///` 當說明，但這裡的 `///` 是給讀 code 的人看的
#[derive(Debug, Default, PartialEq, Parser)]
#[command(
    version,
    disable_version_flag = true,
    about,
    long_about = None
)]
struct Args {
    /// `--lang`：沒給就跟著系統語言
    #[arg(
        long,
        value_name = "en|zh-TW",
        value_parser = parse_lang,
        help = "Screen language [default: the system language, else en]"
    )]
    lang: Option<Lang>,
    /// `--tasks`：任務檔的路徑；沒給就用預設位置
    #[arg(
        long,
        value_name = "PATH",
        help = "Task file to show [default: ~/.local/share/roh-cal/tasks.md]"
    )]
    tasks: Option<PathBuf>,
    /// `-v` / `-V` / `--version`：印出版本就結束。clap 內建的版本參數只收 `-V`，
    /// 所以關掉內建的（`disable_version_flag`），自己加一個多收小寫 `-v`（`-V` 不顯示在 `--help`）
    #[arg(
        short = 'v',
        short_alias = 'V',
        long,
        action = clap::ArgAction::Version,
        help = "Print version"
    )]
    version: (),
}

/// `--lang` 的值換成 `Lang`；不支援的語言回傳錯誤（clap 會在前面加上是哪個參數、哪個值）
fn parse_lang(code: &str) -> Result<Lang, String> {
    Lang::from_code(code).ok_or_else(|| "supported: en, zh-TW".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    /// 把 `"--lang en"` 這樣的字串切成參數讀進來（test 用；前面補上第 0 個的程式名稱）
    fn args(line: &str) -> Result<Args, clap::Error> {
        Args::try_parse_from(std::iter::once("roh-cal").chain(line.split_whitespace()))
    }

    #[test]
    fn args_definition_is_valid() {
        // clap 建議每個 app 都有的檢查：參數的設定有沒有互相衝突（例：兩個參數同名）
        Args::command().debug_assert();
    }

    #[test]
    fn parse_args_reads_lang_and_tasks() {
        let parsed = args("--tasks testdata/tasks.md --lang zh-TW").expect("valid args");
        assert_eq!(parsed.lang, Some(Lang::ZhTw));
        assert_eq!(parsed.tasks, Some(PathBuf::from("testdata/tasks.md")));
        assert_eq!(args("--lang en").expect("valid").lang, Some(Lang::En));
        assert_eq!(args("").expect("no args"), Args::default());
    }

    #[test]
    fn parse_args_rejects_mistakes() {
        for line in ["--lang", "--tasks", "--lang ja", "--foo bar"] {
            assert!(args(line).is_err(), "{line}");
        }
    }

    #[test]
    fn help_and_version_print_instead_of_running() {
        // --help / -h、--version / -v / -V：clap 印出說明或版本就結束（用 Err 回報，main 裡的 parse() 會印出來再結束）
        for (line, kind) in [
            ("--help", ErrorKind::DisplayHelp),
            ("-h", ErrorKind::DisplayHelp),
            ("--version", ErrorKind::DisplayVersion),
            ("-v", ErrorKind::DisplayVersion),
            ("-V", ErrorKind::DisplayVersion),
        ] {
            assert_eq!(args(line).expect_err(line).kind(), kind, "{line}");
        }
        // 說明是英文：第一行是 Cargo.toml 的 description，有 --lang 的說明，沒有中文的 `///` 註解
        let help = Args::command().render_long_help().to_string();
        assert!(
            help.starts_with("A simple, keyboard-driven terminal calendar for plain-text tasks\n")
        );
        assert!(help.contains("Screen language") && help.contains("Task file to show"));
        assert!(!help.contains("系統語言") && !help.contains("命令列參數"));
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
