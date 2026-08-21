# HANDOVER - 2026-08-21 - CodeYourPCB

## Goal

A PCB tool a professional can lay a board out in, and whose every number can
be traced to where it came from.

## Where the work lives

- **The container is the source of truth.** `ssh flightcore "docker exec -u abc code-server bash -lc 'export PATH=/config/.cargo/bin:$PATH; cd /workspace/codeyourpcb && <cmd>'"`
- Local `~/szymontex/codeyourpcb` is stale. Do not read it.
- Branch `fix/cli-check-drc`, tracking origin, **493 commits ahead of main, all
  pushed**, tree clean at `1e2f971`.
- `docs/TRACKER.md` in the container is the real record. Read it first.
- One untracked file, `viewer/shot.mjs`, belongs to another project that writes
  into this checkout. Leave it. Two of its siblings were swept into commits
  earlier by `git add -A`, which is why staging is by named path now.

## Editing rules that must not be broken

- Fetch from the container, edit locally, `scp` back, check `md5sum` both sides.
- **Anchor on a line, not on a substring.** Three regressions this session came
  from the same mistake: `s.index()` matching inside a different function, a
  markdown `## heading` match landing inside a `### heading`, and a struct
  derive ending up on the wrong type. Cuts spanning more than one item go by
  line number with both ends asserted.
- **Every new test gets a mutation.** A test a mutation does not kill is empty.
  A mutation that *survives* is a gap - one survived this session and the gap
  it found was real.
- **A status read through a pipe is not the command's status.** `cmd | head`
  then `$?` gives `head`'s. That nearly produced a false finding today.
- `./scripts/quality-gate.sh` before any commit. It caught a startup crash, the
  `doc_lazy_continuation` lint four separate times, a manual `% 100 == 0`, a
  tree-sitter-only compile break, and a viewer with no name for a new violation
  kind. Never begin a wrapped doc line with a dash - that is the lint.
- **Push after a green gate** - standing consent, 2026-08-20.
- Stage named paths. `viewer/pkg/*.wasm` is gitignored and needs `git add -f`
  when it genuinely changed; the gate rebuilds it, so `git checkout --` it when
  the commit has nothing to do with the viewer.

## The pattern that cost the most this session

**Read the code against the action before acting on it.** Six tracker actions
turned out to be already done or already false when measured:

- the allocator's "41% in malloc/free" was **0.46%**
- a `get_unchecked` action was already in the code
- a grid-packing suspect had been measured and reverted
- `pad <name>` had shipped
- the variant grid's winners were already shipped as variants
- the one-knob neighbourhood probe was already written - **that action was mine**

Two more worth keeping: **a symmetric fixture cannot catch an index error**
(three shipped index bugs, one cause; `cypcb-fixtures` is the answer), and
**provenance over silence** - every fab number says where it came from, and
where a form cannot answer, the report says *not checked, not passed*.

## What went in this session, newest first

`1e2f971` a thirteenth routing variant, found by probing a neighbourhood ·
`5162f5e` best-of-N re-measured, a doubly stale action closed · `f1a7490`
v2-interfaces can show its own subject · `cb09422` every command an example
prints is one that runs · `f249844` examples keep their assertions ·
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
between issues of a document nobody here can read. Transcribing it would be
guessing which revision. Not the microstrip's position, where the form is
unambiguous and only a third-party cross-check was missing.

**A third-party reference value for the impedance forms.** Four attempts
failed, including KiCad's own calculator carrying an open issue saying it may
be more than 20% out. Both anchors in the tests were worked out **on paper
before the code was run**, which is what makes them evidence.

## Next step

`docs/TRACKER.md`, V2: **re-run the neighbourhood probe now that it has
moved.** Adding a thirteenth variant changes what "the variant this board
picks" means, so `plane_board`'s new winner has a neighbourhood nobody has
looked at, and the five local optima were confirmed against a twelve-point list.
One command, 77 seconds:

    cargo test --release -p cypcb-autoroute \
      --test is_the_best_variant_a_local_optimum -- --ignored --nocapture

After that, the largest live item is **per-segment trace width** - what
neck-down level 3 needs, and what would let the neck rule measure the necked
stretch rather than only the coherence of the declaration.

## Report style

Polish: result, next step, blocker. Details to the tracker, not the chat.
