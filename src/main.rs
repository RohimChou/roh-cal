//! roh-cal: a terminal calendar that reads a plain-text task file

mod app;
mod calendar;
mod config;
mod edit;
mod files;
mod paths;
mod search;
mod tasks;
mod texts;
mod theme;
mod ui;

use app::App;
use app::Notice;
use clap::{Parser, builder::PossibleValuesParser};
use color_eyre::{Result, eyre::WrapErr};
use jiff::Zoned;
use ratatui::crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
};
use std::io::{ErrorKind, stdout};
use std::path::{Path, PathBuf};
use texts::Lang;

/// Reads the command-line arguments, task file, and config file, then runs the TUI until quit. If the config
/// file or the task file given by `--tasks` cannot be read, prints what is wrong (without starting the TUI)
fn main() -> Result<()> {
    // On errors, print a readable error message and backtrace
    color_eyre::install()?;
    // Read the command-line arguments: bad arguments, --help, --version print here and exit (before the TUI)
    let args = Args::parse();
    // Language: --lang → the system language → English
    let lang = args.lang.unwrap_or_else(Lang::from_system);
    // Task file: one given by --tasks must be readable; at the default location, missing means no tasks
    let (path, required) = match args.tasks {
        Some(path) => (Some(path), true),
        None => (paths::tasks_file(), false),
    };
    let text = match &path {
        Some(path) => load_tasks(path, required)?,
        None => String::new(),
    };
    // Config file: if missing, use all defaults; if it has a mistake, print which line here (before the TUI)
    let config_path = paths::config_file();
    let mut settings = match &config_path {
        Some(path) => config::load(path)?,
        None => config::Settings::default(),
    };
    // --theme beats the config file for this run only: closing the settings page saves only what changed on
    // the page, so --theme is not written to the config file
    if let Some(theme) = args.theme {
        settings.set_theme(&theme);
    }
    // Today: the date in the system time zone
    let today = Zoned::now().date();
    // Enter TUI mode (raw mode + alternate screen); the terminal is restored when the closure ends
    ratatui::run(|terminal| {
        // The mouse is on meanwhile: _mouse is dropped when the closure ends, which turns the mouse off
        let _mouse = MouseCapture::enable()?;
        let mut app = App::new(lang, today, Vec::new());
        // Items, mistakes and calendars from the task file
        app.show_tasks(text);
        app.settings = settings;
        app.apply_theme();
        // Changes made on the settings page are saved back to this same file
        // (if it does not exist yet, it is created only when something changed)
        app.config_path = config_path;
        // Checking and editing write back to this same file (the default file is kept even if it does not exist
        // yet; adding an item creates it)
        app.tasks_path = path;
        // Mistakes in the task file: say so on the bottom row right away (the tasks page shows which lines)
        if !app.warnings.is_empty() {
            app.notice = Some(Notice::Mistakes(app.warnings.len()));
        }
        app.run(terminal)
    })
}

/// Reads the task file's text. `required` = the user named this file (`--tasks`), so a missing file is
/// an error; a missing file at the default location just means no tasks (empty)
fn load_tasks(path: &Path, required: bool) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if !required && error.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(error) => {
            Err(error).wrap_err_with(|| format!("cannot read task file: {}", path.display()))
        }
    }
}

/// Keeps mouse events on while it lives; drop turns them off (on a normal exit, an error, or a panic alike)
///
/// `ratatui::run` only restores raw mode and the alternate screen; we must turn the mouse off ourselves,
/// or the terminal keeps receiving mouse escape codes after we exit
struct MouseCapture;

impl MouseCapture {
    /// Turns on mouse events (wheel, clicks)
    fn enable() -> Result<Self> {
        execute!(stdout(), EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for MouseCapture {
    /// Turns off mouse events
    fn drop(&mut self) {
        // drop cannot return an error, and nothing more can be done if turning it off fails, so ignore it
        let _ = execute!(stdout(), DisableMouseCapture);
    }
}

/// Command-line arguments, e.g. `roh-cal --lang zh-TW --tasks ~/notes/tasks.md`; clap generates `--help`.
/// The English that `--help` prints: the first line is `description` in `Cargo.toml` (`about` has no value),
/// and the argument texts are in `help`. clap uses `///` as help by default, but these `///` are for people
/// reading the code
#[derive(Debug, Default, PartialEq, Parser)]
#[command(
    version,
    disable_version_flag = true,
    about,
    long_about = None
)]
struct Args {
    /// `--lang`: if not given, follow the system language
    #[arg(
        long,
        value_name = "en|zh-TW",
        value_parser = parse_lang,
        help = "Screen language [default: the system language, else en]"
    )]
    lang: Option<Lang>,
    /// `--theme`: which built-in theme to use (only names in `theme::BUILTIN`; clap lists them in `--help`);
    /// if not given, use the config file's
    #[arg(
        long,
        value_name = "NAME",
        value_parser = PossibleValuesParser::new(theme::names()),
        help = "Color theme [default: the config file's, else nord]"
    )]
    theme: Option<String>,
    /// `--tasks`: path of the task file; if not given, use the default location
    #[arg(
        long,
        value_name = "PATH",
        help = "Task file to show [default: ~/.local/share/roh-cal/tasks.md]"
    )]
    tasks: Option<PathBuf>,
    /// `-v` / `-V` / `--version`: print the version and exit. clap's built-in version flag only takes `-V`,
    /// so the built-in one is turned off (`disable_version_flag`) and this one also takes lowercase `-v`
    /// (`-V` is not shown in `--help`)
    #[arg(
        short = 'v',
        short_alias = 'V',
        long,
        action = clap::ArgAction::Version,
        help = "Print version"
    )]
    version: (),
}

/// Turns the `--lang` value into a `Lang`; an unsupported language returns an error (clap adds which
/// argument and which value in front)
fn parse_lang(code: &str) -> Result<Lang, String> {
    Lang::from_code(code).ok_or_else(|| "supported: en, zh-TW".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    /// Parses a string like `"--lang en"` as arguments (for tests; the program name is added as argument 0)
    fn args(line: &str) -> Result<Args, clap::Error> {
        Args::try_parse_from(std::iter::once("roh-cal").chain(line.split_whitespace()))
    }

    #[test]
    fn args_definition_is_valid() {
        // The check clap recommends for every app: do the argument definitions conflict
        // (e.g. two arguments with the same name)
        Args::command().debug_assert();
    }

    #[test]
    fn parse_args_reads_lang_and_tasks() {
        let parsed = args("--tasks testdata/tasks.md --lang zh-TW").expect("valid args");
        assert_eq!(parsed.lang, Some(Lang::ZhTw));
        assert_eq!(parsed.tasks, Some(PathBuf::from("testdata/tasks.md")));
        assert_eq!(args("--lang en").expect("valid").lang, Some(Lang::En));
        assert_eq!(
            args("--theme terminal").expect("valid").theme.as_deref(),
            Some("terminal")
        );
        assert_eq!(args("").expect("no args"), Args::default());
    }

    #[test]
    fn parse_args_rejects_mistakes() {
        for line in [
            "--lang",
            "--tasks",
            "--lang ja",
            "--foo bar",
            "--theme solarized",
        ] {
            assert!(args(line).is_err(), "{line}");
        }
    }

    #[test]
    fn help_and_version_print_instead_of_running() {
        // --help / -h, --version / -v / -V: clap prints the help or version and exits
        // (reported as an Err; parse() in main prints it, then exits)
        for (line, kind) in [
            ("--help", ErrorKind::DisplayHelp),
            ("-h", ErrorKind::DisplayHelp),
            ("--version", ErrorKind::DisplayVersion),
            ("-v", ErrorKind::DisplayVersion),
            ("-V", ErrorKind::DisplayVersion),
        ] {
            assert_eq!(args(line).expect_err(line).kind(), kind, "{line}");
        }
        // The help comes from Cargo.toml's description and the `help` texts, never from the `///` comments
        // (those are for people reading the code)
        let help = Args::command().render_long_help().to_string();
        assert!(
            help.starts_with("A simple, keyboard-driven terminal calendar for plain-text tasks\n")
        );
        assert!(help.contains("Screen language") && help.contains("Task file to show"));
        assert!(
            !help.contains("follow the system language")
                && !help.contains("Command-line arguments")
        );
    }

    #[test]
    fn missing_task_file_is_an_error_only_when_asked_for() {
        // A file named with --tasks does not exist → error
        let missing = Path::new("testdata/no-such-file.md");
        assert!(load_tasks(missing, true).is_err());
        // The file at the default location does not exist → no tasks
        assert_eq!(load_tasks(missing, false).expect("no tasks"), "");
        // The sample file can be read (it has one date that is wrong on purpose)
        let text = load_tasks(Path::new("testdata/tasks.md"), true).expect("sample file");
        let parsed = tasks::parse(&text);
        assert_eq!((parsed.items.len(), parsed.warnings.len()), (57, 1));
    }
}
