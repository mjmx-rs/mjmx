//! Integration tests for the `http-loader` feature.
//!
//! **No test here touches the public internet.** Resolution, splicing, and cycle
//! detection are exercised through a stubbed [`HttpFetch`] transport, and the one
//! test that must prove the real `ureq` transport works binds a throwaway
//! `TcpListener` on `127.0.0.1:0` and serves a single canned response.
#![cfg(feature = "http-loader")]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;

use mjmx_allocator::Allocator;
use mjmx_ast::{AstBuilder, Document, Node};
use mjmx_diagnostics::{Diagnostic, Severity};
use mjmx_syntax::MjmlTag;
use mjmx_transformer::{
    CombinedLoader, HttpFetch, HttpLoader, IncludeBase, resolve_includes, resolve_includes_from,
};

/// A canned-response transport: URL → body, with a log of what was requested.
struct StubFetch {
    files: HashMap<String, String>,
    requested: Mutex<Vec<String>>,
}

impl StubFetch {
    fn new(files: &[(&str, &str)]) -> Self {
        Self {
            files: files
                .iter()
                .map(|(url, body)| ((*url).to_string(), (*body).to_string()))
                .collect(),
            requested: Mutex::new(Vec::new()),
        }
    }
}

impl HttpFetch for StubFetch {
    fn get(&self, url: &str) -> Result<String, String> {
        self.requested.lock().unwrap().push(url.to_string());
        self.files
            .get(url)
            .cloned()
            .ok_or_else(|| format!("404 for {url}"))
    }
}

/// Count elements with `tag` anywhere in the tree.
fn count_tag(document: &Document<'_>, tag: MjmlTag) -> usize {
    fn walk(nodes: &[Node<'_>], tag: MjmlTag) -> usize {
        nodes
            .iter()
            .filter_map(Node::as_element)
            .map(|el| usize::from(el.tag == tag) + walk(&el.children, tag))
            .sum()
    }
    walk(&document.children, tag)
}

#[test]
fn absolute_url_include_is_fetched_and_spliced() {
    let stub = StubFetch::new(&[(
        "https://cdn.test/parts/header.mjml",
        "<mj-section><mj-column><mj-text>remote</mj-text></mj-column></mj-section>",
    )]);
    let loader = HttpLoader::with_fetch(Box::new(stub));

    let alloc = Allocator::new();
    let builder = AstBuilder::new(&alloc);
    let parsed = mjmx_parser::parse(
        "<mjml><mj-body><mj-include path=\"https://cdn.test/parts/header.mjml\" /></mj-body></mjml>",
        &alloc,
    );
    let (document, diags) = resolve_includes(
        &builder,
        &parsed.document,
        &loader,
        std::path::Path::new("/proj"),
    );

    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(count_tag(&document, MjmlTag::MjInclude), 0);
    assert_eq!(count_tag(&document, MjmlTag::MjText), 1);
}

#[test]
fn relative_include_inside_a_remote_partial_resolves_against_its_url() {
    let stub = StubFetch::new(&[
        (
            "https://cdn.test/parts/outer.mjml",
            // `./` stays in parts/, `../` climbs to the site root.
            "<mj-section><mj-column><mj-include path=\"./inner.mjml\" />\
             <mj-include path=\"../shared/foot.mjml\" /></mj-column></mj-section>",
        ),
        ("https://cdn.test/parts/inner.mjml", "<mj-text>in</mj-text>"),
        (
            "https://cdn.test/shared/foot.mjml",
            "<mj-text>foot</mj-text>",
        ),
    ]);
    let loader = HttpLoader::with_fetch(Box::new(stub));

    let alloc = Allocator::new();
    let builder = AstBuilder::new(&alloc);
    let parsed = mjmx_parser::parse(
        "<mjml><mj-body><mj-include path=\"https://cdn.test/parts/outer.mjml\" /></mj-body></mjml>",
        &alloc,
    );
    let (document, diags) = resolve_includes_from(
        &builder,
        &parsed.document,
        &loader,
        &IncludeBase::dir("/proj"),
    );

    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(count_tag(&document, MjmlTag::MjInclude), 0);
    assert_eq!(count_tag(&document, MjmlTag::MjText), 2);
}

#[test]
fn a_document_loaded_from_a_url_base_resolves_bare_relative_paths() {
    let stub = StubFetch::new(&[("https://cdn.test/site/p.mjml", "<mj-text>p</mj-text>")]);
    let loader = HttpLoader::with_fetch(Box::new(stub));

    let alloc = Allocator::new();
    let builder = AstBuilder::new(&alloc);
    let parsed = mjmx_parser::parse(
        "<mjml><mj-body><mj-include path=\"p.mjml\" /></mj-body></mjml>",
        &alloc,
    );
    let (document, diags) = resolve_includes_from(
        &builder,
        &parsed.document,
        &loader,
        // No trailing slash on purpose: `IncludeBase::url` normalizes it.
        &IncludeBase::url("https://cdn.test/site"),
    );

    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(count_tag(&document, MjmlTag::MjText), 1);
}

#[test]
fn url_cycles_are_detected() {
    let stub = StubFetch::new(&[
        (
            "https://cdn.test/a.mjml",
            "<mj-include path=\"./b.mjml\" />",
        ),
        (
            "https://cdn.test/b.mjml",
            // Spelled differently on purpose: normalization must still match a.mjml.
            "<mj-include path=\"../a.mjml\" />",
        ),
    ]);
    let loader = HttpLoader::with_fetch(Box::new(stub));

    let alloc = Allocator::new();
    let builder = AstBuilder::new(&alloc);
    let parsed = mjmx_parser::parse("<mj-include path=\"https://cdn.test/a.mjml\" />", &alloc);
    let (_document, diags) = resolve_includes_from(
        &builder,
        &parsed.document,
        &loader,
        &IncludeBase::dir("/proj"),
    );

    assert!(
        diags
            .iter()
            .any(|d| d.severity == Severity::Error && d.message.contains("cycle")),
        "expected a cycle diagnostic, got {diags:?}"
    );
}

#[test]
fn missing_remote_partial_is_an_error_not_a_panic() {
    let loader = HttpLoader::with_fetch(Box::new(StubFetch::new(&[])));

    let alloc = Allocator::new();
    let builder = AstBuilder::new(&alloc);
    let parsed = mjmx_parser::parse("<mj-include path=\"https://cdn.test/gone.mjml\" />", &alloc);
    let (_document, diags) = resolve_includes_from(
        &builder,
        &parsed.document,
        &loader,
        &IncludeBase::dir("/proj"),
    );

    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
}

#[test]
fn combined_loader_mixes_local_and_remote_in_one_document() {
    let dir = std::env::temp_dir().join(format!(
        "mjmx-combined-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("local.mjml"), "<mj-text>local</mj-text>").unwrap();

    let stub = StubFetch::new(&[
        (
            "https://cdn.test/remote.mjml",
            // A relative include inside a remote partial must stay remote — it must
            // never fall back to the local filesystem.
            "<mj-text>remote</mj-text><mj-include path=\"./nested.mjml\" />",
        ),
        ("https://cdn.test/nested.mjml", "<mj-text>nested</mj-text>"),
    ]);
    let loader = CombinedLoader::with_http(HttpLoader::with_fetch(Box::new(stub)));

    let alloc = Allocator::new();
    let builder = AstBuilder::new(&alloc);
    let parsed = mjmx_parser::parse(
        "<mjml><mj-body><mj-section><mj-column>\
         <mj-include path=\"./local.mjml\" />\
         <mj-include path=\"https://cdn.test/remote.mjml\" />\
         </mj-column></mj-section></mj-body></mjml>",
        &alloc,
    );
    let (document, diags) = resolve_includes(&builder, &parsed.document, &loader, &dir);

    std::fs::remove_dir_all(&dir).ok();
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(count_tag(&document, MjmlTag::MjText), 3);
}

#[test]
fn local_loader_refuses_to_resolve_from_a_url_base() {
    // The safety property behind `CombinedLoader`: nothing reachable from a remote
    // partial can name a local file.
    let loader = mjmx_transformer::LocalLoader;
    let err: Diagnostic = mjmx_transformer::IncludeLoader::load(
        &loader,
        "../../etc/passwd",
        &IncludeBase::url("https://cdn.test/x/"),
    )
    .unwrap_err();
    assert!(err.message.contains("does not fetch over the network"));
}

#[test]
fn real_ureq_transport_fetches_from_a_local_socket() {
    // Bind an ephemeral loopback port and serve exactly one canned partial. This is
    // the only test that exercises `UreqFetch`, and it never leaves the machine.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let body = "<mj-text>over the wire</mj-text>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let loader = HttpLoader::new();
    let alloc = Allocator::new();
    let builder = AstBuilder::new(&alloc);
    let source = format!(
        "<mjml><mj-body><mj-section><mj-column>\
         <mj-include path=\"http://127.0.0.1:{port}/parts/p.mjml\" />\
         </mj-column></mj-section></mj-body></mjml>"
    );
    let parsed = mjmx_parser::parse(&source, &alloc);
    let (document, diags) = resolve_includes_from(
        &builder,
        &parsed.document,
        &loader,
        &IncludeBase::dir("/proj"),
    );

    server.join().unwrap();
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(count_tag(&document, MjmlTag::MjText), 1);
}
