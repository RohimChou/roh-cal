//! roh-cal：terminal 月曆，讀純文字的任務檔

use color_eyre::Result;

fn main() -> Result<()> {
    // 出錯時印出好讀的錯誤訊息與 backtrace
    color_eyre::install()?;
    println!("Hello, world!");
    Ok(())
}
