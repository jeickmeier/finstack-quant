//! Shared Total Return Swap (TRS) pricing engine implementation.

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
    OvernightCouponProjectionInput, OvernightProjectionCurve, adjust_overnight_accrual_boundaries,
    project_overnight_coupon,
};
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_cashflows::fixings::ProjectedFixing;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::fixings::fixing_series_id;

/// Financing-leg `(payment_date, amount)` flows and the `FIXING:` index
/// observations they project (see [`TrsEngine::cashflows_financing_leg_with_fixings`]).
pub type FinancingLegFlows = (Vec<(Date, Money)>, Vec<ProjectedFixing>);

/// Projected TRS financing coupon for one accrual window.
pub(super) struct FinancingPeriodProjection {
    /// Index rate excluding spread.
    pub(super) rate: f64,
    /// Accrual fraction used for the coupon amount.
    pub(super) year_fraction: f64,
    /// Compound factor `∏(1 + rᵢ dᵢ)` or `1 + rate × τ` for term financing.
    pub(super) compound_factor: f64,
    /// Raw index observations the projection used, when requested.
    pub(super) fixings: Vec<ProjectedFixing>,
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

    pub(super) fn unsigned_coupon(&self, notional: f64, spread: f64) -> f64 {
        notional * (self.compound_factor - 1.0) + notional * spread * self.year_fraction
    }
}

/// Period-invariant inputs of [`financing_period_projection`] for one
/// financing leg.
pub(super) struct FinancingLegContext<'a> {
    /// Financing leg terms: forward curve id, day count and compounding.
    pub(super) financing: &'a FinancingLegSpec,
    /// Forward curve projecting the leg's rate.
    pub(super) fwd: &'a ForwardCurve,
    /// Published `FIXING:{forward_curve_id}` series, when the market has one.
    pub(super) fixings: Option<&'a ScalarTimeSeries>,
    /// Coupon frequency of the financing schedule.
    pub(super) coupon_frequency: Tenor,
    /// Valuation date splitting realized from projected fixings.
    pub(super) as_of: Date,
    /// Overnight fixing calendar id; empty resolves the currency default.
    pub(super) calendar_id: &'a str,
    /// Business-day convention for overnight accrual boundaries.
    pub(super) accrual_bdc: BusinessDayConvention,
    /// Leg currency, used to resolve the default fixing calendar.
    pub(super) currency: Currency,
    /// Whether to report projected fixings as exposures.
    pub(super) collect_fixings: bool,
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
///
/// `period_year_fraction` is the period's accrual on the financing leg's day
/// count; every other input is the period-invariant `leg` context.
pub(super) fn financing_period_projection(
    leg: &FinancingLegContext<'_>,
    period_start: Date,
    period_end: Date,
    period_year_fraction: f64,
) -> finstack_quant_core::Result<FinancingPeriodProjection> {
    let &FinancingLegContext {
        financing,
        fwd,
        fixings,
        coupon_frequency,
        as_of,
        calendar_id,
        accrual_bdc,
        currency,
        collect_fixings,
    } = leg;
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
                    )?,
                )
            } else if period_start == as_of {
                match fixings {
                    Some(series) => match series.get_value_exact(period_start) {
                        Ok(rate) => Some(rate),
                        Err(err) if err.kind() == finstack_quant_core::ErrorKind::NotFound => None,
                        Err(err) => return Err(err),
                    },
                    None => None,
                }
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
                    crate::instruments::pricing::overnight::resolve_overnight_fixing_calendar(
                        None,
                        currency,
                        "TRS financing",
                    )?
                } else if calendar_id == finstack_quant_core::dates::WEEKENDS_ONLY_ID {
                    &finstack_quant_core::dates::WEEKENDS_ONLY
                } else {
                    crate::instruments::pricing::overnight::resolve_overnight_fixing_calendar(
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
                end_is_termination_date: None,
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
                .projected_fixings(financing.forward_curve_id.as_str())
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
    let index_id = finstack_quant_core::market_data::fixings::validate_fixing_index_id(
        financing.forward_curve_id.as_str(),
    )?;
    let fixing_id = fixing_series_id(index_id);
    let fixings = match market.get_series(&fixing_id) {
        Ok(series) => Some(series),
        Err(err) if err.kind() == finstack_quant_core::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    let period_schedule = schedule.period_schedule()?;
    // Overnight accrual boundaries always land on business days; unadjusted
    // schedules fall back to Modified Following for that adjustment.
    let accrual_bdc = if schedule.params.adjust_accrual_dates {
        schedule.params.business_day_convention
    } else {
        BusinessDayConvention::ModifiedFollowing
    };
    let leg = FinancingLegContext {
        financing,
        fwd: fwd.as_ref(),
        fixings,
        coupon_frequency: schedule.params.frequency,
        as_of,
        calendar_id: schedule.params.calendar_id.as_str(),
        accrual_bdc,
        currency,
        collect_fixings,
    };

    for i in 1..period_schedule.get_dates().len() {
        let period_start = period_schedule.get_dates()[i - 1];
        let period_end = period_schedule.get_dates()[i];
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
        let projected = financing_period_projection(&leg, period_start, period_end, yf)?;
        visit(payment_date, projected)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn financing_period_rate(
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
    let leg = FinancingLegContext {
        financing,
        fwd,
        fixings,
        coupon_frequency,
        as_of,
        calendar_id,
        accrual_bdc: BusinessDayConvention::ModifiedFollowing,
        currency,
        collect_fixings: false,
    };
    Ok(financing_period_projection(&leg, period_start, period_end, period_year_fraction)?.rate)
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
    /// * `inputs` — Valuation date, period dates, and schedule year fractions
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
        for_each_total_return_period(params, market, as_of, model, |date, payment| {
            pv.add(payment * disc.as_ref().df_between_dates(as_of, date)?)
        })?;
        Money::new(pv.get_total()?, currency)
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
    ///   currency of every flow), and discount curve id
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
        let currency = params.notional.currency();
        let mut flows = Vec::new();
        for_each_total_return_period(params, market, as_of, model, |payment_date, payment| {
            flows.push((payment_date, Money::new(payment, currency)?));
            Ok(())
        })?;
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
        let spread_decimal = financing_spread_decimal(financing)?;
        let disc = market.get_discount(financing.discount_curve_id.as_str())?;
        let currency = notional.currency();
        let mut pv = NeumaierAccumulator::new();
        for_each_financing_period(
            financing,
            schedule,
            market,
            as_of,
            currency,
            false,
            |payment_date, projected| {
                let amount = projected.unsigned_coupon(notional.amount(), spread_decimal);
                // A completed accrual remains in PV until its lagged payment date.
                pv.add(amount * disc.as_ref().df_between_dates(as_of, payment_date)?)
            },
        )?;
        Money::new(pv.get_total()?, currency)
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
    /// The `(payment_date, amount)` flows and the `FIXING:{forward_curve_id}`
    /// observations projected at `as_of`: each future term reset for `simple`
    /// legs, each overnight observation for compounded legs.
    pub fn cashflows_financing_leg_with_fixings(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<FinancingLegFlows> {
        let spread_decimal = financing_spread_decimal(financing)?;
        let currency = notional.currency();
        let mut flows = Vec::new();
        let mut projected_fixings = Vec::new();
        for_each_financing_period(
            financing,
            schedule,
            market,
            as_of,
            currency,
            true,
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
    /// [`crate::constants::ANNUITY_EPSILON`] (1e-12),
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
                annuity.add(df * projected.year_fraction)?;
                Ok(())
            },
        )?;

        let result = annuity.get_total()? * notional.amount();

        if result.abs() < crate::constants::ANNUITY_EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Financing annuity ({:.2e}) is below minimum threshold ({:.2e}). \
                 This may indicate all periods have expired or extreme discounting scenarios. \
                 Cannot compute par spread with near-zero annuity.",
                result,
                crate::constants::ANNUITY_EPSILON
            )));
        }

        Ok(result)
    }

    pub(super) fn pv_financing_float_only(
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
                total_pv.add(payment * df)?;
                Ok(())
            },
        )?;

        total_pv.get_total()
    }

    /// Par financing spread in basis points: the spread that zeroes the
    /// total-return receiver's NPV.
    ///
    /// ```text
    /// NPV = PV(total_return) - PV(financing_float) - s * Annuity = 0
    /// s_par = (PV(total_return) - PV(financing_float)) / Annuity
    /// ```
    ///
    /// `PV(financing_float)` is the spread-free financing leg PV. The result
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
    /// Returns `Error::CurrencyMismatch` when `total_return_pv.currency() != notional.currency()`,
    /// `Error::Validation` when `|annuity|` is below
    /// [`crate::constants::ANNUITY_EPSILON`] or the result is not finite, and
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
        if total_return_pv.currency() != notional.currency() {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: notional.currency(),
                actual: total_return_pv.currency(),
            });
        }
        if annuity.abs() < crate::constants::ANNUITY_EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "TRS par spread: financing annuity {annuity:.3e} below ANNUITY_EPSILON \
                 ({:.0e}). Division would amplify rounding noise into the par spread.",
                crate::constants::ANNUITY_EPSILON
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

fn financing_spread_decimal(financing: &FinancingLegSpec) -> finstack_quant_core::Result<f64> {
    Ok(financing.spread_bp.to_f64().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "TRS financing spread_bp ({}) is not representable as f64",
            financing.spread_bp
        ))
    })? / 10_000.0)
}

fn for_each_total_return_period(
    params: TotalReturnLegParams<'_>,
    market: &MarketContext,
    as_of: Date,
    model: &impl TrsReturnModel,
    mut visit: impl FnMut(Date, f64) -> finstack_quant_core::Result<()>,
) -> finstack_quant_core::Result<()> {
    let period_schedule = params.schedule.period_schedule()?;
    for i in 1..period_schedule.get_dates().len() {
        let period_start = period_schedule.get_dates()[i - 1];
        let period_end = period_schedule.get_dates()[i];
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
        visit(payment_date, payment)?;
    }
    Ok(())
}
