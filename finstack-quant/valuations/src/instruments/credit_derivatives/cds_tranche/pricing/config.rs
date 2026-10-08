//! Numerical pricing, expected-loss, and sensitivity helpers for CDS tranches.
//!
use crate::cashflow::primitives::CashFlow;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::{Error as CoreError, Result as CoreResult};
use finstack_quant_models::correlation::copula::{Copula, CopulaSpec};
use std::sync::OnceLock;

// Pricing Model Constants
//
// These are fixed parameters of the tranche pricing model; callers choose only
// the copula via [`CdsTranchePricer::with_copula`].

/// Absolute error budget for one capped pool expectation, in portfolio
/// fractions. Tail truncation and nested-factor quadrature each consume a
/// share of this budget. Student-t pricing uses the copula's product Gauss
/// rule and ignores it.
pub(super) const INTEGRATION_TOLERANCE: f64 = 1e-10;

/// Maximum adaptive subdivisions per conditioning interval. The summed
/// absolute quadrature error must meet the total budget, or pricing fails.
pub(super) const INTEGRATION_MAX_DEPTH: usize = 20;

/// Minimum correlation value for numerical stability (avoids division by near-zero)
pub(super) const MIN_CORRELATION: f64 = 0.01;

/// Maximum correlation value for numerical stability (avoids degenerate cases)
pub(super) const MAX_CORRELATION: f64 = 0.99;

/// Boundary width for smooth correlation clamping transitions
pub(super) const CORR_BOUNDARY_WIDTH: f64 = 0.005;

/// Grid step for the heterogeneous exact convolution (fraction of portfolio
/// notional). Grid error is separate from the factor integration tolerance.
pub(super) const GRID_STEP: f64 = 0.001;

/// Settlement lag for index CDS (T+1 since Big Bang 2009)
pub(super) const INDEX_SETTLEMENT_DAYS: i32 = 1;

/// Settlement lag for bespoke CDS tranches (T+3 per ISDA)
pub(super) const BESPOKE_SETTLEMENT_DAYS: i32 = 3;

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

/// Hard cap on convolution PMF points. Exceeding it returns an error;
/// an exact calculation never silently switches to an approximation.
pub(super) const MAX_GRID_POINTS: usize = 200_000;

/// Maximum conservative issuer × grid-point visits per conditional
/// convolution. This resource limit is not an approximation error budget and
/// does not bound the number of adaptive factor quadrature evaluations.
pub(super) const MAX_CONVOLUTION_WORK: usize = 5_000_000;

/// Tolerance for par spread solver convergence
pub(super) const PAR_SPREAD_TOLERANCE: f64 = 1e-6;

/// Copula-based pricing engine for CDS tranches.
///
/// Supports the Gaussian, Student-t, random-factor-loading and multi-factor
/// copulas selected through [`CopulaSpec`]. Recovery is the index (or
/// per-issuer) recovery rate.
///
/// The copula instance is constructed lazily and cached for the pricer's
/// lifetime. Gaussian, RFL, and multi-factor expected losses use adaptive
/// integration over every conditioning factor with explicit tail and
/// convergence budgets. Student-t uses the copula's product Gauss rule.
///
/// Default settings follow ISDA standard model conventions: mid-period
/// protection timing, accrual-on-default in the premium leg, T+1 settlement
/// for index tranches and T+3 for bespoke tranches.
///
/// The copula specification is validated by [`CdsTranchePricer::with_copula`]
/// and remains immutable for the pricer's lifetime, so the cached copula
/// cannot drift from the specification used by uncached calculations.
pub struct CdsTranchePricer {
    pub(super) copula_spec: CopulaSpec,
    pub(super) copula_cache: OnceLock<Box<dyn Copula + Send + Sync>>,
}

/// Validate a copula specification before constructing a pricer.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the Student-t
/// degrees of freedom or the random-factor-loading volatility are outside
/// their documented ranges.
pub(super) fn validate_copula_spec(spec: &CopulaSpec) -> CoreResult<()> {
    match spec {
        CopulaSpec::Gaussian | CopulaSpec::MultiFactor => Ok(()),
        CopulaSpec::StudentT { degrees_of_freedom } => CopulaSpec::student_t(*degrees_of_freedom)
            .map(|_| ())
            .map_err(|error| CoreError::Validation(error.to_string())),
        CopulaSpec::RandomFactorLoading { loading_volatility } => {
            let value = *loading_volatility;
            if value.is_finite() && (0.0..=0.5).contains(&value) {
                Ok(())
            } else {
                Err(CoreError::Validation(format!(
                    "copula_spec.loading_volatility must be finite and in [0, 0.5], got {value}"
                )))
            }
        }
    }
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

impl Default for CdsTranchePricer {
    fn default() -> Self {
        Self::new()
    }
}

impl CdsTranchePricer {
    /// Copula specification used for every valuation performed by this pricer.
    pub fn get_copula_spec(&self) -> &CopulaSpec {
        &self.copula_spec
    }
}
