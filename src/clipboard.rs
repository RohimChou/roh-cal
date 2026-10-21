//! Putting text on the system clipboard (letting go of a drag in the day popup): the text goes to the
//! platform's clipboard command, `pbcopy` on macOS, else `wl-copy`, `xclip` or `xsel`. This works inside tmux
//! too, where the terminal escape code for the clipboard (OSC 52) is not let through by default

use color_eyre::{Result, eyre::eyre};
use std::io::Write;
use std::process::{Command, Stdio};

/// The clipboard commands to try, in order, each with its arguments: `pbcopy` on macOS (`macos`); elsewhere
/// Wayland's `wl-copy` first when it runs (`wayland`, `$WAYLAND_DISPLAY` is set), then X11's `xclip`, `xsel`
fn commands(macos: bool, wayland: bool) -> Vec<&'static [&'static str]> {
    if macos {
        return vec![&["pbcopy"]];
    }
    let x11: [&'static [&'static str]; 2] = [
        &["xclip", "-selection", "clipboard"],
        &["xsel", "--clipboard", "--input"],
    ];
    let wayland_first = wayland.then_some(&["wl-copy"][..]);
    wayland_first.into_iter().chain(x11).collect()
}

/// Puts `text` on the system clipboard (see `commands`)
pub fn copy(text: &str) -> Result<()> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    copy_with(&commands(cfg!(target_os = "macos"), wayland), text)
}

/// Gives `text` to the first of `commands` that starts (on its standard input) and waits for it to finish;
/// an error when none starts, or the one that starts fails
fn copy_with(commands: &[&[&str]], text: &str) -> Result<()> {
    for command in commands {
        let Some((program, args)) = command.split_first() else {
            continue;
        };
        // Its output must not land on the screen roh-cal draws
        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            // Not installed: try the next one
            continue;
        };
        // Dropping the input at the end of this block closes it, so the command sees the end of the text
        if let Some(mut input) = child.stdin.take() {
            input.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        return if status.success() {
            Ok(())
        } else {
            Err(eyre!("{program} failed ({status})"))
        };
    }
    Err(eyre!(
        "no clipboard command found ({})",
        commands
            .iter()
            .filter_map(|command| command.first())
            .copied()
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_uses_pbcopy_and_wayland_comes_before_x11() {
        assert_eq!(commands(true, true), [&["pbcopy"][..]]);
        let names = |wayland| -> Vec<&str> {
            commands(false, wayland)
                .iter()
                .map(|command| command[0])
                .collect()
        };
        assert_eq!(names(true), ["wl-copy", "xclip", "xsel"]);
        assert_eq!(names(false), ["xclip", "xsel"]);
    }

    #[test]
    fn the_first_command_that_starts_gets_the_text() {
        // A command that is not installed is skipped; `sh` writes what it gets to a file
        let path = std::env::temp_dir().join(format!("roh-cal-clipboard-{}", std::process::id()));
        let write = format!("cat > '{}'", path.display());
        let commands: [&[&str]; 2] = [&["roh-cal-no-such-command"], &["sh", "-c", &write]];
        copy_with(&commands, "Send the weekly update 🎂\nPack").expect("copied");
        let copied = std::fs::read_to_string(&path).expect("read");
        let _ = std::fs::remove_file(&path);
        assert_eq!(copied, "Send the weekly update 🎂\nPack");
        // The one that starts fails: an error, the ones after it are not tried
        let fails: [&[&str]; 2] = [&["sh", "-c", "exit 3"], &["sh", "-c", "cat"]];
        assert!(copy_with(&fails, "x").is_err());
        // None starts: an error that names them
        let none: [&[&str]; 1] = [&["roh-cal-no-such-command"]];
        let error = copy_with(&none, "x").expect_err("nothing to copy with");
        assert!(error.to_string().contains("roh-cal-no-such-command"));
    }
}
