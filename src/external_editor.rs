//! Opening the task file in the user's own editor (`o` on the tasks page): `$VISUAL`, else `$EDITOR`, else
//! `vi`, the same order git uses

use color_eyre::Result;
use ratatui::{
    DefaultTerminal,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{EnterAlternateScreen, enable_raw_mode},
    },
};
use std::ffi::OsString;
use std::io::stdout;
use std::path::Path;
use std::process::Command;

/// Editors that start on line N when given `+N` before the file (others, e.g. VS Code, would take `+N` for
/// a file name, so they only get the file)
const LINE_EDITORS: [&str; 7] = ["vi", "vim", "nvim", "nano", "emacs", "micro", "kak"];

/// The command that opens `path` on `line` (from 1; `None` = at the top): the editor in `visual`
/// (`$VISUAL`), else in `editor` (`$EDITOR`), else `vi`; an empty variable counts as unset. The variable may
/// carry its own arguments, e.g. `code --wait`
pub fn command(
    visual: Option<OsString>,
    editor: Option<OsString>,
    path: &Path,
    line: Option<usize>,
) -> Command {
    let chosen = [visual, editor]
        .into_iter()
        .flatten()
        .map(|value| value.to_string_lossy().into_owned())
        .find(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "vi".to_string());
    let mut words = chosen.split_whitespace();
    // `chosen` is not blank, so it has a first word
    let program = words.next().unwrap_or("vi");
    let mut command = Command::new(program);
    command.args(words);
    // `/opt/homebrew/bin/nvim` is nvim too
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program);
    if let Some(line) = line
        && LINE_EDITORS.contains(&name)
    {
        command.arg(format!("+{line}"));
    }
    command.arg(path);
    command
}

/// Hands the terminal to `command` (the editor) until it quits, then takes the terminal back; the next draw
/// redraws all of it. Returns whether the editor could be started (`false`: e.g. not installed); an error =
/// the terminal could not be taken back
pub fn open(terminal: &mut DefaultTerminal, command: &mut Command) -> Result<bool> {
    // Give the terminal back as it was before roh-cal: mouse off, raw mode off, the normal screen
    execute!(stdout(), DisableMouseCapture)?;
    ratatui::try_restore()?;
    // The editor runs in this same terminal; wait until it quits (its exit code does not matter, e.g. `:cq`)
    let started = command.status().is_ok();
    // Take the terminal back: the same steps as at start (`ratatui::run`, then the mouse in `main`)
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;
    Ok(started)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The program and its arguments, as text
    fn parts(command: &Command) -> (String, Vec<String>) {
        (
            command.get_program().to_string_lossy().into_owned(),
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
        )
    }

    #[test]
    fn visual_then_editor_then_vi_on_the_chosen_line() {
        let path = Path::new("/notes/tasks.md");
        let set = |value: &str| Some(OsString::from(value));
        let expect = |program: &str, args: &[&str]| {
            (
                program.to_string(),
                args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
            )
        };
        // $VISUAL beats $EDITOR; nvim starts on the line, also when named by its full path
        assert_eq!(
            parts(&command(set("nvim"), set("nano"), path, Some(12))),
            expect("nvim", &["+12", "/notes/tasks.md"])
        );
        assert_eq!(
            parts(&command(None, set("/opt/homebrew/bin/nvim"), path, Some(3))),
            expect("/opt/homebrew/bin/nvim", &["+3", "/notes/tasks.md"])
        );
        // An empty $VISUAL counts as unset; neither set → vi
        assert_eq!(
            parts(&command(set(" "), set("nano"), path, Some(1))),
            expect("nano", &["+1", "/notes/tasks.md"])
        );
        assert_eq!(
            parts(&command(None, None, path, Some(5))),
            expect("vi", &["+5", "/notes/tasks.md"])
        );
        // An editor that does not take `+N` only gets the file, after its own arguments
        assert_eq!(
            parts(&command(None, set("code --wait"), path, Some(12))),
            expect("code", &["--wait", "/notes/tasks.md"])
        );
        // Nothing chosen: only the file
        assert_eq!(
            parts(&command(set("nvim"), None, path, None)),
            expect("nvim", &["/notes/tasks.md"])
        );
    }
}
