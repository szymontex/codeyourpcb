//! LSP Backend implementing the LanguageServer trait.
//!
//! The Backend struct holds all server state and implements the tower-lsp
//! LanguageServer trait for handling LSP requests and notifications.

use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, Hover, HoverParams, HoverProviderCapability, InitializedParams,
    InitializeParams, InitializeResult, SaveOptions, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Uri,
};
use tower_lsp::{Client, LanguageServer};
use tracing::{debug, info};

use crate::diagnostics::run_diagnostics;
use crate::document::DocumentState;
use crate::hover::hover_at_position;

/// LSP Backend holding server state and implementing LanguageServer.
pub struct Backend {
    /// Client handle for sending notifications back to the editor.
    client: Client,
    /// Open documents indexed by URI.
    documents: Arc<DashMap<Uri, DocumentState>>,
}

impl Backend {
    /// Create a new backend with the given client.
    pub fn new(client: Client) -> Self {
        Backend {
            client,
            documents: Arc::new(DashMap::new()),
        }
    }

    /// Parse a document and update its state.
    fn parse_document(&self, uri: &Uri) {
        if let Some(mut doc) = self.documents.get_mut(uri) {
            doc.parse();
        }
    }

    /// Build the board world for a document (for DRC).
    fn build_world(&self, uri: &Uri) -> bool {
        if let Some(mut doc) = self.documents.get_mut(uri) {
            doc.build_world()
        } else {
            false
        }
    }

    /// Publish diagnostics for a document.
    async fn publish_diagnostics(&self, uri: &Uri) {
        if let Some(doc) = self.documents.get(uri) {
            let diagnostics = run_diagnostics(&doc);
            self.client
                .publish_diagnostics(uri.clone(), diagnostics, Some(doc.version))
                .await;
        }
    }
}

#[async_trait::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        info!("CodeYourPCB LSP initializing");

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Full document sync - we receive the entire document on changes
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                // Hover provider for component/net info
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // We'll add more capabilities as we implement them:
                // - completion_provider
                // - definition_provider
                // - references_provider
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "cypcb-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        info!("CodeYourPCB LSP initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        info!("CodeYourPCB LSP shutting down");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let version = params.text_document.version;

        debug!("Document opened: {:?}", uri);

        // Create document state
        let doc = DocumentState::new(uri.clone(), content, version);
        self.documents.insert(uri.clone(), doc);

        // Parse and build world
        self.parse_document(&uri);
        self.build_world(&uri);

        // Publish diagnostics
        self.publish_diagnostics(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        debug!("Document changed: {:?}", uri);

        // With FULL sync, we get the entire document in the first change
        if let Some(change) = params.content_changes.into_iter().next() {
            if let Some(mut doc) = self.documents.get_mut(&uri) {
                doc.update(change.text, version);
            }
        }

        // Re-parse on every change for immediate feedback
        self.parse_document(&uri);

        // Build world for DRC (consider debouncing for large files)
        self.build_world(&uri);

        // Publish diagnostics
        self.publish_diagnostics(&uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        debug!("Document saved: {:?}", uri);

        // If save includes text, update the document
        if let Some(text) = params.text {
            if let Some(mut doc) = self.documents.get_mut(&uri) {
                let new_version = doc.version + 1;
                doc.update(text, new_version);
            }

            // Re-parse and build world
            self.parse_document(&uri);
            self.build_world(&uri);
        }

        // Always publish diagnostics on save
        self.publish_diagnostics(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        debug!("Document closed: {:?}", uri);

        // Remove from open documents
        self.documents.remove(&uri);

        // Clear diagnostics
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        debug!("Hover request at {:?}", position);

        if let Some(doc) = self.documents.get(&uri) {
            Ok(hover_at_position(&doc, &position))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Backend tests require async runtime, covered by integration tests
    // Here we test basic construction

    #[test]
    fn test_backend_documents_concurrent() {
        // DashMap supports concurrent access
        let docs: Arc<DashMap<String, i32>> = Arc::new(DashMap::new());
        docs.insert("a".into(), 1);
        docs.insert("b".into(), 2);

        assert_eq!(*docs.get("a").unwrap(), 1);
        assert_eq!(*docs.get("b").unwrap(), 2);
    }
}
