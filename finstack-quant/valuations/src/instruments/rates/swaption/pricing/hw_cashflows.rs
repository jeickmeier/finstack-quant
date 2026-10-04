//! Contractual exercise cashflows shared by European and Bermudan HW trees.
use crate::cashflow::builder::periods::{build_periods, period_accrual, BuildPeriodsParams};
use crate::cashflow::builder::specs::RollRule;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::parameters::{FixedLegSpec, FloatLegSpec, OptionType};
use crate::instruments::rates::swaption::CashSettlementMethod;
use crate::instruments::SettlementType;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::Result;
use finstack_quant_models::rates::clock::model_time;
use finstack_quant_models::rates::hull_white::{
    hw1f_delayed_coupon_adjustment, HullWhiteCalibrationParams,
};
use finstack_quant_models::trees::HullWhiteTree;

pub(crate) struct HwSwaptionCashflows {
    fixed: Vec<(f64, f64)>,
    fixed_payments_per_year: f64,
    float: Vec<(f64, f64, f64, f64, f64)>,
    spread: f64,
    compounded: bool,
}

impl HwSwaptionCashflows {
    pub(crate) fn new(
        fixed: &FixedLegSpec,
        float: &FloatLegSpec,
        exercise: Date,
        as_of: Date,
    ) -> Result<Self> {
        use crate::instruments::rates::irs::FloatingLegCompounding;
        let compounded = match float.compounding {
            FloatingLegCompounding::Simple => false,
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
            | FloatingLegCompounding::CompoundedWithObservationShift { shift_days: 0 }
            | FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 0 } => true,
            _ => return Err(finstack_quant_core::Error::Validation(
                "Hull-White swaption trees require simple floating coupons or compounded coupons without observation shifts, lookbacks or rate cutoffs".into())),
        };
        let fixed_params = fixed.schedule_params(false);
        let fixed_payments_per_year = fixed.frequency.payments_per_year();
        let fixed = build_periods(fixed_params)?
            .iter()
            .filter(|p| p.accrual_end > exercise && p.payment_date > exercise)
            .map(|p| {
                Ok((
                    model_time(as_of, p.payment_date),
                    period_accrual(
                        p,
                        p.accrual_start.max(exercise),
                        p.accrual_end,
                        &fixed_params,
                    )?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let float_params = BuildPeriodsParams {
            start: float.start,
            end: float.end,
            frequency: float.frequency,
            stub: float.stub,
            business_day_convention: float.business_day_convention,
            calendar_id: float
                .calendar_id
                .as_deref()
                .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
            end_of_month: float.end_of_month,
            day_count: float.day_count,
            payment_lag_days: float.payment_lag_days,
            reset_lag_days: Some(float.reset_lag_days),
            adjust_accrual_dates: false,
            roll_rule: RollRule::None,
        };
        let spread = decimal_to_f64(float.spread_bp, "Hull-White floating spread_bp")? / 10_000.0;
        let float = build_periods(float_params)?
            .iter()
            .filter(|p| p.accrual_end > exercise && p.payment_date > exercise)
            .map(|p| {
                Ok((
                    model_time(as_of, p.accrual_start.max(exercise)),
                    model_time(as_of, p.accrual_end),
                    model_time(as_of, p.payment_date),
                    period_accrual(
                        p,
                        p.accrual_start.max(exercise),
                        p.accrual_end,
                        &float_params,
                    )?,
                    model_time(as_of, p.reset_date.unwrap_or(p.accrual_start).max(exercise)),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            fixed,
            fixed_payments_per_year,
            float,
            spread,
            compounded,
        })
    }

    pub(crate) fn horizon(&self) -> f64 {
        self.fixed
            .iter()
            .map(|&(pay, _)| pay)
            .chain(
                self.float
                    .iter()
                    .flat_map(|&(start, end, pay, _, _)| [start, end, pay]),
            )
            .fold(0.0, f64::max)
    }

    pub(crate) fn value(&self, node: HwExerciseNode<'_>, terms: HwExerciseTerms) -> f64 {
        let HwExerciseNode {
            tree,
            step,
            index,
            discount,
        } = node;
        let bond = |t| tree.bond_price(step, index, t, discount);
        self.evaluate(
            tree.time_at_step(step),
            tree.config().kappa,
            tree.config().sigma,
            bond,
            terms,
        )
        .2
    }

    pub(crate) fn evaluate(
        &self,
        t: f64,
        kappa: f64,
        sigma: f64,
        bond: impl Fn(f64) -> f64,
        terms: HwExerciseTerms,
    ) -> (f64, f64, f64) {
        let annuity: f64 = self.fixed.iter().map(|&(pay, tau)| tau * bond(pay)).sum();
        if annuity <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let floating: f64 = self
            .float
            .iter()
            .map(|&(start, end, pay, tau, reset)| {
                let adjustment = hw1f_delayed_coupon_adjustment(
                    HullWhiteCalibrationParams { kappa, sigma },
                    t,
                    start,
                    end,
                    pay,
                    reset,
                    self.compounded,
                );
                (bond(start) / bond(end) * adjustment - 1.0 + self.spread * tau) * bond(pay)
            })
            .sum();
        let swap_rate = floating / annuity;
        let payoff_annuity = match (terms.settlement, terms.cash_method) {
            (SettlementType::Cash, CashSettlementMethod::ParYield) => {
                par_yield_annuity(self.fixed.len(), self.fixed_payments_per_year, swap_rate)
            }
            (SettlementType::Cash, CashSettlementMethod::ZeroCoupon) => {
                self.fixed.iter().map(|&(_, tau)| tau).sum::<f64>() * bond(self.horizon())
            }
            _ => annuity,
        };
        let sign = match terms.option_type {
            OptionType::Call => 1.0,
            OptionType::Put => -1.0,
        };
        (
            swap_rate,
            annuity,
            (sign * (swap_rate - terms.strike)).max(0.0) * payoff_annuity * terms.notional,
        )
    }
}

pub(crate) struct HwExerciseNode<'a> {
    pub tree: &'a HullWhiteTree,
    pub step: usize,
    pub index: usize,
    pub discount: &'a dyn Discounting,
}

/// Par-yield cash-settlement annuity of a fixed leg, valued on the settlement
/// date.
///
/// The ISDA par-yield cash price discounts a level annuity of `1/m` per
/// period at the settlement swap rate itself, compounded at the fixed-leg
/// frequency:
///
/// ```text
/// A(S) = Σᵢ₌₁ⁿ (1/m) / (1 + S/m)ⁱ = (1 − (1 + S/m)^{−n}) / S
/// ```
///
/// `n` is the number of fixed-leg payment periods, not a day-count year
/// fraction times `m`: an annual ACT/360 leg over five years has five
/// periods, although its accruals sum to 5.07.
///
/// # Arguments
///
/// * `periods` - Number of fixed-leg payment periods `n`.
/// * `payments_per_year` - Fixed-leg payment frequency `m`.
/// * `swap_rate` - Settlement swap rate `S` as a decimal.
pub(crate) fn par_yield_annuity(periods: usize, payments_per_year: f64, swap_rate: f64) -> f64 {
    crate::instruments::rates::cms_common::par_annuity(
        swap_rate,
        periods as f64 / payments_per_year,
        payments_per_year,
    )
}

/// Lift a Bermudan engine value to at least the value of exercising today.
///
/// The exercise engines model strictly future decision dates. When `as_of` is
/// itself a live exercise date the holder can still exercise that day, so the
/// option is worth the larger of exercising now and continuing. Exercising at
/// model time zero is deterministic: every bond price is a discount factor of
/// today's curve, so no model volatility enters and all engines agree.
///
/// # Arguments
///
/// * `swaption` - Bermudan swaption whose lockout-filtered exercise schedule
///   and underlying legs define the exercise.
/// * `discount` - Discount curve of the single-curve valuation.
/// * `as_of` - Valuation date; only an exercise date equal to it matters.
/// * `continuation` - Engine value of the remaining, strictly future exercise
///   rights in the swaption's currency.
///
/// # Errors
///
/// Returns an error when the underlying legs cannot be scheduled or the curve
/// cannot be re-based to `as_of`.
pub(crate) fn value_with_exercise_today(
    swaption: &crate::instruments::rates::swaption::BermudanSwaption,
    discount: &dyn Discounting,
    as_of: Date,
    continuation: f64,
) -> Result<f64> {
    if !swaption
        .exercise_schedule
        .effective_dates()
        .contains(&as_of)
    {
        return Ok(continuation);
    }
    let flows = HwSwaptionCashflows::new(
        &swaption.underlying_fixed_leg,
        &swaption.underlying_float_leg,
        as_of,
        as_of,
    )?;
    let curve = finstack_quant_models::rates::clock::ModelDiscountCurve::new(discount, as_of)?;
    let terms = HwExerciseTerms {
        strike: swaption.strike_f64()?,
        notional: swaption.notional.amount(),
        option_type: swaption.option_type,
        settlement: swaption.settlement,
        cash_method: CashSettlementMethod::default(),
    };
    // Zero volatility switches every convexity adjustment off; the
    // mean-reversion argument is then irrelevant.
    let (_, _, exercise_today) =
        flows.evaluate(0.0, 1.0, 0.0, |maturity| curve.df(maturity), terms);
    Ok(continuation.max(exercise_today))
}

#[derive(Clone, Copy)]
pub(crate) struct HwExerciseTerms {
    pub strike: f64,
    pub notional: f64,
    pub option_type: OptionType,
    pub settlement: SettlementType,
    pub cash_method: CashSettlementMethod,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_floating_payment_matches_integrated_gaussian_covariance() {
        let (t, start, end, pay) = (1.0_f64, 2.0_f64, 3.0_f64, 3.2_f64);
        let (kappa, sigma) = (0.05_f64, 0.02_f64);
        let bond = |maturity: f64| (-0.03 * (maturity - t)).exp();
        let terms = HwExerciseTerms {
            strike: 0.0,
            notional: 1.0,
            option_type: OptionType::Call,
            settlement: SettlementType::Physical,
            cash_method: CashSettlementMethod::CollateralizedCashPrice,
        };
        // Integrate the Brownian loading products directly. Under the payment
        // measure, the log coupon ratio shifts by minus its covariance with
        // the end-to-payment discounting integral.
        for compounded in [false, true] {
            let flows = HwSwaptionCashflows {
                fixed: vec![(pay, 1.0)],
                fixed_payments_per_year: 1.0,
                float: vec![(start, end, pay, 1.0, start)],
                spread: 0.001,
                compounded,
            };
            let last = if compounded { end } else { start };
            let n = 20_000;
            let du = (last - t) / f64::from(n);
            let covariance: f64 = (0..n)
                .map(|i| {
                    let u = t + (f64::from(i) + 0.5) * du;
                    let coupon_loading = if compounded {
                        sigma * ((-kappa * (start.max(u) - u)).exp() - (-kappa * (end - u)).exp())
                            / kappa
                    } else {
                        sigma * ((-kappa * (start - u)).exp() - (-kappa * (end - u)).exp()) / kappa
                    };
                    let delay_loading =
                        sigma * ((-kappa * (end - u)).exp() - (-kappa * (pay - u)).exp()) / kappa;
                    coupon_loading * delay_loading * du
                })
                .sum();
            let expected =
                (bond(start) / bond(end) * (-covariance).exp() - 1.0 + 0.001) * bond(pay);
            let actual = flows.evaluate(t, kappa, sigma, bond, terms).2;
            assert!(
                (actual - expected).abs() < 1e-11,
                "compounded={compounded}: actual={actual}, expected={expected}"
            );
        }
    }
}
