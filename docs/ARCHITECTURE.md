# roh-cal 架構說明
給要改 roh-cal（v2）的工程師或 AI agent：動手前先讀這篇，再看 code。怎麼用見 [README](../README.md)，任務檔格式見 [task-file-format.md](task-file-format.md)。

## 跑起來
```sh
cargo run --release -- --tasks testdata/tasks.md    # 用範例任務檔打開
cargo test                                          # 全部 test
cargo fmt && cargo clippy --all-targets -- -D warnings
```

## 模組地圖
| 檔案 | 做什麼 | 畫畫面 |
| :- | :- | :- |
| `src/main.rs` | 讀命令列參數、讀任務檔、開 / 關 TUI 和滑鼠 | |
| `src/app.rs` | 程式的狀態 `App`、主迴圈、按鍵和滑鼠；版型（`View`：`1` 版型 1、`3` 版型 3） | |
| `src/tasks.rs` | 任務檔 parser：純文字 → `Item` | |
| `src/edit.rs` | 改任務檔的純 function：檔案現在的內容 + 要改的那件 → 改好的內容；只動那一行，那一行在別處改過就回傳 `Changed` | |
| `src/files.rs` | 安全寫檔：先寫暫存檔再 rename（symlink 寫進真正的檔案） | |
| `src/app/editor.rs` | 一行編輯框（`e`）：直接改那一行的文字（tui-input），`edit::check` 即時檢查，有問題就存不了（`App::editor`） | |
| `src/ui/editor.rs` | 畫編輯框：最下面、整個 terminal 寬，下面那列是即時檢查的結果 | ✓ |
| `src/app/moving.rs` | 改期（`r`）：月曆就是選日期的地方；選取的日期移了幾天，那件事就搬幾天（`App::moving`） | |
| `src/app/write.rs` | 寫任務檔的流程：先重讀、改、寫；在別處改過就重新載入；結果放在 `App::notice`（最下面一列） | |
| `src/calendar.rs` | 日期計算：一個月有哪幾週、第幾週 | |
| `src/texts.rs` | 畫面上的字（英文 / 繁中） | |
| `src/theme.rs` | 顏色：每個角色配一個顏色 | |
| `src/ui.rs` | 把畫面切成 side menu（terminal 至少 120 格寬才有）、月曆、事項、下面的操作說明；滑鼠點到什麼；**畫面 test 都在這** | ✓ |
| `src/ui/months.rs` | 版型 1 的排版：每個月幾列、列號、捲到哪（只算數字） | |
| `src/ui/planner.rs` | 畫版型 1 的左右兩個框 | ✓ |
| `src/ui/side.rs` | 畫 side menu：日曆清單（日曆 = 任務檔的 `#` 分組，`App::groups`） | ✓ |
| `src/ui/month.rs` | 版型 3（大月曆）：6 週等高的格子（`Grid`，畫畫面和滑鼠換算都用它）、一週一週捲（`App::month_top`） | ✓ |
| `tools/` | 另外算預期畫面的 Python（見「改了畫面之後」） | |

## 一輪怎麼跑
```text
App::run 的迴圈
 ├─ 記下 terminal 大小（app.screen）
 ├─ ui::render → planner::render（版型 1；版型 3 是 month::render，見下面「版型 3」）
 │   ├─ Months::window_for    選取的那個月，右邊在月份裡捲到哪 → app.agenda_window
 │   ├─ Months::scrolled_to   整個畫面捲到哪                 → app.calendar_top
 │   ├─ Months::visible_rows  畫面上每一列是什麼
 │   └─ 一列一列畫，左右兩個框的同一列在同一個高度
 └─ 等一個輸入 → App::handle_key / App::handle_mouse（滑鼠點的位置用 ui::click_at 換算）
```
畫畫面和換算滑鼠位置用的是同一組 function（`ui::areas`、`inner_areas`、`visible_rows`），所以兩邊一定對得起來；改版面時改這幾個就好。

## 版型 1 的排版（`ui/months.rs`）
```text
 第 0 列 → │  10月 40 │  1  2  3  4 ││  10/3 讀完 The Book   ┃   一個月一塊：週數和事排成幾列，取多的那個，
           │       41 │  5  6 ...   ││  10/5 更新 VPN 憑證   ┃   但最多佔月曆的 2/3（至少是週數）
           │       ...                                      │
           │──────────┼─────────────││──────── 還有 21 件 ─│   放不下的事在那一塊裡捲（window）：
           │  11月 44 │          1  ││  11/2 買生日禮物      │   右框線上有捲軸 ┃、下面那條線寫還有幾件
```

| 觀念 | 說明 |
| :- | :- |
| 列號 | 今天那個月的第一列是第 0 列，往下是正、往上是負；`block_start`、`block_at` 從第 0 列一個月一個月數過去 |
| 一塊 | 一個月 = `month_height` 列 + 1 條線；`month_height` = 排成幾列（`layout_height`）和上限取小的 |
| 分欄 | 右邊夠寬時分 1～4 欄（每欄至少 28 格），先排滿第一欄再排第二欄 |
| 月份裡捲（`window`） | 只有選取的那個月會捲，其他月顯示前面幾件；記在 `App::agenda_window` |
| 整個畫面捲（`scrolled_to`） | 選取那週離上下邊緣至少留 2 列（像 vim 的 scrolloff），再讓要看的事也看得到 |
| 一行（`Entry`） | 右邊的一行 = 哪件事 + 列在哪天；`App::agenda()` 排好的順序就是右邊由上到下的順序，`agenda_index`、滑鼠點到的 `Click::Item` 都是這裡面的位置 |
| 要看的事 | 鍵盤在右邊 = 選到的那一行；鍵盤在月曆 = 選取那天的事 |

## 版型 3 的排版（`ui/month.rs`）
| 觀念 | 說明 |
| :- | :- |
| 格子 | 週數欄 5 格，7 天平分剩下的寬度（扣掉 7 條直線），除不盡的零頭從星期一開始一欄多 1 格；6 週一樣高，除不盡的列留在最下面 |
| 捲動 | `App::month_top` = 最上面那週的星期一；剛打開 / `t` / `[` `]` / 換版型 = 選取那個月 1 號的前一週（`None`），選取的那週移出 6 週才一週一週捲 |
| 選取那個月 | 選取的日期在哪個月：其他月份的日子變灰；跨月那週的週數看這個月的日子 |
| 格子裡 | 第一行日期，下面一行一件事（順序跟版型 1 一樣）；放不下就少列一件、最後一行 `+N`；格子只有 1 行時畫點點；標題太長先往右借隔壁格同一行的空位（像試算表，`render_week`），還是放不下就切掉。畫面 test 的任務檔是 `testdata/month.md` |

## 改了畫面之後
1. 改 Rust，跑 `cargo test`：失敗的就是畫面變了的 test
2. 預期畫面**不要從 Rust 畫出來的結果抄**：照新的規格改 `tools/expected_screen.py`（它是另外寫的一份，兩邊一樣才算對），再跑 `python3 tools/regen_ui_screens.py`，用 `git diff src/ui.rs` 看預期畫面變了哪裡、是不是你要的
3. 新增畫面 test：在 `tools/regen_ui_screens.py` 的 `PLAN` 加一筆；版型 3 的畫面加環境變數 `LAYOUT=3`（參數見 `expected_screen.py` 開頭）
4. 顏色不在 Python 裡：用 test 量關鍵格子的顏色（例：`colors_by_role`）

## 寫法規矩
- 註解用繁體中文、技術詞留英文（`render`、`borrow`、`scroll`…）；function 上面用 `///` 寫用途
- 畫面的字放 `texts.rs`、顏色放 `theme.rs`（畫面只講角色，例：`theme.today`），`ui/` 裡不寫死
- 錯誤處理用 `color_eyre::Result` + `?`，不用 `unwrap()`（test 例外）
- commit 前 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 都要乾淨；改到邏輯就補 test，寫完故意改壞一兩處，確認 test 抓得到
- 點點、符號只用 East Asian Width 是 N 或 Na 的字（都是 1 格；`•`、`●` 是 A，開了 CJK 寬字元的 terminal 會變 2 格）；查法：`python3 -c "import unicodedata as u; print(u.east_asian_width('•'))"`
- commit 訊息：`<type>: <scope> | <summary>`，type 是 `feat` / `fix` / `refac` / `doc` / `style` / `test` / `chore` / `perf` / `ci`；改到按鍵、參數時同一個 commit 更新 README

## 踩過的坑
| 坑 | 怎麼避開 |
| :- | :- |
| `Line::styled(...)` 的顏色會套到**整列** | 同一列畫兩段字時，顏色放在 `Span` 上（例：表頭的日期、線上的「還有 N 件」） |
| TestBackend 裡，寬字元（中文）後半那格永遠是預設樣式 | 量顏色時跳過那格；`text_rows` 也是這樣把字接起來 |
| `cargo add ... \| head` | 管線一關，cargo 在寫 `Cargo.toml` 之前就被中斷；不要接 `head` |

## 跨多天的事（規則 3b，`@2026-10-20..2026-10-25`）
`Item::date` 是第一天、`Item::end` 是最後一天（只有一天是 `None`）；`Item::covers(day)` = 頭尾和中間每一天。

| 地方 | 怎麼顯示 |
| :- | :- |
| 月曆（版型 1） | 範圍內每一天的日期加底線（`planner::in_multi_day`；不改顏色，做完的任務不加） |
| 大月曆（版型 3） | 範圍內每一天的格子都列這件事（`App::items_on`：比較早開始的排在那天最前面），日期一樣加底線 |
| 右邊 | `App::agenda()` 在開始那天列一行，日期寫成範圍：`10/20~25`、跨月 `10/28~11/3`、跨年 `12/30~1/2`；碰到的之後每個月再各列一行（列在 1 號、排在那天最前面）。日期至少 5 格，比較長的把標題往右推 |
| 選取 | `Entry::is_on(day)`：那件事蓋到 `day`、而且這一行列在 `day` 那個月，就算那天的事（鍵盤在月曆時墊底色、右邊捲到它）；`Tab` 換到右邊時，比較早開始但蓋到選取那天的排在前面，先選它；在右邊上下選到它時，選取的日期本來就是它的日子就不動 |
| 字元 | `~` 是 East Asian Width Na（1 格）；`–`（en dash）是 A，不要用 |

## 可以改進的地方
- `planner::render` 有 130 多行：捲動和兩個框都在裡面，可以拆成「算捲動」和「畫一列」
- 畫面 test 寫死很多座標：版面一動（例：拿掉一列）就要改一大片；可以改成先找到某個字在哪，再檢查它的顏色
- `App::agenda()` 每次呼叫都重新排序：事很多（幾千件）時可能要先排好存起來
