//! Where a board comes from.
//!
//! The importer has been fixed five times - footprint libraries keyed by
//! geometry, malformed coordinates refused, `np_thru_hole` understood, copper
//! pours carried - and every one of those fixes served tests and benchmarks
//! only. `cypcb check board.kicad_pcb` handed the file to the DSL parser and
//! printed:
//!
//! ```text
//! cypcb::parse::missing
//!   × Missing a definition
//!    ╭─[1:1]
//!  1 │ (kicad_pcb (version 20240108) (generator "pcbnew") ...
//! ```
//!
//! So the product could not open a KiCad board at all. This is the one place
//! that decides which reader a file goes to, and every command that loads a
//! board goes through it.

use std::path::Path;

use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A board, however it was written.
pub struct LoadedBoard {
    pub world: BoardWorld,
    pub library: FootprintLibrary,
    /// The file's own text, for pointing diagnostics at lines of it.
    pub source: String,
}

/// Whether this path is a KiCad board rather than a `.cypcb` design.
pub fn is_kicad(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("kicad_pcb"))
}

/// Read a KiCad board into the same shape the DSL path produces.
///
/// Anything the importer would not carry is printed rather than dropped: a
/// board that arrives without its ground plane and says nothing is a board
/// whose Gerber ships without a ground plane.
pub fn load_kicad(path: &Path) -> Result<LoadedBoard> {
    let source = std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", path.display()))?;

    let parsed = cypcb_kicad::parse_kicad_pcb(path)
        .map_err(|e| miette::miette!("{e}"))
        .wrap_err_with(|| format!("Failed to read KiCad board {}", path.display()))?;

    for refusal in &parsed.metadata.zone_refusals {
        eprintln!("warning: {refusal}");
    }

    let mut world = parsed.world;
    let library = parsed.library;
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);

    // The copper the file already carries. Without this a board that arrives
    // routed reads as unrouted: every trace in the file would be dropped on
    // the floor and the checker would report every pin as unreached.
    if let Some(routes) = parsed.reference_routes {
        cypcb_router::apply_routes(&mut world, &routes);
        world.rebuild_spatial_index_from_library(&library);
    }

    Ok(LoadedBoard {
        world,
        library,
        source,
    })
}
