extern crate structopt;

mod paint;
mod parse_diff;

use std::io::{self, BufRead, ErrorKind, Write};
use std::process;
use std::str::FromStr;

use console::strip_ansi_codes;
use structopt::StructOpt;
use syntect::highlighting::{Color, ThemeSet};
use syntect::parsing::SyntaxReference;

#[derive(StructOpt, Debug)]
#[structopt(name = "delta",
            about = "A syntax-highlighter for git. Use 'delta | less -R' as core.pager in .gitconfig")]
struct Opt {
    /// Use diff highlighting colors appropriate for a light terminal
    /// background. This is the default.
    #[structopt(long = "light")]
    light: bool,

    /// Use diff highlighting colors appropriate for a dark terminal
    /// background.
    #[structopt(long = "dark")]
    dark: bool,

    #[structopt(long = "plus-color")]
    /// The background color (RGB hex) to use for added lines. The
    /// default is "#d0ffd0" if you are using --light, and "#013B01"
    /// if you are using --dark.
    plus_color: Option<String>,

    #[structopt(long = "minus-color")]
    /// The background color (RGB hex) to use for removed lines. The
    /// default is "#ffd0d0" if you are using --light, and "#3f0001" if
    /// you are using --dark.
    minus_color: Option<String>,

    #[structopt(long = "theme")]
    /// The syntax highlighting theme to use. Options are Light:
    /// ("InspiredGitHub", "Solarized (light)", "base16-ocean.light"),
    /// Dark: ("Solarized, (dark)", "base16-eighties.dark",
    /// "base16-mocha.dark", "base16-ocean.dark").
    theme: Option<String>,

    /// The width (in characters) of the diff highlighting. By
    /// default, the highlighting extends to the last character on
    /// each line
    #[structopt(short = "w", long = "width")]
    width: Option<usize>,
}

#[derive(PartialEq)]
enum State {
    Commit,
    DiffMeta,
    DiffHunk,
    Unknown,
}

fn main() {
    match delta() {
        Err(error) => {
            match error.kind() {
                ErrorKind::BrokenPipe => process::exit(0),
                _ => eprintln!("{}", error),
            }
        }
        _ => (),
    }
}

fn delta() -> std::io::Result<()> {
    let mut opt = Opt::from_args();
    let theme_set = ThemeSet::load_defaults();
    let paint_config = parse_args(&theme_set, &mut opt);

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let mut syntax: Option<&SyntaxReference> = None;
    let mut output = String::new();
    let mut state = State::Unknown;
    let mut did_emit_line: bool;

    for _line in stdin.lock().lines() {
        let raw_line = _line?;
        let line: String = strip_ansi_codes(&raw_line).to_string();
        did_emit_line = false;
        if line.starts_with("diff --") {
            state = State::DiffMeta;
            syntax = match parse_diff::get_file_extension_from_diff_line(&line) {
                Some(extension) => paint_config.syntax_set.find_syntax_by_extension(extension),
                None => None,
            };
        } else if line.starts_with("commit") {
            state = State::Commit;
        } else if line.starts_with("@@") {
            state = State::DiffHunk;
        } else if state == State::DiffHunk {
            match syntax {
                Some(syntax) => {
                    paint::paint_line(line, syntax, &paint_config, &mut output);
                    writeln!(stdout, "{}", output)?;
                    output.truncate(0);
                    did_emit_line = true;
                }
                None => (),
            }
        }
        if !did_emit_line {
            writeln!(stdout, "{}", raw_line)?;
        }
    }
    Ok(())
}

fn parse_args<'a>(theme_set: &'a ThemeSet, opt: &'a mut Opt) -> paint::Config<'a> {

    if opt.light && opt.dark {
        eprintln!("--light or --dark cannot be used together. Default is --light.");
        process::exit(1);
    }
    let theme_name = match opt.theme {
        Some(ref theme) => {
            if !theme_set.themes.contains_key(theme.as_str()) {
                eprintln!("Invalid theme: '{}'", theme);
                process::exit(1);
            }
            theme
        }
        None => {
            if !(opt.light || opt.dark) {
                opt.light = true;
            }
            match opt.light {
                true => "InspiredGitHub",
                false => "base16-mocha.dark",
            }
        }
    };
    let minus_color = opt.minus_color.as_ref().and_then(
        |s| Color::from_str(s).ok(),
    );
    let plus_color = opt.plus_color.as_ref().and_then(
        |s| Color::from_str(s).ok(),
    );
    paint::get_config(
        &theme_set.themes[theme_name],
        theme_name,
        plus_color,
        minus_color,
        opt.width,
    )
}
