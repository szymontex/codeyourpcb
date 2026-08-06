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

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ast::{Definition, SourceFile};

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
    let mut visiting = Vec::new();
    resolve_into(file, origin, &mut visiting, errors)
}

fn resolve_into(
    file: &SourceFile,
    origin: &Path,
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

        let source = match std::fs::read_to_string(&resolved) {
            Ok(source) => source,
            Err(reason) => {
                errors.push(ImportError::Unreadable {
                    path: written,
                    resolved,
                    reason: reason.to_string(),
                });
                continue;
            }
        };

        let parsed = crate::parse(&source);
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
        let inner = resolve_into(&parsed.value, &resolved, visiting, errors);
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
        resolve_into(&parsed.value, &path, &mut visiting, &mut errors);

        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ImportError::Cycle { .. })),
            "{errors:?}"
        );
    }
}
