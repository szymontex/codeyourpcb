//! A directory a test has to itself.
//!
//! A test that writes to `temp_dir().join("cypcb-something")` shares that
//! directory with every other run of itself on the machine: the nightly gate
//! and a gate started by hand from another checkout empty it under each other.
//! `saving_a_design_checks_it_again` did exactly that and ran out of time
//! inside a full run. The name here carries the process id and a counter, so
//! two runs never meet, and the directory goes away when the test is done
//! with it, so a unique name does not leave one directory behind per run.
//!
//! Set `CYPCB_KEEP_SCRATCH=1` to keep the directory of a test that fails; its
//! path is printed so it can be looked at.

use std::ffi::OsStr;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static MADE: AtomicUsize = AtomicUsize::new(0);

/// An empty directory under the machine's temporary directory, removed when
/// this is dropped.
#[derive(Debug)]
pub struct ScratchDir {
    path: PathBuf,
}

/// A fresh, empty directory named after `tag`, unique to this process and to
/// this call.
pub fn scratch_dir(tag: &str) -> ScratchDir {
    let made = MADE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("{tag}-{}-{made}", std::process::id()));
    // A process id is reused once its process is gone; what that process
    // left behind is not this test's.
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch directory under the temporary directory");
    ScratchDir { path }
}

impl ScratchDir {
    /// The directory itself.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A path inside this directory that keeps the directory for as long as
    /// the path is held - for a helper that makes the directory, writes a file
    /// into it and hands back only the file.
    pub fn holding(self, path: PathBuf) -> ScratchPath {
        ScratchPath { path, _dir: self }
    }
}

impl Deref for ScratchDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for ScratchDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<OsStr> for ScratchDir {
    fn as_ref(&self) -> &OsStr {
        self.path.as_os_str()
    }
}

/// A path whose scratch directory lives exactly as long as it does.
#[derive(Debug)]
pub struct ScratchPath {
    path: PathBuf,
    _dir: ScratchDir,
}

impl Deref for ScratchPath {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for ScratchPath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<OsStr> for ScratchPath {
    fn as_ref(&self) -> &OsStr {
        self.path.as_os_str()
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        if std::thread::panicking() && std::env::var_os("CYPCB_KEEP_SCRATCH").is_some() {
            eprintln!("kept for CYPCB_KEEP_SCRATCH: {}", self.path.display());
            return;
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_calls_with_one_tag_get_two_directories() {
        let first = scratch_dir("cypcb-scratch-selftest");
        let second = scratch_dir("cypcb-scratch-selftest");
        assert_ne!(first.path(), second.path());
        assert!(first.is_dir() && second.is_dir());
        let name = first.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.contains(&std::process::id().to_string()),
            "the name carries the process id: {name}"
        );
    }

    #[test]
    fn the_directory_goes_when_the_test_is_done() {
        let dir = scratch_dir("cypcb-scratch-selftest");
        std::fs::write(dir.join("left.txt"), "x").unwrap();
        let path = dir.to_path_buf();
        drop(dir);
        assert!(!path.exists(), "{} is still there", path.display());
    }

    #[test]
    fn a_held_path_keeps_its_directory() {
        let dir = scratch_dir("cypcb-scratch-selftest");
        let home = dir.to_path_buf();
        let file = dir.join("board.cypcb");
        std::fs::write(&file, "x").unwrap();
        let held = dir.holding(file);
        assert!(held.is_file(), "the directory went with the ScratchDir");
        drop(held);
        assert!(!home.exists(), "{} is still there", home.display());
    }

    #[test]
    fn a_failing_test_keeps_it_only_when_asked() {
        // Unwinding through a drop is what a failing assertion does.
        let kept = std::panic::catch_unwind(|| {
            let dir = scratch_dir("cypcb-scratch-selftest");
            let path = dir.to_path_buf();
            std::panic::panic_any(path);
        })
        .unwrap_err();
        let path = kept.downcast::<PathBuf>().unwrap();
        let asked = std::env::var_os("CYPCB_KEEP_SCRATCH").is_some();
        assert_eq!(path.exists(), asked, "{}", path.display());
        let _ = std::fs::remove_dir_all(&*path);
    }
}
