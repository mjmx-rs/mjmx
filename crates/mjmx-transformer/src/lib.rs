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
//! Loading is delegated to an [`IncludeLoader`]. [`LocalLoader`] (filesystem) is
//! always available; [`HttpLoader`] and [`CombinedLoader`] exist only behind the
//! non-default `http-loader` feature — see the [`http`] module for the security
//! rationale. The pass **never panics**: missing files, read errors, and include
//! cycles are reported as [`Diagnostic`]s and the offending include is skipped.
//!
//! The second pass is [`migrate`], the MJML v3 → v4 source migration.

#[cfg(feature = "http-loader")]
pub mod http;
mod loader;
mod migrate;

use std::path::Path;

use mjmx_allocator::Vec as ArenaVec;
use mjmx_ast::{AstBuilder, Document, Element, Node};
use mjmx_diagnostics::Diagnostic;
use mjmx_span::Span;
use mjmx_syntax::MjmlTag;
use mjmx_traverse::{Visit, walk_element};

#[cfg(feature = "http-loader")]
pub use http::{CombinedLoader, HttpFetch, HttpLoader, UreqFetch, is_http_url};
pub use loader::{IncludeBase, IncludeLoader, Loaded, LocalLoader};
pub use migrate::migrate;

/// Resolve every `<mj-include>` in `document`, returning a rebuilt document and any
/// diagnostics gathered along the way.
///
/// `root_dir` is the directory the top-level document lives in; the first level of
/// includes is resolved relative to it, and nested includes relative to their own
/// parent file. If the document contains no includes it is rebuilt shallowly and no
/// I/O happens. This function never panics.
///
/// Use [`resolve_includes_from`] when the document's base is not a local directory.
#[must_use]
pub fn resolve_includes<'a>(
    builder: &AstBuilder<'a>,
    document: &Document<'a>,
    loader: &dyn IncludeLoader,
    root_dir: &Path,
) -> (Document<'a>, Vec<Diagnostic>) {
    resolve_includes_from(builder, document, loader, &IncludeBase::dir(root_dir))
}

/// Like [`resolve_includes`], but the top-level document's base is given explicitly.
///
/// Pass an [`IncludeBase::Url`] when the document itself came off the network, so
/// its relative includes resolve against that URL instead of the filesystem.
#[must_use]
pub fn resolve_includes_from<'a>(
    builder: &AstBuilder<'a>,
    document: &Document<'a>,
    loader: &dyn IncludeLoader,
    root: &IncludeBase,
) -> (Document<'a>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();

    // Fast path: nothing to expand, so avoid a deep rebuild and all I/O.
    if !contains_include(document) {
        let children = builder.vec_from_iter(document.children.iter().copied());
        return (builder.document(children, document.span), diagnostics);
    }

    let mut stack: Vec<String> = Vec::new();
    let children = rebuild_nodes(
        builder,
        &document.children,
        loader,
        root,
        &mut stack,
        &mut diagnostics,
    );
    (builder.document(children, document.span), diagnostics)
}

/// Drop every `<mj-include>` from `document` instead of resolving it — mjml's
/// `ignoreIncludes`.
///
/// This is *removal*, not a no-op: upstream's parser returns before building the node
/// (`mjml-parser-xml/src/index.js:238`), so an ignored include leaves nothing behind —
/// neither the tag nor the content it would have pulled in. No I/O happens, and a
/// document without includes is rebuilt shallowly.
#[must_use]
pub fn strip_includes<'a>(builder: &AstBuilder<'a>, document: &Document<'a>) -> Document<'a> {
    if !contains_include(document) {
        let children = builder.vec_from_iter(document.children.iter().copied());
        return builder.document(children, document.span);
    }
    let children = strip_nodes(builder, &document.children);
    builder.document(children, document.span)
}

/// Rebuild a node list with every `<mj-include>` removed, recursing into elements.
fn strip_nodes<'a>(builder: &AstBuilder<'a>, nodes: &[Node<'a>]) -> ArenaVec<'a, Node<'a>> {
    let mut out = builder.vec();
    for node in nodes {
        match node {
            Node::Element(el) if el.tag == MjmlTag::MjInclude => {}
            Node::Element(el) => {
                let attributes = builder.vec_from_iter(el.attributes.iter().copied());
                let children = strip_nodes(builder, &el.children);
                let rebuilt = builder.element(el.tag, el.raw_tag, attributes, children, el.span);
                out.push(builder.element_node(rebuilt));
            }
            other => out.push(*other),
        }
    }
    out
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
    from: &IncludeBase,
    stack: &mut Vec<String>,
    diags: &mut Vec<Diagnostic>,
) -> ArenaVec<'a, Node<'a>> {
    let mut out = builder.vec();
    for node in nodes {
        match node {
            Node::Element(el) if el.tag == MjmlTag::MjInclude => {
                resolve_include(builder, el, loader, from, stack, &mut out, diags);
            }
            Node::Element(el) => {
                let rebuilt = rebuild_element(builder, el, loader, from, stack, diags);
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
    from: &IncludeBase,
    stack: &mut Vec<String>,
    diags: &mut Vec<Diagnostic>,
) -> &'a Element<'a> {
    let attributes = builder.vec_from_iter(el.attributes.iter().copied());
    let children = rebuild_nodes(builder, &el.children, loader, from, stack, diags);
    builder.element(el.tag, el.raw_tag, attributes, children, el.span)
}

/// Expand one `<mj-include>` element, pushing the replacement node(s) onto `out`.
fn resolve_include<'a>(
    builder: &AstBuilder<'a>,
    el: &Element<'a>,
    loader: &dyn IncludeLoader,
    from: &IncludeBase,
    stack: &mut Vec<String>,
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
        "html" => match loader.load(path, from) {
            Ok(loaded) => out.push(builder.element_node(wrap_raw_text(
                builder,
                MjmlTag::MjRaw,
                "mj-raw",
                &loaded.contents,
                el.span,
            ))),
            Err(diag) => diags.push(annotate(diag, el.span, path)),
        },
        // CSS: wrap in <mj-style> so it lands in the head <style> block.
        "css" => match loader.load(path, from) {
            Ok(loaded) => out.push(builder.element_node(wrap_raw_text(
                builder,
                MjmlTag::MjStyle,
                "mj-style",
                &loaded.contents,
                el.span,
            ))),
            Err(diag) => diags.push(annotate(diag, el.span, path)),
        },
        // Default: an MJML fragment, parsed and spliced (with cycle detection).
        _ => resolve_mjml_include(builder, el, path, loader, from, stack, out, diags),
    }
}

/// Expand a `type="mjml"` include: load, parse as a fragment, recurse, splice.
#[allow(clippy::too_many_arguments)]
fn resolve_mjml_include<'a>(
    builder: &AstBuilder<'a>,
    el: &Element<'a>,
    path: &str,
    loader: &dyn IncludeLoader,
    from: &IncludeBase,
    stack: &mut Vec<String>,
    out: &mut ArenaVec<'a, Node<'a>>,
    diags: &mut Vec<Diagnostic>,
) {
    // The loader owns resolution, so it also owns the cycle identity: only it knows
    // how to canonicalize (a real path via the filesystem, a normalized absolute URL
    // over HTTP). That means the load happens first and the identity is checked
    // after — one redundant fetch/read closes a cycle, and recursion still stops
    // because the identity is already on the stack.
    let loaded = match loader.load(path, from) {
        Ok(loaded) => loaded,
        Err(diag) => {
            diags.push(annotate(diag, el.span, path));
            return;
        }
    };

    if stack.contains(&loaded.id) {
        diags.push(
            Diagnostic::error(format!("include cycle: {}", cycle_chain(stack, &loaded.id)))
                .with_label(
                    el.span,
                    "this <mj-include> re-enters a file already being resolved",
                )
                .with_help("break the cycle so the include graph is acyclic"),
        );
        return;
    }

    // Copy the loaded source into the arena so the parsed nodes can share lifetime 'a.
    let source = builder.atom(&loaded.contents).as_str();
    let parsed = mjmx_parser::parse(source, builder.allocator());
    // Surface malformed-partial diagnostics (spans are relative to the partial).
    diags.extend(parsed.errors);

    stack.push(loaded.id);
    let children = rebuild_nodes(
        builder,
        &parsed.document.children,
        loader,
        &loaded.base,
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

/// Render the current cycle as `a -> b -> a` using short names for readability.
fn cycle_chain(stack: &[String], repeated: &str) -> String {
    let mut parts: Vec<&str> = stack.iter().map(|id| display_name(id)).collect();
    parts.push(display_name(repeated));
    parts.join(" -> ")
}

/// A short label for an include identity: the last path/URL segment, else the whole
/// thing. Works for both local paths and URLs since both are `/`-separated.
fn display_name(id: &str) -> &str {
    id.rsplit('/').find(|s| !s.is_empty()).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_allocator::Allocator;
    use mjmx_diagnostics::Severity;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// An in-memory loader for tests: maps an absolute path key to file contents.
    ///
    /// Paths are treated lexically (no real filesystem), and each file's directory is
    /// its parent, so relative nested includes resolve the same way the real loader
    /// would. Keys are the lexically-joined paths the transformer produces.
    struct MemLoader {
        files: HashMap<PathBuf, String>,
    }

    impl IncludeLoader for MemLoader {
        fn load(&self, path: &str, from: &IncludeBase) -> Result<Loaded, Diagnostic> {
            let from_dir = from
                .as_dir()
                .ok_or_else(|| Diagnostic::error("MemLoader needs a directory base"))?;
            let full = normalize(&from_dir.join(path));
            match self.files.get(&full) {
                Some(contents) => {
                    let dir = full.parent().unwrap_or(from_dir).to_path_buf();
                    Ok(Loaded {
                        contents: contents.clone(),
                        base: IncludeBase::Dir(dir),
                        id: full.display().to_string(),
                    })
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
