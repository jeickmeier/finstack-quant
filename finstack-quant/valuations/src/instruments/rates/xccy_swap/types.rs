//! XCCY swap types and pricing.
//!
//! Market-standard conventions implemented:
//! - Floating coupons projected from forward curves on accrual boundaries
//! - Cashflows discounted with the leg-specific discount curve (multi-curve)
//! - Leg PVs converted into the reporting currency using spot FX at `as_of`
//! - Explicit calendars / business-day conventions; no implicit calendar fallbacks
//!
//! Notes:
//! - Reset lag is modeled via `build_periods` (`reset_lag_days` sets the fixing date).
//! - Term legs project a simple forward over the accrual window. Overnight RFR
//!   legs (`compounding` other than Simple) use the shared daily compounded
//!   projector. Historical fixings are required once the fixing date is past.

use crate::cashflow::builder::{schedule::merge_cashflow_schedules, CashFlowSchedule, Notional};
use crate::cashflow::primitives::CFKind;
use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::parameters::legs::FloatLegSpec;
use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::PayReceive;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;
use rust_decimal::Decimal;

/// Threshold for extremely negative forward rates that warrant a warning.
/// Even JPY/CHF/EUR rarely go below -1%, so -5% indicates potential curve issues.
const EXTREME_NEGATIVE_RATE_THRESHOLD: f64 = -0.05;

/// Projected economics of one XCCY floating period.
///
/// Overnight legs store the shared projector's compound factor and
/// holiday-adjusted year fraction. Term legs reconstruct the same
/// `N × (CF − 1) + N × spread × τ` coupon from the simple forward.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProjectedXccyPeriod {
    /// Index rate excluding spread.
    pub rate: f64,
    /// Accrual fraction used for the coupon amount.
    pub year_fraction: f64,
    /// Last overnight observation or the term fixing date.
    pub fixing_date: Option<Date>,
    /// Compound factor `∏(1 + rᵢ dᵢ)` or `1 + rate × τ` for term legs.
    compound_factor: f64,
}

impl ProjectedXccyPeriod {
    /// Unsigned coupon `N × (compound_factor − 1) + N × spread × τ`.
    ///
    /// # Arguments
    ///
    /// * `notional` - Period notional in the leg currency. MtM resetting
    ///   legs pass the reset notional, not the original contractual amount.
    /// * `spread` - Arithmetic spread in decimal rate units, not basis points.
    pub(crate) fn unsigned_coupon(&self, notional: f64, spread: f64) -> f64 {
        notional * (self.compound_factor - 1.0) + notional * spread * self.year_fraction
    }

    /// All-in simple rate including the arithmetic spread.
    ///
    /// # Arguments
    ///
    /// * `spread` - Arithmetic spread in decimal rate units, not basis points.
    pub(crate) fn all_in_rate(&self, spread: f64) -> f64 {
        self.rate + spread
    }
}

/// Returns the sign of a leg's initial principal exchange.
///
/// # Market Convention
///
/// The leg you receive is economically a lending position: you pay out the
/// principal at the start (`-1.0`), receive coupons, and receive the principal
/// back at the end. For a USD/EUR XCCY swap where you receive USD, the initial
/// exchange pays USD notional to the counterparty. This follows the ISDA
/// convention that the receiver of a leg provides the initial funding in that
/// currency.
///
/// # Arguments
///
/// * `side` - Whether the holder pays or receives the leg's coupons.
#[inline]
pub(crate) fn initial_principal_sign(side: PayReceive) -> f64 {
    -side.sign()
}

/// Notional exchange convention for XCCY swaps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_export", ts(export, rename_all = "snake_case"))]
#[non_exhaustive]
pub enum NotionalExchange {
    /// No principal exchange.
    None,
    /// Exchange principal at maturity only.
    Final,
    /// Exchange principal at start and maturity (typical for fixed-notional XCCY basis swaps).
    #[default]
    InitialAndFinal,
    /// Mark-to-market resetting. The notional of `resetting_side` is re-marked at each
    /// of its coupon reset dates to match the constant leg's notional in current FX.
    /// A rebalancing cashflow is paid on the resetting leg only — the constant leg's
    /// principal-and-coupon schedule is unchanged, matching standard MtM-XCCY market
    /// convention (QuantLib's `MtMCrossCurrencyBasisSwap` follows the same pattern).
    /// Under CIP no-FX-vol the constant-currency leg of the FX swap that funds the
    /// rebalancing is PV-fair from today's perspective, so the resetting-leg flow is
    /// the only cashflow that needs to be emitted explicitly. Implies initial AND
    /// final principal exchange.
    MtmResetting {
        /// Which leg (`Leg1` or `Leg2`) has its notional reset each period.
        resetting_side: ResettingSide,
    },
}

impl std::fmt::Display for NotionalExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Final => write!(f, "final"),
            Self::InitialAndFinal => write!(f, "initial_and_final"),
            Self::MtmResetting { resetting_side } => {
                write!(f, "mtm_resetting:{resetting_side}")
            }
        }
    }
}

/// Identifies which leg of an XCCY swap has its notional reset under
/// MtM-resetting. `Leg1` and `Leg2` refer to `XccySwap::leg1` and `XccySwap::leg2`
/// respectively.
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_export", ts(export, rename_all = "snake_case"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ResettingSide {
    /// The first leg (`XccySwap::leg1`) has its notional reset each period.
    Leg1,
    /// The second leg (`XccySwap::leg2`) has its notional reset each period.
    Leg2,
}

impl std::fmt::Display for ResettingSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leg1 => write!(f, "leg1"),
            Self::Leg2 => write!(f, "leg2"),
        }
    }
}

impl std::str::FromStr for ResettingSide {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "leg1" => Ok(Self::Leg1),
            "leg2" => Ok(Self::Leg2),
            _ => Err(format!("Unknown resetting side: '{s}'. Valid: leg1, leg2")),
        }
    }
}

/// One floating leg of an XCCY swap.
///
/// The leg's schedule, curves, spread, lags and compounding are a canonical
/// [`FloatLegSpec`]; the leg currency is `notional`'s currency. A reset lag of
/// `0` fixes on the accrual start; negative reset lags are rejected.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XccySwapLeg {
    /// Leg notional in the leg currency.
    pub notional: Money,
    /// Pay/receive direction for this leg.
    pub side: PayReceive,
    /// Floating-leg terms (curves, dates, frequency, spread in bp, lags,
    /// calendars, compounding).
    pub leg: FloatLegSpec,
}

impl XccySwapLeg {
    /// Reset lag handed to the period builder: `0` fixes on the accrual start
    /// and is passed as "no separate reset date".
    pub(crate) fn period_reset_lag(&self) -> Option<i32> {
        (self.leg.reset_lag_days != 0).then_some(self.leg.reset_lag_days)
    }
}

/// Cross-currency floating-for-floating swap.
///
/// Each leg owns its own dates, stub conventions, and calendar. The parent struct
/// only holds the instrument identity, notional exchange mode, and reporting currency.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XccySwap {
    /// Unique identifier for this instrument.
    pub id: InstrumentId,
    /// First leg.
    pub leg1: XccySwapLeg,
    /// Second leg.
    pub leg2: XccySwapLeg,
    /// Whether and when principal is exchanged.
    #[serde(default)]
    pub notional_exchange: NotionalExchange,
    /// PV reporting currency (output currency of `value`/`npv`).
    pub reporting_currency: Currency,
    /// Allow a weekends-only calendar when either leg's `leg.calendar_id` is
    /// missing or cannot be resolved.
    ///
    /// When `false` (default), missing calendars are treated as input errors.
    #[builder(default)]
    #[serde(default)]
    pub allow_calendar_fallback: bool,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
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
    /// Attributes for instrument selection and tagging.
    pub attributes: crate::instruments::common_impl::traits::Attributes,
}

impl XccySwap {
    /// Convenience constructor.
    ///
    /// Dates and stub conventions are now owned by each leg.
    pub fn new(
        id: impl Into<String>,
        leg1: XccySwapLeg,
        leg2: XccySwapLeg,
        reporting_currency: Currency,
    ) -> Self {
        Self {
            id: InstrumentId::new(id.into()),
            leg1,
            leg2,
            notional_exchange: NotionalExchange::InitialAndFinal,
            reporting_currency,
            allow_calendar_fallback: false,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: crate::instruments::common_impl::traits::Attributes::default(),
        }
    }

    /// Create a canonical example USD/EUR 5Y cross-currency basis swap ($10M notional).
    ///
    /// Returns a 5-year XCCY swap with quarterly SOFR on the USD leg
    /// and quarterly EURIBOR on the EUR leg, with initial and final notional exchange.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use time::Month;

        let start = Date::from_calendar_date(2024, Month::January, 3).expect("Valid example date");
        let end = Date::from_calendar_date(2029, Month::January, 3).expect("Valid example date");

        let usd_leg = XccySwapLeg {
            notional: Money::from((10_000_000_i64, Currency::USD)),
            side: PayReceive::Receive,
            leg: FloatLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: Decimal::ZERO,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::ShortFront,
                reset_lag_days: 0,
                fixing_calendar_id: None,
                start,
                end,
                compounding: Default::default(),
                payment_lag_days: 0,
                end_of_month: false,
            },
        };

        let eur_leg = XccySwapLeg {
            notional: Money::from((9_200_000_i64, Currency::EUR)),
            side: PayReceive::Pay,
            leg: FloatLegSpec {
                discount_curve_id: CurveId::new("EUR-OIS"),
                forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                spread_bp: Decimal::from(10),
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::ShortFront,
                reset_lag_days: 0,
                fixing_calendar_id: None,
                start,
                end,
                compounding: Default::default(),
                payment_lag_days: 0,
                end_of_month: false,
            },
        };

        let mut swap = Self::new("XCCY-USDEUR-5Y", usd_leg, eur_leg, Currency::USD);
        swap.allow_calendar_fallback = true;
        swap
    }

    /// Set notional exchange convention.
    pub fn with_notional_exchange(mut self, exchange: NotionalExchange) -> Self {
        self.notional_exchange = exchange;
        self
    }

    /// Partition the two legs into `(constant_leg, resetting_leg)` based on the
    /// given side. Errors if both legs share a currency (already guarded by
    /// `validate_leg`, but this surfaces the intent explicitly).
    pub(crate) fn partition_legs(
        &self,
        resetting_side: ResettingSide,
    ) -> Result<(&XccySwapLeg, &XccySwapLeg)> {
        let (constant, resetting) = match resetting_side {
            ResettingSide::Leg1 => (&self.leg2, &self.leg1),
            ResettingSide::Leg2 => (&self.leg1, &self.leg2),
        };
        if constant.notional.currency() == resetting.notional.currency() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}': MtM-reset partition requires different currencies on the two legs; both are {}",
                self.id, constant.notional.currency()
            )));
        }
        Ok((constant, resetting))
    }

    /// Validate the swap's static configuration.
    ///
    /// Checks each leg independently (notional currency consistency, finite/positive
    /// notional, non-negative payment lag) and then applies additional guards
    /// when [`NotionalExchange::MtmResetting`] is configured:
    ///
    /// - The two legs must have different currencies (`partition_legs` guard).
    /// - Both legs must share the same start and end dates (schedule alignment).
    ///
    /// Frequency, day count, calendar, BDC, stub, payment lag, and reset lag are
    /// leg-specific and are applied independently by the MtM pricer.
    ///
    /// FX-matrix reachability requires a runtime `MarketContext` and is therefore
    /// checked separately by `validate_fx_reachable` at the start of
    /// `base_value`. A passing `validate()` does *not* imply the swap is priceable —
    /// it only guarantees the static configuration is well-formed.
    pub fn validate(&self) -> Result<()> {
        self.validate_leg(&self.leg1)?;
        self.validate_leg(&self.leg2)?;

        // MtM notional exchanges require aligned contractual start/end dates;
        // coupon schedules remain leg-specific.
        crate::instruments::common_impl::pricing::overnight_conventions::reject_simple_overnight(
            self.leg1.leg.forward_curve_id.as_str(),
            &self.leg1.leg.compounding,
        )?;
        crate::instruments::common_impl::pricing::overnight_conventions::reject_simple_overnight(
            self.leg2.leg.forward_curve_id.as_str(),
            &self.leg2.leg.compounding,
        )?;

        if let NotionalExchange::MtmResetting { resetting_side } = &self.notional_exchange {
            // Confirm resetting_side resolves and yields different currencies.
            self.partition_legs(*resetting_side)?;

            if self.leg1.leg.start != self.leg2.leg.start || self.leg1.leg.end != self.leg2.leg.end
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "XccySwap '{}': MtmResetting requires both legs to share start and end \
                     dates (schedule alignment), got leg1=[{}, {}] leg2=[{}, {}]",
                    self.id,
                    self.leg1.leg.start,
                    self.leg1.leg.end,
                    self.leg2.leg.start,
                    self.leg2.leg.end
                )));
            }
        }

        Ok(())
    }

    /// Pre-flight check that every leg whose currency differs from
    /// [`Self::reporting_currency`] is reachable through the market's FX matrix.
    ///
    /// Runs at the top of `Instrument::base_value` so a missing or
    /// underspecified FX matrix surfaces as a single, informative error
    /// (naming both currencies and the offending leg) rather than a generic
    /// `NotFound { id: "fx_matrix" }` raised mid-loop deep inside cashflow
    /// conversion.
    ///
    /// We probe with `as_of`-equivalent tenor `payment_date = leg.start` (or
    /// any concrete date), since [`finstack_quant_core::money::fx::FxMatrix`] resolves
    /// reachability up front independent of forward-date specifics.
    fn validate_fx_reachable(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
    ) -> Result<()> {
        let needs_fx = self.leg1.notional.currency() != self.reporting_currency
            || self.leg2.notional.currency() != self.reporting_currency;
        if !needs_fx {
            return Ok(());
        }
        let fx = market.fx().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}' requires fx_matrix in market context: leg1={} leg2={} reporting={}",
                self.id.as_str(),
                self.leg1.notional.currency(),
                self.leg2.notional.currency(),
                self.reporting_currency,
            ))
        })?;

        for (label, leg) in [("leg1", &self.leg1), ("leg2", &self.leg2)] {
            if leg.notional.currency() == self.reporting_currency {
                continue;
            }
            // Probe FX with a representative payment date (leg.start). Reachability
            // failure here will surface as a precise currency-pair error rather than
            // a generic NotFound from the inner cashflow loop.
            fx.rate(FxQuery::new(
                leg.notional.currency(),
                self.reporting_currency,
                leg.leg.start,
            ))
            .map_err(|err| {
                finstack_quant_core::Error::Validation(format!(
                    "XccySwap '{}' FX path unreachable for {}: {}->{} ({})",
                    self.id.as_str(),
                    label,
                    leg.notional.currency(),
                    self.reporting_currency,
                    err,
                ))
            })?;
        }
        Ok(())
    }

    fn validate_leg(&self, leg: &XccySwapLeg) -> Result<()> {
        if leg.leg.compounding
            == crate::instruments::rates::irs::FloatingLegCompounding::SimpleAverage
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}' leg.compounding = simple_average is not supported; \
                 use simple or a compounded_* variant",
                self.id
            )));
        }
        if leg.leg.fixing_calendar_id.is_some() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}' leg.fixing_calendar_id is not supported; the reset lag \
                 rolls on leg.calendar_id",
                self.id
            )));
        }
        if !leg.notional.amount().is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap leg notional must be finite".to_string(),
            ));
        }
        if leg.notional.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap leg notional must be positive".to_string(),
            ));
        }
        if leg.leg.payment_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap payment lag must be non-negative".to_string(),
            ));
        }
        if leg.leg.reset_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap reset lag must be non-negative".to_string(),
            ));
        }
        let calendar_resolves = leg.leg.calendar_id.as_deref().is_some_and(|id| {
            crate::cashflow::builder::calendar::resolve_calendar_strict(id).is_ok()
        });
        if !calendar_resolves && !self.allow_calendar_fallback {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}' leg {} requires a resolvable calendar_id; set allow_calendar_fallback=true to opt into weekends_only",
                self.id, leg.notional.currency()
            )));
        }
        // Decimal is always finite; no NaN/infinity check required.
        Ok(())
    }

    /// Resolve the calendar ID used for both schedule build and overnight
    /// observation. Unresolved IDs error unless `allow_calendar_fallback`.
    ///
    /// # Arguments
    ///
    /// * `leg` - XCCY leg whose `calendar_id` is resolved; the swap-level
    ///   `allow_calendar_fallback` decides whether an unresolved ID falls back.
    pub(crate) fn resolve_leg_calendar_id<'a>(&self, leg: &'a XccySwapLeg) -> Result<&'a str> {
        match leg.leg.calendar_id.as_deref() {
            Some(id)
                if crate::cashflow::builder::calendar::resolve_calendar_strict(id).is_ok() =>
            {
                Ok(id)
            }
            _ if self.allow_calendar_fallback => {
                Ok(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID)
            }
            _ => Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap leg {} requires a resolvable calendar_id; set allow_calendar_fallback=true to opt into weekends_only",
                leg.notional.currency()
            ))),
        }
    }

    /// Holiday calendar matching [`Self::resolve_leg_calendar_id`].
    ///
    /// # Arguments
    ///
    /// * `leg` - XCCY leg whose resolved calendar ID is turned into a calendar.
    fn resolve_leg_calendar(
        &self,
        leg: &XccySwapLeg,
    ) -> Result<&'static dyn finstack_quant_core::dates::HolidayCalendar> {
        crate::cashflow::builder::calendar::resolve_calendar_strict(
            self.resolve_leg_calendar_id(leg)?,
        )
    }

    /// Build one leg's accrual periods. Single source for both the pricing
    /// path (`pv_leg_in_reporting_currency`) and the reporting path
    /// (`leg_coupon_schedule`), so period boundaries, payment dates, and
    /// reset dates can never drift between priced and reported cashflows.
    fn leg_build_periods(
        &self,
        leg: &XccySwapLeg,
        calendar_id: &str,
    ) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
        crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: leg.leg.start,
                end: leg.leg.end,
                frequency: leg.leg.frequency,
                stub: leg.leg.stub,
                business_day_convention: leg.leg.business_day_convention,
                calendar_id,
                end_of_month: leg.leg.end_of_month,
                day_count: leg.leg.day_count,
                payment_lag_days: leg.leg.payment_lag_days,
                reset_lag_days: leg.period_reset_lag(),
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        )
    }

    /// Projected index rate, coupon year fraction, and compound factor for one
    /// floating period. Overnight legs use the shared projector on the same
    /// calendar that built the schedule.
    ///
    /// # Arguments
    ///
    /// * `leg` - Floating XCCY leg supplying compounding, day count, and calendar.
    /// * `fwd` - Forward curve used to project unfixed overnight or term rates.
    /// * `fixings` - Optional historical fixing series for past reset dates.
    /// * `period` - Accrual period whose coupon is being projected.
    /// * `as_of` - Valuation date that splits realized fixings from forwards.
    /// * `projected_fixings` - Optional schedule sink for raw rate observations;
    ///   cashflow construction supplies it, while pure PV calls use `None`.
    pub(crate) fn projected_leg_period(
        &self,
        leg: &XccySwapLeg,
        fwd: &finstack_quant_core::market_data::term_structures::ForwardCurve,
        fixings: Option<&finstack_quant_core::market_data::scalars::ScalarTimeSeries>,
        period: &crate::cashflow::builder::periods::SchedulePeriod,
        as_of: Date,
        projected_fixings: Option<&mut Vec<crate::cashflow::fixings::ProjectedFixing>>,
    ) -> Result<ProjectedXccyPeriod> {
        use crate::instruments::common_impl::pricing::overnight::{
            adjust_overnight_accrual_boundaries, project_overnight_coupon,
            OvernightCouponProjectionInput, OvernightProjectionCurve,
        };
        use crate::instruments::common_impl::pricing::time::rate_between_on_dates;
        use crate::instruments::rates::irs::FloatingLegCompounding;

        let projected = if !matches!(leg.leg.compounding, FloatingLegCompounding::Simple) {
            let calendar = self.resolve_leg_calendar(leg)?;
            let (accrual_start, accrual_end) = adjust_overnight_accrual_boundaries(
                period.accrual_start,
                period.accrual_end,
                leg.leg.business_day_convention,
                calendar,
            )?;
            if accrual_end <= accrual_start {
                ProjectedXccyPeriod {
                    rate: 0.0,
                    year_fraction: 0.0,
                    fixing_date: None,
                    compound_factor: 1.0,
                }
            } else {
                let projection = project_overnight_coupon(OvernightCouponProjectionInput {
                    curve: OvernightProjectionCurve::Forward(fwd),
                    fixings,
                    fixing_id: leg.leg.forward_curve_id.as_str(),
                    as_of,
                    accrual_start,
                    accrual_end,
                    day_count: leg.leg.day_count,
                    coupon_frequency: Some(leg.leg.frequency),
                    compounding: &leg.leg.compounding,
                    fixing_calendar: calendar,
                    compounded_spread: 0.0,
                    need_observation_exposures: projected_fixings.is_some(),
                })?;
                if let Some(out) = projected_fixings {
                    out.extend(projection.observation_exposures.iter().map(|observation| {
                        crate::cashflow::fixings::ProjectedFixing {
                            series_id: format!("FIXING:{}", leg.leg.forward_curve_id),
                            date: observation.observation_start,
                            value: Some(observation.projected_rate),
                        }
                    }));
                }
                ProjectedXccyPeriod {
                    rate: projection.rate,
                    year_fraction: projection.accrual_year_fraction,
                    fixing_date: Some(projection.fixing_date),
                    compound_factor: projection.compound_factor,
                }
            }
        } else {
            let fixing_date = period.reset_date.unwrap_or(period.accrual_start);
            let forward_rate = if fixing_date < as_of {
                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                    fixings,
                    leg.leg.forward_curve_id.as_str(),
                    fixing_date,
                    as_of,
                )?
            } else {
                rate_between_on_dates(fwd, period.accrual_start, period.accrual_end)?
            };
            if let Some(out) = projected_fixings {
                out.push(crate::cashflow::fixings::ProjectedFixing {
                    series_id: format!("FIXING:{}", leg.leg.forward_curve_id),
                    date: fixing_date,
                    value: Some(forward_rate),
                });
            }
            ProjectedXccyPeriod {
                rate: forward_rate,
                year_fraction: period.accrual_year_fraction,
                fixing_date: Some(fixing_date),
                compound_factor: 1.0 + forward_rate * period.accrual_year_fraction,
            }
        };
        if !projected.rate.is_finite() || !projected.compound_factor.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Non-finite forward rate for period {} to {}",
                period.accrual_start, period.accrual_end
            )));
        }
        Ok(projected)
    }

    /// Coupon schedule for one leg, projected through the SAME rate path the
    /// pricer discounts (`projected_leg_period`): window-consistent
    /// forwards for future fixings, recorded fixings for past ones. Keeping a
    /// single projection prevents the reported cashflows from silently
    /// drifting away from what `base_value` actually prices.
    fn leg_coupon_schedule(
        &self,
        leg: &XccySwapLeg,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<CashFlowSchedule> {
        let calendar_id = self.resolve_leg_calendar_id(leg)?;
        let periods = self.leg_build_periods(leg, calendar_id)?;
        let fwd = market.get_forward(&leg.leg.forward_curve_id)?;
        let fixing_series_id = finstack_quant_core::market_data::fixings::fixing_series_id(
            leg.leg.forward_curve_id.as_str(),
        );
        let fixings = market.get_series(&fixing_series_id).ok();
        let spread = decimal_to_f64(leg.leg.spread_bp, "XccySwap leg spread_bp")? / 10_000.0;

        let mut flows = Vec::with_capacity(periods.len());
        let mut projected_fixings = Vec::new();
        for period in &periods {
            let projected = self.projected_leg_period(
                leg,
                fwd.as_ref(),
                fixings,
                period,
                as_of,
                Some(&mut projected_fixings),
            )?;
            let all_in = projected.all_in_rate(spread);
            let amount = leg.side.sign() * projected.unsigned_coupon(leg.notional.amount(), spread);
            flows.push(crate::cashflow::primitives::CashFlow::new(
                period.payment_date,
                projected.fixing_date.or(period.reset_date),
                Money::new(amount, leg.notional.currency())?,
                crate::cashflow::primitives::CFKind::FloatReset,
                projected.year_fraction,
                Some(all_in),
            ));
        }
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            leg.leg.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(leg.notional),
                meta: crate::cashflow::builder::CashFlowMeta {
                    projected_fixings,
                    ..Default::default()
                },
            },
        ))
    }

    fn leg_principal_schedule(&self, leg: &XccySwapLeg, anchor: Date) -> Result<CashFlowSchedule> {
        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(
            Money::from((0_i64, leg.notional.currency())),
            anchor,
            leg.leg.end,
        );
        // MtmResetting also requires initial AND final exchange; this arm makes the helper non-panicky if accidentally called on an MtM swap. `base_value` dispatches MtmResetting to `pricing_mtm::pv_mtm_reset` before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) {
            let initial_amount = initial_principal_sign(leg.side) * leg.notional.amount();
            let _ = builder.add_principal_event(
                leg.leg.start,
                leg.leg.start,
                Money::from((0_i64, leg.notional.currency())),
                Some(Money::new(-initial_amount, leg.notional.currency())?),
                CFKind::Notional,
            );
        }
        // MtmResetting also requires final exchange; same defensive note as above — the MtM live-pricing path dispatches before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) {
            let final_amount = leg.side.sign() * leg.notional.amount();
            let _ = builder.add_principal_event(
                leg.leg.end,
                leg.leg.end,
                Money::from((0_i64, leg.notional.currency())),
                Some(Money::new(-final_amount, leg.notional.currency())?),
                CFKind::Notional,
            );
        }
        Ok(builder.build(None)?.with_notional(Notional::par(
            leg.notional.amount(),
            leg.notional.currency(),
        )?))
    }

    /// Calculate the present value of a leg and convert that PV at valuation-date spot.
    ///
    /// # Market-Standard FX Conversion
    ///
    /// Cashflows are first discounted on their own currency curve. The resulting
    /// foreign-currency PV must therefore be converted at valuation-date spot.
    /// Multiplying an already foreign-discounted amount by a payment-date forward
    /// FX would count the foreign/domestic carry twice.
    ///
    /// # Returns
    ///
    /// Present value in the **reporting currency**, not the leg currency.
    fn pv_leg_in_reporting_currency(
        &self,
        leg: &XccySwapLeg,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        self.validate_leg(leg)?;

        let calendar_id = self.resolve_leg_calendar_id(leg)?;
        let periods = self.leg_build_periods(leg, calendar_id)?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap leg schedule must contain at least 1 period".to_string(),
            ));
        }

        let unsettled_coupon = periods.iter().any(|period| period.payment_date > as_of);
        let unsettled_initial = matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) && leg.leg.start > as_of;
        let unsettled_final = matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) && leg.leg.end > as_of;
        if !unsettled_coupon && !unsettled_initial && !unsettled_final {
            return Ok(Money::from((0_i64, self.reporting_currency)));
        }

        let disc = context.get_discount(&leg.leg.discount_curve_id)?;
        let fwd = context.get_forward(&leg.leg.forward_curve_id)?;
        let fixing_series_id = finstack_quant_core::market_data::fixings::fixing_series_id(
            leg.leg.forward_curve_id.as_str(),
        );
        let fixings = context.get_series(&fixing_series_id).ok();
        let fx = context.fx();

        let mut pv = NeumaierAccumulator::new();

        // Convert an already-discounted leg-currency PV at valuation-date spot.
        let convert_pv = |amount: f64| -> Result<f64> {
            if leg.notional.currency() == self.reporting_currency {
                return Ok(amount);
            }
            let fx_matrix = fx.ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "fx_matrix".to_string(),
                })
            })?;

            let rate = fx_matrix
                .rate(FxQuery::new(
                    leg.notional.currency(),
                    self.reporting_currency,
                    as_of,
                ))?
                .rate;
            Ok(amount * rate)
        };

        // Notional exchanges (principal)
        // Use relative date-based discounting (validated against Bloomberg SWPM).
        // MtmResetting also requires initial AND final exchange; this arm makes the helper non-panicky if accidentally called on an MtM swap. `base_value` dispatches MtmResetting to `pricing_mtm::pv_mtm_reset` before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) && leg.leg.start > as_of
        {
            let df = relative_df_discount_curve(disc.as_ref(), as_of, leg.leg.start)?;
            let cf_leg_currency = initial_principal_sign(leg.side) * leg.notional.amount() * df;
            let cf_rep = convert_pv(cf_leg_currency)?;
            pv.add(cf_rep);
        }

        // MtmResetting also requires final exchange; same defensive note as above — the MtM live-pricing path dispatches before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) && leg.leg.end > as_of
        {
            let df = relative_df_discount_curve(disc.as_ref(), as_of, leg.leg.end)?;
            let cf_leg_currency = leg.side.sign() * leg.notional.amount() * df;
            let cf_rep = convert_pv(cf_leg_currency)?;
            pv.add(cf_rep);
        }

        // Floating coupons
        for period in periods {
            if period.payment_date <= as_of {
                continue;
            }

            let projected =
                self.projected_leg_period(leg, fwd.as_ref(), fixings, &period, as_of, None)?;
            // Warn about extremely negative forward rates which may indicate curve issues.
            // Even in negative rate environments (JPY/CHF/EUR), rates below -5% are unusual.
            if projected.rate < EXTREME_NEGATIVE_RATE_THRESHOLD {
                tracing::warn!(
                    instrument_id = %self.id.as_str(),
                    period_start = %period.accrual_start,
                    period_end = %period.accrual_end,
                    forward_rate = projected.rate,
                    threshold = EXTREME_NEGATIVE_RATE_THRESHOLD,
                    "Forward rate is highly negative; verify curve construction"
                );
            }

            let spread = decimal_to_f64(leg.leg.spread_bp, "XccySwap leg spread_bp")? / 10_000.0;
            let coupon = leg.side.sign() * projected.unsigned_coupon(leg.notional.amount(), spread);

            // Use relative date-based discounting for numerical stability.
            let df = relative_df_discount_curve(disc.as_ref(), as_of, period.payment_date)?;
            let cf_leg_currency = coupon * df;

            let cf_rep = convert_pv(cf_leg_currency)?;
            pv.add(cf_rep);
        }

        Money::new(pv.total(), self.reporting_currency)
    }
}

impl crate::instruments::common_impl::traits::Instrument for XccySwap {
    impl_instrument_base!(crate::pricer::InstrumentType::XccySwap);
    fn last_payment_date(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<finstack_quant_core::dates::Date>> {
        let schedule =
            crate::cashflow::traits::CashflowProvider::cashflow_schedule(self, curves, as_of)?;
        Ok(schedule
            .get_flows()
            .iter()
            .map(|flow| flow.date)
            .chain(self.expiry())
            .max())
    }

    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        XccySwap::validate(self)
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.leg1.leg.discount_curve_id.clone());
        deps.add_discount_curve(self.leg2.leg.discount_curve_id.clone());
        deps.add_forward_curve(self.leg1.leg.forward_curve_id.clone());
        deps.add_forward_curve(self.leg2.leg.forward_curve_id.clone());
        if self.leg1.notional.currency() != self.reporting_currency {
            deps.add_fx_pair(self.leg1.notional.currency(), self.reporting_currency);
        }
        if self.leg2.notional.currency() != self.reporting_currency {
            deps.add_fx_pair(self.leg2.notional.currency(), self.reporting_currency);
        }
        deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
            self.leg1.leg.forward_curve_id.as_str(),
        ));
        deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
            self.leg2.leg.forward_curve_id.as_str(),
        ));
        if let NotionalExchange::MtmResetting { resetting_side } = self.notional_exchange {
            let (constant_leg, resetting_leg) = self.partition_legs(resetting_side)?;
            deps.add_series_id(
                crate::instruments::rates::xccy_swap::pricing_mtm::mtm_fx_fixing_series_id(
                    resetting_leg.notional.currency(),
                    constant_leg.notional.currency(),
                ),
            );
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        self.validate_leg(&self.leg1)?;
        self.validate_leg(&self.leg2)?;

        if self.leg1.notional.currency() == self.leg2.notional.currency() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap legs must have different currencies; both are {}. \
                 Use BasisSwap for same-currency basis trades.",
                self.leg1.notional.currency()
            )));
        }

        // Pre-flight FX reachability: fail loud with currency-pair context
        // BEFORE descending into per-cashflow conversion. Without this, missing
        // FX surfaces as `NotFound { id: "fx_matrix" }` from deep inside the
        // schedule loop, hiding which leg/pair was the offender.
        self.validate_fx_reachable(market)?;

        if let NotionalExchange::MtmResetting { resetting_side } = self.notional_exchange {
            // Run the full `validate()` so MtM-specific notional-exchange
            // alignment and currency checks fire before per-period math.
            self.validate()?;
            return crate::instruments::rates::xccy_swap::pricing_mtm::pv_mtm_reset(
                self,
                resetting_side,
                market,
                as_of,
            );
        }

        // pv_leg_in_reporting_currency builds its own period schedule; no need to pre-build here.
        let pv1_rep = self.pv_leg_in_reporting_currency(&self.leg1, market, as_of)?;
        let pv2_rep = self.pv_leg_in_reporting_currency(&self.leg2, market, as_of)?;

        pv1_rep.checked_add(pv2_rep)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.leg1.leg.start)
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for XccySwap {
    fn raw_cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.validate_leg(&self.leg1)?;
        self.validate_leg(&self.leg2)?;

        let anchor = if as_of < self.leg1.leg.start {
            as_of
        } else {
            self.leg1.leg.start - time::Duration::days(1)
        };

        // MtM-reset path: constant leg behaves like a vanilla fixed-notional leg; the
        // resetting leg has per-period notional plus rebalancing flows. Dispatch is
        // routed through `pricing_mtm::mtm_resetting_leg_schedule` which mirrors the
        // PV cashflow stream but emits records instead of summing.
        if let NotionalExchange::MtmResetting { resetting_side } = self.notional_exchange {
            self.validate()?;
            let schedule =
                crate::instruments::rates::xccy_swap::pricing_mtm::mtm_cashflow_schedule(
                    self,
                    resetting_side,
                    market,
                    as_of,
                )?;
            return Ok(schedule
                .with_notional(Notional::par(0.0, self.reporting_currency)?)
                .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected));
        }

        let leg1_schedule = self.leg_coupon_schedule(&self.leg1, market, as_of)?;
        let leg2_schedule = self.leg_coupon_schedule(&self.leg2, market, as_of)?;
        let leg1_principal = self.leg_principal_schedule(&self.leg1, anchor)?;
        let leg2_principal = self.leg_principal_schedule(&self.leg2, anchor)?;

        Ok(merge_cashflow_schedules(
            [leg1_schedule, leg1_principal, leg2_schedule, leg2_principal],
            Notional::par(0.0, self.reporting_currency)?,
            self.leg1.leg.day_count,
        )
        .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use time::Month;

    fn no_fallback_swap() -> XccySwap {
        let mut swap = XccySwap::example();
        swap.allow_calendar_fallback = false;
        swap
    }

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid test date")
    }

    #[test]
    fn xccy_swap_cashflow_provider_emits_multi_currency_flows() {
        let as_of = date(2025, Month::January, 1);
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots(vec![(0.0, 1.0), (1.0, 0.95)])
                    .build()
                    .expect("usd curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots(vec![(0.0, 1.0), (1.0, 0.97)])
                    .build()
                    .expect("eur curve"),
            )
            .insert(
                ForwardCurve::builder("USD-SOFR-3M", 0.25)
                    .base_date(as_of)
                    .knots(vec![(0.0, 0.04), (1.0, 0.04)])
                    .build()
                    .expect("usd forward"),
            )
            .insert(
                ForwardCurve::builder("EUR-EURIBOR-3M", 0.25)
                    .base_date(as_of)
                    .knots(vec![(0.0, 0.03), (1.0, 0.03)])
                    .build()
                    .expect("eur forward"),
            );

        let start = date(2025, Month::January, 2);
        let end = date(2026, Month::January, 2);
        let mut swap = XccySwap::new(
            "XCCY-CF",
            XccySwapLeg {
                notional: Money::from((1_000_000_i64, Currency::USD)),
                side: PayReceive::Receive,
                leg: FloatLegSpec {
                    forward_curve_id: CurveId::new("USD-SOFR-3M"),
                    discount_curve_id: CurveId::new("USD-OIS"),
                    start,
                    end,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    business_day_convention: BusinessDayConvention::ModifiedFollowing,
                    stub: StubKind::ShortFront,
                    spread_bp: Decimal::ZERO,
                    payment_lag_days: 0,
                    calendar_id: None,
                    reset_lag_days: 0,
                    compounding: Default::default(),
                    fixing_calendar_id: None,
                    end_of_month: false,
                },
            },
            XccySwapLeg {
                notional: Money::from((900_000_i64, Currency::EUR)),
                side: PayReceive::Pay,
                leg: FloatLegSpec {
                    forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                    discount_curve_id: CurveId::new("EUR-OIS"),
                    start,
                    end,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    business_day_convention: BusinessDayConvention::ModifiedFollowing,
                    stub: StubKind::ShortFront,
                    spread_bp: Decimal::ZERO,
                    payment_lag_days: 0,
                    calendar_id: None,
                    reset_lag_days: 0,
                    compounding: Default::default(),
                    fixing_calendar_id: None,
                    end_of_month: false,
                },
            },
            Currency::USD,
        );
        swap.allow_calendar_fallback = true;

        let flows = swap
            .dated_cashflows(&market, as_of)
            .expect("xccy contractual schedule should build");

        assert!(
            flows.len() >= 6,
            "xccy swap should emit principal and coupon flows"
        );
        assert!(flows
            .iter()
            .any(|(_, money)| money.currency() == Currency::USD));
        assert!(flows
            .iter()
            .any(|(_, money)| money.currency() == Currency::EUR));
    }

    #[test]
    fn base_value_fails_loud_when_fx_matrix_is_missing() {
        // Reproduces the audit scenario: USD/EUR XCCY with EUR reporting,
        // market context has both curves but NO FxMatrix. Pre-flight
        // reachability must reject up front with a message naming the
        // instrument id and both leg currencies, NOT a generic NotFound
        // surfaced from inside the per-cashflow loop.
        let as_of = date(2025, Month::January, 1);
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots(vec![(0.0, 1.0), (1.0, 0.95)])
                    .build()
                    .expect("usd curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots(vec![(0.0, 1.0), (1.0, 0.97)])
                    .build()
                    .expect("eur curve"),
            )
            .insert(
                ForwardCurve::builder("USD-SOFR-3M", 0.25)
                    .base_date(as_of)
                    .knots(vec![(0.0, 0.04), (1.0, 0.04)])
                    .build()
                    .expect("usd forward"),
            )
            .insert(
                ForwardCurve::builder("EUR-EURIBOR-3M", 0.25)
                    .base_date(as_of)
                    .knots(vec![(0.0, 0.03), (1.0, 0.03)])
                    .build()
                    .expect("eur forward"),
            );

        let start = date(2025, Month::January, 2);
        let end = date(2026, Month::January, 2);
        let mut swap = XccySwap::new(
            "XCCY-NOFX",
            XccySwapLeg {
                notional: Money::from((1_000_000_i64, Currency::USD)),
                side: PayReceive::Receive,
                leg: FloatLegSpec {
                    forward_curve_id: CurveId::new("USD-SOFR-3M"),
                    discount_curve_id: CurveId::new("USD-OIS"),
                    start,
                    end,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    business_day_convention: BusinessDayConvention::ModifiedFollowing,
                    stub: StubKind::ShortFront,
                    spread_bp: Decimal::ZERO,
                    payment_lag_days: 0,
                    calendar_id: None,
                    reset_lag_days: 0,
                    compounding: Default::default(),
                    fixing_calendar_id: None,
                    end_of_month: false,
                },
            },
            XccySwapLeg {
                notional: Money::from((900_000_i64, Currency::EUR)),
                side: PayReceive::Pay,
                leg: FloatLegSpec {
                    forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                    discount_curve_id: CurveId::new("EUR-OIS"),
                    start,
                    end,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    business_day_convention: BusinessDayConvention::ModifiedFollowing,
                    stub: StubKind::ShortFront,
                    spread_bp: Decimal::ZERO,
                    payment_lag_days: 0,
                    calendar_id: None,
                    reset_lag_days: 0,
                    compounding: Default::default(),
                    fixing_calendar_id: None,
                    end_of_month: false,
                },
            },
            Currency::EUR, // reporting != either leg directly
        );
        swap.allow_calendar_fallback = true;

        let err = swap
            .base_value(&market, as_of)
            .expect_err("missing FxMatrix must be rejected pre-flight");
        let msg = format!("{err}");
        assert!(
            msg.contains("XCCY-NOFX"),
            "error must name the instrument id, got: {msg}"
        );
        assert!(
            msg.contains("fx_matrix") || msg.contains("FX path"),
            "error must explain that FX is required, got: {msg}"
        );
    }

    #[test]
    // schema-rejection-test: leg-level allow_calendar_fallback moved to XccySwap.
    fn xccy_leg_rejects_leg_level_allow_calendar_fallback() {
        let swap = XccySwap::example();
        let mut json = serde_json::to_value(&swap.leg1).expect("serialize leg");
        json["allow_calendar_fallback"] = serde_json::Value::Bool(true);
        let err = serde_json::from_value::<XccySwapLeg>(json)
            .expect_err("leg-level allow_calendar_fallback must be rejected");
        assert!(err.to_string().contains("allow_calendar_fallback"), "{err}");
    }

    #[test]
    fn resetting_side_fromstr_display_roundtrip() {
        use std::str::FromStr;
        for side in [ResettingSide::Leg1, ResettingSide::Leg2] {
            let s = side.to_string();
            let parsed = ResettingSide::from_str(&s).expect("roundtrip parse");
            assert_eq!(side, parsed, "roundtrip failed for {s}");
        }
        for noncanonical in ["leg_1", "Leg1", " leg1"] {
            assert!(ResettingSide::from_str(noncanonical).is_err());
        }
        assert!(ResettingSide::from_str("garbage").is_err());
    }

    #[test]
    fn notional_exchange_serde_mtm_resetting_roundtrip() {
        let original = NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg1,
        };
        let json = serde_json::to_string(&original).expect("serialise");
        assert_eq!(json, r#"{"mtm_resetting":{"resetting_side":"leg1"}}"#);
        let parsed: NotionalExchange = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(original, parsed);
    }

    #[test]
    fn partition_legs_returns_constant_then_resetting() {
        let swap = XccySwap::example().with_notional_exchange(NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg2, // EUR leg resets
        });
        let (constant, resetting) = swap
            .partition_legs(ResettingSide::Leg2)
            .expect("partition succeeds");
        assert_eq!(constant.notional.currency(), Currency::USD);
        assert_eq!(resetting.notional.currency(), Currency::EUR);

        // Symmetrically, when leg1 resets, leg2 (EUR) is constant.
        let (constant_l1, resetting_l1) = swap
            .partition_legs(ResettingSide::Leg1)
            .expect("partition succeeds");
        assert_eq!(constant_l1.notional.currency(), Currency::EUR);
        assert_eq!(resetting_l1.notional.currency(), Currency::USD);
    }

    #[test]
    fn partition_legs_errors_when_legs_share_currency() {
        // Force both legs to USD to exercise the same-currency guard inside
        // `partition_legs`. `validate()` should also reject this shape, but this test
        // pins the guard at the helper level since `partition_legs` is the primary
        // contract used by Task 7's PV path.
        let mut swap = XccySwap::example();
        swap.leg2.notional = finstack_quant_core::money::Money::from((1_i64, Currency::USD));

        let err = swap
            .partition_legs(ResettingSide::Leg2)
            .expect_err("same-currency legs must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("different currencies") && msg.contains("USD"),
            "expected currency-mismatch error mentioning USD; got: {msg}"
        );
    }

    #[test]
    fn validate_rejects_mtm_reset_with_misaligned_leg_schedules() {
        use finstack_quant_core::money::Money;

        let start = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let end = Date::from_calendar_date(2030, time::Month::January, 2).expect("valid date");
        let start_off =
            Date::from_calendar_date(2025, time::Month::February, 3).expect("valid date");

        let leg1 = XccySwapLeg {
            notional: Money::from((9_200_000_i64, Currency::EUR)),
            side: PayReceive::Receive,
            leg: FloatLegSpec {
                forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: 0,
                compounding: Default::default(),
                fixing_calendar_id: None,
                end_of_month: false,
            },
        };
        let mut leg2 = leg1.clone();
        leg2.notional = Money::from((10_000_000_i64, Currency::USD));
        leg2.side = PayReceive::Pay;
        leg2.leg.forward_curve_id = CurveId::new("USD-SOFR-3M");
        leg2.leg.discount_curve_id = CurveId::new("USD-OIS");
        leg2.leg.start = start_off; // misaligned start

        let mut swap = XccySwap::new("MTM-MISALIGNED", leg1, leg2, Currency::USD)
            .with_notional_exchange(NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg1,
            });
        swap.allow_calendar_fallback = true;

        let err = swap
            .validate()
            .expect_err("misaligned schedules must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("MtmResetting") && msg.contains("schedule"),
            "expected MtM-reset schedule-alignment error, got: {msg}"
        );
    }

    #[test]
    fn validate_accepts_mtm_reset_with_leg_specific_frequencies() {
        use finstack_quant_core::money::Money;

        let start = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
        let end = Date::from_calendar_date(2030, time::Month::January, 2).expect("valid date");

        let leg1 = XccySwapLeg {
            notional: Money::from((9_200_000_i64, Currency::EUR)),
            side: PayReceive::Receive,
            leg: FloatLegSpec {
                forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: 0,
                compounding: Default::default(),
                fixing_calendar_id: None,
                end_of_month: false,
            },
        };
        let mut leg2 = leg1.clone();
        leg2.notional = Money::from((10_000_000_i64, Currency::USD));
        leg2.side = PayReceive::Pay;
        leg2.leg.forward_curve_id = CurveId::new("USD-SOFR-3M");
        leg2.leg.discount_curve_id = CurveId::new("USD-OIS");
        leg2.leg.frequency = Tenor::semi_annual();

        let mut swap = XccySwap::new("MTM-FREQ", leg1, leg2, Currency::USD).with_notional_exchange(
            NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg1,
            },
        );
        swap.allow_calendar_fallback = true;

        swap.validate()
            .expect("each MtM leg owns its coupon frequency");
    }

    #[test]
    fn validate_accepts_well_formed_mtm_resetting_swap() {
        // The canonical example swap has aligned schedules, matching frequencies, and
        // different currencies. Wrapping it in MtmResetting must pass `validate()`.
        let swap = XccySwap::example().with_notional_exchange(NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg2,
        });
        swap.validate()
            .expect("well-formed MtmResetting swap should pass validate");
    }

    #[test]
    fn scenario_price_shock_applies_through_the_override_channel() {
        use finstack_quant_core::market_data::term_structures::ForwardCurve;
        use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
        use std::sync::Arc;

        let base = Date::from_calendar_date(2024, Month::January, 3).expect("base date");
        let curve = |id: &str, rate: f64| {
            DiscountCurve::builder(CurveId::new(id))
                .base_date(base)
                .knots(vec![(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
                .build()
                .expect("discount curve")
        };
        let forward = |id: &str, rate: f64| {
            ForwardCurve::builder(CurveId::new(id), 0.25)
                .base_date(base)
                .knots(vec![(0.0, rate), (5.0, rate)])
                .build()
                .expect("forward curve")
        };
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("set EUR/USD rate");
        let market = MarketContext::new()
            .insert(curve("USD-OIS", 0.02))
            .insert(curve("EUR-OIS", 0.01))
            .insert(forward("USD-SOFR-3M", 0.02))
            .insert(forward("EUR-EURIBOR-3M", 0.01))
            .insert_fx(FxMatrix::new(provider));

        let swap = XccySwap::example();
        let unshocked = swap.value(&market, base).expect("unshocked value");
        let mut shocked_swap = swap.clone();
        shocked_swap
            .get_scenario_pricing_overrides_mut()
            .expect("XccySwap exposes scenario overrides")
            .scenario_price_shock_decimal = Some(-0.05);
        let shocked = shocked_swap.value(&market, base).expect("shocked value");
        // Reference: ScenarioPricingOverrides scales the PV by (1 + shock). The
        // shock is applied to the raw PV before Money's decimal storage, so the
        // two sides can differ in the last ulp (1e-12 relative).
        let expected = unshocked.amount() * (1.0 + -0.05);
        assert!(
            (shocked.amount() - expected).abs() <= 1.0e-12 * expected.abs(),
            "shocked {} vs expected {expected}",
            shocked.amount()
        );
        assert!(unshocked.amount().abs() > 1.0, "non-trivial base PV");

        let json = serde_json::to_value(&shocked_swap).expect("serialize");
        assert_eq!(
            json["scenario_pricing_overrides"]["scenario_price_shock_decimal"],
            -0.05
        );
        let unshocked_json = serde_json::to_value(&swap).expect("serialize");
        assert!(unshocked_json.get("instrument_pricing_overrides").is_none());
        assert!(unshocked_json.get("metric_pricing_overrides").is_none());
        assert!(unshocked_json.get("scenario_pricing_overrides").is_none());
    }

    #[test]
    fn base_value_dispatches_mtm_resetting_to_pricing_mtm() {
        use finstack_quant_core::market_data::term_structures::ForwardCurve;
        use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
        use std::sync::Arc;
        use time::Month;

        let base = Date::from_calendar_date(2024, Month::January, 3).expect("base date");

        // Build minimal curves matching the IDs used by XccySwap::example().
        let usd_disc = DiscountCurve::builder(CurveId::new("USD-OIS"))
            .base_date(base)
            .knots(vec![(0.0, 1.0), (5.0, (-0.02_f64 * 5.0).exp())])
            .build()
            .expect("usd disc");
        let eur_disc = DiscountCurve::builder(CurveId::new("EUR-OIS"))
            .base_date(base)
            .knots(vec![(0.0, 1.0), (5.0, (-0.01_f64 * 5.0).exp())])
            .build()
            .expect("eur disc");
        let usd_fwd = ForwardCurve::builder(CurveId::new("USD-SOFR-3M"), 0.25)
            .base_date(base)
            .knots(vec![(0.0, 0.02), (5.0, 0.02)])
            .build()
            .expect("usd fwd");
        let eur_fwd = ForwardCurve::builder(CurveId::new("EUR-EURIBOR-3M"), 0.25)
            .base_date(base)
            .knots(vec![(0.0, 0.01), (5.0, 0.01)])
            .build()
            .expect("eur fwd");

        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("set EUR/USD rate");
        let fx = FxMatrix::new(provider);

        let ctx = MarketContext::new()
            .insert(usd_disc)
            .insert(eur_disc)
            .insert(usd_fwd)
            .insert(eur_fwd)
            .insert_fx(fx);

        let swap = XccySwap::example().with_notional_exchange(NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg2,
        });

        // The PV should be a finite number; we are not asserting the exact value here.
        // Task 9 will do CIP-invariance.
        let pv = swap
            .base_value(&ctx, base)
            .expect("MtM-reset PV should compute");
        assert!(
            pv.amount().is_finite(),
            "MtM-reset PV must be finite, got {}",
            pv.amount()
        );
        assert_eq!(pv.currency(), Currency::USD);
    }

    #[test]
    fn overnight_estr_leg_matches_shared_compounding_engine() {
        use crate::instruments::common_impl::pricing::overnight::{
            project_overnight_coupon, OvernightCouponProjectionInput, OvernightProjectionCurve,
        };
        use crate::instruments::rates::irs::FloatingLegCompounding;
        use finstack_quant_core::dates::calendar_by_id;
        use finstack_quant_core::market_data::term_structures::ForwardCurve;

        let start = Date::from_calendar_date(2025, time::Month::January, 2).expect("date");
        let end = Date::from_calendar_date(2025, time::Month::April, 2).expect("date");
        let fwd = ForwardCurve::builder("EUR-ESTR-OIS", 1.0 / 365.0)
            .base_date(start)
            .knots(vec![(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("estr forward");
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
        let period = crate::cashflow::builder::periods::SchedulePeriod {
            accrual_start: start,
            accrual_end: end,
            payment_date: end,
            reset_date: Some(start),
            accrual_year_fraction: 0.25,
            unadjusted_start: start,
            unadjusted_end: end,
        };
        let leg = XccySwapLeg {
            notional: Money::from((1_000_000_i64, Currency::EUR)),
            side: PayReceive::Pay,
            leg: FloatLegSpec {
                forward_curve_id: CurveId::new("EUR-ESTR-OIS"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: Some("target2".into()),
                reset_lag_days: 0,
                compounding,
                fixing_calendar_id: None,
                end_of_month: false,
            },
        };
        let mut mismatched = period;
        mismatched.accrual_year_fraction = 0.50;
        let projected = no_fallback_swap()
            .projected_leg_period(&leg, &fwd, None, &mismatched, start, None)
            .expect("overnight xccy period");
        let calendar = calendar_by_id("target2").expect("target2");
        let expected = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&fwd),
            fixings: None,
            fixing_id: "EUR-ESTR-OIS",
            as_of: start,
            accrual_start: start,
            accrual_end: end,
            day_count: DayCount::Act360,
            coupon_frequency: Some(Tenor::quarterly()),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: false,
        })
        .expect("shared engine");
        assert!(
            (projected.rate - expected.rate).abs() < 1e-12,
            "xccy overnight rate {} != shared engine {}",
            projected.rate,
            expected.rate
        );
        let amount = projected.unsigned_coupon(leg.notional.amount(), 0.0);
        let expected_amount = 1_000_000.0 * (expected.compound_factor - 1.0);
        assert!(
            (amount - expected_amount).abs() < 1e-6,
            "xccy overnight amount {amount} != N*(CF-1) {expected_amount}"
        );
        let wrong_schedule_amount = projected.rate * 1_000_000.0 * mismatched.accrual_year_fraction;
        assert!(
            (amount - wrong_schedule_amount).abs() > 1.0,
            "overnight coupon must not use the unadjusted schedule year fraction"
        );
    }

    #[test]
    fn overnight_unresolvable_calendar_without_fallback_errors() {
        use crate::instruments::rates::irs::FloatingLegCompounding;
        use finstack_quant_core::market_data::term_structures::ForwardCurve;

        let start = Date::from_calendar_date(2025, time::Month::January, 2).expect("date");
        let end = Date::from_calendar_date(2025, time::Month::April, 2).expect("date");
        let fwd = ForwardCurve::builder("EUR-ESTR-OIS", 1.0 / 365.0)
            .base_date(start)
            .knots(vec![(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("estr forward");
        let period = crate::cashflow::builder::periods::SchedulePeriod {
            accrual_start: start,
            accrual_end: end,
            payment_date: end,
            reset_date: Some(start),
            accrual_year_fraction: 0.25,
            unadjusted_start: start,
            unadjusted_end: end,
        };
        let mut leg = XccySwapLeg {
            notional: Money::from((1_000_000_i64, Currency::EUR)),
            side: PayReceive::Pay,
            leg: FloatLegSpec {
                forward_curve_id: CurveId::new("EUR-ESTR-OIS"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: Some("not-a-calendar".into()),
                reset_lag_days: 0,
                compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
                fixing_calendar_id: None,
                end_of_month: false,
            },
        };
        let err = no_fallback_swap()
            .projected_leg_period(&leg, &fwd, None, &period, start, None)
            .expect_err("unresolvable calendar must fail");
        assert!(
            err.to_string().contains("calendar"),
            "expected calendar error, got {err}"
        );

        let disc =
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("EUR-OIS")
                .base_date(start)
                .knots(vec![(0.0, 1.0), (1.0, 0.97)])
                .build()
                .expect("discount");
        let market = MarketContext::new().insert(disc).insert(fwd);
        let swap = XccySwap::new(
            "EUR-OIS-BAD-CAL",
            leg.clone(),
            {
                leg.notional = Money::from((1_000_000_i64, Currency::USD));
                leg.leg.forward_curve_id = CurveId::new("USD-SOFR-3M");
                leg.leg.discount_curve_id = CurveId::new("USD-OIS");
                leg.leg.compounding = FloatingLegCompounding::Simple;
                leg.leg.calendar_id = Some("usny".into());
                leg
            },
            Currency::USD,
        );
        let err = swap
            .base_value(&market, start)
            .expect_err("pricing must reject the unresolvable overnight calendar");
        assert!(
            err.to_string().contains("calendar"),
            "expected calendar error from base_value, got {err}"
        );
    }

    #[test]
    fn validate_rejects_simple_compounding_on_overnight_index() {
        let mut swap = XccySwap::example();
        swap.leg1.leg.forward_curve_id = CurveId::new("USD-SOFR-OIS");
        swap.leg1.leg.compounding = crate::instruments::rates::irs::FloatingLegCompounding::Simple;
        let err = swap
            .validate()
            .expect_err("Simple on USD-SOFR-OIS must fail");
        assert!(
            format!("{err}").contains("Overnight RFR"),
            "expected overnight/Simple rejection, got {err}"
        );
    }
}
