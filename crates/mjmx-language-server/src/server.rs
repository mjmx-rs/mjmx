//! The `tower-lsp` adapter: protocol plumbing around [`crate::analysis`].
//!
//! This is the only place in the crate that is async. It keeps an in-memory store
//! of open documents (**incremental** text sync — see [`apply_change`]), and every
//! request converts to a pure call into `analysis` and back.

use std::collections::HashMap;
use std::sync::RwLock;

use mjmx_formatter::FormatOptions;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentFormattingParams, DocumentSymbolParams, DocumentSymbolResponse, Hover, HoverParams,
    HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams, MessageType,
    OneOf, ServerCapabilities, ServerInfo, TextDocumentContentChangeEvent,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url,
};
use tower_lsp::{Client, LanguageServer};

use crate::analysis;
use crate::line_index::LineIndex;

/// Apply one LSP content change to `text`.
///
/// Clients send two shapes down the same channel, and both must be handled:
///
/// - **ranged** (`range: Some(_)`) — splice `change.text` over the given range.
///   The range is in LSP coordinates (line + UTF-16 code units), so it is
///   converted back to byte offsets through `index`, which describes `text` *as it
///   is right now*. That is why changes are applied one at a time, each against a
///   freshly built index: within a single `didChange` batch every change is
///   specified relative to the document as left by the previous one.
/// - **full replacement** (`range: None`) — `change.text` *is* the new document.
///   Clients still send this shape under incremental sync (e.g. after an undo of a
///   large edit, or on a resync), so it is not a legacy path.
///
/// A start offset past the end clamps to the end and an inverted range (`end` before
/// `start`) is normalised, so a malformed client cannot panic the server.
fn apply_change(index: &LineIndex, change: TextDocumentContentChangeEvent) -> String {
    let Some(range) = change.range else {
        // Full replacement: the event carries the whole document.
        return change.text;
    };

    let start = index.offset(range.start);
    let end = index.offset(range.end).max(start);
    let text = index.text();
    let mut updated = String::with_capacity(text.len() - (end - start) + change.text.len());
    updated.push_str(&text[..start]);
    updated.push_str(&change.text);
    updated.push_str(&text[end..]);
    updated
}

/// The language server: a client handle plus the open-document store.
#[derive(Debug)]
pub struct Backend {
    client: Client,
    /// Open documents, each held as a prebuilt [`LineIndex`] over its text.
    documents: RwLock<HashMap<Url, LineIndex>>,
}

impl Backend {
    /// Create a backend talking to `client`.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
        }
    }

    /// Read the index for `uri`, if the document is open.
    fn document(&self, uri: &Url) -> Option<LineIndex> {
        self.documents.read().ok()?.get(uri).cloned()
    }

    /// Store `text` for `uri` and publish fresh diagnostics for it.
    async fn refresh(&self, uri: Url, text: String, version: Option<i32>) {
        self.refresh_index(uri, LineIndex::new(text), version).await;
    }

    /// [`Self::refresh`] for a document whose index has already been built.
    async fn refresh_index(&self, uri: Url, index: LineIndex, version: Option<i32>) {
        if let Ok(mut documents) = self.documents.write() {
            documents.insert(uri.clone(), index.clone());
        }
        let diagnostics = analysis::diagnostics(index.text(), &index, &uri);
        self.client
            .publish_diagnostics(uri, diagnostics, version)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                completion_provider: Some(CompletionOptions {
                    // `<` opens tag completion, a space inside a tag opens attribute
                    // completion; both are also reachable by typing normally.
                    trigger_characters: Some(vec!["<".to_string(), " ".to_string()]),
                    ..CompletionOptions::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "mjmx-language-server".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "mjmx-language-server ready")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        self.refresh(document.uri, document.text, Some(document.version))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if params.content_changes.is_empty() {
            return;
        }

        // Incremental sync: fold the changes in order, each against the document as
        // the previous one left it. An unknown document starts from empty, which is
        // only reachable if a client sends `didChange` without `didOpen`; a full
        // replacement then still recovers the right text.
        let mut index = self.document(&uri).unwrap_or_else(|| LineIndex::new(""));
        for change in params.content_changes {
            index = LineIndex::new(apply_change(&index, change));
        }

        self.refresh_index(uri, index, Some(params.text_document.version))
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = match params.text {
            Some(text) => text,
            None => match self.document(&uri) {
                Some(index) => index.text().to_string(),
                None => return,
            },
        };
        self.refresh(uri, text, None).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Ok(mut documents) = self.documents.write() {
            documents.remove(&uri);
        }
        // Clear the squiggles for a file that is no longer open.
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let position = params.text_document_position;
        let Some(index) = self.document(&position.text_document.uri) else {
            return Ok(None);
        };
        let items = analysis::completion(&index, position.position);
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let position = params.text_document_position_params;
        let Some(index) = self.document(&position.text_document.uri) else {
            return Ok(None);
        };
        Ok(analysis::hover(&index, position.position))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let Some(index) = self.document(&params.text_document.uri) else {
            return Ok(None);
        };
        let options = FormatOptions {
            indent_width: params.options.tab_size as usize,
        };
        Ok(analysis::formatting(&index, &options))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let Some(index) = self.document(&params.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(DocumentSymbolResponse::Nested(
            analysis::document_symbols(&index),
        )))
    }
}

#[cfg(test)]
mod tests {
    use tower_lsp::lsp_types::{Position, Range};

    use super::{LineIndex, TextDocumentContentChangeEvent, apply_change};

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    /// A ranged (incremental) change event.
    fn edit(start: Position, end: Position, text: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range: Some(Range { start, end }),
            range_length: None,
            text: text.to_string(),
        }
    }

    /// A full-replacement change event (`range: None`).
    fn replace(text: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_string(),
        }
    }

    /// Apply a sequence of changes the way [`super::Backend::did_change`] does.
    fn apply_all(text: &str, changes: Vec<TextDocumentContentChangeEvent>) -> String {
        let mut index = LineIndex::new(text);
        for change in changes {
            index = LineIndex::new(apply_change(&index, change));
        }
        index.text().to_string()
    }

    #[test]
    fn a_range_less_event_replaces_the_whole_document() {
        assert_eq!(apply_all("<mjml>\n</mjml>\n", vec![replace("x")]), "x");
        // Including replacing with nothing.
        assert_eq!(apply_all("anything", vec![replace("")]), "");
    }

    #[test]
    fn a_full_replacement_after_ranged_edits_still_wins() {
        let changes = vec![
            edit(pos(0, 0), pos(0, 1), "X"),
            replace("<mjml></mjml>"),
            edit(pos(0, 6), pos(0, 6), "!"),
        ];
        assert_eq!(apply_all("abc", changes), "<mjml>!</mjml>");
    }

    #[test]
    fn an_insertion_is_an_empty_range() {
        assert_eq!(
            apply_all("<mjml></mjml>", vec![edit(pos(0, 6), pos(0, 6), "hi")]),
            "<mjml>hi</mjml>"
        );
    }

    #[test]
    fn a_deletion_is_an_empty_replacement() {
        assert_eq!(
            apply_all("<mjml>junk</mjml>", vec![edit(pos(0, 6), pos(0, 10), "")]),
            "<mjml></mjml>"
        );
    }

    #[test]
    fn an_insertion_at_end_of_document_appends() {
        let text = "line one\nline two";
        // The very last position of the document.
        let index = LineIndex::new(text);
        let end = index.position(text.len());
        assert_eq!(end, pos(1, 8));
        assert_eq!(
            apply_all(text, vec![edit(end, end, "!")]),
            "line one\nline two!"
        );
    }

    #[test]
    fn an_insertion_past_the_end_of_document_clamps_instead_of_panicking() {
        let text = "abc";
        assert_eq!(
            apply_all(text, vec![edit(pos(99, 99), pos(99, 99), "!")]),
            "abc!"
        );
        // A column past the end of a line clamps to that line's end.
        assert_eq!(
            apply_all("ab\ncd", vec![edit(pos(0, 99), pos(0, 99), "!")]),
            "ab!\ncd"
        );
    }

    #[test]
    fn an_inverted_range_is_normalised_rather_than_panicking() {
        // `end` before `start`: a malformed client must not take the server down.
        assert_eq!(
            apply_all("abcdef", vec![edit(pos(0, 4), pos(0, 1), "X")]),
            "abcdXef"
        );
    }

    #[test]
    fn an_edit_spanning_line_boundaries_joins_lines() {
        let text = "one\ntwo\nthree\n";
        // Delete from the middle of line 0 to the middle of line 2.
        assert_eq!(
            apply_all(text, vec![edit(pos(0, 1), pos(2, 2), "-")]),
            "o-ree\n"
        );
    }

    #[test]
    fn an_edit_can_introduce_new_lines_and_the_index_follows() {
        let text = "a\nb";
        let once = apply_all(text, vec![edit(pos(1, 0), pos(1, 0), "x\ny\n")]);
        assert_eq!(once, "a\nx\ny\nb");
        // The follow-up edit is expressed against the *new* line numbering.
        let index = LineIndex::new(&once);
        assert_eq!(index.line_count(), 4);
        assert_eq!(
            apply_all(&once, vec![edit(pos(3, 0), pos(3, 1), "B")]),
            "a\nx\ny\nB"
        );
    }

    // --- multi-byte characters -------------------------------------------------
    //
    // The bug class this whole module exists to prevent: LSP columns are UTF-16
    // code units, `String` indices are UTF-8 bytes. "é" is 2 bytes / 1 unit, "🎉"
    // is 4 bytes / 2 units. Getting this wrong splices mid-character and corrupts
    // the document (or panics on a non-char-boundary slice).

    #[test]
    fn an_edit_after_two_byte_characters_lands_on_the_right_byte() {
        // "café" — the `é` is 2 bytes, so byte 5 but column 4.
        assert_eq!(
            apply_all("café!", vec![edit(pos(0, 4), pos(0, 5), "?")]),
            "café?"
        );
    }

    #[test]
    fn an_edit_after_an_astral_character_lands_on_the_right_byte() {
        // "🎉" is 4 bytes and 2 UTF-16 units, so the `b` is at column 2, byte 4.
        assert_eq!(
            apply_all("🎉ab", vec![edit(pos(0, 2), pos(0, 3), "X")]),
            "🎉Xb"
        );
    }

    #[test]
    fn a_multi_byte_character_can_be_deleted_whole() {
        assert_eq!(
            apply_all("a🎉b", vec![edit(pos(0, 1), pos(0, 3), "")]),
            "ab"
        );
        assert_eq!(apply_all("aéb", vec![edit(pos(0, 1), pos(0, 2), "")]), "ab");
    }

    #[test]
    fn multi_byte_characters_can_be_inserted() {
        assert_eq!(
            apply_all("ab", vec![edit(pos(0, 1), pos(0, 1), "é🎉")]),
            "aé🎉b"
        );
        // And the follow-up edit sees the new UTF-16 columns: "é🎉" is 3 units.
        assert_eq!(
            apply_all("aé🎉b", vec![edit(pos(0, 4), pos(0, 5), "B")]),
            "aé🎉B"
        );
    }

    #[test]
    fn a_range_surrounded_by_multi_byte_text_edits_only_the_middle() {
        let text = "héllo 🎉 <mj-text>wörld</mj-text> 🎉";
        let index = LineIndex::new(text);
        let start = index.position(text.find("wörld").unwrap());
        let end = index.position(text.find("</mj-text>").unwrap());
        assert_eq!(
            apply_all(text, vec![edit(start, end, "wörld!")]),
            "héllo 🎉 <mj-text>wörld!</mj-text> 🎉"
        );
    }

    #[test]
    fn multi_byte_edits_spanning_lines_stay_aligned() {
        let text = "héllo 🎉\nwörld 🎉\ntail";
        // From just after the first emoji to just before the second line's emoji.
        assert_eq!(
            apply_all(text, vec![edit(pos(0, 8), pos(1, 6), "|")]),
            "héllo 🎉|🎉\ntail"
        );
    }

    #[test]
    fn crlf_documents_edit_without_eating_the_carriage_return() {
        let text = "<mjml>\r\n  <mj-body>\r\n</mjml>\r\n";
        // Column 99 on line 0 clamps *before* the `\r`, so the terminator survives.
        assert_eq!(
            apply_all(text, vec![edit(pos(0, 6), pos(0, 99), "!")]),
            "<mjml>!\r\n  <mj-body>\r\n</mjml>\r\n"
        );
        // An edit spanning a CRLF terminator removes both bytes with it.
        assert_eq!(
            apply_all(text, vec![edit(pos(0, 0), pos(1, 2), "")]),
            "<mj-body>\r\n</mjml>\r\n"
        );
    }

    #[test]
    fn edits_apply_in_order_within_one_batch() {
        // Each change is expressed against the document the previous one produced,
        // which is exactly why `did_change` rebuilds the index between them.
        let changes = vec![
            edit(pos(0, 0), pos(0, 0), "🎉"),
            // After the emoji, "abc" starts at column 2.
            edit(pos(0, 2), pos(0, 3), "A"),
            edit(pos(0, 4), pos(0, 5), "C"),
        ];
        assert_eq!(apply_all("abc", changes), "🎉AbC");
    }

    // --- property-style differential test --------------------------------------

    /// A deterministic xorshift PRNG, so a failure is always reproducible and the
    /// test needs no dependency.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            if bound == 0 {
                0
            } else {
                (self.next() % bound as u64) as usize
            }
        }
    }

    /// Round `offset` down to the nearest offset that has an LSP [`Position`] of its
    /// own: a UTF-8 character boundary that is not *inside* a `\r\n` terminator.
    ///
    /// A caret can sit before or after a CRLF pair but never between the two bytes,
    /// so byte offsets there are not round-trippable through `Position` by anyone —
    /// that is a property of the protocol, not a bug in the conversion, and the
    /// differential test must not generate them.
    fn snap(text: &str, mut offset: usize) -> usize {
        while offset > 0 && !text.is_char_boundary(offset) {
            offset -= 1;
        }
        let bytes = text.as_bytes();
        if offset > 0 && bytes.get(offset) == Some(&b'\n') && bytes[offset - 1] == b'\r' {
            offset - 1
        } else {
            offset
        }
    }

    #[test]
    fn random_edit_sequences_match_a_plain_string_splice() {
        // The oracle: the same edits applied directly to a `String` by byte offset.
        // The subject: those offsets expressed as LSP UTF-16 positions and put back
        // through `apply_change`. Any offset/encoding mistake diverges immediately.
        const INSERTS: [&str; 8] = ["", "x", "\n", "é", "🎉", "\r\n", "<mj-text>", "ö🎉\n"];

        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        for case in 0..200u32 {
            let mut oracle = String::from("<mjml>\r\n  <mj-body>\né🎉\n    café\n</mjml>\n");
            for step in 0..25u32 {
                let a = snap(&oracle, rng.below(oracle.len() + 1));
                let b = snap(&oracle, rng.below(oracle.len() + 1));
                let (start, end) = if a <= b { (a, b) } else { (b, a) };
                let inserted = INSERTS[rng.below(INSERTS.len())];

                let index = LineIndex::new(oracle.clone());
                let change = edit(index.position(start), index.position(end), inserted);
                let applied = apply_change(&index, change);

                oracle.replace_range(start..end, inserted);
                assert_eq!(
                    applied, oracle,
                    "diverged at case {case} step {step} \
                     (bytes {start}..{end}, inserting {inserted:?})"
                );
            }
        }
    }
}
