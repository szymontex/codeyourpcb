//! FreeRouting CLI integration.
//!
//! Provides a wrapper for running FreeRouting as an external process
//! with timeout, cancellation, and progress tracking support.
//!
//! # Usage
//!
//! ```rust,ignore
//! use cypcb_router::freerouting::{FreeRoutingRunner, RoutingConfig};
//!
//! let config = RoutingConfig::new("freerouting.jar".into());
//! let runner = FreeRoutingRunner::new(config);
//!
//! // Run routing (blocking)
//! let result = runner.route(&dsn_path, &ses_path, &net_lookup)?;
//!
//! // Or with progress callback
//! let result = runner.route_with_progress(&dsn_path, &ses_path, &net_lookup, |progress| {
//!     println!("Pass {}: {} routed, {} unrouted", progress.pass, progress.routed, progress.unrouted);
//! })?;
//! ```
//!
//! # Cancellation
//!
//! The runner supports cooperative cancellation via a shared atomic flag:
//!
//! ```rust,ignore
//! let runner = FreeRoutingRunner::new(config);
//! let cancel_flag = runner.cancel_flag();
//!
//! // In another thread
//! cancel_flag.store(true, Ordering::SeqCst);
//!
//! // The route() call will terminate and return Cancelled status
//! ```

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cypcb_world::NetId;
use thiserror::Error;

use crate::ses::import_ses;
use crate::types::{RoutingResult, RoutingStatus};

/// Errors that can occur during FreeRouting execution.
#[derive(Debug, Error)]
pub enum RoutingError {
    /// Java runtime not found.
    #[error("Java not found. Please install Java 21+ to use FreeRouting")]
    JavaNotFound,

    /// FreeRouting JAR file not found.
    #[error("FreeRouting JAR not found at: {0}")]
    JarNotFound(PathBuf),

    /// DSN input file not found.
    #[error("DSN input file not found: {0}")]
    DsnNotFound(PathBuf),

    /// Process failed to start or crashed.
    #[error("FreeRouting process failed: {0}")]
    ProcessFailed(String),

    /// IO error during process communication.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// SES import error.
    #[error("Failed to import routing results: {0}")]
    ImportError(#[from] crate::ses::SesImportError),

    /// Routing was cancelled.
    #[error("Routing cancelled by user")]
    Cancelled,

    /// Routing timed out.
    #[error("Routing timed out after {0} seconds")]
    Timeout(u64),
}

/// Configuration for FreeRouting execution.
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    /// Path to freerouting.jar.
    pub jar_path: PathBuf,

    /// Timeout in seconds (default: 300 = 5 minutes).
    pub timeout_secs: u64,

    /// Maximum routing passes (FreeRouting -mp flag).
    pub max_passes: Option<u32>,

    /// Enable fanout optimization.
    pub fanout: bool,

    /// Additional Java arguments.
    pub java_args: Vec<String>,
}

impl RoutingConfig {
    /// Create a new routing configuration with the given JAR path.
    pub fn new(jar_path: PathBuf) -> Self {
        RoutingConfig {
            jar_path,
            timeout_secs: 300, // 5 minutes default
            max_passes: None,
            fanout: true,
            java_args: Vec::new(),
        }
    }

    /// Set the timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set the maximum number of routing passes.
    pub fn with_max_passes(mut self, passes: u32) -> Self {
        self.max_passes = Some(passes);
        self
    }

    /// Enable or disable fanout optimization.
    pub fn with_fanout(mut self, enabled: bool) -> Self {
        self.fanout = enabled;
        self
    }
}

/// Progress information during routing.
#[derive(Debug, Clone, Default)]
pub struct RoutingProgress {
    /// Current routing pass number.
    pub pass: u32,

    /// Number of connections routed so far.
    pub routed: u32,

    /// Number of connections remaining unrouted.
    pub unrouted: u32,

    /// Elapsed time in seconds.
    pub elapsed_secs: u64,
}

/// FreeRouting CLI runner.
pub struct FreeRoutingRunner {
    config: RoutingConfig,
    cancel_flag: Arc<AtomicBool>,
}

impl FreeRoutingRunner {
    /// Create a new FreeRouting runner with the given configuration.
    pub fn new(config: RoutingConfig) -> Self {
        FreeRoutingRunner {
            config,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a clone of the cancel flag for external cancellation.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel_flag)
    }

    /// Request cancellation of the current routing operation.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Reset the cancel flag (call before starting a new routing operation).
    pub fn reset(&self) {
        self.cancel_flag.store(false, Ordering::SeqCst);
    }

    /// Run FreeRouting on a DSN file and return the routing results.
    ///
    /// # Arguments
    ///
    /// * `dsn_path` - Path to the input DSN file
    /// * `ses_path` - Path where the output SES file will be written
    /// * `net_lookup` - Map from net name to NetId for import
    ///
    /// # Returns
    ///
    /// A `RoutingResult` on success, or `RoutingError` on failure.
    pub fn route(
        &self,
        dsn_path: &Path,
        ses_path: &Path,
        net_lookup: &HashMap<String, NetId>,
    ) -> Result<RoutingResult, RoutingError> {
        self.route_with_progress(dsn_path, ses_path, net_lookup, |_| {})
    }

    /// Run FreeRouting with progress callback.
    ///
    /// The callback is invoked periodically with progress updates parsed
    /// from FreeRouting's stdout.
    pub fn route_with_progress<F>(
        &self,
        dsn_path: &Path,
        ses_path: &Path,
        net_lookup: &HashMap<String, NetId>,
        mut on_progress: F,
    ) -> Result<RoutingResult, RoutingError>
    where
        F: FnMut(RoutingProgress),
    {
        // Reset cancel flag at start
        self.reset();

        // Verify prerequisites
        self.verify_java()?;
        self.verify_jar()?;

        if !dsn_path.exists() {
            return Err(RoutingError::DsnNotFound(dsn_path.to_path_buf()));
        }

        // Build command
        let mut child = self.spawn_freerouting(dsn_path, ses_path)?;

        // Monitor process with timeout and cancellation
        let start_time = Instant::now();
        let timeout = Duration::from_secs(self.config.timeout_secs);

        // Read stdout for progress
        let stdout = child.stdout.take();
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);

            for line in reader.lines() {
                // Check cancellation
                if self.cancel_flag.load(Ordering::SeqCst) {
                    let _ = child.kill();
                    return self.handle_cancellation(ses_path, net_lookup);
                }

                // Check timeout
                if start_time.elapsed() > timeout {
                    let _ = child.kill();
                    return self.handle_timeout(ses_path, net_lookup);
                }

                if let Ok(line) = line {
                    // Parse progress from FreeRouting output
                    if let Some(progress) = parse_progress(&line, start_time.elapsed().as_secs()) {
                        on_progress(progress);
                    }
                }
            }
        }

        // Wait for process to finish
        let status = child.wait()?;

        // Check final cancellation
        if self.cancel_flag.load(Ordering::SeqCst) {
            return self.handle_cancellation(ses_path, net_lookup);
        }

        // Import results
        if ses_path.exists() {
            let result = import_ses(ses_path, net_lookup)?;
            Ok(result)
        } else if status.success() {
            // Process succeeded but no SES file - maybe no routing needed
            Ok(RoutingResult::default())
        } else {
            Err(RoutingError::ProcessFailed(format!(
                "FreeRouting exited with status: {}",
                status
            )))
        }
    }

    /// Verify Java is available.
    fn verify_java(&self) -> Result<(), RoutingError> {
        match Command::new("java").arg("-version").output() {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    Err(RoutingError::JavaNotFound)
                }
            }
            Err(_) => Err(RoutingError::JavaNotFound),
        }
    }

    /// Verify JAR file exists.
    fn verify_jar(&self) -> Result<(), RoutingError> {
        if self.config.jar_path.exists() {
            Ok(())
        } else {
            Err(RoutingError::JarNotFound(self.config.jar_path.clone()))
        }
    }

    /// Spawn the FreeRouting process.
    fn spawn_freerouting(
        &self,
        dsn_path: &Path,
        ses_path: &Path,
    ) -> Result<Child, RoutingError> {
        let mut cmd = Command::new("java");

        // Add Java arguments
        for arg in &self.config.java_args {
            cmd.arg(arg);
        }

        // Add JAR path
        cmd.arg("-jar").arg(&self.config.jar_path);

        // Add FreeRouting arguments
        cmd.arg("-de").arg(dsn_path); // Design input
        cmd.arg("-do").arg(ses_path); // Design output (SES)

        // Optional: max passes
        if let Some(mp) = self.config.max_passes {
            cmd.arg("-mp").arg(mp.to_string());
        }

        // Run headless (required for CLI mode in FreeRouting 2.x)
        cmd.arg("--gui.enabled=false");
        cmd.arg("-dct"); // Don't check freerouting.net for updates

        // Capture stdout/stderr
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            RoutingError::ProcessFailed(format!("Failed to spawn FreeRouting: {}", e))
        })?;

        Ok(child)
    }

    /// Handle cancellation - return partial results if available.
    fn handle_cancellation(
        &self,
        ses_path: &Path,
        net_lookup: &HashMap<String, NetId>,
    ) -> Result<RoutingResult, RoutingError> {
        // Try to import partial results if SES file exists
        if ses_path.exists() {
            match import_ses(ses_path, net_lookup) {
                Ok(mut result) => {
                    result.status = RoutingStatus::Partial {
                        unrouted_count: 0, // Unknown at this point
                    };
                    Ok(result)
                }
                Err(_) => Err(RoutingError::Cancelled),
            }
        } else {
            Err(RoutingError::Cancelled)
        }
    }

    /// Handle timeout - return partial results if available.
    fn handle_timeout(
        &self,
        ses_path: &Path,
        net_lookup: &HashMap<String, NetId>,
    ) -> Result<RoutingResult, RoutingError> {
        // Try to import partial results if SES file exists
        if ses_path.exists() {
            match import_ses(ses_path, net_lookup) {
                Ok(mut result) => {
                    result.status = RoutingStatus::Partial {
                        unrouted_count: 0,
                    };
                    Ok(result)
                }
                Err(_) => Err(RoutingError::Timeout(self.config.timeout_secs)),
            }
        } else {
            Err(RoutingError::Timeout(self.config.timeout_secs))
        }
    }
}

/// Parse progress information from FreeRouting stdout.
///
/// FreeRouting outputs lines like:
/// - "Pass N: X connections routed"
/// - "X incomplete" or "X unrouted"
fn parse_progress(line: &str, elapsed_secs: u64) -> Option<RoutingProgress> {
    let line_lower = line.to_lowercase();

    // Look for pass number
    let pass = if let Some(idx) = line_lower.find("pass") {
        let rest = &line[idx + 4..];
        let digits: String = rest.chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse().unwrap_or(0)
    } else {
        0
    };

    // Look for routed count
    let routed = extract_number_before(&line_lower, "routed")
        .or_else(|| extract_number_before(&line_lower, "complete"))
        .unwrap_or(0);

    // Look for unrouted count
    let unrouted = extract_number_before(&line_lower, "unrouted")
        .or_else(|| extract_number_before(&line_lower, "incomplete"))
        .unwrap_or(0);

    // Only return progress if we found meaningful info
    if pass > 0 || routed > 0 || unrouted > 0 {
        Some(RoutingProgress {
            pass,
            routed,
            unrouted,
            elapsed_secs,
        })
    } else {
        None
    }
}

/// Extract a number that appears before a keyword.
fn extract_number_before(line: &str, keyword: &str) -> Option<u32> {
    let idx = line.find(keyword)?;
    let before = &line[..idx];

    // Find the last number before the keyword
    let mut num_str = String::new();
    for c in before.chars().rev() {
        if c.is_ascii_digit() {
            num_str.insert(0, c);
        } else if !num_str.is_empty() {
            break;
        }
    }

    if num_str.is_empty() {
        None
    } else {
        num_str.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_config_builder() {
        let config = RoutingConfig::new("freerouting.jar".into())
            .with_timeout(600)
            .with_max_passes(10)
            .with_fanout(false);

        assert_eq!(config.timeout_secs, 600);
        assert_eq!(config.max_passes, Some(10));
        assert!(!config.fanout);
    }

    #[test]
    fn test_parse_progress_pass() {
        let progress = parse_progress("Pass 5: routing connections", 10);
        assert!(progress.is_some());
        assert_eq!(progress.unwrap().pass, 5);
    }

    #[test]
    fn test_parse_progress_routed() {
        let progress = parse_progress("42 connections routed", 20);
        assert!(progress.is_some());
        let p = progress.unwrap();
        assert_eq!(p.routed, 42);
        assert_eq!(p.elapsed_secs, 20);
    }

    #[test]
    fn test_parse_progress_unrouted() {
        let progress = parse_progress("5 connections unrouted", 30);
        assert!(progress.is_some());
        assert_eq!(progress.unwrap().unrouted, 5);
    }

    #[test]
    fn test_parse_progress_combined() {
        let progress = parse_progress("Pass 3: 100 routed, 5 unrouted", 15);
        assert!(progress.is_some());
        let p = progress.unwrap();
        assert_eq!(p.pass, 3);
        assert_eq!(p.routed, 100);
        assert_eq!(p.unrouted, 5);
    }

    #[test]
    fn test_parse_progress_no_match() {
        let progress = parse_progress("Starting FreeRouting...", 0);
        assert!(progress.is_none());
    }

    #[test]
    fn test_extract_number_before() {
        assert_eq!(extract_number_before("42 routed", "routed"), Some(42));
        assert_eq!(extract_number_before("has 123 connections routed", "routed"), Some(123));
        assert_eq!(extract_number_before("no number here routed", "routed"), None);
        assert_eq!(extract_number_before("no keyword here", "routed"), None);
    }

    #[test]
    fn test_cancel_flag() {
        let config = RoutingConfig::new("freerouting.jar".into());
        let runner = FreeRoutingRunner::new(config);

        // Initial state
        assert!(!runner.cancel_flag.load(Ordering::SeqCst));

        // Cancel
        runner.cancel();
        assert!(runner.cancel_flag.load(Ordering::SeqCst));

        // Reset
        runner.reset();
        assert!(!runner.cancel_flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_cancel_flag_shared() {
        let config = RoutingConfig::new("freerouting.jar".into());
        let runner = FreeRoutingRunner::new(config);

        let external_flag = runner.cancel_flag();

        // Set from external flag
        external_flag.store(true, Ordering::SeqCst);

        // Should be visible on runner
        assert!(runner.cancel_flag.load(Ordering::SeqCst));
    }

    // Note: Full integration tests with actual FreeRouting would require:
    // - FreeRouting JAR available
    // - Java installed
    // - Actual DSN file
    // These are better suited for integration tests with skip annotations.
}
