# T07: LSP Completions & Go-to-Definition

**Slice:** S05 — **Milestone:** M001

## Description

Add autocomplete and go-to-definition to the LSP server for a complete IDE experience.

Purpose: Autocomplete reduces typing and prevents errors. Go-to-definition enables quick navigation. DEV-02 requirement (second half).
Output: Working completions and navigation in IDE

## Must-Haves

- [ ] "Autocomplete suggests footprint names"
- [ ] "Autocomplete suggests net names in pin references"
- [ ] "Autocomplete suggests component names"
- [ ] "Go-to-definition navigates from pin ref to component"

## Files

- `crates/cypcb-lsp/src/completion.rs`
- `crates/cypcb-lsp/src/goto.rs`
- `crates/cypcb-lsp/src/backend.rs`
