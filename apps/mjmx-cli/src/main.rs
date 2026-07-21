//! `mjmx` — the command-line interface.
//!
//! Two modes:
//!
//! - **render** (default): `mjmx <file>` reads MJML from a file (or stdin), renders it
//!   to HTML with [`mjmx::render`], and writes the result to a file (or stdout). The
//!   optional `--validate` flag runs the structural linter first and prints its
//!   diagnostics to stderr; `--strict` additionally refuses to render when the linter
//!   found any error-severity diagnostics.
//! - **lint**: `mjmx lint <file>` parses and validates the document with
//!   [`mjmx::lint`], prints diagnostics to stderr, and exits non-zero if any are
//!   error-severity. It never renders.
//! - **fmt**: `mjmx fmt <file>` parses the document and prints canonical MJML source
//!   with [`mjmx::format`]. By default it writes the formatted source to stdout;
//!   `--write`/`-w` rewrites the file in place, and `--check` writes nothing but
//!   exits non-zero when the file is not already formatted.
//! - **dev**: `mjmx dev <file>` starts a local live-preview server for a file: it
//!   renders the file, serves a browser preview, watches the file (and its
//!   directory, for `<mj-include>` partials) for changes, and hot-reloads the
//!   browser over a WebSocket on every save. Runs until Ctrl-C.
//!
//! Diagnostics are always written to stderr; a non-zero exit code is returned on
//! failure.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use mjmx::{Diagnostic, FormatOptions, RenderOptions, Severity};
use mjmx_dev_server::DevOptions;

/// Compile or validate an MJML document.
#[derive(Debug, Parser)]
#[command(name = "mjmx", version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    /// Subcommand to run; when omitted, the top-level render arguments are used.
    #[command(subcommand)]
    command: Option<Command>,

    /// Render arguments (used when no subcommand is given).
    #[command(flatten)]
    render: RenderArgs,
}

/// The available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Validate an MJML document structurally without rendering it.
    Lint(LintArgs),
    /// Format an MJML document into canonical MJML source.
    Fmt(FmtArgs),
    /// Serve a live browser preview with hot-reload on save.
    Dev(DevArgs),
}

/// Arguments for the default render path.
#[derive(Debug, Args)]
struct RenderArgs {
    /// Input `.mjml` file. Reads from stdin when omitted.
    input: Option<PathBuf>,

    /// Write HTML to this file instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Pretty-print (re-indent) the output. If combined with `--minify`,
    /// `--minify` wins.
    #[arg(long)]
    beautify: bool,

    /// Minify the output (strip insignificant whitespace). Takes precedence over
    /// `--beautify`.
    #[arg(long)]
    minify: bool,

    /// Responsive breakpoint in pixels (columns collapse below this width).
    #[arg(long, value_name = "PX", default_value_t = 480)]
    breakpoint: u32,

    /// Run the structural linter before rendering and print diagnostics to stderr.
    #[arg(long)]
    validate: bool,

    /// With `--validate`, refuse to render if the linter reports any errors.
    #[arg(long, requires = "validate")]
    strict: bool,
}

/// Arguments for the `lint` subcommand.
#[derive(Debug, Args)]
struct LintArgs {
    /// Input `.mjml` file. Reads from stdin when omitted.
    input: Option<PathBuf>,
}

/// Arguments for the `fmt` subcommand.
#[derive(Debug, Args)]
struct FmtArgs {
    /// Input `.mjml` file. Reads from stdin when omitted.
    input: Option<PathBuf>,

    /// Rewrite the file in place with the formatted output (requires a file input).
    #[arg(short, long, conflicts_with = "check")]
    write: bool,

    /// Do not write anything; exit non-zero if the input is not already formatted.
    #[arg(long)]
    check: bool,
}

/// Arguments for the `dev` subcommand.
#[derive(Debug, Args)]
struct DevArgs {
    /// Input `.mjml` file to preview (required; there is nothing to watch on stdin).
    input: PathBuf,

    /// TCP port to bind on `127.0.0.1`.
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Open the preview URL in the default browser on startup.
    #[arg(long)]
    open: bool,

    /// Pretty-print the previewed HTML. If combined with `--minify`, `--minify` wins.
    #[arg(long)]
    beautify: bool,

    /// Minify the previewed HTML. Takes precedence over `--beautify`.
    #[arg(long)]
    minify: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Lint(args)) => run_lint(&args),
        Some(Command::Fmt(args)) => run_fmt(&args),
        Some(Command::Dev(args)) => run_dev(args),
        None => run_render(&cli.render),
    }
}

/// The `dev` subcommand: start the live-preview server, blocking until Ctrl-C.
fn run_dev(args: DevArgs) -> ExitCode {
    let options = DevOptions {
        port: args.port,
        open: args.open,
        render: RenderOptions {
            breakpoint: RenderOptions::default().breakpoint,
            beautify: args.beautify,
            minify: args.minify,
        },
    };

    match mjmx_dev_server::run(args.input, options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("mjmx: dev server error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// The `fmt` subcommand: parse + format, then print / rewrite / check.
fn run_fmt(args: &FmtArgs) -> ExitCode {
    let source = match read_source(args.input.as_ref()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("mjmx: error reading input: {err}");
            return ExitCode::FAILURE;
        }
    };

    let formatted = match mjmx::format(&source, &FormatOptions::default()) {
        Ok(formatted) => formatted,
        Err(diagnostics) => {
            print_diagnostics(&diagnostics);
            return ExitCode::FAILURE;
        }
    };

    if args.check {
        if formatted == source {
            return ExitCode::SUCCESS;
        }
        match args.input.as_ref() {
            Some(path) => eprintln!("mjmx: {} is not formatted", path.display()),
            None => eprintln!("mjmx: input is not formatted"),
        }
        return ExitCode::FAILURE;
    }

    if args.write {
        let Some(path) = args.input.as_ref() else {
            eprintln!("mjmx: --write requires a file input (cannot rewrite stdin)");
            return ExitCode::FAILURE;
        };
        if let Err(err) = fs::write(path, &formatted) {
            eprintln!("mjmx: error writing output: {err}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }

    // Default: print to stdout. `formatted` already ends with a newline.
    print!("{formatted}");
    ExitCode::SUCCESS
}

/// The `lint` subcommand: parse + validate, print diagnostics, exit non-zero on error.
fn run_lint(args: &LintArgs) -> ExitCode {
    let source = match read_source(args.input.as_ref()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("mjmx: error reading input: {err}");
            return ExitCode::FAILURE;
        }
    };

    let diagnostics = mjmx::lint(&source);
    print_diagnostics(&diagnostics);
    if has_errors(&diagnostics) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// The default render path, with optional `--validate` / `--strict` linting.
fn run_render(args: &RenderArgs) -> ExitCode {
    let source = match read_source(args.input.as_ref()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("mjmx: error reading input: {err}");
            return ExitCode::FAILURE;
        }
    };

    if args.validate {
        let diagnostics = mjmx::lint(&source);
        print_diagnostics(&diagnostics);
        if args.strict && has_errors(&diagnostics) {
            eprintln!("mjmx: refusing to render due to lint errors (--strict)");
            return ExitCode::FAILURE;
        }
    }

    let options = RenderOptions {
        breakpoint: args.breakpoint,
        beautify: args.beautify,
        minify: args.minify,
    };

    // A file input has a base directory, so `<mj-include>` resolves relative to it via
    // `render_file`; stdin has no base directory and uses the plain string renderer.
    let rendered = match args.input.as_ref() {
        Some(path) => mjmx::render_file(path, &options),
        None => mjmx::render(&source, &options),
    };

    let html = match rendered {
        Ok(html) => html,
        Err(diagnostics) => {
            print_diagnostics(&diagnostics);
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = write_output(args.output.as_ref(), &html) {
        eprintln!("mjmx: error writing output: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

/// Whether any diagnostic is error-severity.
fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// Print every diagnostic to stderr using its `Display` formatting.
fn print_diagnostics(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}");
    }
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
