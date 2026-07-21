//! The `tower-lsp` adapter: protocol plumbing around [`crate::analysis`].
//!
//! This is the only place in the crate that is async. It keeps an in-memory store
//! of open documents (full text sync — the documents are small and reparsing is
//! cheap), and every request converts to a pure call into `analysis` and back.

use std::collections::HashMap;
use std::sync::RwLock;

use mjmx_formatter::FormatOptions;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentFormattingParams, DocumentSymbolParams, DocumentSymbolResponse, Hover, HoverParams,
    HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams, MessageType,
    OneOf, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextEdit, Url,
};
use tower_lsp::{Client, LanguageServer};

use crate::analysis;
use crate::line_index::LineIndex;

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
        let index = LineIndex::new(text);
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
                    TextDocumentSyncKind::FULL,
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
        // Full sync: the last content change carries the whole document.
        let Some(change) = params.content_changes.into_iter().next_back() else {
            return;
        };
        self.refresh(
            params.text_document.uri,
            change.text,
            Some(params.text_document.version),
        )
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
