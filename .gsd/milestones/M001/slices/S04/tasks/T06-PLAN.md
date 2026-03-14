# T06: CLI Export Command

**Slice:** S04 — **Milestone:** M001

## Description

Implement CLI export command with manufacturer presets and organized file output.

Purpose: The CLI is the primary interface for headless export (requirement DEV-01). Users run `cypcb export project.cypcb` to generate all manufacturing files in a single command.

Output: CLI export subcommand that generates complete manufacturing file set.

## Must-Haves

- [ ] "CLI export command creates all manufacturing files"
- [ ] "Export creates organized output folder structure"
- [ ] "JLCPCB preset generates correct file set"
- [ ] "Export can run headless without GUI"

## Files

- `crates/cypcb-export/src/job.rs`
- `crates/cypcb-export/src/presets.rs`
- `crates/cypcb-export/src/lib.rs`
- `crates/cypcb-cli/src/commands/export.rs`
- `crates/cypcb-cli/src/commands/mod.rs`
- `crates/cypcb-cli/src/main.rs`
