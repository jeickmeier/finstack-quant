use crate::cashflow::primitives::CashFlow;
use crate::correlation::copula::{Copula, CopulaSpec};
use crate::correlation::recovery::RecoverySpec;
use crate::correlation::Result as CorrelationResult;
use finstack_core::dates::{Date, StubKind};
use finstack_core::market_data::term_structures::CreditIndexData;
use finstack_core::math::GaussHermiteQuadrature;
use finstack_core::types::Percentage;
use std::sync::OnceLock;

// ============================================================================
// Default Configuration Constants
// ============================================================================

/// Default quadrature order for Gauss-Hermite integration.
///
/// Industry standard (QuantLib, Bloomberg) uses 20-50 points.
/// 7 points is insufficient for accurate resolution of:
/// - Step-function-like integrands at extreme correlations
/// - Student-t heavy tails
pub(super) const DEFAULT_QUADRATURE_ORDER: u8 = 20;

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

// ============================================================================
// Numerical-Stability Constants
//
// These epsilons/limits are never varied by callers; they are fixed
// numerical-stability parameters of the pricing model.
// ============================================================================

/// Numerical tolerance for integration convergence and boundary checks
pub(super) const NUMERICAL_TOLERANCE: f64 = 1e-10;

/// Clip parameter for CDF arguments to prevent overflow (±10 sigma)
pub(super) const CDF_CLIP: f64 = 10.0;

/// Lower correlation threshold for adaptive integration (below this, use higher order).
/// Rationale: Near ρ=0, conditional probability is highly sensitive to market factor.
pub(super) const ADAPTIVE_INTEGRATION_LOW: f64 = 0.05;

/// Upper correlation threshold for adaptive integration (above this, use higher order).
/// Rationale: Near ρ=1, integrand approaches step function requiring more points.
pub(super) const ADAPTIVE_INTEGRATION_HIGH: f64 = 0.95;

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

/// LGD floor to avoid zero exposure in corner cases
pub(super) const LGD_FLOOR: f64 = 1e-6;

/// Minimum grid step to avoid degenerate convolution buckets
pub(super) const GRID_STEP_MIN: f64 = 1e-6;

/// Hard cap on convolution PMF points before falling back to SPA
pub(super) const MAX_GRID_POINTS: usize = 200_000;

/// Maximum iterations for par spread solver
pub(super) const PAR_SPREAD_MAX_ITER: usize = 50;

/// Tolerance for par spread solver convergence
pub(super) const PAR_SPREAD_TOLERANCE: f64 = 1e-6;

// ============================================================================
// Helper Functions
// ============================================================================

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
    // ========================================================================
    // Model Selection
    // ========================================================================
    /// Copula model specification (default: Gaussian)
    pub copula_spec: CopulaSpec,
    /// Recovery model specification (default: use index recovery rate)
    pub recovery_spec: Option<RecoverySpec>,
    /// Whether to validate base correlation for arbitrage-free conditions
    pub validate_arbitrage_free: bool,
    /// Whether to enforce expected loss monotonicity in the EL curve.
    ///
    /// When `true` (default), if a computed EL value is less than the previous
    /// date's EL (which can occur due to base correlation model inconsistencies),
    /// it will be clamped to the previous value to ensure monotonicity.
    /// This prevents small arbitrage in leg PV calculations.
    pub enforce_el_monotonicity: bool,

    // ========================================================================
    // Numerical Integration
    // ========================================================================
    /// Number of quadrature points for numerical integration (5, 7, or 10)
    pub quadrature_order: u8,
    /// Whether to use issuer-specific curves if available
    pub use_issuer_curves: bool,
    /// Minimum correlation value for numerical stability
    pub min_correlation: f64,
    /// Maximum correlation value for numerical stability
    pub max_correlation: f64,

    // ========================================================================
    // Risk Metric Parameters
    // ========================================================================
    /// CS01 bump size, applied as an additive hazard-rate basis-point bump.
    pub cs01_bump_size: f64,
    /// Correlation bump for correlation delta calculation (absolute)
    pub corr_bump_abs: f64,

    // ========================================================================
    // ISDA Convention Settings
    // ========================================================================
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

    // ========================================================================
    // Numerical Stability
    // ========================================================================
    /// Smooth boundary width for correlation clamping transitions
    pub corr_boundary_width: f64,

    // ========================================================================
    // Heterogeneous Portfolio Settings
    // ========================================================================
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
            validate_arbitrage_free: true,
            enforce_el_monotonicity: true, // Prevent EL from decreasing over time

            // Numerical integration
            quadrature_order: DEFAULT_QUADRATURE_ORDER,
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
            hetero_method: HeteroMethod::Spa,
            grid_step: DEFAULT_GRID_STEP,
        }
    }
}

impl CDSTranchePricerConfig {
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

    /// Create configuration with multi-factor copula.
    ///
    /// # Arguments
    /// * `num_factors` - Number of systematic factors
    pub fn with_multi_factor_copula(mut self, num_factors: usize) -> Self {
        self.copula_spec = CopulaSpec::multi_factor(num_factors);
        self
    }

    /// Enable stochastic recovery with market-standard calibration.
    ///
    /// Uses typical calibration from CDX equity tranche:
    /// - Mean: 40%, Vol: 25%, Correlation: -40%
    pub fn with_stochastic_recovery(mut self) -> Self {
        self.recovery_spec = Some(RecoverySpec::market_standard_stochastic());
        self
    }

    /// Enable stochastic recovery with custom parameters.
    ///
    /// # Arguments
    /// * `mean` - Mean recovery rate (typical: 0.40)
    /// * `vol` - Recovery volatility (typical: 0.20-0.30)
    /// * `corr` - Correlation with factor (typical: -0.30 to -0.50)
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

    /// Enable or disable arbitrage-free validation of base correlation.
    pub fn with_arbitrage_validation(mut self, enabled: bool) -> Self {
        self.validate_arbitrage_free = enabled;
        self
    }

    /// Set quadrature order for numerical integration.
    pub fn with_quadrature_order(mut self, order: u8) -> Self {
        self.quadrature_order = order;
        self
    }
}

/// Heterogeneous expected loss evaluation method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeteroMethod {
    /// Genuine saddle-point approximation (SPA): the conditional tranchelet
    /// loss `E[min(L,K) | Z]` is evaluated from the exact conditional
    /// cumulant-generating function via a Lugannani-Rice / Antonov-Mechkov-
    /// Misirpashaev saddle-point expansion. Captures the true skew of the
    /// loss distribution and never places mass on `L < 0`.
    Spa,
    /// Exact convolution method (slower but more accurate)
    ExactConvolution,
}

/// Copula-based pricing engine for CDS tranches.
///
/// Supports multiple copula models (Gaussian, Student-t, RFL, Multi-factor)
/// and optional stochastic recovery for market-standard tranche pricing.
///
/// The copula instance and quadrature table are constructed lazily on first use
/// and cached for the pricer's lifetime. Heterogeneous EL evaluation calls into
/// copula dispatch and quadrature selection from hot integration loops.
///
/// **Cache invariant:** `params.copula_spec` and `params.quadrature_order` must
/// not be mutated after the first call to `Self::copula` or quadrature
/// selection. To change either, construct a new pricer via `Self::with_params`.
/// Other config fields (`grid_step`, `hetero_method`, `use_issuer_curves`, etc.)
/// are *not* cached and may be mutated freely.
pub struct CDSTranchePricer {
    pub(super) params: CDSTranchePricerConfig,
    pub(super) copula_cache: OnceLock<Box<dyn Copula + Send + Sync>>,
    pub(super) quadrature_cache: OnceLock<GaussHermiteQuadrature>,
}

pub(super) type ProjectionInputs = (
    std::sync::Arc<CreditIndexData>,
    Date,
    Vec<Date>,
    Vec<(Date, f64)>,
);

#[derive(Debug, Clone)]
pub(super) struct ProjectedDiscountedRow {
    pub(super) cashflow: CashFlow,
    pub(super) discount_time: Option<f64>,
}

impl Default for CDSTranchePricer {
    fn default() -> Self {
        Self::new()
    }
}

impl CDSTranchePricer {
    /// Get the current configuration.
    pub fn config(&self) -> &CDSTranchePricerConfig {
        &self.params
    }
}
