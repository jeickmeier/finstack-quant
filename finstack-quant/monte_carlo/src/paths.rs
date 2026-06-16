//! Captured-path data structures for Monte Carlo diagnostics.
//!
//! These types store the optional path data produced by
//! [`crate::engine::McEngine::price_with_capture`]. They are intended for
//! visualization, debugging, and downstream analysis rather than for the
//! performance-critical inner simulation loop.
//!
//! # Conventions
//!
//! - `time` values are year fractions.
//! - `final_value` and per-point `payoff_value` are in the same numeric units as
//!   the priced payoff amount.
//! - Cashflow amounts use the sign convention `positive = inflow`, `negative = outflow`.
//! - Captured datasets may contain every path or only a sampled subset.

use std::collections::BTreeMap;

use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Default state-variable index for the spot in single-factor layouts.
///
/// Higher indices are intentionally not aliased here: `state[1]` means
/// "variance" in stochastic-vol models and "short rate" in HW1F-style rate
/// models, so naming both would invite confusion at call sites that mix
/// model families. Multi-asset and less standard models should prefer
/// [`PathDataset::process_params.factor_names`](PathDataset::process_params)
/// or [`PathDataset::state_var_keys`] to interpret captured state vectors.
pub(crate) mod state_indices {
    /// Spot price (equity/FX) — slot 0 across all single-asset processes.
    pub const IDX_SPOT: usize = 0;
}

/// Classifies captured cashflows by economic meaning.
///
/// These tags are diagnostic metadata only. They do not change pricing logic by
/// themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CashflowType {
    /// Principal deployment (draws) or repayment
    Principal,
    /// Interest payment on drawn amounts
    Interest,
    /// Commitment fee on undrawn amounts
    CommitmentFee,
    /// Usage fee on drawn amounts
    UsageFee,
    /// Facility fee on total commitment
    FacilityFee,
    /// One-time upfront fee
    UpfrontFee,
    /// Recovery proceeds on default
    Recovery,
    /// Mark-to-market P&L at timestep
    MarkToMarket,
    /// Other/generic cashflow
    Other,
}

/// A single captured point along a Monte Carlo path.
///
/// Captures the process state at a specific time step, together with any
/// cashflows emitted at that step and, optionally, a payoff snapshot.
///
/// # State Vector Layout
///
/// The `state` vector contains the raw state variables in process-defined order.
/// For simple single-asset models, the crate's internal `state_indices`
/// constants provide common aliases.
/// For multi-asset or process-specific layouts, consult
/// [`PathDataset::process_params.factor_names`](PathDataset::process_params) or
/// [`PathDataset::state_var_keys`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPoint {
    /// Time step index (0 = initial, N = final)
    pub step: usize,
    /// Time in years from valuation date
    pub time: f64,
    /// State variables at this point (spot, variance, rate, etc.)
    /// Indexed by position - see `state_indices` for standard layout
    pub state: SmallVec<[f64; 8]>,
    /// Optional payoff snapshot at this point.
    ///
    /// This is populated only when path capture requested payoff snapshots. It
    /// uses the payoff's native amount units and is not additionally discounted
    /// inside `PathPoint`.
    pub payoff_value: Option<f64>,
    /// Typed cashflows generated at this time step as `(time, amount, type)`.
    ///
    /// Amounts follow the sign convention `positive = inflow`,
    /// `negative = outflow`.
    #[serde(default)]
    pub cashflows: Vec<(f64, f64, CashflowType)>,
}

impl PathPoint {
    /// Create a path point with no state entries or cashflows.
    pub fn new(step: usize, time: f64) -> Self {
        Self {
            step,
            time,
            state: SmallVec::new(),
            payoff_value: None,
            cashflows: Vec::new(),
        }
    }

    /// Create a path point with an explicit raw state vector.
    pub fn with_state(step: usize, time: f64, state: SmallVec<[f64; 8]>) -> Self {
        Self {
            step,
            time,
            state,
            payoff_value: None,
            cashflows: Vec::new(),
        }
    }

    /// Store a payoff snapshot for this point.
    pub fn set_payoff(&mut self, value: f64) {
        self.payoff_value = Some(value);
    }

    /// Get spot price (convenience method for standard single-asset models).
    ///
    /// Returns the value at `state_indices::IDX_SPOT` if it exists.
    /// For multi-asset models, use `state` directly with the schema from `PathDataset`.
    pub fn spot(&self) -> Option<f64> {
        self.state.get(state_indices::IDX_SPOT).copied()
    }

    /// Get variance (convenience method for standard stochastic volatility models).
    ///
    /// Returns `state[1]` if present, treating it as the variance state for
    /// Heston-family models. For multi-asset or non-stochastic-vol models,
    /// use `state` directly with the schema from `PathDataset`.
    pub fn variance(&self) -> Option<f64> {
        // Slot 1 is variance in stochastic-vol models. Same numeric slot,
        // different meaning, in HW1F-style rate models — see `short_rate`.
        self.state.get(1).copied()
    }

    /// Get the short rate for common interest-rate layouts.
    ///
    /// Returns `state[1]` if present, treating it as the short-rate state for
    /// HW1F-style models. This shares the same numeric slot as
    /// [`Self::variance`]; the meaning is process-defined and the caller is
    /// responsible for picking the accessor that matches the process family.
    pub fn short_rate(&self) -> Option<f64> {
        self.state.get(1).copied()
    }

    /// Add a generic cashflow to this point.
    ///
    /// # Arguments
    /// * `time` - Cashflow time in years.
    /// * `amount` - Cashflow amount (`positive = inflow`, `negative = outflow`).
    pub fn add_cashflow(&mut self, time: f64, amount: f64) {
        self.cashflows.push((time, amount, CashflowType::Other));
    }

    /// Add a typed cashflow to this point.
    ///
    /// # Arguments
    /// * `time` - Cashflow time in years.
    /// * `amount` - Cashflow amount (`positive = inflow`, `negative = outflow`).
    /// * `cf_type` - Economic category for the cashflow.
    pub fn add_typed_cashflow(&mut self, time: f64, amount: f64, cf_type: CashflowType) {
        self.cashflows.push((time, amount, cf_type));
    }

    /// Get all cashflows at this point.
    pub fn get_cashflows(&self) -> &[(f64, f64, CashflowType)] {
        &self.cashflows
    }

    /// Get cashflows by type.
    ///
    /// Returns (time, amount) pairs for all cashflows matching the given type.
    pub fn get_cashflows_by_type(&self, cf_type: CashflowType) -> Vec<(f64, f64)> {
        self.cashflows
            .iter()
            .filter(|(_, _, t)| *t == cf_type)
            .map(|(time, amount, _)| (*time, *amount))
            .collect()
    }

    /// Get principal flows (convenience method).
    pub fn principal_flows(&self) -> Vec<(f64, f64)> {
        self.get_cashflows_by_type(CashflowType::Principal)
    }

    /// Get interest flows (convenience method).
    pub fn interest_flows(&self) -> Vec<(f64, f64)> {
        self.get_cashflows_by_type(CashflowType::Interest)
    }

    /// Get total cashflow amount at this timestep.
    pub fn total_cashflow(&self) -> f64 {
        self.cashflows.iter().map(|(_, amt, _)| amt).sum()
    }

    /// Get total cashflow amount by type.
    pub fn total_cashflow_by_type(&self, cf_type: CashflowType) -> f64 {
        self.cashflows
            .iter()
            .filter(|(_, _, t)| *t == cf_type)
            .map(|(_, amt, _)| amt)
            .sum()
    }
}

/// A complete captured Monte Carlo path.
///
/// Contains all captured points for a single simulated path, plus the final
/// discounted value used in summary statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedPath {
    /// Path identifier (0-indexed)
    pub path_id: usize,
    /// Time points along the path
    pub points: Vec<PathPoint>,
    /// Final discounted payoff value for this path.
    ///
    /// This is the path-level amount after the engine applies the run's
    /// `discount_factor`.
    pub final_value: f64,
    /// Internal rate of return inferred from the captured cashflow amounts, if calculable.
    #[serde(default)]
    pub irr: Option<f64>,
}

impl SimulatedPath {
    /// Create an empty captured path.
    pub fn new(path_id: usize) -> Self {
        Self {
            path_id,
            points: Vec::new(),
            final_value: 0.0,
            irr: None,
        }
    }

    /// Create an empty path with preallocated point capacity.
    pub fn with_capacity(path_id: usize, capacity: usize) -> Self {
        Self {
            path_id,
            points: Vec::with_capacity(capacity),
            final_value: 0.0,
            irr: None,
        }
    }

    /// Append a captured point to the path.
    pub fn add_point(&mut self, point: PathPoint) {
        self.points.push(point);
    }

    /// Set the final discounted path value.
    pub fn set_final_value(&mut self, value: f64) {
        self.final_value = value;
    }

    /// Store the inferred internal rate of return.
    pub fn set_irr(&mut self, irr: f64) {
        self.irr = Some(irr);
    }

    /// Return the number of captured points in the path.
    pub fn num_steps(&self) -> usize {
        self.points.len()
    }

    /// Get a point by step index.
    pub fn point(&self, step: usize) -> Option<&PathPoint> {
        self.points.get(step)
    }

    /// Get the initial point.
    pub fn initial_point(&self) -> Option<&PathPoint> {
        self.points.first()
    }

    /// Get the final point.
    pub fn terminal_point(&self) -> Option<&PathPoint> {
        self.points.last()
    }

    fn extract_cf<T, F>(&self, f: F) -> Vec<T>
    where
        F: Fn(&(f64, f64, CashflowType)) -> T,
    {
        self.points
            .iter()
            .flat_map(|p| p.cashflows.iter())
            .map(f)
            .collect()
    }

    /// Extract all cashflows from the path.
    ///
    /// Returns all `(time, amount)` pairs in path order using the same sign
    /// convention as [`PathPoint::cashflows`].
    pub fn extract_cashflows(&self) -> Vec<(f64, f64)> {
        self.extract_cf(|&(t, a, _)| (t, a))
    }

    /// Extract cashflow amounts in path order.
    ///
    /// This is primarily used for IRR-style calculations that only need the
    /// amount series and not the timestamps.
    pub fn extract_cashflow_amounts(&self) -> Vec<f64> {
        self.extract_cf(|&(_, a, _)| a)
    }

    /// Extract typed cashflows from the path.
    ///
    /// Returns all (time, amount, type) tuples across all timesteps.
    pub fn extract_typed_cashflows(&self) -> Vec<(f64, f64, CashflowType)> {
        self.extract_cf(|cf| *cf)
    }

    /// Extract cashflows by type.
    ///
    /// Returns all (time, amount) pairs for cashflows of the specified type.
    pub fn extract_cashflows_by_type(&self, cf_type: CashflowType) -> Vec<(f64, f64)> {
        self.points
            .iter()
            .flat_map(|p| p.cashflows.iter())
            .filter(|(_, _, t)| *t == cf_type)
            .map(|&(t, a, _)| (t, a))
            .collect()
    }
}

/// Records how a captured dataset was selected from the full simulation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PathSamplingMethod {
    /// Every path in the run was captured.
    All,
    /// Deterministic sample with target size `count`.
    ///
    /// The engine uses deterministic Bernoulli sampling, so `count` is the
    /// target number of captured paths on average rather than a strict promise.
    RandomSample {
        /// Target number of paths to capture on average.
        count: usize,
        /// Seed used by the deterministic sampling rule.
        seed: u64,
    },
}

impl std::fmt::Display for PathSamplingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::RandomSample { count, seed } => {
                write!(f, "random_sample(n={}, seed={})", count, seed)
            }
        }
    }
}

/// Metadata describing the process behind a captured dataset.
///
/// This structure is typically populated by
/// [`crate::process::metadata::ProcessMetadata`] implementations and stored in a
/// [`PathDataset`]. It describes how to interpret captured state vectors rather
/// than how to price the instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessParams {
    /// Process type identifier, such as `"GBM"` or `"Heston"`.
    pub process_type: String,
    /// Process parameters keyed by implementation-defined names such as `r`,
    /// `q`, `sigma`, `kappa`, or `theta`.
    pub parameters: BTreeMap<String, f64>,
    /// Optional row-major `n x n` correlation matrix.
    pub correlation: Option<Vec<f64>>,
    /// Names describing the order of captured state-vector entries.
    pub factor_names: Vec<String>,
}

impl ProcessParams {
    /// Create empty metadata for a process family.
    pub fn new(process_type: impl Into<String>) -> Self {
        Self {
            process_type: process_type.into(),
            parameters: BTreeMap::default(),
            correlation: None,
            factor_names: Vec::new(),
        }
    }

    /// Add a named process parameter.
    pub fn add_param(&mut self, key: impl Into<String>, value: f64) {
        self.parameters.insert(key.into(), value);
    }

    /// Attach a row-major correlation matrix.
    ///
    /// When `factor_names` is also present, its order must match the ordering of
    /// this matrix.
    pub fn with_correlation(mut self, correlation: Vec<f64>) -> Self {
        self.correlation = Some(correlation);
        self
    }

    /// Attach state-vector names in capture order.
    pub fn with_factors(mut self, names: Vec<String>) -> Self {
        self.factor_names = names;
        self
    }

    /// Infer the matrix dimension from `correlation`.
    pub fn dim(&self) -> Option<usize> {
        self.correlation.as_ref().and_then(|corr| {
            let dim = (corr.len() as f64).sqrt().round() as usize;
            if dim * dim == corr.len() {
                Some(dim)
            } else {
                None
            }
        })
    }

    /// Validate metadata consistency for captured-path consumers.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// * `correlation` is present but is not a square matrix
    /// * `correlation` is present but empty
    /// * `factor_names` is present and its length does not match the implied
    ///   correlation dimension
    pub fn validate(&self) -> Result<()> {
        if let Some(correlation) = &self.correlation {
            let dim = self.dim().ok_or_else(|| {
                Error::Validation(
                    "ProcessParams correlation metadata must be a square matrix".to_string(),
                )
            })?;

            if !self.factor_names.is_empty() && self.factor_names.len() != dim {
                return Err(Error::Validation(format!(
                    "ProcessParams factor_names length {} does not match correlation dimension {}",
                    self.factor_names.len(),
                    dim
                )));
            }

            if correlation.is_empty() {
                return Err(Error::Validation(
                    "ProcessParams correlation metadata cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Captured-path collection plus the metadata needed to interpret it.
///
/// The dataset may contain every path or only a deterministic sample of the
/// full simulation, depending on the value of `sampling_method`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathDataset {
    /// Captured paths in deterministic order.
    pub paths: Vec<SimulatedPath>,
    /// Total number of paths simulated by the engine.
    pub num_paths_total: usize,
    /// Sampling method used to retain `paths`.
    pub sampling_method: PathSamplingMethod,
    /// Metadata needed to interpret captured state vectors.
    pub process_params: ProcessParams,
}

impl PathDataset {
    /// Create an empty captured-path dataset.
    pub fn new(
        num_paths_total: usize,
        sampling_method: PathSamplingMethod,
        process_params: ProcessParams,
    ) -> Self {
        // Pre-allocate based on sampling method
        let estimated_capacity = match sampling_method {
            PathSamplingMethod::All => num_paths_total,
            PathSamplingMethod::RandomSample { count, .. } => count,
        };
        Self {
            paths: Vec::with_capacity(estimated_capacity),
            num_paths_total,
            sampling_method,
            process_params,
        }
    }

    /// Append a captured path to the dataset.
    pub fn add_path(&mut self, path: SimulatedPath) {
        self.paths.push(path);
    }

    /// Return the number of captured paths currently stored.
    pub fn num_captured(&self) -> usize {
        self.paths.len()
    }

    /// Get a path by index.
    pub fn path(&self, index: usize) -> Option<&SimulatedPath> {
        self.paths.get(index)
    }

    /// Return `true` when the dataset contains every simulated path.
    pub fn is_complete(&self) -> bool {
        self.sampling_method == PathSamplingMethod::All && self.paths.len() == self.num_paths_total
    }

    /// Return `num_captured / num_paths_total`.
    pub fn sampling_ratio(&self) -> f64 {
        if self.num_paths_total == 0 {
            0.0
        } else {
            self.paths.len() as f64 / self.num_paths_total as f64
        }
    }

    /// Return names describing the captured state-vector layout.
    ///
    /// Returns `process_params.factor_names` when available. Otherwise the names
    /// are synthesized as `state_0`, `state_1`, ... based on the widest
    /// captured state vector in the dataset.
    pub fn state_var_keys(&self) -> Vec<String> {
        // Use factor names from process metadata if available
        if !self.process_params.factor_names.is_empty() {
            return self.process_params.factor_names.clone();
        }

        // Otherwise, generate generic names based on max dimension
        let max_dim = self
            .paths
            .iter()
            .flat_map(|path| path.points.iter())
            .map(|point| point.state.len())
            .max()
            .unwrap_or(0);

        (0..max_dim).map(|i| format!("state_{}", i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_point_creation() {
        let mut point = PathPoint::new(0, 0.0);
        assert_eq!(point.step, 0);
        assert_eq!(point.time, 0.0);
        assert!(point.state.is_empty());
        assert!(point.payoff_value.is_none());
        assert!(point.cashflows.is_empty());

        // Create a point with state
        let mut state = SmallVec::new();
        state.push(100.0); // spot
        let mut point_with_state = PathPoint::with_state(0, 0.0, state);
        assert_eq!(point_with_state.spot(), Some(100.0));

        point_with_state.set_payoff(42.5);
        assert_eq!(point_with_state.payoff_value, Some(42.5));

        // Test cashflows
        point.add_cashflow(0.25, 1000.0);
        point.add_cashflow(0.25, 500.0);
        assert_eq!(point.cashflows.len(), 2);
        assert_eq!(
            point.get_cashflows()[0],
            (0.25, 1000.0, CashflowType::Other)
        );
        assert_eq!(point.get_cashflows()[1], (0.25, 500.0, CashflowType::Other));
        assert_eq!(point.total_cashflow(), 1500.0);

        // Test typed cashflows
        let mut point2 = PathPoint::new(1, 0.5);
        point2.add_typed_cashflow(0.5, 100.0, CashflowType::Interest);
        point2.add_typed_cashflow(0.5, 50.0, CashflowType::Principal);
        assert_eq!(point2.principal_flows().len(), 1);
        assert_eq!(point2.interest_flows().len(), 1);
        assert_eq!(point2.total_cashflow_by_type(CashflowType::Interest), 100.0);
    }

    #[test]
    fn test_simulated_path() {
        let mut path = SimulatedPath::with_capacity(1, 10);
        assert_eq!(path.path_id, 1);
        assert_eq!(path.num_steps(), 0);

        let mut state1 = SmallVec::new();
        state1.push(100.0); // spot
        let point1 = PathPoint::with_state(0, 0.0, state1);
        path.add_point(point1);

        let mut state2 = SmallVec::new();
        state2.push(102.0); // spot
        let point2 = PathPoint::with_state(1, 0.1, state2);
        path.add_point(point2);

        assert_eq!(path.num_steps(), 2);
        assert_eq!(
            path.initial_point()
                .expect("Path should have initial point")
                .spot(),
            Some(100.0)
        );
        assert_eq!(
            path.terminal_point()
                .expect("Path should have terminal point")
                .spot(),
            Some(102.0)
        );

        path.set_final_value(5.0);
        assert_eq!(path.final_value, 5.0);
    }

    #[test]
    fn test_simulated_path_cashflows() {
        let mut path = SimulatedPath::with_capacity(1, 10);

        // Add points with cashflows
        let mut point1 = PathPoint::new(0, 0.0);
        point1.add_cashflow(0.0, -100.0); // Initial outflow
        path.add_point(point1);

        let mut point2 = PathPoint::new(1, 0.25);
        point2.add_cashflow(0.25, 5.0); // Interest payment
        point2.add_cashflow(0.25, 2.0); // Fee payment
        path.add_point(point2);

        let mut point3 = PathPoint::new(2, 0.50);
        point3.add_cashflow(0.50, 5.0); // Interest payment
        path.add_point(point3);

        // Extract all cashflows
        let all_cashflows = path.extract_cashflows();
        assert_eq!(all_cashflows.len(), 4);
        assert_eq!(all_cashflows[0], (0.0, -100.0));
        assert_eq!(all_cashflows[1], (0.25, 5.0));
        assert_eq!(all_cashflows[2], (0.25, 2.0));
        assert_eq!(all_cashflows[3], (0.50, 5.0));

        let amounts = path.extract_cashflow_amounts();
        assert_eq!(amounts, vec![-100.0, 5.0, 2.0, 5.0]);
    }

    #[test]
    fn test_process_params() {
        let mut params = ProcessParams::new("GBM");
        params.add_param("r", 0.05);
        params.add_param("q", 0.02);
        params.add_param("sigma", 0.2);

        assert_eq!(params.process_type, "GBM");
        assert_eq!(params.parameters.get("r"), Some(&0.05));
        assert_eq!(params.parameters.get("sigma"), Some(&0.2));
    }

    #[test]
    fn test_process_params_serializes_parameters_in_key_order() {
        let mut params = ProcessParams::new("Heston");
        params.add_param("theta", 0.04);
        params.add_param("kappa", 2.0);
        params.add_param("sigma_v", 0.3);

        let json = serde_json::to_string(&params).expect("serialize process params");

        let kappa_idx = json.find("\"kappa\"").expect("kappa key present");
        let sigma_idx = json.find("\"sigma_v\"").expect("sigma_v key present");
        let theta_idx = json.find("\"theta\"").expect("theta key present");
        assert!(kappa_idx < sigma_idx);
        assert!(sigma_idx < theta_idx);
    }

    #[test]
    fn test_process_params_rejects_non_square_correlation_metadata() {
        let params = ProcessParams::new("MultiGBM").with_correlation(vec![1.0, 0.5, 0.5]);

        let err = params
            .validate()
            .expect_err("non-square correlation metadata should be rejected");
        assert!(err.to_string().contains("square"));
    }

    #[test]
    fn test_process_params_rejects_factor_name_dimension_mismatch() {
        let params = ProcessParams::new("MultiGBM")
            .with_correlation(vec![1.0, 0.5, 0.5, 1.0])
            .with_factors(vec![
                "spot_0".to_string(),
                "spot_1".to_string(),
                "spot_2".to_string(),
            ]);

        let err = params
            .validate()
            .expect_err("factor name mismatch should be rejected");
        assert!(err.to_string().contains("factor_names"));
    }

    #[test]
    fn test_path_dataset() {
        let process_params = ProcessParams::new("GBM");
        let mut dataset = PathDataset::new(
            100,
            PathSamplingMethod::RandomSample {
                count: 10,
                seed: 42,
            },
            process_params,
        );

        assert_eq!(dataset.num_paths_total, 100);
        assert_eq!(dataset.num_captured(), 0);
        assert!(!dataset.is_complete());

        let path1 = SimulatedPath::new(0);
        let path2 = SimulatedPath::new(1);
        dataset.add_path(path1);
        dataset.add_path(path2);

        assert_eq!(dataset.num_captured(), 2);
        assert_eq!(dataset.sampling_ratio(), 0.02);
    }

    #[test]
    fn test_sampling_method_display() {
        let all = PathSamplingMethod::All;
        assert_eq!(all.to_string(), "all");

        let sample = PathSamplingMethod::RandomSample {
            count: 100,
            seed: 42,
        };
        assert_eq!(sample.to_string(), "random_sample(n=100, seed=42)");
    }

    #[test]
    fn test_state_var_keys_extraction() {
        // Test with factor names in metadata
        let mut process_params = ProcessParams::new("GBM");
        process_params.factor_names = vec!["spot".to_string(), "variance".to_string()];
        let mut dataset = PathDataset::new(10, PathSamplingMethod::All, process_params);

        let mut path1 = SimulatedPath::new(0);
        let mut state1 = SmallVec::new();
        state1.push(100.0); // spot
        state1.push(0.04); // variance
        let point1 = PathPoint::with_state(0, 0.0, state1);
        path1.add_point(point1);
        dataset.add_path(path1);

        let keys = dataset.state_var_keys();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], "spot");
        assert_eq!(keys[1], "variance");

        // Test with no factor names (should generate generic names)
        let process_params2 = ProcessParams::new("GBM");
        let mut dataset2 = PathDataset::new(10, PathSamplingMethod::All, process_params2);

        let mut path2 = SimulatedPath::new(0);
        let mut state2 = SmallVec::new();
        state2.push(105.0); // state_0
        state2.push(0.03); // state_1
        let point2 = PathPoint::with_state(0, 0.0, state2);
        path2.add_point(point2);
        dataset2.add_path(path2);

        let keys2 = dataset2.state_var_keys();
        assert_eq!(keys2.len(), 2);
        assert_eq!(keys2[0], "state_0");
        assert_eq!(keys2[1], "state_1");
    }
}
