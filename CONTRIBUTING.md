# Contributing
Thanks for taking the time to help with roh-cal.

## Bugs and ideas
Open an issue at <https://github.com/RohimChou/roh-cal/issues>. For a bug, please include what you did, what you expected and what happened, your OS and terminal, and, when it is about the task file, the few lines involved.

## Build and try
You need Rust (stable). roh-cal writes your changes back to the task file it shows, so try it on a copy of the sample file:

```sh
cp testdata/tasks.md /tmp/tasks.md
cargo run -- --tasks /tmp/tasks.md
```

## Before a pull request
Make sure these pass:

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

## Conventions
- Small commits, one idea each. Message: `<type>: <scope> | <summary>`, where type is one of `feat`, `fix`, `refac`, `doc`, `style`, `test`, `chore`, `perf`, `ci`
- A change to keys, command-line options, config names or the task file format updates `README.md` and `docs/` in the same commit
- Logic changes come with tests. Screen tests compare against screens computed separately by `tools/expected_screen.py`; after changing what is drawn, update that script, run `python3 tools/regen_ui_screens.py` and review `git diff src/ui/tests.rs`
- Characters drawn on screen must have an East Asian Width of N or Na: ambiguous-width characters (A) take two cells in CJK terminals and break the alignment
- Errors go through `color_eyre::Result` and `?`; no `unwrap()` outside tests
- Code comments are in plain English; a function's purpose goes in a `///` doc comment. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (in Traditional Chinese) maps the modules, the layout model and the screen tests
