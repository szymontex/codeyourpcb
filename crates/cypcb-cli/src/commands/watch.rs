//! Re-check a design every time it is saved.

use std::path::PathBuf;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_drc::{run_drc, Preset, PresetRules};
use cypcb_watcher::{FileWatcher, WatchEvent};

/// Check a design, then check it again every time it changes.
///
/// The browser has had hot reload since the dev server was written; a terminal
/// had nothing, so checking a board meant running `cypcb check` by hand after
/// every save. `cypcb-watcher` was written for exactly this and had no caller
/// at all - 184 lines and three passing tests that nothing in the workspace
/// used.
#[derive(Args)]
pub struct WatchCommand {
    /// Input .cypcb file
    file: PathBuf,

    /// Manufacturer preset for design rules
    #[arg(short, long, default_value = "jlcpcb")]
    preset: String,
}

impl WatchCommand {
    pub fn run(self) -> Result<()> {
        let preset = Preset::from_name(&self.preset).ok_or_else(|| {
            let available: Vec<&str> = Preset::all().iter().map(|p| p.name()).collect();
            miette::miette!(
                "Unknown preset '{}'. Available presets: {}",
                self.preset,
                available.join(", ")
            )
        })?;

        // The watcher takes a directory: an editor writes a save as a rename
        // over the file, which a watch on the file itself stops seeing after
        // the first one.
        let directory = self
            .file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let watched = std::fs::canonicalize(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        println!("Watching {} - press Ctrl+C to stop", self.file.display());
        self.check_once(&preset);

        let watcher = FileWatcher::new(&directory)
            .map_err(|e| miette::miette!("{e}"))
            .wrap_err_with(|| format!("Failed to watch {}", directory.display()))?;

        // What each file held when it was last checked. The debouncer forwards
        // a save as a stream of notifications - measured at one every 200ms
        // for as long as the command ran, 24 checks for a single edit - and a
        // file whose bytes have not moved is not a save whatever the operating
        // system says about it.
        let mut last_seen: std::collections::HashMap<PathBuf, Vec<u8>> =
            std::collections::HashMap::new();
        if let Ok(bytes) = std::fs::read(&watched) {
            last_seen.insert(watched.clone(), bytes);
        }

        loop {
            match watcher.recv() {
                Ok(WatchEvent::Modified(path)) => {
                    // The directory is watched, so filter to the design and
                    // whatever it imports - a library changing is a reason to
                    // check the board that imports it.
                    let changed =
                        std::fs::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path));
                    if changed != watched && !self.imports(&changed) {
                        continue;
                    }
                    let Ok(bytes) = std::fs::read(&changed) else {
                        continue;
                    };
                    if last_seen.get(&changed) == Some(&bytes) {
                        continue;
                    }
                    last_seen.insert(changed.clone(), bytes);

                    println!();
                    println!("--- {} changed", changed.display());
                    self.check_once(&preset);
                }
                Ok(WatchEvent::Error(err)) => eprintln!("Watch error: {err}"),
                Err(_) => break,
            }
        }

        Ok(())
    }

    /// Whether the design imports this file, so a library edit re-checks it.
    fn imports(&self, changed: &std::path::Path) -> bool {
        let Ok(source) = std::fs::read_to_string(&self.file) else {
            return false;
        };
        let parsed = cypcb_parser::parse(&source);
        let base = self.file.parent().unwrap_or(std::path::Path::new("."));

        parsed.value.definitions.iter().any(|def| match def {
            cypcb_parser::ast::Definition::Import(import) => {
                std::fs::canonicalize(base.join(&import.path.value))
                    .map(|path| path == changed)
                    .unwrap_or(false)
            }
            _ => false,
        })
    }

    /// The board as it is on disk right now, however it is written.
    fn board(&self) -> std::result::Result<cypcb_world::BoardWorld, String> {
        if crate::board_source::is_kicad(&self.file) {
            return crate::board_source::load_kicad(&self.file)
                .map(|loaded| loaded.world)
                .map_err(|err| format!("{err:?}"));
        }

        let source = std::fs::read_to_string(&self.file)
            .map_err(|err| format!("Failed to read {}: {err}", self.file.display()))?;

        let mut parsed = cypcb_parser::parse(&source);
        if !parsed.errors.is_empty() {
            let first = parsed.errors.remove(0);
            return Err(format!("{:?}", miette::Report::new(first)));
        }

        let mut import_errors = Vec::new();
        let ast = cypcb_parser::resolve_imports(&parsed.value, &self.file, &mut import_errors);
        for error in &import_errors {
            eprintln!("Import error: {error}");
        }

        let mut world = cypcb_world::BoardWorld::new();
        let mut library = cypcb_world::footprint::FootprintLibrary::new();
        let mut sync = cypcb_world::sync_ast_to_world(&ast, &source, &mut world, &mut library);
        if !sync.errors.is_empty() {
            let first = sync.errors.remove(0);
            return Err(format!("{:?}", miette::Report::new(first)));
        }

        Ok(world)
    }

    /// One pass: read, check, and say what came back.
    ///
    /// Never exits the process - a watch that dies on the first bad save is a
    /// watch nobody leaves running.
    fn check_once(&self, preset: &Preset) {
        let mut world = match self.board() {
            Ok(world) => world,
            Err(message) => {
                eprintln!("{message}");
                return;
            }
        };
        let drc = run_drc(&mut world, &preset.rules());
        if drc.violations.is_empty() {
            println!(
                "OK: {} passed DRC against {} in {}ms",
                self.file.display(),
                preset.name(),
                drc.duration_ms
            );
            return;
        }

        println!(
            "{} DRC violation(s) against {}:",
            drc.violations.len(),
            preset.name()
        );
        for violation in drc.violations.iter().take(20) {
            println!(
                "  {} at ({:.3}mm, {:.3}mm): {}",
                violation.kind,
                violation.location.x.to_mm(),
                violation.location.y.to_mm(),
                violation.message
            );
        }
        if drc.violations.len() > 20 {
            println!("  ... and {} more", drc.violations.len() - 20);
        }
    }
}
