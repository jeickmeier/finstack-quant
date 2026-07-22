//! XCCY swap types and pricing.
//!
//! Market-standard conventions implemented:
//! - Floating coupons projected from forward curves on accrual boundaries
//! - Cashflows discounted with the leg-specific discount curve (multi-curve)
//! - Leg PVs converted into the reporting currency using spot FX at `as_of`
//! - Explicit calendars / business-day conventions; no implicit calendar fallbacks
//!
//! Notes:
//! - This is a deterministic-curve pricer (no fixings). Reset lag is therefore not modeled
//!   separately; the forward rate is taken directly from the forward curve for the accrual period.

use crate::cashflow::builder::{schedule::merge_cashflow_schedules, CashFlowSchedule, Notional};
use crate::cashflow::primitives::CFKind;
use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::pricing::swap_legs::robust_relative_df;
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

/// Whether the holder pays or receives a leg.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
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
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "pay" | "payer" => Ok(Self::Pay),
            "receive" | "rec" | "receiver" => Ok(Self::Receive),
            other => Err(format!(
                "Unknown leg side: '{}'. Valid: pay, receive, rec",
                other
            )),
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
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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

impl std::str::FromStr for NotionalExchange {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "none" => Ok(Self::None),
            "final" | "final_only" => Ok(Self::Final),
            "initial_and_final" | "initialandfinal" | "both" => Ok(Self::InitialAndFinal),
            other => {
                if let Some(side_str) = other.strip_prefix("mtm_resetting:") {
                    let resetting_side = side_str.parse::<ResettingSide>()?;
                    Ok(Self::MtmResetting { resetting_side })
                } else {
                    Err(format!(
                        "Unknown notional exchange: '{s}'. Valid: none, final, initial_and_final, both, mtm_resetting:leg1, mtm_resetting:leg2"
                    ))
                }
            }
        }
    }
}

/// Identifies which leg of an XCCY swap has its notional reset under
/// MtM-resetting. `Leg1` and `Leg2` refer to `XccySwap::leg1` and `XccySwap::leg2`
/// respectively.
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_export", ts(export, rename_all = "snake_case"))]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
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
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "leg1" | "leg_1" => Ok(Self::Leg1),
            "leg2" | "leg_2" => Ok(Self::Leg2),
            other => Err(format!(
                "Unknown resetting side: '{other}'. Valid: leg1, leg2"
            )),
        }
    }
}

/// One floating leg of an XCCY swap.
///
/// Each leg owns its own dates, discount curve, calendar, and stub conventions,
/// following the IRS leg-centric pattern.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
    #[schemars(with = "String")]
    pub start: Date,
    /// End date of the leg.
    #[schemars(with = "String")]
    pub end: Date,
    /// Coupon frequency.
    pub frequency: Tenor,
    /// Accrual day count.
    pub day_count: DayCount,
    /// Business day convention for schedule dates.
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Stub period handling rule.
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Spread in basis points (e.g. `Decimal::from(5)` = 5bp).
    #[serde(default)]
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
    schemars::JsonSchema,
)]
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
            notional: Money::new(10_000_000.0, Currency::USD),
            side: LegSide::Receive,
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: None,
            allow_calendar_fallback: true,
        };

        let eur_leg = XccySwapLeg {
            currency: Currency::EUR,
            notional: Money::new(9_200_000.0, Currency::EUR),
            side: LegSide::Pay,
            forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
            discount_curve_id: CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(10),
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: None,
            allow_calendar_fallback: true,
        };

        Self::new("XCCY-USDEUR-5Y", usd_leg, eur_leg, Currency::USD)
    }

    /// Set notional exchange convention.
    pub fn with_notional_exchange(mut self, exchange: NotionalExchange) -> Self {
        self.notional_exchange = exchange;
        self
    }

    /// Returns `Some(resetting_side)` if the swap is configured as MtM-resetting,
    /// `None` otherwise. Convenience over matching on `notional_exchange` directly.
    pub fn is_mtm_resetting(&self) -> Option<ResettingSide> {
        match self.notional_exchange {
            NotionalExchange::MtmResetting { resetting_side } => Some(resetting_side),
            _ => None,
        }
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
    /// Runs at the top of [`Instrument::base_value`] so a missing or
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
        // Validate notional is finite and positive
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

    /// Build one leg's accrual periods. Single source for both the pricing
    /// path (`pv_leg_in_reporting_ccy`) and the reporting path
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
                bdc: leg.bdc,
                calendar_id,
                end_of_month: false,
                day_count: leg.day_count,
                payment_lag_days: leg.payment_lag_days,
                reset_lag_days: leg.reset_lag_days,
                adjust_accrual_dates: false,
            },
        )
    }

    /// The single floating-rate projection shared by pricing and reporting:
    /// a recorded fixing when the period's fixing date has passed (the reset
    /// lag selects that date), otherwise the window-consistent simple forward
    /// over the accrual period (which, on a deterministic curve, is
    /// independent of the observation lag).
    fn projected_leg_period_rate(
        leg: &XccySwapLeg,
        fwd: &finstack_quant_core::market_data::term_structures::ForwardCurve,
        fixings: Option<&finstack_quant_core::market_data::scalars::ScalarTimeSeries>,
        period: &crate::cashflow::builder::periods::SchedulePeriod,
        as_of: Date,
    ) -> Result<f64> {
        use crate::instruments::common_impl::pricing::time::rate_between_on_dates;

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
        if !forward_rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Non-finite forward rate for period {} to {}",
                period.accrual_start, period.accrual_end
            )));
        }
        Ok(forward_rate)
    }

    /// Coupon schedule for one leg, projected through the SAME rate path the
    /// pricer discounts (`projected_leg_period_rate`): window-consistent
    /// forwards for future fixings, recorded fixings for past ones. Keeping a
    /// single projection prevents the reported cashflows from silently
    /// drifting away from what `base_value` actually prices.
    ///
    /// The only intentional difference from the pricing path is calendar
    /// resolution: reporting is strict (an unresolvable `calendar_id` is an
    /// error unless `allow_calendar_fallback` is set), while pricing falls
    /// back to weekends-only.
    fn leg_coupon_schedule(
        &self,
        leg: &XccySwapLeg,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<CashFlowSchedule> {
        let calendar_id = match leg.calendar_id.as_deref() {
            Some(id) if crate::cashflow::builder::calendar::resolve_calendar_strict(id).is_ok() => {
                id
            }
            _ if leg.allow_calendar_fallback => {
                crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID
            }
            _ => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "XccySwap '{}' leg {} requires a resolvable calendar_id",
                    self.id, leg.currency
                )))
            }
        };
        let periods = self.leg_build_periods(leg, calendar_id)?;
        let fwd = market.get_forward(&leg.forward_curve_id)?;
        let fixing_series_id = finstack_quant_core::market_data::fixings::fixing_series_id(
            leg.forward_curve_id.as_str(),
        );
        let fixings = market.get_series(&fixing_series_id).ok();
        let spread = decimal_to_f64(leg.spread_bp, "XccySwap leg spread_bp")? / 10_000.0;

        let mut flows = Vec::with_capacity(periods.len());
        for period in &periods {
            let rate = Self::projected_leg_period_rate(leg, fwd.as_ref(), fixings, period, as_of)?
                + spread;
            let amount = leg.side.coupon_sign()
                * leg.notional.amount()
                * rate
                * period.accrual_year_fraction;
            flows.push(crate::cashflow::primitives::CashFlow::new(
                period.payment_date,
                period.reset_date,
                Money::new(amount, leg.currency),
                crate::cashflow::primitives::CFKind::FloatReset,
                period.accrual_year_fraction,
                Some(rate),
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
        let _ = builder.principal(Money::new(0.0, leg.currency), anchor, leg.end);
        // MtmResetting also requires initial AND final exchange; this arm makes the helper non-panicky if accidentally called on an MtM swap. `base_value` dispatches MtmResetting to `pricing_mtm::pv_mtm_reset` before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) {
            let initial_amount = leg.side.initial_principal_sign() * leg.notional.amount();
            let _ = builder.add_principal_event(
                leg.start,
                Money::new(0.0, leg.currency),
                Some(Money::new(-initial_amount, leg.currency)),
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
                Money::new(0.0, leg.currency),
                Some(Money::new(-final_amount, leg.currency)),
                CFKind::Notional,
            );
        }
        Ok(builder
            .build(None)?
            .with_notional(Notional::par(leg.notional.amount(), leg.currency)))
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
    fn pv_leg_in_reporting_ccy(
        &self,
        leg: &XccySwapLeg,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        self.validate_leg(leg)?;

        // Pricing keeps the historical weekends-only fallback for an
        // unresolvable calendar; reporting (`leg_coupon_schedule`) is strict.
        let calendar_id = if leg.calendar_id.as_deref().is_some_and(|id| {
            crate::cashflow::builder::calendar::resolve_calendar_strict(id).is_ok()
        }) {
            leg.calendar_id.as_deref().unwrap_or_default()
        } else {
            crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID
        };
        let periods = self.leg_build_periods(leg, calendar_id)?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "XccySwap leg schedule must contain at least 1 period".to_string(),
            ));
        }

        let unsettled_coupon = periods.iter().any(|period| period.payment_date >= as_of);
        let unsettled_initial = matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) && leg.start >= as_of;
        let unsettled_final = matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) && leg.end >= as_of;
        if !unsettled_coupon && !unsettled_initial && !unsettled_final {
            return Ok(Money::new(0.0, self.reporting_currency));
        }

        // Curves
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
        // Use robust_relative_df for numerical stability (validated against Bloomberg SWPM)
        // MtmResetting also requires initial AND final exchange; this arm makes the helper non-panicky if accidentally called on an MtM swap. `base_value` dispatches MtmResetting to `pricing_mtm::pv_mtm_reset` before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::InitialAndFinal | NotionalExchange::MtmResetting { .. }
        ) && leg.start >= as_of
        {
            let df = robust_relative_df(disc.as_ref(), as_of, leg.start)?;
            let cf_leg_ccy = leg.side.initial_principal_sign() * leg.notional.amount() * df;
            let cf_rep = convert_pv(cf_leg_ccy)?;
            pv.add(cf_rep);
        }

        // MtmResetting also requires final exchange; same defensive note as above — the MtM live-pricing path dispatches before this method is reached.
        if matches!(
            self.notional_exchange,
            NotionalExchange::Final
                | NotionalExchange::InitialAndFinal
                | NotionalExchange::MtmResetting { .. }
        ) && leg.end >= as_of
        {
            let df = robust_relative_df(disc.as_ref(), as_of, leg.end)?;
            let cf_leg_ccy = leg.side.final_principal_sign() * leg.notional.amount() * df;
            let cf_rep = convert_pv(cf_leg_ccy)?;
            pv.add(cf_rep);
        }

        // Floating coupons
        for period in periods {
            if period.payment_date < as_of {
                continue;
            }

            let forward_rate =
                Self::projected_leg_period_rate(leg, fwd.as_ref(), fixings, &period, as_of)?;
            // Warn about extremely negative forward rates which may indicate curve issues.
            // Even in negative rate environments (JPY/CHF/EUR), rates below -5% are unusual.
            if forward_rate < EXTREME_NEGATIVE_RATE_THRESHOLD {
                tracing::warn!(
                    instrument_id = %self.id.as_str(),
                    period_start = %period.accrual_start,
                    period_end = %period.accrual_end,
                    forward_rate = forward_rate,
                    threshold = EXTREME_NEGATIVE_RATE_THRESHOLD,
                    "Forward rate is highly negative; verify curve construction"
                );
            }

            let spread = decimal_to_f64(leg.spread_bp, "XccySwap leg spread_bp")?;
            let total_rate = forward_rate + spread / 10_000.0;
            let coupon = leg.side.coupon_sign()
                * leg.notional.amount()
                * total_rate
                * period.accrual_year_fraction;

            // Use robust_relative_df for numerical stability
            let df = robust_relative_df(disc.as_ref(), as_of, period.payment_date)?;
            let cf_leg_ccy = coupon * df;

            let cf_rep = convert_pv(cf_leg_ccy)?;
            pv.add(cf_rep);
        }

        Ok(Money::new(pv.total(), self.reporting_currency))
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

        // pv_leg_in_reporting_ccy builds its own period schedule; no need to pre-build here.
        let pv1_rep = self.pv_leg_in_reporting_ccy(&self.leg1, market, as_of)?;
        let pv2_rep = self.pv_leg_in_reporting_ccy(&self.leg2, market, as_of)?;

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
                .with_notional(Notional::par(0.0, self.reporting_currency))
                .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected));
        }

        let leg1_schedule = self.leg_coupon_schedule(&self.leg1, market, as_of)?;
        let leg2_schedule = self.leg_coupon_schedule(&self.leg2, market, as_of)?;
        let leg1_principal = self.leg_principal_schedule(&self.leg1, anchor)?;
        let leg2_principal = self.leg_principal_schedule(&self.leg2, anchor)?;

        Ok(merge_cashflow_schedules(
            [leg1_schedule, leg1_principal, leg2_schedule, leg2_principal],
            Notional::par(0.0, self.reporting_currency),
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
                notional: Money::new(1_000_000.0, Currency::USD),
                side: LegSide::Receive,
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                discount_curve_id: CurveId::new("USD-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: None,
                allow_calendar_fallback: true,
            },
            XccySwapLeg {
                currency: Currency::EUR,
                notional: Money::new(900_000.0, Currency::EUR),
                side: LegSide::Pay,
                forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: None,
                allow_calendar_fallback: true,
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
                notional: Money::new(1_000_000.0, Currency::USD),
                side: LegSide::Receive,
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                discount_curve_id: CurveId::new("USD-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: None,
                allow_calendar_fallback: true,
            },
            XccySwapLeg {
                currency: Currency::EUR,
                notional: Money::new(900_000.0, Currency::EUR),
                side: LegSide::Pay,
                forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: None,
                allow_calendar_fallback: true,
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

        fn assert_leg_side(label: &str, expected: LegSide) {
            assert!(matches!(LegSide::from_str(label), Ok(value) if value == expected));
        }

        let variants = [LegSide::Pay, LegSide::Receive];
        for v in variants {
            let s = v.to_string();
            let parsed = LegSide::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        assert_leg_side("rec", LegSide::Receive);
        assert!(LegSide::from_str("invalid").is_err());
    }

    #[test]
    fn notional_exchange_fromstr_display_roundtrip() {
        use std::str::FromStr;

        fn assert_notional_exchange(label: &str, expected: NotionalExchange) {
            assert!(matches!(NotionalExchange::from_str(label), Ok(value) if value == expected));
        }

        let variants = [
            NotionalExchange::None,
            NotionalExchange::Final,
            NotionalExchange::InitialAndFinal,
        ];
        for v in variants {
            let s = v.to_string();
            let parsed = NotionalExchange::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        assert_notional_exchange("both", NotionalExchange::InitialAndFinal);
        assert!(NotionalExchange::from_str("invalid").is_err());
    }

    #[test]
    fn resetting_side_fromstr_display_roundtrip() {
        use std::str::FromStr;
        for side in [ResettingSide::Leg1, ResettingSide::Leg2] {
            let s = side.to_string();
            let parsed = ResettingSide::from_str(&s).expect("roundtrip parse");
            assert_eq!(side, parsed, "roundtrip failed for {s}");
        }
        // Underscore alias path
        assert_eq!(
            ResettingSide::from_str("leg_1").expect("leg_1 alias"),
            ResettingSide::Leg1,
            "roundtrip failed for leg_1"
        );
        assert_eq!(
            ResettingSide::from_str("leg_2").expect("leg_2 alias"),
            ResettingSide::Leg2,
            "roundtrip failed for leg_2"
        );
        assert!(ResettingSide::from_str("garbage").is_err());
    }

    #[test]
    fn notional_exchange_mtm_resetting_display_and_parse_roundtrip() {
        use std::str::FromStr;
        let variants = [
            NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg1,
            },
            NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg2,
            },
        ];
        for v in variants {
            let s = v.to_string();
            let parsed = NotionalExchange::from_str(&s).expect("roundtrip parse");
            assert_eq!(v, parsed, "roundtrip failed for '{s}'");
        }
        // Negative cases: malformed mtm_resetting inputs must be rejected
        assert!(NotionalExchange::from_str("mtm_resetting").is_err());
        assert!(NotionalExchange::from_str("mtm_resetting:").is_err());
        assert!(NotionalExchange::from_str("mtm_resetting:garbage").is_err());
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
    fn is_mtm_resetting_returns_correct_side() {
        let mut swap = XccySwap::example();
        assert_eq!(swap.is_mtm_resetting(), None);

        swap = swap.with_notional_exchange(NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg2,
        });
        assert_eq!(swap.is_mtm_resetting(), Some(ResettingSide::Leg2));
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
        swap.leg2.notional = finstack_quant_core::money::Money::new(1.0, Currency::USD);

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
            notional: Money::new(9_200_000.0, Currency::EUR),
            side: LegSide::Receive,
            forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
            discount_curve_id: CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: None,
            allow_calendar_fallback: true,
        };
        let mut leg2 = leg1.clone();
        leg2.currency = Currency::USD;
        leg2.notional = Money::new(10_000_000.0, Currency::USD);
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
            notional: Money::new(9_200_000.0, Currency::EUR),
            side: LegSide::Receive,
            forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
            discount_curve_id: CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: None,
            allow_calendar_fallback: true,
        };
        let mut leg2 = leg1.clone();
        leg2.currency = Currency::USD;
        leg2.notional = Money::new(10_000_000.0, Currency::USD);
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
}
