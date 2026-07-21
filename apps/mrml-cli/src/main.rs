//! `mrml` — the command-line interface.
//!
//! Reads MJML from a file (or stdin), renders it to HTML with [`mrml::render`], and
//! writes the result to a file (or stdout). Diagnostics are printed to stderr and a
//! non-zero exit code is returned on failure.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use mrml::RenderOptions;

/// Compile an MJML document to responsive HTML.
#[derive(Debug, Parser)]
#[command(name = "mrml", version, about, long_about = None)]
struct Cli {
    /// Input `.mjml` file. Reads from stdin when omitted.
    input: Option<PathBuf>,

    /// Write HTML to this file instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Pretty-print the output. Accepted but not yet honored.
    #[arg(long)]
    beautify: bool,

    /// Minify the output. Accepted but not yet honored.
    #[arg(long)]
    minify: bool,

    /// Responsive breakpoint in pixels (columns collapse below this width).
    #[arg(long, value_name = "PX", default_value_t = 480)]
    breakpoint: u32,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let source = match read_source(cli.input.as_ref()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("mrml: error reading input: {err}");
            return ExitCode::FAILURE;
        }
    };

    let options = RenderOptions {
        breakpoint: cli.breakpoint,
        beautify: cli.beautify,
        minify: cli.minify,
    };

    let html = match mrml::render(&source, &options) {
        Ok(html) => html,
        Err(diagnostics) => {
            for diagnostic in &diagnostics {
                eprintln!("{diagnostic}");
            }
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = write_output(cli.output.as_ref(), &html) {
        eprintln!("mrml: error writing output: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

/// Read MJML source from `path`, or from stdin when `path` is `None`.
fn read_source(path: Option<&PathBuf>) -> std::io::Result<String> {
    match path {
        Some(path) => fs::read_to_string(path),
        None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

/// Write `html` to `path`, or to stdout when `path` is `None`.
fn write_output(path: Option<&PathBuf>, html: &str) -> std::io::Result<()> {
    match path {
        Some(path) => fs::write(path, html),
        None => {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(html.as_bytes())?;
            stdout.write_all(b"\n")?;
            Ok(())
        }
    }
}
