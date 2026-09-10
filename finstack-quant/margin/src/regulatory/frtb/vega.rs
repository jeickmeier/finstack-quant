//! FRTB vega risk charge computation.
//!
//! Vega sensitivities are volatility-weighted, then aggregated using
//! the same two-level (intra-bucket, inter-bucket) formula as delta,
//! but with vega-specific risk weights and correlations.

use super::aggregation::{
    inter_bucket, inter_bucket_plus_undiversified, inter_bucket_with, intra_bucket_pairwise,
};
use super::params::{self, equity, fx, girr};
use super::types::{CorrelationScenario, FrtbRiskClass, FrtbSensitivities};
use finstack_quant_core::HashMap;
use std::collections::BTreeMap;

/// Compute the vega risk charge for a single risk class under one
/// correlation scenario.
///
/// # Arguments
///
/// * `risk_class` - FRTB risk class to calculate.
/// * `sensitivities` - Vega sensitivities using the scale convention
///   documented in [`super::types::FrtbSensitivities`].
/// * `scenario` - Low, medium, or high correlation scenario applied to the
///   prescribed vega correlations.
///
/// # Returns
///
/// The non-negative vega risk charge for `risk_class` under `scenario`.
///
/// # References
///
/// - BCBS FRTB Minimum Capital Requirements: `docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements`
///
pub fn vega_charge(
    risk_class: FrtbRiskClass,
    sensitivities: &FrtbSensitivities,
    scenario: CorrelationScenario,
) -> f64 {
    match risk_class {
        FrtbRiskClass::Girr => girr_vega(sensitivities, scenario),
        FrtbRiskClass::CsrNonSec => csr_nonsec_vega(sensitivities, scenario),
        FrtbRiskClass::CsrSecCtp => csr_sec_ctp_vega(sensitivities, scenario),
        FrtbRiskClass::CsrSecNonCtp => csr_sec_nonctp_vega(sensitivities, scenario),
        FrtbRiskClass::Equity => equity_vega(sensitivities, scenario),
        FrtbRiskClass::Commodity => commodity_vega(sensitivities, scenario),
        FrtbRiskClass::Fx => fx_vega(sensitivities, scenario),
    }
}

// GIRR vega

fn girr_vega(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.girr_vega.is_empty() {
        return 0.0;
    }

    // Group by currency bucket, carrying option maturity and underlying
    // tenor so intra-bucket correlation can reflect both dimensions per
    // MAR21.93.
    // Entry: (ws, option_maturity_years, underlying_tenor_years)
    type VegaEntry = (f64, (f64, f64)); // (weighted_vega, (option_maturity_years, underlying_tenor_years))
    let mut by_currency: HashMap<_, Vec<VegaEntry>> = HashMap::default();
    for ((ccy, opt_mat, und_tenor), vega) in &sens.girr_vega {
        let ws = vega * girr::GIRR_VEGA_RISK_WEIGHT;
        // Default to 5Y if the label is unrecognised — matches the GIRR
        // delta fallback and is dominated by the exp-decay elsewhere.
        let t_opt = girr::tenor_to_years(opt_mat).unwrap_or(5.0);
        let t_und = girr::tenor_to_years(und_tenor).unwrap_or(5.0);
        by_currency
            .entry(*ccy)
            .or_default()
            .push((ws, (t_opt, t_und)));
    }

    let inter_gamma = scenario.scale_correlation(girr::GIRR_INTER_BUCKET_CORRELATION);

    // Intra-bucket aggregation with MAR21.93 correlation
    // rho = min(rho_opt_mat * rho_under_mat, 1)
    // rho_opt_mat = exp(-alpha * |T_k - T_l| / min(T_k, T_l)), alpha=0.01
    // rho_under_mat = exp(-alpha * |U_k - U_l| / min(U_k, U_l)), alpha=0.01
    let bucket_results: Vec<_> = by_currency
        .values()
        .map(|entries| {
            intra_bucket_pairwise(entries, |&(t_opt_i, t_und_i), &(t_opt_j, t_und_j)| {
                let rho_opt = exp_decay_rho(t_opt_i, t_opt_j, 0.01);
                let rho_und = exp_decay_rho(t_und_i, t_und_j, 0.01);
                scenario.scale_correlation((rho_opt * rho_und).min(1.0))
            })
        })
        .collect();

    inter_bucket(&bucket_results, inter_gamma)
}

/// Exponential-decay correlation between two tenors / maturities.
///
/// `rho = exp(-alpha * |T_i - T_j| / min(T_i, T_j))` — the canonical
/// Basel tenor correlation form used across GIRR. `min(T_i, T_j)` is
/// floored at a small positive value to avoid division by zero for
/// zero-tenor cases.
fn exp_decay_rho(t_i: f64, t_j: f64, alpha: f64) -> f64 {
    let min_t = t_i.min(t_j).max(1.0 / 365.0);
    (-alpha * (t_i - t_j).abs() / min_t).exp()
}

// CSR vega (non-sec, sec CTP, sec non-CTP)

fn csr_nonsec_vega(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_bucketed_vega(&sens.csr_nonsec_vega, FrtbRiskClass::CsrNonSec, scenario)
}

fn csr_sec_ctp_vega(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_bucketed_vega(&sens.csr_sec_ctp_vega, FrtbRiskClass::CsrSecCtp, scenario)
}

fn csr_sec_nonctp_vega(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_bucketed_vega(
        &sens.csr_sec_nonctp_vega,
        FrtbRiskClass::CsrSecNonCtp,
        scenario,
    )
}

// Equity vega

fn equity_vega(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_bucketed_vega(&sens.equity_vega, FrtbRiskClass::Equity, scenario)
}

// Commodity vega

fn commodity_vega(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_bucketed_vega(&sens.commodity_vega, FrtbRiskClass::Commodity, scenario)
}

// FX vega

fn fx_vega(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    let entries: Vec<_> = sens
        .fx_vega
        .iter()
        .map(|((a, b, tenor), v)| {
            (
                v * fx::FX_VEGA_RISK_WEIGHT,
                (*a, *b, girr::tenor_to_years(tenor).unwrap_or(f64::NAN)),
            )
        })
        .collect();
    intra_bucket_pairwise(&entries, |(ai, bi, ti), (aj, bj, tj)| {
        let pair_rho = if ai == aj && bi == bj {
            1.0
        } else {
            fx::FX_INTER_PAIR_CORRELATION
        };
        scenario.scale_correlation(pair_rho * exp_decay_rho(*ti, *tj, 0.01))
    })
    .0
}

/// Generic bucketed vega aggregation for (name, bucket, tenor) keys.
fn generic_bucketed_vega(
    sensitivities: &HashMap<(String, u8, String), f64>,
    class: FrtbRiskClass,
    scenario: CorrelationScenario,
) -> f64 {
    let mut buckets = BTreeMap::<u8, Vec<(f64, (&str, f64))>>::new();
    for ((name, bucket, tenor), vega) in sensitivities {
        let weight = if class == FrtbRiskClass::Equity {
            equity::equity_vega_risk_weight(*bucket)
        } else {
            1.0
        };
        buckets.entry(*bucket).or_default().push((
            vega * weight,
            (name, girr::tenor_to_years(tenor).unwrap_or(f64::NAN)),
        ));
    }
    let ids: Vec<_> = buckets.keys().copied().collect();
    let results: Vec<_> = buckets
        .iter()
        .map(|(b, entries)| {
            if params::other_bucket(class, *b) {
                return (
                    entries.iter().map(|(w, _)| w.abs()).sum(),
                    entries.iter().map(|(w, _)| w).sum(),
                );
            }
            intra_bucket_pairwise(entries, |(ni, ti), (nj, tj)| {
                let rn = if ni == nj {
                    1.0
                } else {
                    params::name_correlation(class, *b)
                };
                scenario.scale_correlation(rn * exp_decay_rho(*ti, *tj, 0.01))
            })
        })
        .collect();
    if class == FrtbRiskClass::CsrSecNonCtp {
        return inter_bucket_plus_undiversified(&ids, &results, |b| params::other_bucket(class, b));
    }
    inter_bucket_with(&results, |i, j| {
        scenario.scale_correlation(params::bucket_correlation(class, ids[i], ids[j]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    /// Assert `actual` matches `expected` to a tight relative tolerance.
    fn assert_charge(actual: f64, expected: f64, what: &str) {
        let tol = expected.abs() * 1e-9;
        assert!(
            (actual - expected).abs() <= tol,
            "{what}: expected {expected}, got {actual}"
        );
    }

    // Each test below uses **non-unit, multi-factor** sensitivities so the
    // vega risk weight genuinely multiplies and the intra/inter-bucket
    // correlations genuinely aggregate. An earlier revision fed a single
    // 100.0 and asserted 100.0, which passed whether or not the weight was
    // applied at all.
    //
    // The 100% vega risk weight used by GIRR, CSR (all three sub-classes),
    // commodity and FX is the **published** value, not a placeholder:
    // MAR21.92 footnote 24 gives
    //   RW_k = min(RW_sigma * sqrt(LH_risk class) / sqrt(10), 100%)
    // with RW_sigma = 55%, and MAR21.92 Table 13 lists the liquidity horizon
    // per risk class (GIRR 60 days, CSR all sub-classes 120, commodity 120,
    // FX 40). Every one of those exceeds the 100% cap. Table 13 publishes the
    // resulting 100% directly. Only equity (large cap / indices, LH 20 days)
    // lands below the cap, at 77.78%.
    //
    // `params::tests::vega_risk_weights_match_the_mar21_92_liquidity_horizon_formula`
    // recomputes the constants from that formula.

    #[test]
    fn girr_vega_applies_weight_and_mar21_89_maturity_correlation() {
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_girr_vega(Currency::USD, "1Y", "5Y", 200_000.0);
        sens.add_girr_vega(Currency::USD, "5Y", "5Y", 100_000.0);

        let charge = vega_charge(FrtbRiskClass::Girr, &sens, CorrelationScenario::Medium);

        // WS_k = vega_k * 1.00 (GIRR vega risk weight, MAR21.92 Table 13).
        // Both factors share the 5Y underlying tenor, so rho_underlying = 1;
        // the option maturities differ, so
        //   rho_option = exp(-0.01 * |1 - 5| / min(1, 5)) = exp(-0.04)
        //              = 0.9607894391523232
        // and rho = min(rho_option * rho_underlying, 1) = 0.9607894391523232.
        //
        //   K^2 = 200_000^2 + 100_000^2
        //         + 2 * 0.9607894391523232 * 200_000 * 100_000
        //       = 4.0e10 + 1.0e10 + 3.8431577566092928e10
        //       = 8.843157756609293e10
        //   K   = 297_374.4736289464
        //
        // Single currency bucket, so the charge is K.
        assert_charge(charge, 297_374.473_628_946_4, "GIRR vega");
    }

    #[test]
    fn csr_nonsec_vega_applies_weight_and_bucket_aggregation() {
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.csr_nonsec_vega
            .insert(("ACME".to_string(), 1, "1Y".to_string()), 300_000.0);
        sens.csr_nonsec_vega
            .insert(("BETA".to_string(), 1, "1Y".to_string()), 100_000.0);
        sens.csr_nonsec_vega
            .insert(("GAMMA".to_string(), 3, "1Y".to_string()), 200_000.0);

        let charge = vega_charge(FrtbRiskClass::CsrNonSec, &sens, CorrelationScenario::Medium);

        // WS_k = vega_k * 1.00 (CSR non-sec vega risk weight, MAR21.92
        // Table 13, LH = 120 days -> capped at 100%).
        //
        // Intra-bucket uses rho_name = 35% (MAR21.54):
        //   K_1^2 = 300_000^2 + 100_000^2 + 2 * 0.35 * 300_000 * 100_000
        //         = 9.0e10 + 1.0e10 + 2.1e10 = 1.21e11
        //   K_1   = 347_850.5426185217, S_1 = 400_000
        //   K_3   = S_3 = 200_000
        //
        // Table 5 sectors 1/3 have gamma=10%: variance = 121e9 + 40e9 + 16e9.
        assert_charge(charge, 177_000_000_000.0_f64.sqrt(), "CSR non-sec vega");
    }

    #[test]
    fn commodity_vega_applies_weight_and_intra_bucket_correlation() {
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.commodity_vega
            .insert(("COAL_A".to_string(), 1, "1Y".to_string()), 200_000.0);
        sens.commodity_vega
            .insert(("COAL_B".to_string(), 1, "1Y".to_string()), 100_000.0);

        let charge = vega_charge(FrtbRiskClass::Commodity, &sens, CorrelationScenario::Medium);

        // WS_k = vega_k * 1.00 (commodity vega risk weight, MAR21.92
        // Table 13, LH = 120 days -> capped at 100%).
        //
        //   K_1^2 = 200_000^2 + 100_000^2 + 2 * 0.55 * 200_000 * 100_000
        //         = 4.0e10 + 1.0e10 + 2.2e10 = 7.2e10
        //   K_1   = 268_328.15729997476
        //
        // Single bucket, so the charge is K_1. 55% is the MAR21.83 Table 12
        // rho_cty for bucket 1 (correct for this bucket; see the deviation
        // notes in `params/commodity.rs` for the others).
        assert_charge(charge, 268_328.157_299_974_76, "commodity vega");
    }

    #[test]
    fn fx_vega_applies_weight_and_mar21_89_pair_correlation() {
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_fx_vega(Currency::EUR, Currency::USD, "1Y", 300_000.0);
        sens.add_fx_vega(Currency::USD, Currency::JPY, "1Y", 100_000.0);

        let charge = vega_charge(FrtbRiskClass::Fx, &sens, CorrelationScenario::Medium);

        // WS_k = vega_k * 1.00 (FX vega risk weight, MAR21.92 Table 13,
        // LH = 40 days -> 0.55 * sqrt(4) = 1.10, capped at 100%).
        //
        // FX is a single bucket with a uniform 60% correlation (MAR21.93):
        //   K^2 = (1 - 0.60) * (300_000^2 + 100_000^2)
        //         + 0.60 * (400_000)^2
        //       = 0.40 * 1.0e11 + 0.60 * 1.6e11 = 1.36e11
        //   K   = 368_781.7782917155
        assert_charge(charge, 368_781.778_291_715_5, "FX vega");
    }

    #[test]
    fn vega_charges_would_change_if_the_risk_weight_changed() {
        // Direct guard on the audit finding that the old unit-input tests
        // "would still pass if the weight were deleted". Scaling a
        // sensitivity scales the charge linearly, so a charge that ignored
        // the weight entirely could not track it.
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_fx_vega(Currency::EUR, Currency::USD, "1Y", 300_000.0);

        let base = vega_charge(FrtbRiskClass::Fx, &sens, CorrelationScenario::Medium);
        assert_charge(
            base,
            300_000.0 * fx::FX_VEGA_RISK_WEIGHT,
            "FX vega, one factor",
        );

        let mut doubled = FrtbSensitivities::new(Currency::USD);
        doubled.add_fx_vega(Currency::EUR, Currency::USD, "1Y", 600_000.0);
        let scaled = vega_charge(FrtbRiskClass::Fx, &doubled, CorrelationScenario::Medium);
        assert_charge(scaled, 2.0 * base, "FX vega is linear in the sensitivity");
    }

    #[test]
    fn empty_sensitivities_produce_zero_vega_for_every_risk_class() {
        let sens = FrtbSensitivities::new(Currency::USD);
        for &risk_class in FrtbRiskClass::ALL {
            let charge = vega_charge(risk_class, &sens, CorrelationScenario::Medium);
            assert_charge(charge, 0.0, &format!("{risk_class} vega with no inputs"));
        }
    }
}
