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
    realized_period_cash: Money,
) -> Result<()> {
    let time_period_days = inputs.time_period_days;

    // Same-day window: no elapsed carry.
    if time_period_days <= 0.0 {
        attribution.carry = Money::from((0_i64, inputs.ccy));
        return Ok(());
    }

    // Theta / CarryTotal / CouponIncome / PullToPar / RollDown / FundingCost
    // are PERIOD TOTALS over the producer's `theta_period` (default 1D),
    // capped at expiry. Discrete coupon-like metrics must not be linearly
    // extrapolated: a missing `theta_period_days` stamp on a window other
    // than the 1-day default is a hard error, and CouponIncome is replaced
    // by realized `[T0, T1)` cash when the window differs from the producer
    // horizon.
    let theta_horizon_days = inputs
        .val_t0
        .measures
        .get(MetricId::ThetaPeriodDays.as_str())
        .copied()
        .filter(|d| d.is_finite() && *d > 0.0);

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

    if time_period_days > 1.0
        && theta_horizon_days.is_none()
        && (has_carry_total || has_theta || has_coupon)
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "metrics-based carry requires theta_period_days when the attribution \
             window is {time_period_days} days (≠ 1-day producer default); \
             discrete coupons must not be linearly extrapolated"
        )));
    }

    let carry_scale = match theta_horizon_days {
        Some(horizon) => time_period_days / horizon,
        None => 1.0,
    };
    let scale_differs_from_horizon = (carry_scale - 1.0).abs() > 1e-9;

    if let Some(horizon) = theta_horizon_days {
        if (horizon - time_period_days).abs() > 1e-9 {
            attribution.meta.notes.push(format!(
                "Carry metrics: continuous legs normalized from a {horizon}-day \
                 producer horizon to the {time_period_days}-day attribution window; \
                 coupon income uses realized period cash, not a linear scale"
            ));
        }
    }

    if scale_differs_from_horizon && !has_coupon && (has_carry_total || has_theta) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "metrics-based carry cannot linearly scale CarryTotal/Theta across a \
             {time_period_days}-day window that differs from the producer horizon; \
             request CouponIncome (realized cash is used) or stamp a matching \
             theta_period_days"
        )));
    }

    let get_scaled = |id: MetricId, notes: &mut Vec<String>, flag: &mut bool| -> Option<Money> {
        inputs.val_t0.measures.get(id.as_str()).map(|value| {
            factor_money_or_invalid(value * carry_scale, inputs.ccy, id.as_str(), notes, flag)
        })
    };

    if has_carry_total {
        let coupon_income = if scale_differs_from_horizon {
            Some(realized_period_cash)
        } else {
            inputs
                .val_t0
                .measures
                .get(MetricId::CouponIncome.as_str())
                .map(|value| {
                    factor_money_or_invalid(
                        *value,
                        inputs.ccy,
                        MetricId::CouponIncome.as_str(),
                        &mut attribution.meta.notes,
                        non_finite_detected,
                    )
                })
        };

        let pull_to_par = get_scaled(
            MetricId::PullToPar,
            &mut attribution.meta.notes,
            non_finite_detected,
        );
        let roll_down = get_scaled(
            MetricId::RollDown,
            &mut attribution.meta.notes,
            non_finite_detected,
        );
        let funding_cost = get_scaled(
            MetricId::FundingCost,
            &mut attribution.meta.notes,
            non_finite_detected,
        );

        if scale_differs_from_horizon {
            let coupon_amt = coupon_income.map(|m| m.amount()).unwrap_or(0.0);
            let ptp_amt = pull_to_par.map(|m| m.amount()).unwrap_or(0.0);
            let rd_amt = roll_down.map(|m| m.amount()).unwrap_or(0.0);
            let funding_amt = funding_cost.map(|m| m.amount()).unwrap_or(0.0);
            attribution.carry = factor_money_or_invalid(
                coupon_amt + ptp_amt + rd_amt - funding_amt,
                inputs.ccy,
                "carry total (reconstructed)",
                &mut attribution.meta.notes,
                non_finite_detected,
            );
        } else if let Some(carry_total) = inputs.val_t0.measures.get(MetricId::CarryTotal.as_str())
        {
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
            *theta * carry_scale,
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
