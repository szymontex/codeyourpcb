//! The IPC tables do not cite a clause they cannot show.
//!
//! `cargo test -p cypcb-rules --test the_ipc_tables_do_not_cite_a_clause_they_cannot_show`
//!
//! `RulesPreset::provenance` has told the **user** the honest version since it
//! was written: `ipc_class2 is a design standard rather than a fabricator.
//! These figures are this tool's reading of IPC, which is not a public
//! document.` The source file told the **developer** something stronger -
//! `Values based on IPC-2221B Tables 6-1, 6-2` and a `Source:` line per class -
//! and a table number is a citation. A citation nobody here can produce is
//! worse than none, because it tells the next reader the figure was checked.
//!
//! This is the same rule the fab tables follow, arrived at from the other
//! direction: those say which page a number came off, and where no page states
//! one the comment says `UNSOURCED`. IPC has no page to name, so the file says
//! that instead of naming a table.

use std::path::{Path, PathBuf};

fn ipc_source() -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/presets/ipc.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn no_table_number_is_cited() {
    let source = ipc_source();
    let cited: Vec<&str> = source
        .lines()
        .filter(|line| line.trim_start().starts_with("//"))
        .filter(|line| {
            let lowered = line.to_lowercase();
            lowered.contains("table 6-")
                || lowered.contains("tables 6-")
                || lowered.contains("clause ")
        })
        .collect();
    assert!(
        cited.is_empty(),
        "a table this project cannot open is cited as though it had been read: {cited:#?}"
    );
}

#[test]
fn no_figure_claims_a_standard_as_its_source() {
    // `Source: IPC-2221B Class 1 requirements` on a function reads as "these
    // numbers came out of the standard". They did not: the class structure is
    // public and the figures are this project's.
    let source = ipc_source();
    let claimed: Vec<&str> = source
        .lines()
        .filter(|line| line.trim_start().starts_with("//"))
        .filter(|line| line.contains("Source:"))
        .collect();
    assert!(
        claimed.is_empty(),
        "a `Source:` line in this file claims a document nobody here has read: {claimed:#?}"
    );
}

#[test]
fn the_file_says_out_loud_that_the_figures_are_this_projects() {
    // The other half: deleting the overclaim is only right if what replaces it
    // states the position. A file with no provenance at all is the same gap in
    // a quieter form.
    let source = ipc_source();
    let lowered = source.to_lowercase();
    assert!(
        lowered.contains("not a public document"),
        "the file has to say IPC is not something a reader can open"
    );
    assert!(
        lowered.contains("this project's own table")
            || lowered.contains("the figures are this project's"),
        "and that the numbers are this project's rather than the standard's"
    );
}

#[test]
fn the_user_facing_caveat_still_agrees_with_the_file() {
    // The two have to keep saying the same thing, which is the whole reason
    // this was a defect rather than a wording preference.
    use cypcb_rules::presets::RulesPreset;
    for preset in [
        RulesPreset::IpcClass1,
        RulesPreset::IpcClass2,
        RulesPreset::IpcClass3,
    ] {
        let caveat = preset
            .provenance()
            .caveat(preset.name())
            .unwrap_or_else(|| panic!("{preset:?} has no caveat"));
        assert!(
            caveat.contains("not a public document"),
            "{preset:?}: {caveat}"
        );
    }
}
