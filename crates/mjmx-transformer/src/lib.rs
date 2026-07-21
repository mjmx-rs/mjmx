//! MJML-to-MJML transform passes for `mjmx`.
//!
//! The AST is immutable arena references, so a "transform" is a **rebuild**: the pass
//! walks the source tree and constructs a new one through
//! [`AstBuilder`](mjmx_ast::AstBuilder), splicing in replacement nodes where needed.
//! The new tree shares the same arena (lifetime `'a`) as the input, so loaded content
//! parsed into that arena can be spliced directly.
//!
//! The only pass today is [`resolve_includes`], which expands `<mj-include>`:
//!
//! - **`type="mjml"`** (default) — load the file, parse it as a *fragment* (partials
//!   usually have no `<mjml>` root), recursively resolve includes inside it (tracked
//!   against that file's own directory), and splice its nodes in place of the
//!   `<mj-include>`.
//! - **`type="html"`** — wrap the loaded text in an `<mj-raw>` so it is emitted
//!   verbatim (mjml's passthrough behavior).
//! - **`type="css"`** — wrap the loaded text in an `<mj-style>` so it lands in the
//!   head `<style>` block (mjml's behavior).
//!
//! Loading is delegated to an [`IncludeLoader`]; the filesystem [`LocalLoader`] is the
//! only implementation for now. The pass **never panics**: missing files, read
//! errors, and include cycles are reported as [`Diagnostic`]s and the offending
//! include is skipped.

mod loader;

use std::path::{Path, PathBuf};

use mjmx_allocator::Vec as ArenaVec;
use mjmx_ast::{AstBuilder, Document, Element, Node};
use mjmx_diagnostics::Diagnostic;
use mjmx_span::Span;
use mjmx_syntax::MjmlTag;
use mjmx_traverse::{Visit, walk_element};

pub use loader::{IncludeLoader, LocalLoader};

/// Resolve every `<mj-include>` in `document`, returning a rebuilt document and any
/// diagnostics gathered along the way.
///
/// `root_dir` is the directory the top-level document lives in; the first level of
/// includes is resolved relative to it, and nested includes relative to their own
/// parent file. If the document contains no includes it is rebuilt shallowly and no
/// I/O happens. This function never panics.
#[must_use]
pub fn resolve_includes<'a>(
    builder: &AstBuilder<'a>,
    document: &Document<'a>,
    loader: &dyn IncludeLoader,
    root_dir: &Path,
) -> (Document<'a>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();

    // Fast path: nothing to expand, so avoid a deep rebuild and all filesystem work.
    if !contains_include(document) {
        let children = builder.vec_from_iter(document.children.iter().copied());
        return (builder.document(children, document.span), diagnostics);
    }

    let mut stack: Vec<PathBuf> = Vec::new();
    let children = rebuild_nodes(
        builder,
        &document.children,
        loader,
        root_dir,
        &mut stack,
        &mut diagnostics,
    );
    (builder.document(children, document.span), diagnostics)
}

/// Whether the document contains any `<mj-include>` element (at any depth).
fn contains_include(document: &Document<'_>) -> bool {
    struct Finder {
        found: bool,
    }
    impl<'a> Visit<'a> for Finder {
        fn visit_element(&mut self, element: &Element<'a>) {
            if element.tag == MjmlTag::MjInclude {
                self.found = true;
                return; // no need to descend further
            }
            walk_element(self, element);
        }
    }
    let mut finder = Finder { found: false };
    finder.visit_document(document);
    finder.found
}

/// Rebuild a node list, expanding any `<mj-include>` in place.
fn rebuild_nodes<'a>(
    builder: &AstBuilder<'a>,
    nodes: &[Node<'a>],
    loader: &dyn IncludeLoader,
    from_dir: &Path,
    stack: &mut Vec<PathBuf>,
    diags: &mut Vec<Diagnostic>,
) -> ArenaVec<'a, Node<'a>> {
    let mut out = builder.vec();
    for node in nodes {
        match node {
            Node::Element(el) if el.tag == MjmlTag::MjInclude => {
                resolve_include(builder, el, loader, from_dir, stack, &mut out, diags);
            }
            Node::Element(el) => {
                let rebuilt = rebuild_element(builder, el, loader, from_dir, stack, diags);
                out.push(builder.element_node(rebuilt));
            }
            // Text and comment nodes are `Copy` leaves; carry them over unchanged.
            other => out.push(*other),
        }
    }
    out
}

/// Rebuild a single element, recursing into its children (attributes are `Copy`).
fn rebuild_element<'a>(
    builder: &AstBuilder<'a>,
    el: &Element<'a>,
    loader: &dyn IncludeLoader,
    from_dir: &Path,
    stack: &mut Vec<PathBuf>,
    diags: &mut Vec<Diagnostic>,
) -> &'a Element<'a> {
    let attributes = builder.vec_from_iter(el.attributes.iter().copied());
    let children = rebuild_nodes(builder, &el.children, loader, from_dir, stack, diags);
    builder.element(el.tag, el.raw_tag, attributes, children, el.span)
}

/// Expand one `<mj-include>` element, pushing the replacement node(s) onto `out`.
fn resolve_include<'a>(
    builder: &AstBuilder<'a>,
    el: &Element<'a>,
    loader: &dyn IncludeLoader,
    from_dir: &Path,
    stack: &mut Vec<PathBuf>,
    out: &mut ArenaVec<'a, Node<'a>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(path) = el.attr("path") else {
        diags.push(
            Diagnostic::error("<mj-include> is missing a `path` attribute").with_label(
                el.span,
                "add a path=\"...\" pointing at the file to include",
            ),
        );
        return;
    };
    let include_type = el.attr("type").unwrap_or("mjml");

    match include_type {
        // Raw HTML passthrough: wrap in <mj-raw> so codegen emits it verbatim.
        "html" => match loader.load(path, from_dir) {
            Ok((contents, _)) => out.push(builder.element_node(wrap_raw_text(
                builder,
                MjmlTag::MjRaw,
                "mj-raw",
                &contents,
                el.span,
            ))),
            Err(diag) => diags.push(annotate(diag, el.span, path)),
        },
        // CSS: wrap in <mj-style> so it lands in the head <style> block.
        "css" => match loader.load(path, from_dir) {
            Ok((contents, _)) => out.push(builder.element_node(wrap_raw_text(
                builder,
                MjmlTag::MjStyle,
                "mj-style",
                &contents,
                el.span,
            ))),
            Err(diag) => diags.push(annotate(diag, el.span, path)),
        },
        // Default: an MJML fragment, parsed and spliced (with cycle detection).
        _ => resolve_mjml_include(builder, el, path, loader, from_dir, stack, out, diags),
    }
}

/// Expand a `type="mjml"` include: load, parse as a fragment, recurse, splice.
#[allow(clippy::too_many_arguments)]
fn resolve_mjml_include<'a>(
    builder: &AstBuilder<'a>,
    el: &Element<'a>,
    path: &str,
    loader: &dyn IncludeLoader,
    from_dir: &Path,
    stack: &mut Vec<PathBuf>,
    out: &mut ArenaVec<'a, Node<'a>>,
    diags: &mut Vec<Diagnostic>,
) {
    // Cycle key: the resolved absolute path of the target file. Canonicalize when
    // possible so `./a.mjml` and `a.mjml` (or `../x/../a.mjml`) map to one key.
    let candidate = from_dir.join(path);
    let key = candidate.canonicalize().unwrap_or(candidate);

    if stack.contains(&key) {
        diags.push(
            Diagnostic::error(format!("include cycle: {}", cycle_chain(stack, &key)))
                .with_label(
                    el.span,
                    "this <mj-include> re-enters a file already being resolved",
                )
                .with_help("break the cycle so the include graph is acyclic"),
        );
        return;
    }

    let (contents, file_dir) = match loader.load(path, from_dir) {
        Ok(loaded) => loaded,
        Err(diag) => {
            diags.push(annotate(diag, el.span, path));
            return;
        }
    };

    // Copy the loaded source into the arena so the parsed nodes can share lifetime 'a.
    let source = builder.atom(&contents).as_str();
    let parsed = mjmx_parser::parse(source, builder.allocator());
    // Surface malformed-partial diagnostics (spans are relative to the partial).
    diags.extend(parsed.errors);

    stack.push(key);
    let children = rebuild_nodes(
        builder,
        &parsed.document.children,
        loader,
        &file_dir,
        stack,
        diags,
    );
    stack.pop();

    out.extend(children.iter().copied());
}

/// Build an element of `tag` whose single child is `content` as a verbatim text node.
fn wrap_raw_text<'a>(
    builder: &AstBuilder<'a>,
    tag: MjmlTag,
    raw_tag: &str,
    content: &str,
    span: Span,
) -> &'a Element<'a> {
    let text = builder.text(builder.atom(content), span);
    let children = builder.vec_from_iter([builder.text_node(text)]);
    builder.element(tag, builder.atom(raw_tag), builder.vec(), children, span)
}

/// Attach the offending include's span to a loader diagnostic.
fn annotate(diag: Diagnostic, span: Span, path: &str) -> Diagnostic {
    diag.with_label(
        span,
        format!("<mj-include path=\"{path}\"> could not be resolved"),
    )
}

/// Render the current cycle as `a -> b -> a` using file names for readability.
fn cycle_chain(stack: &[PathBuf], repeated: &Path) -> String {
    let mut parts: Vec<String> = stack.iter().map(|p| display_name(p)).collect();
    parts.push(display_name(repeated));
    parts.join(" -> ")
}

/// A short display label for a path: its file name if present, else the full path.
fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_allocator::Allocator;
    use mjmx_diagnostics::Severity;
    use std::collections::HashMap;

    /// An in-memory loader for tests: maps an absolute path key to file contents.
    ///
    /// Paths are treated lexically (no real filesystem), and each file's directory is
    /// its parent, so relative nested includes resolve the same way the real loader
    /// would. Keys are the lexically-joined paths the transformer produces.
    struct MemLoader {
        files: HashMap<PathBuf, String>,
    }

    impl IncludeLoader for MemLoader {
        fn load(&self, path: &str, from_dir: &Path) -> Result<(String, PathBuf), Diagnostic> {
            let full = normalize(&from_dir.join(path));
            match self.files.get(&full) {
                Some(contents) => {
                    let dir = full.parent().unwrap_or(from_dir).to_path_buf();
                    Ok((contents.clone(), dir))
                }
                None => Err(Diagnostic::error(format!(
                    "no such file: {}",
                    full.display()
                ))),
            }
        }
    }

    /// Lexically normalize `.`/`..` segments so join keys match (no I/O).
    fn normalize(p: &Path) -> PathBuf {
        let mut out = PathBuf::new();
        for comp in p.components() {
            use std::path::Component;
            match comp {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                other => out.push(other.as_os_str()),
            }
        }
        out
    }

    fn count_tag(nodes: &[Node<'_>], tag: MjmlTag) -> usize {
        nodes
            .iter()
            .filter_map(Node::as_element)
            .map(|el| {
                (if el.tag == tag { 1 } else { 0 })
                    + count_tag(&el.children.iter().copied().collect::<Vec<_>>(), tag)
            })
            .sum()
    }

    #[test]
    fn nested_include_inlines() {
        let alloc = Allocator::new();
        let builder = AstBuilder::new(&alloc);
        let root = PathBuf::from("/proj");
        let files = HashMap::from([
            (
                PathBuf::from("/proj/inner.mjml"),
                "<mj-text>deep</mj-text>".to_string(),
            ),
            (
                PathBuf::from("/proj/outer.mjml"),
                "<mj-section><mj-column><mj-include path=\"./inner.mjml\" /></mj-column></mj-section>"
                    .to_string(),
            ),
        ]);
        let loader = MemLoader { files };

        let ret = mjmx_parser::parse(
            "<mjml><mj-body><mj-include path=\"./outer.mjml\" /></mj-body></mjml>",
            &alloc,
        );
        let (doc, diags) = resolve_includes(&builder, &ret.document, &loader, &root);

        assert!(diags.is_empty(), "expected no diagnostics, got {diags:?}");
        // The mj-include nodes are gone; the section + inner text are spliced in.
        assert_eq!(count_tag(&doc.children, MjmlTag::MjInclude), 0);
        assert_eq!(count_tag(&doc.children, MjmlTag::MjSection), 1);
        assert_eq!(count_tag(&doc.children, MjmlTag::MjText), 1);
    }

    #[test]
    fn cycle_is_reported_without_hanging() {
        let alloc = Allocator::new();
        let builder = AstBuilder::new(&alloc);
        let root = PathBuf::from("/proj");
        let files = HashMap::from([
            (
                PathBuf::from("/proj/a.mjml"),
                "<mj-include path=\"./b.mjml\" />".to_string(),
            ),
            (
                PathBuf::from("/proj/b.mjml"),
                "<mj-include path=\"./a.mjml\" />".to_string(),
            ),
        ]);
        let loader = MemLoader { files };

        let ret = mjmx_parser::parse("<mj-include path=\"./a.mjml\" />", &alloc);
        let (_doc, diags) = resolve_includes(&builder, &ret.document, &loader, &root);

        assert!(
            diags
                .iter()
                .any(|d| d.severity == Severity::Error && d.message.contains("cycle")),
            "expected a cycle diagnostic, got {diags:?}"
        );
    }

    #[test]
    fn missing_file_is_reported() {
        let alloc = Allocator::new();
        let builder = AstBuilder::new(&alloc);
        let root = PathBuf::from("/proj");
        let loader = MemLoader {
            files: HashMap::new(),
        };

        let ret = mjmx_parser::parse(
            "<mjml><mj-body><mj-include path=\"./nope.mjml\" /></mj-body></mjml>",
            &alloc,
        );
        let (doc, diags) = resolve_includes(&builder, &ret.document, &loader, &root);

        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        // The unresolved include is dropped, not left in the tree.
        assert_eq!(count_tag(&doc.children, MjmlTag::MjInclude), 0);
    }

    #[test]
    fn html_include_becomes_raw() {
        let alloc = Allocator::new();
        let builder = AstBuilder::new(&alloc);
        let root = PathBuf::from("/proj");
        let files = HashMap::from([(PathBuf::from("/proj/snippet.html"), "<p>hi</p>".to_string())]);
        let loader = MemLoader { files };

        let ret = mjmx_parser::parse(
            "<mjml><mj-body><mj-section><mj-column>\
             <mj-include path=\"./snippet.html\" type=\"html\" /></mj-column></mj-section></mj-body></mjml>",
            &alloc,
        );
        let (doc, diags) = resolve_includes(&builder, &ret.document, &loader, &root);

        assert!(diags.is_empty(), "got {diags:?}");
        assert_eq!(count_tag(&doc.children, MjmlTag::MjRaw), 1);
    }

    #[test]
    fn css_include_becomes_style() {
        let alloc = Allocator::new();
        let builder = AstBuilder::new(&alloc);
        let root = PathBuf::from("/proj");
        let files = HashMap::from([(
            PathBuf::from("/proj/theme.css"),
            ".a{color:red}".to_string(),
        )]);
        let loader = MemLoader { files };

        let ret = mjmx_parser::parse(
            "<mjml><mj-head><mj-include path=\"./theme.css\" type=\"css\" /></mj-head><mj-body></mj-body></mjml>",
            &alloc,
        );
        let (doc, diags) = resolve_includes(&builder, &ret.document, &loader, &root);

        assert!(diags.is_empty(), "got {diags:?}");
        assert_eq!(count_tag(&doc.children, MjmlTag::MjStyle), 1);
    }

    #[test]
    fn no_include_is_noop_passthrough() {
        let alloc = Allocator::new();
        let builder = AstBuilder::new(&alloc);
        let root = PathBuf::from("/proj");
        let loader = LocalLoader;

        let ret = mjmx_parser::parse(
            "<mjml><mj-body><mj-section><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>",
            &alloc,
        );
        let (doc, diags) = resolve_includes(&builder, &ret.document, &loader, &root);
        assert!(diags.is_empty());
        assert_eq!(count_tag(&doc.children, MjmlTag::MjText), 1);
    }
}
