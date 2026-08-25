//! What the Gerber set cannot carry is said at the point of export.
//!
//! `cargo test -p cypcb-cli --test the_export_names_what_the_files_cannot_carry`
//!
//! `to-kicad` names the drill spans, the fabricator and the net constraints it
//! drops. `export` said nothing, and it drops one thing: a **stiffener**.
//!
//! That is the right call about the file. The Gerber job file's material
//! stackup is specified as the layers of the bare board and only those, and a
//! stiffener is bonded on after the stack is pressed - `material_type` returns
//! nothing for it on purpose, next to solder paste, which is deposited at
//! assembly. It is the wrong thing to do in silence: a design that states one
//! is asking for a board nobody can make from this set of files alone.
//!
//! `examples/rigid-flex.cypcb` states `stiffener 0.2mm material "FR4"` under
//! the rigid end that carries the connector.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn export(who: &str, example: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-export-says-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args([
            "export",
            example,
            "-o",
            dir.to_str().expect("a path that is text"),
        ])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "`cypcb export {example}` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn a_stated_stiffener_is_named_with_its_thickness_and_material() {
    let said = export("flex", "examples/rigid-flex.cypcb");

    assert!(
        said.contains("the stiffener this design states (0.200mm of FR4)"),
        "the design states a stiffener and the export has to name it:\n{said}"
    );
    assert!(
        said.contains("bonded on after it is built"),
        "and say why it is not in the files, which is the half that stops \
         somebody trying to find it in them:\n{said}"
    );
}

#[test]
fn a_board_with_no_stiffener_is_told_nothing_about_one() {
    // The half that keeps the other from being noise. `examples/four-layer.cypcb`
    // states a full stackup and no stiffener.
    let said = export("rigid", "examples/four-layer.cypcb");
    assert!(
        !said.contains("stiffener"),
        "nothing was bonded to this board, so nothing is owed about one:\n{said}"
    );
}
