//! Total Return Swap (TRS) pricing engine.
//!
//! This module provides shared pricing infrastructure for equity and fixed income
//! total return swaps. It separates the common period iteration and discounting
//! logic from underlying-specific return calculations.
//!
//! # Architecture
//!
//! The TRS pricing engine uses a trait-based approach:
//! - [`TrsReturnModel`]: Trait for underlying-specific return calculations
//! - [`TrsEngine`]: Shared pricing logic for all TRS types
//!
//! This allows equity TRS and fixed income TRS to share the common infrastructure
//! while implementing their own return calculation logic.
//!
//! # Financing-leg rate compounding
//!
//! [`TrsEngine::pv_financing_leg`] and [`TrsEngine::pv_financing_float_only`]
//! project each period's floating rate according to the financing leg's
//! `FinancingLegSpec.compounding` setting:
//!
//! - `simple` — the **discount-factor-implied simple forward** over the accrual
//!   period ([`ForwardCurve::rate_between_dates`]), correct for a term-rate-financed TRS
//!   (e.g. 3M Term SOFR) where the period length matches the index tenor.
//! - `compounded_*` — **daily-compounds** the overnight forward via
//!   [`crate::instruments::pricing::overnight::project_overnight_coupon`]
//!   with the leg's lookback / observation shift / rate cut-off
//!   (`compounded_in_arrears` with zero lookback is cleared-OIS style).
//! - `simple_average` — rejected.
//!   The simple average would drop the daily-compounding convexity (~12–15 bp
//!   of rate at current levels).
//!
//! [`TrsEngine::financing_annuity`] sums discounted accrual fractions. Term
//! legs use the schedule year fraction; overnight legs use the projector
//! year fraction so `pv_financing_leg = pv_financing_float_only + spread × annuity`.

use crate::instruments::FinancingLegSpec;
use crate::instruments::TrsScheduleSpec;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use rust_decimal::prelude::ToPrimitive;

use crate::instruments::pricing::overnight::{
    adjust_overnight_accrual_boundaries, project_overnight_coupon, OvernightCouponProjectionInput,
    OvernightProjectionCurve,
};
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_cashflows::fixings::ProjectedFixing;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::fixings::fixing_series_id;

/// Financing-leg `(payment_date, amount)` flows and the `FIXING:` index
/// observations they project (see [`TrsEngine::cashflows_financing_leg_with_fixings`]).
pub type FinancingLegFlows = (Vec<(Date, Money)>, Vec<ProjectedFixing>);

/// Projected TRS financing coupon for one accrual window.
struct FinancingPeriodProjection {
    /// Index rate excluding spread.
    rate: f64,
    /// Accrual fraction used for the coupon amount.
    year_fraction: f64,
    /// Compound factor `∏(1 + rᵢ dᵢ)` or `1 + rate × τ` for term financing.
    compound_factor: f64,
    /// Raw index observations the projection used, when requested.
    fixings: Vec<ProjectedFixing>,
}

impl FinancingPeriodProjection {
    fn new(
        rate: f64,
        year_fraction: f64,
        compound_factor: f64,
    ) -> finstack_quant_core::Result<Self> {
        let projection = Self {
            rate,
            year_fraction,
            compound_factor,
            fixings: Vec::new(),
        };
        if !projection.rate.is_finite()
            || !projection.compound_factor.is_finite()
            || !projection.year_fraction.is_finite()
        {
            return Err(finstack_quant_core::Error::Validation(
                "Non-finite TRS financing projection".to_string(),
            ));
        }
        Ok(projection)
    }

    fn unsigned_coupon(&self, notional: f64, spread: f64) -> f64 {
        notional * (self.compound_factor - 1.0) + notional * spread * self.year_fraction
    }
}

/// Project one financing-leg accrual period's floating rate, excluding spread.
///
/// `simple` legs use the discount-factor-implied simple forward over the period;
/// compounded (OIS / RFR) legs daily-compound the overnight forward
/// and return the equivalent simple rate `(∏(1+rᵢ·dᵢ)−1)/τ`, capturing the
/// daily-compounding convexity the arithmetic average drops.
///
/// A `simple` period that started before `as_of` requires its published
/// fixing. One that starts on `as_of` uses that day's fixing when the series
/// already holds it and the forward projection otherwise; later periods are
/// always projected. Only projected rates are reported as fixing exposures.
///
/// For compounded periods that have already started
/// (`period_start <= as_of < period_end`), the function splices realized daily
/// fixings (from `fixings`) with projected overnight forwards through the shared
/// overnight coupon projector. Fully-future periods (`period_start > as_of`)
/// use the same projector with entirely projected observations. Missing realized
/// fixings for an in-progress period are errors.
#[allow(clippy::too_many_arguments)]
fn financing_period_projection(
    financing: &FinancingLegSpec,
    fwd: &ForwardCurve,
    fixings: Option<&ScalarTimeSeries>,
    period_start: Date,
    period_end: Date,
    period_year_fraction: f64,
    coupon_frequency: Tenor,
    as_of: Date,
    calendar_id: &str,
    accrual_bdc: BusinessDayConvention,
    currency: Currency,
    collect_fixings: bool,
) -> finstack_quant_core::Result<FinancingPeriodProjection> {
    match financing.compounding {
        FloatingLegCompounding::SimpleAverage => Err(finstack_quant_core::Error::Validation(
            "financing_leg.compounding = simple_average is not supported; use simple or a \
             compounded_* variant"
                .to_string(),
        )),
        FloatingLegCompounding::Simple => {
            let published = if period_start < as_of {
                Some(
                    finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                        fixings,
                        financing.forward_curve_id.as_str(),
                        period_start,
                        as_of,
                    )?,
                )
            } else if period_start == as_of {
                fixings.and_then(|series| series.value_on_exact(period_start).ok())
            } else {
                None
            };
            let is_projected = published.is_none();
            let rate = match published {
                Some(rate) => rate,
                None => fwd.rate_between_dates(period_start, period_end, period_year_fraction)?,
            };
            let mut projection = FinancingPeriodProjection::new(
                rate,
                period_year_fraction,
                1.0 + rate * period_year_fraction,
            )?;
            if collect_fixings && is_projected {
                projection.fixings.push(ProjectedFixing {
                    series_id: fixing_series_id(financing.forward_curve_id.as_str()),
                    date: period_start,
                    value: Some(rate),
                });
            }
            Ok(projection)
        }
        compounding @ (FloatingLegCompounding::CompoundedInArrears { .. }
        | FloatingLegCompounding::CompoundedWithObservationShift { .. }
        | FloatingLegCompounding::CompoundedWithRateCutoff { .. }) => {
            let calendar: &dyn finstack_quant_core::dates::HolidayCalendar =
                if calendar_id.is_empty() {
                    super::overnight::resolve_overnight_fixing_calendar(
                        None,
                        currency,
                        "TRS financing",
                    )?
                } else if calendar_id == finstack_quant_core::dates::WEEKENDS_ONLY_ID {
                    &finstack_quant_core::dates::WEEKENDS_ONLY
                } else {
                    super::overnight::resolve_overnight_fixing_calendar(
                        Some(calendar_id),
                        currency,
                        "TRS financing",
                    )?
                };
            let (accrual_start, accrual_end) = adjust_overnight_accrual_boundaries(
                period_start,
                period_end,
                accrual_bdc,
                calendar,
            )?;
            if accrual_end <= accrual_start {
                return FinancingPeriodProjection::new(0.0, 0.0, 1.0);
            }
            let projection = project_overnight_coupon(OvernightCouponProjectionInput {
                curve: OvernightProjectionCurve::Forward(fwd),
                fixings,
                fixing_id: financing.forward_curve_id.as_str(),
                as_of,
                accrual_start,
                accrual_end,
                day_count: financing.day_count,
                coupon_frequency: Some(coupon_frequency),
                coupon_period: if financing.day_count
                    == finstack_quant_core::dates::DayCount::Act365L
                {
                    (period_start, period_end)
                } else {
                    (accrual_start, accrual_end)
                },
                compounding: &compounding,
                fixing_calendar: calendar,
                compounded_spread: 0.0,
                need_observation_exposures: collect_fixings,
            })?;
            let mut financing_projection = FinancingPeriodProjection::new(
                projection.rate,
                projection.accrual_year_fraction,
                projection.compound_factor,
            )?;
            financing_projection.fixings = projection
                .observation_exposures
                .iter()
                .map(|observation| ProjectedFixing {
                    series_id: fixing_series_id(financing.forward_curve_id.as_str()),
                    date: observation.observation_start,
                    value: Some(observation.projected_rate),
                })
                .collect();
            Ok(financing_projection)
        }
    }
}

/// Visit every financing period paid after `as_of`, in schedule order, with
/// its payment date and projected coupon.
///
/// Accrues on the financing leg's day count (not the schedule day count,
/// which governs date generation) and projects the period rate per the leg's
/// compounding convention: simple term rate, or daily-compounded overnight
/// splicing realized `FIXING:{forward_curve_id}` observations into
/// in-progress periods. The fixing series is optional here; a missing fixing
/// errors only when an in-progress period needs it.
fn for_each_financing_period(
    financing: &FinancingLegSpec,
    schedule: &TrsScheduleSpec,
    market: &MarketContext,
    as_of: Date,
    currency: Currency,
    collect_fixings: bool,
    mut visit: impl FnMut(Date, FinancingPeriodProjection) -> finstack_quant_core::Result<()>,
) -> finstack_quant_core::Result<()> {
    let fwd = market.get_forward(financing.forward_curve_id.as_str())?;
    let fixings = finstack_quant_core::market_data::fixings::get_fixing_series(
        market,
        financing.forward_curve_id.as_str(),
    )
    .ok();
    let period_schedule = schedule.period_schedule()?;
    // Overnight accrual boundaries always land on business days; unadjusted
    // schedules fall back to Modified Following for that adjustment.
    let accrual_bdc = if schedule.params.adjust_accrual_dates {
        schedule.params.business_day_convention
    } else {
        BusinessDayConvention::ModifiedFollowing
    };

    for i in 1..period_schedule.dates.len() {
        let period_start = period_schedule.dates[i - 1];
        let period_end = period_schedule.dates[i];
        let payment_date = schedule.payment_date_for(period_end)?;

        if payment_date <= as_of {
            continue;
        }

        let ctx = DayCountContext {
            frequency: Some(schedule.params.frequency),
            coupon_period: (financing.day_count == DayCount::Act365L)
                .then_some((period_start, period_end)),
            ..Default::default()
        };
        let yf = financing
            .day_count
            .year_fraction(period_start, period_end, ctx)?;
        let projected = financing_period_projection(
            financing,
            fwd.as_ref(),
            fixings,
            period_start,
            period_end,
            yf,
            schedule.params.frequency,
            as_of,
            schedule.params.calendar_id.as_str(),
            accrual_bdc,
            currency,
            collect_fixings,
        )?;
        visit(payment_date, projected)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn financing_period_rate(
    financing: &FinancingLegSpec,
    fwd: &ForwardCurve,
    fixings: Option<&ScalarTimeSeries>,
    period_start: Date,
    period_end: Date,
    period_year_fraction: f64,
    coupon_frequency: Tenor,
    as_of: Date,
    calendar_id: &str,
    currency: Currency,
) -> finstack_quant_core::Result<f64> {
    Ok(financing_period_projection(
        financing,
        fwd,
        fixings,
        period_start,
        period_end,
        period_year_fraction,
        coupon_frequency,
        as_of,
        calendar_id,
        BusinessDayConvention::ModifiedFollowing,
        currency,
        false,
    )?
    .rate)
}

/// Parameters for total return leg calculation.
#[derive(Debug, Clone)]
pub struct TotalReturnLegParams<'a> {
    /// Schedule specification for payment periods.
    pub schedule: &'a TrsScheduleSpec,
    /// Notional amount for the leg.
    pub notional: Money,
    /// Discount curve identifier.
    pub discount_curve_id: &'a str,
    /// Initial level of the underlying (if known).
    pub initial_level: Option<f64>,
}

/// Trait for underlying-specific total return models.
///
/// Implementations of this trait provide the logic for calculating
/// total returns over a period for different underlying types (equity vs fixed income).
///
/// # Return Value Contract
///
/// Implementations **must** return:
/// - **Finite values**: Returns must be finite (`is_finite() == true`). NaN or Inf values
///   will propagate through PV calculations and break determinism guarantees.
/// - **Reasonable bounds**: While there's no hard limit, returns outside [-1.0, 10.0] per period
///   are unusual and may indicate a bug. Returns below -1.0 imply more than 100% loss.
///
/// # Example Implementation
pub trait TrsReturnModel: Send + Sync {
    /// Computes total return over a period.
    ///
    /// # Arguments
    /// * `inputs` — Valuation date, period dates, year fractions, and initial level
    /// * `context` — Market context for data access
    ///
    /// # Returns
    ///
    /// Total return as a decimal (e.g., 0.05 for 5% return).
    ///
    /// # Contract
    ///
    /// - Return value **must** be finite
    /// - Return value **should** be in a reasonable range (typically -1.0 to 10.0 per period)
    /// - Implementations should return an error rather than returning NaN/Inf
    fn period_return(
        &self,
        inputs: &PeriodReturnInputs,
        context: &MarketContext,
    ) -> finstack_quant_core::Result<f64>;
}

/// Inputs for a single-period total-return computation.
///
/// Bundles the valuation date, period dates, and the schedule-day-count year
/// fractions the engine has already computed. Implementations should prefer
/// the *dates* (with date-based curve lookups) over the raw year fractions
/// whenever a discount factor is needed, so results stay correct when the
/// curve base date differs from `as_of`.
#[derive(Debug, Clone, Copy)]
pub struct PeriodReturnInputs {
    /// Valuation date.
    pub as_of: Date,
    /// Start date of the period.
    pub period_start: Date,
    /// End date of the period.
    pub period_end: Date,
    /// Year fraction from `as_of` to the period start (negative when the
    /// period is already in progress).
    pub t_start: f64,
    /// Year fraction from `as_of` to the period end.
    pub t_end: f64,
    /// Initial level of the underlying.
    pub initial_level: f64,
}

/// Common TRS pricing engine for shared calculations.
///
/// Provides utility functions for calculating present values of TRS legs
/// and other common pricing operations shared between equity and fixed income TRS.
pub struct TrsEngine;

impl TrsEngine {
    /// Calculates the present value of a total return leg using shared logic.
    ///
    /// This method contains the common period iteration and discounting logic,
    /// while delegating underlying-specific return calculations to the model.
    ///
    /// # Arguments
    /// * `params` — Parameters for the total return leg calculation
    /// * `market` — Market context containing curves and market data
    /// * `as_of` — Valuation date; total-return cashflows on or before this
    ///   date are excluded from the present value
    /// * `model` — Model implementing TrsReturnModel for underlying-specific logic
    ///
    /// # Returns
    /// Present value of the total return leg in the instrument's currency.
    pub fn pv_total_return_leg_with_model(
        params: TotalReturnLegParams,
        market: &MarketContext,
        as_of: Date,
        model: &impl TrsReturnModel,
    ) -> finstack_quant_core::Result<Money> {
        let disc = market.get_discount(params.discount_curve_id)?;
        let currency = params.notional.currency();
        let mut pv = NeumaierAccumulator::new();
        for (date, amount) in
            Self::cashflows_total_return_leg_with_model(params, market, as_of, model)?
        {
            pv.add(amount.amount() * disc.as_ref().df_between_dates(as_of, date)?);
        }
        Money::new(pv.total(), currency)
    }

    /// Projects the undiscounted total-return leg cashflows, one per period.
    ///
    /// Each period pays `notional × period return` on its payment date, where
    /// the return comes from `model`. Periods whose payment date is on or
    /// before `as_of` are omitted; a completed accrual stays in the list until
    /// its (possibly lagged) payment date.
    ///
    /// # Arguments
    /// * `params` — Total return leg parameters: schedule, notional (sign and
    ///   currency of every flow), discount curve id and initial index level
    ///   (defaults to `1.0` when absent)
    /// * `market` — Market context the return model reads (spot, curves,
    ///   fixings)
    /// * `as_of` — Valuation date; flows paying on or before it are excluded
    /// * `model` — Underlying-specific period return model
    ///
    /// # Returns
    /// `(payment_date, amount)` pairs in schedule order, in the notional's
    /// currency.
    ///
    /// # Errors
    /// Returns an error when the schedule cannot be built, a payment date
    /// cannot be resolved, the model fails, or the model returns a non-finite
    /// period return.
    pub fn cashflows_total_return_leg_with_model(
        params: TotalReturnLegParams,
        market: &MarketContext,
        as_of: Date,
        model: &impl TrsReturnModel,
    ) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let period_schedule = params.schedule.period_schedule()?;

        let mut flows = Vec::new();
        let currency = params.notional.currency();

        for i in 1..period_schedule.dates.len() {
            let period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];
            let payment_date = params.schedule.payment_date_for(period_end)?;

            if payment_date <= as_of {
                continue;
            }

            let day_count = params.schedule.params.day_count;
            let t_start =
                day_count.signed_year_fraction(as_of, period_start, DayCountContext::default())?;
            let t_end =
                day_count.signed_year_fraction(as_of, period_end, DayCountContext::default())?;

            let total_return = model.period_return(
                &PeriodReturnInputs {
                    as_of,
                    period_start,
                    period_end,
                    t_start,
                    t_end,
                    initial_level: params.initial_level.unwrap_or(1.0),
                },
                market,
            )?;

            if !total_return.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "TRS return model produced non-finite return ({}) for period {} to {}",
                    total_return, period_start, period_end
                )));
            }

            let payment = params.notional.amount() * total_return;

            // A completed accrual remains in PV until its lagged payment date.
            flows.push((payment_date, Money::new(payment, currency)?));
        }

        Ok(flows)
    }

    /// Calculates the present value of the financing leg.
    ///
    /// This is shared by both equity and fixed income TRS.
    ///
    /// # Arguments
    /// * `financing` — Financing leg specification
    /// * `schedule` — Schedule specification for payment periods
    /// * `notional` — Notional amount for the leg
    /// * `market` — Market context containing curves and market data
    /// * `as_of` — Valuation date; financing cashflows on or before this date
    ///   are excluded from the present value
    ///
    /// # Returns
    /// Present value of the financing leg in the instrument's currency.
    pub fn pv_financing_leg(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let disc = market.get_discount(financing.discount_curve_id.as_str())?;
        let currency = notional.currency();
        let mut pv = NeumaierAccumulator::new();
        for (date, amount) in
            Self::cashflows_financing_leg(financing, schedule, notional, market, as_of)?
        {
            pv.add(amount.amount() * disc.as_ref().df_between_dates(as_of, date)?);
        }
        Money::new(pv.total(), currency)
    }

    /// Projects the undiscounted financing leg cashflows, one per period.
    ///
    /// Each period pays `notional × (projected rate + spread) × accrual`, with
    /// the rate projected per the leg's compounding convention (simple term
    /// rate or daily-compounded overnight, splicing realized fixings from the
    /// `FIXING:{forward_curve_id}` series for in-progress periods). Amounts are
    /// unsigned; callers apply the swap side.
    ///
    /// # Arguments
    /// * `financing` — Financing leg specification: forward and discount curve
    ///   ids, spread in basis points, day count and compounding convention
    /// * `schedule` — Payment schedule specification (dates, frequency,
    ///   calendar, payment lag)
    /// * `notional` — Notional amount; its currency is the currency of every flow
    /// * `market` — Market context holding the forward curve and fixings
    /// * `as_of` — Valuation date; flows paying on or before it are excluded
    ///
    /// # Returns
    /// `(payment_date, amount)` pairs in schedule order.
    ///
    /// # Errors
    /// Returns an error when the forward curve is missing, the spread is not
    /// representable as `f64`, the schedule or a payment date cannot be built,
    /// or a required fixing for an in-progress period is missing.
    pub fn cashflows_financing_leg(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        Ok(Self::financing_leg_flows(financing, schedule, notional, market, as_of, false)?.0)
    }

    /// Financing-leg cashflows together with the raw index observations they
    /// project, for cashflow exports whose fixings a horizon roll must hold.
    ///
    /// # Arguments
    /// * `financing` — Forward/discount curve ids, spread, day count and
    ///   compounding of the financing leg.
    /// * `schedule` — Payment periods, calendar and payment lag.
    /// * `notional` — Currency amount the coupons accrue on.
    /// * `market` — Must contain `financing.forward_curve_id`; in-progress
    ///   periods also read realized `FIXING:{forward_curve_id}` observations.
    /// * `as_of` — Valuation date; periods paid on or before it are excluded.
    ///
    /// # Returns
    /// The `(payment_date, amount)` flows of [`Self::cashflows_financing_leg`]
    /// and the `FIXING:{forward_curve_id}` observations projected at `as_of`:
    /// each future term reset for `simple` legs, each overnight observation for
    /// compounded legs.
    pub fn cashflows_financing_leg_with_fixings(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<FinancingLegFlows> {
        Self::financing_leg_flows(financing, schedule, notional, market, as_of, true)
    }

    fn financing_leg_flows(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        market: &MarketContext,
        as_of: Date,
        collect_fixings: bool,
    ) -> finstack_quant_core::Result<FinancingLegFlows> {
        let spread_decimal = financing.spread_bp.to_f64().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "TRS financing spread_bp ({}) is not representable as f64",
                financing.spread_bp
            ))
        })? / 10_000.0;
        let currency = notional.currency();
        let mut flows = Vec::new();
        let mut projected_fixings = Vec::new();
        for_each_financing_period(
            financing,
            schedule,
            market,
            as_of,
            currency,
            collect_fixings,
            |payment_date, projected| {
                let payment = projected.unsigned_coupon(notional.amount(), spread_decimal);
                // A completed accrual remains in PV until its lagged payment date.
                flows.push((payment_date, Money::new(payment, currency)?));
                projected_fixings.extend(projected.fixings);
                Ok(())
            },
        )?;
        Ok((flows, projected_fixings))
    }

    /// Calculates the financing annuity for par spread calculation.
    ///
    /// Term legs sum discounted schedule year fractions. Overnight legs sum
    /// the projector accrual fractions used by [`Self::pv_financing_leg`] so
    /// `pv_financing_leg = pv_financing_float_only + spread × annuity`.
    ///
    /// # Arguments
    /// * `financing` — Discount/forward curve ids, day count, and compounding
    ///   that select schedule versus overnight projector year fractions.
    /// * `schedule` — Payment dates, calendar, and lag used to discount each
    ///   period's accrual fraction.
    /// * `notional` — Scale of the annuity (currency amount, not percent of par).
    /// * `market` — Must contain `financing.discount_curve_id` and
    ///   `financing.forward_curve_id` (overnight legs also read fixings).
    /// * `as_of` — Valuation date; periods ending on or before this date are
    ///   excluded.
    ///
    /// # Returns
    /// Financing annuity (sum of discounted year fractions × notional).
    ///
    /// # Errors
    ///
    /// Returns an error if a required curve is missing, overnight projection
    /// fails, or the computed annuity is below
    /// [`crate::instruments::pricing::swap_legs::ANNUITY_EPSILON`] (1e-12),
    /// which would cause divide-by-zero in downstream par spread calculations.
    /// This typically occurs when:
    /// - All periods have already expired (payment dates before as_of)
    /// - Extreme discounting scenarios with very high rates
    pub fn financing_annuity(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let disc = market.get_discount(financing.discount_curve_id.as_str())?;
        let mut annuity = NeumaierAccumulator::new();
        // Term legs accrue on the schedule year fraction. Overnight legs use
        // the projector fraction so the spread term matches `unsigned_coupon`
        // (`N × spread × proj_yf`).
        for_each_financing_period(
            financing,
            schedule,
            market,
            as_of,
            notional.currency(),
            false,
            |payment_date, projected| {
                let df = disc.as_ref().df_between_dates(as_of, payment_date)?;
                annuity.add(df * projected.year_fraction);
                Ok(())
            },
        )?;

        let result = annuity.total() * notional.amount();

        if result.abs() < crate::instruments::pricing::swap_legs::ANNUITY_EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Financing annuity ({:.2e}) is below minimum threshold ({:.2e}). \
                 This may indicate all periods have expired or extreme discounting scenarios. \
                 Cannot compute par spread with near-zero annuity.",
                result,
                crate::instruments::pricing::swap_legs::ANNUITY_EPSILON
            )));
        }

        Ok(result)
    }

    /// PV of the financing leg excluding the spread component.
    ///
    /// This is the "floating-only" PV: the present value of projected forward rate
    /// payments without the spread. Used by par spread calculators to solve for the
    /// spread that zeroes the NPV.
    ///
    /// Relationship: `pv_financing_leg = pv_financing_float_only + spread × annuity`
    ///
    /// # Arguments
    /// * `financing` — Financing leg specification (curves, day count,
    ///   compounding); its spread is ignored.
    /// * `schedule` — Payment schedule specification (dates, calendar, lag).
    /// * `notional` — Notional the floating coupons accrue on, in the
    ///   currency of the result.
    /// * `market` — Market holding `financing.discount_curve_id`,
    ///   `financing.forward_curve_id` and any `FIXING:{forward_curve_id}` series.
    /// * `as_of` — Valuation date; periods paid on or before it are excluded.
    ///
    /// # Returns
    /// Discounted sum of the spread-free financing coupons.
    ///
    /// # Errors
    /// Returns an error when a curve is missing, a payment date cannot be
    /// resolved, or a required fixing for an in-progress period is missing.
    pub fn pv_financing_float_only(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let disc = market.get_discount(financing.discount_curve_id.as_str())?;
        let mut total_pv = NeumaierAccumulator::new();
        for_each_financing_period(
            financing,
            schedule,
            market,
            as_of,
            notional.currency(),
            false,
            |payment_date, projected| {
                let payment = projected.unsigned_coupon(notional.amount(), 0.0);
                let df = disc.as_ref().df_between_dates(as_of, payment_date)?;
                total_pv.add(payment * df);
                Ok(())
            },
        )?;

        Ok(total_pv.total())
    }

    /// Par financing spread in basis points: the spread that zeroes the
    /// total-return receiver's NPV.
    ///
    /// ```text
    /// NPV = PV(total_return) - PV(financing_float) - s * Annuity = 0
    /// s_par = (PV(total_return) - PV(financing_float)) / Annuity
    /// ```
    ///
    /// `PV(financing_float)` is [`Self::pv_financing_float_only`]. The result
    /// is a market-level quote (analogous to a swap rate) and does not depend
    /// on the trade side.
    ///
    /// # Arguments
    /// * `total_return_pv` — Present value of the total-return leg (unsigned,
    ///   receiver perspective) in the notional's currency.
    /// * `financing` — Financing leg specification (curves, day count,
    ///   compounding; its own spread is ignored).
    /// * `schedule` — Payment schedule shared by both legs.
    /// * `notional` — Notional the financing coupons accrue on.
    /// * `market` — Market holding the financing discount and forward curves
    ///   and any `FIXING:{forward_curve_id}` series.
    /// * `as_of` — Valuation date; periods paid on or before it are excluded.
    /// * `annuity` — Financing annuity from [`Self::financing_annuity`]
    ///   (currency amount per unit of spread).
    ///
    /// # Returns
    /// Par spread in basis points (`1.0` = 1bp).
    ///
    /// # Errors
    /// Returns `Error::Validation` when `|annuity|` is below
    /// [`super::swap_legs::ANNUITY_EPSILON`] or the result is not finite, and
    /// propagates financing-leg projection errors.
    pub fn par_spread_bp(
        total_return_pv: Money,
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        market: &MarketContext,
        as_of: Date,
        annuity: f64,
    ) -> finstack_quant_core::Result<f64> {
        if annuity.abs() < super::swap_legs::ANNUITY_EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "TRS par spread: financing annuity {annuity:.3e} below ANNUITY_EPSILON \
                 ({:.0e}). Division would amplify rounding noise into the par spread.",
                super::swap_legs::ANNUITY_EPSILON
            )));
        }
        let float_pv = Self::pv_financing_float_only(financing, schedule, notional, market, as_of)?;
        let par_spread = (total_return_pv.amount() - float_pv) / annuity * BASIS_POINTS_PER_UNIT;
        if !par_spread.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Par spread calculation produced invalid value: {par_spread}. \
                 Check that financing annuity is non-zero."
            )));
        }
        Ok(par_spread)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        financing_period_projection, financing_period_rate, TotalReturnLegParams, TrsEngine,
        TrsReturnModel,
    };
    use crate::instruments::pricing::swap_legs;
    use crate::instruments::FinancingLegSpec;
    use crate::instruments::TrsScheduleSpec;
    use finstack_quant_cashflows::builder::FloatingLegCompounding;
    use finstack_quant_cashflows::builder::ScheduleParams;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{
        BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind, Tenor,
    };
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::market_data::traits::Discounting;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;
    use time::Month;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("month"), d).expect("date")
    }

    #[test]
    fn act365l_financing_coupons_preserve_actual_periods_and_the_pv_spread_identity() {
        for (start, end, frequency, stub, denominator) in [
            (
                date(2023, 3, 1),
                date(2024, 3, 1),
                Tenor::annual(),
                StubKind::ShortFront,
                366.0,
            ),
            (
                date(2023, 12, 1),
                date(2024, 3, 1),
                Tenor::quarterly(),
                StubKind::ShortFront,
                366.0,
            ),
            (
                date(2024, 3, 1),
                date(2025, 1, 1),
                Tenor::annual(),
                StubKind::ShortFront,
                365.0,
            ),
            (
                date(2023, 12, 1),
                date(2025, 1, 1),
                Tenor::annual(),
                StubKind::LongFront,
                366.0,
            ),
        ] {
            let as_of = start.previous_day().expect("valuation date");
            let schedule = TrsScheduleSpec::from_params(
                start,
                end,
                ScheduleParams {
                    frequency,
                    day_count: DayCount::Act365L,
                    business_day_convention: BusinessDayConvention::Unadjusted,
                    calendar_id: "weekends_only".into(),
                    stub,
                    end_of_month: false,
                    payment_lag_days: 0,
                    adjust_accrual_dates: false,
                    roll_rule: finstack_quant_cashflows::builder::RollRule::None,
                },
            );
            let market = MarketContext::new()
                .insert(
                    DiscountCurve::builder("DISC")
                        .base_date(as_of)
                        .day_count(DayCount::Act365F)
                        .knots([(0.0, 1.0), (3.0, 1.0)])
                        .build()
                        .expect("flat discount"),
                )
                .insert(
                    // Express the 5% contractual ACT/365L rate on the curve's
                    // ACT/365F basis while retaining the actual coupon tenor.
                    ForwardCurve::builder("TERM", (end - start).whole_days() as f64 / 365.0)
                        .base_date(as_of)
                        .day_count(DayCount::Act365F)
                        .knots([
                            (0.0, 0.05 * 365.0 / denominator),
                            (3.0, 0.05 * 365.0 / denominator),
                        ])
                        .build()
                        .expect("flat forward"),
                );
            let notional = Money::from((1_000_000_i64, Currency::USD));
            let total_fraction = (end - start).whole_days() as f64 / denominator;
            for compounding in [
                FloatingLegCompounding::Simple,
                FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
            ] {
                let financing = FinancingLegSpec {
                    discount_curve_id: CurveId::new("DISC"),
                    forward_curve_id: CurveId::new("TERM"),
                    spread_bp: Decimal::from(100),
                    day_count: DayCount::Act365L,
                    compounding,
                };
                let annuity =
                    TrsEngine::financing_annuity(&financing, &schedule, notional, &market, as_of)
                        .expect("ACT/365L annuity");
                let float_pv = TrsEngine::pv_financing_float_only(
                    &financing, &schedule, notional, &market, as_of,
                )
                .expect("ACT/365L floating PV");
                let total_pv =
                    TrsEngine::pv_financing_leg(&financing, &schedule, notional, &market, as_of)
                        .expect("ACT/365L financing PV");
                let expected_float = if matches!(compounding, FloatingLegCompounding::Simple) {
                    50_000.0 * total_fraction
                } else {
                    let mut factor = 1.0;
                    let mut day = start;
                    while day < end {
                        let next = day
                            .add_weekdays(1)
                            .expect("next overnight interval")
                            .min(end);
                        factor *= 1.0 + 0.05 * (next - day).whole_days() as f64 / denominator;
                        day = next;
                    }
                    1_000_000.0 * (factor - 1.0)
                };
                assert!((annuity / notional.amount() - total_fraction).abs() < 1e-14);
                assert!(
                    (float_pv - expected_float).abs() < 1e-7,
                    "{start} -> {end}, {compounding:?}: expected {expected_float}, got {float_pv}"
                );
                assert!((total_pv.amount() - float_pv - 0.01 * annuity).abs() < 1e-7);
            }
        }
    }

    struct FlatReturnModel {
        rate: f64,
    }

    struct SequencedReturnModel {
        returns: [f64; 3],
        next_idx: std::sync::atomic::AtomicUsize,
    }

    impl TrsReturnModel for FlatReturnModel {
        fn period_return(
            &self,
            _inputs: &super::PeriodReturnInputs,
            _context: &MarketContext,
        ) -> finstack_quant_core::Result<f64> {
            Ok(self.rate)
        }
    }

    impl TrsReturnModel for SequencedReturnModel {
        fn period_return(
            &self,
            _inputs: &super::PeriodReturnInputs,
            _context: &MarketContext,
        ) -> finstack_quant_core::Result<f64> {
            let idx = self.next_idx.load(std::sync::atomic::Ordering::SeqCst);
            self.next_idx
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.returns[idx])
        }
    }

    #[test]
    fn test_trs_annuity_epsilon_is_reasonable() {
        // Verify the threshold catches near-zero but allows reasonable values
        let eps = swap_legs::ANNUITY_EPSILON;
        assert!(eps > 0.0, "ANNUITY_EPSILON should be positive");
        assert!(eps < 1e-10, "ANNUITY_EPSILON should be small");

        // A typical annuity for a 1-year quarterly swap with $1M notional would be
        // roughly 0.25 * 4 * 1M * 0.95 = 950,000, which is well above epsilon
        let typical_annuity = 950_000.0;
        assert!(
            typical_annuity > eps,
            "Typical annuity should be above threshold"
        );
    }

    #[test]
    fn trs_total_return_leg_preserves_small_residual_after_large_cancelling_periods() {
        let as_of = date(2025, 1, 1);
        let end = date(2025, 4, 1);
        let schedule = TrsScheduleSpec::from_params(
            as_of,
            end,
            ScheduleParams {
                frequency: Tenor::monthly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        );

        let disc = DiscountCurve::builder(CurveId::new("DISC"))
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1.0)])
            .build()
            .expect("discount curve");

        let ctx = MarketContext::new().insert(disc);
        let params = TotalReturnLegParams {
            schedule: &schedule,
            notional: Money::from((1_i64, Currency::USD)),
            discount_curve_id: "DISC",
            initial_level: Some(1.0),
        };
        let model = SequencedReturnModel {
            returns: [1e16, 1.0, -1e16],
            next_idx: std::sync::atomic::AtomicUsize::new(0),
        };

        let pv =
            TrsEngine::pv_total_return_leg_with_model(params, &ctx, as_of, &model).expect("pv");

        assert_eq!(pv.amount(), 1.0);
    }

    #[test]
    fn trs_total_return_leg_uses_curve_df_between_dates() {
        let as_of = date(2024, 12, 31);
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let schedule = TrsScheduleSpec::from_params(
            start,
            end,
            ScheduleParams {
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        );

        let disc = DiscountCurve::builder(CurveId::new("DISC"))
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("discount curve");

        let ctx = MarketContext::new().insert(disc.clone());
        let params = TotalReturnLegParams {
            schedule: &schedule,
            notional: Money::from((1_000_000_i64, Currency::USD)),
            discount_curve_id: "DISC",
            initial_level: Some(100.0),
        };
        let model = FlatReturnModel { rate: 0.05 };

        let pv =
            TrsEngine::pv_total_return_leg_with_model(params, &ctx, as_of, &model).expect("pv");

        let period_schedule = schedule.period_schedule().expect("schedule");
        let mut expected = 0.0;
        let mut naive = 0.0;
        let ctx_day_count = DayCountContext::default();

        for i in 1..period_schedule.dates.len() {
            let _period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];
            let df = disc.df_between_dates(as_of, period_end).expect("df");
            let t_end = schedule
                .params
                .day_count
                .year_fraction(as_of, period_end, ctx_day_count)
                .expect("t_end");
            let df_naive = disc.df(t_end);
            let payment = 1_000_000.0 * model.rate;
            expected += payment * df;
            naive += payment * df_naive;
        }

        let diff = (pv.amount() - expected).abs();
        let tol = 1e-8 * 1_000_000.0;
        assert!(diff < tol, "PV should use curve DF: diff={}", diff);
        assert!(
            (expected - naive).abs() > 1e-6,
            "Expected curve-based DF to differ from naive DF"
        );
    }

    #[test]
    fn trs_financing_leg_uses_curve_clock_and_contractual_accrual() {
        let as_of = date(2024, 12, 31);
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let schedule = TrsScheduleSpec::from_params(
            start,
            end,
            ScheduleParams {
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        );

        let disc = DiscountCurve::builder(CurveId::new("DISC"))
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("discount curve");

        let fwd = ForwardCurve::builder(CurveId::new("FWD"), 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (1.0, 0.04)])
            .build()
            .expect("forward curve");

        let ctx = MarketContext::new()
            .insert(disc.clone())
            .insert(fwd.clone());

        let financing = FinancingLegSpec {
            discount_curve_id: CurveId::new("DISC"),
            forward_curve_id: CurveId::new("FWD"),
            spread_bp: Decimal::ZERO,
            day_count: DayCount::Act365F,
            compounding: FloatingLegCompounding::Simple,
        };

        let pv = TrsEngine::pv_financing_leg(
            &financing,
            &schedule,
            Money::from((1_000_000_i64, Currency::USD)),
            &ctx,
            as_of,
        )
        .expect("pv");

        let period_schedule = schedule.period_schedule().expect("schedule");
        let mut expected = 0.0;
        let mut naive = 0.0;
        let mut curve_accrual_pv = 0.0;
        let ctx_day_count = DayCountContext::default();

        for i in 1..period_schedule.dates.len() {
            let period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];
            // Use financing.day_count for accrual (matches engine after fix)
            let yf = financing
                .day_count
                .year_fraction(period_start, period_end, ctx_day_count)
                .expect("yf");
            let growth = fwd.df_on_date(period_start).expect("start projection DF")
                / fwd.df_on_date(period_end).expect("end projection DF")
                - 1.0;
            let df = disc.df_between_dates(as_of, period_end).expect("df");
            expected += 1_000_000.0 * growth * df;
            let curve_accrual = fwd
                .day_count()
                .year_fraction(period_start, period_end, ctx_day_count)
                .expect("curve accrual");
            curve_accrual_pv += 1_000_000.0 * growth / curve_accrual * yf * df;

            let t_start = schedule
                .params
                .day_count
                .year_fraction(as_of, period_start, ctx_day_count)
                .expect("t_start");
            let t_end = schedule
                .params
                .day_count
                .year_fraction(as_of, period_end, ctx_day_count)
                .expect("t_end");
            let fwd_naive = fwd
                .rate_period(t_start, t_end)
                .expect("finite ordered period");
            let df_naive = disc.df(t_end);
            naive += 1_000_000.0 * fwd_naive * yf * df_naive;
        }

        let diff = (pv.amount() - expected).abs();
        let tol = 1e-8 * 1_000_000.0;
        assert!(diff < tol, "PV should use curve time: diff={}", diff);
        assert!(
            (expected - curve_accrual_pv).abs() > 100.0,
            "curve clock must not replace contractual coupon accrual"
        );
        assert!(
            (expected - naive).abs() > 1e-6,
            "Expected curve-based forward/DF to differ from naive time mapping"
        );
    }

    /// Regression test: an in-progress compounded financing period that
    /// straddles `as_of` must splice realized daily fixings (period_start → as_of)
    /// with projected forwards (as_of → period_end), NOT project the whole period
    /// from the forward curve. This test verifies that the spliced result differs
    /// from the all-projected result, and matches a hand-computed expected value.
    #[test]
    fn trs_financing_leg_ois_in_progress_splices_realized_and_projected() {
        // Period: 2025-01-06 (Mon) → 2025-02-03 (Mon), 28 days. as_of = 2025-01-20 (Mon, mid-period).
        // Both endpoints are Mondays so ModifiedFollowing leaves them unchanged.
        // Realized fixings for Jan 6..17 (2 full weeks), projected from Jan 20 onward.
        let period_start = date(2025, 1, 6);
        let period_end = date(2025, 2, 3);
        let as_of = date(2025, 1, 20);

        // Flat OIS forward at 5 % (overnight tenor ~ 1/365).
        let fwd = ForwardCurve::builder(CurveId::new("SOFR"), 1.0 / 365.0)
            .base_date(period_start)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.05), (1.0, 0.05)])
            .build()
            .expect("forward curve");

        // Realized fixings at 4 % for every weekday in Jan 01..15.
        // (Weekday-only grid: Jan 1 is Wednesday; we enumerate Jan 1–15 weekdays.)
        let realized_dates: Vec<Date> = {
            let mut ds = Vec::new();
            let mut d = period_start;
            while d < as_of {
                let wd = d.weekday();
                use time::Weekday;
                if !matches!(wd, Weekday::Saturday | Weekday::Sunday) {
                    ds.push(d);
                }
                d = d.next_day().expect("next day");
            }
            ds
        };
        let realized_obs: Vec<(Date, f64)> = realized_dates.iter().map(|&d| (d, 0.04)).collect();
        let fixing_series =
            ScalarTimeSeries::new("FIXING:SOFR", realized_obs, None).expect("fixing series");

        let disc = DiscountCurve::builder(CurveId::new("DISC"))
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (1.0, 1.0)])
            .build()
            .expect("discount curve");

        let ctx = MarketContext::new()
            .insert(disc)
            .insert(fwd.clone())
            .insert_series(fixing_series);

        // TRS with a single monthly period (period_start → period_end).
        // Use StubKind::Short so ScheduleBuilder does not require an exact integer
        // multiple of the tenor between start and end dates.
        let schedule = TrsScheduleSpec::from_params(
            period_start,
            period_end,
            ScheduleParams {
                frequency: Tenor::monthly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".into(),
                stub: StubKind::ShortBack,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        );

        let financing = FinancingLegSpec {
            discount_curve_id: CurveId::new("DISC"),
            forward_curve_id: CurveId::new("SOFR"),
            spread_bp: Decimal::ZERO,
            day_count: DayCount::Act360,
            compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        };

        let notional = Money::from((1_000_000_i64, Currency::USD));
        let pv_spliced = TrsEngine::pv_financing_leg(&financing, &schedule, notional, &ctx, as_of)
            .expect("pv_spliced");

        // Compute expected spliced compound independently:
        // realized sub-period: weekdays in [Jan 6, Jan 17], each at 4 %
        // projected sub-period: weekdays in [Jan 20, Feb 3), each at 5 %
        let ctx_day_count = DayCountContext::default();
        use time::Weekday;
        let mut cf = 1.0_f64;
        let mut d = period_start;
        while d < period_end {
            let next_d = {
                // Step one weekday forward (weekends_only calendar → skip Sat/Sun)
                let mut nd = d.next_day().expect("next");
                while matches!(nd.weekday(), Weekday::Saturday | Weekday::Sunday) {
                    nd = nd.next_day().expect("next");
                }
                nd.min(period_end)
            };
            let dcf = DayCount::Act360
                .year_fraction(d, next_d, ctx_day_count)
                .expect("dcf");
            let r = if d < as_of { 0.04 } else { 0.05 };
            cf *= 1.0 + r * dcf;
            d = next_d;
        }
        let yf = DayCount::Act360
            .year_fraction(period_start, period_end, ctx_day_count)
            .expect("yf");
        let expected_rate = (cf - 1.0) / yf;
        let expected_pv = 1_000_000.0 * expected_rate * yf; // df = 1

        // Compute the all-projected value (the buggy result before the fix).
        let ctx_no_fixings = MarketContext::new()
            .insert(
                DiscountCurve::builder(CurveId::new("DISC"))
                    .base_date(as_of)
                    .day_count(DayCount::Act360)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .build()
                    .expect("disc no_fixings"),
            )
            .insert(fwd);
        let pv_all_projected =
            TrsEngine::pv_financing_leg(&financing, &schedule, notional, &ctx_no_fixings, as_of);
        // Before the fix, this would succeed (no fixings required) and give a
        // different value; after the fix it must fail (fixings missing for an
        // in-progress period) OR we simply verify that pv_spliced matches expected.

        // The spliced PV must match the hand-computed expected value.
        let diff = (pv_spliced.amount() - expected_pv).abs();
        assert!(
            diff < 1.0, // within $1 on $1M notional
            "Spliced PV ({:.2}) must match expected ({:.2}); diff = {:.4}",
            pv_spliced.amount(),
            expected_pv,
            diff
        );

        // The all-projected path (no fixings) must error after the fix, because
        // a missing realized fixing for an in-progress OIS period is a hard error.
        assert!(
            pv_all_projected.is_err(),
            "In-progress OIS financing period without fixings must error after the fix"
        );

        // The spliced result must differ materially from the all-projected-at-5%
        // result (which would be CF_5pct - 1 normalized, i.e. biased high since
        // the realized sub-period was actually at 4%).
        let mut cf_all5 = 1.0_f64;
        let mut d2 = period_start;
        while d2 < period_end {
            let next_d = {
                let mut nd = d2.next_day().expect("next");
                while matches!(nd.weekday(), Weekday::Saturday | Weekday::Sunday) {
                    nd = nd.next_day().expect("next");
                }
                nd.min(period_end)
            };
            let dcf = DayCount::Act360
                .year_fraction(d2, next_d, ctx_day_count)
                .expect("dcf");
            cf_all5 *= 1.0 + 0.05 * dcf;
            d2 = next_d;
        }
        let rate_all5 = (cf_all5 - 1.0) / yf;
        let pv_all5 = 1_000_000.0 * rate_all5 * yf;
        assert!(
            (pv_spliced.amount() - pv_all5).abs() > 100.0,
            "Spliced PV ({:.2}) must differ materially from all-5% projected ({:.2})",
            pv_spliced.amount(),
            pv_all5
        );
    }

    #[test]
    fn trs_term_rate_started_period_requires_exact_reset_fixing() {
        let period_start = date(2025, 1, 6);
        let period_end = date(2025, 4, 7);
        let as_of = date(2025, 2, 3);
        let fwd = ForwardCurve::builder("USD-TERM-3M", 0.25)
            .base_date(period_start)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.05), (1.0, 0.05)])
            .build()
            .expect("forward curve");
        let financing = FinancingLegSpec {
            discount_curve_id: CurveId::new("DISC"),
            forward_curve_id: CurveId::new("USD-TERM-3M"),
            spread_bp: Decimal::ZERO,
            day_count: DayCount::Act360,
            compounding: FloatingLegCompounding::Simple,
        };
        let year_fraction = DayCount::Act360
            .year_fraction(period_start, period_end, DayCountContext::default())
            .expect("year fraction");

        let missing = financing_period_rate(
            &financing,
            &fwd,
            None,
            period_start,
            period_end,
            year_fraction,
            Tenor::quarterly(),
            as_of,
            finstack_quant_core::dates::WEEKENDS_ONLY_ID,
            Currency::USD,
        )
        .expect_err("started term period without fixing must fail");
        assert!(missing.to_string().contains("FIXING:USD-TERM-3M"));

        let fixings =
            ScalarTimeSeries::new("FIXING:USD-TERM-3M", vec![(period_start, 0.041)], None)
                .expect("fixing series");
        let observed = financing_period_rate(
            &financing,
            &fwd,
            Some(&fixings),
            period_start,
            period_end,
            year_fraction,
            Tenor::quarterly(),
            as_of,
            finstack_quant_core::dates::WEEKENDS_ONLY_ID,
            Currency::USD,
        )
        .expect("exact reset fixing");
        assert!((observed - 0.041).abs() < 1e-14);

        let future = financing_period_rate(
            &financing,
            &fwd,
            None,
            period_end,
            period_end.add_months(3).expect("valid date shift"),
            year_fraction,
            Tenor::quarterly(),
            as_of,
            finstack_quant_core::dates::WEEKENDS_ONLY_ID,
            Currency::USD,
        )
        .expect("future term period projects");
        assert!(future.is_finite());
    }

    #[test]
    fn trs_term_rate_period_resetting_on_valuation_date_uses_published_fixing_or_projects() {
        let as_of = date(2025, 1, 6);
        let period_end = date(2025, 4, 7);
        let fwd = ForwardCurve::builder("USD-TERM-3M", 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.05), (1.0, 0.05)])
            .build()
            .expect("forward curve");
        let financing = FinancingLegSpec {
            discount_curve_id: CurveId::new("DISC"),
            forward_curve_id: CurveId::new("USD-TERM-3M"),
            spread_bp: Decimal::ZERO,
            day_count: DayCount::Act360,
            compounding: FloatingLegCompounding::Simple,
        };
        let year_fraction = DayCount::Act360
            .year_fraction(as_of, period_end, DayCountContext::default())
            .expect("year fraction");
        let project = |fixings: Option<&ScalarTimeSeries>| {
            financing_period_projection(
                &financing,
                &fwd,
                fixings,
                as_of,
                period_end,
                year_fraction,
                Tenor::quarterly(),
                as_of,
                finstack_quant_core::dates::WEEKENDS_ONLY_ID,
                BusinessDayConvention::ModifiedFollowing,
                Currency::USD,
                true,
            )
            .expect("projection")
        };

        let published = ScalarTimeSeries::new("FIXING:USD-TERM-3M", vec![(as_of, 0.041)], None)
            .expect("fixing series");
        let fixed = project(Some(&published));
        assert!((fixed.rate - 0.041).abs() < 1e-14);
        assert!(
            fixed.fixings.is_empty(),
            "a published fixing is not an exposure"
        );

        let projected = project(None);
        let expected = fwd
            .rate_between_dates(as_of, period_end, year_fraction)
            .expect("forward rate");
        assert!((projected.rate - expected).abs() < 1e-14);
        assert_eq!(projected.fixings.len(), 1);
        assert_eq!(projected.fixings[0].date, as_of);
    }

    #[test]
    fn trs_financing_leg_overnight_compounding_exceeds_term_rate_on_upward_curve() {
        // On an upward-sloping forward curve, daily-compounding an OIS financing
        // leg picks up positive convexity that the simple term-rate average
        // drops. A `simple` and a compounded leg over the same
        // curves must therefore price differently, with OIS strictly higher.
        let as_of = date(2024, 12, 31);
        let start = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let schedule = TrsScheduleSpec::from_params(
            start,
            end,
            ScheduleParams {
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        );

        let disc = DiscountCurve::builder(CurveId::new("DISC"))
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("discount curve");

        // Steep upward-sloping overnight forward: 3% -> 6% over the year.
        let fwd = ForwardCurve::builder(CurveId::new("FWD"), 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (1.0, 0.06)])
            .build()
            .expect("forward curve");

        let ctx = MarketContext::new().insert(disc).insert(fwd);
        let notional = Money::from((1_000_000_i64, Currency::USD));

        let term = FinancingLegSpec::new("DISC", "FWD", Decimal::ZERO, DayCount::Act365F);
        let ois = term
            .clone()
            .with_compounding(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 });

        let pv_term = TrsEngine::pv_financing_leg(&term, &schedule, notional, &ctx, as_of)
            .expect("term-rate financing pv");
        let pv_ois = TrsEngine::pv_financing_leg(&ois, &schedule, notional, &ctx, as_of)
            .expect("OIS financing pv");

        assert!(
            pv_ois.amount() > pv_term.amount(),
            "OIS compounding must exceed term-rate on an upward curve: ois={}, term={}",
            pv_ois.amount(),
            pv_term.amount()
        );

        // The convexity gap should be material (a fraction of a bp up to a few
        // bp of notional) but not absurd — guards against a degenerate
        // near-zero pass or a units blunder.
        let gap_bp = (pv_ois.amount() - pv_term.amount()) / notional.amount() * 10_000.0;
        assert!(
            (0.1..50.0).contains(&gap_bp),
            "OIS-vs-term convexity gap {gap_bp} bp is outside the expected range"
        );
    }

    #[test]
    fn overnight_compounded_matches_shared_overnight_engine() {
        use crate::instruments::pricing::overnight::{
            project_overnight_coupon, OvernightCouponProjectionInput, OvernightProjectionCurve,
        };
        use finstack_quant_cashflows::builder::FloatingLegCompounding;
        use finstack_quant_core::dates::calendar_by_id_strict;

        let start = date(2025, 1, 2);
        let end = date(2025, 4, 2);
        let fwd = ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 365.0)
            .base_date(start)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.04), (1.0, 0.04)])
            .build()
            .expect("forward");
        let financing = FinancingLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR-OIS"),
            spread_bp: Decimal::ZERO,
            day_count: DayCount::Act360,
            compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        };
        let yf = DayCount::Act360
            .year_fraction(start, end, DayCountContext::default())
            .expect("yf");
        let actual = financing_period_rate(
            &financing,
            &fwd,
            None,
            start,
            end,
            yf,
            Tenor::quarterly(),
            start,
            "usny",
            Currency::USD,
        )
        .expect("trs overnight");
        let calendar = calendar_by_id_strict("usny").expect("usny");
        let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
        let expected = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&fwd),
            fixings: None,
            fixing_id: "USD-SOFR-OIS",
            as_of: start,
            accrual_start: start,
            accrual_end: end,
            day_count: DayCount::Act360,
            coupon_frequency: None,
            coupon_period: (start, end),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: false,
        })
        .expect("shared engine");
        assert!(
            (actual - expected.rate).abs() < 1e-12,
            "TRS overnight {actual} != shared engine {}",
            expected.rate
        );
        let projected = financing_period_projection(
            &financing,
            &fwd,
            None,
            start,
            end,
            0.50,
            Tenor::quarterly(),
            start,
            "usny",
            BusinessDayConvention::ModifiedFollowing,
            Currency::USD,
            false,
        )
        .expect("trs overnight projection");
        let amount = projected.unsigned_coupon(1_000_000.0, 0.0);
        let expected_amount = 1_000_000.0 * (expected.compound_factor - 1.0);
        assert!(
            (amount - expected_amount).abs() < 1e-6,
            "TRS overnight amount {amount} != N*(CF-1) {expected_amount}"
        );
        let wrong = actual * 1_000_000.0 * 0.50;
        assert!(
            (amount - wrong).abs() > 1.0,
            "TRS overnight coupon must not use the unadjusted schedule year fraction"
        );
    }
}
