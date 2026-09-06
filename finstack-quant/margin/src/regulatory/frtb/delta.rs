//! FRTB delta risk charge computation.
//!
//! Two-level aggregation (intra-bucket then inter-bucket) with
//! correlation scenario scaling.

use super::aggregation::{
    inter_bucket, inter_bucket_plus_undiversified, inter_bucket_with, intra_bucket_pairwise,
};
use super::params::{self, commodity, csr, equity, fx, girr};
use super::types::{CorrelationScenario, FrtbRiskClass, FrtbSensitivities};
use finstack_quant_core::HashMap;
use std::collections::BTreeMap;

/// Compute the delta risk charge for a single risk class under one
/// correlation scenario.
///
/// Formula (two-level aggregation, MAR21.4-21.6):
///
/// 1. Weighted sensitivity: `WS_k = s_k * RW_k`
/// 2. Intra-bucket: `K_b = sqrt(max(sum_k sum_l rho_kl * WS_k * WS_l, 0))`
/// 3. Inter-bucket standard formula: try
///    `Delta² = sum_b K_b² + sum_{b != c} gamma_bc * S_b * S_c`
///    with uncapped `S_b = sum_k WS_k`. If this is non-negative, take
///    `Delta = sqrt(Delta²)`.
/// 4. Alternative formula (MAR21.6): if `Delta² < 0`, replace every `S_b`
///    with `S_b_capped = max(-K_b, min(S_b, K_b))` and recompute. The
///    alternative value is guaranteed non-negative.
///
/// # Arguments
///
/// * `risk_class` - FRTB risk class to calculate.
/// * `sensitivities` - Bucketed sensitivities using the scale convention
///   documented in [`super::types::FrtbSensitivities`].
/// * `scenario` - Low, medium, or high correlation scenario applied to the
///   prescribed correlation tables.
///
/// # Returns
///
/// The non-negative delta risk charge for `risk_class` under `scenario`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_margin::regulatory::frtb::delta::delta_charge;
/// use finstack_quant_margin::regulatory::frtb::{
///     CorrelationScenario, FrtbRiskClass, FrtbSensitivities,
/// };
///
/// let mut sensitivities = FrtbSensitivities::new(Currency::USD);
/// sensitivities.add_girr_delta(Currency::USD, "5Y", 1_000_000.0);
///
/// let charge = delta_charge(
///     FrtbRiskClass::Girr,
///     &sensitivities,
///     CorrelationScenario::Medium,
/// );
/// assert!(charge >= 0.0);
/// ```
///
/// # References
///
/// - BCBS FRTB Minimum Capital Requirements: `docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements`
///
pub fn delta_charge(
    risk_class: FrtbRiskClass,
    sensitivities: &FrtbSensitivities,
    scenario: CorrelationScenario,
) -> f64 {
    match risk_class {
        FrtbRiskClass::Girr => girr_delta(sensitivities, scenario),
        FrtbRiskClass::CsrNonSec => csr_nonsec_delta(sensitivities, scenario),
        FrtbRiskClass::CsrSecCtp => csr_sec_ctp_delta(sensitivities, scenario),
        FrtbRiskClass::CsrSecNonCtp => csr_sec_nonctp_delta(sensitivities, scenario),
        FrtbRiskClass::Equity => equity_delta(sensitivities, scenario),
        FrtbRiskClass::Commodity => commodity_delta(sensitivities, scenario),
        FrtbRiskClass::Fx => fx_delta(sensitivities, scenario),
    }
}

// GIRR delta

/// Internal tag discriminating the three kinds of GIRR risk factor.
///
/// Basel assigns separate risk weights and intra-bucket correlations to
/// yield-curve tenors, inflation and cross-currency basis. Modelling
/// them with a single `f64` tenor axis (and sentinel values for the
/// special factors) is fragile; the tag makes the discriminator explicit
/// inside the aggregation routine without altering the public
/// [`FrtbSensitivities`] map shapes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum GirrFactor {
    /// A specific yield-curve tenor, expressed in years.
    Tenor(f64),
    /// The currency's inflation risk factor.
    Inflation,
    /// The currency's cross-currency basis risk factor.
    XccyBasis,
}

fn girr_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.girr_delta.is_empty()
        && sens.girr_inflation_delta.is_empty()
        && sens.girr_xccy_basis_delta.is_empty()
    {
        return 0.0;
    }

    // Group by currency (bucket = currency for GIRR).
    let mut by_currency: HashMap<_, Vec<(f64, GirrFactor)>> = HashMap::default();
    for ((ccy, tenor), delta) in &sens.girr_delta {
        let rw = girr_risk_weight(tenor);
        let ws = delta * rw;
        let tenor_years = girr::tenor_to_years(tenor).unwrap_or(5.0);
        by_currency
            .entry(*ccy)
            .or_default()
            .push((ws, GirrFactor::Tenor(tenor_years)));
    }

    for (ccy, delta) in &sens.girr_inflation_delta {
        let ws = delta * girr::GIRR_INFLATION_RISK_WEIGHT;
        by_currency
            .entry(*ccy)
            .or_default()
            .push((ws, GirrFactor::Inflation));
    }
    for (ccy, delta) in &sens.girr_xccy_basis_delta {
        let ws = delta * girr::GIRR_XCCY_BASIS_RISK_WEIGHT;
        by_currency
            .entry(*ccy)
            .or_default()
            .push((ws, GirrFactor::XccyBasis));
    }

    // Intra-bucket aggregation per currency.
    let bucket_results: Vec<_> = by_currency
        .values()
        .map(|entries| {
            intra_bucket_pairwise(entries, |&fac_i, &fac_j| {
                intra_girr_correlation(fac_i, fac_j, scenario)
            })
        })
        .collect();

    // Inter-bucket aggregation across currencies.
    let gamma = scenario.scale_correlation(girr::GIRR_INTER_BUCKET_CORRELATION);
    inter_bucket(&bucket_results, gamma)
}

/// Intra-GIRR correlation between two risk factors (MAR21.46-21.49).
fn intra_girr_correlation(
    fac_i: GirrFactor,
    fac_j: GirrFactor,
    scenario: CorrelationScenario,
) -> f64 {
    use GirrFactor::{Inflation, Tenor, XccyBasis};
    // FRTB correlation table: each factor pair is listed explicitly per the
    // regulatory text even where values coincide.
    #[allow(clippy::match_same_arms)]
    let base_rho = match (fac_i, fac_j) {
        (Tenor(t_i), Tenor(t_j)) => girr::girr_tenor_correlation(t_i, t_j),
        (Inflation, Inflation) | (XccyBasis, XccyBasis) => 1.0,
        (Inflation, Tenor(_)) | (Tenor(_), Inflation) => girr::GIRR_INFLATION_CORRELATION,
        (XccyBasis, Tenor(_)) | (Tenor(_), XccyBasis) => girr::GIRR_XCCY_BASIS_CORRELATION,
        (Inflation, XccyBasis) | (XccyBasis, Inflation) => girr::GIRR_XCCY_BASIS_CORRELATION,
    };
    scenario.scale_correlation(base_rho)
}

// CSR Non-Sec delta

fn csr_nonsec_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    csr_bucketed_delta(
        &sens.csr_nonsec_delta,
        csr::csr_nonsec_risk_weight,
        FrtbRiskClass::CsrNonSec,
        scenario,
    )
}

fn csr_sec_ctp_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    csr_bucketed_delta(
        &sens.csr_sec_ctp_delta,
        csr::csr_sec_ctp_risk_weight,
        FrtbRiskClass::CsrSecCtp,
        scenario,
    )
}

fn csr_sec_nonctp_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    csr_bucketed_delta(
        &sens.csr_sec_nonctp_delta,
        csr::csr_sec_nonctp_risk_weight,
        FrtbRiskClass::CsrSecNonCtp,
        scenario,
    )
}

// Equity delta

fn equity_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    let mut buckets = BTreeMap::<u8, Vec<(f64, (&str, bool))>>::new();
    for (repo, map) in [(false, &sens.equity_delta), (true, &sens.equity_repo_delta)] {
        for ((name, bucket), delta) in map {
            let weight = equity::equity_risk_weight(*bucket) / if repo { 100.0 } else { 1.0 };
            buckets
                .entry(*bucket)
                .or_default()
                .push((delta * weight, (name, repo)));
        }
    }
    let ids: Vec<_> = buckets.keys().copied().collect();
    let results: Vec<_> = buckets
        .iter()
        .map(|(b, entries)| {
            if params::other_bucket(FrtbRiskClass::Equity, *b) {
                (
                    entries.iter().map(|(w, _)| w.abs()).sum(),
                    entries.iter().map(|(w, _)| w).sum(),
                )
            } else {
                intra_bucket_pairwise(entries, |(ni, ri), (nj, rj)| {
                    let name_rho = if ni == nj {
                        1.0
                    } else {
                        params::name_correlation(FrtbRiskClass::Equity, *b)
                    };
                    scenario.scale_correlation(name_rho * if ri == rj { 1.0 } else { 0.999 })
                })
            }
        })
        .collect();
    inter_bucket_with(&results, |i, j| {
        scenario.scale_correlation(params::bucket_correlation(
            FrtbRiskClass::Equity,
            ids[i],
            ids[j],
        ))
    })
}

// Commodity delta

fn commodity_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    csr_bucketed_delta(
        &sens.commodity_delta,
        commodity::commodity_risk_weight,
        FrtbRiskClass::Commodity,
        scenario,
    )
}

// FX delta

fn fx_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.fx_delta.is_empty() {
        return 0.0;
    }

    // FX is a single bucket with a uniform off-diagonal correlation. Use
    // the closed-form `Σ_ij ρ_ij · ws_i · ws_j = (1-ρ)·Σws² + ρ·(Σws)²`
    // to compute K_b in O(n) instead of O(n²).
    let rho = scenario.scale_correlation(fx::FX_INTER_PAIR_CORRELATION);
    let mut sum_ws = 0.0;
    let mut sum_ws_sq = 0.0;
    for d in sens.fx_delta.values() {
        let ws = d * fx::FX_DELTA_RISK_WEIGHT;
        sum_ws += ws;
        sum_ws_sq += ws * ws;
    }
    let k_squared = (1.0 - rho) * sum_ws_sq + rho * sum_ws * sum_ws;
    k_squared.max(0.0).sqrt()
}

/// GIRR risk weight lookup by tenor label.
///
/// Falls back to `1.1` (the FRTB d457 default at the long end of the
/// curve) for unknown tenors, with a single `tracing::warn!` so an
/// upstream typo or registry mismatch is visible rather than silently
/// producing the fallback.
fn girr_risk_weight(tenor: &str) -> f64 {
    static GIRR_RW_BY_TENOR: std::sync::LazyLock<finstack_quant_core::HashMap<&'static str, f64>> =
        std::sync::LazyLock::new(|| girr::GIRR_DELTA_RISK_WEIGHTS.iter().copied().collect());
    GIRR_RW_BY_TENOR.get(tenor).copied().unwrap_or_else(|| {
        tracing::warn!(
            tenor,
            "GIRR risk weight: unknown tenor label, falling back to 1.1"
        );
        1.1
    })
}

/// CSR-specific delta aggregation with intra-bucket
/// `rho = rho_name * rho_tenor * rho_basis`.
///
/// Per MAR21.54 (non-sec) and equivalent sections for sec CTP / sec non-CTP:
///
/// ```text
/// rho_kl = rho_name(name_k, name_l)
///        * rho_tenor(tenor_k, tenor_l)
///        * rho_basis(basis_k, basis_l)
/// ```
fn csr_bucketed_delta(
    sensitivities: &HashMap<(String, u8, String, String), f64>,
    risk_weight_fn: impl Fn(u8) -> f64,
    class: FrtbRiskClass,
    scenario: CorrelationScenario,
) -> f64 {
    let mut buckets = BTreeMap::<u8, Vec<(f64, (&str, &str, &str))>>::new();
    for ((name, bucket, tenor, basis), delta) in sensitivities {
        buckets
            .entry(*bucket)
            .or_default()
            .push((delta * risk_weight_fn(*bucket), (name, tenor, basis)));
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
            intra_bucket_pairwise(entries, |(ni, ti, bi), (nj, tj, bj)| {
                let rn = if ni == nj {
                    1.0
                } else {
                    params::name_correlation(class, *b)
                };
                let rt = if ti == tj {
                    1.0
                } else {
                    match class {
                        FrtbRiskClass::Commodity => 0.99,
                        FrtbRiskClass::CsrSecNonCtp => 0.8,
                        _ => 0.65,
                    }
                };
                scenario.scale_correlation(
                    rn * rt
                        * if bi == bj {
                            1.0
                        } else if class == FrtbRiskClass::CsrSecCtp {
                            0.99
                        } else {
                            0.999
                        },
                )
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
