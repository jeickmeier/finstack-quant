//! FRTB delta risk charge computation.
//!
//! Two-level aggregation (intra-bucket then inter-bucket) with
//! correlation scenario scaling.

use super::aggregation::{inter_bucket, intra_bucket_uniform_map};
use super::params::{commodity, csr, equity, fx, girr};
use super::types::{CorrelationScenario, FrtbRiskClass, FrtbSensitivities};
use finstack_quant_core::HashMap;

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
/// sensitivities.add_girr_delta(Currency::USD, "5y", 1_000_000.0);
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
/// - BCBS FRTB Minimum Capital Requirements:
///   `docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements`
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

// ---------------------------------------------------------------------------
// GIRR delta
// ---------------------------------------------------------------------------

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
    let mut bucket_results: Vec<(f64, f64)> = Vec::new(); // (K_b, S_b uncapped)
    for entries in by_currency.values() {
        let mut k_squared = 0.0;
        for (i, &(ws_i, fac_i)) in entries.iter().enumerate() {
            for (j, &(ws_j, fac_j)) in entries.iter().enumerate() {
                let rho = if i == j {
                    1.0
                } else {
                    intra_girr_correlation(fac_i, fac_j, scenario)
                };
                k_squared += rho * ws_i * ws_j;
            }
        }
        let k_b = k_squared.max(0.0).sqrt();
        let s_b: f64 = entries.iter().map(|(ws, _)| ws).sum();
        bucket_results.push((k_b, s_b));
    }

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

// ---------------------------------------------------------------------------
// CSR Non-Sec delta
// ---------------------------------------------------------------------------

fn csr_nonsec_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.csr_nonsec_delta.is_empty() {
        return 0.0;
    }
    csr_bucketed_delta(
        &sens.csr_nonsec_delta,
        csr::csr_nonsec_risk_weight,
        csr::CSR_NONSEC_INTRA_BUCKET_NAME_CORRELATION,
        csr::CSR_NONSEC_INTRA_BUCKET_TENOR_CORRELATION,
        csr::CSR_NONSEC_INTER_BUCKET_CORRELATION,
        scenario,
    )
}

fn csr_sec_ctp_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.csr_sec_ctp_delta.is_empty() {
        return 0.0;
    }
    // Sec CTP intra-bucket correlation is defined uniformly (MAR21.78),
    // so name and tenor factors are both equal to the prescribed constant
    // and the triple-rho reduces to a single-rho aggregation — but we
    // route through the same helper for consistency.
    csr_bucketed_delta(
        &sens.csr_sec_ctp_delta,
        csr::csr_sec_ctp_risk_weight,
        csr::CSR_SEC_CTP_INTRA_BUCKET_CORRELATION,
        1.0,
        csr::CSR_SEC_CTP_INTER_BUCKET_CORRELATION,
        scenario,
    )
}

fn csr_sec_nonctp_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.csr_sec_nonctp_delta.is_empty() {
        return 0.0;
    }
    csr_bucketed_delta(
        &sens.csr_sec_nonctp_delta,
        csr::csr_sec_nonctp_risk_weight,
        csr::CSR_SEC_NONCTP_INTRA_BUCKET_CORRELATION,
        1.0,
        csr::CSR_SEC_NONCTP_INTER_BUCKET_CORRELATION,
        scenario,
    )
}

// ---------------------------------------------------------------------------
// Equity delta
// ---------------------------------------------------------------------------

fn equity_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.equity_delta.is_empty() {
        return 0.0;
    }

    // Group by bucket.
    let mut by_bucket: HashMap<u8, Vec<f64>> = HashMap::default();
    for ((_, bucket), delta) in &sens.equity_delta {
        let rw = equity::equity_risk_weight(*bucket);
        let ws = delta * rw;
        by_bucket.entry(*bucket).or_default().push(ws);
    }

    let intra_rho = scenario.scale_correlation(equity::EQUITY_INTRA_BUCKET_CORRELATION);
    let inter_gamma = scenario.scale_correlation(equity::EQUITY_INTER_BUCKET_CORRELATION);

    let bucket_results = intra_bucket_uniform_map(&by_bucket, intra_rho);
    inter_bucket(&bucket_results, inter_gamma)
}

// ---------------------------------------------------------------------------
// Commodity delta
// ---------------------------------------------------------------------------

fn commodity_delta(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.commodity_delta.is_empty() {
        return 0.0;
    }

    let mut by_bucket: HashMap<u8, Vec<f64>> = HashMap::default();
    for ((_, bucket, _), delta) in &sens.commodity_delta {
        let rw = commodity::commodity_risk_weight(*bucket);
        let ws = delta * rw;
        by_bucket.entry(*bucket).or_default().push(ws);
    }

    let intra_rho = scenario.scale_correlation(commodity::COMMODITY_INTRA_BUCKET_CORRELATION);
    let inter_gamma = scenario.scale_correlation(commodity::COMMODITY_INTER_BUCKET_CORRELATION);

    let bucket_results = intra_bucket_uniform_map(&by_bucket, intra_rho);
    inter_bucket(&bucket_results, inter_gamma)
}

// ---------------------------------------------------------------------------
// FX delta
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

/// CSR-specific delta aggregation with full intra-bucket `rho = rho_name * rho_tenor`.
///
/// Per MAR21.54 (non-sec) and equivalent sections for sec CTP / sec non-CTP,
/// the intra-bucket correlation between two weighted sensitivities within
/// the same bucket factorises:
///
/// ```text
/// rho_kl = rho_name(name_k, name_l)
///        * rho_tenor(tenor_k, tenor_l)
///        * rho_basis(basis_k, basis_l)
/// ```
///
/// where each component is 1 if the two dimensions match and the prescribed
/// correlation otherwise. Because the sensitivity map carries only
/// `(name, bucket, tenor)` and has no explicit basis dimension, this
/// implementation assumes a single basis per record and computes
/// `rho_name * rho_tenor` only — callers mixing bond vs CDS basis should
/// pre-apply the `rho_basis` factor (e.g. 0.999 for non-sec) to their
/// sensitivities before calling, or the capital number will be slightly
/// overstated (less offset than Basel allows).
fn csr_bucketed_delta(
    sensitivities: &HashMap<(String, u8, String), f64>,
    risk_weight_fn: impl Fn(u8) -> f64,
    name_correlation: f64,
    tenor_correlation: f64,
    inter_bucket_gamma: f64,
    scenario: CorrelationScenario,
) -> f64 {
    // Group weighted sensitivities by bucket, preserving name + tenor.
    type Entry = (String, String, f64); // (name, tenor, weighted_sensitivity)
    let mut by_bucket: HashMap<u8, Vec<Entry>> = HashMap::default();
    for ((name, bucket, tenor), delta) in sensitivities {
        let rw = risk_weight_fn(*bucket);
        let ws = delta * rw;
        by_bucket
            .entry(*bucket)
            .or_default()
            .push((name.clone(), tenor.clone(), ws));
    }

    let scaled_name = scenario.scale_correlation(name_correlation);
    let scaled_tenor = scenario.scale_correlation(tenor_correlation);
    let scaled_inter = scenario.scale_correlation(inter_bucket_gamma);

    // Intra-bucket aggregation with factorised rho.
    let mut bucket_results: Vec<(f64, f64)> = Vec::new();
    for entries in by_bucket.values() {
        let mut k_squared = 0.0;
        for (i, (name_i, tenor_i, ws_i)) in entries.iter().enumerate() {
            for (j, (name_j, tenor_j, ws_j)) in entries.iter().enumerate() {
                let rho = if i == j {
                    1.0
                } else {
                    let rn = if name_i == name_j { 1.0 } else { scaled_name };
                    let rt = if tenor_i == tenor_j {
                        1.0
                    } else {
                        scaled_tenor
                    };
                    rn * rt
                };
                k_squared += rho * ws_i * ws_j;
            }
        }
        let k_b = k_squared.max(0.0).sqrt();
        let s_b: f64 = entries.iter().map(|(_, _, ws)| *ws).sum();
        bucket_results.push((k_b, s_b));
    }

    inter_bucket(&bucket_results, scaled_inter)
}
