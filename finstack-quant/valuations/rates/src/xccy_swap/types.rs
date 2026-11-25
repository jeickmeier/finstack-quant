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

use crate::FloatLegSpec;
use finstack_quant_cashflows::builder::{CashFlowSchedule, Notional, merge_cashflow_schedules};
use finstack_quant_core::Result;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations_core::impl_instrument_base;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::instruments::{NotionalExchange, ResettingSide};
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
#[serde(deny_unknown_fields, remote = "Self")]
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
    /// Attributes for instrument selection and tagging.
    pub attributes: finstack_quant_core::types::Attributes,
}

finstack_quant_valuations_core::validate_on_deserialize!(XccySwap);

/// Options controlling one XCCY period projection.
pub(crate) struct XccyPeriodProjectionArgs<'a> {
    /// Whether `period.accrual_end` ends on the leg's termination date.
    pub(crate) end_is_termination_date: bool,
    /// Optional collector for projected rate observations.
    pub(crate) projected_fixings:
        Option<&'a mut Vec<finstack_quant_cashflows::fixings::ProjectedFixing>>,
}

impl XccySwap {
    /// Convenience constructor.
    ///
    /// Dates and stub conventions are now owned by each leg.
    pub fn new(
        id: impl Into<InstrumentId>,
        leg1: XccySwapLeg,
        leg2: XccySwapLeg,
        reporting_currency: Currency,
    ) -> Self {
        Self {
            id: id.into(),
            leg1,
            leg2,
            notional_exchange: NotionalExchange::InitialAndFinal,
            reporting_currency,
            allow_calendar_fallback: false,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: finstack_quant_core::types::Attributes::default(),
        }
    }

    /// Create a canonical example USD/EUR 5Y cross-currency basis swap ($10M notional).
    ///
    /// Returns a 5-year XCCY swap with quarterly SOFR on the USD leg
    /// and quarterly EURIBOR on the EUR leg, with initial and final notional exchange.
    pub fn example() -> finstack_quant_core::Result<Self> {
        let start = time::macros::date!(2024 - 01 - 03);
        let end = time::macros::date!(2029 - 01 - 03);

        let usd_leg = XccySwapLeg {
            notional: Money::from((10_000_000_i64, Currency::USD)),
            side: PayReceive::Receive,
            leg: FloatLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                index_id: "USD-SOFR-3M".into(),
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
                index_id: "EUR-EURIBOR-3M".into(),
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
        Ok(swap)
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
                self.id,
                constant.notional.currency()
            )));
        }
        Ok((constant, resetting))
    }

    /// Validate the swap's static configuration.
    ///
    /// Checks each leg independently (notional currency consistency, finite/positive
    /// notional, non-negative payment lag), requires the two legs to have
    /// different currencies (same-currency basis trades are a `BasisSwap`), and
    /// then applies additional guards when [`NotionalExchange::MtmResetting`] is
    /// configured:
    ///
    /// - The resetting side must resolve (`partition_legs` guard).
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

        if self.leg1.notional.currency() == self.leg2.notional.currency() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap legs must have different currencies; both are {}. \
                 Use BasisSwap for same-currency basis trades.",
                self.leg1.notional.currency()
            )));
        }

        // MtM notional exchanges require aligned contractual start/end dates;
        // coupon schedules remain leg-specific.
        finstack_quant_valuations_core::instruments::pricing::overnight_conventions::reject_simple_overnight(
            self.leg1.leg.index_id.as_str(),
            &self.leg1.leg.compounding,
        )?;
        finstack_quant_valuations_core::instruments::pricing::overnight_conventions::reject_simple_overnight(
            self.leg2.leg.index_id.as_str(),
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
            || self.leg2.notional.currency() != self.reporting_currency
            || matches!(
                self.notional_exchange,
                NotionalExchange::MtmResetting { .. }
            );
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
                finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
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
        if let NotionalExchange::MtmResetting { resetting_side } = self.notional_exchange {
            let (constant_leg, resetting_leg) = self.partition_legs(resetting_side)?;
            fx.rate(FxQuery::new(
                resetting_leg.notional.currency(),
                constant_leg.notional.currency(),
                constant_leg.leg.start,
                finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
            ))
            .map_err(|err| {
                finstack_quant_core::Error::Validation(format!(
                    "XccySwap '{}' FX path unreachable for MtM reset: {}->{} ({})",
                    self.id.as_str(),
                    resetting_leg.notional.currency(),
                    constant_leg.notional.currency(),
                    err,
                ))
            })?;
        }
        Ok(())
    }

    fn validate_leg(&self, leg: &XccySwapLeg) -> Result<()> {
        finstack_quant_core::validation::validate_date_range_strict(
            leg.leg.start,
            leg.leg.end,
            &format!("XccySwap '{}' leg {}", self.id, leg.notional.currency()),
        )?;
        finstack_quant_core::market_data::fixings::validate_fixing_index_id(
            leg.leg.index_id.as_str(),
        )?;
        if leg.leg.compounding
            == finstack_quant_cashflows::builder::FloatingLegCompounding::SimpleAverage
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
        let calendar_resolves = leg
            .leg
            .calendar_id
            .as_deref()
            .is_some_and(|id| finstack_quant_core::dates::calendar_by_id_strict(id).is_ok());
        if !calendar_resolves && !self.allow_calendar_fallback {
            return Err(finstack_quant_core::Error::Validation(format!(
                "XccySwap '{}' leg {} requires a resolvable calendar_id; set allow_calendar_fallback=true to opt into weekends_only",
                self.id,
                leg.notional.currency()
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
            Some(id) if finstack_quant_core::dates::calendar_by_id_strict(id).is_ok() => Ok(id),
            _ if self.allow_calendar_fallback => Ok(finstack_quant_core::dates::WEEKENDS_ONLY_ID),
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
        finstack_quant_core::dates::calendar_by_id_strict(self.resolve_leg_calendar_id(leg)?)
    }

    pub(crate) fn adjusted_leg_start(&self, leg: &XccySwapLeg) -> Result<Date> {
        let cal = self.resolve_leg_calendar(leg)?;
        finstack_quant_core::dates::adjust(leg.leg.start, leg.leg.business_day_convention, cal)
    }

    pub(crate) fn adjusted_leg_end(&self, leg: &XccySwapLeg) -> Result<Date> {
        let cal = self.resolve_leg_calendar(leg)?;
        finstack_quant_core::dates::adjust(leg.leg.end, leg.leg.business_day_convention, cal)
    }

    /// Build one leg's accrual periods. Single source for the pricing paths
    /// (`pv_leg_in_reporting_currency` and the MtM-resetting pricer) and the
    /// reporting path (`leg_coupon_schedule`), so period boundaries, payment
    /// dates, and reset dates can never drift between priced and reported
    /// cashflows.
    pub(crate) fn leg_build_periods(
        &self,
        leg: &XccySwapLeg,
        calendar_id: &str,
    ) -> Result<Vec<finstack_quant_cashflows::builder::SchedulePeriod>> {
        finstack_quant_cashflows::builder::build_periods(leg.leg.schedule_params(
            calendar_id,
            leg.period_reset_lag(),
            false,
        ))
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
        fwd: &crate::swap_legs::ResolvedProjectionCurve,
        fixings: Option<&finstack_quant_core::market_data::scalars::ScalarTimeSeries>,
        period: &finstack_quant_cashflows::builder::SchedulePeriod,
        as_of: Date,
        args: XccyPeriodProjectionArgs<'_>,
    ) -> Result<ProjectedXccyPeriod> {
        use finstack_quant_cashflows::builder::FloatingLegCompounding;

        let XccyPeriodProjectionArgs {
            end_is_termination_date,
            projected_fixings,
        } = args;

        let projected = if !matches!(leg.leg.compounding, FloatingLegCompounding::Simple) {
            let calendar = self.resolve_leg_calendar(leg)?;
            match crate::swap_legs::project_float_leg_overnight(
                &leg.leg,
                period,
                fwd,
                fixings,
                as_of,
                crate::swap_legs::OvernightPeriodParams {
                    fixing_calendar: calendar,
                    end_is_termination_date,
                    need_observation_exposures: projected_fixings.is_some(),
                },
            )? {
                None => ProjectedXccyPeriod {
                    rate: 0.0,
                    year_fraction: 0.0,
                    fixing_date: None,
                    compound_factor: 1.0,
                },
                Some(projection) => {
                    if let Some(out) = projected_fixings {
                        out.extend(projection.projected_fixings(leg.leg.index_id.as_str()));
                    }
                    ProjectedXccyPeriod {
                        rate: projection.rate,
                        year_fraction: projection.accrual_year_fraction,
                        fixing_date: Some(projection.fixing_date),
                        compound_factor: projection.compound_factor,
                    }
                }
            }
        } else {
            let fixing_date = period.reset_date.unwrap_or(period.accrual_start);
            let forward_rate = if fixing_date < as_of {
                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                    fixings,
                    leg.leg.index_id.as_str(),
                    fixing_date,
                )?
            } else {
                fwd.rate_between_dates(
                    period.accrual_start,
                    period.accrual_end,
                    period.accrual_year_fraction,
                )?
            };
            if let Some(out) = projected_fixings {
                out.push(finstack_quant_cashflows::fixings::ProjectedFixing {
                    series_id: fixing_series_id(leg.leg.index_id.as_str()),
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
        let fwd = crate::swap_legs::ResolvedProjectionCurve::resolve(
            market,
            &leg.leg.forward_curve_id,
            &leg.leg.discount_curve_id,
        )?;
        let fixings = crate::swap_legs::optional_fixing_series(market, leg.leg.index_id.as_str())?;
        let spread = leg.leg.spread_bp.as_f64() / 10_000.0;

        let mut flows = Vec::with_capacity(periods.len());
        let mut projected_fixings = Vec::new();
        let final_period_end = periods.last().map(|period| period.accrual_end);
        for period in &periods {
            if period.payment_date <= as_of {
                continue;
            }
            let projected = self.projected_leg_period(
                leg,
                &fwd,
                fixings,
                period,
                as_of,
                XccyPeriodProjectionArgs {
                    end_is_termination_date: Some(period.accrual_end) == final_period_end,
                    projected_fixings: Some(&mut projected_fixings),
                },
            )?;
            let all_in = projected.all_in_rate(spread);
            let amount = leg.side.sign() * projected.unsigned_coupon(leg.notional.amount(), spread);
            flows.push(
                finstack_quant_core::cashflow::CashFlowParams::new(
                    period.payment_date,
                    projected.fixing_date.or(period.reset_date),
                    Money::from_model_value(amount, leg.notional.currency())?,
                    finstack_quant_core::cashflow::CFKind::Floating,
                    projected.year_fraction,
                    Some(all_in),
                )
                .build()?,
            );
        }
        Ok(finstack_quant_cashflows::schedule_from_classified_flows(
            flows,
            leg.leg.day_count,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(leg.notional),
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    projected_fixings,
                    ..Default::default()
                },
            },
        ))
    }

    fn leg_principal_schedule(&self, leg: &XccySwapLeg, anchor: Date) -> Result<CashFlowSchedule> {
        let start_date = self.adjusted_leg_start(leg)?;
        let end_date = self.adjusted_leg_end(leg)?;
        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(
            Money::from((0_i64, leg.notional.currency())),
            anchor,
            end_date,
        );
        // MtmResetting also requires initial AND final exchange; this arm makes the helper non-panicky if accidentally called on an MtM swap. `base_value` dispatches MtmResetting to `pricing_mtm::pv_mtm_reset` before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) {
            let initial_amount = initial_principal_sign(leg.side) * leg.notional.amount();
            let _ = builder.add_principal_event(
                start_date,
                start_date,
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
                end_date,
                end_date,
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
    /// Unrounded present value in the **reporting currency**, not the leg currency.
    fn pv_leg_in_reporting_currency_raw(
        &self,
        leg: &XccySwapLeg,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        self.validate_leg(leg)?;

        let calendar_id = self.resolve_leg_calendar_id(leg)?;
        let periods = self.leg_build_periods(leg, calendar_id)?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap leg schedule must contain at least 1 period".to_string(),
            ));
        }

        let start_date = self.adjusted_leg_start(leg)?;
        let end_date = self.adjusted_leg_end(leg)?;

        let unsettled_coupon = periods.iter().any(|period| period.payment_date > as_of);
        let unsettled_initial = matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) && start_date > as_of;
        let unsettled_final = matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) && end_date > as_of;
        if !unsettled_coupon && !unsettled_initial && !unsettled_final {
            return Ok(0.0);
        }

        let disc = context.get_discount(&leg.leg.discount_curve_id)?;
        let fwd = crate::swap_legs::ResolvedProjectionCurve::resolve(
            context,
            &leg.leg.forward_curve_id,
            &leg.leg.discount_curve_id,
        )?;
        let fixings = crate::swap_legs::optional_fixing_series(context, leg.leg.index_id.as_str())?;
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
                    finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
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
        ) && start_date > as_of
        {
            let df = disc.as_ref().df_between_dates(as_of, start_date)?;
            let cf_leg_currency = initial_principal_sign(leg.side) * leg.notional.amount() * df;
            let cf_rep = convert_pv(cf_leg_currency)?;
            pv.add(cf_rep)?;
        }

        // MtmResetting also requires final exchange; same defensive note as above — the MtM live-pricing path dispatches before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) && end_date > as_of
        {
            let df = disc.as_ref().df_between_dates(as_of, end_date)?;
            let cf_leg_currency = leg.side.sign() * leg.notional.amount() * df;
            let cf_rep = convert_pv(cf_leg_currency)?;
            pv.add(cf_rep)?;
        }

        // Floating coupons
        let final_period_end = periods.last().map(|period| period.accrual_end);
        for period in periods {
            if period.payment_date <= as_of {
                continue;
            }

            let projected = self.projected_leg_period(
                leg,
                &fwd,
                fixings,
                &period,
                as_of,
                XccyPeriodProjectionArgs {
                    end_is_termination_date: Some(period.accrual_end) == final_period_end,
                    projected_fixings: None,
                },
            )?;
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

            let spread = leg.leg.spread_bp.as_f64() / 10_000.0;
            let coupon = leg.side.sign() * projected.unsigned_coupon(leg.notional.amount(), spread);

            // Use relative date-based discounting for numerical stability.
            let df = disc.as_ref().df_between_dates(as_of, period.payment_date)?;
            let cf_leg_currency = coupon * df;

            let cf_rep = convert_pv(cf_leg_currency)?;
            pv.add(cf_rep)?;
        }

        pv.get_total()
    }
}

impl finstack_quant_valuations_core::instruments::Instrument for XccySwap {
    fn cashflow_export_terms(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<
        finstack_quant_valuations_core::instruments::cashflow_export::CashflowExportTerms,
    > {
        // Each leg discounts on its own currency's curve; the envelope reports
        // in the swap's reporting currency on that currency's leg curve.
        let mut terms = finstack_quant_valuations_core::instruments::cashflow_export::CashflowExportTerms::from_schedule(
            finstack_quant_cashflows::CashflowProvider::cashflow_schedule(self, market, as_of)?,
        );
        terms.currency_discount_curves.insert(
            self.leg1.notional.currency(),
            self.leg1.leg.discount_curve_id.clone(),
        );
        terms.currency_discount_curves.insert(
            self.leg2.notional.currency(),
            self.leg2.leg.discount_curve_id.clone(),
        );
        terms.discount_curve_id = terms
            .currency_discount_curves
            .get(&self.reporting_currency)
            .cloned();
        terms.reporting_currency = Some(self.reporting_currency);
        Ok(terms)
    }

    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::XccySwap,
        crate::RATES
    );
    fn last_payment_date(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<finstack_quant_core::dates::Date>> {
        let schedule =
            finstack_quant_cashflows::CashflowProvider::cashflow_schedule(self, curves, as_of)?;
        Ok(schedule
            .get_flows()
            .iter()
            .map(|flow| flow.get_date())
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
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.leg1.leg.discount_curve_id.clone());
        deps.add_discount_curve(self.leg2.leg.discount_curve_id.clone());
        for leg in [&self.leg1.leg, &self.leg2.leg] {
            if leg.compounding == finstack_quant_cashflows::builder::FloatingLegCompounding::Simple
                || leg.forward_curve_id != leg.discount_curve_id
            {
                deps.add_forward_curve(leg.forward_curve_id.clone());
            }
        }
        if self.leg1.notional.currency() != self.reporting_currency {
            deps.add_fx_pair(self.leg1.notional.currency(), self.reporting_currency);
        }
        if self.leg2.notional.currency() != self.reporting_currency {
            deps.add_fx_pair(self.leg2.notional.currency(), self.reporting_currency);
        }
        deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
            self.leg1.leg.index_id.as_str(),
        ));
        deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
            self.leg2.leg.index_id.as_str(),
        ));
        if let NotionalExchange::MtmResetting { resetting_side } = self.notional_exchange {
            let (constant_leg, resetting_leg) = self.partition_legs(resetting_side)?;
            deps.add_fx_pair(
                resetting_leg.notional.currency(),
                constant_leg.notional.currency(),
            );
            deps.add_series_id(crate::xccy_swap::pricing_mtm::mtm_fx_fixing_series_id(
                resetting_leg.notional.currency(),
                constant_leg.notional.currency(),
            ));
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        let pv = self.base_value_raw(market, as_of)?;
        Money::from_model_value(pv, self.reporting_currency)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.leg1.leg.end.max(self.leg2.leg.end))
    }

    fn base_value_raw(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        // One structural check shared with `validate_invariants`: leg
        // invariants, distinct leg currencies and MtM schedule alignment.
        self.validate()?;

        // Short-circuit expired swaps before probing FX/curves (settled trades have 0 PV).
        if !self.has_unsettled_flows(as_of)? {
            return Ok(0.0);
        }

        // Pre-flight FX reachability: fail loud with currency-pair context
        // BEFORE descending into per-cashflow conversion. Without this, missing
        // FX surfaces as `NotFound { id: "fx_matrix" }` from deep inside the
        // schedule loop, hiding which leg/pair was the offender.
        self.validate_fx_reachable(market)?;

        if let NotionalExchange::MtmResetting { resetting_side } = self.notional_exchange {
            return crate::xccy_swap::pricing_mtm::pv_mtm_reset_raw(
                self,
                resetting_side,
                market,
                as_of,
            );
        }

        // pv_leg_in_reporting_currency_raw builds its own period schedule; no need to pre-build here.
        let pv1_rep = self.pv_leg_in_reporting_currency_raw(&self.leg1, market, as_of)?;
        let pv2_rep = self.pv_leg_in_reporting_currency_raw(&self.leg2, market, as_of)?;

        Ok(pv1_rep + pv2_rep)
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl XccySwap {
    /// Returns `true` if either leg still has an unsettled coupon or principal exchange after `as_of`.
    pub(crate) fn has_unsettled_flows(&self, as_of: Date) -> Result<bool> {
        for leg in [&self.leg1, &self.leg2] {
            let periods = self.leg_build_periods(leg, self.resolve_leg_calendar_id(leg)?)?;
            if periods.is_empty() {
                return Err(finstack_quant_core::Error::Validation(
                    "XccySwap leg schedule must contain at least 1 period".to_string(),
                ));
            }
            let start_date = self.adjusted_leg_start(leg)?;
            let end_date = self.adjusted_leg_end(leg)?;
            let unsettled_coupon = periods.iter().any(|p| p.payment_date > as_of);
            let unsettled_initial = matches!(
                self.notional_exchange,
                NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
            ) && start_date > as_of;
            let unsettled_final = matches!(
                self.notional_exchange,
                NotionalExchange::Final
                    | NotionalExchange::InitialAndFinal
                    | NotionalExchange::MtmResetting { .. }
            ) && end_date > as_of;
            if unsettled_coupon || unsettled_initial || unsettled_final {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for XccySwap {
    fn raw_cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        self.validate()?;

        if !self.has_unsettled_flows(as_of)? {
            return Ok(finstack_quant_cashflows::schedule_from_classified_flows(
                Vec::new(),
                self.leg1.leg.day_count,
                finstack_quant_cashflows::ScheduleBuildOpts {
                    notional_hint: Some(Money::from((0_i64, self.reporting_currency))),
                    ..Default::default()
                },
            )
            .with_representation(
                finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
            ));
        }

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
            let schedule = crate::xccy_swap::pricing_mtm::mtm_cashflow_schedule(
                self,
                resetting_side,
                market,
                as_of,
            )?;
            return Ok(schedule
                .with_notional(Notional::par(0.0, self.reporting_currency)?)
                .with_representation(
                    finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                ));
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
        .with_representation(finstack_quant_cashflows::builder::CashflowRepresentation::Projected))
    }
}
