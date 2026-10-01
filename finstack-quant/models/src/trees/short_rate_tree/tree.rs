use std::sync::Arc;

use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::{Error, Result};

use super::black_karasinski::BkTrinomialLattice;
use super::{ShortRateModel, ShortRateTreeConfig};

/// Result of short-rate tree calibration with quality metrics.
///
/// Provides diagnostic information about calibration quality, allowing
/// users to assess whether the tree is suitable for their use case.
#[derive(Debug, Clone, Default)]
pub struct TreeCalibrationResult {
    /// Maximum calibration error in basis points.
    pub max_error_bp: f64,
    /// Step at which maximum error occurred.
    pub max_error_step: usize,
    /// Number of steps where the solver failed and fallback was used.
    pub fallback_count: usize,
    /// Whether calibration completed successfully.
    pub converged: bool,
}

impl TreeCalibrationResult {
    /// Returns true if calibration quality is acceptable (max error < 1bp, no fallbacks).
    #[must_use]
    pub fn is_acceptable(&self) -> bool {
        self.converged && self.max_error_bp < 1.0 && self.fallback_count == 0
    }
}

/// Short-rate tree for valuing bonds with embedded options
#[derive(Debug, Clone)]
pub struct ShortRateTree {
    pub(super) config: ShortRateTreeConfig,
    /// Calibrated short rates at each node: `rates[step][node]`
    pub(super) rates: Arc<Vec<Vec<f64>>>,
    /// Time steps in years
    pub(super) time_steps: Vec<f64>,
    /// Calibration quality metrics (populated after calibration).
    pub(super) calibration_quality: Option<TreeCalibrationResult>,
    /// Trinomial Black-Karasinski lattice (set when BDT model has κ ≠ 0).
    pub(super) bk_trinomial: Option<BkTrinomialLattice>,
}

impl ShortRateTree {
    /// Create a new short-rate tree with the given configuration.
    pub fn new(config: ShortRateTreeConfig) -> Self {
        Self {
            config,
            rates: Arc::new(Vec::new()),
            time_steps: Vec::new(),
            calibration_quality: None,
            bk_trinomial: None,
        }
    }

    /// Returns the calibration result if calibration has been performed.
    ///
    /// # Returns
    ///
    /// - `Some(TreeCalibrationResult)` with quality metrics if calibrated
    /// - `None` if not yet calibrated
    #[must_use]
    pub fn calibration_result(&self) -> Option<&TreeCalibrationResult> {
        self.calibration_quality.as_ref()
    }

    /// Calibrate the tree to match a given discount curve.
    ///
    /// Fitting commits atomically. A failed calibration preserves the previous
    /// rates, time coordinates, transition lattice, and calibration diagnostics.
    ///
    /// # Arguments
    ///
    /// * `discount_curve` - Risk-free discount curve the lattice must reprice
    ///   at every step (Arrow-Debreu forward induction). Discount factors on
    ///   the calibration grid must be finite and strictly positive.
    /// * `time_to_maturity` - Positive lattice horizon in years; the step
    ///   width is `time_to_maturity / steps`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `steps == 0`, if `time_to_maturity`
    /// is not finite and positive, if the configured model rejects its
    /// parameters (e.g. Ho-Lee with non-zero mean reversion), a discount
    /// target is non-finite or non-positive, or the calibrated lattice fails
    /// to reprice the curve within `curve_fit_tolerance_bp`.
    pub fn calibrate(
        &mut self,
        discount_curve: &dyn Discounting,
        time_to_maturity: f64,
    ) -> Result<()> {
        if self.config.steps == 0 {
            return Err(Error::Validation(
                "short-rate tree requires at least one step".into(),
            ));
        }
        if !time_to_maturity.is_finite() || time_to_maturity <= 0.0 {
            return Err(Error::Validation(format!(
                "short-rate tree requires a finite, positive time to maturity, got {time_to_maturity}"
            )));
        }
        for (name, value) in [
            ("volatility", self.config.volatility),
            ("mean_reversion", self.config.mean_reversion),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(Error::Validation(format!(
                    "short-rate tree {name} must be finite and non-negative, got {value}"
                )));
            }
        }
        if !self.config.curve_fit_tolerance_bp.is_finite()
            || self.config.curve_fit_tolerance_bp <= 0.0
        {
            return Err(Error::Validation(format!(
                "short-rate tree curve-fit tolerance must be finite and positive, got {}",
                self.config.curve_fit_tolerance_bp
            )));
        }

        let dt = time_to_maturity / self.config.steps as f64;
        if !dt.is_finite() || dt <= 0.0 {
            return Err(Error::Validation(
                "short-rate tree step size must be finite and positive".into(),
            ));
        }
        let mut candidate = Self::new(self.config.clone());
        candidate.time_steps = (0..=self.config.steps).map(|i| i as f64 * dt).collect();
        for &time in &candidate.time_steps {
            let target = discount_curve.df(time);
            if !target.is_finite() || target <= 0.0 {
                return Err(Error::Validation(format!(
                    "short-rate tree discount factor must be finite and positive at time {time}, got {target}"
                )));
            }
        }

        let mut rates = vec![Vec::new(); self.config.steps + 1];

        match self.config.model {
            ShortRateModel::HoLee => candidate.calibrate_ho_lee(&mut rates, discount_curve, dt)?,
            ShortRateModel::BlackDermanToy => {
                let kappa = self.config.mean_reversion;
                if kappa < 0.0 {
                    return Err(Error::Validation(format!(
                        "Black-Karasinski mean reversion must be non-negative, got {kappa}"
                    )));
                }
                if kappa.abs() < 1e-12 {
                    // κ = 0: standard binomial BDT calibration.
                    candidate.calibrate_bdt(&mut rates, discount_curve, dt)?;
                } else {
                    // κ ≠ 0: genuine trinomial Black-Karasinski lattice in
                    // x = ln r .
                    candidate.calibrate_bk_trinomial(&mut rates, discount_curve, dt, kappa)?;
                }
            }
        }

        candidate.rates = Arc::new(rates);
        *self = candidate;

        Ok(())
    }

    /// Get the short rate at a specific node.
    ///
    /// # Node Ordering
    ///
    /// The ordering convention differs by model:
    ///
    /// | Model | Node 0 | Node N |
    /// |-------|--------|--------|
    /// | Ho-Lee | **lowest** rate | **highest** rate |
    /// | BDT (κ = 0, binomial) | **highest** rate (`α·u^(n-1)`) | **lowest** rate (`α·u^(-(n-1))`) |
    /// | BK (κ ≠ 0, trinomial) | **lowest** rate (j = −j_max) | **highest** rate (j = +j_max) |
    pub fn rate_at_node(&self, step: usize, node: usize) -> Result<f64> {
        if step >= self.rates.len() || node >= self.rates[step].len() {
            return Err(Error::internal(format!(
                "short-rate tree node out of bounds: step={step}, node={node}"
            )));
        }
        Ok(self.rates[step][node])
    }

    /// Get time at step
    pub fn time_at_step(&self, step: usize) -> Result<f64> {
        if step >= self.time_steps.len() {
            return Err(Error::internal(format!(
                "short-rate tree time step out of bounds: step={step}"
            )));
        }
        Ok(self.time_steps[step])
    }

    /// Validate a pricing request against the grid whose rates were calibrated.
    pub(super) fn validate_pricing_horizon(&self, time_to_maturity: f64) -> Result<f64> {
        let calibrated_horizon =
            self.time_steps.last().copied().ok_or_else(|| {
                Error::internal("short-rate tree must be calibrated before pricing")
            })?;
        let tolerance = 1e-12_f64.max(calibrated_horizon.abs() * 1e-10);
        if !time_to_maturity.is_finite()
            || time_to_maturity <= 0.0
            || (time_to_maturity - calibrated_horizon).abs() > tolerance
        {
            return Err(Error::Validation(format!(
                "short-rate pricing horizon {time_to_maturity} does not match the calibrated horizon {calibrated_horizon}"
            )));
        }
        Ok(calibrated_horizon)
    }

    pub(super) fn validate_lattice_geometry(&self) -> Result<()> {
        if self.rates.len() != self.config.steps + 1 {
            return Err(Error::internal(format!(
                "short-rate tree lattice geometry mismatch: expected {} rate rows, got {}",
                self.config.steps + 1,
                self.rates.len()
            )));
        }

        // Black-Karasinski trinomial lattice: width grows 2·step+1 until the
        // j_max cap, then stays at 2·j_max+1. Binomial lattices grow step+1.
        let expected_width = |step: usize| match &self.bk_trinomial {
            Some(lattice) => 2 * step.min(lattice.j_max) + 1,
            None => step + 1,
        };
        for (step, rates_at_step) in self.rates.iter().enumerate() {
            let expected = expected_width(step);
            if rates_at_step.len() != expected {
                return Err(Error::internal(format!(
                    "short-rate tree lattice geometry mismatch: step {} expected {} nodes, got {}",
                    step,
                    expected,
                    rates_at_step.len()
                )));
            }
        }

        Ok(())
    }
}
