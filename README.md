# roh-cal
A simple, keyboard-driven terminal calendar for plain-text tasks.

> v2 is a rewrite in progress. The first version lives on the [`v1`](https://github.com/RohimChou/roh-cal/tree/v1) branch.

## Run
```sh
cargo run --release -- --tasks testdata/tasks.md    # try it with the sample task file
```

| Option | Meaning |
| :- | :- |
| `--lang en\|zh-TW` | Screen language. Default: the system language, else English |
| `--tasks <path>` | Task file to show. Default: `~/.local/share/roh-cal/tasks.md` (`$XDG_DATA_HOME` respected); a missing default file just means no tasks |
| `-h`, `--help` | Print the options |
| `-v`, `-V`, `--version` | Print the version |

The task file format is in [docs/task-file-format.md](docs/task-file-format.md).

## Keys
| Key | Action |
| :- | :- |
| `h` `j` `k` `l` / arrows | Previous day / next week / previous week / next day |
| `t` | Jump to today |
| Mouse wheel / click | Move one week / select a day (hold `Shift` to select text in the terminal) |
| `q` / `Esc` / `Ctrl+C` | Quit |
