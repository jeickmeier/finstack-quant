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
//! `FinancingRateCompounding` setting:
//!
//! - `TermRate` — the **simple arithmetic-average forward** over the accrual
//!   period ([`rate_period_on_dates`]), correct for a term-rate-financed TRS
//!   (e.g. 3M Term SOFR) where the period length matches the index tenor.
//! - `OvernightCompounded` — **daily-compounds** the overnight forward via
//!   `swap_legs::compounded_forward_projection`, `(∏(1+rᵢ·dᵢ)−1)/τ`, correct
//!   for an overnight-indexed (OIS / RFR) financing leg (SOFR/SONIA/€STR). The
//!   simple average would drop the daily-compounding convexity (~12–15 bp of
//!   rate at current levels).
//!
//! [`TrsEngine::financing_annuity`] projects no rate (it sums discounted year
//! fractions only) and is therefore independent of the compounding choice.

use crate::instruments::common_impl::parameters::legs::{
    FinancingLegSpec, FinancingRateCompounding,
};
use crate::instruments::common_impl::parameters::trs_common::TrsScheduleSpec;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::math::NeumaierAccumulator;
use finstack_core::money::Money;
use rust_decimal::prelude::ToPrimitive;

use crate::instruments::common_impl::pricing::time::{
    rate_period_on_dates, relative_df_discount_curve,
};

/// Project one financing-leg accrual period's floating rate, excluding spread.
///
/// `TermRate` legs use the simple arithmetic-average forward over the period;
/// `OvernightCompounded` (OIS / RFR) legs daily-compound the overnight forward
/// and return the equivalent simple rate `(∏(1+rᵢ·dᵢ)−1)/τ`, capturing the
/// daily-compounding convexity the arithmetic average drops.
///
/// For `OvernightCompounded` periods that have already started
/// (`period_start <= as_of < period_end`), the function splices realized daily
/// fixings (from `fixings`) with projected overnight forwards, matching the
/// behaviour of `compounded_spliced_projection` used by `pv_floating_leg`.
/// Fully-future periods (`period_start > as_of`) are projected entirely from the
/// forward curve via `compounded_forward_projection`. A missing realized fixing
/// for an in-progress period is a hard error (not a silent projection).
#[allow(clippy::too_many_arguments)]
fn financing_period_rate(
    financing: &FinancingLegSpec,
    fwd: &ForwardCurve,
    fixings: Option<&ScalarTimeSeries>,
    period_start: Date,
    period_end: Date,
    period_year_fraction: f64,
    as_of: Date,
    calendar_id: &str,
) -> finstack_core::Result<f64> {
    match financing.compounding {
        FinancingRateCompounding::TermRate => rate_period_on_dates(fwd, period_start, period_end),
        FinancingRateCompounding::OvernightCompounded => {
            // The daily-compounding observation grid needs either a registered
            // holiday calendar or `None` (weekday-only stepping). The
            // weekends-only sentinel and an empty id both mean weekday-only,
            // which the compounded helpers express as `None`.
            let obs_calendar = match calendar_id {
                "" => None,
                id if id == crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID => None,
                id => Some(id),
            };

            if period_start <= as_of {
                // In-progress (or fully-accrued-but-unpaid) period: splice realized
                // daily fixings with projected forwards.  A missing fixing is a hard
                // error — consistent with `pv_floating_leg` / `swap_legs`.
                super::swap_legs::compounded_spliced_projection(
                    fwd,
                    fixings,
                    fwd.id().as_str(),
                    period_start,
                    period_end,
                    as_of,
                    period_year_fraction,
                    0, // no observation lookback modelled for the TRS funding leg
                    obs_calendar,
                    None,
                    None,
                )
            } else {
                // Fully-future period: project entirely from the forward curve.
                super::swap_legs::compounded_forward_projection(
                    fwd,
                    period_start,
                    period_end,
                    period_year_fraction,
                    0, // no observation lookback modelled for the TRS funding leg
                    obs_calendar,
                    None,
                    None,
                )
            }
        }
    }
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
    /// Contract size multiplier for the underlying.
    pub contract_size: f64,
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
///
/// ```ignore
/// impl TrsReturnModel for EquityReturn {
///     fn period_return(
///         &self,
///         inputs: &PeriodReturnInputs,
///         context: &MarketContext,
///     ) -> finstack_core::Result<f64> {
///         let start_price = context.get_equity_spot(self.ticker, inputs.t_start)?;
///         let end_price = context.get_equity_spot(self.ticker, inputs.t_end)?;
///
///         // Return as decimal (e.g., 0.05 for 5% return)
///         let ret = (end_price - start_price) / initial_level;
///
///         // Validate return is reasonable
///         if !ret.is_finite() {
///             return Err(Error::Validation("Non-finite return".into()));
///         }
///         Ok(ret)
///     }
/// }
/// ```
pub trait TrsReturnModel {
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
    ) -> finstack_core::Result<f64>;
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
    /// * `context` — Market context containing curves and market data
    /// * `as_of` — Valuation date
    /// * `model` — Model implementing TrsReturnModel for underlying-specific logic
    ///
    /// # Returns
    /// Present value of the total return leg in the instrument's currency.
    pub fn pv_total_return_leg_with_model(
        params: TotalReturnLegParams,
        context: &MarketContext,
        as_of: Date,
        model: &impl TrsReturnModel,
    ) -> finstack_core::Result<Money> {
        if params.schedule.end <= as_of {
            return Err(finstack_core::Error::Validation(
                "TRS maturity must be after valuation date".to_string(),
            ));
        }

        let disc = context.get_discount(params.discount_curve_id)?;
        let period_schedule = params.schedule.period_schedule()?;

        let mut total_pv = NeumaierAccumulator::new();
        let currency = params.notional.currency();
        let ctx = DayCountContext::default();

        for i in 1..period_schedule.dates.len() {
            let period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];

            if period_end <= as_of {
                continue;
            }

            // Signed time to the period start: negative when the period is
            // already in progress (seasoned trade). Day counts reject inverted
            // date ranges, so compute the magnitude forward and negate.
            let t_start = if period_start >= as_of {
                params
                    .schedule
                    .params
                    .dc
                    .year_fraction(as_of, period_start, ctx)?
            } else {
                -params
                    .schedule
                    .params
                    .dc
                    .year_fraction(period_start, as_of, ctx)?
            };
            let t_end = params
                .schedule
                .params
                .dc
                .year_fraction(as_of, period_end, ctx)?;

            let total_return = model.period_return(
                &PeriodReturnInputs {
                    as_of,
                    period_start,
                    period_end,
                    t_start,
                    t_end,
                    initial_level: params.initial_level.unwrap_or(1.0),
                },
                context,
            )?;

            if !total_return.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "TRS return model produced non-finite return ({}) for period {} to {}",
                    total_return, period_start, period_end
                )));
            }

            let payment = params.notional.amount() * total_return * params.contract_size;

            // Discount to payment date (accrual end + payment lag).
            // The full-period payment already captures accrued value; discounting
            // the entire cashflow to as_of gives the correct dirty PV without
            // any separate accrual addition.
            let payment_date = params.schedule.payment_date_for(period_end)?;
            let df = relative_df_discount_curve(disc.as_ref(), as_of, payment_date)?;
            total_pv.add(payment * df);
        }

        Ok(Money::new(total_pv.total(), currency))
    }

    /// Calculates the present value of the financing leg.
    ///
    /// This is shared by both equity and fixed income TRS.
    ///
    /// # Arguments
    /// * `financing` — Financing leg specification
    /// * `schedule` — Schedule specification for payment periods
    /// * `notional` — Notional amount for the leg
    /// * `context` — Market context containing curves and market data
    /// * `as_of` — Valuation date
    ///
    /// # Returns
    /// Present value of the financing leg in the instrument's currency.
    pub fn pv_financing_leg(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Money> {
        if schedule.end <= as_of {
            return Err(finstack_core::Error::Validation(
                "TRS maturity must be after valuation date".to_string(),
            ));
        }

        let disc = context.get_discount(financing.discount_curve_id.as_str())?;
        let fwd = context.get_forward(financing.forward_curve_id.as_str())?;
        // For OvernightCompounded legs, realized fixings for in-progress periods
        // are sourced from MarketContext using the canonical `FIXING:{forward_curve_id}`
        // key.  The same pattern is used by `basis_swap` / `pv_floating_leg`.
        // `get_fixing_series` returns `None` (not an error) when absent; the
        // error is deferred to `financing_period_rate` when a fixing is actually
        // required for an in-progress period.
        let fixings = finstack_core::market_data::fixings::get_fixing_series(
            context,
            financing.forward_curve_id.as_str(),
        )
        .ok();
        let period_schedule = schedule.period_schedule()?;

        let mut total_pv = NeumaierAccumulator::new();
        let currency = notional.currency();
        let ctx = DayCountContext::default();
        let spread_decimal = financing.spread_bp.to_f64().ok_or_else(|| {
            finstack_core::Error::Validation(format!(
                "TRS financing spread_bp ({}) is not representable as f64",
                financing.spread_bp
            ))
        })? / 10_000.0;

        for i in 1..period_schedule.dates.len() {
            let period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];

            if period_end <= as_of {
                continue;
            }

            // Use the financing leg's day count for accrual (not the schedule DC
            // which governs date generation).
            let yf = financing
                .day_count
                .year_fraction(period_start, period_end, ctx)?;

            // Project the period rate per the leg's compounding convention
            // (simple term-rate average vs daily-compounded OIS).
            // For in-progress OIS periods, splices realized fixings with projected
            // forwards; fully-future periods use forward-curve projection only.
            let fwd_rate = financing_period_rate(
                financing,
                fwd.as_ref(),
                fixings,
                period_start,
                period_end,
                yf,
                as_of,
                schedule.params.calendar_id.as_str(),
            )?;
            let total_rate = fwd_rate + spread_decimal;
            let payment = notional.amount() * total_rate * yf;

            // Discount to payment date (accrual end + payment lag).
            // The full-period payment already captures accrued value; discounting
            // the entire cashflow to as_of gives the correct dirty PV.
            let payment_date = schedule.payment_date_for(period_end)?;
            let df = relative_df_discount_curve(disc.as_ref(), as_of, payment_date)?;
            total_pv.add(payment * df);
        }

        Ok(Money::new(total_pv.total(), currency))
    }

    /// Calculates the financing annuity for par spread calculation.
    ///
    /// # Arguments
    /// * `financing` — Financing leg specification
    /// * `schedule` — Schedule specification for payment periods
    /// * `notional` — Notional amount for the leg
    /// * `context` — Market context containing curves and market data
    /// * `as_of` — Valuation date
    ///
    /// # Returns
    /// Financing annuity (sum of discounted year fractions × notional).
    ///
    /// # Errors
    ///
    /// Returns an error if the computed annuity is below
    /// [`crate::instruments::common_impl::pricing::swap_legs::ANNUITY_EPSILON`] (1e-12),
    /// which would cause divide-by-zero in downstream par spread calculations.
    /// This typically occurs when:
    /// - All periods have already expired (payment dates before as_of)
    /// - Extreme discounting scenarios with very high rates
    pub fn financing_annuity(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<f64> {
        if schedule.end <= as_of {
            return Err(finstack_core::Error::Validation(
                "TRS maturity must be after valuation date".to_string(),
            ));
        }

        let disc = context.get_discount(financing.discount_curve_id.as_str())?;
        let period_schedule = schedule.period_schedule()?;

        let mut annuity = NeumaierAccumulator::new();
        let ctx = DayCountContext::default();

        for i in 1..period_schedule.dates.len() {
            let period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];

            if period_end <= as_of {
                continue;
            }

            // Use the financing leg's day count for accrual year fraction.
            let yf = financing
                .day_count
                .year_fraction(period_start, period_end, ctx)?;

            // Discount to payment date (accrual end + payment lag).
            let payment_date = schedule.payment_date_for(period_end)?;
            let df = relative_df_discount_curve(disc.as_ref(), as_of, payment_date)?;

            annuity.add(df * yf);
        }

        let result = annuity.total() * notional.amount();

        if result.abs() < super::swap_legs::ANNUITY_EPSILON {
            return Err(finstack_core::Error::Validation(format!(
                "Financing annuity ({:.2e}) is below minimum threshold ({:.2e}). \
                 This may indicate all periods have expired or extreme discounting scenarios. \
                 Cannot compute par spread with near-zero annuity.",
                result,
                super::swap_legs::ANNUITY_EPSILON
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
    pub fn pv_financing_float_only(
        financing: &FinancingLegSpec,
        schedule: &TrsScheduleSpec,
        notional: Money,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<f64> {
        if schedule.end <= as_of {
            return Err(finstack_core::Error::Validation(
                "TRS maturity must be after valuation date".to_string(),
            ));
        }

        let disc = context.get_discount(financing.discount_curve_id.as_str())?;
        let fwd = context.get_forward(financing.forward_curve_id.as_str())?;
        let fixings = finstack_core::market_data::fixings::get_fixing_series(
            context,
            financing.forward_curve_id.as_str(),
        )
        .ok();
        let period_schedule = schedule.period_schedule()?;

        let mut total_pv = NeumaierAccumulator::new();
        let ctx = DayCountContext::default();

        for i in 1..period_schedule.dates.len() {
            let period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];

            if period_end <= as_of {
                continue;
            }

            let yf = financing
                .day_count
                .year_fraction(period_start, period_end, ctx)?;

            // Project the period rate per the leg's compounding convention
            // (simple term-rate average vs daily-compounded OIS).
            // In-progress OIS periods splice realized fixings with projected forwards.
            let fwd_rate = financing_period_rate(
                financing,
                fwd.as_ref(),
                fixings,
                period_start,
                period_end,
                yf,
                as_of,
                schedule.params.calendar_id.as_str(),
            )?;
            let payment = notional.amount() * fwd_rate * yf;

            let payment_date = schedule.payment_date_for(period_end)?;
            let df = relative_df_discount_curve(disc.as_ref(), as_of, payment_date)?;
            total_pv.add(payment * df);
        }

        Ok(total_pv.total())
    }
}

#[cfg(test)]
mod tests {
    use super::{TotalReturnLegParams, TrsEngine, TrsReturnModel};
    use crate::cashflow::builder::ScheduleParams;
    use crate::instruments::common_impl::parameters::legs::{
        FinancingLegSpec, FinancingRateCompounding,
    };
    use crate::instruments::common_impl::parameters::trs_common::TrsScheduleSpec;
    use crate::instruments::common_impl::pricing::swap_legs;
    use crate::instruments::common_impl::pricing::time::{
        rate_period_on_dates, relative_df_discount_curve,
    };
    use finstack_core::currency::Currency;
    use finstack_core::dates::{
        BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
    };
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
    use rust_decimal::Decimal;
    use std::cell::Cell;
    use time::Month;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("month"), d).expect("date")
    }

    struct FlatReturnModel {
        rate: f64,
    }

    struct SequencedReturnModel {
        returns: [f64; 3],
        next_idx: Cell<usize>,
    }

    impl TrsReturnModel for FlatReturnModel {
        fn period_return(
            &self,
            _inputs: &super::PeriodReturnInputs,
            _context: &MarketContext,
        ) -> finstack_core::Result<f64> {
            Ok(self.rate)
        }
    }

    impl TrsReturnModel for SequencedReturnModel {
        fn period_return(
            &self,
            _inputs: &super::PeriodReturnInputs,
            _context: &MarketContext,
        ) -> finstack_core::Result<f64> {
            let idx = self.next_idx.get();
            self.next_idx.set(idx + 1);
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
                freq: Tenor::monthly(),
                dc: DayCount::Act365F,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
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
            notional: Money::new(1.0, Currency::USD),
            discount_curve_id: "DISC",
            contract_size: 1.0,
            initial_level: Some(1.0),
        };
        let model = SequencedReturnModel {
            returns: [1e16, 1.0, -1e16],
            next_idx: Cell::new(0),
        };

        let pv =
            TrsEngine::pv_total_return_leg_with_model(params, &ctx, as_of, &model).expect("pv");

        assert_eq!(pv.amount(), 1.0);
    }

    #[test]
    fn trs_total_return_leg_uses_curve_df_between_dates() {
        let as_of = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let schedule = TrsScheduleSpec::from_params(
            as_of,
            end,
            ScheduleParams {
                freq: Tenor::quarterly(),
                dc: DayCount::Act365F,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
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
            notional: Money::new(1_000_000.0, Currency::USD),
            discount_curve_id: "DISC",
            contract_size: 1.0,
            initial_level: Some(100.0),
        };
        let model = FlatReturnModel { rate: 0.05 };

        let pv =
            TrsEngine::pv_total_return_leg_with_model(params, &ctx, as_of, &model).expect("pv");

        let period_schedule = schedule.period_schedule().expect("schedule");
        let mut expected = 0.0;
        let mut naive = 0.0;
        let ctx_dc = DayCountContext::default();

        for i in 1..period_schedule.dates.len() {
            let _period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];
            let df = relative_df_discount_curve(&disc, as_of, period_end).expect("df");
            let t_end = schedule
                .params
                .dc
                .year_fraction(as_of, period_end, ctx_dc)
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
    fn trs_financing_leg_uses_curve_time_for_forward_rates() {
        let as_of = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let schedule = TrsScheduleSpec::from_params(
            as_of,
            end,
            ScheduleParams {
                freq: Tenor::quarterly(),
                dc: DayCount::Act365F,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
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
            compounding: FinancingRateCompounding::TermRate,
        };

        let pv = TrsEngine::pv_financing_leg(
            &financing,
            &schedule,
            Money::new(1_000_000.0, Currency::USD),
            &ctx,
            as_of,
        )
        .expect("pv");

        let period_schedule = schedule.period_schedule().expect("schedule");
        let mut expected = 0.0;
        let mut naive = 0.0;
        let ctx_dc = DayCountContext::default();

        for i in 1..period_schedule.dates.len() {
            let period_start = period_schedule.dates[i - 1];
            let period_end = period_schedule.dates[i];
            // Use financing.day_count for accrual (matches engine after fix)
            let yf = financing
                .day_count
                .year_fraction(period_start, period_end, ctx_dc)
                .expect("yf");
            let fwd_rate = rate_period_on_dates(&fwd, period_start, period_end).expect("fwd");
            let df = relative_df_discount_curve(&disc, as_of, period_end).expect("df");
            expected += 1_000_000.0 * fwd_rate * yf * df;

            let t_start = schedule
                .params
                .dc
                .year_fraction(as_of, period_start, ctx_dc)
                .expect("t_start");
            let t_end = schedule
                .params
                .dc
                .year_fraction(as_of, period_end, ctx_dc)
                .expect("t_end");
            let fwd_naive = fwd.rate_period(t_start, t_end);
            let df_naive = disc.df(t_end);
            naive += 1_000_000.0 * fwd_naive * yf * df_naive;
        }

        let diff = (pv.amount() - expected).abs();
        let tol = 1e-8 * 1_000_000.0;
        assert!(diff < tol, "PV should use curve time: diff={}", diff);
        assert!(
            (expected - naive).abs() > 1e-6,
            "Expected curve-based forward/DF to differ from naive time mapping"
        );
    }

    /// Regression test: an in-progress `OvernightCompounded` financing period that
    /// straddles `as_of` must splice realized daily fixings (period_start → as_of)
    /// with projected forwards (as_of → period_end), NOT project the whole period
    /// from the forward curve. This test verifies that the spliced result differs
    /// from the all-projected result, and matches a hand-computed expected value.
    #[test]
    fn trs_financing_leg_ois_in_progress_splices_realized_and_projected() {
        use finstack_core::market_data::scalars::ScalarTimeSeries;

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
                freq: Tenor::monthly(),
                dc: DayCount::Act365F,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::ShortBack,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
            },
        );

        let financing = FinancingLegSpec {
            discount_curve_id: CurveId::new("DISC"),
            forward_curve_id: CurveId::new("SOFR"),
            spread_bp: Decimal::ZERO,
            day_count: DayCount::Act360,
            compounding: FinancingRateCompounding::OvernightCompounded,
        };

        let notional = Money::new(1_000_000.0, Currency::USD);
        let pv_spliced = TrsEngine::pv_financing_leg(&financing, &schedule, notional, &ctx, as_of)
            .expect("pv_spliced");

        // Compute expected spliced compound independently:
        // realized sub-period: weekdays in [Jan 6, Jan 17], each at 4 %
        // projected sub-period: weekdays in [Jan 20, Feb 3), each at 5 %
        let ctx_dc = DayCountContext::default();
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
                .year_fraction(d, next_d, ctx_dc)
                .expect("dcf");
            let r = if d < as_of { 0.04 } else { 0.05 };
            cf *= 1.0 + r * dcf;
            d = next_d;
        }
        let yf = DayCount::Act360
            .year_fraction(period_start, period_end, ctx_dc)
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
                .year_fraction(d2, next_d, ctx_dc)
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
    fn trs_financing_leg_overnight_compounding_exceeds_term_rate_on_upward_curve() {
        // On an upward-sloping forward curve, daily-compounding an OIS financing
        // leg picks up positive convexity that the simple term-rate average
        // drops. A `TermRate` and an `OvernightCompounded` leg over the same
        // curves must therefore price differently, with OIS strictly higher.
        let as_of = date(2025, 1, 1);
        let end = date(2026, 1, 1);
        let schedule = TrsScheduleSpec::from_params(
            as_of,
            end,
            ScheduleParams {
                freq: Tenor::quarterly(),
                dc: DayCount::Act365F,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
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
        let notional = Money::new(1_000_000.0, Currency::USD);

        let term = FinancingLegSpec::new("DISC", "FWD", Decimal::ZERO, DayCount::Act365F);
        let ois = term
            .clone()
            .with_compounding(FinancingRateCompounding::OvernightCompounded);

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
}
