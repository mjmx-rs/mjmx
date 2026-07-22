//! `mjmx` — the command-line interface.
//!
//! Modes:
//!
//! - **render** (default): `mjmx <file>` reads MJML from a file (or stdin), renders it
//!   to HTML with [`mjmx::render`], and writes the result to a file (or stdout). The
//!   optional `--validate` flag prints the diagnostics the render reported but did not
//!   fail on; `--validation-level strict` (or its older spelling `--strict`) refuses to
//!   render when validation found any error-severity diagnostics.
//! - **lint**: `mjmx lint <file>` parses and validates the document with
//!   [`mjmx::lint`], prints diagnostics to stderr, and exits non-zero if any are
//!   error-severity. It never renders.
//! - **fmt**: `mjmx fmt <file>` parses the document and prints canonical MJML source
//!   with [`mjmx::format`]. By default it writes the formatted source to stdout;
//!   `--write`/`-w` rewrites the file in place, and `--check` writes nothing but
//!   exits non-zero when the file is not already formatted.
//! - **migrate**: `mjmx migrate <file>` converts an MJML **v3** document to v4 with
//!   [`mjmx::migrate`] and prints the migrated MJML source. `--write`/`-w` rewrites
//!   the file in place; notes describing every change go to stderr.
//! - **dev**: `mjmx dev <file>` starts a local live-preview server for a file: it
//!   renders the file, serves a browser preview, watches the file (and its
//!   directory, for `<mj-include>` partials) for changes, and hot-reloads the
//!   browser over a WebSocket on every save. Runs until Ctrl-C.
//! - **lsp**: `mjmx lsp` speaks the Language Server Protocol over stdin/stdout, for
//!   editors. It is the same server as the standalone `mjmx-language-server`
//!   binary, exposed here so a single `mjmx` on `PATH` is enough. Takes no
//!   arguments and runs until the client closes the connection.
//!
//! Diagnostics are always written to stderr; a non-zero exit code is returned on
//! failure.
//!
//! # Render options and mjml
//!
//! mjml's own CLI passes the `mjml2html` options through a dotted namespace
//! (`--config.keepComments false`, `--config.validationLevel strict`). We expose them
//! as **flat, idiomatic flags** instead (`--no-keep-comments`, `--validation-level
//! strict`): they show up in `--help`, they get shell completion and typo suggestions
//! from clap, and they match the style `--beautify` / `--minify` / `--breakpoint`
//! already established here. Every option's help text says whether it matches an mjml
//! option or is an mjmx extension.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use mjmx::{
    Diagnostic, FontFace, FormatOptions, MinifyOptions, RenderOptions, Severity, ValidationLevel,
};
use mjmx_dev_server::DevOptions;
use serde::de::{MapAccess, Visitor};

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
    /// Convert an MJML v3 document to v4 MJML source.
    Migrate(MigrateArgs),
    /// Serve a live browser preview with hot-reload on save.
    Dev(DevArgs),
    /// Speak the Language Server Protocol over stdin/stdout (for editors).
    Lsp,
}

/// The render options shared by the default render path and `mjmx dev`.
///
/// One flag per [`RenderOptions`] field, so the conversion below is an exhaustive
/// struct literal — a future field added to `RenderOptions` breaks this build instead
/// of being silently left at its default on every surface.
#[derive(Debug, Args)]
struct RenderOptionArgs {
    /// Pretty-print (re-indent) the output. If combined with `--minify`, `--minify`
    /// wins. [mjml: `beautify`]
    #[arg(long)]
    beautify: bool,

    /// Minify the output (strip insignificant whitespace). Takes precedence over
    /// `--beautify`. [mjml: `minify`]
    #[arg(long)]
    minify: bool,

    /// How to minify, as a JSON object in mjml's own shape — e.g.
    /// `'{"minifyCSS": true, "removeEmptyAttributes": false}'`. Recognised keys are
    /// the four html-minifier switches mjml sets: `collapseWhitespace` (default
    /// true), `minifyCSS` (false), `caseSensitive` (true) and
    /// `removeEmptyAttributes` (true). Only consulted with `--minify`.
    /// [mjml: `minifyOptions`]
    #[arg(long, value_name = "JSON")]
    minify_options: Option<String>,

    /// Responsive breakpoint in pixels (columns collapse below this width).
    /// [mjmx extension: mjml hardcodes 480px, only `<mj-breakpoint>` changes it]
    #[arg(long, value_name = "PX", default_value_t = 480)]
    breakpoint: u32,

    /// Keep MJML source comments in the output HTML. On by default; pass
    /// `--no-keep-comments` to strip them. [mjml: `keepComments`]
    #[arg(long, overrides_with = "no_keep_comments")]
    keep_comments: bool,

    /// Strip MJML source comments from the output HTML.
    /// [mjml: `keepComments: false`]
    #[arg(long, overrides_with = "keep_comments")]
    no_keep_comments: bool,

    /// How hard to insist the document validates: report and render anyway (soft),
    /// refuse to render on any error (strict), or do not validate (skip).
    /// [mjml: `validationLevel`]
    #[arg(long, value_name = "LEVEL", default_value_t = ValidationLevelArg::Soft)]
    validation_level: ValidationLevelArg,

    /// Directory (or file) that relative `<mj-include>` paths resolve against.
    /// Overrides the input file's own directory. [mjml: `filePath`]
    #[arg(long, value_name = "PATH")]
    file_path: Option<PathBuf>,

    /// Web font to import, *replacing* the built-in table rather than extending it
    /// (mjml's own semantics). Either a JSON object (`'{"Inter":"https://…"}'`) or a
    /// single `NAME=URL` pair; repeat the flag to declare more. [mjml: `fonts`]
    #[arg(long, value_name = "JSON|NAME=URL")]
    fonts: Vec<String>,

    /// Skip `<mj-include>` tags instead of resolving them. They are *dropped*, along
    /// with the content they would have pulled in — mjml's own behavior.
    /// [mjml: `ignoreIncludes`]
    #[arg(long)]
    ignore_includes: bool,

    /// Also emit the responsive column rules in an `@media only print` block, so a
    /// printed email keeps its desktop layout. [mjml: `printerSupport`]
    #[arg(long)]
    printer_support: bool,
}

/// `--validation-level` values, spelled exactly as mjml's `validationLevel` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ValidationLevelArg {
    /// Validate, report, and render anyway. mjml's default, and ours.
    Soft,
    /// Validate and refuse to render if any error-severity diagnostic is produced.
    Strict,
    /// Do not validate at all.
    Skip,
}

impl std::fmt::Display for ValidationLevelArg {
    /// Renders as the mjml spelling, which is also what `--help` shows as the default.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Soft => "soft",
            Self::Strict => "strict",
            Self::Skip => "skip",
        })
    }
}

impl From<ValidationLevelArg> for ValidationLevel {
    fn from(value: ValidationLevelArg) -> Self {
        match value {
            ValidationLevelArg::Soft => Self::Soft,
            ValidationLevelArg::Strict => Self::Strict,
            ValidationLevelArg::Skip => Self::Skip,
        }
    }
}

impl RenderOptionArgs {
    /// Build the [`RenderOptions`] these flags describe.
    ///
    /// Fails only on a malformed `--fonts` or `--minify-options` spec; every other
    /// flag is validated by clap.
    fn to_render_options(&self) -> Result<RenderOptions, String> {
        Ok(RenderOptions {
            breakpoint: self.breakpoint,
            beautify: self.beautify,
            minify: self.minify,
            minify_options: parse_minify_options(self.minify_options.as_deref())?,
            // `--keep-comments` is the default, so the negative flag is what carries
            // information; the positive one exists to override an earlier negative.
            keep_comments: !self.no_keep_comments,
            validation_level: self.validation_level.into(),
            file_path: self.file_path.clone(),
            fonts: parse_fonts(&self.fonts)?,
            ignore_includes: self.ignore_includes,
            printer_support: self.printer_support,
        })
    }
}

/// Arguments for the default render path.
#[derive(Debug, Args)]
struct RenderArgs {
    /// Input `.mjml` file. Reads from stdin when omitted.
    input: Option<PathBuf>,

    /// Write HTML to this file instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// The mjml2html options.
    #[command(flatten)]
    options: RenderOptionArgs,

    /// Print the validation diagnostics the render reported but did not fail on.
    ///
    /// Reporting only: what is *enforced* is `--validation-level`. With
    /// `--validation-level skip` nothing is validated, so this prints nothing.
    #[arg(long)]
    validate: bool,

    /// Refuse to render if validation reports any errors.
    ///
    /// The older spelling of `--validation-level strict`, kept working; the two are
    /// the same switch and may not both be given.
    #[arg(long, conflicts_with = "validation_level")]
    strict: bool,

    /// Allow `<mj-include>` to fetch `http(s)://` URLs (requires a file input and a
    /// binary built with `--features http-loader`).
    ///
    /// Off by default: a template that names a URL decides what this process
    /// connects to. Only enable it for templates you trust.
    #[arg(long)]
    allow_http_includes: bool,
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

    /// Spaces per indentation level. [mjmx extension: mjml has no formatter]
    #[arg(long, value_name = "N", default_value_t = FormatOptions::default().indent_width)]
    indent_width: usize,
}

/// Arguments for the `migrate` subcommand.
#[derive(Debug, Args)]
struct MigrateArgs {
    /// Input `.mjml` file. Reads from stdin when omitted.
    input: Option<PathBuf>,

    /// Rewrite the file in place with the migrated output (requires a file input).
    #[arg(short, long)]
    write: bool,

    /// Do not print the per-change notes to stderr.
    #[arg(short, long)]
    quiet: bool,

    /// Spaces per indentation level in the migrated source.
    /// [mjmx extension: mjml-migrate does not re-indent]
    #[arg(long, value_name = "N", default_value_t = FormatOptions::default().indent_width)]
    indent_width: usize,
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

    /// The mjml2html options the preview renders with — the same set the default
    /// render path accepts, so what you preview is what `mjmx <file>` produces.
    #[command(flatten)]
    options: RenderOptionArgs,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Lint(args)) => run_lint(&args),
        Some(Command::Fmt(args)) => run_fmt(&args),
        Some(Command::Migrate(args)) => run_migrate(&args),
        Some(Command::Dev(args)) => run_dev(args),
        Some(Command::Lsp) => run_lsp(),
        None => run_render(&cli.render),
    }
}

/// The `lsp` subcommand: serve the protocol over stdio until the client hangs up.
///
/// Identical to running the standalone `mjmx-language-server` binary — both are a
/// single call into the crate's synchronous front door, which owns its own runtime.
fn run_lsp() -> ExitCode {
    mjmx_language_server::run();
    ExitCode::SUCCESS
}

/// The `dev` subcommand: start the live-preview server, blocking until Ctrl-C.
fn run_dev(args: DevArgs) -> ExitCode {
    let render = match args.options.to_render_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("mjmx: {message}");
            return ExitCode::FAILURE;
        }
    };
    let options = DevOptions {
        port: args.port,
        open: args.open,
        render,
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

    let format_options = FormatOptions {
        indent_width: args.indent_width,
    };
    let formatted = match mjmx::format(&source, &format_options) {
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

/// The `migrate` subcommand: parse + migrate v3 → v4, then print or rewrite.
fn run_migrate(args: &MigrateArgs) -> ExitCode {
    let source = match read_source(args.input.as_ref()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("mjmx: error reading input: {err}");
            return ExitCode::FAILURE;
        }
    };

    let format_options = FormatOptions {
        indent_width: args.indent_width,
    };
    let (migrated, notes) = match mjmx::migrate_with_notes(&source, &format_options) {
        Ok(result) => result,
        Err(diagnostics) => {
            print_diagnostics(&diagnostics);
            return ExitCode::FAILURE;
        }
    };

    // Notes go to stderr so stdout stays a clean stream of migrated MJML.
    if !args.quiet {
        print_diagnostics(&notes);
    }

    if args.write {
        let Some(path) = args.input.as_ref() else {
            eprintln!("mjmx: --write requires a file input (cannot rewrite stdin)");
            return ExitCode::FAILURE;
        };
        if let Err(err) = fs::write(path, &migrated) {
            eprintln!("mjmx: error writing output: {err}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }

    // `migrated` already ends with a newline.
    print!("{migrated}");
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

/// The default render path, with optional `--validate` reporting.
fn run_render(args: &RenderArgs) -> ExitCode {
    let source = match read_source(args.input.as_ref()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("mjmx: error reading input: {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut options = match args.options.to_render_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("mjmx: {message}");
            return ExitCode::FAILURE;
        }
    };

    // `--strict` predates `--validation-level` and means exactly the same thing. The
    // two conflict at the clap level, so at most one of them is set and there is no
    // precedence question; validation itself is the umbrella crate's job now, not a
    // separate `mjmx::lint` pre-pass.
    if args.strict {
        options.validation_level = ValidationLevel::Strict;
    }

    // A file input has a base directory, so `<mj-include>` resolves relative to it via
    // `render_file`; stdin has no base directory and uses the plain string renderer.
    let rendered = match args.input.as_ref() {
        Some(path) if args.allow_http_includes => render_file_allow_http(path, &options),
        Some(path) => mjmx::render_file_with_diagnostics(path, &options),
        None if args.allow_http_includes => {
            eprintln!("mjmx: --allow-http-includes requires a file input");
            return ExitCode::FAILURE;
        }
        None => mjmx::render_with_diagnostics(&source, &options),
    };

    let (html, reported) = match rendered {
        Ok(result) => result,
        Err(diagnostics) => {
            print_diagnostics(&diagnostics);
            return ExitCode::FAILURE;
        }
    };

    // Soft findings are silent unless asked for, so the default `mjmx file.mjml` stays
    // a clean HTML-on-stdout pipe.
    if args.validate {
        print_diagnostics(&reported);
    }

    if let Err(err) = write_output(args.output.as_ref(), &html) {
        eprintln!("mjmx: error writing output: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

/// Render with network includes enabled — the `--allow-http-includes` path.
///
/// Two definitions: the real one when the binary was built with the `http-loader`
/// feature, and a refusal when it was not, so the flag always parses and always
/// explains itself instead of being silently absent from `--help`.
#[cfg(feature = "http-loader")]
fn render_file_allow_http(
    path: &std::path::Path,
    options: &RenderOptions,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    mjmx::render_file_allow_http_with_diagnostics(path, options)
}

/// The no-network build's stand-in for `render_file_allow_http`.
#[cfg(not(feature = "http-loader"))]
fn render_file_allow_http(
    _path: &std::path::Path,
    _options: &RenderOptions,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    Err(vec![Diagnostic::error(
        "this mjmx was built without HTTP include support; \
         rebuild with `--features http-loader` to use --allow-http-includes",
    )])
}

/// Whether any diagnostic is error-severity.
fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// A `{"Name": "https://…"}` font table, deserialized **in source order**.
///
/// `serde_json::Map` is a `BTreeMap` unless the `preserve_order` feature is on, and the
/// order is observable — it decides the order of the `<link>` tags in the rendered head
/// — so the entries are collected as a `Vec` rather than through `Map`.
struct FontTable(Vec<(String, String)>);

impl<'de> serde::Deserialize<'de> for FontTable {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TableVisitor;

        impl<'de> Visitor<'de> for TableVisitor {
            type Value = FontTable;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object mapping font names to stylesheet URLs")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<FontTable, M::Error> {
                let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((name, href)) = map.next_entry::<String, String>()? {
                    entries.push((name, href));
                }
                Ok(FontTable(entries))
            }
        }

        deserializer.deserialize_map(TableVisitor)
    }
}

/// Turn the `--fonts` occurrences into mjml's *replacement* font table.
///
/// `None` — no occurrence — keeps mjml's five built-in Google fonts. Any occurrence
/// replaces them wholesale, which is mjml's `fonts` semantics (replace, never merge).
/// Occurrences accumulate in the order given; a repeated name updates the URL of the
/// existing entry in place rather than appending a duplicate.
fn parse_fonts(specs: &[String]) -> Result<Option<Vec<FontFace<'static>>>, String> {
    if specs.is_empty() {
        return Ok(None);
    }
    let mut faces: Vec<FontFace<'static>> = Vec::new();
    for spec in specs {
        for (name, href) in parse_font_spec(spec)? {
            match faces.iter_mut().find(|face| face.name == name) {
                Some(face) => face.href = href.into(),
                None => faces.push(FontFace::new(name, href)),
            }
        }
    }
    Ok(Some(faces))
}

/// Parse one `--fonts` occurrence: a JSON object in mjml's own shape, or `NAME=URL`.
fn parse_font_spec(spec: &str) -> Result<Vec<(String, String)>, String> {
    let spec = spec.trim();
    if spec.starts_with('{') {
        let FontTable(entries) = serde_json::from_str(spec)
            .map_err(|err| format!("--fonts: invalid JSON font table: {err}"))?;
        return Ok(entries);
    }
    let Some((name, href)) = spec.split_once('=') else {
        return Err(format!(
            "--fonts: expected a JSON object or NAME=URL, got '{spec}'"
        ));
    };
    let (name, href) = (name.trim(), href.trim());
    if name.is_empty() || href.is_empty() {
        return Err(format!(
            "--fonts: both a name and a URL are required in '{spec}'"
        ));
    }
    Ok(vec![(name.to_owned(), href.to_owned())])
}

/// mjml's `minifyOptions` object, under its own camelCase key names.
///
/// `deny_unknown_fields` is deliberate: mjml would silently pass an unrecognised key
/// through to html-minifier, but this crate implements exactly the four switches mjml
/// sets, so accepting `removeComments` would promise something that never happens.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MinifyOptionsSpec {
    collapse_whitespace: Option<bool>,
    #[serde(rename = "minifyCSS")]
    minify_css: Option<bool>,
    case_sensitive: Option<bool>,
    remove_empty_attributes: Option<bool>,
}

/// Parse `--minify-options`; `None` (the flag omitted) is mjml's `minifyOptions: {}`.
fn parse_minify_options(spec: Option<&str>) -> Result<MinifyOptions, String> {
    let defaults = MinifyOptions::default();
    let Some(spec) = spec else {
        return Ok(defaults);
    };
    let parsed: MinifyOptionsSpec = serde_json::from_str(spec.trim()).map_err(|err| {
        format!(
            "--minify-options: invalid JSON object: {err} (recognised keys: \
             collapseWhitespace, minifyCSS, caseSensitive, removeEmptyAttributes)"
        )
    })?;
    Ok(MinifyOptions {
        collapse_whitespace: parsed
            .collapse_whitespace
            .unwrap_or(defaults.collapse_whitespace),
        minify_css: parsed.minify_css.unwrap_or(defaults.minify_css),
        case_sensitive: parsed.case_sensitive.unwrap_or(defaults.case_sensitive),
        remove_empty_attributes: parsed
            .remove_empty_attributes
            .unwrap_or(defaults.remove_empty_attributes),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a `mjmx …` command line into the default render path's arguments.
    fn render_args(argv: &[&str]) -> RenderArgs {
        let cli = Cli::try_parse_from(std::iter::once("mjmx").chain(argv.iter().copied()))
            .expect("arguments should parse");
        assert!(cli.command.is_none(), "expected the default render path");
        cli.render
    }

    /// Parse a command line and build the render options it describes.
    fn render_options(argv: &[&str]) -> RenderOptions {
        render_args(argv)
            .options
            .to_render_options()
            .expect("options should build")
    }

    #[test]
    fn defaults_match_mjml() {
        let options = render_options(&["in.mjml"]);
        assert_eq!(options.breakpoint, 480);
        assert!(!options.beautify);
        assert!(!options.minify);
        // mjml's `keepComments` defaults to true.
        assert!(options.keep_comments);
        assert_eq!(options.validation_level, ValidationLevel::Soft);
        assert_eq!(options.file_path, None);
        assert_eq!(options.fonts, None);
        assert!(!options.ignore_includes);
        assert!(!options.printer_support);
        assert_eq!(options.minify_options, MinifyOptions::default());
    }

    #[test]
    fn ignore_includes_and_printer_support_are_plain_switches() {
        let options = render_options(&["in.mjml", "--ignore-includes", "--printer-support"]);
        assert!(options.ignore_includes);
        assert!(options.printer_support);
    }

    #[test]
    fn minify_options_json_sets_only_the_keys_it_names() {
        // mjml spreads `minifyOptions` over the defaults, so omitted keys keep theirs.
        let options = render_options(&["in.mjml", "--minify-options", r#"{"minifyCSS": true}"#]);
        let defaults = MinifyOptions::default();
        assert!(options.minify_options.minify_css);
        assert_eq!(
            options.minify_options.collapse_whitespace,
            defaults.collapse_whitespace
        );
        assert_eq!(
            options.minify_options.remove_empty_attributes,
            defaults.remove_empty_attributes
        );
    }

    #[test]
    fn minify_options_accepts_every_mjml_key() {
        let options = render_options(&[
            "in.mjml",
            "--minify-options",
            r#"{"collapseWhitespace": false, "minifyCSS": true,
                "caseSensitive": false, "removeEmptyAttributes": true}"#,
        ]);
        assert_eq!(
            options.minify_options,
            MinifyOptions {
                collapse_whitespace: false,
                minify_css: true,
                case_sensitive: false,
                remove_empty_attributes: true,
            }
        );
    }

    #[test]
    fn minify_options_rejects_unknown_and_malformed_input() {
        // An unrecognised html-minifier switch is refused rather than silently
        // ignored: this build implements exactly the four mjml sets.
        let err = render_args(&["in.mjml", "--minify-options", r#"{"removeComments": true}"#])
            .options
            .to_render_options()
            .expect_err("unknown key must be refused");
        assert!(err.contains("removeComments"), "{err}");

        let err = render_args(&["in.mjml", "--minify-options", "not json"])
            .options
            .to_render_options()
            .expect_err("malformed JSON must be refused");
        assert!(err.contains("--minify-options"), "{err}");
    }

    #[test]
    fn no_keep_comments_switches_comments_off() {
        assert!(!render_options(&["in.mjml", "--no-keep-comments"]).keep_comments);
    }

    #[test]
    fn keep_comments_flags_are_last_wins() {
        assert!(
            render_options(&["in.mjml", "--no-keep-comments", "--keep-comments"]).keep_comments
        );
        assert!(
            !render_options(&["in.mjml", "--keep-comments", "--no-keep-comments"]).keep_comments
        );
    }

    #[test]
    fn validation_level_accepts_every_mjml_spelling() {
        for (flag, expected) in [
            ("soft", ValidationLevel::Soft),
            ("strict", ValidationLevel::Strict),
            ("skip", ValidationLevel::Skip),
        ] {
            let options = render_options(&["in.mjml", "--validation-level", flag]);
            assert_eq!(
                options.validation_level, expected,
                "for --validation-level {flag}"
            );
        }
    }

    #[test]
    fn validation_level_rejects_unknown_values() {
        assert!(Cli::try_parse_from(["mjmx", "in.mjml", "--validation-level", "loud"]).is_err());
    }

    #[test]
    fn strict_is_the_older_spelling_of_validation_level_strict() {
        let args = render_args(&["in.mjml", "--strict"]);
        assert!(args.strict);
        // The flag is applied on top of the parsed options by `run_render`.
        assert_eq!(
            args.options.to_render_options().unwrap().validation_level,
            ValidationLevel::Soft
        );
    }

    #[test]
    fn strict_no_longer_requires_validate() {
        assert!(Cli::try_parse_from(["mjmx", "in.mjml", "--strict"]).is_ok());
    }

    #[test]
    fn strict_conflicts_with_validation_level() {
        assert!(
            Cli::try_parse_from(["mjmx", "in.mjml", "--strict", "--validation-level", "skip"])
                .is_err()
        );
    }

    #[test]
    fn validate_is_reporting_only_and_composes_with_any_level() {
        let args = render_args(&["in.mjml", "--validate", "--validation-level", "skip"]);
        assert!(args.validate);
        assert_eq!(
            args.options.to_render_options().unwrap().validation_level,
            ValidationLevel::Skip
        );
    }

    #[test]
    fn file_path_is_taken_as_the_include_base() {
        let options = render_options(&["in.mjml", "--file-path", "partials/"]);
        assert_eq!(options.file_path, Some(PathBuf::from("partials/")));
    }

    #[test]
    fn fonts_accept_a_json_object_in_source_order() {
        let options = render_options(&[
            "in.mjml",
            "--fonts",
            r#"{"Zeta": "https://z.example/z.css", "Alpha": "https://a.example/a.css"}"#,
        ]);
        let fonts = options.fonts.expect("a table replaces the built-ins");
        assert_eq!(
            fonts,
            vec![
                FontFace::new("Zeta", "https://z.example/z.css"),
                FontFace::new("Alpha", "https://a.example/a.css"),
            ]
        );
    }

    #[test]
    fn fonts_accept_repeated_name_equals_url_pairs() {
        let options = render_options(&[
            "in.mjml",
            "--fonts",
            "Inter=https://i.example/i.css",
            "--fonts",
            "Mono=https://m.example/m.css",
        ]);
        assert_eq!(
            options.fonts.unwrap(),
            vec![
                FontFace::new("Inter", "https://i.example/i.css"),
                FontFace::new("Mono", "https://m.example/m.css"),
            ]
        );
    }

    #[test]
    fn a_repeated_font_name_updates_in_place() {
        let options = render_options(&[
            "in.mjml",
            "--fonts",
            "Inter=https://old.example/i.css",
            "--fonts",
            "Inter=https://new.example/i.css",
        ]);
        assert_eq!(
            options.fonts.unwrap(),
            vec![FontFace::new("Inter", "https://new.example/i.css")]
        );
    }

    #[test]
    fn malformed_fonts_specs_are_rejected() {
        for spec in ["Inter", "{not json}", "=https://i.example/i.css", "Inter="] {
            let args = render_args(&["in.mjml", "--fonts", spec]);
            assert!(
                args.options.to_render_options().is_err(),
                "expected '{spec}' to be rejected"
            );
        }
    }

    #[test]
    fn the_dev_subcommand_takes_the_same_render_options() {
        let cli = Cli::try_parse_from([
            "mjmx",
            "dev",
            "in.mjml",
            "--breakpoint",
            "600",
            "--no-keep-comments",
            "--validation-level",
            "skip",
            "--fonts",
            "Inter=https://i.example/i.css",
        ])
        .expect("arguments should parse");
        let Some(Command::Dev(args)) = cli.command else {
            panic!("expected the dev subcommand");
        };
        let options = args.options.to_render_options().unwrap();
        assert_eq!(options.breakpoint, 600);
        assert!(!options.keep_comments);
        assert_eq!(options.validation_level, ValidationLevel::Skip);
        assert_eq!(
            options.fonts.unwrap(),
            vec![FontFace::new("Inter", "https://i.example/i.css")]
        );
    }

    #[test]
    fn fmt_and_migrate_expose_indent_width() {
        let cli = Cli::try_parse_from(["mjmx", "fmt", "in.mjml", "--indent-width", "4"]).unwrap();
        let Some(Command::Fmt(args)) = cli.command else {
            panic!("expected the fmt subcommand");
        };
        assert_eq!(args.indent_width, 4);

        let cli = Cli::try_parse_from(["mjmx", "migrate", "in.mjml"]).unwrap();
        let Some(Command::Migrate(args)) = cli.command else {
            panic!("expected the migrate subcommand");
        };
        assert_eq!(args.indent_width, FormatOptions::default().indent_width);
    }
}
