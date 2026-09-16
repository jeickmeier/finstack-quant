//! Revolving credit facility types and instrument trait implementations.
//!
//! Defines the `RevolvingCredit` instrument with support for deterministic and
//! stochastic cashflow modeling. Supports standard fee structures (upfront,
//! commitment, usage, and facility fees) and both fixed and floating rate bases.

use finstack_quant_core::dates::{Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Bps, CurveId, InstrumentId, Rate};
use rust_decimal::Decimal;

use crate::cashflow::builder::{evaluate_fee_tiers, FeeTier, FloatingRateSpec};
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use rust_decimal::prelude::ToPrimitive;

/// Revolving credit facility instrument.
///
/// Models a credit facility with draws/repayments, interest payments on drawn
/// amounts, and fees (commitment, usage, facility, upfront). Supports both
/// deterministic schedules and stochastic utilization via Monte Carlo.
///
/// See unit tests and `examples/` for usage.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RevolvingCredit {
    /// Unique identifier for the facility.
    pub id: InstrumentId,

    /// Total committed amount for the facility.
    pub commitment_amount: Money,

    /// Current drawn amount (initial utilization).
    pub drawn_amount: Money,

    /// Date when the facility becomes available.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub commitment_date: Date,

    /// Date when the facility expires.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,

    /// Base rate specification (fixed or floating).
    pub base_rate_spec: BaseRateSpec,

    /// Day count convention for interest accrual.
    pub day_count: DayCount,

    /// Payment frequency for interest and fees.
    pub frequency: Tenor,

    /// Fee structure for the facility.
    pub fees: RevolvingCreditFees,

    /// Draw and repayment schedule (deterministic or stochastic).
    pub draw_repay_spec: DrawRepaySpec,

    /// Discount curve identifier for pricing.
    pub discount_curve_id: CurveId,

    /// Optional credit curve identifier for credit risk modeling.
    ///
    /// When provided, survival probabilities from the hazard curve are applied
    /// to discount cashflows, adjusting for default risk.
    #[serde(default)]
    pub credit_curve_id: Option<CurveId>,

    /// Recovery rate on default (used when credit_curve_id is present).
    ///
    /// Represents the fraction of exposure recovered in the event of default.
    /// Typical values: 0.30-0.50 for senior secured facilities.
    /// Callers must provide the value explicitly; zero recovery remains valid.
    pub recovery_rate: f64,

    /// Loan-equivalent exposure: the fraction of the undrawn commitment assumed
    /// to be drawn at default (Basel credit conversion factor), as a decimal in
    /// `[0, 1]`.
    ///
    /// Enters the default leg as additional exposure that the lender funds at
    /// par and recovers at `recovery_rate`, so each unit of LEQ draw costs
    /// `(1 − recovery_rate)` at default. Typical values are 0.3–0.75 depending
    /// on rating and covenant protection. Defaults to `0.0` (no draw at
    /// default), which reproduces the plain recovery leg.
    #[builder(default)]
    #[serde(default)]
    pub leq: f64,

    /// Stub rule for schedule generation when dates don't align with frequency.
    ///
    /// Determines how to handle partial periods at the start or end of the schedule:
    /// - `ShortFront`: Short stub at the beginning (most common for RCFs)
    /// - `ShortBack`: Short stub at the end
    /// - `LongFront`: Long stub at the beginning
    /// - `LongBack`: Long stub at the end
    /// - `None`: No stub allowed (dates must align exactly)
    ///
    /// Defaults to `ShortFront` for maximum flexibility with unaligned dates.
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "default_stub_kind")]
    pub stub: StubKind,

    /// Attributes for scenario selection and tagging.
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Default stub kind for revolving credit facilities.
fn default_stub_kind() -> StubKind {
    StubKind::ShortFront
}

/// Validate that fee tiers are sorted by threshold in strictly ascending order.
///
/// Fee tier evaluation picks the highest tier where utilization >= threshold,
/// so tiers must be strictly ascending for the algorithm to work correctly.
/// Duplicate thresholds are rejected because the first would be unreachable.
fn validate_fee_tier_ordering(tiers: &[FeeTier], context: &str) -> finstack_quant_core::Result<()> {
    for (index, tier) in tiers.iter().enumerate() {
        if !(Decimal::ZERO..=Decimal::ONE).contains(&tier.threshold) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit {context}[{index}].threshold must be in [0, 1], got {}",
                tier.threshold
            )));
        }
        if tier.bp < Decimal::ZERO {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit {context}[{index}].bp must be non-negative, got {}",
                tier.bp
            )));
        }
    }
    for i in 1..tiers.len() {
        if tiers[i].threshold <= tiers[i - 1].threshold {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit {} must be sorted by threshold strictly ascending: \
                 tier[{}].threshold ({}) <= tier[{}].threshold ({})",
                context,
                i,
                tiers[i].threshold,
                i - 1,
                tiers[i - 1].threshold
            )));
        }
    }
    Ok(())
}

impl RevolvingCredit {
    /// Create a canonical example revolving credit facility (USD, deterministic draws).
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::{DayCount, Tenor};
        use time::macros::date;
        let commitment = Money::from((50_000_000_i64, Currency::USD));
        let initial_draw = Money::from((10_000_000_i64, Currency::USD));
        let start = date!(2024 - 01 - 01);
        let end = date!(2027 - 01 - 01);
        let base_rate = BaseRateSpec::Floating(FloatingRateSpec {
            index_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::from(250),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: Some(Decimal::ZERO),
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        });
        let fees = RevolvingCreditFees::flat(25.0, 10.0, 5.0)?;
        let draw_repay = DrawRepaySpec::Deterministic(vec![
            DrawRepayEvent {
                date: date!(2024 - 03 - 01),
                amount: Money::from((5_000_000_i64, Currency::USD)),
                is_draw: true,
            },
            DrawRepayEvent {
                date: date!(2025 - 06 - 01),
                amount: Money::from((3_000_000_i64, Currency::USD)),
                is_draw: false,
            },
        ]);
        RevolvingCredit::builder()
            .id(InstrumentId::new("RCF-USD-3Y"))
            .commitment_amount(commitment)
            .drawn_amount(initial_draw)
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(base_rate)
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(fees)
            .draw_repay_spec(draw_repay)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .recovery_rate(0.0)
            .stub(StubKind::ShortFront)
            .attributes(Attributes::new())
            .build()
    }
}

/// Base rate specification for revolving credit interest.
///
/// Defines whether the facility pays a fixed rate or a floating rate
/// tied to a market index plus margin.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[allow(clippy::large_enum_variant)]
#[serde(rename_all = "snake_case")]
pub enum BaseRateSpec {
    /// Fixed rate (annualized).
    Fixed {
        /// Annual interest rate (e.g., 0.05 for 5%).
        rate: f64,
    },

    /// Floating rate using canonical FloatingRateSpec.
    ///
    /// Composes the standard floating rate specification with full support
    /// for floors, caps, and gearing.
    Floating(FloatingRateSpec),
}

impl BaseRateSpec {
    /// Create a fixed base rate using a typed rate.
    pub fn fixed_rate(rate: Rate) -> Self {
        Self::Fixed {
            rate: rate.as_decimal(),
        }
    }
}

/// Fee structure for a revolving credit facility.
///
/// Contains the various fees charged on the facility:
/// - Upfront: one-time fee at commitment
/// - Commitment: annual fee on undrawn amount (can be tiered by utilization)
/// - Usage: annual fee on drawn amount (can be tiered by utilization)
/// - Facility: annual fee on total commitment
///
/// Flat fees can be represented as single-tier vectors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RevolvingCreditFees {
    /// One-time upfront fee paid by borrower to lender at commitment.
    pub upfront_fee: Option<Money>,

    /// Commitment fee tiers (utilization-based). Empty vector means no commitment fee.
    /// Tiers should be sorted by threshold ascending.
    #[serde(default)]
    pub commitment_fee_tiers: Vec<FeeTier>,

    /// Usage fee tiers (utilization-based). Empty vector means no usage fee.
    /// Tiers should be sorted by threshold ascending.
    #[serde(default)]
    pub usage_fee_tiers: Vec<FeeTier>,

    /// Annual facility fee rate on total commitment (basis points).
    /// Facility fee is not tiered (applies to total commitment regardless of utilization).
    pub facility_fee_bp: f64,
}

impl Default for RevolvingCreditFees {
    fn default() -> Self {
        Self {
            upfront_fee: None,
            commitment_fee_tiers: Vec::new(),
            usage_fee_tiers: Vec::new(),
            facility_fee_bp: 0.0,
        }
    }
}

impl RevolvingCreditFees {
    /// Create fees with flat (non-tiered) commitment and usage fees.
    ///
    /// Convenience constructor for simple fee structures without utilization tiers.
    ///
    /// # Arguments
    ///
    /// * `commitment_fee_bp` - Annual commitment fee on the undrawn amount, in
    ///   basis points (e.g. `25.0` for 25bp). Must be finite; values `<= 0`
    ///   produce no commitment fee tier.
    /// * `usage_fee_bp` - Annual usage fee on the drawn amount, in basis
    ///   points. Must be finite; values `<= 0` produce no usage fee tier.
    /// * `facility_fee_bp` - Annual facility fee on the total commitment, in
    ///   basis points. Must be finite.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when any fee input
    /// is not finite (previously non-finite inputs silently produced a zero
    /// fee in release builds, masking data-quality errors).
    pub fn flat(
        commitment_fee_bp: f64,
        usage_fee_bp: f64,
        facility_fee_bp: f64,
    ) -> finstack_quant_core::Result<Self> {
        for (name, bp) in [
            ("commitment_fee_bp", commitment_fee_bp),
            ("usage_fee_bp", usage_fee_bp),
            ("facility_fee_bp", facility_fee_bp),
        ] {
            if !bp.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "RevolvingCreditFees::flat: {name} must be finite, got {bp}"
                )));
            }
        }
        let make_tier = |bp: f64| -> Vec<FeeTier> {
            if bp > 0.0 {
                vec![FeeTier {
                    threshold: Decimal::ZERO,
                    bp: Decimal::try_from(bp).unwrap_or(Decimal::ZERO),
                }]
            } else {
                Vec::new()
            }
        };

        Ok(Self {
            upfront_fee: None,
            commitment_fee_tiers: make_tier(commitment_fee_bp),
            usage_fee_tiers: make_tier(usage_fee_bp),
            facility_fee_bp,
        })
    }

    /// Create fees with flat (non-tiered) commitment and usage fees using typed bp.
    pub fn flat_bp(commitment_fee_bp: Bps, usage_fee_bp: Bps, facility_fee_bp: Bps) -> Self {
        let make_tier = |bp: Bps| -> Vec<FeeTier> {
            if !bp.is_zero() {
                vec![FeeTier {
                    threshold: Decimal::ZERO,
                    bp: Decimal::from(bp.as_bp()),
                }]
            } else {
                Vec::new()
            }
        };

        Self {
            upfront_fee: None,
            commitment_fee_tiers: make_tier(commitment_fee_bp),
            usage_fee_tiers: make_tier(usage_fee_bp),
            facility_fee_bp: facility_fee_bp.as_bp() as f64,
        }
    }

    /// Get commitment fee bp for given utilization (evaluates tiers).
    ///
    /// Returns the fee rate from the highest tier where utilization >= threshold.
    /// Tiers should be sorted by threshold ascending.
    /// If no tiers match or tiers are empty, returns 0.0.
    ///
    /// # Panics (debug builds only)
    ///
    /// Asserts that `utilization` is finite.
    pub fn commitment_fee_bp(&self, utilization: f64) -> f64 {
        debug_assert!(
            utilization.is_finite(),
            "commitment_fee_bp: utilization is not finite ({utilization})"
        );
        let util = Decimal::try_from(utilization).unwrap_or(Decimal::ZERO);
        evaluate_fee_tiers(&self.commitment_fee_tiers, util)
            .ok()
            .and_then(|bp| bp.to_f64())
            .unwrap_or(0.0)
    }

    /// Get usage fee bp for given utilization (evaluates tiers).
    ///
    /// Returns the fee rate from the highest tier where utilization >= threshold.
    /// Tiers should be sorted by threshold ascending.
    /// If no tiers match or tiers are empty, returns 0.0.
    ///
    /// # Panics (debug builds only)
    ///
    /// Asserts that `utilization` is finite.
    pub fn usage_fee_bp(&self, utilization: f64) -> f64 {
        debug_assert!(
            utilization.is_finite(),
            "usage_fee_bp: utilization is not finite ({utilization})"
        );
        let util = Decimal::try_from(utilization).unwrap_or(Decimal::ZERO);
        evaluate_fee_tiers(&self.usage_fee_tiers, util)
            .ok()
            .and_then(|bp| bp.to_f64())
            .unwrap_or(0.0)
    }
}

/// Draw and repayment specification.
///
/// Determines whether the facility uses a known (deterministic) schedule
/// or stochastic utilization for Monte Carlo pricing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DrawRepaySpec {
    /// Deterministic schedule of draws and repayments.
    Deterministic(Vec<DrawRepayEvent>),

    /// Stochastic utilization for Monte Carlo simulation.
    Stochastic(Box<StochasticUtilizationSpec>),
}

/// A single draw or repayment event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DrawRepayEvent {
    /// Date of the draw or repayment.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,

    /// Amount being drawn or repaid (absolute value).
    pub amount: Money,

    /// True if this is a draw, false if it's a repayment.
    pub is_draw: bool,
}

/// Specification for stochastic utilization modeling.
///
/// Defines the stochastic process and simulation parameters for
/// Monte Carlo pricing with uncertain draw/repayment patterns. Credit risk is
/// incorporated via hazard-rate survival weighting (no explicit default events).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StochasticUtilizationSpec {
    /// Utilization process specification.
    pub utilization_process: UtilizationProcess,

    /// Number of Monte Carlo paths to simulate.
    pub num_paths: usize,

    /// Random seed for reproducibility. Simulation is always deterministic:
    /// `None` falls back to a fixed default seed (42). Because the seed is
    /// fixed, bump-and-reprice sensitivities (DV01/CS01/Theta) reuse the same
    /// random variates for base and bumped valuations — common random
    /// numbers — so finite-difference Greeks are free of Monte Carlo noise.
    pub seed: Option<u64>,

    /// Use antithetic variance reduction when simulating paths (default: false).
    /// Mutually exclusive with `use_sobol_qmc`; validation rejects the combination.
    #[serde(default)]
    pub antithetic: bool,

    /// Use Sobol quasi-Monte Carlo RNG instead of Philox (default: false).
    /// Mutually exclusive with `antithetic`; validation rejects the combination.
    #[serde(default)]
    pub use_sobol_qmc: bool,

    /// Advanced Monte Carlo configuration (optional).
    ///
    /// When present, enables multi-factor modeling with credit spread
    /// and interest rate dynamics, correlation, and default modeling.
    pub mc_config: Option<McConfig>,
}

/// Advanced Monte Carlo configuration for revolving credit facilities.
///
/// Enables multi-factor modeling with credit risk, interest rate dynamics,
/// correlation between factors, and default modeling.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct McConfig {
    /// Correlation matrix (3x3) between [utilization, rate, credit].
    ///
    /// Must be symmetric, positive definite, with ones on diagonal.
    /// If None, factors are assumed independent.
    pub correlation_matrix: Option<[[f64; 3]; 3]>,

    /// Recovery rate on default (e.g., 0.4 for 40% recovery).
    ///
    /// Used when propagating credit risk from market-anchored stochastic specs to
    /// the deterministic fallback in `value()`. The path generator itself uses
    /// `RevolvingCredit::recovery_rate` for hazard-to-spread mapping, so these
    /// values should be kept consistent. When constructing `McConfig`, set this
    /// to the same value as `RevolvingCredit::recovery_rate`.
    pub recovery_rate: f64,

    /// Credit spread process specification.
    pub credit_spread_process: CreditSpreadProcessSpec,

    /// Interest rate process specification (for floating rates).
    ///
    /// If None, assumes fixed rate (no stochastic dynamics).
    pub interest_rate_process: Option<InterestRateProcessSpec>,

    /// Optional utilization–credit correlation used when `correlation_matrix` is None.
    ///
    /// If provided, builds a 3×3 matrix with:
    ///   [ [1, 0, rho], [0, 1, 0], [rho, 0, 1] ]
    /// representing correlation between utilization and credit, with the rate
    /// factor uncorrelated (kept fixed in 2‑factor mode).
    #[serde(default)]
    pub util_credit_corr: Option<f64>,
}

impl McConfig {
    /// Validate the configuration parameters.
    ///
    /// Checks that:
    /// - Correlation matrix (if provided) is positive semi-definite
    /// - Recovery rate is in [0, 1]
    /// - CIR parameters satisfy Feller condition if applicable
    /// - Credit spread parameters are valid
    ///
    /// # Returns
    ///
    /// `Ok(())` if all parameters are valid, otherwise returns an error
    /// describing the validation failure.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use super::MAX_RECOVERY_RATE;
        use finstack_quant_core::InputError;

        // Validate the explicit decimal recovery input, including both bounds.
        validation::require_with(
            self.recovery_rate.is_finite()
                && self.recovery_rate >= 0.0
                && self.recovery_rate <= MAX_RECOVERY_RATE,
            || {
                format!(
                    "Recovery rate must be a finite decimal in [0, {}], got {}",
                    MAX_RECOVERY_RATE, self.recovery_rate
                )
            },
        )?;

        // Validate correlation matrix if provided
        if let Some(corr) = self.correlation_matrix {
            // Check positive semi-definiteness
            finstack_quant_core::math::linalg::check_correlation_matrix(
                &corr.iter().flatten().copied().collect::<Vec<_>>(),
                3,
            )?;
        }

        // Validate util_credit_corr if provided
        if let Some(rho) = self.util_credit_corr {
            validation::require_or(rho.abs() <= 1.0, InputError::Invalid)?;
        }

        // Validate credit spread process parameters
        match &self.credit_spread_process {
            CreditSpreadProcessSpec::Cir {
                kappa,
                theta,
                sigma,
                initial,
            } => {
                // All parameters must be non-negative
                validation::require_or(
                    *kappa > 0.0 && *theta >= 0.0 && *sigma >= 0.0 && *initial >= 0.0,
                    InputError::Invalid,
                )?;
            }
            CreditSpreadProcessSpec::Constant(spread) => {
                validation::require_or(*spread >= 0.0, InputError::Invalid)?;
            }
            CreditSpreadProcessSpec::MarketAnchored {
                kappa,
                implied_vol,
                tenor_years,
                ..
            } => {
                validation::require_or(*kappa > 0.0 && *implied_vol >= 0.0, InputError::Invalid)?;
                if let Some(tenor) = tenor_years {
                    validation::require_or(*tenor > 0.0, InputError::Invalid)?;
                }
            }
        }

        // Validate interest rate process if provided
        if let Some(InterestRateProcessSpec::HullWhite1F { kappa, sigma, .. }) =
            &self.interest_rate_process
        {
            validation::require_or(*kappa > 0.0 && *sigma >= 0.0, InputError::Invalid)?;
        }
        if let Some(InterestRateProcessSpec::HullWhite1F { initial, theta, .. }) =
            &self.interest_rate_process
        {
            validation::validate_f64_finite(*initial, "Hull-White initial short rate")?;
            validation::validate_f64_finite(*theta, "Hull-White mean-reversion level")?;
        }

        Ok(())
    }
}

/// Credit spread process specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CreditSpreadProcessSpec {
    /// CIR process for stochastic credit spread/hazard rate.
    ///
    /// Models credit spread as: dλ_t = κ(θ - λ_t)dt + σ√λ_t dW_t
    Cir {
        /// Mean reversion speed (κ)
        kappa: f64,
        /// Long-term mean (θ)
        theta: f64,
        /// Volatility (σ)
        sigma: f64,
        /// Initial credit spread
        initial: f64,
    },
    /// Constant credit spread (no dynamics).
    Constant(f64),

    /// Market-anchored credit spread process calibrated to a hazard curve and CDS option vol.
    ///
    /// At the simulation anchor (the later of valuation and commitment date),
    /// the initial spread is set from the contemporaneous hazard and the mean
    /// level is anchored to conditional survival over the remaining tenor.
    /// Volatility is scaled from the CDS index option implied volatility.
    MarketAnchored {
        /// Credit curve identifier in `MarketContext` used to anchor spreads.
        credit_curve_id: CurveId,
        /// Mean reversion speed (κ) of the CIR process.
        kappa: f64,
        /// Annualized CDS (index) option implied volatility for spreads.
        implied_vol: f64,
        /// Optional tenor in years; if None, uses facility maturity horizon.
        #[serde(default)]
        tenor_years: Option<f64>,
    },
}

impl CreditSpreadProcessSpec {
    /// Short wire-style name of the process variant, for diagnostics.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Cir { .. } => "cir",
            Self::Constant(_) => "constant",
            Self::MarketAnchored { .. } => "market_anchored",
        }
    }
}

/// Interest rate process specification (for floating rates).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum InterestRateProcessSpec {
    /// Hull-White 1-factor model for short rate.
    ///
    /// Models short rate as: dr_t = κ[θ(t) - r_t]dt + σ dW_t
    ///
    /// Two calibration modes, selected by `sigma`:
    /// - `sigma > 0` (stochastic): the pricer fits a time-dependent θ(t) to
    ///   the facility's discount curve and reads the initial short rate from
    ///   that curve; the supplied `initial`/`theta` are ignored.
    /// - `sigma == 0` (deterministic parity mode): the supplied constant
    ///   `initial` and `theta` are used verbatim, with no curve fitting.
    ///
    /// Consequently the σ → 0 limit of the stochastic mode does **not**
    /// converge to the σ = 0 branch unless the supplied `initial`/`theta`
    /// are themselves curve-consistent. When running a volatility ladder
    /// down to zero, keep σ strictly positive for curve-fitted dynamics.
    #[serde(rename = "hull_white_1f")]
    HullWhite1F {
        /// Mean reversion speed (κ)
        kappa: f64,
        /// Volatility (σ)
        sigma: f64,
        /// Initial short rate used only when `sigma == 0`. For a stochastic
        /// process, the pricer derives the initial rate from the facility's
        /// discount curve at the valuation anchor.
        initial: f64,
        /// Constant mean reversion level used only when `sigma == 0`. For a
        /// stochastic process, the pricer fits a time-dependent θ(t) to the
        /// facility's discount curve.
        theta: f64,
    },
}

/// Utilization process for stochastic draws/repayments.
///
/// For the 80/20 implementation, we support a single mean-reverting process.
/// This can be extended in the future to support other processes (jump-diffusion,
/// regime-switching, etc.).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UtilizationProcess {
    /// Mean-reverting utilization rate process.
    ///
    /// Models utilization as reverting to a long-term target with specified
    /// speed and volatility. Uses Ornstein-Uhlenbeck dynamics:
    /// dU(t) = speed * (target_rate - U(t)) * dt + volatility * dW(t)
    ///
    /// The simulated process is a **clamped** OU: each step uses the exact OU
    /// transition and is then clamped to `[0, 1]`. When `target_rate` is well
    /// inside the interval and `volatility` is moderate the clamp fires only
    /// on rare tail excursions, but with `target_rate` near 0 or 1 and/or
    /// high `volatility` the clamp binds frequently and biases the simulated
    /// mean toward the interior. Keep the stationary standard deviation
    /// `volatility / sqrt(2 * speed)` small relative to the distance from
    /// `target_rate` to the nearest boundary.
    MeanReverting {
        /// Target utilization rate (0.0 to 1.0).
        target_rate: f64,
        /// Mean reversion speed (annualized).
        speed: f64,
        /// Volatility of utilization changes (annualized).
        volatility: f64,
        /// Sensitivity of the utilization target to the simulated credit
        /// spread (adverse selection), as a decimal per unit of relative
        /// spread change.
        ///
        /// The target used by the OU step becomes
        /// `θ(t) = clamp(target_rate + spread_sensitivity · (s(t) / s(0) − 1), 0, 1)`
        /// where `s(t)` is the simulated spread and `s(0)` its initial level,
        /// so a spread that doubles raises the target by `spread_sensitivity`.
        /// Defaults to `0.0` (no link); the utilization/credit shock
        /// correlation in `McConfig` applies on top of it. Ignored when the
        /// facility has no credit-spread process.
        #[serde(default)]
        spread_sensitivity: f64,
    },
}

impl RevolvingCredit {
    /// Validate all structural invariants of the revolving credit facility.
    ///
    /// Checks:
    /// - Commitment amount is positive
    /// - Drawn amount does not exceed commitment
    /// - Currency consistency between drawn and commitment amounts
    /// - Commitment date is before maturity date
    /// - Recovery rate is finite and in [0, 1]
    /// - Fee tiers are sorted by threshold ascending
    /// - Base rate fixed rate is finite
    ///
    /// # Errors
    ///
    /// Returns a validation error describing the first failed check.
    ///
    /// # Example
    ///
    /// ```text
    /// let facility = RevolvingCredit::builder()
    ///     .id("RCF-001".into())
    ///     // ... other fields ...
    ///     .recovery_rate(0.40)
    ///     .build()?;
    /// facility.validate()?; // Validates all parameters
    /// ```
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use super::MAX_RECOVERY_RATE;

        // Commitment amount must be positive
        validation::validate_money_gt(
            self.commitment_amount,
            0.0,
            "RevolvingCredit commitment_amount",
        )?;
        validation::validate_money_finite(self.drawn_amount, "RevolvingCredit drawn_amount")?;

        // Drawn amount must be non-negative (check before relationship check
        // so a negative drawn_amount is reported clearly rather than passing
        // the drawn <= commitment check vacuously)
        validation::require_with(self.drawn_amount.amount() >= 0.0, || {
            format!(
                "RevolvingCredit drawn_amount must be non-negative, got {}",
                self.drawn_amount
            )
        })?;

        // Drawn amount must not exceed commitment
        validation::require_with(
            self.drawn_amount.amount() <= self.commitment_amount.amount(),
            || {
                format!(
                    "RevolvingCredit drawn_amount ({}) must not exceed commitment_amount ({})",
                    self.drawn_amount, self.commitment_amount
                )
            },
        )?;

        // Currency consistency
        validation::validate_money_currency(
            self.drawn_amount,
            self.commitment_amount.currency(),
            "RevolvingCredit drawn_amount currency must match commitment_amount",
        )?;

        // Date ordering: commitment must be before maturity
        validation::validate_date_range_strict_with(
            self.commitment_date,
            self.maturity,
            |start, end| {
                format!(
                    "RevolvingCredit commitment_date ({}) must be before maturity ({})",
                    start, end
                )
            },
        )?;

        // Recovery is an explicit decimal fraction. Downstream hazard mappings
        // handle the full-recovery boundary without changing the stored input.
        validation::require_with(
            self.recovery_rate.is_finite()
                && self.recovery_rate >= 0.0
                && self.recovery_rate <= MAX_RECOVERY_RATE,
            || {
                format!(
                    "RevolvingCredit recovery_rate must be a finite decimal in [0, {}], got {}",
                    MAX_RECOVERY_RATE, self.recovery_rate
                )
            },
        )?;

        validation::require_with(
            self.leq.is_finite() && (0.0..=1.0).contains(&self.leq),
            || {
                format!(
                    "RevolvingCredit leq must be a finite decimal in [0, 1], got {}",
                    self.leq
                )
            },
        )?;

        // Validate fee tier ordering: thresholds must be strictly ascending
        validate_fee_tier_ordering(&self.fees.commitment_fee_tiers, "commitment_fee_tiers")?;
        validate_fee_tier_ordering(&self.fees.usage_fee_tiers, "usage_fee_tiers")?;

        if let Some(upfront_fee) = self.fees.upfront_fee {
            validation::validate_money_finite(upfront_fee, "RevolvingCredit upfront_fee")?;
            validation::validate_money_currency(
                upfront_fee,
                self.commitment_amount.currency(),
                "RevolvingCredit upfront_fee currency",
            )?;
            validation::require_with(upfront_fee.amount() >= 0.0, || {
                format!(
                    "RevolvingCredit upfront_fee must be non-negative, got {}",
                    upfront_fee
                )
            })?;
        }

        // Validate facility fee is non-negative
        validation::validate_f64_non_negative(
            self.fees.facility_fee_bp,
            "RevolvingCredit facility_fee_bp",
        )?;

        // Validate the complete coupon specification through the same canonical
        // conversion used by the cashflow engine. This keeps the public
        // validation/JSON boundary aligned with pricing for gearing, decimal
        // conversion, and index/all-in floor-cap ordering.
        match &self.base_rate_spec {
            BaseRateSpec::Fixed { rate } => {
                validation::validate_f64_finite(*rate, "RevolvingCredit fixed base rate")?;
            }
            BaseRateSpec::Floating(spec) => {
                validation::require_with(spec.gearing > Decimal::ZERO, || {
                    format!(
                        "RevolvingCredit floating gearing must be positive, got {}",
                        spec.gearing
                    )
                })?;
                if let (Some(floor), Some(cap)) = (spec.index_floor_bp, spec.index_cap_bp) {
                    validation::require_with(floor <= cap, || {
                        format!(
                            "RevolvingCredit index_floor_bp ({floor}) must not exceed index_cap_bp ({cap})"
                        )
                    })?;
                }
                if let (Some(floor), Some(cap)) = (spec.all_in_floor_bp, spec.all_in_cap_bp) {
                    validation::require_with(floor <= cap, || {
                        format!(
                            "RevolvingCredit all_in_floor_bp ({floor}) must not exceed all_in_cap_bp ({cap})"
                        )
                    })?;
                }
                let _ = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                let _ = crate::instruments::common_impl::pricing::overnight_conventions::resolved_overnight_compounding(
                    spec.index_id.as_str(),
                    spec.overnight_compounding.as_ref(),
                )?;
            }
        }

        match &self.draw_repay_spec {
            DrawRepaySpec::Deterministic(events) => {
                let mut events = events.iter().collect::<Vec<_>>();
                events.sort_by_key(|event| event.date);
                let mut balance = self.drawn_amount.amount();
                for event in events {
                    validation::require_with(
                        event.date > self.commitment_date && event.date <= self.maturity,
                        || {
                            format!(
                                "RevolvingCredit draw/repay event dated {} must be after \
                                 commitment ({}) and on or before maturity ({})",
                                event.date, self.commitment_date, self.maturity
                            )
                        },
                    )?;
                    validation::validate_money_finite(
                        event.amount,
                        "RevolvingCredit draw/repay amount",
                    )?;
                    validation::validate_money_gt(
                        event.amount,
                        0.0,
                        "RevolvingCredit draw/repay amount",
                    )?;
                    validation::validate_money_currency(
                        event.amount,
                        self.commitment_amount.currency(),
                        "RevolvingCredit draw/repay amount currency",
                    )?;
                    if event.is_draw {
                        balance += event.amount.amount();
                        validation::require_with(
                            balance <= self.commitment_amount.amount(),
                            || {
                                format!(
                                    "RevolvingCredit draw on {} would increase balance to {}, \
                                     above commitment {}",
                                    event.date, balance, self.commitment_amount
                                )
                            },
                        )?;
                    } else {
                        validation::require_with(event.amount.amount() <= balance, || {
                            format!(
                                "RevolvingCredit repayment on {} of {} exceeds current balance {}",
                                event.date, event.amount, balance
                            )
                        })?;
                        balance -= event.amount.amount();
                    }
                }
            }
            DrawRepaySpec::Stochastic(spec) => {
                validation::require_with(spec.num_paths >= 2, || {
                    format!(
                        "RevolvingCredit stochastic num_paths must be at least 2, got {}",
                        spec.num_paths
                    )
                })?;
                // Antithetic pairing is defined for pseudorandom draws only;
                // negating Sobol points destroys the low-discrepancy
                // structure. Reject the combination rather than silently
                // dropping the antithetic flag.
                validation::require_with(!(spec.antithetic && spec.use_sobol_qmc), || {
                    "RevolvingCredit stochastic spec cannot combine antithetic \
                     variance reduction with Sobol QMC; disable one of the two flags"
                        .to_string()
                })?;
                match &spec.utilization_process {
                    UtilizationProcess::MeanReverting {
                        target_rate,
                        speed,
                        volatility,
                        spread_sensitivity,
                    } => {
                        validation::require_with(
                            target_rate.is_finite() && (0.0..=1.0).contains(target_rate),
                            || {
                                format!(
                                    "RevolvingCredit utilization target_rate must be finite and \
                                     in [0, 1], got {target_rate}"
                                )
                            },
                        )?;
                        validation::validate_f64_positive(
                            *speed,
                            "RevolvingCredit utilization speed",
                        )?;
                        validation::validate_f64_non_negative(
                            *volatility,
                            "RevolvingCredit utilization volatility",
                        )?;
                        validation::require_with(spread_sensitivity.is_finite(), || {
                            format!(
                                "RevolvingCredit utilization spread_sensitivity must be finite, \
                                 got {spread_sensitivity}"
                            )
                        })?;
                    }
                }
                if let Some(mc_config) = &spec.mc_config {
                    mc_config.validate()?;
                    validation::require_with(
                        (mc_config.recovery_rate - self.recovery_rate).abs() <= 1e-12,
                        || {
                            format!(
                                "RevolvingCredit McConfig recovery_rate ({}) must equal the \
                                 facility recovery_rate ({})",
                                mc_config.recovery_rate, self.recovery_rate
                            )
                        },
                    )?;
                    // A hazard curve on the facility is both the CS01 bump
                    // target and the anchor for pathwise survival. Any other
                    // spread process ignores the curve, so a hazard bump would
                    // reprice to the same PV and report a silent zero CS01.
                    if let Some(curve) = &self.credit_curve_id {
                        match &mc_config.credit_spread_process {
                            CreditSpreadProcessSpec::MarketAnchored {
                                credit_curve_id, ..
                            } if credit_curve_id == curve => {}
                            CreditSpreadProcessSpec::MarketAnchored {
                                credit_curve_id, ..
                            } => {
                                return Err(finstack_quant_core::Error::Validation(format!(
                                    "RevolvingCredit {}: McConfig market-anchored credit curve \
                                     '{}' must equal the facility credit_curve_id '{}'",
                                    self.id, credit_curve_id, curve
                                )));
                            }
                            other => {
                                return Err(finstack_quant_core::Error::Validation(format!(
                                    "RevolvingCredit {}: credit_curve_id '{}' requires \
                                     CreditSpreadProcessSpec::MarketAnchored on that curve; the \
                                     supplied '{}' process ignores the curve, so hazard CS01 \
                                     would silently report zero. Drop credit_curve_id to price \
                                     on an explicit spread process",
                                    self.id,
                                    curve,
                                    other.kind()
                                )));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the current undrawn amount.
    pub fn undrawn_amount(&self) -> finstack_quant_core::Result<Money> {
        self.commitment_amount.checked_sub(self.drawn_amount)
    }

    /// Get the current utilization rate (drawn / committed).
    pub fn utilization_rate(&self) -> f64 {
        if self.commitment_amount.amount() == 0.0 {
            0.0
        } else {
            self.drawn_amount.amount() / self.commitment_amount.amount()
        }
    }

    /// Check if the facility uses deterministic cashflows.
    pub fn is_deterministic(&self) -> bool {
        matches!(self.draw_repay_spec, DrawRepaySpec::Deterministic(_))
    }

    /// Check if the facility uses stochastic utilization.
    pub fn is_stochastic(&self) -> bool {
        matches!(self.draw_repay_spec, DrawRepaySpec::Stochastic(_))
    }

    /// Check if the facility has a credit curve configured for CS01 calculations.
    ///
    /// Returns `true` if `credit_curve_id` is set, indicating that credit risk
    /// sensitivity (CS01) calculations are meaningful for this facility.
    pub fn has_credit_curve(&self) -> bool {
        self.credit_curve_id.is_some()
    }
}

// Implement the Instrument trait
impl crate::instruments::common_impl::traits::Instrument for RevolvingCredit {
    impl_instrument_base!(crate::pricer::InstrumentType::RevolvingCredit);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(credit_curve_id) = &self.credit_curve_id {
            deps.add_credit_curve(credit_curve_id.clone());
        }
        if let BaseRateSpec::Floating(spec) = &self.base_rate_spec {
            deps.add_forward_curve(spec.index_id.clone());
            deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                spec.index_id.as_str(),
            ));
        }
        Ok(deps)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        self.attributes()
            .get_meta("pricing_model")
            .and_then(|model_str| {
                <crate::pricer::ModelKey as ::std::str::FromStr>::from_str(model_str).ok()
            })
            .unwrap_or(crate::pricer::ModelKey::Discounting)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        // Optional model override via attributes metadata (e.g., meta["pricing_model"] = "monte_carlo_gbm")
        if let Some(model_str) = self.attributes().get_meta("pricing_model") {
            if let Ok(model) = <crate::pricer::ModelKey as ::std::str::FromStr>::from_str(model_str)
            {
                let registry = crate::pricer::standard_pricer_registry();
                let result = registry
                    .price_with_metrics(self, model, curves, as_of, &[], Default::default())
                    .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
                return Ok(result.value);
            }
        }

        // Use the same automatic deterministic/Monte Carlo dispatch as the
        // registry and host-language JSON entry points. A stochastic facility
        // must have one canonical value regardless of which public lifecycle
        // invokes it.
        crate::instruments::fixed_income::revolving_credit::pricing::unified::RevolvingCreditPricer::price(
            self, curves, as_of,
        )
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.commitment_date)
    }

    crate::impl_focused_pricing_overrides!();
}

// Implement CashflowProvider for standard cashflow interface
impl crate::cashflow::traits::CashflowScheduleSource for RevolvingCredit {
    fn notional(&self) -> finstack_quant_core::Result<Option<finstack_quant_core::money::Money>> {
        Ok(Some(self.commitment_amount))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        // Stochastic facilities expose their Monte Carlo expected schedule
        // (path-averaged flow by flow) so theta carry and the exporters work.
        if self.is_stochastic() {
            return crate::instruments::fixed_income::revolving_credit::pricing::unified::RevolvingCreditPricer::expected_cashflows(
                self, curves, as_of,
            )
            .map(|schedule| {
                schedule.with_representation(
                    crate::cashflow::builder::CashflowRepresentation::Projected,
                )
            });
        }

        use crate::instruments::fixed_income::revolving_credit::cashflow_engine::CashflowEngine;
        // Resolve fixings for floating-rate facilities (graceful: None if missing)
        let fixings = match &self.base_rate_spec {
            BaseRateSpec::Floating(spec) => {
                finstack_quant_core::market_data::fixings::get_fixing_series(
                    curves,
                    spec.index_id.as_ref(),
                )
                .ok()
            }
            BaseRateSpec::Fixed { .. } => None,
        };
        let engine = CashflowEngine::new(self, Some(curves), as_of, fixings)?;
        let path_schedule = engine.generate_deterministic()?;
        Ok(path_schedule
            .schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
mod dependency_tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    #[test]
    fn hull_white_process_uses_canonical_acronym_spelling() {
        let process = InterestRateProcessSpec::HullWhite1F {
            kappa: 0.1,
            sigma: 0.01,
            initial: 0.03,
            theta: 0.03,
        };
        let value = serde_json::to_value(process).expect("serialize Hull-White process");
        assert!(value.get("hull_white_1f").is_some());

        // schema-rejection-test
        assert!(
            serde_json::from_value::<InterestRateProcessSpec>(serde_json::json!({
                "hull_white1_f": {
                    "kappa": 0.1,
                    "sigma": 0.01,
                    "initial": 0.03,
                    "theta": 0.03
                }
            }))
            .is_err()
        );
    }

    #[test]
    fn floating_revolver_uses_the_canonical_fixing_series_id() {
        let facility = RevolvingCredit::example().expect("floating example");
        let BaseRateSpec::Floating(spec) = &facility.base_rate_spec else {
            unreachable!("example must use a floating base rate");
        };
        let expected =
            finstack_quant_core::market_data::fixings::fixing_series_id(spec.index_id.as_str());

        let deps =
            crate::instruments::Instrument::market_dependencies(&facility).expect("dependencies");
        assert_eq!(deps.series_ids, vec![expected]);
    }

    #[test]
    fn deterministic_events_are_validated_and_replayed_chronologically() {
        let mut facility = RevolvingCredit::example().expect("example");
        facility.draw_repay_spec = DrawRepaySpec::Deterministic(vec![
            DrawRepayEvent {
                date: date!(2025 - 06 - 01),
                amount: Money::from((20_000_000_i64, Currency::USD)),
                is_draw: false,
            },
            DrawRepayEvent {
                date: date!(2025 - 01 - 01),
                amount: Money::from((20_000_000_i64, Currency::USD)),
                is_draw: true,
            },
        ]);

        facility
            .validate()
            .expect("chronologically valid events must not depend on input order");
        let balance = super::super::cashflow_engine::calculate_drawn_balance_at_date(
            &facility,
            date!(2025 - 12 - 31),
        )
        .expect("balance");
        assert_eq!(balance.amount(), 10_000_000.0);
    }

    #[test]
    fn validation_rejects_events_outside_life_and_mismatched_mc_recovery() {
        let mut facility = RevolvingCredit::example().expect("example");
        facility.draw_repay_spec = DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
            date: date!(2028 - 01 - 01),
            amount: Money::from((1_000_000_i64, Currency::USD)),
            is_draw: true,
        }]);
        assert!(facility
            .validate()
            .expect_err("post-maturity draw must fail")
            .to_string()
            .contains("maturity"));

        facility.recovery_rate = 0.4;
        facility.draw_repay_spec = DrawRepaySpec::Stochastic(Box::new(StochasticUtilizationSpec {
            utilization_process: UtilizationProcess::MeanReverting {
                target_rate: 0.5,
                speed: 1.0,
                volatility: 0.1,
                spread_sensitivity: 0.0,
            },
            num_paths: 100,
            seed: Some(7),
            antithetic: false,
            use_sobol_qmc: false,
            mc_config: Some(McConfig {
                correlation_matrix: None,
                recovery_rate: 0.35,
                credit_spread_process: CreditSpreadProcessSpec::Constant(0.01),
                interest_rate_process: None,
                util_credit_corr: None,
            }),
        }));
        assert!(facility
            .validate()
            .expect_err("mismatched recovery assumptions must fail")
            .to_string()
            .contains("must equal"));
    }

    #[test]
    fn validation_rejects_invalid_floating_rate_economics() {
        let mut facility = RevolvingCredit::example().expect("example");
        let BaseRateSpec::Floating(spec) = &mut facility.base_rate_spec else {
            unreachable!("example must be floating");
        };
        spec.gearing = rust_decimal::Decimal::ZERO;

        let err = facility
            .validate()
            .expect_err("non-positive floating gearing must fail validation");
        assert!(
            err.to_string().contains("gearing"),
            "unexpected validation error: {err}"
        );
    }
}
