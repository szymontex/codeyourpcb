# The language server

`cypcb-lsp` is a Language Server Protocol server for `.cypcb` files. It reads
the language with the same parser the command line uses, checks the board with
the same design rules, and answers over stdio - so an editor shows what
`cypcb check` would say, while the file is being written.

This page is checked by a test. `crates/cypcb-lsp/tests/the_manual_matches_the_server.rs`
starts the binary, asks it what it can do, and fails if this page and the answer
disagree in either direction - a capability claimed here and missing, or added
to the server and not written down.

## Build and run

```bash
cargo build -p cypcb-lsp --release
./target/release/cypcb-lsp
```

The server talks on stdin and stdout. It takes no options: `--stdio` is
accepted and ignored, because that is what editors habitually pass.

The `server` feature is on by default. It was off for months, and the crate
stopped compiling without anybody being told, so the flag now has to be turned
*off* deliberately (`--no-default-features`) if you want the diagnostics and
hover modules without the binary.

## What it answers

| Request | Capability | What it gives |
|---|---|---|
| Hover | `hoverProvider` | The part, net, footprint, board, zone or trace under the cursor, with its value, its pins and the nets they reach |
| Completion | `completionProvider` | Footprint names, net names, part names, property keys, layer names and top-level keywords. Trigger characters: `.`, space and `"` |
| Go to definition | `definitionProvider` | From a pin reference or a net name to where that part or net is declared, in the same file |
| Document sync | `textDocumentSync` | Full text on open and on change, and the text again on save |

Diagnostics are published on open, on change and on save, without being asked.
They carry two sources:

- `cypcb-parser` - syntax errors, unknown units, unknown component types, each
  on the span that caused it.
- `cypcb-drc` - design rule violations against the JLCPCB 2-layer rules, on the
  line where the part or trace is written. `unconnected-pin` and
  `unrouted-pin` come back as warnings rather than errors: a part exists before
  its net does, and a board being written is not a broken board.

The list is capped at 100 diagnostics per file, with a final entry saying how
many were dropped.

## What it does not answer

None of these is implemented, and none is advertised, so an editor will not
offer them: `semanticTokensProvider`, `referencesProvider`, `renameProvider`,
`documentFormattingProvider`, `codeActionProvider`, `documentSymbolProvider`,
`workspaceSymbolProvider`, `signatureHelpProvider`.

Syntax highlighting therefore does not come from this server. The browser
editor highlights `.cypcb` with its own Monaco grammar, which is a separate
piece of work in `viewer/src/editor/`.

## Connecting an editor

**Neovim**, using the built-in client. Written from the LSP interface above,
not executed - there is no Neovim in this project's build container, so treat
it as a starting point rather than a tested recipe:

```lua
vim.filetype.add({ extension = { cypcb = 'cypcb' } })

vim.api.nvim_create_autocmd('FileType', {
  pattern = 'cypcb',
  callback = function(args)
    vim.lsp.start({
      name = 'cypcb-lsp',
      cmd = { '/path/to/codeyourpcb/target/release/cypcb-lsp' },
      root_dir = vim.fs.dirname(vim.fs.find({ '.git' }, { upward = true })[1]),
    }, { bufnr = args.buf })
  end,
})
```

**VS Code**: no extension ships in this repository. A client has to be written -
a `LanguageClient` with `ServerOptions` pointing at the binary and
`documentSelector: [{ scheme: 'file', language: 'cypcb' }]` - and nobody has
written one. The browser application is not that client: it talks to the engine
through WASM, which `docs/api/lsp-server.md` describes.

**Anything else**: the server is an ordinary stdio LSP server. Point the client
at the binary and give it the `cypcb` language id.

## Verification

```bash
# The binary builds and the protocol tests drive it end to end.
cargo test -p cypcb-lsp

# What the server actually advertises, which the table above has to match.
cargo test -p cypcb-lsp --test the_manual_matches_the_server
```

Last verified: 2026-08-08.
