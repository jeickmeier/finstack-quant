//! Numerical pricing, expected-loss, and sensitivity helpers for CDS tranches.
//!
use crate::cashflow::primitives::CashFlow;
use finstack_quant_core::dates::{Date, StubKind};
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::types::Percentage;
use finstack_quant_core::{Error as CoreError, Result as CoreResult};
use finstack_quant_models::correlation::copula::{Copula, CopulaSpec};
use finstack_quant_models::correlation::recovery::RecoverySpec;
use finstack_quant_models::correlation::Result as CorrelationResult;
use std::sync::OnceLock;

// Default Configuration Constants

/// Absolute error budget for one capped pool expectation, in portfolio fractions.
pub(super) const DEFAULT_INTEGRATION_TOLERANCE: f64 = 1e-10;

/// Minimum correlation value for numerical stability (avoids division by near-zero)
const DEFAULT_MIN_CORRELATION: f64 = 0.01;

/// Maximum correlation value for numerical stability (avoids degenerate cases)
const DEFAULT_MAX_CORRELATION: f64 = 0.99;

/// Default bump size for CS01 calculation in basis points
const DEFAULT_CS01_BUMP_SIZE: f64 = 1.0;

/// Default correlation bump for Correlation01 calculation (absolute, e.g., 0.01 = 1%)
const DEFAULT_CORR_BUMP_ABS: f64 = 0.01;

/// Boundary width for smooth correlation clamping transitions
const DEFAULT_CORR_BOUNDARY_WIDTH: f64 = 0.005;

/// Grid step for exact convolution method (fraction of portfolio notional)
const DEFAULT_GRID_STEP: f64 = 0.001;

/// Default settlement lag for index CDS (T+1 since Big Bang 2009)
const DEFAULT_INDEX_SETTLEMENT_LAG: i32 = 1;

/// Default settlement lag for bespoke CDS tranches (T+3 per ISDA)
const DEFAULT_BESPOKE_SETTLEMENT_LAG: i32 = 3;

// Numerical-Stability Constants
//
// These epsilons/limits are never varied by callers; they are fixed
// numerical-stability parameters of the pricing model.

/// Numerical tolerance for integration convergence and boundary checks
pub(super) const NUMERICAL_TOLERANCE: f64 = 1e-10;

/// Clip parameter for CDF arguments to prevent overflow (±10 sigma)
pub(super) const CDF_CLIP: f64 = 10.0;

/// Probability clamp epsilon to avoid 0/1 extremes in probits/CDFs
pub(super) const PROBABILITY_CLIP: f64 = 1e-12;

/// Homogeneity-detection tolerance for the heterogeneous EL path.
///
/// A bespoke pool is treated as "effectively homogeneous" — and routed to the
/// faster homogeneous binomial path — when its per-issuer default
/// probabilities, LGDs *and* weights are each uniform to within this
/// absolute tolerance. The SAME tolerance is applied to all three vectors so
/// the model-branch switch is consistent: a pool that is uniform in PD must
/// not be judged heterogeneous in LGD (or vice versa) merely because the
/// checks used different epsilons.
///
/// `1e-9` is appropriate because PD, LGD and weight are all dimensionless
/// quantities in `[0, 1]`: it is tight enough that any genuinely
/// heterogeneous pool (issuer dispersion ≫ `1e-9`) is detected, and loose
/// enough to absorb the floating-point round-off in equal weights built as
/// `1.0 / n` (worst-case `n·ε ≈ 125 · 2.2e-16 ≈ 3e-14 ≪ 1e-9`).
pub(super) const HOMOGENEITY_TOLERANCE: f64 = 1e-9;

/// Minimum grid step to avoid degenerate convolution buckets
pub(super) const GRID_STEP_MIN: f64 = 1e-6;

/// Hard cap on convolution PMF points before falling back to the
/// moment-matched normal approximation
pub(super) const MAX_GRID_POINTS: usize = 200_000;

/// Maximum iterations for par spread solver
pub(super) const PAR_SPREAD_MAX_ITER: usize = 50;

/// Tolerance for par spread solver convergence
pub(super) const PAR_SPREAD_TOLERANCE: f64 = 1e-6;

/// Parameters for the CDS Tranche pricing model.
///
/// This configuration controls all aspects of tranche pricing including:
/// - Copula model selection (Gaussian, Student-t, RFL, Multi-factor)
/// - Recovery model (constant or stochastic)
/// - Numerical integration parameters
/// - Risk metric bump sizes and methods
/// - ISDA convention settings
/// - Settlement and schedule generation
///
/// # ISDA Compliance
///
/// Default settings follow ISDA standard model conventions:
/// - Mid-period protection timing (`mid_period_protection = true`)
/// - Act/360 day count (set on instrument)
/// - Quarterly payment frequency on IMM dates
/// - T+1 settlement for index CDS
///
/// # Extended Models
///
/// The pricer supports multiple copula and recovery models:
///
/// ## Copula Models
/// - **Gaussian** (default): Standard one-factor, no tail dependence
/// - **Student-t**: Fat tails, captures tail dependence
/// - **RFL**: Random factor loading, stochastic correlation
/// - **Multi-factor**: Sector-specific correlation structure
///
/// ## Recovery Models
/// - **Constant** (default): Fixed recovery rate
/// - **Stochastic**: Recovery correlated with market factor
#[derive(Debug, Clone)]
pub struct CDSTranchePricerConfig {
    // Model Selection
    /// Copula model specification (default: Gaussian)
    pub copula_spec: CopulaSpec,
    /// Recovery model specification (default: use index recovery rate)
    pub recovery_spec: Option<RecoverySpec>,

    /// Absolute numerical integration budget in portfolio-notional fractions
    /// (default `1e-10`). Must lie in `[1e-12, 1e-4]`. Tail truncation and
    /// nested-factor quadrature each consume a share of this budget.
    ///
    /// Student-t pricing uses the copula's product Gauss rule by default and
    /// ignores this budget unless
    /// [`Self::adaptive_student_t_integration`] is enabled.
    pub integration_tolerance: f64,
    /// Maximum adaptive subdivisions per conditioning interval (default 20).
    /// Zero permits only the initial refinement check. The summed absolute
    /// quadrature error must meet the total budget, or pricing fails.
    ///
    /// Student-t product-Gauss pricing ignores this depth.
    pub integration_max_depth: usize,
    /// When `true`, Student-t factor integrals use nested adaptive Simpson
    /// subject to [`Self::integration_tolerance`]. When `false` (default),
    /// Student-t uses the copula's fixed product Gauss–Laguerre ×
    /// Gauss–Hermite rule.
    pub adaptive_student_t_integration: bool,
    /// Whether to use issuer-specific curves if available
    pub use_issuer_curves: bool,
    /// Minimum correlation value for numerical stability
    pub min_correlation: f64,
    /// Maximum correlation value for numerical stability
    pub max_correlation: f64,

    // Risk Metric Parameters
    /// CS01 bump size in basis points, applied as a parallel par-spread bump
    /// with hazard-curve recalibration.
    pub cs01_bump_size: f64,
    /// Correlation bump for correlation delta calculation (absolute)
    pub corr_bump_abs: f64,

    // ISDA Convention Settings
    /// Whether to use mid-period discounting for protection leg (ISDA standard: true)
    pub mid_period_protection: bool,
    /// Whether to include accrual-on-default in the premium leg
    pub accrual_on_default_enabled: bool,
    /// Stub convention for schedule generation
    pub schedule_stub: StubKind,
    /// If true, generate ISDA coupon dates (IMM-20 schedule)
    pub use_isda_coupon_dates: bool,
    /// Settlement lag in business days for index CDS (default: 1 for Big Bang)
    pub index_settlement_lag: i32,
    /// Settlement lag in business days for bespoke tranches (default: 3 per ISDA)
    pub bespoke_settlement_lag: i32,

    // Numerical Stability
    /// Smooth boundary width for correlation clamping transitions
    pub corr_boundary_width: f64,

    // Heterogeneous Portfolio Settings
    /// Heterogeneous issuer method when issuer curves are available
    pub hetero_method: HeteroMethod,
    /// Grid step for exact convolution method (fraction of portfolio notional)
    pub grid_step: f64,
}

impl Default for CDSTranchePricerConfig {
    fn default() -> Self {
        Self {
            // Model selection
            copula_spec: CopulaSpec::default(),
            recovery_spec: None, // Use index recovery rate by default

            // Numerical integration
            integration_tolerance: DEFAULT_INTEGRATION_TOLERANCE,
            integration_max_depth: 20,
            adaptive_student_t_integration: false,
            use_issuer_curves: true,
            min_correlation: DEFAULT_MIN_CORRELATION,
            max_correlation: DEFAULT_MAX_CORRELATION,

            // Risk metrics
            cs01_bump_size: DEFAULT_CS01_BUMP_SIZE,
            corr_bump_abs: DEFAULT_CORR_BUMP_ABS,

            // ISDA conventions
            mid_period_protection: true, // ISDA standard
            accrual_on_default_enabled: true,
            schedule_stub: StubKind::ShortFront,
            use_isda_coupon_dates: false,
            index_settlement_lag: DEFAULT_INDEX_SETTLEMENT_LAG,
            bespoke_settlement_lag: DEFAULT_BESPOKE_SETTLEMENT_LAG,

            // Numerical stability
            corr_boundary_width: DEFAULT_CORR_BOUNDARY_WIDTH,

            // Heterogeneous portfolio
            hetero_method: HeteroMethod::NormalApprox,
            grid_step: DEFAULT_GRID_STEP,
        }
    }
}

impl CDSTranchePricerConfig {
    /// Validate numerical and model parameters before constructing a pricer.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when a parameter is
    /// non-finite, outside its documented range, or requests an unsupported
    /// quadrature rule.
    pub fn validate(&self) -> CoreResult<()> {
        match &self.copula_spec {
            CopulaSpec::Gaussian | CopulaSpec::MultiFactor => {}
            CopulaSpec::StudentT { degrees_of_freedom } => {
                CopulaSpec::student_t(*degrees_of_freedom)
                    .map_err(|error| CoreError::Validation(error.to_string()))?;
            }
            CopulaSpec::RandomFactorLoading { loading_volatility } => {
                validate_range(
                    "copula_spec.loading_volatility",
                    *loading_volatility,
                    0.0,
                    0.5,
                )?;
            }
        }
        if let Some(recovery) = &self.recovery_spec {
            recovery
                .validate()
                .map_err(|error| CoreError::Validation(error.to_string()))?;
        }
        validate_range(
            "integration_tolerance",
            self.integration_tolerance,
            1e-12,
            1e-4,
        )?;
        if self.integration_max_depth > 30 {
            return Err(CoreError::Validation(
                "integration_max_depth must not exceed 30".to_owned(),
            ));
        }
        validate_range("min_correlation", self.min_correlation, 0.0, 1.0)?;
        validate_range("max_correlation", self.max_correlation, 0.0, 1.0)?;
        if self.min_correlation >= self.max_correlation {
            return Err(CoreError::Validation(format!(
                "min_correlation {} must be less than max_correlation {}",
                self.min_correlation, self.max_correlation
            )));
        }
        validate_positive("cs01_bump_size", self.cs01_bump_size)?;
        validate_positive("corr_bump_abs", self.corr_bump_abs)?;
        validate_range("corr_boundary_width", self.corr_boundary_width, 0.0, 1.0)?;
        validate_positive("grid_step", self.grid_step)?;
        if self.index_settlement_lag < 0 || self.bespoke_settlement_lag < 0 {
            return Err(CoreError::Validation(format!(
                "settlement lags must be non-negative, got index={} bespoke={}",
                self.index_settlement_lag, self.bespoke_settlement_lag
            )));
        }
        Ok(())
    }

    /// Create configuration with Student-t copula.
    ///
    /// # Arguments
    /// * `df` - Degrees of freedom (typical: 4-10 for CDX)
    pub fn with_student_t_copula(mut self, df: f64) -> CorrelationResult<Self> {
        self.copula_spec = CopulaSpec::student_t(df)?;
        Ok(self)
    }

    /// Create configuration with Random Factor Loading copula.
    ///
    /// # Arguments
    /// * `loading_vol` - Loading volatility (typical: 0.05-0.20)
    pub fn with_rfl_copula(mut self, loading_vol: f64) -> Self {
        self.copula_spec = CopulaSpec::random_factor_loading(loading_vol);
        self
    }

    /// Create configuration with Random Factor Loading copula using typed volatility.
    pub fn with_rfl_copula_pct(mut self, loading_vol: Percentage) -> Self {
        self.copula_spec = CopulaSpec::random_factor_loading(loading_vol.as_decimal());
        self
    }

    /// Create configuration with the global-plus-sector two-factor copula.
    #[must_use]
    pub fn with_multi_factor_copula(mut self) -> Self {
        self.copula_spec = CopulaSpec::multi_factor();
        self
    }

    /// Enable stochastic recovery with market-standard calibration.
    ///
    /// Uses typical calibration from CDX equity tranche:
    /// - Mean: 40%, Vol: 25%, Correlation: +40% (canonical low-factor-stress
    ///   convention: recovery falls when the systematic factor falls)
    pub fn with_stochastic_recovery(mut self) -> Self {
        self.recovery_spec = Some(RecoverySpec::market_standard_stochastic());
        self
    }

    /// Enable stochastic recovery with custom parameters.
    ///
    /// # Arguments
    /// * `mean` - Mean recovery rate (typical: 0.40)
    /// * `vol` - Recovery volatility (typical: 0.20-0.30)
    /// * `corr` - Correlation with factor (typical: +0.30 to +0.50 under the
    ///   canonical low-factor-stress convention)
    pub fn with_custom_stochastic_recovery(mut self, mean: f64, vol: f64, corr: f64) -> Self {
        self.recovery_spec = Some(RecoverySpec::MarketCorrelated {
            mean_recovery: mean.clamp(0.0, 1.0),
            recovery_volatility: vol.clamp(0.0, 0.5),
            factor_correlation: corr.clamp(-1.0, 1.0),
        });
        self
    }

    /// Enable stochastic recovery with custom parameters using typed percentages.
    pub fn with_custom_stochastic_recovery_pct(
        mut self,
        mean: Percentage,
        vol: Percentage,
        corr: f64,
    ) -> Self {
        self.recovery_spec = Some(RecoverySpec::MarketCorrelated {
            mean_recovery: mean.as_decimal().clamp(0.0, 1.0),
            recovery_volatility: vol.as_decimal().clamp(0.0, 0.5),
            factor_correlation: corr.clamp(-1.0, 1.0),
        });
        self
    }

    /// Set constant recovery rate (overriding index recovery).
    pub fn with_constant_recovery(mut self, rate: f64) -> Self {
        self.recovery_spec = Some(RecoverySpec::Constant {
            rate: rate.clamp(0.0, 1.0),
        });
        self
    }

    /// Set constant recovery rate using a typed percentage.
    pub fn with_constant_recovery_pct(mut self, rate: Percentage) -> Self {
        self.recovery_spec = Some(RecoverySpec::Constant {
            rate: rate.as_decimal().clamp(0.0, 1.0),
        });
        self
    }

    /// Set the absolute integration error budget for capped pool expectations.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Absolute error budget in portfolio-notional fractions,
    ///   between `1e-12` and `1e-4`. Validated when constructing the pricer.
    ///   Student-t product-Gauss pricing ignores this budget unless
    ///   [`Self::with_adaptive_student_t_integration`] is enabled.
    #[must_use]
    pub fn with_integration_tolerance(mut self, tolerance: f64) -> Self {
        self.integration_tolerance = tolerance;
        self
    }

    /// Choose nested adaptive Simpson for Student-t factor integrals.
    ///
    /// The default Student-t path uses the copula's product Gauss rule.
    /// Enable this only when a caller must meet
    /// [`Self::integration_tolerance`] rather than the fixed quadrature
    /// accuracy.
    ///
    /// # Arguments
    ///
    /// * `enabled` - `true` to use nested adaptive Simpson over the Student-t
    ///   mixing variable and systematic factor; `false` (default) to use
    ///   product Gauss–Laguerre × Gauss–Hermite quadrature.
    #[must_use]
    pub fn with_adaptive_student_t_integration(mut self, enabled: bool) -> Self {
        self.adaptive_student_t_integration = enabled;
        self
    }
}

fn validate_positive(name: &str, value: f64) -> CoreResult<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "{name} must be finite and positive, got {value}"
        )))
    }
}

fn validate_range(name: &str, value: f64, min: f64, max: f64) -> CoreResult<()> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "{name} must be finite and in [{min}, {max}], got {value}"
        )))
    }
}

/// Heterogeneous expected loss evaluation method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeteroMethod {
    /// Moment-matched normal (CLT) approximation for heterogeneous pool loss.
    ///
    /// Matches the conditional loss mean and variance with a Gaussian; can
    /// place bounded probability mass below zero. Renamed from the misleading
    /// `Spa` (2026-07 credit-derivatives audit M5): this is **not** a
    /// CGF-based saddle-point method. Pools at or below
    /// `credit::SMALL_POOL_THRESHOLD` constituents are always routed to
    /// exact convolution regardless of this setting; the measured bias of
    /// this approximation above that threshold is ≤ ~0.2% of tranche PV
    /// (see the threshold's doc table).
    NormalApprox,
    /// Exact convolution method (slower but more accurate)
    ExactConvolution,
}

/// Copula-based pricing engine for CDS tranches.
///
/// Supports multiple copula models (Gaussian, Student-t, RFL, Multi-factor)
/// and optional stochastic recovery for market-standard tranche pricing.
///
/// The copula instance is constructed lazily and cached for the pricer's
/// lifetime. Gaussian, RFL, and multi-factor expected losses use adaptive
/// integration over every conditioning factor with explicit tail and
/// convergence budgets. Student-t uses the copula's product Gauss rule
/// unless adaptive integration is enabled.
///
/// Configuration is validated by [`CDSTranchePricer::with_params`] and remains
/// immutable for the pricer's lifetime. The cached copula
/// therefore cannot drift from the settings used by uncached calculations.
pub struct CDSTranchePricer {
    pub(super) params: CDSTranchePricerConfig,
    pub(super) copula_cache: OnceLock<Box<dyn Copula + Send + Sync>>,
}

/// Which per-name exposure a capped pool expectation integrates over.
///
/// The copula machinery computes `E[min(Σᵢ wᵢ·eᵢ·Bᵢ, cap)]`; the exposure
/// `eᵢ` is either the loss given default (loss side, erodes the tranche from
/// the bottom) or the recovered notional (recovery side, amortizes the
/// detachment from the top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PoolExposure {
    /// Per-name exposure `1 − Rᵢ` — expected tranche LOSS.
    Loss,
    /// Per-name exposure `Rᵢ` — expected capped RECOVERED notional.
    Recovery,
}

/// One point of the projected tranche erosion curve.
#[derive(Debug, Clone, Copy)]
pub(super) struct ElWdPoint {
    /// Payment date.
    pub(super) date: Date,
    /// Cumulative expected LOSS as a fraction of tranche notional.
    pub(super) el_fraction: f64,
    /// Cumulative expected senior-side recovery WRITEDOWN as a fraction of
    /// tranche notional. `el_fraction + wd_fraction ≤ 1`.
    pub(super) wd_fraction: f64,
}

/// Effective tranche structure after realized losses and recoveries.
#[derive(Debug, Clone, Copy)]
pub(super) struct EffectiveStructure {
    /// Attachment re-normalized to the surviving pool, in `[0, 1]`.
    pub(super) eff_attach: f64,
    /// Detachment re-normalized to the surviving pool, in `[0, 1]`.
    pub(super) eff_detach: f64,
    /// Surviving pool fraction `1 − defaulted_notional` (NOT `1 − loss`).
    pub(super) pool_factor: f64,
}

pub(super) type ProjectionInputs = (
    std::sync::Arc<CreditIndexData>,
    Date,
    Vec<Date>,
    Vec<ElWdPoint>,
);

/// Where to discount a projected tranche cashflow.
///
/// All discounting is performed on the DISCOUNT curve's own time axis
/// (its day count and base date) as a relative factor from `as_of`,
/// mirroring the single-name CDS `df_asof_to` convention. The hazard
/// curve's axis is never used for discount lookups.
#[derive(Debug, Clone, Copy)]
pub(super) enum DiscountAt {
    /// Discount at the cashflow's payment date:
    /// `df_between_dates(as_of, cashflow.date)`.
    PaymentDate,
    /// Discount at a point inside the coupon period `[start, cashflow.date]`:
    /// the time is interpolated at `fraction` of the period measured on the
    /// discount curve's axis, then divided by `df` at `as_of` (relative DF).
    /// Used for mid-period protection timing.
    WithinPeriod {
        /// Period start date.
        start: Date,
        /// Survival-weighted default-time fraction of the period, in `[0, 1]`.
        fraction: f64,
    },
}

#[derive(Debug, Clone)]
pub(super) struct ProjectedDiscountedRow {
    pub(super) cashflow: CashFlow,
    pub(super) discount_at: DiscountAt,
}

impl Default for CDSTranchePricer {
    fn default() -> Self {
        Self::new()
    }
}

impl CDSTranchePricer {
    /// Get the current configuration.
    pub fn get_config(&self) -> &CDSTranchePricerConfig {
        &self.params
    }
}
