/// Selects how simulated paths are captured for diagnostics.
///
/// Use [`PathCaptureMode::All`] when every path should be retained. Use
/// [`PathCaptureMode::Sample`] when you only need a representative subset for
/// plotting or debugging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathCaptureMode {
    /// Capture every simulated path.
    All,
    /// Capture a deterministic sample of paths.
    ///
    /// The sample is selected by hashing `path_id` together with `seed` and
    /// comparing the result against the implied sampling probability
    /// `count / num_paths`. This makes the capture decision reproducible across
    /// serial and parallel runs, but the realized number of captured paths is
    /// generally close to `count`, not guaranteed to equal it exactly.
    Sample {
        /// Target number of paths to capture on average.
        count: usize,
        /// Seed controlling the deterministic sampling decision.
        seed: u64,
    },
}

/// Configures optional path capture during Monte Carlo pricing.
///
/// Captured paths can include state vectors, cashflows, and optionally payoff
/// snapshots at each time step. The engine validates that sampled capture counts
/// are between `1` and `num_paths`, and that capture is not combined with
/// antithetic pricing.
#[derive(Debug, Clone)]
pub struct PathCaptureConfig {
    /// Whether path capture is enabled
    pub enabled: bool,
    /// Capture mode (all paths or sample)
    pub capture_mode: PathCaptureMode,
    /// Whether to capture payoff values at each timestep
    pub capture_payoffs: bool,
}

impl PathCaptureConfig {
    /// Create a disabled path-capture configuration.
    ///
    /// # Returns
    ///
    /// A configuration with capture disabled and no payoff snapshots.
    pub fn new() -> Self {
        Self {
            enabled: false,
            capture_mode: PathCaptureMode::All,
            capture_payoffs: false,
        }
    }

    /// Enable capture for every simulated path.
    ///
    /// # Returns
    ///
    /// A configuration that records all paths but does not capture intermediate
    /// payoff values unless [`Self::with_payoffs`] is called.
    pub fn all() -> Self {
        Self {
            enabled: true,
            capture_mode: PathCaptureMode::All,
            capture_payoffs: false,
        }
    }

    /// Enable capture for a deterministic sample of paths.
    ///
    /// # Arguments
    ///
    /// * `count` - Target number of captured paths on average. Runtime
    ///   validation requires `1 <= count <= num_paths`.
    /// * `seed` - Sampling seed used in the hash-based selection rule.
    ///
    /// # Returns
    ///
    /// A configuration that records an expected sample of paths. The realized
    /// number of captured paths can differ from `count`.
    pub fn sample(count: usize, seed: u64) -> Self {
        Self {
            enabled: true,
            capture_mode: PathCaptureMode::Sample { count, seed },
            capture_payoffs: false,
        }
    }

    /// Record payoff snapshots at each captured time step.
    ///
    /// # Returns
    ///
    /// The same configuration with `capture_payoffs` enabled.
    pub fn with_payoffs(mut self) -> Self {
        self.capture_payoffs = true;
        self
    }

    /// Decide whether a particular path should be captured.
    ///
    /// # Arguments
    ///
    /// * `path_id` - Zero-based Monte Carlo path identifier.
    /// * `num_paths` - Total number of simulated paths in the run.
    ///
    /// # Returns
    ///
    /// `true` if the path should be recorded under the configured capture mode.
    /// For sampled capture this uses deterministic Bernoulli sampling, so the
    /// total number of `true` results is approximate.
    pub fn should_capture(&self, path_id: usize, num_paths: usize) -> bool {
        if !self.enabled {
            return false;
        }

        match self.capture_mode {
            PathCaptureMode::All => true,
            PathCaptureMode::Sample { count, seed } => {
                // Use hash-based sampling for determinism
                // This ensures same paths are selected across runs
                // Use a proper hash function that provides good distribution
                let mut hash = path_id as u64;
                hash = hash.wrapping_mul(0x9e3779b97f4a7c15); // Multiplicative hash constant
                hash ^= seed;
                hash = hash.wrapping_mul(0x9e3779b97f4a7c15);
                hash ^= hash >> 16; // Mix bits
                hash = hash.wrapping_mul(0x85ebca6b);
                hash ^= hash >> 13;
                hash = hash.wrapping_mul(0xc2b2ae35);
                hash ^= hash >> 16;

                let sample_prob = count as f64 / num_paths as f64;
                let threshold = (u64::MAX as f64 * sample_prob) as u64;
                hash < threshold
            }
        }
    }
}

impl Default for PathCaptureConfig {
    fn default() -> Self {
        Self::new()
    }
}
