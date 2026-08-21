# HANDOVER - 2026-08-21 - CodeYourPCB

## Goal

A PCB tool a professional can lay a board out in, and whose every number can
be traced to where it came from.

## Where the work lives

- **The container is the source of truth.** `ssh flightcore "docker exec -u abc code-server bash -lc 'export PATH=/config/.cargo/bin:$PATH; cd /workspace/codeyourpcb && <cmd>'"`
- Local `~/szymontex/codeyourpcb` is stale. Do not read it.
- Branch `fix/cli-check-drc`, tracking origin, **488 commits ahead of main, all
  pushed**, tree clean at `f249844`.
- `docs/TRACKER.md` in the container is the real record. Read it first.

## Editing rules that must not be broken

- Fetch from the container, edit locally, `scp` back, check `md5sum` both sides.
- **Anchor on a line, not on a substring.** Three separate regressions this
  session came from the same mistake: `s.index()` matching inside a different
  function, a markdown `## heading` match landing inside a `### heading`, and a
  struct derive ending up on the wrong type. Cuts spanning more than one item
  go by line number with both ends asserted.
- **Every new test gets a mutation.** A test a mutation does not kill is empty.
  A mutation that *survives* is a gap - one survived this session and the gap
  it found was real.
- **Verify before saying done.** The command whose output proves the claim goes
  in the commit message. Running the binary counts; reasoning does not.
- `./scripts/quality-gate.sh` before any commit. It caught a startup crash, a
  clippy lint four separate times, a manual `% 100 == 0`, a tree-sitter-only
  compile break and a viewer that had no name for a new violation kind. It is
  not a formality.
- New keyboard shortcuts must be listed in `CLAUDE.md` or the registry test
  fails.
- **Push after a green gate** - standing consent, 2026-08-20.
- Stage named paths. `git add -A` swept two foreign files from another project
  into commits this session; `viewer/pkg/*.wasm` is gitignored and needs
  `git add -f`.

## Patterns worth knowing before touching anything

**A symmetric fixture cannot catch an index error.** Three shipped index bugs
had one cause: neighbouring layers of a symmetric stack give the same answer,
so a rule reading the wrong index produces the right number. `cypcb-fixtures`
now holds a stack where all four copper layers answer differently. Use it for
anything that reads a layer index.

**Re-measure before rewriting.** Four items on the tracker turned out to be
already done or wrong when measured: the allocator "41% in malloc/free" was
0.46%, a `get_unchecked` action was already in the code, a grid-packing suspect
had been reverted, and `pad <name>` had shipped. Read the code against the
action before acting on it.

**Provenance over silence.** Every fab number says where it came from - a
published page, this tool's arithmetic on one, `UNSOURCED`, or a standard it
cannot link to. Where a form cannot answer, the report says **not checked, not
passed** rather than staying quiet.

## What went in this session, newest first

`f249844` examples keep their assertions, guarded by a test that runs the CLI ·
`a0079a9` `spec { output 3.3V }` on a part · `5be584f` `within` evaluates ·
`d4fa265` neck-down level 2 · `d069d9f` `cypcb-fixtures` · `37d201b` inner
layer off-by-one · `6f7db5f` a board is checked against its own layer count's
table · `325a108` the impedance rule · `59241a6` `impedance 90ohm` on a net ·
`bba4619` `Stackup::environment_of` · `f99a5d8` IPC-2141 microstrip and
stripline · `d060b5b` `export --house` · `a9e8c7a` variant panel deleted ·
`14ec826` IPC presets stop citing an unreadable table · `44f867f` silk, mask
and paste figures read against their pages

## Two things deliberately left closed, with reasons

**The asymmetric stripline.** IPC-2141A computes it in several steps whose
sub-equations were revised by an official errata, so the published form differs
between issues of a document nobody here can read. That is not the microstrip's
position - there the form is unambiguous and only a third-party cross-check was
missing, which is why that one shipped with a caveat. Transcribing this one
would be guessing which revision.

**A third-party reference value for the impedance forms.** Four attempts
failed: two timeouts on the Analog Devices tutorial, a calculator page that
renders its formula as an image, and KiCad's own microstrip calculator carrying
an open issue saying it may be more than 20% out. Both anchors in the tests
were instead worked out **on paper before the code was run**, which is what
makes them evidence.

## Next step

`docs/TRACKER.md`, V6: **the same guard for the other promise examples make -
that their commands work.** Several carry a `// Check it with: cypcb check ...`
header, one carries `cypcb route` and `cypcb export`, and nothing runs them.
The by-hand sweep this session found two real defects; the only reason it is
not still finding them is that it was run once, by a person, on one day.

After that, the largest live item is **per-segment trace width**, which is what
neck-down level 3 needs and what would let the neck rule measure the necked
stretch rather than only the coherence of the declaration.

## Report style

Polish: result, next step, blocker. Details to the tracker, not the chat.
