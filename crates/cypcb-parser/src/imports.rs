//! Resolving `import` statements.
//!
//! A module library is only worth writing if another file can use it, and
//! until now `import "lib/dividers.cypcb"` parsed and nothing read it.
//!
//! # Where a path points
//!
//! Relative to the file doing the importing, the way every other language
//! resolves a path written in a source file. There is no project root to
//! configure and no search path to get wrong: a file names its neighbours the
//! way it sees them.
//!
//! # What comes across
//!
//! Definitions worth reusing - modules, footprints and interfaces. Not a
//! board, not components, not nets: importing a file must not place parts on
//! the importing design, or `import` would be textual inclusion under another
//! name.
//!
//! `import "x.cypcb"` takes everything reusable; `import A, B from "x.cypcb"`
//! takes only what it names, and says so when a name is not there.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::ast::{Definition, SourceFile};

/// Where the text behind an `import` comes from.
///
/// The command line and the language server read the disk. A browser tab has
/// no disk: the viewer loads a design that imports a block library and the
/// engine cannot open `lib/blocks.cypcb`, so the board arrives with every
/// module missing. Both need the same resolution - relative paths, cycles,
/// selective imports, a library built from libraries - and only differ in
/// where the bytes come from, so that is the one thing this abstracts.
pub trait ImportSource {
    /// The text at this path, or a sentence saying why it could not be had.
    fn read(&self, path: &Path) -> Result<String, String>;
}

/// Reads the filesystem. What the CLI and the language server use.
pub struct FromDisk;

impl ImportSource for FromDisk {
    fn read(&self, path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|reason| reason.to_string())
    }
}

/// Reads a set of files the host already has.
///
/// The keys are paths as an import writes them, normalised the same way the
/// resolver normalises: `lib/blocks.cypcb`, not an absolute path, because the
/// host that supplies them - a browser - has no notion of where the design
/// lives on a disk.
pub struct FromMemory {
    files: HashMap<String, String>,
}

impl FromMemory {
    /// Build a source from path-to-text pairs.
    pub fn new(files: HashMap<String, String>) -> Self {
        FromMemory { files }
    }
}

impl ImportSource for FromMemory {
    fn read(&self, path: &Path) -> Result<String, String> {
        let key = path.to_string_lossy();
        self.files.get(key.as_ref()).cloned().ok_or_else(|| {
            let mut known: Vec<&str> = self.files.keys().map(String::as_str).collect();
            known.sort_unstable();
            if known.is_empty() {
                "the host supplied no files".to_string()
            } else {
                format!("the host supplied: {}", known.join(", "))
            }
        })
    }
}

/// Something that stopped an import from being resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportError {
    /// The file named does not exist, or could not be read.
    Unreadable {
        /// Path as written in the source.
        path: String,
        /// Where it was resolved to.
        resolved: PathBuf,
        /// Why the read failed.
        reason: String,
    },
    /// The imported file does not parse.
    Unparsable {
        /// Path as written in the source.
        path: String,
        /// First parse error from that file.
        reason: String,
    },
    /// A file imports itself, directly or through others.
    Cycle {
        /// The chain of files, from the first to the repeat.
        chain: String,
    },
    /// A selective import names something the file does not define.
    NotFound {
        /// The name that was asked for.
        name: String,
        /// Path as written in the source.
        path: String,
    },
}

impl ImportError {
    /// The path as the source wrote it.
    ///
    /// What an editor needs to underline the `import` statement the error came
    /// from rather than the first character of the file.
    pub fn path(&self) -> &str {
        match self {
            ImportError::Unreadable { path, .. } => path,
            ImportError::Unparsable { path, .. } => path,
            ImportError::NotFound { path, .. } => path,
            ImportError::Cycle { chain } => chain,
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Unreadable {
                path,
                resolved,
                reason,
            } => write!(
                f,
                "cannot read import '{path}' (looked in {}): {reason}",
                resolved.display()
            ),
            ImportError::Unparsable { path, reason } => {
                write!(f, "import '{path}' does not parse: {reason}")
            }
            ImportError::Cycle { chain } => write!(f, "import cycle: {chain}"),
            ImportError::NotFound { name, path } => {
                write!(f, "'{path}' does not define '{name}'")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Resolve every `import` in `file`, returning it with the imported
/// definitions in place of the import statements.
///
/// `origin` is the path of the file being resolved; its directory is what
/// relative imports are resolved against. Pass the path even for a file read
/// from stdin - the directory is what matters.
///
/// Errors are collected rather than returned: a file with one bad import
/// still yields everything else, which is what an editor needs.
pub fn resolve_imports(
    file: &SourceFile,
    origin: &Path,
    errors: &mut Vec<ImportError>,
) -> SourceFile {
    resolve_imports_from(file, origin, &FromDisk, errors)
}

/// The same resolution, against text the caller supplies.
///
/// For a host with no filesystem. `origin` is still the path of the file being
/// resolved - it is what relative imports are joined to - and for a design
/// that lives nowhere, a bare file name is the honest answer: `lib/x.cypcb`
/// then resolves to `lib/x.cypcb`.
pub fn resolve_imports_from(
    file: &SourceFile,
    origin: &Path,
    source: &dyn ImportSource,
    errors: &mut Vec<ImportError>,
) -> SourceFile {
    let mut visiting = Vec::new();
    resolve_into(file, origin, source, &mut visiting, errors)
}

fn resolve_into(
    file: &SourceFile,
    origin: &Path,
    source: &dyn ImportSource,
    visiting: &mut Vec<PathBuf>,
    errors: &mut Vec<ImportError>,
) -> SourceFile {
    let base = origin.parent().unwrap_or_else(|| Path::new("."));
    let mut definitions = Vec::with_capacity(file.definitions.len());

    for definition in &file.definitions {
        let Definition::Import(import) = definition else {
            definitions.push(definition.clone());
            continue;
        };

        let written = import.path.value.clone();
        let resolved = normalise(&base.join(&written));

        if visiting.iter().any(|seen| seen == &resolved) {
            let mut chain: Vec<String> = visiting.iter().map(|p| p.display().to_string()).collect();
            chain.push(resolved.display().to_string());
            errors.push(ImportError::Cycle {
                chain: chain.join(" -> "),
            });
            continue;
        }

        let text = match source.read(&resolved) {
            Ok(text) => text,
            Err(reason) => {
                errors.push(ImportError::Unreadable {
                    path: written,
                    resolved,
                    reason,
                });
                continue;
            }
        };

        let parsed = crate::parse(&text);
        if let Some(first) = parsed.errors.first() {
            errors.push(ImportError::Unparsable {
                path: written,
                reason: first.to_string(),
            });
            continue;
        }

        // Resolve the imported file's own imports before taking anything from
        // it, so a library may be built from libraries.
        visiting.push(resolved.clone());
        let inner = resolve_into(&parsed.value, &resolved, source, visiting, errors);
        visiting.pop();

        let wanted: Option<HashSet<&str>> = if import.names.is_empty() {
            None
        } else {
            Some(import.names.iter().map(|n| n.value.as_str()).collect())
        };

        let mut taken: HashSet<String> = HashSet::new();
        for imported in &inner.definitions {
            let Some(name) = reusable_name(imported) else {
                continue;
            };
            if wanted.as_ref().is_some_and(|names| !names.contains(name)) {
                continue;
            }
            taken.insert(name.to_string());
            definitions.push(imported.clone());
        }

        // What the taken definitions need to work.
        //
        // `import Divider from "lib/blocks.cypcb"` used to take the module and
        // nothing else, so a block whose parts use a footprint the same file
        // declares arrived without it: `unknown footprint: 'TINY'` on a design
        // that named no footprint at all. A library that cannot be used
        // without knowing what is inside it is not a library, so a selective
        // import takes the footprints, interfaces and modules that what it
        // named depends on - and says nothing about them, because the design
        // never asked for them by name.
        if wanted.is_some() {
            let mut needed: Vec<String> = Vec::new();
            let mut frontier: Vec<String> = taken.iter().cloned().collect();
            while let Some(name) = frontier.pop() {
                let Some(definition) = inner
                    .definitions
                    .iter()
                    .find(|d| reusable_name(d) == Some(name.as_str()))
                else {
                    continue;
                };
                for dependency in depends_on(definition) {
                    if taken.contains(&dependency) || needed.contains(&dependency) {
                        continue;
                    }
                    needed.push(dependency.clone());
                    frontier.push(dependency);
                }
            }

            for name in needed {
                if let Some(definition) = inner
                    .definitions
                    .iter()
                    .find(|d| reusable_name(d) == Some(name.as_str()))
                {
                    definitions.push(definition.clone());
                }
            }
        }

        if let Some(names) = wanted {
            for name in names {
                if !taken.contains(name) {
                    errors.push(ImportError::NotFound {
                        name: name.to_string(),
                        path: import.path.value.clone(),
                    });
                }
            }
        }
    }

    SourceFile {
        version: file.version,
        definitions,
        span: file.span,
    }
}

/// The name a definition is known by, if it is one an import can carry.
///
/// A board, a component, a net, a trace or a zone belongs to the design that
/// wrote it. Only the reusable things cross a file boundary.
fn reusable_name(definition: &Definition) -> Option<&str> {
    match definition {
        Definition::Module(module) => Some(module.name.value.as_str()),
        Definition::Footprint(footprint) => Some(footprint.name.value.as_str()),
        Definition::Interface(interface) => Some(interface.name.value.as_str()),
        _ => None,
    }
}

/// The names a definition cannot work without.
///
/// A module needs the footprints its parts name, the interfaces it claims and
/// the modules it instantiates. Anything else - a net, a value - is written
/// out in full and needs nothing from the file it came from.
fn depends_on(definition: &Definition) -> Vec<String> {
    let Definition::Module(module) = definition else {
        return Vec::new();
    };

    let mut names = Vec::new();
    for claim in &module.implements {
        names.push(claim.interface.value.clone());
    }
    for inner in &module.definitions {
        match inner {
            Definition::Component(component) => names.push(component.footprint.value.clone()),
            Definition::ModuleInstance(instance) => names.push(instance.module.value.clone()),
            _ => {}
        }
    }
    names
}

/// Flatten `.` and `..` without touching the filesystem, so a cycle through
/// `lib/../lib/x.cypcb` is still recognised as the same file.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cypcb-imports-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const LIBRARY: &str = r#"version 1

module Divider {
    pin IN
    pin OUT

    component R1 resistor "0402" {
        value 10kohm
        at 1mm, 1mm
    }

    net IN {
        R1.1
    }

    net OUT {
        R1.2
    }
}

module Filter {
    pin A

    component C1 capacitor "0402" {
        value 100nF
        at 1mm, 1mm
    }

    net A {
        C1.1
    }
}
"#;

    fn resolve(dir: &Path, main: &str) -> (SourceFile, Vec<ImportError>) {
        let path = write(dir, "main.cypcb", main);
        let parsed = crate::parse(main);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let mut errors = Vec::new();
        let out = resolve_imports(&parsed.value, &path, &mut errors);
        (out, errors)
    }

    fn module_names(file: &SourceFile) -> Vec<&str> {
        let mut names: Vec<&str> = file
            .definitions
            .iter()
            .filter_map(|d| match d {
                Definition::Module(m) => Some(m.name.value.as_str()),
                _ => None,
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_plain_import_brings_every_reusable_definition() {
        let dir = temp_dir("all");
        write(&dir, "lib/blocks.cypcb", LIBRARY);

        let (file, errors) = resolve(
            &dir,
            "version 1\n\nimport \"lib/blocks.cypcb\"\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n",
        );

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(module_names(&file), vec!["Divider", "Filter"]);
        assert!(
            file.definitions
                .iter()
                .any(|d| matches!(d, Definition::Board(_))),
            "the importing file keeps its own board"
        );
    }

    #[test]
    fn a_selective_import_takes_only_what_it_names() {
        let dir = temp_dir("selective");
        write(&dir, "lib/blocks.cypcb", LIBRARY);

        let (file, errors) = resolve(
            &dir,
            "version 1\n\nimport Divider from \"lib/blocks.cypcb\"\n",
        );

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(module_names(&file), vec!["Divider"]);
    }

    #[test]
    fn asking_for_something_that_is_not_there_says_so() {
        let dir = temp_dir("missing-name");
        write(&dir, "lib/blocks.cypcb", LIBRARY);

        let (_, errors) = resolve(
            &dir,
            "version 1\n\nimport Regulator from \"lib/blocks.cypcb\"\n",
        );

        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ImportError::NotFound { name, .. } if name == "Regulator"),
            "{errors:?}"
        );
    }

    #[test]
    fn a_missing_file_names_where_it_looked() {
        let dir = temp_dir("missing-file");
        let (_, errors) = resolve(&dir, "version 1\n\nimport \"lib/nope.cypcb\"\n");

        assert_eq!(errors.len(), 1);
        let message = errors[0].to_string();
        assert!(
            message.contains("lib/nope.cypcb") && message.contains("looked in"),
            "{message}"
        );
    }

    #[test]
    fn a_library_may_be_built_from_libraries() {
        let dir = temp_dir("nested");
        write(&dir, "lib/inner.cypcb", LIBRARY);
        write(
            &dir,
            "lib/outer.cypcb",
            "version 1\n\nimport Divider from \"inner.cypcb\"\n",
        );

        let (file, errors) = resolve(&dir, "version 1\n\nimport \"lib/outer.cypcb\"\n");

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            module_names(&file),
            vec!["Divider"],
            "what outer took from inner comes through, and nothing else"
        );
    }

    #[test]
    fn a_file_that_imports_itself_is_reported_rather_than_followed() {
        let dir = temp_dir("cycle");
        write(&dir, "a.cypcb", "version 1\n\nimport \"b.cypcb\"\n");
        write(&dir, "b.cypcb", "version 1\n\nimport \"a.cypcb\"\n");

        let path = dir.join("a.cypcb");
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::parse(&source);
        let mut errors = Vec::new();
        let mut visiting = vec![normalise(&path)];
        resolve_into(&parsed.value, &path, &FromDisk, &mut visiting, &mut errors);

        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ImportError::Cycle { .. })),
            "{errors:?}"
        );
    }
}
