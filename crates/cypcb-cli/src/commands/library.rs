//! The footprints a person already has, indexed and searched.
//!
//! `cypcb-library` was 3751 lines and 41 tests that nothing called: a SQLite
//! schema, a search over it, and an importer that reads the `.pretty` folders
//! and `.kicad_mod` files every KiCad user has on disk. The decision to keep
//! it named the one thing missing - a path between the crate and a person -
//! and this is that path.
//!
//! Two verbs, because a library is only useful when both exist: put footprints
//! in, and find one again.
//!
//! The database is a file like any other. It defaults to `cypcb-library.db` in
//! the working directory rather than to somewhere under a home directory: a
//! tool that writes outside the directory it was run in surprises the person
//! who ran it, and a project that keeps its parts beside its boards is the
//! ordinary case here.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use cypcb_library::{LibraryManager, SearchFilters};
use miette::{IntoDiagnostic, Result, WrapErr};

/// Index and search the footprint libraries on this machine.
#[derive(Args)]
pub struct LibraryCommand {
    /// Where the index lives (default: cypcb-library.db in this directory)
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    action: LibraryAction,
}

#[derive(Subcommand)]
enum LibraryAction {
    /// Read every `.pretty` folder under a directory into the index
    ///
    /// A KiCad footprint library is a folder called `<name>.pretty` full of
    /// `.kicad_mod` files. This walks the directory it is given, imports each
    /// library it finds, and says how many footprints each one brought.
    Import(ImportArgs),
    /// Find a footprint by name, description, package or manufacturer
    Search(SearchArgs),
    /// List what has been imported
    List,
}

#[derive(Args)]
struct ImportArgs {
    /// The directory to walk. Every `<name>.pretty` folder under it is read.
    directory: PathBuf,
}

#[derive(Args)]
struct SearchArgs {
    /// What to look for
    query: String,

    /// How many results to print
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

impl LibraryCommand {
    pub fn run(&self) -> Result<()> {
        let path = self
            .db
            .clone()
            .unwrap_or_else(|| PathBuf::from("cypcb-library.db"));
        let mut manager = LibraryManager::new(&path)
            .into_diagnostic()
            .wrap_err_with(|| format!("opening the index at {}", path.display()))?;

        match &self.action {
            LibraryAction::Import(args) => {
                let directory = args
                    .directory
                    .canonicalize()
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        format!("reading the directory {}", args.directory.display())
                    })?;
                // The importer resolves a library by name against its search
                // paths, so the directory being imported has to be one of them.
                manager.add_kicad_search_path(directory.clone());

                let imported = manager
                    .auto_import_folder(&directory)
                    .into_diagnostic()
                    .wrap_err("importing the libraries in that directory")?;

                if imported.is_empty() {
                    println!(
                        "No .pretty folder under {}: a KiCad footprint library is a folder \
                         called <name>.pretty holding .kicad_mod files.",
                        directory.display()
                    );
                    return Ok(());
                }

                let libraries = manager.list_libraries().into_diagnostic()?;
                let mut total = 0usize;
                for name in &imported {
                    let count = libraries
                        .iter()
                        .find(|library| &library.name == name)
                        .map(|library| library.component_count)
                        .unwrap_or(0);
                    total += count;
                    println!("{name}: {count} footprint(s)");
                }
                println!(
                    "Indexed {total} footprint(s) from {} librar{} into {}",
                    imported.len(),
                    if imported.len() == 1 { "y" } else { "ies" },
                    path.display()
                );
            }

            LibraryAction::Search(args) => {
                let filters = SearchFilters {
                    limit: args.limit,
                    ..SearchFilters::default()
                };
                let results = manager
                    .search(&args.query, &filters)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("searching for {:?}", args.query))?;

                if results.is_empty() {
                    println!("Nothing in {} matches {:?}", path.display(), args.query);
                    return Ok(());
                }

                for result in &results {
                    let component = &result.component;
                    let description = component.metadata.description.as_deref().unwrap_or("");
                    println!(
                        "{}  [{}]{}{}",
                        component.id.name,
                        component.library,
                        if description.is_empty() { "" } else { "  " },
                        description
                    );
                }
                println!("{} result(s)", results.len());
            }

            LibraryAction::List => {
                let libraries = manager.list_libraries().into_diagnostic()?;
                if libraries.is_empty() {
                    println!(
                        "{} holds nothing yet. `cypcb library import <directory>` fills it.",
                        path.display()
                    );
                    return Ok(());
                }
                for library in &libraries {
                    println!(
                        "{} ({})  {} footprint(s)",
                        library.name, library.source, library.component_count
                    );
                }
            }
        }

        Ok(())
    }
}
