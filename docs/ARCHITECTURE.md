# roh-cal 架構說明
給要改 roh-cal（v2）的工程師或 AI agent：動手前先讀這篇，再看 code。怎麼用見 [README](../README.md)，任務檔格式見 [task-file-format.md](task-file-format.md)。

## 跑起來
```sh
cargo run --release -- --sample                     # 用內建的範例任務檔打開（每次一份新的）
cargo test                                          # 全部 test
cargo fmt && cargo clippy --all-targets -- -D warnings
```

## 模組地圖
| 檔案 | 做什麼 | 畫畫面 |
| :- | :- | :- |
| `src/main.rs` | 讀命令列參數（`--theme` 只收內建的名字；`--sample` 先寫一份新的範例檔再打開，不能跟 `--tasks` 一起用）、讀任務檔和設定檔、開 / 關 TUI 和滑鼠；第一次開才播開場動畫（`first_run`：記號檔不在才播，並馬上寫下去；寫不進去的話下次再播一次；沒有 HOME 就不播，沒地方記），`--sample` 每次都播（`plays_intro`；記號檔照樣寫，用 `--sample` 第一次開之後，一般開不會又播）；設定頁「全部還原」（`R` `y`）刪掉記號檔（`App::splash_marker`），下次開又播 | |
| `src/sample.rs` | 內建的範例任務檔：`include_str!` 編譯時把 `src/sample.md` 放進程式，`cargo install` 裝的也有；故意寫得短，每一種寫法各一個（test 用的是另一個、大很多的 `testdata/tasks.md`）。`text(today)` 把每個 `YYYY-MM-DD` 整週整週地挪（`WEEK` = 範例當作「這週」的那個週一 → 今天這週的週一），星期幾不變；不存在的日期（`@2026-13-01`）照留。範例檔改寫時，「這週」要維持在 `WEEK` 那週 | |
| `src/app.rs` | 程式的狀態 `App`、主迴圈、按鍵和滑鼠（double click = 同一天或同一件事 0.5 秒內點兩下，`App::is_double_click`；事項要 double click 才打勾，點一下不會誤打勾）；對選到的那件事的鍵（`Space` `x` `/` `r` `e` `a` `A` `d`）事項欄、跳窗、任務頁共用 `handle_item_key`，各自只寫自己特有的鍵；版型（`View`：`1` 版型 1、`2` 版型 2、`3` 任務頁）；開著的跳窗、正在做的事（`Mode`，同時只有一種） | |
| `src/tasks.rs` | 任務檔 parser：純文字 → `Item`（框是 `Mark`：`[ ]` / `[/]` 進行中 / `[x]`，存成 `Item::done`、`Item::in_progress`），和每一個標題（`HeadingLine`：第幾行、原本的字；日曆改名、刪除用） | |
| `src/test_utils.rs` | test 共用的小工具（只在跑 test 時編譯）：`press` 按一個鍵、`type_text` 在編輯框打字、`mouse` 送一個滑鼠動作、`mouse_event` 只做出滑鼠事件（交給 `App::handle_mouse`） | |
| `src/paths.rs` | 任務檔、設定檔、`--sample` 範例檔（cache，每次重寫）、開場動畫播過的記號檔（state：`splash-seen`，空的檔）的預設位置（XDG） | |
| `src/config.rs` | 設定檔 `config.toml` → `Settings`（`theme`、`[calendar]`、`[month]` 的開關、`[calendar_list] width`、`[day_popup] by_calendar`、`[colors]`，`App::settings`）；沒寫的用預設值，名字打錯算錯誤；換 theme 走 `Settings::set_theme`。設定頁關掉時存回去（`save`：toml_edit，只寫在頁面上改過的那幾行，註解、在別處改的都留著；寫出來讀不進來就不寫） | |
| `src/edit.rs` | 改任務檔的純 function：檔案現在的內容 + 要改的那件 → 改好的內容；只動要改的地方（一行、刪除或改縮排或搬到別的日曆的那一塊、上下搬的兩塊、新增的那一行），那一行在別處改過就回傳 `Changed` | |
| `src/files.rs` | 安全寫檔：先寫暫存檔再 rename（symlink 寫進真正的檔案） | |
| `src/external_editor.rs` | 任務頁的 `o`：用使用者的編輯器打開任務檔（`$VISUAL` → `$EDITOR` → `vi`，跟 git 一樣的順序）。`command` 組指令（vi / vim / nvim / nano / emacs / micro / kak 才加 `+行號`，VS Code 這類會把它當檔名）；`open` 先把 terminal 還回去（滑鼠、raw mode、alternate screen 都關掉），編輯器關掉再拿回來、下一次整個重畫。按鍵只設 `App::open_editor`，真的開是在 `App::run`（只有主迴圈拿著 terminal）；改了什麼靠本來的自己重新載入 | |
| `src/clipboard.rs` | 把字放進系統剪貼簿（跳窗放開拖曳時）：交給系統的剪貼簿指令（macOS `pbcopy`；其他 `wl-copy`（有 `$WAYLAND_DISPLAY` 才試）→ `xclip` → `xsel`，第一個跑得起來的，`commands`），不加 crate；在 tmux 裡也能用（tmux 預設擋掉程式送給終端的剪貼簿跳脫碼 OSC 52）；指令的輸出丟掉，不會蓋到畫面 | |
| `src/search.rs` | 模糊搜尋（像 fzy / fzf）：打的字照順序出現就算找到，中間可以隔別的字（`sf` 找得到 `Work > SFAP`）；比對交給 nucleo-matcher（Helix 編輯器用的那套），比較像的排前面，`Found::positions` 是對到的字在第幾個（畫面用來上色） | |
| `src/app/editor.rs` | 一行編輯框：`e` 編輯（下面的「日曆」欄寫它在的日曆；在 `#` 清單選了別的日曆 = `Target::Edit` 的 `calendar`，存檔時整塊（含子任務、筆記）搬到那個日曆最後面、變最外層，`edit::move_to_calendar`，`u` 一次退回）、`a` 新增（放進哪個日曆寫在下面的「日曆」欄；打 `#` 和日曆名字的一部分會跳出 `#` 清單（`HashList`，`search::search` 模糊比對每個日曆的整條路徑 `Work > SFAP`），打字時開、`Esc` / 移動游標 / 選定時關，選定 = 那個 `#` 字拿掉、`edit::Place::InCalendar` 換成它；那天沒有事時預設第一個不是假日的日曆）、`A` 新增子任務、日曆清單的 `a` 新增日曆（`A` = 加在選到的日曆裡面：`Target::AddCalendar` 的 `parent`，存成 `#` 多一個的標題、放在那個日曆最後一段的最後，`edit::add_calendar_in`；下面的「上層日曆」寫加到哪裡；新增日曆的編輯框打 `#` 也能選上層，`#` 清單第一個是「最外層」（`HashList::top`，路徑是空的），同一層不能同名照選到的上層重算）、`e` 日曆改名（同名的每一個標題都改，`#` 的層級不變；打 `#` 選了別的上層 = 整個搬過去：`edit::move_calendar` 把它每一段（標題到下一個一樣多或更少 `#` 的標題之前，含裡面的事和子日曆）拿出來、照新的上層調 `#` 的數量，接在上層最後一段的最後；清單不列自己、自己裡面的、和搬過去會超過 6 個 `#` 的）（`Target`）；直接打那一行的文字（tui-input）；版型 1、2 上，選取那天的事不顯示自己的日期（`Editor::day`：`edit::hide_day` 拿掉 `@日期`、時間留成 `@10:00`，存檔前 `edit::show_day` 補回去；跨多天的範圍照舊；開了沒改就存不寫檔）；`^id` 後面只打了優先度、日期、時間時，`edit::block_id_last` 把 `^id` 搬回行尾（`Editor::line`，每個畫面都一樣），`edit::check`（日曆是 `edit::check_calendar`：名字不能空、不能跟別的日曆一樣）即時檢查，有問題就存不了（`App::editor`） | |
| `src/ui/editor.rs` | 畫編輯框：主畫面正中間（跟「那天的事」跳窗一樣：side menu 右邊、最下面一列上面的那一塊，上下左右都置中）、左右各留 2 格、最寬 80 格（主畫面不到 30 格寬就整個寬）；放得下時框線和字之間上下各空一列、左右各空一格（`PADDING`：不到 30 格寬不空格、太矮不空列），框的高度跟著裡面的東西（`layout`）：打字那列、空一列，再來是讀到什麼，一欄一列、標籤在前、只列有的（`fields`：事項是標題、日期、優先度（用那個顏色）、日曆，子任務是上層事項；日曆是名字、類型（假日日曆才有）、上層日曆；事項照寬鬆的讀法讀，日期寫錯也看得到其他欄；錯的地方自己一列紅字；標題空的時候留白、名字空的時候是灰色提示，按 Enter 存不了之後才有紅字，`Editor::tried`），空一列，最後是可以打什麼（`hints`：「可以打」後面每一樣是標籤（灰）+ 寫法（說明頁段落標題的藍，`help_section`），標籤跟上面的欄位同一組字：事項 `日期 @今天  時間 @10:00  優先度 ! !! !!!  日曆 #名字`，`! !! !!!` 各用自己的優先度顏色；日曆 `假日日曆 type:holiday  上層日曆 #名字`；沒有日曆可選時不寫 `#`；放不下就換行，接著排在第一樣下面）；上框只寫在做什麼（新增事項、編輯事項…），下框只寫 Enter / Esc；太矮就不空上下那列、下面的被切掉。打的字有 priority 就馬上用那個顏色（`Editor::priority`，還不能存也算），打的字裡的寫法（`@日期`、時間、`! !! !!!`、`#` 字）用斜體（`typed_spans`）；框外再清出一圈空白（`ui::clear_with_ring`，不蓋最下面的操作說明），不會跟周圍的字黏在一起。`#` 清單（`list_area`，畫面和滑鼠都用它）：在打的字正下方、左框對齊 `#`（超出右邊就往左移），寬度 = 最長的日曆或下框的按鍵，最多 8 個日曆、選到的跑到下面就捲；對到的字用 `focus` 色加粗，選到的那列墊 `selection` 底色 | ✓ |
| `src/app/help.rs` | 說明頁（`?`）開著時的按鍵和滑鼠：`Tab` 換精簡版 / 完整版，再按 `?` 從精簡版換到完整版、完整版再按就關（按三次 = 精簡、完整、關）、放不下時上下捲、點框外關掉（`App::help`） | |
| `src/ui/help.rs` | 畫說明頁：畫面正中間、左右兩欄、各欄自己往下排（太窄排成一欄、太矮可以捲）。精簡版只列鍵盤在的地方（`App::focus`）能用的段落：那個地方專用的在左邊、哪裡都能用的（畫面、系統）在右邊，上框寫在哪裡；完整版全部列，這裡用不到的段落變灰。內容在 `texts.rs` 的 `help`（`HelpPage::short` 精簡版的段落、`left` / `right` 完整版的兩欄；每段的 `places` = 在哪裡能用，空的 = 哪裡都能用） | ✓ |
| `src/app/settings.rs` | 設定頁（`,`）：設定檔的設定一列一列（`Row`、`rows()`，名字 = 設定檔的名字；`theme` 那列照 `theme::BUILTIN` 的順序換，顏色那幾列在 theme 的色票裡換、`[colors]` 只記跟 theme 不一樣的）、每個設定改得到哪裡（`Row::places`：版型、日曆清單、那天的事跳窗；顏色也算：週數、跨年粗線只有月曆用得到，選取的括號只有版型 1，格線、跨多天的條只有大月曆，`checked` 只有日曆清單和跳窗的開關），那些地方現在都不在畫面上就變暗（`Row::applies_here`、`App::on_screen`：日曆清單收起來、跳窗沒開都算不在）、選列、改值、還原（`App::settings_page`）；改的是 `App::settings`，`App::apply_theme` 換顏色，畫面馬上跟著變 | |
| `src/app/picker.rs` | 自訂顏色（設定頁的顏色那列按 `e`）：`Picker` 記著紅綠藍、選到哪個、打到一半的色碼、打開前 `[colors]` 那一行（`Esc` 放回去）；每改一次就寫進 `App::settings` 再 `apply_theme`，畫面馬上跟著變；跟 theme 原本的顏色一樣就不記在 `[colors]`。狀態放在 `Mode::Settings` 裡（只有設定頁開著時才有） | |
| `src/ui/picker.rs` | 畫自訂顏色的框：疊在畫面正中間，顏色本身（墊底色的空白格）+ 色碼、紅綠藍各一條（每一格 = 那個值的顏色，`|` 是現在的位置；都只用空白墊底色，沒用 `█` 這種 A 寬度的字），滑鼠點到哪一條、哪個值（`at`） | ✓ |
| `src/ui/settings.rs` | 畫設定頁：畫面正中間，照設定檔的表分段（段落之間不空行、框裡上下不留白，80 × 24 放得下），顏色前面有色塊；框的寬度 = 設定那幾列、說明、下框的按鍵裡最寬的，說明和按鍵的字要寫得比設定那幾列短，框才不會被撐寬；`[colors]` 一次最多列 10 個顏色（`COLOR_ROWS`），選到下面的顏色時那一段自己捲（`SettingsPage::colors_top`，旁邊的右框線有捲軸）；整頁放不下時再捲到選到的那列（`SettingsPage::top`），下面一行說明選到的那一列；目前的版型用不到的設定（例：版型 1 的 `[month]`）字變暗，選到它時說明下面再一行寫它在哪幾個版型用得到 | ✓ |
| `src/app/day.rs` | 「那天的事」跳窗（版型 1、2 都有）：月曆上 `Enter` 或 double click（同一天 0.5 秒內點兩下，`App::click_calendar_day`）打開（`Focus::Day`，選到第幾件記在 `App::day_index`，順序 = `App::day_items`：`items_on(selected)`，`[day_popup] by_calendar` 開著時再照最外層日曆分組，日曆照日曆清單的順序、沒在日曆裡的排最前面，組內順序不變）；按鍵跟版型 1 右邊一樣，多一個 `c`（點下框右邊的開關也一樣，`App::toggle_by_calendar`）切換照不照日曆分組、馬上存進設定檔（`App::save_switch`，跟「隱藏做完的」共用），選到的那件照行號選回來；寫檔走同一套（`chosen_item()` 回傳跳窗選到的那件），寫完 `keep_day_choice` / `choose_day_line` 調整選的位置，那天沒事了就關掉；滑鼠拖曳選字：按下時記住按在標題的哪個字（`TextDrag::anchor`，`TextSpot` = 第幾件的第幾個字，排序就是閱讀順序），按著移動時記住現在在哪個字（`drag_day`，移到日曆名字、空白列上不變，拖出框外算最近的那列），`day_selection` 是頭到尾兩端都算；按任何鍵、下一次按下滑鼠、重新讀任務檔就清掉（`App::day_drag`）；拖過之後不算 double click（`last_click` 清掉）；放開（`release_day`）就把拖過的字（`day_selected_text`：每件取它那段標題，一件一行，跳窗裡換行的標題還是一行）放進 `to_copy`，主迴圈 `copy_out` 交給 `clipboard::copy`，右下角提示幾個字或失敗（`App::toast`，`Toast`，2 秒後 `end_finished_toast` 拿掉；有提示時主迴圈最多等到它該消失） | |
| `src/ui/day.rs` | 畫跳窗：疊在主畫面（大月曆，或版型 1 的月曆 + 事項）正中間（最寬到左右各留 2 格，`SIDE_MARGIN`，跟編輯框一樣；主畫面不到 30 格寬就整個寬），一件一列（`popup_line`：最前面是 `ui::item_marker`（跟任務頁共用），任務是勾選框 `ui::check_box`（做完 ☑），行程是 `∙`，顏色跟時間一樣 `planner::secondary_color`；只有跨多天的事寫日期範圍，單日的不寫，上框已經寫了是哪天；時間、標題、顏色跟版型 1 共用 `planner::time_and_title`、`planner::item_color`；選到的那件只墊底色，沒有 `[10/17]` 括號；標題放不下就用任務頁的 `wrap` 換行，接著時間後面往下排，比勾選框內縮，看得出是同一件；`App::day_top` 算的是行），放得下時框線和事之間上下各空一列、左右各空一格（`PADDING`，上下 / 左右分開看；事太多放不下就不空列），框外再清出一圈空白（`ui::clear_with_ring`，只到大月曆的邊），放不下就捲（跑出框外才捲，記在 `App::day_top`）、右框線上有捲軸；照日曆分組時（`[day_popup] by_calendar`，預設開）每個最外層日曆的第一件上面多一列日曆名字（`heading_line`，`label` 色；點了沒反應），底下的事縮 2 格；畫面由上到下切成一段一段（`Piece`：一件事的幾行，或一列日曆名字），捲動、滑鼠都照它算，選到一組的第一件時連名字一起捲進來；不分組時每件第一行最右邊寫它最外層的日曆（`ui::calendar_tag`，顏色跟時間一樣 `planner::secondary_color`；沒在日曆裡、假日日曆的不寫；框的寬度把它算進去）；下框右邊是分組的開關（`switch_line`：`☑ 照日曆分組`，跟日曆清單的開關一樣，☑ `checked` 色、☐ 灰；滑鼠範圍 `switch_area`）；每一行記著標題的每個字在第幾格（`TitleCell`，`title_cells` 把 `wrap` 切好的行對回標題的第幾個字：`wrap` 只會拿掉斷行處的空白，跳過就對得上；🚧 不算標題的字），`text_at` 把滑鼠位置換成 `TextSpot`（同一行在標題左邊 / 右邊 = 那行第一個 / 最後一個字），拖過的字墊 `text_selection` 底色、字一律用 `text` 色（`!!!` 的紅疊在藍底上看不清楚；`text_selection` 是每套 theme 的藍和背景 4:6 混，比選取列的 `selection` 亮，疊在選到的那件上也看得出來，test 檢查每套都跟 `selection` 不一樣）；`item_at`、`text_at` 共用 `piece_at` 找第幾行是哪一塊；下框左邊寫選到那件的完整路徑（`ui::calendar_path`，任務頁日曆標題的 `label` 色；框線畫完再自己畫上去，太長就切掉、跟開關之間留一格框線；用 `Span` 上色，用 `Line` 的 style 會把後面的框線也染色）；上框右邊寫那天的任務「未完成 N / 已完成 M」（`top_titles`，數字是 `App::day_counts`：有勾的日曆裡蓋到那天的任務，行程不算，進行中算未完成，被「隱藏做完的」藏起來的也算；未完成照剩幾成用優先度的紅橘黃 `open_color`，已完成一律 `today` 的綠），框的寬度也要放得下上框的日期和數字 | ✓ |
| `src/app/moving.rs` | 改期（`r`）：月曆就是選日期的地方；選取的日期移了幾天，那件事就搬幾天（`App::moving`） | |
| `src/app/tasks.rs` | 任務頁（`3`）的每一列（`TaskRow`：標題 / 一件事；照檔案順序列出每一個有勾的日曆的標題，沒有事的、事全部被「隱藏做完的」藏起來的也列；「隱藏做完的」在這裡用自己的規則 `App::listed`：做完的事底下還有沒做完的就留著（灰、劃掉），不然沒做完的那件在樹裡沒有位置；月曆、事項欄、跳窗只藏做完的那一件（`App::hidden_as_done`），底下沒做完的照樣列；日曆像 side menu 一樣排成樹：它外面幾層的日曆沒在正上方列過就先列標題，例：`# Work` 底下只有 `## SFAP`，`Work` 照樣有一列；`App::headings` 是空的（test 只給事）時，事換日曆才列標題；標題列記著自己那行標題的行號 `TaskRow::Heading::line`，游標（`App::tasks_line`）也能停在上面，`a` 就加在那個日曆最後面（`App::tasks_chosen_calendar`），其他對事的鍵不做事）、`j` `k` `l` `h` `=` `-`、`Shift+→` `Shift+←` 整棵子樹展開 / 收合（右鍵點 `▸` 也是，`App::right_click_task`）；日曆也能收合：收起來的日曆記路徑在 `App::collapsed`（不存檔），在它裡面的標題和事都不列（`Calendars::hides`），裡面沒有東西的日曆不能收（`App::filled_calendars`）；標題列記著自己的路徑、能不能收、收了沒（`TaskRow::Heading` 的 `path` `children` `collapsed`），`l` `h` `Shift+→` `Shift+←` `=` `-`、點 `▸` 對標題列也有用；寫完檔選到的事若在收起來的日曆裡，那個日曆和它外面幾層會打開（`App::choose_task_line`）、`gg` `G` 第一件 / 最後一件（跟日曆清單一樣，`App::pending_g` 等第二個 `g`）、`[` `]` 跳到上一個 / 下一個日曆的標題列（只算自己有一行標題的，跟 `j` `k` 選得到的一樣）、縮排 `L` `H`、上下搬 `J` `K`；選到的那件記檔案的第幾行（`App::tasks_line`），展開的記在 `App::expanded` | |
| `src/ui/tasks.rs` | 畫任務頁：表頭（右邊寫有幾處寫錯）、樹狀的每一列（標題照它在幾層日曆裡面縮排，每層 2 格，名字前面是 `▾`（收起來是 `▸`，後面接裡面有幾件：它和子日曆最外層的事，有沒有日期都算，`App::calendar_item_count`；裡面沒東西的日曆是空白）；事在自己日曆的標題底下再縮一層（`▸` 跟標題的名字對齊），再加上自己是第幾層子任務，`levels`；再來是 `ui::item_marker`（任務 `☐` / `☑`，行程 `∙`，跟跳窗共用；進行中也是 `☐`，標題前面有 🚧）；右邊日期、時間各一欄、上下對齊，寫錯的那一行紅字；標題太長就換行：`wrap` 在空白、中文字之間斷，`layout_rows` 算每一列佔幾行）、最下面選到的那一行原本的文字或哪裡寫錯（`App::warnings`）；捲到哪記在 `App::tasks_top`（算的是行，不是第幾件；選到的那件整件都要看得到） | ✓ |
| `src/app/write.rs` | 寫任務檔的流程：先重讀、改、寫（打勾 `Space` 和進行中 `/` 走同一條路 `change_chosen_box`，只差換框的 function：`edit::toggle` / `edit::toggle_progress`；換完框那件可能在那天換位置（進行中排最前面），`App::keep_chosen_line` 照行號把它選回來，藏起來了（隱藏做完的）就留在原位置 = 下一件；在別處改過、重新載入時（`take_change_from_elsewhere`）也一樣，行號先用 `edit::line_map` 換成新檔案的；日曆清單的 `L` `H` 調整階層也在這：`edit::shift_calendar` 改 `#` 的數量，改名、移動後 `App::carry_checked` 讓勾選狀態跟著新的路徑；`d` 刪除日曆也在這：先在最下面一列問，再按 `d`（`dd`）或 `y` 才刪，`Mode::ConfirmDeleteCalendar`；刪除事項也一樣，`Mode::ConfirmDelete`；刪掉同名的每一個標題到下一個標題之前的每一行，`edit::delete_calendar`）；在別處改過就重新載入；結果放在 `App::notice`（最下面一列）。沒在寫的時候在別處改的也會載入（`refresh_tasks`，主迴圈每一輪叫它；不提示；編輯框、改期、問要不要刪的時候先等，關掉再載入）：`App::tasks_text` 記著畫面上是哪個內容（剛打開、roh-cal 自己寫、復原、重新載入時更新），檔案的修改時間變了才讀，讀到的跟它不一樣才算別處改的。改的是 `App::chosen_item()`（右邊、任務頁或版型 2 跳窗選到的那件）；寫完用 `edit::line_map` 把畫面記著的行號（任務頁選到的、展開的）搬到新的行號（上下搬照 `edit::Swap` 換了哪兩塊搬，復原時反過來） | |
| `src/calendar.rs` | 日期計算：一個月有哪幾週、第幾週、週數欄寫什麼（`week_label`：`[calendar] relative_weeks` 時寫跟選取那週差幾週） | |
| `src/texts.rs` | 畫面上的字（英文 / 繁中） | |
| `src/theme.rs` | 顏色：每個角色配一個顏色（`Theme`，角色名 = `ROLES`）；一套 theme 是一個 TOML 檔（`[palette]` 色票 + `[roles]` 每個角色用哪個），內建的在 `themes/`、用 `include_str!` 編進執行檔；`Theme::build` = theme + 設定檔 `[colors]` 蓋掉的顏色；`Theme::dimmed` = 用不到的東西（設定頁的列、說明頁的段落）變暗的樣式：`other_month` 的顏色，是 `reset` 時改用 terminal 自己的 faint 字（SGR 2；terminal 那套只有 16 色，沒辦法把字和背景混一半） | |
| `themes/` | 內建的 theme 檔：`catppuccin-mocha`、`gruvbox-dark`、`tokyo-night`、`dracula`、`nord-bright`（預設，`theme::DEFAULT`；Nord 每個顏色亮一點、鮮一點，色相不變；從背景混出來的照同樣比例重算）、`one-dark`、`rose-pine`、`terminal`（只用 terminal 的 16 色）；加一套 = 加一個檔 + `theme::BUILTIN` 加一筆（`--help`、設定頁跟著有；README 的清單、test 的抽查要自己加） | |
| `src/ui.rs` | 把畫面切成 side menu（terminal 至少 120 格寬才有；不夠寬時鍵盤在 side menu = 蓋在主畫面左邊，`side_overlay`）、月曆、事項、下面的操作說明（月曆和任務頁的最左邊是版型的鍵 `123`：現在的版型那個字亮起來（`focus` 色、粗體），點一下換版型，`Click::View`；鍵盤在跳窗、side menu 時換成它們自己的說明，沒有 `123`）；滑鼠點到什麼；事的標題 `title_span`（每個畫面共用：做完刪除線；進行中粗體、前面加 🚧，emoji 固定 2 格寬、自帶顏色）；右下角的提示（`App::toast`）蓋在最下面一列的最右邊，前後各空一格，`notice` 色 | ✓ |
| `src/ui/tests.rs` | 畫面 test（大部分；說明頁、設定頁的在 `ui/help/tests.rs`、`ui/settings/tests.rs`）：預期畫面由 `tools/regen_ui_screens.py` 重算、寫回這個檔 | |
| `src/ui/months.rs` | 版型 1 的排版：每個月幾列、列號、捲到哪（只算數字） | |
| `src/ui/planner.rs` | 畫版型 1 的左右兩個框；月曆一天 `DAY_WIDTH` = 4 格（空格、靠右的日期 2 格、日期後面 1 格），寬度、標題列、滑鼠換算都從它算。日期後面那格是點點（`dots`：那天幾件事，數 `App::items_on`，跟著日曆勾選、隱藏做完的；點字 `⠂ ⠒ ⠖ ⠶`，最多 4 個；4 個點是同一個字、只能一個顏色，用那天還沒做完的事裡最急的那件的顏色（`dots_color`：最高的優先度，沒有就看有沒有假日的藍，都沒有就 `muted`；做完的、過去的行程不算）；設定檔 `[calendar] dots = false` 時那格空白、月曆寬度不變）；選到的那天沒點點是 `[17]`，有點點是 `[17⠒]`（`]` 放進下一天的第一個空格，週日放進最後多的那格）；右邊事項欄選到的那件只墊底色，不加括號 | ✓ |
| `src/ui/side.rs` | 畫 side menu（標題下面兩個開關 + 一條線；選到哪一列記在 `App::side_row`（`SideRow`：`HideAll`、`HideDone`、`Calendar(第幾個)`），讀的時候用 `App::chosen_side_row`：沒有日曆時 `Calendar` 算「隱藏做完的」。「隱藏全部」全部日曆都沒勾時才是 ☑（`App::all_hidden`，不另外存），按了 = 全部取消勾選 / 全部勾回來；「隱藏做完的」切換 `[calendar_list] hide_done` 並馬上存進設定檔（任何畫面按 `.` 也一樣，`App::toggle_hide_done`：選到的那件照行號選回來，藏起來了就留在原位置，超過最後一件就選最後一件，跳窗空了就關掉，`App::keep_choice_in_range`）；上下移動照畫面的順序換算，`App::choose_group`；重新讀任務檔時開關照樣選著，選到的日曆照路徑找回來（在別處改檔、上面多了或少了日曆也一樣，`App::set_items`），它被刪了就選原本位置那個、超過最後一個就選最後一個）：日曆清單（日曆 = 任務檔的 `#` 分組，`App::groups`；底下還沒有事的標題也算；`#` 越少越上層，日曆 = 整條路徑 `Heading::parents` + 名字，清單照樹排、裡面的縮排 2 格；勾 / 取消一個日曆時裡面的日曆跟著一樣（`App::toggle_calendar`）；顯示與否只看自己的勾，不看外層（`App::is_shown` 看 `Group::includes`，任務頁的標題看 `Group::is`），所以外層沒勾、裡面有勾的照樣顯示）；寬度是設定檔的 `[calendar_list] width`（22～60 格），沒寫 = 自動：跟著最長的名字，22～38 格（`CalendarListSettings::AUTO_MAX_WIDTH`）；設定頁在自動時按 `h` `l`，從畫面上的寬度開始調（`App::change_setting`）；滑鼠按住右框線（寬的時候旁邊那條主畫面的框線也算，`ui::on_side_border`）拖曳：移幾格寬幾格、變成固定寬度（`App::drag_side_border`，按下時的位置和寬度記在 `SideDrag`），放開才存進設定檔（`App::save_switch`）；雙擊右框線回到自動並馬上存（`App::press_side_border`，拖過之後再點不算雙擊）；放不下時捲到選到的那個看得到（跑出框外才捲，點一下看得到的那個不會捲走；捲到哪記在 `App::side_top`），右框線上有捲軸 | ✓ |
| `src/ui/month.rs` | 版型 2（大月曆）：6 列等高的格子（`Grid`，畫畫面和滑鼠換算都用它）；一列是一整週（星期一～日，跨月的那週兩個月的日子在同一列）；一週一週捲（`App::month_top`）；滾輪只捲畫面、選到的那天不動，可以捲到畫面外（`App::month_free` 記著捲的時候選到哪天，選的日子一變，畫面就照常跟著它） | ✓ |
| `src/ui/splash.rs` | 第一次開 rcal（和每次 `--sample`）時播的開場動畫（`Mode::Splash`，蓋住整個畫面，播 5.5 秒 = `SPLASH_FOR`）：靜的星空（固定位置的星星各自慢慢閃；其中三顆是點綴，在角落（寬和高外側的三分之一）：左上、右下各一顆金色（往橘飄），右上一顆藍色（往紫飄，`weekend` → `focus`））、大字 `roh-cal` 一個字一個字掉下來、越掉越快、落地後小彈一下（每欄 1～3 列高，`bounce_height`）才停、最下排揚起一點灰塵、停下來後顏色波浪、字都停下來（1.4 秒，`SETTLED`）才打出副標、日期慢慢浮現、3.6 秒（`GLINT_AT`）斜面閃光掃一次（每件事錯開），每 6 秒一顆流星（尾巴是 braille 點，`Dots`：一格 2 × 4 點，斜線才不會一階一階）；左下角灰字提示按任意鍵跳過；星星和流星不進中間那塊（`quiet`）。畫面全部由「播了幾秒」（`seconds`）算出來，同一個秒數畫出來一定一樣，test 直接帶秒數畫 | ✓ |
| `tools/` | 另外算預期畫面的 Python（見「改了畫面之後」） | |

## 一輪怎麼跑
```text
App::run 的迴圈
 ├─ App::refresh_tasks   任務檔在別處改過（修改時間變了、內容跟畫面的不一樣）就重新載入，每個版型都跟著變
 ├─ 記下 terminal 大小（app.screen）
 ├─ ui::render
 │   ├─ 主畫面：planner::render（版型 1）/ month::render（版型 2，見下面「版型 2」）/ tasks::render（任務頁）
 │   │   版型 1：
 │   │   ├─ Months::window_for    選取的那個月，右邊在月份裡捲到哪 → app.agenda_window
 │   │   ├─ Months::scrolled_to   整個畫面捲到哪                 → app.calendar_top
 │   │   ├─ Months::visible_rows  畫面上每一列是什麼
 │   │   └─ 一列一列畫，左右兩個框的同一列在同一個高度
 │   ├─ 跳窗（版型 2 那天的事、不夠寬時的 side menu、編輯框、說明頁、設定頁）蓋在最上面；設定頁先捲到選到的那列（settings::scrolled）
 │   └─ 開場動畫（Mode::Splash）蓋住全部：splash::render，帶「播了幾秒」；播滿 5.5 秒（SPLASH_FOR），畫之前就關掉（App::end_finished_splash）
 └─ 等一個輸入，最多等 1 秒（REFRESH_EVERY；沒輸入就直接下一輪，回去檢查任務檔）；開場動畫播放中只等 33 毫秒（FRAME_EVERY），它才動得起來
     → App::handle_key / App::handle_mouse（滑鼠點的位置用 ui::click_at 換算）
     └─ 先看 App::mode（同時只會有一種）：開場動畫播放中，任何鍵、點一下、滾輪都只是跳過它（不清最下面一列的通知，
        剛打開時的「任務檔有 N 處寫錯」才看得到）；說明頁、設定頁、問要不要刪、編輯框、改期中各自收按鍵和滑鼠，
        都沒開（Mode::Normal）才照鍵盤在哪個框處理；多一種模式時，沒處理到的 match compiler 會抓到
```
畫畫面和換算滑鼠位置用的是同一組 function（`ui::areas`、`inner_areas`、`visible_rows`），所以兩邊一定對得起來；改版面時改這幾個就好。

## 版型 1 的排版（`ui/months.rs`）
```text
 第 0 列 → │  10月 40 │  1  2  3  4 ││  10/3 讀完 The Book   ┃   一個月一塊：週數和事排成幾列，取多的那個，
           │       41 │  5  6 ...   ││  10/5 更新 VPN 憑證   ┃   但最多佔月曆的 2/3、最多 16 列（至少是週數）
           │       ...                                      │
           │──────────┼─────────────││──────── 還有 21 件 ─│   放不下的事在那一塊裡捲（window）：
           │  11月 44 │          1  ││  11/2 買生日禮物      │   右框線上有捲軸 ┃、下面那條線寫還有幾件
```

| 觀念 | 說明 |
| :- | :- |
| 列號 | 今天那個月的第一列是第 0 列，往下是正、往上是負；`block_start`、`block_at` 從第 0 列一個月一個月數過去 |
| 一塊 | 一個月 = `month_height` 列 + 1 條線；`month_height` = 排成幾列（`layout_height`）和上限取小的 |
| 分欄 | 只有 1 欄，再寬也一樣：一個月的第 i 列就是那個月的第 i 件（捲過的話加上捲了幾列，`Row::Week::item`） |
| 月份裡捲（`window`） | 只有選取的那個月會捲，其他月顯示前面幾件；記在 `App::agenda_window` |
| 整個畫面捲（`scrolled_to`） | 選取那週離上下邊緣至少留 2 列（像 vim 的 scrolloff），再讓要看的事也看得到 |
| 一行（`Entry`） | 右邊的一行 = 哪件事 + 列在哪天；`App::agenda()` 排好的順序就是右邊由上到下的順序（同一天：進行中 `[/]` 最前面，再來是從上個月接過來的、沒時間的、照時間；一樣的照檔案順序。版型 2 的格子用 `App::items_on`：進行中最前面，再來是比較早開始的、照時間；跳窗用 `App::day_items`，再照日曆分組），`agenda_index`、滑鼠點到的 `Click::Item` 都是這裡面的位置 |
| 要看的事 | 鍵盤在右邊 = 選到的那一行；鍵盤在月曆 = 選取那天的事 |

## 版型 2 的排版（`ui/month.rs`）
| 觀念 | 說明 |
| :- | :- |
| 格子 | 週數欄 5 格，7 天平分剩下的寬度（扣掉 7 條直線），除不盡的零頭從星期一開始一欄多 1 格；6 列一樣高；除不盡留下來的列，接著畫後面的週（那一週的第一列放得下就畫，最後一週被下框切掉；`Grid::weeks`），只剩 1 列、放不下時那列算第 6 週的（捲動照樣看 6 週）。一列 = 一整週（星期一～日），跨月的那週兩個月的日子在同一列。格線的每一段看它兩邊是哪兩天（`Rule`）：同一個月淡淡的，月底和下個月 1 號之間用框線的顏色，12/31 和 1/1 之間是粗線 `━` `┃`（`year_line` 色），所以月份的邊是一道階梯（像紙本月曆每個月的外框）；週數欄底下那段跟星期一的一樣；交叉的地方粗細照四段各自的（`cross`）、顏色用四段裡最重的。設定檔 `[month] grid_lines = false`：不畫格線、格子緊貼著（`Grid::gap` = 0），多出來的格數給格子；只有月份交界照樣畫階梯線（`render_month_edges`，畫在格子之後）：直線畫在 1 號那格最左邊那格（格子裡每一行都是空白開頭，不會蓋到字），橫線畫在兩週之間空出來的一列（`Grid::week_gaps`：只有換月的兩週之間有，那幾列從格子高度扣） |
| 捲動 | `App::month_top` = 最上面那週的星期一；剛打開 / `t` / `[` `]` / 換版型 = 1 號那週（`None`；一個月最多 6 週，整個月都看得到），選取的那週移出 6 列才一週一週捲；設定檔 `[month] center_cursor = true`：選取的那週固定在第 3 列（`month::scrolled`） |
| 選取那個月 | 選取的那天整格墊 `selection` 的底色（像 v1；日期不加括號）。選取的日期在哪個月：那個月的日期粗體（其他月份的日子不變灰，有月份的階梯線分開就夠了）。週數看那週裡選取那個月的日子（沒有就看星期一；只有跨年那週會不一樣）；有 1 號的那週，週數下面一行寫那個月（`Nov` / `11月`），1 月再下一行寫年份（`week_column_text`：跟週數一樣靠右；格子不夠高就少寫） |
| 格子裡 | 第一行日期，下面一行一件事：先排一整列（`layout_row`）：跨多天的事每件在那一列佔同一行（`Bar::lane`，越早開始越上面，那一行空出來就給下一件用），一天的事再從上面填空著的行（順序 `App::items_on`：進行中最前面；進行中的不寫時間，格子太窄、🚧 之後就放不下標題，時間在跳窗看）；放不下就少列一件、最後一行 `+N`（空著的行不算）；跨多天的事最後畫（`render_bars`）：同一行連成一條 `range` 底色、蓋過中間的格線、標題只寫在開頭，哪天那一行被 `+N` 佔了、或換了月份就斷開（新的那段再寫一次標題）；格子只有 1 行時畫點點；標題太長先往右借隔壁格同一行的空位（像試算表，`render_week`；不借選取的那格，底色才乾淨；也不借到下個月的格子），還是放不下就切掉。畫面 test 的任務檔是 `testdata/month.md` |

## 改了畫面之後
1. 改 Rust，跑 `cargo test`：失敗的就是畫面變了的 test
2. 預期畫面**不要從 Rust 畫出來的結果抄**：照新的規格改 `tools/expected_screen.py`（它是另外寫的一份，兩邊一樣才算對），再跑 `python3 tools/regen_ui_screens.py`，用 `git diff src/ui/tests.rs` 看預期畫面變了哪裡、是不是你要的
3. 新增畫面 test：在 `tools/regen_ui_screens.py` 的 `PLAN` 加一筆；版型 2 的畫面加環境變數 `LAYOUT=2`、任務頁 `LAYOUT=3`（參數見 `expected_screen.py` 開頭）
4. 顏色不在 Python 裡：用 test 量關鍵格子的顏色（例：`colors_by_role`）

## 寫法規矩
- 程式的註解用英文（直白、精簡；2026-10-18 起，給國外的人也看得懂）；function 上面用 `///` 寫用途。這份文件和其他給自己看的筆記照樣用中文
- 畫面的字放 `texts.rs`、顏色放 `theme.rs`（畫面只講角色，例：`theme.today`），`ui/` 裡不寫死
- 錯誤處理用 `color_eyre::Result` + `?`，不用 `unwrap()`（test 例外）
- test 放哪：test 超過 100 行就搬到自己的檔 `<模組>/tests.rs`（例：`src/edit.rs` 的 test 在 `src/edit/tests.rs`，原本的檔只留 `#[cfg(test)] mod tests;`），程式和 test 分開讀；按鍵、打字、滑鼠這種每個 test 都要的小工具放 `src/test_utils.rs`，不要每個 test 檔自己抄一份；短的照樣寫在檔案最後的 `mod tests { ... }`。模組的路徑沒變（還是 `edit::tests`），`use super::*;` 照樣拿得到私有的東西；`include_str!` 的路徑從 test 檔自己的位置算
- commit 前 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 都要乾淨；改到邏輯就補 test，寫完故意改壞一兩處，確認 test 抓得到
- 點點、符號只用 East Asian Width 是 N 或 Na 的字（都是 1 格；`•`、`●` 是 A，開了 CJK 寬字元的 terminal 會變 2 格）；查法：`python3 -c "import unicodedata as u; print(u.east_asian_width('•'))"`
- commit 訊息：`<type>: <scope> | <summary>`，type 是 `feat` / `fix` / `refac` / `doc` / `style` / `test` / `chore` / `perf` / `ci`；改到按鍵、參數時同一個 commit 更新 README

## 踩過的坑
| 坑 | 怎麼避開 |
| :- | :- |
| `Line::styled(...)` 的顏色會套到**整列** | 同一列畫兩段字時，顏色放在 `Span` 上（例：表頭的日期、線上的「還有 N 件」） |
| TestBackend 裡，寬字元（中文）後半那格永遠是預設樣式 | 量顏色時跳過那格；`text_rows` 也是這樣把字接起來 |
| 跳窗蓋在月曆上時，左邊那格的中文字（佔 2 格）後半格會蓋到跳窗的左框線（ratatui 假設中文字後面那格是空的，不會送框線給 terminal） | 畫跳窗之前先呼叫 `ui::clear_wide_left_of` 把那格換成空白（編輯框、版型 2 的跳窗走 `ui::clear_with_ring`，它會清外面那一圈的左邊） |
| `cargo add ... \| head` | 管線一關，cargo 在寫 `Cargo.toml` 之前就被中斷；不要接 `head` |
| 很多字型沒有 `☐`：terminal 去別的字型借（常借到中日韓的寬字型），Ghostty 看到後面接一般空格，就把借來的符號畫成 2 格，蓋掉空格，框變得又大又擠 | 勾選框一律用 `ui::check_box`：框後面接不斷行空格（U+00A0），看起來一樣，但 Ghostty 不會撐大；test 的預期畫面寫成 `\u{a0}`（`tools/expected_screen.py` 的 `BOX_ON` / `BOX_OFF` 也一樣） |

## 跨多天的事（規則 3b，`@2026-10-20..2026-10-25`）
`Item::date` 是第一天、`Item::end` 是最後一天（只有一天是 `None`）；`Item::covers(day)` = 頭尾和中間每一天。

| 地方 | 怎麼顯示 |
| :- | :- |
| 月曆（版型 1） | 範圍內每一天的日期加底線（`planner::in_multi_day`；不改顏色，做完的任務不加）；同一件事蓋到的相鄰兩天，中間的格子（前一天的點點格、後一天日期前面的空格和括號）也畫底線，連成一條（`planner::ranges_on`，顏色用前一天日期的顏色，`underline_color`）；兩件緊貼的不連；換週（週日 → 下一列的週一）、換月的地方斷開。兩件以上重疊的日期和中間的格子，底線改用 `range_overlap` 角色的顏色（`planner::range_underline`；ratatui 只有一種底線，畫不出雙底線；內建 theme 都用 `!!` 的橘色） |
| 大月曆（版型 2） | 範圍內的日子畫成一條（見上面「版型 2 的排版」的「格子裡」；換列、換月的地方斷開，下一段開頭再寫一次標題，時間只寫在真正開始那天），日期一樣加底線 |
| 右邊 | `App::agenda()` 在開始那天列一行，日期寫成範圍：`10/20~25`、跨月 `10/28~11/3`、跨年 `12/30~1/2`；碰到的之後每個月再各列一行（列在 1 號、排在那天最前面）。日期至少 5 格，比較長的把標題往右推 |
| 選取 | `Entry::is_on(day)`：那件事蓋到 `day`、而且這一行列在 `day` 那個月，就算那天的事（鍵盤在月曆時墊底色、右邊捲到它）；`Tab` 換到右邊時，比較早開始但蓋到選取那天的排在前面，先選它；在右邊上下選到它時，選取的日期本來就是它的日子就不動 |
| 字元 | `~` 是 East Asian Width Na（1 格）；`–`（en dash）是 A，不要用 |

## 可以改進的地方
- `planner::render` 有 130 多行：捲動和兩個框都在裡面，可以拆成「算捲動」和「畫一列」。捲動的結果可以像設定頁一樣由 `ui::render` 記回 `app`（`planner::scrolled`、`month` 也一樣）：每個 render 就只要 `&App`，捲動也能不靠 TestBackend 單獨測
- `day_color`、`in_multi_day`、`date_text`、`open_item_color` 放在 `planner.rs`，但版型 2、任務頁、side menu 都在用：可以搬到 `ui.rs`
- `edit.rs` 認「這一行沒被改過」有兩份寫法（`find_line` 和 `replace_line` 裡自己比一次）；拿縮排有 `indent_of` 了，`add`、`replace` 還是自己寫一次：可以合成一個
- test 裡還有 17 處就地寫 `MouseEvent { ... }`（`ui/tests.rs` 8 處、`app/write/tests.rs` 5 處等）：可以改用 `test_utils::mouse` / `mouse_event`
- 任務頁捲動上下留 2 列：點一下靠近上下邊的那列，清單會捲動，第二下就點到別件，所以那幾列 double click 打不了勾（版型 2 的跳窗、side menu 已經改成跑出框外才捲）
- 操作說明比 80 格寬的：任務頁英文 96 格、中文 87 格，月曆的英文 82 格（加了 `123 view`），窄的 terminal 後面會被切掉；可以放不下時整段拿掉比較不重要的鍵（例：`q`、`,`），不要切在字中間
- `L`、`A` 縮排一律加 2 格空白：用 tab 縮排的檔（Obsidian 預設）會變成 tab、空白混著；可以改成照上面那件現有子任務的寫法
- `read_tasks` 和 `read_tasks_for_adding` 只差檔案不存在時怎麼辦：可以合成一個、多一個參數
- 顏色角色 `other_month` 現在只用在設定頁、說明頁用不到的那幾段（大月曆不再讓其他月份變灰），名字跟用途對不起來；要改名得想清楚使用者設定檔 `[colors]` 裡舊的名字怎麼辦
- 畫面 test 寫死很多座標：版面一動（例：拿掉一列）就要改一大片；可以改成先找到某個字在哪，再檢查它的顏色（設定頁的 test 已經改成「相對框的位置」，用 `y_of(Row)` 找列）
- `App::agenda()` 每次呼叫都重新排序：事很多（幾千件）時可能要先排好存起來
- `App::tasks_rows()` 每按一個鍵、每畫一次都重算，找子任務是兩層迴圈：事很多時可以先算好每件的子任務
- 任務頁選到的那件、展開的子任務是用行號記的：在別處改過、重新載入時也用 `edit::line_map` 跟著搬，但它假設只改了一段；別處一次改好幾段（或改到選到的那行、行數又變了）時對不到，會選回第一件
- `refresh_tasks` 每秒看一次修改時間（`std::fs::metadata`）：很便宜，但不是馬上；要即時可以換成 `notify` crate 監看檔案（多一個 crate、多一條 thread）
- 最下面一列是通知（例：剛打開時的「任務檔有 1 處寫錯」）時，點最左邊 3 格照樣會換版型：點的那一下先清掉通知，`click_at` 看到的是 `123`；要分得出來得在清掉通知之前記下它
- 開場動畫的小工具（`noise`、`mix`、`gradient`、`cycle`、braille 的 `Dots`）都放在 `ui/splash.rs`：別的畫面要用（例：其他地方也要漸層色、braille 線）時，搬到 `ui.rs` 或自己一個 module
- `tools/expected_screen.py` 有 4 種畫面（版型 1、3、任務頁、side menu）擠在一個檔：可以一個版型一個檔，共用的（字寬、讀任務檔）放一起
