//! CodeYourPCB Language Server entry point.
//!
//! Starts the LSP server, by default listening on stdio.
//!
//! # Usage
//!
//! ```bash
//! # Default: stdio transport (for editors)
//! cypcb-lsp --stdio
//!
//! # Or just:
//! cypcb-lsp
//! ```

#[cfg(feature = "server")]
use tower_lsp::{LspService, Server};

#[cfg(feature = "server")]
use cypcb_lsp::Backend;

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    // Create LSP service
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend::new(client));

    // Serve on stdio
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(not(feature = "server"))]
fn main() {
    eprintln!("cypcb-lsp: server feature is required");
    eprintln!("Build with: cargo build -p cypcb-lsp --features server");
    std::process::exit(1);
}
