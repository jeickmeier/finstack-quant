use super::super::helpers::factor_money_or_invalid;
use super::super::types::{CrossFactorDetail, PnlAttribution};
use super::context::MetricsContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::metrics::MetricId;
use indexmap::IndexMap;

fn add_cross_factor_term(
    by_pair: &mut IndexMap<String, Money>,
    total: &mut f64,
    label: &str,
    pnl: f64,
    currency: finstack_quant_core::currency::Currency,
    notes: &mut Vec<String>,
    result_invalid: &mut bool,
) {
    if pnl.is_finite() && pnl.abs() < 1e-12 {
        return;
    }
    let money = factor_money_or_invalid(pnl, currency, label, notes, result_invalid);
    // Only accumulate into total if finite; the sentinel zero already keeps the
    // sum well-behaved when result_invalid is set.
    if pnl.is_finite() {
        *total += pnl;
    }
    by_pair.insert(label.to_string(), money);
}

pub(super) fn apply(
    inputs: &MetricsContext<'_>,
    attribution: &mut PnlAttribution,
    non_finite_detected: &mut bool,
) {
    // Cross-factor terms.
    //
    // Same seven pairs as the parallel attribution (see
    // [`crate::parallel::attribute_pnl_parallel`]
    // for the economic justification of the selection): Rates×Credit,
    // Rates×Vol, Spot×Vol, Spot×Credit, FX×Vol, FX×Rates and Credit×Vol
    // (the convertible pair). Each multiplies a
    // mixed second-partial (`CrossGamma_X_Y` metric) by the two observed
    // moves; the result enters `cross_factor_pnl` instead of either
    // factor's univariate P&L. Pairs not listed flow into the residual
    // bucket and may be material for books loaded on inflation /
    // structured / non-standard cross-effects — for those, prefer
    // parallel/waterfall attribution.
    //
    // UNIT CONTRACT for Spot cross-gamma metrics:
    // `CrossGammaSpotVol` and `CrossGammaSpotCredit` are produced by
    // `CrossFactorCalculator` using percentage-point–normalised finite
    // differences: the spot bump denominator is `spot_bump_decimal × 100`
    // (e.g. 1.0 for a 1 % bump) and the vol/credit denominator is
    // similarly in percentage-point units.  Therefore the attribution
    // below must multiply by `avg_spot_shift_pct` (percentage-point spot
    // move), `avg_vol_shift_abs` (vol points) and `avg_credit_shift_bp`
    // (basis points) — each matching its cross-gamma metric's convention.
    //
    // WARNING: Do NOT substitute `MetricId::Vanna` here as a fallback for
    // `CrossGammaSpotVol`.  `Vanna` is defined as ∂²V/(∂S_abs × ∂σ)
    // — per unit spot, per vol point — and differs from
    // `CrossGammaSpotVol` by a factor of S₀ / 100 (the spot axes differ:
    // per unit spot vs per 1 pct-pt spot move).  Using `Vanna` with
    // percentage-point moves would mis-scale the cross P&L by 100/S₀.
    //
    // TWIST LIMITATION: the rate/credit cross terms below multiply the
    // mixed second-partial by the *signed average* shifts
    // (`avg_rate_shift_bp`, `avg_credit_shift_bp`). For a twisted curve
    // those averages collapse toward zero, so the cross P&L is understated
    // — the same caveat the rates/credit convexity blocks already emit a
    // twist note for. Prefer parallel/waterfall attribution when curves are
    // twisted and cross-gamma materiality matters.
    let mut cross_total = 0.0;
    let mut cross_by_pair = IndexMap::new();
    let currency = inputs.ccy;

    let pairs = [
        (
            MetricId::CrossGammaRatesCredit,
            "Rates×Credit",
            inputs.shifts.avg_rate_shift_bp,
            inputs.shifts.avg_credit_shift_bp,
        ),
        (
            MetricId::CrossGammaRatesVol,
            "Rates×Vol",
            inputs.shifts.avg_rate_shift_bp,
            inputs.shifts.avg_vol_shift_abs,
        ),
        (
            MetricId::CrossGammaSpotVol,
            "Spot×Vol",
            inputs.shifts.avg_spot_shift_pct,
            inputs.shifts.avg_vol_shift_abs,
        ),
        (
            MetricId::CrossGammaSpotCredit,
            "Spot×Credit",
            inputs.shifts.avg_spot_shift_pct,
            inputs.shifts.avg_credit_shift_bp,
        ),
        (
            MetricId::CrossGammaFxVol,
            "FX×Vol",
            inputs.shifts.fx_shift_pct,
            inputs.shifts.avg_vol_shift_abs,
        ),
        (
            MetricId::CrossGammaFxRates,
            "FX×Rates",
            inputs.shifts.fx_shift_pct,
            inputs.shifts.avg_rate_shift_bp,
        ),
        (
            MetricId::CrossGammaCreditVol,
            "Credit×Vol",
            inputs.shifts.avg_credit_shift_bp,
            inputs.shifts.avg_vol_shift_abs,
        ),
    ];
    for (metric, label, first_move, second_move) in pairs {
        if let (Some(cross_gamma), Some(first_move), Some(second_move)) = (
            inputs.val_t0.measures.get(metric.as_str()).copied(),
            first_move,
            second_move,
        ) {
            add_cross_factor_term(
                &mut cross_by_pair,
                &mut cross_total,
                label,
                cross_gamma * first_move * second_move,
                currency,
                &mut attribution.meta.notes,
                non_finite_detected,
            );
        }
    }

    if !cross_by_pair.is_empty() {
        attribution.cross_factor_pnl = factor_money_or_invalid(
            cross_total,
            currency,
            "cross-factor P&L total",
            &mut attribution.meta.notes,
            non_finite_detected,
        );
        attribution.cross_factor_detail = Some(CrossFactorDetail {
            total: attribution.cross_factor_pnl,
            by_pair: cross_by_pair,
        });
    }
}
