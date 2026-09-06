use super::super::helpers::*;
use super::super::types::*;
use super::context::AttributionInputs;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::metrics::MetricId;

pub(super) fn apply(
    inputs: &AttributionInputs<'_>,
    attribution: &mut PnlAttribution,
    non_finite_detected: &mut bool,
) -> Result<()> {
    let time_period_days = inputs.time_period_days;

    if time_period_days <= 0.0 {
        attribution.carry = Money::from((0_i64, inputs.ccy));
        return Ok(());
    }

    // Theta / CarryTotal / CouponIncome / PullToPar / RollDown / FundingCost
    // are PERIOD TOTALS over the producer's `theta_period` (default 1D),
    // capped at expiry. PullToPar and RollDown also contain payment-date
    // price drops, so replacing only CouponIncome cannot make linear
    // rescaling valid. Require a matching horizon for every carry metric.
    let theta_horizon_days = inputs
        .val_t0
        .measures
        .get(MetricId::ThetaPeriodDays.as_str())
        .copied();

    let has_carry_total = inputs
        .val_t0
        .measures
        .get(MetricId::CarryTotal.as_str())
        .is_some();
    let has_theta = inputs
        .val_t0
        .measures
        .get(MetricId::Theta.as_str())
        .is_some();
    let has_coupon = inputs
        .val_t0
        .measures
        .get(MetricId::CouponIncome.as_str())
        .is_some();
    let has_carry = has_carry_total || has_theta || has_coupon;

    if has_carry {
        match theta_horizon_days {
            None if time_period_days > 1.0 => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "metrics-based carry requires theta_period_days when the attribution \
                     window is {time_period_days} days (≠ 1-day producer default); \
                     discrete coupons must not be linearly extrapolated"
                )));
            }
            Some(horizon)
                if !horizon.is_finite()
                    || horizon <= 0.0
                    || (horizon - time_period_days).abs() > 1e-9 =>
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "metrics-based carry requires a matching theta_period_days horizon ({time_period_days} days); \
                     recompute carry metrics over the attribution window because coupon-related price drops cannot be scaled"
                )));
            }
            _ => {}
        }
    }

    let get_money = |id: MetricId, notes: &mut Vec<String>, flag: &mut bool| -> Option<Money> {
        inputs
            .val_t0
            .measures
            .get(id.as_str())
            .map(|value| factor_money_or_invalid(*value, inputs.ccy, id.as_str(), notes, flag))
    };

    if has_carry_total {
        let coupon_income = get_money(
            MetricId::CouponIncome,
            &mut attribution.meta.notes,
            non_finite_detected,
        );
        let pull_to_par = get_money(
            MetricId::PullToPar,
            &mut attribution.meta.notes,
            non_finite_detected,
        );
        let roll_down = get_money(
            MetricId::RollDown,
            &mut attribution.meta.notes,
            non_finite_detected,
        );
        let funding_cost = get_money(
            MetricId::FundingCost,
            &mut attribution.meta.notes,
            non_finite_detected,
        );

        if let Some(carry_total) = inputs.val_t0.measures.get(MetricId::CarryTotal.as_str()) {
            attribution.carry = factor_money_or_invalid(
                *carry_total,
                inputs.ccy,
                "carry total",
                &mut attribution.meta.notes,
                non_finite_detected,
            );
        }

        attribution.carry_detail = Some(CarryDetail {
            total: attribution.carry,
            coupon_income: coupon_income.map(SourceLine::scalar),
            pull_to_par,
            roll_down: roll_down.map(SourceLine::scalar),
            funding_cost,
        });
    } else if let Some(theta) = inputs.val_t0.measures.get(MetricId::Theta.as_str()) {
        attribution.carry = factor_money_or_invalid(
            *theta,
            inputs.ccy,
            "carry/theta",
            &mut attribution.meta.notes,
            non_finite_detected,
        );
        attribution.carry_detail = Some(CarryDetail {
            total: attribution.carry,
            coupon_income: None,
            pull_to_par: None,
            roll_down: Some(SourceLine::scalar(attribution.carry)),
            funding_cost: None,
        });
    } else {
        note_warning(
            attribution,
            "Metrics-based carry attribution skipped: neither CarryTotal nor Theta metric was present; carry P&L set to zero",
            inputs.instrument.id(),
            "carry",
        );
    }
    Ok(())
}
