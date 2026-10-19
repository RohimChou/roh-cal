//! The sample task file built into the program, for `--sample`

/// The sample task file `testdata/tasks.md`, copied into the program when it is compiled, so `--sample` also
/// works without the repo (e.g. after `cargo install`). The tests read the same file, so changing it changes
/// both
pub const TEXT: &str = include_str!("../testdata/tasks.md");
