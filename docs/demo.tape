# README 的 GIF：用 vhs（https://github.com/charmbracelet/vhs）錄
#
#   brew install vhs
#   cargo build --release
#   vhs docs/demo.tape        # 在 repo 最上層跑，產生 docs/demo.gif
#
# 用複製出來的範例檔、暫存的設定資料夾，不會碰到真的任務檔和設定檔。
# 範例檔的事在 2026 年 10 月左右；在別的時間錄，畫面會停在錄的那天，先按 [ ] 換到 10 月再錄比較好看（改下面的按鍵）

Output docs/demo.gif

Set Shell "bash"
Set FontSize 16
# 夠寬才有左邊的日曆清單（至少 120 格）
Set Width 1400
Set Height 760
Set Padding 12
Set Theme "Catppuccin Mocha"
Set TypingSpeed 120ms

Hide
Type "cp testdata/tasks.md /tmp/roh-cal-demo.md && export XDG_CONFIG_HOME=$(mktemp -d) && clear"
Enter
Show

Type "./target/release/roh-cal --tasks /tmp/roh-cal-demo.md"
Sleep 500ms
Enter
Sleep 2s

# 版型 1：月曆往下幾週、Tab 到右邊選事、打勾、復原
Type "jj"
Sleep 1s
Tab
Sleep 500ms
Type "jj"
Sleep 1s
Space
Sleep 1s
Type "u"
Sleep 1.5s

# 版型 2：大月曆，Enter 打開那天的事
Type "2"
Sleep 2s
Enter
Sleep 2s
Escape
Sleep 1s

# 任務頁：全部展開、往下看、全部收起
Type "3"
Sleep 1.5s
Type "+"
Sleep 1s
Type "jjjj"
Sleep 1.5s
Type "-"
Sleep 1s

# 說明頁
Type "?"
Sleep 2.5s
Type "q"
Sleep 500ms

Type "q"
Sleep 500ms
