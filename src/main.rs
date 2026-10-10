mod app;
mod calendar;
mod i18n;
#[expect(dead_code, reason = "接到畫面之前，parser 只有 test 在用")]
mod tasks;
mod theme;
mod ui;

use app::App;
use color_eyre::{Result, eyre::bail};
use i18n::Lang;
use ratatui::crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use std::io::stdout;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    // 語言：--lang 參數 → 系統語言 → 英文
    let lang = match lang_arg()? {
        Some(lang) => lang,
        None => Lang::from_system(),
    };
    // 進入 TUI 模式（raw mode + alternate screen），closure 結束後自動還原 terminal
    ratatui::run(|terminal| {
        // 開著滑鼠的期間：_mouse 在 closure 結束時被 drop，滑鼠就自動關掉
        let _mouse = MouseCapture::enable()?;
        App::new(lang.texts()).run(terminal)
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

/// 讀命令列參數 `--lang <語言>`；沒給就回傳 `None`，例：`roh-cal --lang en`
///
/// 在進入 TUI 之前讀，參數錯了才能正常印出錯誤訊息
fn lang_arg() -> Result<Option<Lang>> {
    // 第 0 個是程式自己的路徑，跳過
    let mut args = std::env::args().skip(1);
    let mut lang = None;
    while let Some(arg) = args.next() {
        if arg != "--lang" {
            bail!("unknown argument: {arg} (usage: roh-cal [--lang en|zh-TW])");
        }
        let Some(code) = args.next() else {
            bail!("--lang needs a value, e.g. --lang en");
        };
        let Some(parsed) = Lang::from_code(&code) else {
            bail!("unsupported language: {code} (supported: en, zh-TW)");
        };
        lang = Some(parsed);
    }
    Ok(lang)
}
