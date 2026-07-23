//! Live performance-table generator for the mjmx README.
//!
//! Times the two Rust engines (mjmx at its V5 default, and mrml) in-process over the
//! repository's `fixtures/*.mjml`, then shells out to `bench.mjs` for the two Node
//! engines (mjml 4.18.0 and 5.4.0). Every number is measured here and now — nothing is
//! hard-coded — and the markdown table is printed (or, with `--write`, spliced into the
//! README between the `<!-- BENCH:START -->` / `<!-- BENCH:END -->` markers).
//!
//! Methodology (matching the README's caveat): warm in-process render, times exclude
//! process startup. The Rust engines are timed on the fixtures **both** render (mrml
//! fails a few), so they are compared on the same set; the two mjml lines render all
//! fixtures. Single machine, single run — directional, not paper-grade.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Default timed iterations per fixture. Override with `--iters N`.
const DEFAULT_ITERS: u32 = 200;

/// One row of the output table.
struct Row {
    engine: String,
    version: String,
    language: String,
    us_per_render: f64,
}

fn main() {
    let args: Args = Args::parse(std::env::args().skip(1));
    let root = repo_root();
    let fixtures = load_fixtures(&root.join("fixtures"));
    eprintln!("loaded {} fixtures", fixtures.len());

    // Which fixtures each Rust engine can render, so both are timed on the intersection.
    let mjmx_ok: Vec<&String> = fixtures.iter().filter(|s| renders_mjmx(s)).collect();
    let mrml_ok: Vec<&String> = fixtures.iter().filter(|s| renders_mrml(s)).collect();
    let both: Vec<&String> = fixtures
        .iter()
        .filter(|s| renders_mjmx(s) && renders_mrml(s))
        .collect();
    eprintln!(
        "renderable: mjmx {}/{}, mrml {}/{}, both {}",
        mjmx_ok.len(),
        fixtures.len(),
        mrml_ok.len(),
        fixtures.len(),
        both.len()
    );

    let mjmx_us = time(&both, |s| {
        let _ = mjmx::render_default(s);
    }, args.iters);
    let mrml_us = time(&both, |s| {
        if let Ok(root) = mrml::parse(s) {
            let _ = root.element.render(&mrml::prelude::render::RenderOptions::default());
        }
    }, args.iters);

    let mut rows = vec![
        Row {
            engine: "**mjmx**".into(),
            version: "5.4.0 / 4.18.0".into(),
            language: "Rust".into(),
            us_per_render: mjmx_us,
        },
        Row {
            engine: "[mrml](https://github.com/jdrouet/mrml)".into(),
            version: "6.0.1".into(),
            language: "Rust".into(),
            us_per_render: mrml_us,
        },
    ];

    // Node engines: shell out to bench.mjs, which prints one JSON object per engine.
    match run_node_bench(&root.join("benches/perf"), args.iters) {
        Ok(node_rows) => rows.extend(node_rows),
        Err(e) => eprintln!(
            "warning: skipping mjml (Node) rows — {e}\n\
             run `npm install` in benches/perf and ensure `node` is on PATH"
        ),
    }

    let table = render_table(&rows, mjmx_ok.len(), fixtures.len(), both.len());
    if args.write {
        write_readme(&root.join("README.md"), &table);
        eprintln!("README.md updated between the BENCH markers");
    } else {
        println!("{table}");
    }
}

/// Time `run` over `srcs`, `iters` passes each, after one warmup pass. Returns µs/render
/// (total elapsed / total renders).
fn time(srcs: &[&String], run: impl Fn(&str), iters: u32) -> f64 {
    for s in srcs {
        run(s);
    }
    let start = Instant::now();
    for _ in 0..iters {
        for s in srcs {
            run(s);
        }
    }
    let elapsed = start.elapsed();
    let renders = srcs.len() as u128 * u128::from(iters);
    elapsed.as_nanos() as f64 / renders as f64 / 1000.0
}

fn renders_mjmx(src: &str) -> bool {
    mjmx::render_default(src).is_ok()
}

fn renders_mrml(src: &str) -> bool {
    mrml::parse(src).is_ok_and(|root| {
        root.element
            .render(&mrml::prelude::render::RenderOptions::default())
            .is_ok()
    })
}

/// Every `*.mjml` directly under `dir`, sorted for a stable run.
fn load_fixtures(dir: &Path) -> Vec<String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read fixtures dir {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "mjml"))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect()
}

/// Run `bench.mjs` and parse its newline-delimited JSON rows.
fn run_node_bench(dir: &Path, iters: u32) -> Result<Vec<Row>, String> {
    let out = Command::new("node")
        .arg("bench.mjs")
        .arg(iters.to_string())
        .current_dir(dir)
        .output()
        .map_err(|e| format!("could not launch node: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "bench.mjs exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut rows = Vec::new();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("bad JSON from bench.mjs: {e} in {line}"))?;
        rows.push(Row {
            engine: "[mjml](https://github.com/mjmlio/mjml)".into(),
            version: v["version"].as_str().unwrap_or("?").to_string(),
            language: "JS (Node)".into(),
            us_per_render: v["us_per_render"].as_f64().unwrap_or(0.0),
        });
    }
    Ok(rows)
}

/// The markdown table plus its caveat paragraph, ready to splice into the README.
fn render_table(rows: &[Row], mjmx_total: usize, all: usize, both: usize) -> String {
    let baseline = rows.first().map_or(1.0, |r| r.us_per_render);
    let mut out = String::new();
    out.push_str("| Engine | Version | Language | µs / render | Renders / sec | Relative |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for r in rows {
        let rps = if r.us_per_render > 0.0 {
            (1_000_000.0 / r.us_per_render).round() as u64
        } else {
            0
        };
        let rel = baseline / r.us_per_render;
        // Bold the mjmx cells to match the README's styling.
        let (us, rps_s) = if r.engine.starts_with("**") {
            (format!("**{:.1}**", r.us_per_render), format!("**{}**", group_thousands(rps)))
        } else {
            (format!("{:.1}", r.us_per_render), group_thousands(rps))
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.3}× |\n",
            r.engine, r.version, r.language, us, rps_s, rel
        ));
    }
    // Caveat, computed from the actual run.
    let mjml_slow = rows
        .iter()
        .filter(|r| r.language.starts_with("JS"))
        .map(|r| baseline / r.us_per_render)
        .fold(f64::INFINITY, f64::min);
    let mrml_rel = rows
        .iter()
        .find(|r| r.engine.contains("mrml"))
        .map(|r| r.us_per_render / baseline);
    out.push('\n');
    if mjml_slow.is_finite() {
        out.push_str(&format!(
            "mjmx renders **~{}× faster than mjml 5.4.0**",
            (1.0 / mjml_slow).round() as u64
        ));
        if let Some(m) = mrml_rel {
            out.push_str(&format!(" and **~{m:.1}× faster than mrml**"));
        }
        out.push_str(". Rust engines\n");
    } else {
        out.push_str("Rust engines\n");
    }
    out.push_str(&format!(
        "were timed on the {both} fixtures both render (mrml fails {}); both mjml lines render \
         all {all}.\nmjml 5.x is measured through its `Promise`-returning API — 5.0 made \
         `mjml2html` async.\nSingle machine, single run — directional, not paper-grade.",
        mjmx_total - both
    ));
    out
}

/// `73738` → `73,738`.
fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Splice `table` into the README between the BENCH markers, preserving the rest.
fn write_readme(path: &Path, table: &str) {
    const START: &str = "<!-- BENCH:START -->";
    const END: &str = "<!-- BENCH:END -->";
    let text = std::fs::read_to_string(path).expect("read README.md");
    let (Some(s), Some(e)) = (text.find(START), text.find(END)) else {
        panic!("README.md is missing the `{START}` / `{END}` markers — add them around the table");
    };
    let new = format!(
        "{}{START}\n{table}\n{END}{}",
        &text[..s],
        &text[e + END.len()..]
    );
    std::fs::write(path, new).expect("write README.md");
}

/// The repository root: two directories up from this crate's manifest.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve repo root")
}

/// Parsed CLI arguments.
struct Args {
    write: bool,
    iters: u32,
}

impl Args {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut write = false;
        let mut iters = DEFAULT_ITERS;
        let mut it = args.peekable();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--write" => write = true,
                "--iters" => {
                    iters = it
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(DEFAULT_ITERS);
                }
                other => eprintln!("warning: ignoring unknown argument `{other}`"),
            }
        }
        Args { write, iters }
    }
}
