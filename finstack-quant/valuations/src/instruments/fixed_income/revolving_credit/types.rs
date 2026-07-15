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
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RevolvingCredit {
    /// Unique identifier for the facility.
    pub id: InstrumentId,

    /// Total committed amount for the facility.
    pub commitment_amount: Money,

    /// Current drawn amount (initial utilization).
    pub drawn_amount: Money,

    /// Date when the facility becomes available.
    #[schemars(with = "String")]
    pub commitment_date: Date,

    /// Date when the facility expires.
    #[schemars(with = "String")]
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
    /// Defaults to 0.0 if not specified.
    #[builder(default)]
    #[serde(default)]
    pub recovery_rate: f64,

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
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
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
        let commitment = Money::new(50_000_000.0, Currency::USD);
        let initial_draw = Money::new(10_000_000.0, Currency::USD);
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
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        });
        let fees = RevolvingCreditFees::flat(25.0, 10.0, 5.0);
        let draw_repay = DrawRepaySpec::Deterministic(vec![
            DrawRepayEvent {
                date: date!(2024 - 03 - 01),
                amount: Money::new(5_000_000.0, Currency::USD),
                is_draw: true,
            },
            DrawRepayEvent {
                date: date!(2025 - 06 - 01),
                amount: Money::new(3_000_000.0, Currency::USD),
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[allow(clippy::large_enum_variant)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
    /// # Panics (debug builds only)
    ///
    /// Asserts that all fee inputs are finite. Pass `NaN` or `Inf` in debug
    /// builds to surface the error early; in release builds non-finite inputs
    /// produce `Decimal::ZERO` (better than a panic in a long-running process).
    pub fn flat(commitment_fee_bp: f64, usage_fee_bp: f64, facility_fee_bp: f64) -> Self {
        debug_assert!(
            commitment_fee_bp.is_finite(),
            "RevolvingCreditFees::flat: commitment_fee_bp is not finite ({commitment_fee_bp})"
        );
        debug_assert!(
            usage_fee_bp.is_finite(),
            "RevolvingCreditFees::flat: usage_fee_bp is not finite ({usage_fee_bp})"
        );
        debug_assert!(
            facility_fee_bp.is_finite(),
            "RevolvingCreditFees::flat: facility_fee_bp is not finite ({facility_fee_bp})"
        );
        let make_tier = |bps: f64| -> Vec<FeeTier> {
            if bps > 0.0 {
                vec![FeeTier {
                    threshold: Decimal::ZERO,
                    bps: Decimal::try_from(bps).unwrap_or(Decimal::ZERO),
                }]
            } else {
                Vec::new()
            }
        };

        Self {
            upfront_fee: None,
            commitment_fee_tiers: make_tier(commitment_fee_bp),
            usage_fee_tiers: make_tier(usage_fee_bp),
            facility_fee_bp,
        }
    }

    /// Create fees with flat (non-tiered) commitment and usage fees using typed bps.
    pub fn flat_bps(commitment_fee_bp: Bps, usage_fee_bp: Bps, facility_fee_bp: Bps) -> Self {
        let make_tier = |bps: Bps| -> Vec<FeeTier> {
            if !bps.is_zero() {
                vec![FeeTier {
                    threshold: Decimal::ZERO,
                    bps: Decimal::from(bps.as_bps()),
                }]
            } else {
                Vec::new()
            }
        };

        Self {
            upfront_fee: None,
            commitment_fee_tiers: make_tier(commitment_fee_bp),
            usage_fee_tiers: make_tier(usage_fee_bp),
            facility_fee_bp: facility_fee_bp.as_bps() as f64,
        }
    }

    /// Get commitment fee bps for given utilization (evaluates tiers).
    ///
    /// Returns the fee rate from the highest tier where utilization >= threshold.
    /// Tiers should be sorted by threshold ascending.
    /// If no tiers match or tiers are empty, returns 0.0.
    ///
    /// # Panics (debug builds only)
    ///
    /// Asserts that `utilization` is finite.
    pub fn commitment_fee_bps(&self, utilization: f64) -> f64 {
        debug_assert!(
            utilization.is_finite(),
            "commitment_fee_bps: utilization is not finite ({utilization})"
        );
        let util = Decimal::try_from(utilization).unwrap_or(Decimal::ZERO);
        evaluate_fee_tiers(&self.commitment_fee_tiers, util)
            .ok()
            .and_then(|bps| bps.to_f64())
            .unwrap_or(0.0)
    }

    /// Get usage fee bps for given utilization (evaluates tiers).
    ///
    /// Returns the fee rate from the highest tier where utilization >= threshold.
    /// Tiers should be sorted by threshold ascending.
    /// If no tiers match or tiers are empty, returns 0.0.
    ///
    /// # Panics (debug builds only)
    ///
    /// Asserts that `utilization` is finite.
    pub fn usage_fee_bps(&self, utilization: f64) -> f64 {
        debug_assert!(
            utilization.is_finite(),
            "usage_fee_bps: utilization is not finite ({utilization})"
        );
        let util = Decimal::try_from(utilization).unwrap_or(Decimal::ZERO);
        evaluate_fee_tiers(&self.usage_fee_tiers, util)
            .ok()
            .and_then(|bps| bps.to_f64())
            .unwrap_or(0.0)
    }
}

/// Draw and repayment specification.
///
/// Determines whether the facility uses a known (deterministic) schedule
/// or stochastic utilization for Monte Carlo pricing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum DrawRepaySpec {
    /// Deterministic schedule of draws and repayments.
    Deterministic(Vec<DrawRepayEvent>),

    /// Stochastic utilization for Monte Carlo simulation.
    Stochastic(Box<StochasticUtilizationSpec>),
}

/// A single draw or repayment event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DrawRepayEvent {
    /// Date of the draw or repayment.
    #[schemars(with = "String")]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct StochasticUtilizationSpec {
    /// Utilization process specification.
    pub utilization_process: UtilizationProcess,

    /// Number of Monte Carlo paths to simulate.
    pub num_paths: usize,

    /// Random seed for reproducibility (None for non-deterministic).
    pub seed: Option<u64>,

    /// Use antithetic variance reduction when simulating paths (default: false).
    #[serde(default)]
    pub antithetic: bool,

    /// Use Sobol quasi-Monte Carlo RNG instead of Philox (default: false).
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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

        // Validate recovery rate: must be in [0, 1) to avoid division by zero
        // in hazard-to-spread mapping: λ = s / (1 - R)
        validation::require_with(
            self.recovery_rate >= 0.0 && self.recovery_rate < MAX_RECOVERY_RATE,
            || {
                format!(
                    "Recovery rate must be in [0, {:.6}), got {}",
                    MAX_RECOVERY_RATE, self.recovery_rate
                )
            },
        )?;

        // Validate correlation matrix if provided
        if let Some(corr) = self.correlation_matrix {
            // Check positive semi-definiteness
            finstack_quant_core::math::linalg::validate_correlation_matrix(
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

        Ok(())
    }
}

/// Credit spread process specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
    /// The mean level is anchored to the time-average spread implied by the input hazard
    /// curve (over tenor T chosen as facility maturity by default). The initial spread is set
    /// to the first-segment spread, and the volatility is scaled from the CDS index option
    /// implied volatility.
    MarketAnchored {
        /// Hazard curve identifier in `MarketContext` used to anchor spreads.
        hazard_curve_id: CurveId,
        /// Mean reversion speed (κ) of the CIR process.
        kappa: f64,
        /// Annualized CDS (index) option implied volatility for spreads.
        implied_vol: f64,
        /// Optional tenor in years; if None, uses facility maturity horizon.
        #[serde(default)]
        tenor_years: Option<f64>,
    },
}

/// Interest rate process specification (for floating rates).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum InterestRateProcessSpec {
    /// Hull-White 1-factor model for short rate.
    ///
    /// Models short rate as: dr_t = κ[θ(t) - r_t]dt + σ dW_t
    HullWhite1F {
        /// Mean reversion speed (κ)
        kappa: f64,
        /// Volatility (σ)
        sigma: f64,
        /// Initial short rate
        initial: f64,
        /// Constant mean reversion level (θ)
        theta: f64,
    },
}

/// Utilization process for stochastic draws/repayments.
///
/// For the 80/20 implementation, we support a single mean-reverting process.
/// This can be extended in the future to support other processes (jump-diffusion,
/// regime-switching, etc.).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum UtilizationProcess {
    /// Mean-reverting utilization rate process.
    ///
    /// Models utilization as reverting to a long-term target with specified
    /// speed and volatility. Uses Ornstein-Uhlenbeck dynamics:
    /// dU(t) = speed * (target_rate - U(t)) * dt + volatility * dW(t)
    MeanReverting {
        /// Target utilization rate (0.0 to 1.0).
        target_rate: f64,
        /// Mean reversion speed (annualized).
        speed: f64,
        /// Volatility of utilization changes (annualized).
        volatility: f64,
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
    /// - Recovery rate is in [0, 1) (must be strictly less than 1 to avoid
    ///   division by zero in hazard-to-spread mapping: λ = s / (1 - R))
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

        // Recovery rate bounds: must be in [0, MAX_RECOVERY_RATE) to avoid
        // division by zero in hazard-to-spread mapping: λ = s / (1 - R)
        validation::require_with(
            self.recovery_rate >= 0.0 && self.recovery_rate < MAX_RECOVERY_RATE,
            || {
                format!(
                    "RevolvingCredit recovery_rate must be in [0, {:.6}), got {}",
                    MAX_RECOVERY_RATE, self.recovery_rate
                )
            },
        )?;

        // Validate fee tier ordering: thresholds must be strictly ascending
        validate_fee_tier_ordering(&self.fees.commitment_fee_tiers, "commitment_fee_tiers")?;
        validate_fee_tier_ordering(&self.fees.usage_fee_tiers, "usage_fee_tiers")?;

        // Validate facility fee is non-negative
        validation::validate_f64_non_negative(
            self.fees.facility_fee_bp,
            "RevolvingCredit facility_fee_bp",
        )?;

        // Validate base rate if fixed
        if let BaseRateSpec::Fixed { rate } = &self.base_rate_spec {
            validation::validate_f64_finite(*rate, "RevolvingCredit fixed base rate")?;
        }

        // Deterministic draw/repay events must be dated strictly after the
        // commitment date: the position at commitment is defined by
        // drawn_amount, and a commitment-date event would double-count
        // principal (see CashflowEngine::build_deterministic_schedule).
        if let DrawRepaySpec::Deterministic(events) = &self.draw_repay_spec {
            for event in events {
                validation::require_with(event.date > self.commitment_date, || {
                    format!(
                        "RevolvingCredit draw/repay event dated {} must be strictly after the \
                         commitment date ({}); encode the initial position in drawn_amount",
                        event.date, self.commitment_date
                    )
                })?;
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
            deps.add_series_id(spec.index_id.as_str());
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
                let registry = crate::pricer::standard_registry();
                let result = registry
                    .price_with_metrics(self, model, curves, as_of, &[], Default::default())
                    .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
                return Ok(result.value);
            }
        }

        // Route to appropriate pricer based on spec type
        if self.is_deterministic() {
            crate::instruments::fixed_income::revolving_credit::pricer::unified::RevolvingCreditPricer::price_deterministic(
                self, curves, as_of,
            )
        } else {
            // For the value() fast path, route stochastic specs to deterministic pricing.
            // MC remains available via explicit pricer APIs/bindings (e.g., mc_paths_with_capture).
            let mut fallback = self.clone();
            // If the stochastic spec carried a market-anchored hazard reference in its MC config,
            // propagate that to the deterministic fallback so survival weighting is preserved.

            if let super::types::DrawRepaySpec::Stochastic(spec) = &self.draw_repay_spec {
                if let Some(mc_cfg) = &spec.mc_config {
                    if let super::types::CreditSpreadProcessSpec::MarketAnchored {
                        hazard_curve_id,
                        ..
                    } = &mc_cfg.credit_spread_process
                    {
                        fallback.credit_curve_id = Some(hazard_curve_id.clone());
                        fallback.recovery_rate = mc_cfg.recovery_rate;
                    }
                }
            }
            // Ensure deterministic schedule for pricing.
            fallback.draw_repay_spec = super::types::DrawRepaySpec::Deterministic(Vec::new());
            crate::instruments::fixed_income::revolving_credit::pricer::unified::RevolvingCreditPricer::price_deterministic(
                &fallback, curves, as_of,
            )
        }
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.commitment_date)
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

// Implement CashflowProvider for standard cashflow interface
impl crate::cashflow::traits::CashflowScheduleSource for RevolvingCredit {
    fn notional(&self) -> Option<finstack_quant_core::money::Money> {
        Some(self.commitment_amount)
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        // Only works for deterministic specs
        if !self.is_deterministic() {
            return Err(finstack_quant_core::InputError::Invalid.into());
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
