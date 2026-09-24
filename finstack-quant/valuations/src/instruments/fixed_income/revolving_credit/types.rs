//! Revolving credit facility types and instrument trait implementations.
//!
//! Defines the `RevolvingCredit` instrument with support for deterministic and
//! stochastic cashflow modeling. Supports standard fee structures (upfront,
//! commitment, usage, and facility fees) and both fixed and floating rate bases.

use finstack_quant_core::dates::{
    calendar_by_id, BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Bps, CurveId, InstrumentId, Rate};
use rust_decimal::Decimal;

use crate::cashflow::builder::{evaluate_fee_tiers, FeeTier, FloatingRateSpec};
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::fixed_income::loan_terms::{
    CommitmentStep, FeeStep, LetterOfCreditSpec, MarginStepUp, OidEirSpec, ScheduledFee, UpfrontFee,
};
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

    /// Opening commitment of the facility, in force from `commitment_date`
    /// until the first entry of `commitment_schedule`.
    pub commitment_amount: Money,

    /// Scheduled commitment changes (amortizing commitments, availability
    /// expiries, accordions), each in force from its date until the next.
    /// Dates must be strictly increasing, after `commitment_date` and on or
    /// before `maturity`; the drawn balance plus outstanding letters of
    /// credit must never exceed the commitment in force. A step down pays its
    /// `fee_bp` on the reduced amount. Empty by default.
    #[builder(default)]
    #[serde(default)]
    pub commitment_schedule: Vec<CommitmentStep>,

    /// Dated margin changes, cumulative from their dates: a leverage or
    /// ratings grid the analyst has forecast, a scheduled step-up or a
    /// default-rate margin. `delta_bp` shifts the floating spread or the fixed
    /// rate. Dates must be strictly increasing and strictly inside the
    /// facility life. Empty by default.
    #[builder(default)]
    #[serde(default)]
    pub margin_steps: Vec<MarginStepUp>,

    /// Letter-of-credit sub-facility. Outstanding letters of credit reduce
    /// availability and the commitment-fee base, count as usage for fee
    /// tiers, accrue the LC fee (the floating margin unless `fee_bp` is set)
    /// plus the fronting fee, and are contingent exposure at default through
    /// their own `leq`. `None` (the default) means no LC sublimit.
    #[builder(default)]
    #[serde(default)]
    pub lc: Option<LetterOfCreditSpec>,

    /// Dated fixed fees (amendment, waiver, extension, consent), each paid on
    /// its date and emitted as a generic fee flow. Dates must lie after the
    /// commitment date and on or before maturity. Empty by default.
    #[builder(default)]
    #[serde(default)]
    pub scheduled_fees: Vec<ScheduledFee>,

    /// Effective-interest-rate reporting switch for the
    /// `oid_eir_amortization` metric. `None` (the default) reports with fees
    /// included, the same as `Some(OidEirSpec::default())`.
    #[builder(default)]
    #[serde(default)]
    pub oid_eir: Option<OidEirSpec>,

    /// Drawn balance at the simulation anchor, the later of `commitment_date`
    /// and the valuation date, in both deterministic and stochastic mode.
    ///
    /// For a new facility this is the balance funded at commitment; for a
    /// seasoned facility it is the balance observed on the valuation date.
    /// Deterministic draw/repay events describe the future only: an event
    /// dated on or before the valuation date is rejected by the cashflow
    /// engine, because the position at the anchor is defined by this field
    /// alone. The accrual period containing the valuation date accrues on
    /// this balance from its accrual start.
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

    /// Business-day convention applied to payment dates (interest, fees and
    /// principal) and to the fixing-date roll. Accrual boundaries stay
    /// unadjusted. Defaults to `ModifiedFollowing`.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,

    /// Holiday calendar identifier (for example `"usny"`) used to adjust
    /// payment dates, roll fixing dates and count settlement days. `None`
    /// adjusts for weekends only. Validation rejects an unknown identifier.
    /// `FloatingRateSpec::fixing_calendar_id` overrides it for the fixing
    /// date alone.
    #[builder(default)]
    #[serde(default)]
    pub calendar_id: Option<String>,

    /// Business days between an accrual end and its payment date, on
    /// `calendar_id`. `0` (the default) pays on the adjusted accrual end.
    #[builder(default)]
    #[serde(default)]
    pub payment_lag_days: u32,

    /// Business days from the valuation date to the settlement date used by
    /// quote metrics (discount margin, yield, accrued interest). `0` (the
    /// default) settles on the valuation date. The base present value is
    /// always anchored at the valuation date.
    #[builder(default)]
    #[serde(default)]
    pub settlement_days: u32,

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
            .calendar_id("usny".to_string())
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
    /// One-time upfront (arrangement or OID) fee paid by the borrower to the
    /// lender on the commitment date, as an absolute amount or a fraction of
    /// the opening commitment. Enters the present value only while the
    /// commitment date lies after the valuation date, and the effective-rate
    /// metrics always.
    #[serde(default)]
    pub upfront_fee: Option<UpfrontFee>,

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

    /// Dated fee changes, cumulative from their dates, each shifting every
    /// tier of the corresponding fee in basis points per annum. Dates must be
    /// strictly increasing and strictly inside the facility life. Empty by
    /// default.
    #[serde(default)]
    pub steps: Vec<FeeStep>,
}

impl Default for RevolvingCreditFees {
    fn default() -> Self {
        Self {
            upfront_fee: None,
            commitment_fee_tiers: Vec::new(),
            usage_fee_tiers: Vec::new(),
            facility_fee_bp: 0.0,
            steps: Vec::new(),
        }
    }
}

/// Cumulative fee deltas in force on a date, in basis points per annum.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FeeDeltas {
    /// Shift of every commitment-fee tier.
    pub commitment_bp: f64,
    /// Shift of every usage-fee tier.
    pub usage_bp: f64,
    /// Shift of the facility fee.
    pub facility_bp: f64,
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
            steps: Vec::new(),
        })
    }

    /// Cumulative fee deltas in force on `date`: the sum of every step dated
    /// on or before it.
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual date the running fees are wanted for.
    pub fn deltas_at(&self, date: Date) -> FeeDeltas {
        self.steps.iter().filter(|step| step.date <= date).fold(
            FeeDeltas::default(),
            |acc, step| FeeDeltas {
                commitment_bp: acc.commitment_bp + step.commitment_delta_bp,
                usage_bp: acc.usage_bp + step.usage_delta_bp,
                facility_bp: acc.facility_bp + step.facility_delta_bp,
            },
        )
    }

    /// Commitment fee in force on `date` for the given utilization: the tier
    /// rate plus the cumulative step delta, floored at zero.
    ///
    /// # Arguments
    ///
    /// * `utilization` - Drawn plus LC usage over the commitment in force, as a
    ///   decimal in `[0, 1]`, used to select the tier.
    /// * `date` - Accrual date the steps are evaluated on.
    ///
    /// # Errors
    ///
    /// Same as [`Self::commitment_fee_bp`].
    pub fn commitment_fee_bp_at(
        &self,
        utilization: f64,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok((self.commitment_fee_bp(utilization)? + self.deltas_at(date).commitment_bp).max(0.0))
    }

    /// Usage fee in force on `date` for the given utilization (tier rate plus
    /// cumulative step delta, floored at zero).
    ///
    /// # Arguments
    ///
    /// * `utilization` - Drawn plus LC usage over the commitment in force, as a
    ///   decimal in `[0, 1]`, used to select the tier.
    /// * `date` - Accrual date the steps are evaluated on.
    ///
    /// # Errors
    ///
    /// Same as [`Self::usage_fee_bp`].
    pub fn usage_fee_bp_at(
        &self,
        utilization: f64,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok((self.usage_fee_bp(utilization)? + self.deltas_at(date).usage_bp).max(0.0))
    }

    /// Facility fee in force on `date` (flat rate plus cumulative step delta,
    /// floored at zero).
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual date the steps are evaluated on.
    pub fn facility_fee_bp_at(&self, date: Date) -> f64 {
        (self.facility_fee_bp + self.deltas_at(date).facility_bp).max(0.0)
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
            steps: Vec::new(),
        }
    }

    /// Commitment fee in basis points for a utilization: the rate of the
    /// highest tier whose threshold the utilization reaches, or `0.0` when no
    /// tier applies. Tiers must be sorted by threshold ascending.
    ///
    /// # Arguments
    ///
    /// * `utilization` - Drawn plus LC usage over the commitment, as a
    ///   decimal (`0.4` = 40% used).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `utilization` is not finite or the
    /// tiers are not strictly ascending.
    pub fn commitment_fee_bp(&self, utilization: f64) -> finstack_quant_core::Result<f64> {
        tier_bp(&self.commitment_fee_tiers, utilization, "commitment")
    }

    /// Usage fee in basis points for a utilization: the rate of the highest
    /// tier whose threshold the utilization reaches, or `0.0` when no tier
    /// applies. Tiers must be sorted by threshold ascending.
    ///
    /// # Arguments
    ///
    /// * `utilization` - Drawn plus LC usage over the commitment, as a
    ///   decimal (`0.4` = 40% used).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `utilization` is not finite or the
    /// tiers are not strictly ascending.
    pub fn usage_fee_bp(&self, utilization: f64) -> finstack_quant_core::Result<f64> {
        tier_bp(&self.usage_fee_tiers, utilization, "usage")
    }
}

/// Tier rate in basis points for a finite utilization.
fn tier_bp(tiers: &[FeeTier], utilization: f64, label: &str) -> finstack_quant_core::Result<f64> {
    let util = Decimal::try_from(utilization)
        .ok()
        .filter(|_| utilization.is_finite())
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit {label} fee: utilization must be finite, got {utilization}"
            ))
        })?;
    evaluate_fee_tiers(tiers, util)?.to_f64().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "RevolvingCredit {label} fee tier rate is not representable as f64"
        ))
    })
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

    /// Credit spread process specification. Recovery on default is the
    /// facility's `recovery_rate`; the hazard-to-spread mapping reads it there.
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
    /// - CIR parameters satisfy Feller condition if applicable
    /// - Credit spread parameters are valid
    ///
    /// # Returns
    ///
    /// `Ok(())` if all parameters are valid, otherwise returns an error
    /// describing the validation failure.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::InputError;

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
    /// Settlement date of a quote on `as_of`: `settlement_days` business days
    /// forward on `calendar_id`, or weekdays when no calendar is set.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date the settlement lag is counted from.
    ///
    /// # Errors
    ///
    /// Returns an error when `calendar_id` names an unknown calendar or the
    /// business-day roll fails.
    pub fn settlement_date(&self, as_of: Date) -> finstack_quant_core::Result<Date> {
        if self.settlement_days == 0 {
            return Ok(as_of);
        }
        match self.calendar_id.as_deref() {
            Some(calendar_id) => {
                let calendar = calendar_by_id(calendar_id).ok_or_else(|| {
                    finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                        id: format!("calendar:{calendar_id}"),
                    })
                })?;
                as_of.add_business_days(self.settlement_days as i32, calendar)
            }
            None => Ok(as_of.add_weekdays(self.settlement_days as i32)),
        }
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

    /// Drawn balance on `date` of a deterministic draw/repay schedule.
    ///
    /// Starts from `drawn_amount`, the balance at `anchor`, and replays in
    /// order the `events` dated strictly after `anchor` and on or before
    /// `date`. Each event is checked against the commitment in force on its
    /// date and the running balance.
    ///
    /// # Arguments
    ///
    /// * `events` - Draw/repay events sorted by date ascending.
    /// * `anchor` - Date `drawn_amount` refers to; events on or before it are
    ///   not replayed.
    /// * `date` - Date the balance is wanted for; events dated on it apply.
    ///
    /// # Errors
    ///
    /// Returns a validation error when a draw would exceed the commitment in
    /// force or a repayment exceeds the running balance.
    pub(crate) fn drawn_balance_at<'a>(
        &self,
        events: impl IntoIterator<Item = &'a DrawRepayEvent>,
        anchor: Date,
        date: Date,
    ) -> finstack_quant_core::Result<Money> {
        let mut balance = self.drawn_amount;
        for event in events {
            if event.date > date {
                break;
            }
            if event.date > anchor {
                balance = super::utils::apply_draw_repay_event(
                    balance,
                    event,
                    self.commitment_at(event.date),
                )?;
            }
        }
        Ok(balance)
    }

    /// Commitment in force on `date`: the last `commitment_schedule` entry
    /// dated on or before it, else the opening `commitment_amount`.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the commitment is wanted for.
    pub fn commitment_at(&self, date: Date) -> Money {
        self.commitment_schedule
            .iter()
            .rev()
            .find(|step| step.date <= date)
            .map_or(self.commitment_amount, |step| step.amount)
    }

    /// Drawn balance at the simulation anchor over the commitment in force on
    /// `date`, as a decimal in `[0, 1]`.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the commitment is read on, normally the simulation
    ///   anchor.
    pub fn utilization_at(&self, date: Date) -> f64 {
        let commitment = self.commitment_at(date).amount();
        if commitment <= 0.0 {
            0.0
        } else {
            (self.drawn_amount.amount() / commitment).clamp(0.0, 1.0)
        }
    }

    /// Cumulative margin change in force on `date`, in basis points: the sum
    /// of every `margin_steps` entry dated on or before it.
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual date the margin is wanted for.
    pub fn margin_delta_bp_at(&self, date: Date) -> f64 {
        self.margin_steps
            .iter()
            .filter(|step| step.date <= date)
            .map(|step| f64::from(step.delta_bp))
            .sum()
    }

    /// Every date on which a commitment, margin or fee step takes effect,
    /// sorted and deduplicated. Both cashflow engines slice accrual on them.
    pub fn step_dates(&self) -> Vec<Date> {
        let mut dates: Vec<Date> = self
            .commitment_schedule
            .iter()
            .map(|step| step.date)
            .chain(self.margin_steps.iter().map(|step| step.date))
            .chain(self.fees.steps.iter().map(|step| step.date))
            .chain(
                self.lc
                    .iter()
                    .flat_map(|lc| lc.events.iter().map(|event| event.date)),
            )
            .collect();
        dates.sort_unstable();
        dates.dedup();
        dates
    }

    /// Letter-of-credit face outstanding on `date`: the anchor `outstanding`
    /// plus every issuance, less every expiry, dated on or before it. Zero
    /// without an LC sub-facility.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the LC outstanding is wanted for.
    pub fn lc_outstanding_at(&self, date: Date) -> Money {
        let ccy = self.commitment_amount.currency();
        let Some(lc) = &self.lc else {
            return Money::from((0_i64, ccy));
        };
        let amount = lc
            .events
            .iter()
            .filter(|event| event.date <= date)
            .fold(lc.outstanding.amount(), |acc, event| {
                if event.is_issue {
                    acc + event.amount.amount()
                } else {
                    acc - event.amount.amount()
                }
            })
            .max(0.0);
        Money::new(amount, ccy).unwrap_or(Money::from((0_i64, ccy)))
    }

    /// Letter-of-credit fee in force on `date`, in basis points per annum:
    /// `lc.fee_bp` when set, else the floating spread plus the cumulative
    /// margin step delta. Zero without an LC sub-facility.
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual date the fee is evaluated on.
    pub fn lc_fee_bp_at(&self, date: Date) -> f64 {
        let Some(lc) = &self.lc else { return 0.0 };
        match (lc.fee_bp, &self.base_rate_spec) {
            (Some(fee_bp), _) => fee_bp,
            (None, BaseRateSpec::Floating(spec)) => {
                spec.spread_bp.to_f64().unwrap_or(0.0) + self.margin_delta_bp_at(date)
            }
            (None, BaseRateSpec::Fixed { .. }) => 0.0,
        }
        .max(0.0)
    }

    /// Model named by `attributes.meta["pricing_model"]`, when set.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the value is not a `ModelKey` string,
    /// instead of silently pricing with the default model.
    pub(crate) fn pricing_model_override(
        &self,
    ) -> finstack_quant_core::Result<Option<crate::pricer::ModelKey>> {
        self.attributes
            .get_meta("pricing_model")
            .map(|model| {
                <crate::pricer::ModelKey as ::std::str::FromStr>::from_str(model).map_err(|_| {
                    finstack_quant_core::Error::Validation(format!(
                        "RevolvingCredit '{}': unknown pricing_model '{model}'",
                        self.id
                    ))
                })
            })
            .transpose()
    }

    /// Fronting fee in force, in basis points per annum (zero without an LC
    /// sub-facility).
    pub fn fronting_fee_bp(&self) -> f64 {
        self.lc.as_ref().map_or(0.0, |lc| lc.fronting_fee_bp)
    }

    /// Letter-of-credit face assumed drawn at default on `date`:
    /// `lc.leq × LC outstanding`, in the facility currency.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the contingent exposure is wanted for.
    pub fn lc_exposure_at_default(&self, date: Date) -> f64 {
        self.lc
            .as_ref()
            .map_or(0.0, |lc| lc.leq * self.lc_outstanding_at(date).amount())
    }

    /// Upfront fee amount in the facility currency (zero when none), resolved
    /// against the opening commitment.
    pub fn upfront_fee_amount(&self) -> Money {
        self.fees.upfront_fee.as_ref().map_or(
            Money::from((0_i64, self.commitment_amount.currency())),
            |fee| fee.amount(self.commitment_amount),
        )
    }

    /// Scheduled fixed fees dated after `as_of`, as `(date, amount)`.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date; fees dated on or before it are history.
    pub fn scheduled_fees_after(&self, as_of: Date) -> Vec<(Date, Money)> {
        self.scheduled_fees
            .iter()
            .filter(|fee| fee.date > as_of && fee.amount.amount() > 0.0)
            .map(|fee| (fee.date, fee.amount))
            .collect()
    }

    /// Reduction fees payable on commitment step-downs dated after `as_of`:
    /// `(previous commitment − new commitment) × fee_bp`, one entry per step
    /// that lowers the commitment and carries a positive fee.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date; steps dated on or before it are history.
    ///
    /// # Errors
    ///
    /// Returns an error when a fee amount is not representable as `Money`.
    pub fn commitment_step_fees(
        &self,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let mut previous = self.commitment_amount;
        let mut fees = Vec::new();
        for step in &self.commitment_schedule {
            let reduction = previous.amount() - step.amount.amount();
            if step.date > as_of && reduction > 0.0 && step.fee_bp > 0.0 {
                fees.push((
                    step.date,
                    Money::new(reduction * step.fee_bp * 1e-4, previous.currency())?,
                ));
            }
            previous = step.amount;
        }
        Ok(fees)
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
        // `validate` rejects an unknown `pricing_model`, so only a valid
        // override or none reaches here.
        self.pricing_model_override()
            .ok()
            .flatten()
            .unwrap_or(crate::pricer::ModelKey::Discounting)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        // Optional model override via attributes metadata (e.g., meta["pricing_model"] = "monte_carlo_gbm")
        if let Some(model) = self.pricing_model_override()? {
            let registry = crate::pricer::standard_pricer_registry();
            let result = registry
                .price_with_metrics(self, model, curves, as_of, &[], Default::default())
                .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
            return Ok(result.value);
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
            facility.commitment_date,
            date!(2025 - 12 - 31),
        )
        .expect("balance");
        assert_eq!(balance.amount(), 10_000_000.0);
    }

    #[test]
    fn validation_rejects_events_outside_life() {
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
