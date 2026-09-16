//! Call and put exercise for instrument collateral.
//!
//! A contractual schedule says nothing about *when* an issuer calls or a
//! holder puts, so the pool engine evaluates the deal's
//! [`CallExercisePolicy`] / [`PutExercisePolicy`] period by period against
//! each instrument's exercise windows. A decision redeems the instrument's
//! remaining balance at the window price: the par portion is a prepayment,
//! any premium above par is interest proceeds, and any discount below par is
//! a realized loss.

use super::*;
use crate::instruments::fixed_income::structured_credit::types::{
    CallExercisePolicy, PutExercisePolicy,
};
use finstack_quant_core::market_data::traits::Discounting;

/// One exercise window: the instrument may be redeemed at `price_pct` of par
/// on any date in `[start, end]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ExerciseWindow {
    /// First exercise date.
    pub(super) start: Date,
    /// Last exercise date, inclusive.
    pub(super) end: Date,
    /// Redemption price as a percentage of par (100 = par).
    pub(super) price_pct: f64,
}

impl ExerciseWindow {
    /// `true` when some date of the window falls in `(prev_date, pay_date]`.
    pub(super) fn open_in(&self, prev_date: Date, pay_date: Date) -> bool {
        self.start <= pay_date && self.end > prev_date
    }
}

/// How the refinancing rate is observed for `RefinancingIncentive`: the par
/// forward on the deal discount curve, plus the simulated path's departure
/// from that curve when the run carries a rate path.
#[derive(Clone, Copy)]
pub(super) struct RateView<'a> {
    /// Deal discount curve; `None` when no instrument needs it.
    pub(super) curve: Option<&'a dyn Discounting>,
    /// Additive shift of the simulated path over the curve forward
    /// (`0.0` on the deterministic curve).
    pub(super) shift: f64,
}

/// Per-instrument exercise configuration, resolved at preparation.
#[derive(Debug, Clone)]
pub(super) struct ExerciseTerms {
    /// Issuer call windows, in schedule order.
    pub(super) calls: Vec<ExerciseWindow>,
    /// Holder put windows, in schedule order.
    pub(super) puts: Vec<ExerciseWindow>,
    /// Effective call policy.
    pub(super) call_policy: CallExercisePolicy,
    /// Effective put policy.
    pub(super) put_policy: PutExercisePolicy,
    /// Fixed coupon (annual decimal) for the refinancing-incentive test;
    /// `None` for floating instruments, which never exercise under it.
    pub(super) fixed_coupon: Option<f64>,
    /// Contractual maturity, the end of the refinancing horizon.
    pub(super) maturity: Date,
    /// Period index the yield-to-worst rule redeems in, resolved at preparation.
    pub(super) worst_period: Option<usize>,
}

/// Redemption decided for one period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Redemption {
    /// Redemption price as a percentage of par.
    pub(super) price_pct: f64,
}

impl ExerciseTerms {
    /// Decide whether the instrument is redeemed in period `k`.
    ///
    /// Calls are evaluated before puts; the first applicable window wins.
    ///
    /// # Arguments
    ///
    /// * `k` - Index of the legal period being simulated.
    /// * `prev_date` - Period start (exclusive).
    /// * `pay_date` - Period payment date (inclusive).
    /// * `rate_view` - Refinancing-rate source for the incentive rule.
    pub(super) fn evaluate(
        &self,
        k: usize,
        prev_date: Date,
        pay_date: Date,
        rate_view: RateView<'_>,
    ) -> Result<Option<Redemption>> {
        if let Some(call) = self.evaluate_call(k, prev_date, pay_date, rate_view)? {
            return Ok(Some(call));
        }
        Ok(self.evaluate_put(prev_date, pay_date))
    }

    fn evaluate_call(
        &self,
        k: usize,
        prev_date: Date,
        pay_date: Date,
        rate_view: RateView<'_>,
    ) -> Result<Option<Redemption>> {
        let open = || self.calls.iter().find(|w| w.open_in(prev_date, pay_date));
        match &self.call_policy {
            CallExercisePolicy::Contractual => Ok(None),
            CallExercisePolicy::FirstCall => Ok(open().map(|w| Redemption {
                price_pct: w.price_pct,
            })),
            CallExercisePolicy::Worst => Ok(match self.worst_period {
                Some(period) if period == k => open().map(|w| Redemption {
                    price_pct: w.price_pct,
                }),
                _ => None,
            }),
            CallExercisePolicy::RefinancingIncentive { threshold_bp } => {
                let (Some(window), Some(coupon)) = (open(), self.fixed_coupon) else {
                    return Ok(None);
                };
                let curve = rate_view.curve.ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "refinancing-incentive exercise requires the deal discount curve".into(),
                    )
                })?;
                let refinancing =
                    par_forward_rate(curve, pay_date, self.maturity)? + rate_view.shift;
                if (coupon - refinancing) * 10_000.0 > *threshold_bp {
                    Ok(Some(Redemption {
                        price_pct: window.price_pct,
                    }))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn evaluate_put(&self, prev_date: Date, pay_date: Date) -> Option<Redemption> {
        match self.put_policy {
            PutExercisePolicy::Never => None,
            PutExercisePolicy::FirstPut => self
                .puts
                .iter()
                .find(|w| w.open_in(prev_date, pay_date))
                .map(|w| Redemption {
                    price_pct: w.price_pct,
                }),
        }
    }
}

/// Simple par forward rate of `curve` from `start` to `end` (ACT/365F).
///
/// Returns `0.0` when `end <= start`.
pub(super) fn par_forward_rate(curve: &dyn Discounting, start: Date, end: Date) -> Result<f64> {
    if end <= start {
        return Ok(0.0);
    }
    let df = curve.df_between_dates(start, end)?;
    let tau = DayCount::Act365F.year_fraction(start, end, DayCountContext::default())?;
    if df <= 0.0 || tau <= 0.0 {
        return Ok(0.0);
    }
    Ok((1.0 / df - 1.0) / tau)
}

/// Yield-to-worst period: the exercise (or maturity) that minimises the
/// holder's yield given `price` (percentage of par) paid at `as_of`.
///
/// `flows_by_period` are the instrument's bucketed interest-and-fee and
/// repayment cash per legal period (already net of nothing: contractual
/// amounts); `balance_by_period` is the outstanding at the end of each period
/// under the contractual schedule; `period_dates` are the payment dates.
///
/// # Arguments
///
/// * `as_of` - Settlement date of the hypothetical purchase.
/// * `price` - Quoted clean price as a percentage of par.
/// * `opening_balance` - Outstanding at `as_of`.
/// * `period_dates` - Payment date of each legal period.
/// * `cash_by_period` - Contractual interest, fees and repayments per period.
/// * `balance_by_period` - Contractual outstanding after each period.
/// * `calls` - Call windows to test; each is tried at the first period whose
///   payment date is on or after the window start.
pub(super) fn worst_call_period(
    as_of: Date,
    price: f64,
    opening_balance: f64,
    period_dates: &[Date],
    cash_by_period: &[f64],
    balance_by_period: &[f64],
    calls: &[ExerciseWindow],
) -> Result<Option<usize>> {
    if opening_balance <= 0.0 || period_dates.is_empty() {
        return Ok(None);
    }
    let purchase = -price / 100.0 * opening_balance;
    let yield_to = |end_period: usize, redemption_pct: Option<f64>| -> Result<f64> {
        let mut flows: Vec<(Date, f64)> = Vec::with_capacity(end_period + 2);
        flows.push((as_of, purchase));
        for k in 0..=end_period {
            flows.push((period_dates[k], cash_by_period[k]));
        }
        if let Some(pct) = redemption_pct {
            flows.push((
                period_dates[end_period],
                pct / 100.0 * balance_by_period[end_period],
            ));
        }
        finstack_quant_core::cashflow::xirr(&flows, None)
    };
    let last = period_dates.len() - 1;
    let mut best: Option<(f64, Option<usize>)> = None;
    let mut consider = |value: Result<f64>, period: Option<usize>| {
        if let Ok(y) = value {
            if y.is_finite() && best.is_none_or(|(b, _)| y < b) {
                best = Some((y, period));
            }
        }
    };
    consider(yield_to(last, None), None);
    for window in calls {
        let Some(k) = period_dates.iter().position(|d| *d >= window.start) else {
            continue;
        };
        let prev = if k > 0 { period_dates[k - 1] } else { as_of };
        if !window.open_in(prev, period_dates[k]) || balance_by_period[k] <= 0.0 {
            continue;
        }
        consider(yield_to(k, Some(window.price_pct)), Some(k));
    }
    Ok(best.and_then(|(_, period)| period))
}
