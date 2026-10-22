//! Carry decomposition calculator.
//!
//! Computes carry as a decomposition into coupon income, pull-to-par, roll-down,
//! and optional funding cost.
//!
//! Pull-to-par uses the constant-yield accretion identity: holding the solved
//! yield fixed, the dirty PV accretes by `1/DF_y(tau)` over the horizon, and
//! coupons paid inside the horizon are carved out (they are reported
//! separately as coupon income). Roll-down is the residual
//! `total_pv_change - pull_to_par`, so on a flat curve self-consistent with
//! the yield it vanishes. The accretion inverts the same discounting
//! convention the instrument's `Ytm` metric was solved under (Street
//! compounding for bonds).
//!
//! # Market-Held Fixings
//!
//! The horizon PV is repriced on the unrolled curves with every coupon fixing
//! the as-of schedule projected in `[as_of, horizon]` added as an exact-date
//! `FIXING:` observation at its as-of projection, as theta does (see
//! [`theta`](crate::metrics::sensitivities::theta), "Market Held Fixed").
//! Floating-rate bonds and OIS/term swaps whose horizon crosses a fixing
//! therefore reprice instead of failing on the unpublished fixing.

use crate::instruments::pricing::coupon_period::icma_reference_period;
use crate::metrics::period_cash::collect_period_cash_cached;
use crate::metrics::sensitivities::theta::{calculate_theta_date, theta_termination_date};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::Result;
use std::sync::Arc;

/// The yield clock a carry decomposition accretes along.
///
/// Pull-to-par is the constant-yield accretion of the dirty PV over the carry
/// horizon. How a yield turns into an accretion factor depends on the
/// instrument's quoting convention (compounding, coupon frequency, day count),
/// so each family that quotes a yield supplies its clock and registers the
/// calculator with it; the universal registration accretes annually on the
/// context's day count.
pub trait YieldClock: Send + Sync {
    /// Accretion factor `1 / DF_y(as_of, rolled_date)` of the dirty PV under
    /// the constant yield `ytm`.
    ///
    /// # Arguments
    ///
    /// * `context` - Metric context of the instrument under evaluation.
    /// * `ytm` - Yield to maturity as a decimal rate in the instrument's
    ///   quoting convention.
    /// * `rolled_date` - End of the carry horizon.
    fn accretion_factor(&self, context: &MetricContext, ytm: f64, rolled_date: Date)
        -> Result<f64>;
}

/// Annual compounding on the context's day count (ACT/365F when none was
/// stamped); the quasi-coupon period is resolved from the cached cashflow
/// dates for ACT/ACT (ICMA).
struct AnnualYieldClock;

impl YieldClock for AnnualYieldClock {
    fn accretion_factor(
        &self,
        context: &MetricContext,
        ytm: f64,
        rolled_date: Date,
    ) -> Result<f64> {
        let frequency = Tenor::annual();
        let day_count = context.day_count.unwrap_or(DayCount::Act365F);
        let dc_ctx = DayCountContext {
            frequency: Some(frequency),
            coupon_period: context.cashflows.as_deref().and_then(|flows| {
                icma_reference_period(
                    day_count,
                    frequency,
                    flows.iter().map(|(d, _)| *d),
                    context.as_of,
                )
            }),
            ..DayCountContext::default()
        };
        let tau = day_count.year_fraction(context.as_of, rolled_date, dc_ctx)?;
        if tau <= 0.0 {
            return Ok(1.0);
        }
        if ytm < -0.5 {
            tracing::warn!(
                ytm = ytm,
                "Extreme negative yield detected (< -50%). This may indicate a data error."
            );
        }
        let base = 1.0 + ytm;
        if base <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Annual compounding base (1 + y) = {} is non-positive for ytm={}",
                base, ytm
            )));
        }
        Ok(base.powf(tau))
    }
}

pub struct CarryDecompositionCalculator {
    clock: Arc<dyn YieldClock>,
}

impl Default for CarryDecompositionCalculator {
    fn default() -> Self {
        Self::with_clock(AnnualYieldClock)
    }
}

impl CarryDecompositionCalculator {
    /// A carry decomposition accreting along `clock`.
    ///
    /// # Arguments
    ///
    /// * `clock` - Yield clock of the instrument family registering the
    ///   calculator.
    pub fn with_clock(clock: impl YieldClock + 'static) -> Self {
        Self {
            clock: Arc::new(clock),
        }
    }
}

impl MetricCalculator for CarryDecompositionCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let expiry_date = theta_termination_date(context)?;
        let period = crate::metrics::sensitivities::theta::theta_period(context);

        let rolled_date = calculate_theta_date(context.as_of, period, expiry_date)?;

        if rolled_date <= context.as_of {
            context.computed.insert(MetricId::CouponIncome, 0.0);
            context.computed.insert(MetricId::PullToPar, 0.0);
            context.computed.insert(MetricId::RollDown, 0.0);
            context.computed.insert(MetricId::FundingCost, 0.0);
            context
                .computed
                .insert(MetricId::CarryDecompositionDegenerate, 0.0);
            return Ok(0.0);
        }

        let base_pv = context.base_value.amount();
        let base_currency = context.base_value.currency();

        let start_date = context.as_of;
        let cash = collect_period_cash_cached(context, start_date, rolled_date, base_currency)?;
        let coupon_income = cash.income.amount();
        let principal_cash = cash.total.checked_sub(cash.income)?.amount();

        // Unrolled curves, with coupon fixings crossed by the horizon held at
        // their as-of projections (module docs, "Market-Held Fixings").
        let held_market = context.fixings_held_market(rolled_date)?;
        let curved_pv = context.reprice_money(&held_market, rolled_date)?.amount();
        let total_pv_change = curved_pv - base_pv;

        // `degenerate` is set when the pull-to-par / roll-down split cannot be
        // computed because the `Ytm` metric is unavailable for this instrument.
        // In that case `pull_to_par` is reported as `0.0` and `roll_down`
        // absorbs the entire PV change — a silent collapse that the
        // `CarryDecompositionDegenerate` diagnostic surfaces so downstream
        // consumers do not mistake the absorbed amount for genuine roll-down.
        let (pull_to_par, degenerate) = if let Some(&ytm) = context.computed.get(&MetricId::Ytm) {
            // Constant-yield accretion identity (Tuckman & Serrat, "Fixed
            // Income Securities", carry/roll-down decomposition): holding the
            // yield fixed, the dirty PV accretes by `1/DF_y(tau)` over the
            // horizon, and cash paid out inside the horizon is carved out so
            // it is not double counted against `coupon_income`. For a par
            // bond over a full coupon period this is ~0; for a zero-coupon
            // bond it is the full accretion.
            let accretion = self.clock.accretion_factor(context, ytm, rolled_date)?;
            let payments =
                crate::metrics::period_cash::period_cash_by_date_cached(context, rolled_date)?;
            let mut intermediate_accretion = 0.0;
            for (payment_date, amount) in payments {
                let at_payment = self.clock.accretion_factor(context, ytm, payment_date)?;
                intermediate_accretion += amount * (accretion / at_payment - 1.0);
            }
            (
                base_pv * (accretion - 1.0) - coupon_income - intermediate_accretion,
                false,
            )
        } else {
            tracing::warn!(
                instrument_id = context.instrument.id(),
                "Carry decomposition missing YTM dependency; pull_to_par reported as 0.0, \
                     roll_down absorbs the remaining PV change, and \
                     carry_decomposition_degenerate is flagged 1.0"
            );
            (0.0, true)
        };

        // Principal settlement reduces the live instrument PV without being
        // coupon income or curve roll-down. Neutralize that receipt against
        // the price drop before partitioning the remaining carry.
        let roll_down = total_pv_change + principal_cash - pull_to_par;
        let funding_cost = compute_funding_cost(context, rolled_date)?;
        let carry_total = coupon_income + pull_to_par + roll_down - funding_cost;

        context
            .computed
            .insert(MetricId::CouponIncome, coupon_income);
        context.computed.insert(MetricId::PullToPar, pull_to_par);
        context.computed.insert(MetricId::RollDown, roll_down);
        context.computed.insert(MetricId::FundingCost, funding_cost);
        context.computed.insert(
            MetricId::CarryDecompositionDegenerate,
            if degenerate { 1.0 } else { 0.0 },
        );
        // Stamp the realized horizon so consumers rescaling these period
        // totals (e.g. P&L attribution) can normalize instead of assuming 1D.
        context.computed.insert(
            MetricId::ThetaPeriodDays,
            (rolled_date - context.as_of).whole_days() as f64,
        );

        Ok(carry_total)
    }

    fn dependencies(&self) -> &[MetricId] {
        static DEPS: &[MetricId] = &[MetricId::Ytm];
        DEPS
    }
}

pub fn compute_funding_cost(context: &MetricContext, rolled_date: Date) -> Result<f64> {
    let Some(repo_curve_id) = context.instrument.repo_curve_id() else {
        return Ok(0.0);
    };

    let funding_curve = context.curves.get_discount(repo_curve_id.as_str())?;
    let horizon_df = funding_curve.df_between_dates(context.as_of, rolled_date)?;
    Ok(context.base_value.amount() * (1.0 / horizon_df - 1.0))
}
