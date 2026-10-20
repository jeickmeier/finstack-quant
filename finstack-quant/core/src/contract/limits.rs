//! Resource limits for persisted artifact loading.

/// Default maximum encoded artifact size: 64 MiB.
pub const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Default maximum number of artifacts in one load operation.
const DEFAULT_MAX_ARTIFACTS: usize = 100_000;
/// Default maximum number of portfolio positions in one artifact.
const DEFAULT_MAX_POSITIONS: usize = 1_000_000;
/// Default maximum JSON nesting depth accepted by contract loaders.
const DEFAULT_MAX_DEPTH: usize = 96;
/// Default maximum number of diagnostics retained in a validation report.
const DEFAULT_MAX_DIAGNOSTICS: usize = 256;

/// Resource limits applied while loading persisted artifacts.
///
/// The defaults protect ordinary callers from accidentally processing
/// pathologically large documents. Builder methods allow a loader to tighten or
/// relax an individual limit for a known workload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadLimits {
    /// Maximum encoded input size in bytes.
    pub max_bytes: usize,
    /// Maximum number of artifacts accepted in one load operation.
    pub max_artifacts: usize,
    /// Maximum number of portfolio positions accepted in one artifact.
    pub max_positions: usize,
    /// Maximum JSON nesting depth accepted by contract loaders.
    ///
    /// The default of 96 leaves headroom below `serde_json`'s recursion
    /// backstop.
    pub max_depth: usize,
    /// Maximum number of individual diagnostics retained in a report.
    ///
    /// Additional findings set [`ValidationReport::truncated`](super::ValidationReport::truncated).
    pub max_diagnostics: usize,
}

impl LoadLimits {
    /// Set the maximum encoded input size.
    ///
    /// # Arguments
    ///
    /// * `max_bytes` - Maximum number of encoded bytes accepted per input;
    ///   zero rejects every non-empty input.
    #[must_use]
    pub const fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// Set the maximum JSON nesting depth.
    ///
    /// # Arguments
    ///
    /// * `max_depth` - Maximum number of nested JSON containers accepted by a
    ///   loader; zero permits only scalar roots.
    #[must_use]
    pub const fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set the maximum retained diagnostic count.
    ///
    /// # Arguments
    ///
    /// * `max_diagnostics` - Maximum number of findings retained individually;
    ///   further findings only mark the report as truncated.
    #[must_use]
    pub const fn with_max_diagnostics(mut self, max_diagnostics: usize) -> Self {
        self.max_diagnostics = max_diagnostics;
        self
    }
}

impl Default for LoadLimits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            max_artifacts: DEFAULT_MAX_ARTIFACTS,
            max_positions: DEFAULT_MAX_POSITIONS,
            max_depth: DEFAULT_MAX_DEPTH,
            max_diagnostics: DEFAULT_MAX_DIAGNOSTICS,
        }
    }
}
