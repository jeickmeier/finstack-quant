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
use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
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

/// Whether the holder pays or receives a leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LegSide {
    /// Receive the leg's coupons (and final notional, if exchanged).
    Receive,
    /// Pay the leg's coupons (and final notional, if exchanged).
    Pay,
}

impl std::fmt::Display for LegSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pay => write!(f, "pay"),
            Self::Receive => write!(f, "receive"),
        }
    }
}

impl std::str::FromStr for LegSide {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pay" => Ok(Self::Pay),
            "receive" => Ok(Self::Receive),
            _ => Err(format!("Unknown leg side: '{}'. Valid: pay, receive", s)),
        }
    }
}

impl LegSide {
    /// Returns the sign multiplier for coupon cashflows.
    ///
    /// `Receive` leg coupons flow in (`+1.0`); `Pay` leg coupons flow out (`-1.0`).
    #[inline]
    pub(crate) fn coupon_sign(self) -> f64 {
        match self {
            Self::Receive => 1.0,
            Self::Pay => -1.0,
        }
    }

    /// Returns the sign for initial principal exchange.
    ///
    /// # Market Convention
    ///
    /// The leg you "receive" is economically a lending position:
    /// - **At start**: you pay out principal (negative cashflow) to the counterparty
    /// - **During**: you receive interest coupons (positive cashflows)
    /// - **At end**: you receive principal back (positive cashflow)
    ///
    /// # Example
    ///
    /// For a USD/EUR XCCY swap where you receive USD:
    /// - Initial exchange: you pay USD notional to counterparty (-1.0 sign)
    /// - Final exchange: you receive USD notional back (+1.0 sign)
    ///
    /// This follows ISDA conventions where the receiver of a leg provides
    /// the initial funding in that currency.
    #[inline]
    pub(crate) fn initial_principal_sign(self) -> f64 {
        match self {
            Self::Receive => -1.0,
            Self::Pay => 1.0,
        }
    }

    /// Returns the sign for final principal exchange (opposite of initial).
    #[inline]
    pub(crate) fn final_principal_sign(self) -> f64 {
        -self.initial_principal_sign()
    }
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
/// Each leg owns its own dates, discount curve, calendar, and stub conventions,
/// following the IRS leg-centric pattern.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XccySwapLeg {
    /// Leg currency.
    pub currency: Currency,
    /// Leg notional (in leg currency).
    pub notional: Money,
    /// Pay/receive direction for this leg.
    pub side: LegSide,
    /// Projection forward curve.
    pub forward_curve_id: CurveId,
    /// Discount curve for PV in leg currency.
    pub discount_curve_id: CurveId,
    /// Start date of the leg.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date of the leg.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Coupon frequency.
    pub frequency: Tenor,
    /// Accrual day count.
    pub day_count: DayCount,
    /// Business day convention for schedule dates.
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Stub period handling rule.
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Spread in basis points (e.g. `Decimal::from(5)` = 5bp).
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub spread_bp: Decimal,
    /// Payment lag in business days after period end (default: 0).
    #[serde(default)]
    pub payment_lag_days: i32,
    /// Calendar identifier for schedule generation and lags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    /// Reset lag in business days before the accrual start (e.g. 2 for T-2 fixing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_lag_days: Option<i32>,
    /// Allow calendar-day fallback when the calendar cannot be resolved.
    ///
    /// When `false` (default), missing calendars are treated as input errors.
    #[serde(default)]
    pub allow_calendar_fallback: bool,
    /// Overnight vs term compounding for this floating leg.
    ///
    /// Defaults to Simple (term EURIBOR / term SOFR). Set a compounded
    /// variant when the forward curve is an overnight RFR such as €STR or
    /// SOFR OIS.
    #[serde(default)]
    pub compounding: crate::instruments::rates::irs::FloatingLegCompounding,
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
            currency: Currency::USD,
            notional: Money::from((10_000_000_i64, Currency::USD)),
            side: LegSide::Receive,
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
            reset_lag_days: None,
            allow_calendar_fallback: true,
            compounding: Default::default(),
        };

        let eur_leg = XccySwapLeg {
            currency: Currency::EUR,
            notional: Money::from((9_200_000_i64, Currency::EUR)),
            side: LegSide::Pay,
            forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
            discount_curve_id: CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(10),
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: None,
            allow_calendar_fallback: true,
            compounding: Default::default(),
        };

        Self::new("XCCY-USDEUR-5Y", usd_leg, eur_leg, Currency::USD)
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
        if constant.currency == resetting.currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}': MtM-reset partition requires different currencies on the two legs; both are {}",
                self.id, constant.currency
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
            self.leg1.forward_curve_id.as_str(),
            &self.leg1.compounding,
        )?;
        crate::instruments::common_impl::pricing::overnight_conventions::reject_simple_overnight(
            self.leg2.forward_curve_id.as_str(),
            &self.leg2.compounding,
        )?;

        if let NotionalExchange::MtmResetting { resetting_side } = &self.notional_exchange {
            // Confirm resetting_side resolves and yields different currencies.
            self.partition_legs(*resetting_side)?;

            if self.leg1.start != self.leg2.start || self.leg1.end != self.leg2.end {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "XccySwap '{}': MtmResetting requires both legs to share start and end \
                     dates (schedule alignment), got leg1=[{}, {}] leg2=[{}, {}]",
                    self.id, self.leg1.start, self.leg1.end, self.leg2.start, self.leg2.end
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
        let needs_fx = self.leg1.currency != self.reporting_currency
            || self.leg2.currency != self.reporting_currency;
        if !needs_fx {
            return Ok(());
        }
        let fx = market.fx().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}' requires fx_matrix in market context: leg1={} leg2={} reporting={}",
                self.id.as_str(),
                self.leg1.currency,
                self.leg2.currency,
                self.reporting_currency,
            ))
        })?;

        for (label, leg) in [("leg1", &self.leg1), ("leg2", &self.leg2)] {
            if leg.currency == self.reporting_currency {
                continue;
            }
            // Probe FX with a representative payment date (leg.start). Reachability
            // failure here will surface as a precise currency-pair error rather than
            // a generic NotFound from the inner cashflow loop.
            fx.rate(FxQuery::new(
                leg.currency,
                self.reporting_currency,
                leg.start,
            ))
            .map_err(|err| {
                finstack_quant_core::Error::Validation(format!(
                    "XccySwap '{}' FX path unreachable for {}: {}->{} ({})",
                    self.id.as_str(),
                    label,
                    leg.currency,
                    self.reporting_currency,
                    err,
                ))
            })?;
        }
        Ok(())
    }

    fn validate_leg(&self, leg: &XccySwapLeg) -> Result<()> {
        if leg.notional.currency() != leg.currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: leg.currency,
                actual: leg.notional.currency(),
            });
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
        if leg.payment_lag_days < 0 {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap payment lag must be non-negative".to_string(),
            ));
        }
        if leg.reset_lag_days.is_some_and(|lag| lag < 0) {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap reset lag must be non-negative".to_string(),
            ));
        }
        let calendar_resolves = leg.calendar_id.as_deref().is_some_and(|id| {
            crate::cashflow::builder::calendar::resolve_calendar_strict(id).is_ok()
        });
        if !calendar_resolves && !leg.allow_calendar_fallback {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}' leg {} requires a resolvable calendar_id; set allow_calendar_fallback=true to opt into weekends_only",
                self.id, leg.currency
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
    /// * `leg` - XCCY leg whose `calendar_id` and fallback flag are consulted.
    pub(crate) fn resolve_leg_calendar_id(leg: &XccySwapLeg) -> Result<&str> {
        match leg.calendar_id.as_deref() {
            Some(id)
                if crate::cashflow::builder::calendar::resolve_calendar_strict(id).is_ok() =>
            {
                Ok(id)
            }
            _ if leg.allow_calendar_fallback => {
                Ok(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID)
            }
            _ => Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap leg {} requires a resolvable calendar_id; set allow_calendar_fallback=true to opt into weekends_only",
                leg.currency
            ))),
        }
    }

    /// Holiday calendar matching [`Self::resolve_leg_calendar_id`].
    ///
    /// # Arguments
    ///
    /// * `leg` - XCCY leg whose resolved calendar ID is turned into a calendar.
    fn resolve_leg_calendar(
        leg: &XccySwapLeg,
    ) -> Result<&'static dyn finstack_quant_core::dates::HolidayCalendar> {
        crate::cashflow::builder::calendar::resolve_calendar_strict(Self::resolve_leg_calendar_id(
            leg,
        )?)
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
                start: leg.start,
                end: leg.end,
                frequency: leg.frequency,
                stub: leg.stub,
                business_day_convention: leg.business_day_convention,
                calendar_id,
                end_of_month: false,
                day_count: leg.day_count,
                payment_lag_days: leg.payment_lag_days,
                reset_lag_days: leg.reset_lag_days,
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
    pub(crate) fn projected_leg_period(
        leg: &XccySwapLeg,
        fwd: &finstack_quant_core::market_data::term_structures::ForwardCurve,
        fixings: Option<&finstack_quant_core::market_data::scalars::ScalarTimeSeries>,
        period: &crate::cashflow::builder::periods::SchedulePeriod,
        as_of: Date,
    ) -> Result<ProjectedXccyPeriod> {
        use crate::instruments::common_impl::pricing::overnight::{
            adjust_overnight_accrual_boundaries, project_overnight_coupon,
            OvernightCouponProjectionInput, OvernightProjectionCurve,
        };
        use crate::instruments::common_impl::pricing::time::rate_between_on_dates;
        use crate::instruments::rates::irs::FloatingLegCompounding;

        let projected = if !matches!(leg.compounding, FloatingLegCompounding::Simple) {
            let calendar = Self::resolve_leg_calendar(leg)?;
            let (accrual_start, accrual_end) = adjust_overnight_accrual_boundaries(
                period.accrual_start,
                period.accrual_end,
                leg.business_day_convention,
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
                    fixing_id: leg.forward_curve_id.as_str(),
                    as_of,
                    accrual_start,
                    accrual_end,
                    day_count: leg.day_count,
                    coupon_frequency: Some(leg.frequency),
                    compounding: &leg.compounding,
                    fixing_calendar: calendar,
                    compounded_spread: 0.0,
                    need_observation_exposures: false,
                })?;
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
                    leg.forward_curve_id.as_str(),
                    fixing_date,
                    as_of,
                )?
            } else {
                rate_between_on_dates(fwd, period.accrual_start, period.accrual_end)?
            };
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
        let calendar_id = Self::resolve_leg_calendar_id(leg)?;
        let periods = self.leg_build_periods(leg, calendar_id)?;
        let fwd = market.get_forward(&leg.forward_curve_id)?;
        let fixing_series_id = finstack_quant_core::market_data::fixings::fixing_series_id(
            leg.forward_curve_id.as_str(),
        );
        let fixings = market.get_series(&fixing_series_id).ok();
        let spread = decimal_to_f64(leg.spread_bp, "XccySwap leg spread_bp")? / 10_000.0;

        let mut flows = Vec::with_capacity(periods.len());
        for period in &periods {
            let projected = Self::projected_leg_period(leg, fwd.as_ref(), fixings, period, as_of)?;
            let all_in = projected.all_in_rate(spread);
            let amount =
                leg.side.coupon_sign() * projected.unsigned_coupon(leg.notional.amount(), spread);
            flows.push(crate::cashflow::primitives::CashFlow::new(
                period.payment_date,
                projected.fixing_date.or(period.reset_date),
                Money::new(amount, leg.currency)?,
                crate::cashflow::primitives::CFKind::FloatReset,
                projected.year_fraction,
                Some(all_in),
            ));
        }
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            leg.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(leg.notional),
                ..Default::default()
            },
        ))
    }

    fn leg_principal_schedule(&self, leg: &XccySwapLeg, anchor: Date) -> Result<CashFlowSchedule> {
        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(Money::from((0_i64, leg.currency)), anchor, leg.end);
        // MtmResetting also requires initial AND final exchange; this arm makes the helper non-panicky if accidentally called on an MtM swap. `base_value` dispatches MtmResetting to `pricing_mtm::pv_mtm_reset` before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) {
            let initial_amount = leg.side.initial_principal_sign() * leg.notional.amount();
            let _ = builder.add_principal_event(
                leg.start,
                Money::from((0_i64, leg.currency)),
                Some(Money::new(-initial_amount, leg.currency)?),
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
            let final_amount = leg.side.final_principal_sign() * leg.notional.amount();
            let _ = builder.add_principal_event(
                leg.end,
                Money::from((0_i64, leg.currency)),
                Some(Money::new(-final_amount, leg.currency)?),
                CFKind::Notional,
            );
        }
        Ok(builder
            .build(None)?
            .with_notional(Notional::par(leg.notional.amount(), leg.currency)?))
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

        let calendar_id = Self::resolve_leg_calendar_id(leg)?;
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
        ) && leg.start > as_of;
        let unsettled_final = matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) && leg.end > as_of;
        if !unsettled_coupon && !unsettled_initial && !unsettled_final {
            return Ok(Money::from((0_i64, self.reporting_currency)));
        }

        let disc = context.get_discount(&leg.discount_curve_id)?;
        let fwd = context.get_forward(&leg.forward_curve_id)?;
        let fixing_series_id = finstack_quant_core::market_data::fixings::fixing_series_id(
            leg.forward_curve_id.as_str(),
        );
        let fixings = context.get_series(&fixing_series_id).ok();
        let fx = context.fx();

        let mut pv = NeumaierAccumulator::new();

        // Convert an already-discounted leg-currency PV at valuation-date spot.
        let convert_pv = |amount: f64| -> Result<f64> {
            if leg.currency == self.reporting_currency {
                return Ok(amount);
            }
            let fx_matrix = fx.ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "fx_matrix".to_string(),
                })
            })?;

            let rate = fx_matrix
                .rate(FxQuery::new(leg.currency, self.reporting_currency, as_of))?
                .rate;
            Ok(amount * rate)
        };

        // Notional exchanges (principal)
        // Use relative date-based discounting (validated against Bloomberg SWPM).
        // MtmResetting also requires initial AND final exchange; this arm makes the helper non-panicky if accidentally called on an MtM swap. `base_value` dispatches MtmResetting to `pricing_mtm::pv_mtm_reset` before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) && leg.start > as_of
        {
            let df = relative_df_discount_curve(disc.as_ref(), as_of, leg.start)?;
            let cf_leg_currency = leg.side.initial_principal_sign() * leg.notional.amount() * df;
            let cf_rep = convert_pv(cf_leg_currency)?;
            pv.add(cf_rep);
        }

        // MtmResetting also requires final exchange; same defensive note as above — the MtM live-pricing path dispatches before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) && leg.end > as_of
        {
            let df = relative_df_discount_curve(disc.as_ref(), as_of, leg.end)?;
            let cf_leg_currency = leg.side.final_principal_sign() * leg.notional.amount() * df;
            let cf_rep = convert_pv(cf_leg_currency)?;
            pv.add(cf_rep);
        }

        // Floating coupons
        for period in periods {
            if period.payment_date <= as_of {
                continue;
            }

            let projected = Self::projected_leg_period(leg, fwd.as_ref(), fixings, &period, as_of)?;
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

            let spread = decimal_to_f64(leg.spread_bp, "XccySwap leg spread_bp")? / 10_000.0;
            let coupon =
                leg.side.coupon_sign() * projected.unsigned_coupon(leg.notional.amount(), spread);

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

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        XccySwap::validate(self)
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.leg1.discount_curve_id.clone());
        deps.add_discount_curve(self.leg2.discount_curve_id.clone());
        deps.add_forward_curve(self.leg1.forward_curve_id.clone());
        deps.add_forward_curve(self.leg2.forward_curve_id.clone());
        if self.leg1.currency != self.reporting_currency {
            deps.add_fx_pair(self.leg1.currency, self.reporting_currency);
        }
        if self.leg2.currency != self.reporting_currency {
            deps.add_fx_pair(self.leg2.currency, self.reporting_currency);
        }
        deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
            self.leg1.forward_curve_id.as_str(),
        ));
        deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
            self.leg2.forward_curve_id.as_str(),
        ));
        if let NotionalExchange::MtmResetting { resetting_side } = self.notional_exchange {
            let (constant_leg, resetting_leg) = self.partition_legs(resetting_side)?;
            deps.add_series_id(
                crate::instruments::rates::xccy_swap::pricing_mtm::mtm_fx_fixing_series_id(
                    resetting_leg.currency,
                    constant_leg.currency,
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

        if self.leg1.currency == self.leg2.currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap legs must have different currencies; both are {}. \
                 Use BasisSwap for same-currency basis trades.",
                self.leg1.currency
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
        Some(self.leg1.start)
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for XccySwap {
    fn raw_cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.validate_leg(&self.leg1)?;
        self.validate_leg(&self.leg2)?;

        let anchor = if as_of < self.leg1.start {
            as_of
        } else {
            self.leg1.start - time::Duration::days(1)
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
            self.leg1.day_count,
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
        let swap = XccySwap::new(
            "XCCY-CF",
            XccySwapLeg {
                currency: Currency::USD,
                notional: Money::from((1_000_000_i64, Currency::USD)),
                side: LegSide::Receive,
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
                reset_lag_days: None,
                allow_calendar_fallback: true,
                compounding: Default::default(),
            },
            XccySwapLeg {
                currency: Currency::EUR,
                notional: Money::from((900_000_i64, Currency::EUR)),
                side: LegSide::Pay,
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
                reset_lag_days: None,
                allow_calendar_fallback: true,
                compounding: Default::default(),
            },
            Currency::USD,
        );

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
        let swap = XccySwap::new(
            "XCCY-NOFX",
            XccySwapLeg {
                currency: Currency::USD,
                notional: Money::from((1_000_000_i64, Currency::USD)),
                side: LegSide::Receive,
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
                reset_lag_days: None,
                allow_calendar_fallback: true,
                compounding: Default::default(),
            },
            XccySwapLeg {
                currency: Currency::EUR,
                notional: Money::from((900_000_i64, Currency::EUR)),
                side: LegSide::Pay,
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
                reset_lag_days: None,
                allow_calendar_fallback: true,
                compounding: Default::default(),
            },
            Currency::EUR, // reporting != either leg directly
        );

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
    fn leg_side_fromstr_display_roundtrip() {
        use std::str::FromStr;

        let variants = [LegSide::Pay, LegSide::Receive];
        for v in variants {
            let s = v.to_string();
            let parsed = LegSide::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        for noncanonical in ["rec", "payer", "Receive", " receive"] {
            assert!(LegSide::from_str(noncanonical).is_err());
        }
        assert!(LegSide::from_str("invalid").is_err());
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
        assert_eq!(constant.currency, Currency::USD);
        assert_eq!(resetting.currency, Currency::EUR);

        // Symmetrically, when leg1 resets, leg2 (EUR) is constant.
        let (constant_l1, resetting_l1) = swap
            .partition_legs(ResettingSide::Leg1)
            .expect("partition succeeds");
        assert_eq!(constant_l1.currency, Currency::EUR);
        assert_eq!(resetting_l1.currency, Currency::USD);
    }

    #[test]
    fn partition_legs_errors_when_legs_share_currency() {
        // Force both legs to USD to exercise the same-currency guard inside
        // `partition_legs`. `validate()` should also reject this shape, but this test
        // pins the guard at the helper level since `partition_legs` is the primary
        // contract used by Task 7's PV path.
        let mut swap = XccySwap::example();
        swap.leg2.currency = Currency::USD;
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
            currency: Currency::EUR,
            notional: Money::from((9_200_000_i64, Currency::EUR)),
            side: LegSide::Receive,
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
            reset_lag_days: None,
            allow_calendar_fallback: true,
            compounding: Default::default(),
        };
        let mut leg2 = leg1.clone();
        leg2.currency = Currency::USD;
        leg2.notional = Money::from((10_000_000_i64, Currency::USD));
        leg2.side = LegSide::Pay;
        leg2.forward_curve_id = CurveId::new("USD-SOFR-3M");
        leg2.discount_curve_id = CurveId::new("USD-OIS");
        leg2.start = start_off; // misaligned start

        let swap = XccySwap::new("MTM-MISALIGNED", leg1, leg2, Currency::USD)
            .with_notional_exchange(NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg1,
            });

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
            currency: Currency::EUR,
            notional: Money::from((9_200_000_i64, Currency::EUR)),
            side: LegSide::Receive,
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
            reset_lag_days: None,
            allow_calendar_fallback: true,
            compounding: Default::default(),
        };
        let mut leg2 = leg1.clone();
        leg2.currency = Currency::USD;
        leg2.notional = Money::from((10_000_000_i64, Currency::USD));
        leg2.side = LegSide::Pay;
        leg2.forward_curve_id = CurveId::new("USD-SOFR-3M");
        leg2.discount_curve_id = CurveId::new("USD-OIS");
        leg2.frequency = Tenor::semi_annual();

        let swap = XccySwap::new("MTM-FREQ", leg1, leg2, Currency::USD).with_notional_exchange(
            NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg1,
            },
        );

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
            currency: Currency::EUR,
            notional: Money::from((1_000_000_i64, Currency::EUR)),
            side: LegSide::Pay,
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
            calendar_id: Some("target2".to_string()),
            reset_lag_days: None,
            allow_calendar_fallback: false,
            compounding: compounding.clone(),
        };
        let mut mismatched = period;
        mismatched.accrual_year_fraction = 0.50;
        let projected = XccySwap::projected_leg_period(&leg, &fwd, None, &mismatched, start)
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
            currency: Currency::EUR,
            notional: Money::from((1_000_000_i64, Currency::EUR)),
            side: LegSide::Pay,
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
            calendar_id: Some("not-a-calendar".to_string()),
            reset_lag_days: None,
            allow_calendar_fallback: false,
            compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        };
        let err = XccySwap::projected_leg_period(&leg, &fwd, None, &period, start)
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
                leg.currency = Currency::USD;
                leg.notional = Money::from((1_000_000_i64, Currency::USD));
                leg.forward_curve_id = CurveId::new("USD-SOFR-3M");
                leg.discount_curve_id = CurveId::new("USD-OIS");
                leg.compounding = FloatingLegCompounding::Simple;
                leg.calendar_id = Some("usny".to_string());
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
        swap.leg1.forward_curve_id = CurveId::new("USD-SOFR-OIS");
        swap.leg1.compounding = crate::instruments::rates::irs::FloatingLegCompounding::Simple;
        let err = swap
            .validate()
            .expect_err("Simple on USD-SOFR-OIS must fail");
        assert!(
            format!("{err}").contains("Overnight RFR"),
            "expected overnight/Simple rejection, got {err}"
        );
    }
}
