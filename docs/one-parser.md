# One parser: what the second one costs, and what replacing it costs

The `.cypcb` language is read twice. This is the measurement behind choosing
which reader survives, written before any of the work, so the decision can be
checked against numbers rather than taste.

## Why there are two

`cypcb-parser` is a tree-sitter grammar with a generated C parser. C does not
reach the browser: the wasm build compiles the crate with
`default-features = false`, which leaves it **with no parser at all**, so
`viewer/src/wasm.ts` reads the DSL a second time in TypeScript - `parseSource`,
a hand-written line reader.

What that costs is measured, not argued
(`viewer/src/__tests__/parser-drift.test.ts`, every board in `examples/`
through both):

| board | what the browser shows against what the CLI exports |
|---|---|
| `v2-imports.cypcb` | an empty board; the CLI sees 6 parts on 7 nets |
| `v2-modules.cypcb` | 2 parts under their in-module names on 1 net; the CLI instantiates and gets 7 parts on 5 nets |
| `v2-interfaces.cypcb` | 2 parts and 2 nets **out of a body nothing instantiates**; the CLI correctly draws nothing |

The third row is the sharpest: the screen shows a board that does not exist.
A fourth defect - a pin-to-pin trace drawn in the Gerber and invisible on
screen - was fixed in both readers on 2026-08-07, which is what a fix in this
shape costs: twice.

Fourteen of the seventeen boards that parse agree on every field the test
compares, so this is a gap in what the second reader *implements*, not in how
it reads what it covers.

## Route (a): give the container a C toolchain that targets wasm32

The build already refuses to ship a wrong-architecture artifact
(`cypcb-parser/build.rs` checks that the first object starts with `\0asm`),
so the failure is loud. What it would take to satisfy it:

- our generated `grammar/src/parser.c`, and
- the tree-sitter runtime itself: **13,647 lines of C across 13 files** in
  `tree-sitter-0.25.10/src`, plus 2,204 lines of headers, which the crate
  compiles for every target.

Both must be built by clang against a wasm sysroot, and the engine targets
`wasm32-unknown-unknown`, which has no libc. The sysroot that exists is
`wasi-libc`, for `wasm32-wasip1`. That mismatch is the real cost of route (a),
and it buys nothing beyond the status quo: the browser would parse, and the
duplicate TypeScript reader would still be there to delete separately.

## Route (b): read `.cypcb` in Rust

What a Rust reader has to cover, counted:

| piece | size | fate under (b) |
|---|---|---|
| `grammar/grammar.js` | 663 lines, 83 rules | replaced; about a sixth of the rules are lexical (identifier, number, string, units, comments) and become a tokenizer |
| `src/parser.rs` (CST to AST) | 3,226 lines | **replaced, not added to** - a recursive-descent reader produces the AST directly |
| `src/ast.rs` | 1,224 lines, 47 public types | unchanged; this is the contract |
| `src/imports.rs` | 404 lines | unchanged; it works on the AST |
| `src/errors.rs` | 485 lines | unchanged; miette diagnostics keyed by span |
| `viewer/src/wasm.ts` `parseSource` | 439 lines | deleted |
| tree-sitter C runtime | 13,647 lines of C | gone from the build |

Nothing consumes the concrete syntax tree. Every caller - `cypcb-cli` (parse,
check, export, route, score), `cypcb-lsp` (`document.rs` calls `parse()`),
`cypcb-render`, `cypcb-world::sync` - takes the AST. The LSP's hover, goto,
completion and diagnostics work off AST spans, so a reader that fills `ast.rs`
with correct spans serves the tooling unchanged.

## The decision

**Route (b).** Not because it is smaller - it is not - but because route (a)
leaves the defect table above intact. The browser would still need modules and
imports implemented a second time in TypeScript, and every future construct
would land twice.

## How it gets verified

The oracle already exists, which is why this is a project rather than a
rewrite:

1. `crates/cypcb-world/tests/language_conformance.rs` - one fixture that uses
   every construct, asserting what reaches the board model.
2. `crates/cypcb-cli/tests/the_examples_still_say_what_they_show.rs` - every
   example parses, and the two written to fail still fail.
3. `cypcb parse -o ast` prints the tree-sitter AST as JSON today. A new reader
   is correct when it prints byte-identical JSON for every example, which is a
   differential test against the parser being replaced rather than against a
   hand-written expectation.
4. `viewer/src/__tests__/parser-drift.test.ts` - the drift list must reach zero
   and the file must stop being needed.

Order of work, each step shippable on its own:

1. ~~Tokenizer plus the reader for the constructs `parseSource` already covers~~
   **Done.** `src/lexer.rs` and `src/reader.rs`, behind `rust-parser`.
2. ~~Modules, imports, interfaces and assertions~~ **Done.** The differential
   test covers every example except the two written to fail parsing: 147
   definitions across 17 boards, identical. Error parity is pinned separately
   in `tests/error_parity.rs`.
3. ~~Flip the default feature~~ **Done.** `rust-parser` is `cypcb-parser`'s
   default and `parse` is the reader everywhere: the CLI's five commands, the
   LSP, `cypcb-world::sync`, and both builds of `cypcb-render`. Nothing in the
   workspace depends on the `tree-sitter` crate any more -
   `cargo tree --workspace -i tree-sitter` finds no such package - so a default
   build compiles no C at all.

   The grammar, `parser.rs` and the generated `parser.c` stay behind
   `tree-sitter-parser`, because they are what the reader is checked against:
   `differential.rs` and `error_parity.rs` need both parsers in one binary.
   Deleting them would delete the evidence.
4. ~~Delete `parseSource` and the drift test with it~~ **Done.** The viewer
   reads boards through the engine; the second reader, its helpers and the
   drift test are gone.

Spans were the open risk, because the LSP is built on them and the differential
test strips them before comparing. They were then measured rather than assumed,
in `tests/spans_point_at_the_source.rs`: every identifier's span spells the
identifier and every string's span is the quoted literal, checked against the
source text rather than against another parser; and the diagnostic in the same
file reports **147 definitions with identical spans, none different, widest gap
0 bytes**. The two parsers agree on boundaries exactly.

What the browser build paid for it, measured on 2026-08-07:

| | before | after |
|---|---|---|
| `cypcb_render_bg.wasm` | 751,995 | 1,044,164 |
| the same, gzipped | 290,004 | 411,147 |
| all JS in `viewer/dist` | 14,791,848 | 14,781,791 |

**This is not a size win and should not be sold as one**: 292KB of wasm against
10KB of JavaScript. What it buys is that a board using `module` or `import`
draws on screen what the CLI exports, which is what the table at the top of
this document says it did not.

The grammar file stays regardless: it is what the editor's syntax highlighting
and the LSP's future incremental parsing would use, and it is the readable
statement of the language.

## Verification

```sh
# The drift this document is about, board by board
cd viewer && npx vitest run src/__tests__/parser-drift.test.ts

# The sizes quoted above
wc -l crates/cypcb-parser/grammar/grammar.js crates/cypcb-parser/src/*.rs
grep -cE '^pub (struct|enum) ' crates/cypcb-parser/src/ast.rs
wc -l ~/.cargo/registry/src/*/tree-sitter-0.25.10/src/*.c   # inside the container

# Who consumes the parser, and what they take
grep -rln 'cypcb_parser::parse\|CypcbParser' crates/*/src
```

Last verified: 2026-08-07.
