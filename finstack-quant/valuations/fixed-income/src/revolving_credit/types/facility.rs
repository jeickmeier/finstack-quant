//! Core [`RevolvingCredit`] instrument struct and builders.

use super::{DrawRepayEvent, DrawRepaySpec, RevolvingCreditFees};
use crate::loan_terms::{CommitmentStep, LetterOfCreditSpec, MarginStep, OidEirSpec, ScheduledFee};
use finstack_quant_cashflows::builder::{FloatingRateSpec, RateSpec};
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId, InstrumentId};
use rust_decimal::Decimal;

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
#[serde(deny_unknown_fields, remote = "Self")]
pub struct RevolvingCredit {
    /// Unique identifier for the facility.
    pub id: InstrumentId,

    /// Opening commitment of the facility, in force from `issue_date`
    /// until the first entry of `commitment_steps`.
    pub commitment: Money,

    /// Scheduled commitment changes (amortizing commitments, availability
    /// expiries, accordions), each in force from its date until the next.
    /// Dates must be strictly increasing, after `issue_date` and on or
    /// before `maturity`; the drawn balance plus outstanding letters of
    /// credit must never exceed the commitment in force. A step down pays its
    /// `reduction_fee_bp` on the reduced amount. Empty by default.
    #[builder(default)]
    #[serde(default)]
    pub commitment_steps: Vec<CommitmentStep>,

    /// Dated margin changes, cumulative from their dates: a leverage or
    /// ratings grid the analyst has forecast, a scheduled step-up or a
    /// default-rate margin. `delta_bp` shifts the floating spread or the fixed
    /// rate. Dates must be strictly increasing and strictly inside the
    /// facility life. Empty by default.
    #[builder(default)]
    #[serde(default)]
    pub margin_steps: Vec<MarginStep>,

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

    /// Effective-interest-rate reporting switch for the `oid_eir_rate` and
    /// `oid_eir_amortization` metrics. `None` (the default) reports with fees
    /// included, the same as `Some(OidEirSpec::default())`.
    #[builder(default)]
    #[serde(default)]
    pub oid_eir: Option<OidEirSpec>,

    /// Drawn balance at the simulation anchor, the later of `issue_date`
    /// and the valuation date, in both deterministic and stochastic mode.
    ///
    /// For a new facility this is the balance funded at commitment; for a
    /// seasoned facility it is the balance observed on the valuation date.
    /// Deterministic draw/repay events describe the future only: an event
    /// dated on or before the valuation date is rejected by the cashflow
    /// engine, because the position at the anchor is defined by this field
    /// alone. The accrual period containing the valuation date accrues on
    /// this balance from its accrual start.
    pub drawn: Money,

    /// Date when the facility becomes available.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub issue_date: Date,

    /// Date when the facility expires.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,

    /// Base rate specification (fixed or floating).
    pub rate: RateSpec,

    /// Day count convention for interest accrual.
    pub day_count: DayCount,

    /// Annual basis of this loan's yield quotes (yield to maturity, call and
    /// worst) and of its effective-interest-rate accretion, separate from the
    /// coupon accrual `day_count` (resolution O11, record
    /// CORE-CONVENTIONS-017 #12). Required: there is no default basis.
    pub yield_basis: finstack_quant_valuations_core::metrics::returns::YieldBasis,

    /// Payment frequency for interest and fees.
    pub frequency: Tenor,

    /// Quote compounding of the discount margin and z-spread (pricing,
    /// quote inversion and CS01), stated independently of `frequency`
    /// (record CORE-CONVENTIONS-007 #9, O6). Required: there is no default.
    pub quote_compounding: finstack_quant_core::math::Compounding,

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
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,

    /// Holiday calendar identifier (for example `"usny"`) used to adjust
    /// payment dates, roll fixing dates and count settlement days. `None`
    /// adjusts for weekends only. Validation rejects an unknown identifier.
    /// `FloatingRateSpec::fixing_calendar_id` overrides it for the fixing
    /// date alone.
    #[builder(default)]
    #[serde(default)]
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,

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

    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

finstack_quant_valuations_core::validate_on_deserialize!(RevolvingCredit);

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
        let base_rate = RateSpec::Floating(FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
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
            compounding: None,
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
            .commitment(commitment)
            .drawn(initial_draw)
            .issue_date(start)
            .maturity(end)
            .rate(base_rate)
            .day_count(DayCount::Act360)
            .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
            .frequency(Tenor::quarterly())
            .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
            .fees(fees)
            .draw_repay_spec(draw_repay)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .recovery_rate(0.0)
            .stub(StubKind::ShortFront)
            .calendar_id("usny".into())
            .attributes(Attributes::new())
            .build()
    }
}
