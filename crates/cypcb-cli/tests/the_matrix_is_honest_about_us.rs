//! What the feature matrix says about this project is measured, not remembered.
//!
//! `cargo test -p cypcb-cli --test the_matrix_is_honest_about_us`
//!
//! `docs/competition-feature-matrix.md` kept its March answers about its own
//! column into August: modules and typed interfaces read `v2 in progress`,
//! constraint assertions the same, copper pours `❌`, and physical units
//! `❌ Strings only` - each of them a construct the language had shipped and
//! an example in this repository uses. A comparison against nine other tools
//! is worth nothing if the column it can check is the one that is wrong.
//!
//! Two halves, because either alone is empty. The cell has to claim the
//! feature, **and** the construct behind it has to be one the reader
//! understands - proved by misspelling the keyword in a copy of the example
//! and watching the same file stop parsing.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A row of the matrix, the keyword it is about, and the example that uses it.
const SHIPPED: &[(&str, &str, &str)] = &[
    ("Module / hierarchy system", "module", "v2-modules.cypcb"),
    (
        "Typed interfaces (I2C, SPI)",
        "interface",
        "v2-interfaces.cypcb",
    ),
    ("Constraint assertions", "assert", "v2-constraints.cypcb"),
    ("Copper pour / zones", "zone", "pour-island.cypcb"),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn matrix() -> String {
    std::fs::read_to_string(repo_root().join("docs/competition-feature-matrix.md"))
        .expect("the matrix is there")
}

/// The CodeYourPCB cell of the row with this feature name.
fn our_cell(matrix: &str, feature: &str) -> String {
    let mut found: Option<String> = None;
    for line in matrix.lines() {
        if !line.starts_with('|') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() > 3 && fields[1].trim() == feature {
            assert!(found.is_none(), "two rows are named `{feature}`");
            found = Some(fields[2].trim().to_string());
        }
    }
    found.unwrap_or_else(|| panic!("no row of the matrix is named `{feature}`"))
}

/// One file, in a directory of this test's own: cargo runs the tests here at
/// the same time and a shared directory means one wiping what another reads.
fn scratch(who: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-matrix-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the fixture is writable");
    board
}

fn check(board: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(board)
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

/// Replace a keyword wherever it stands alone, so the copy names something
/// the reader has never heard of.
fn misspell(source: &str, keyword: &str) -> String {
    source
        .split_inclusive(|c: char| !(c.is_alphanumeric() || c == '_'))
        .map(|piece| {
            let (word, tail) = piece.split_at(
                piece
                    .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .unwrap_or(piece.len()),
            );
            if word == keyword {
                format!("xq{word}{tail}")
            } else {
                piece.to_string()
            }
        })
        .collect()
}

#[test]
fn a_construct_the_language_has_is_not_written_up_as_missing() {
    let matrix = matrix();
    for (feature, keyword, example) in SHIPPED {
        let cell = our_cell(&matrix, feature);
        assert!(
            cell.starts_with('✅'),
            "`{feature}` reads `{cell}` while `{keyword}` is in the language and \
             `examples/{example}` uses it"
        );

        let source = std::fs::read_to_string(repo_root().join("examples").join(example))
            .expect("the example is there");
        let clean = check(&scratch(keyword, &source));
        assert!(
            !clean.contains("cypcb::parse"),
            "`examples/{example}` does not read:\n{clean}"
        );

        // Without this half the assertion above passes on any file the reader
        // happens to accept, whatever it does or does not say.
        let broken = check(&scratch(
            &format!("{keyword}-misspelt"),
            &misspell(&source, keyword),
        ));
        assert!(
            broken.contains("cypcb::parse"),
            "`{keyword}` misspelt changes nothing in `examples/{example}`, so the \
             reading above proves nothing about it:\n{broken}"
        );
    }
}

/// A board whose trace is three thousandths of an inch wide.
///
/// JLCPCB etches 0.127mm, so 3mil (0.0762mm) is a violation and 3mm is not.
/// The pair is what says the unit is read rather than dropped.
const IN_MILS: &str = r#"version 1

board mils {
    size 30mm x 30mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 10mm
}

net A {
    R1.1
    R2.1
}

trace A {
    from R1.1
    to R2.1
    layer Top
    width 3mil
}
"#;

#[test]
fn a_number_carries_its_unit() {
    let cell = our_cell(&matrix(), "Physical units in DSL");
    assert!(
        cell.starts_with('✅'),
        "`Physical units in DSL` reads `{cell}` while the reader takes mm, mil and oz"
    );

    let said = check(&scratch("mils", IN_MILS));
    assert!(
        said.contains("0.076mm actual"),
        "3mil is 0.076mm and the checker should say so:\n{said}"
    );

    // The same figure with the unit changed. If `mil` were read as `mm` - or
    // dropped - the two boards would report the same thing.
    let wide = check(&scratch("mm", &IN_MILS.replace("width 3mil", "width 3mm")));
    assert!(
        !wide.contains("trace-width"),
        "3mm clears the fab floor, so the unit is what the case above reports:\n{wide}"
    );
}

/// A capability the matrix names, and the line in the server that provides it.
///
/// The needle carries `: Some(` so a commented-out plan - `// -
/// references_provider` sits in that file today - does not read as a feature.
const CAPABILITIES: &[(&str, &str)] = &[
    ("hover", "hover_provider: Some("),
    ("completion", "completion_provider: Some("),
    ("go-to-definition", "definition_provider: Some("),
    ("references", "references_provider: Some("),
    ("rename", "rename_provider: Some("),
    ("formatting", "document_formatting_provider: Some("),
    ("semantic tokens", "semantic_tokens_provider: Some("),
    ("diagnostics", "publish_diagnostics"),
];

/// The language server row says what the server answers, not how it feels.
///
/// That cell read `Full LSP` from March until 2026-08-26, beside a server
/// advertising four things and a source file whose own comment lists
/// `references_provider` as something to add later. "Full" is not a claim
/// anybody can check; a list is.
#[test]
fn the_language_server_row_lists_what_the_server_advertises() {
    let cell = our_cell(&matrix(), "LSP / IDE support");
    let backend = std::fs::read_to_string(repo_root().join("crates/cypcb-lsp/src/backend.rs"))
        .expect("the server is there");

    let said = cell.to_lowercase();
    for (capability, needle) in CAPABILITIES {
        let claimed = said.contains(capability);
        let provided = backend.contains(needle);
        assert_eq!(
            claimed, provided,
            "the matrix says `{cell}`; `{capability}` is claimed: {claimed}, provided: {provided}"
        );
    }
}

/// A verb the API row names, and the method that provides it.
///
/// Two of them are things this project does not do, and they are here so the
/// test fails when somebody writes them into the row rather than into the
/// engine.
const ENGINE: &[(&str, &str)] = &[
    ("load", "pub fn load_source"),
    ("drc", "pub fn get_violations_json"),
    ("route", "pub fn auto_route"),
    ("export", "pub fn export_traces_as_dsl"),
    ("spice", "pub fn spice"),
    ("schematic", "pub fn schematic"),
];

/// The headless API row names what the engine has, not what a browser is.
///
/// It read `WASM API` from March, which says where the code runs and nothing
/// about what a program can ask it. `PcbEngine` carries thirty
/// `#[wasm_bindgen]` methods; the four the row names are the four a headless
/// caller needs, and this fails if one of them goes or if a fifth is claimed.
#[test]
fn the_headless_api_row_names_methods_the_engine_has() {
    let cell = our_cell(&matrix(), "API / headless mode");
    let engine = std::fs::read_to_string(repo_root().join("crates/cypcb-render/src/lib.rs"))
        .expect("the engine is there");

    let said = cell.to_lowercase();
    for (verb, needle) in ENGINE {
        let claimed = said.contains(verb);
        let provided = engine.contains(needle);
        assert_eq!(
            claimed, provided,
            "the matrix says `{cell}`; `{verb}` is claimed: {claimed}, provided: {provided}"
        );
    }
}

/// What the 3D view can draw, and the loader that draws it.
const THREE_D: &[(&str, &str)] = &[
    ("generic", "BoxGeometry"),
    ("gltf", "GLTFLoader"),
    ("step", "STEPLoader"),
];

/// The 3D rows say what the renderer loads and who can assign a model.
///
/// `Component 3D models` read `Generic bodies` while `renderer3d.ts` has
/// carried a GLTF loader and an auto-load pass for components with a model
/// since before this file was written, and `3D model assignment` read a flat
/// no beside an engine method that does exactly that. Both were half right,
/// which is the hardest kind of stale to notice.
#[test]
fn the_three_d_rows_say_what_the_renderer_and_the_engine_do() {
    let matrix = matrix();
    let renderer = std::fs::read_to_string(repo_root().join("viewer/src/renderer3d.ts"))
        .expect("the 3D renderer is there");

    let bodies = our_cell(&matrix, "Component 3D models").to_lowercase();
    for (what, needle) in THREE_D {
        assert_eq!(
            bodies.contains(what),
            renderer.contains(needle),
            "`Component 3D models` reads `{bodies}` and the renderer's `{needle}` says otherwise"
        );
    }

    // Two paths, and the row has to name the one that runs. A part placed from
    // the JLCPCB panel fetches its EasyEDA model directly; the engine's own
    // `register_3d_model` is on a chain nothing completes, so a tick here
    // would claim the dead one.
    //
    // The first draft of this case searched for `register_3d_model` alone and
    // passed while the row said `no caller yet` - the viewer's wrapper is
    // spelled `register3DModel` and the live path does not use either.
    let assignment = our_cell(&matrix, "3D model assignment");
    let viewer = walk(&repo_root().join("viewer/src"));
    let through_the_engine = viewer
        .iter()
        .any(|source| source.contains("register_3d_model"));
    assert_eq!(
        assignment.starts_with('✅'),
        through_the_engine,
        "`3D model assignment` reads `{assignment}` and the engine path in viewer/src: \
         {through_the_engine}"
    );

    let from_the_panel = viewer
        .iter()
        .any(|source| source.contains("fetch3DModel(component.lcsc)"));
    assert_eq!(
        assignment.to_lowercase().contains("jlcpcb"),
        from_the_panel,
        "`3D model assignment` reads `{assignment}` and the panel path: {from_the_panel}"
    );
}

/// Every `.ts` file under a directory, read.
fn walk(dir: &Path) -> Vec<String> {
    let mut sources = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "ts") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    sources.push(text);
                }
            }
        }
    }
    assert!(!sources.is_empty(), "no TypeScript under {}", dir.display());
    sources
}
