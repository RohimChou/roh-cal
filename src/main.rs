//! roh-cal：terminal 月曆，讀純文字的任務檔

mod app;
mod calendar;
mod texts;
mod ui;

use app::App;
use clap::Parser;
use color_eyre::Result;
use jiff::Zoned;
use texts::Lang;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 讀命令列參數：參數錯了、--help、--version 都在這裡印出來就結束（還沒進 TUI）
    let args = Args::parse();
    // 語言：--lang 參數 → 系統語言 → 英文
    let lang = args.lang.unwrap_or_else(Lang::from_system);
    // 今天：系統時區的日期
    let today = Zoned::now().date();
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| App::new(lang, today).run(terminal))
}

/// 命令列參數，例：`roh-cal --lang zh-TW`；`--help` 由 clap 自動產生。
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
    fn parse_args_reads_lang() {
        assert_eq!(args("--lang zh-TW").expect("valid").lang, Some(Lang::ZhTw));
        assert_eq!(args("--lang en").expect("valid").lang, Some(Lang::En));
        assert_eq!(args("").expect("no args"), Args::default());
    }

    #[test]
    fn parse_args_rejects_mistakes() {
        for line in ["--lang", "--lang ja", "--foo bar"] {
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
        assert!(help.contains("Screen language"));
        assert!(!help.contains("系統語言") && !help.contains("命令列參數"));
    }
}
