//! SIMM parameter-table completeness and PSD correlation validation.

use finstack_quant_core::{Error, FxHashMap, Result};

use super::types::SimmParams;
use super::validation::validate_non_negative;
use crate::SimmVersion;
use crate::types::{SimmCreditSector, SimmRiskClass, ordered_pair, ordered_tenor_pair};

pub(super) fn simm_cq_validation_sectors() -> [SimmCreditSector; 13] {
    use SimmCreditSector::*;
    [
        Sovereign,
        Financial,
        BasicMaterials,
        ConsumerGoods,
        TechnologyMedia,
        HealthCare,
        HighYieldSovereign,
        HighYieldFinancial,
        HighYieldBasicMaterials,
        HighYieldConsumerGoods,
        HighYieldTechnologyMedia,
        HighYieldHealthCare,
        Residual,
    ]
}

pub(crate) fn validate_simm_params(p: &SimmParams) -> Result<()> {
    if p.mpor_days == 0 {
        return Err(Error::Validation(
            "simm mpor_days must be greater than zero".to_string(),
        ));
    }
    fn complete_positive_map(
        field: &str,
        values: &FxHashMap<String, f64>,
        keys: &[&str],
    ) -> Result<()> {
        for key in keys {
            let value = values
                .get(*key)
                .copied()
                .ok_or_else(|| Error::Validation(format!("simm.{field} missing {key}")))?;
            if !value.is_finite() || value <= 0.0 {
                return Err(Error::Validation(format!(
                    "simm.{field}[{key}] must be finite and positive"
                )));
            }
        }
        Ok(())
    }
    for (field, values) in [
        ("ir_delta_weights", &p.ir_delta_weights),
        ("ir_delta_weights_low", &p.ir_delta_weights_low),
        ("ir_delta_weights_high", &p.ir_delta_weights_high),
    ] {
        complete_positive_map(field, values, crate::types::SIMM_TENORS)?;
    }
    for (field, values) in [
        (
            "ir_delta_concentration_thresholds",
            &p.ir_delta_concentration_thresholds,
        ),
        (
            "ir_vega_concentration_thresholds",
            &p.ir_vega_concentration_thresholds,
        ),
    ] {
        complete_positive_map(
            field,
            values,
            &["high", "regular_well_traded", "regular_less_traded", "low"],
        )?;
    }
    complete_positive_map(
        "fx_delta_concentration_thresholds",
        &p.fx_delta_concentration_thresholds,
        &["1", "2", "3"],
    )?;
    complete_positive_map(
        "fx_vega_concentration_thresholds",
        &p.fx_vega_concentration_thresholds,
        &["1_1", "1_2", "1_3", "2_2", "2_3", "3_3"],
    )?;
    let commodity_keys: Vec<String> = (1..=17).map(|id| id.to_string()).collect();
    let commodity_keys: Vec<&str> = commodity_keys.iter().map(String::as_str).collect();
    for (field, values) in [
        ("commodity_bucket_weights", &p.commodity_bucket_weights),
        (
            "commodity_delta_concentration_thresholds",
            &p.commodity_delta_concentration_thresholds,
        ),
        (
            "commodity_vega_concentration_thresholds",
            &p.commodity_vega_concentration_thresholds,
        ),
    ] {
        complete_positive_map(field, values, &commodity_keys)?;
    }
    for key in commodity_keys {
        let rho = p
            .commodity_intra_bucket_correlations
            .get(key)
            .copied()
            .ok_or_else(|| {
                Error::Validation(format!(
                    "simm.commodity_intra_bucket_correlations missing {key}"
                ))
            })?;
        if !(-1.0..=1.0).contains(&rho) {
            return Err(Error::Validation(format!(
                "invalid commodity correlation for {key}"
            )));
        }
    }
    for (field, value) in [
        (
            "ir_historical_volatility_ratio",
            p.ir_historical_volatility_ratio,
        ),
        (
            "equity_historical_volatility_ratio",
            p.equity_historical_volatility_ratio,
        ),
        (
            "fx_historical_volatility_ratio",
            p.fx_historical_volatility_ratio,
        ),
        (
            "commodity_historical_volatility_ratio",
            p.commodity_historical_volatility_ratio,
        ),
        ("fx_high_delta_weight", p.fx_high_delta_weight),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(Error::Validation(format!(
                "simm.{field} must be finite and positive"
            )));
        }
    }
    for (field, value) in [
        ("ir_subcurve_correlation", p.ir_subcurve_correlation),
        ("fx_regular_high_correlation", p.fx_regular_high_correlation),
        ("fx_high_high_correlation", p.fx_high_high_correlation),
        ("cq_same_issuer_correlation", p.cq_same_issuer_correlation),
        ("credit_residual_correlation", p.credit_residual_correlation),
    ] {
        if !(-1.0..=1.0).contains(&value) {
            return Err(Error::Validation(format!("simm.{field} must be in [-1,1]")));
        }
    }
    for class in [
        SimmRiskClass::InterestRate,
        SimmRiskClass::CreditQualifying,
        SimmRiskClass::CreditNonQualifying,
        SimmRiskClass::Equity,
        SimmRiskClass::Commodity,
        SimmRiskClass::Fx,
    ] {
        for (field, thresholds) in [
            ("concentration_thresholds", &p.concentration_thresholds),
            (
                "vega_concentration_thresholds",
                &p.vega_concentration_thresholds,
            ),
        ] {
            let value = thresholds
                .get(&class)
                .copied()
                .ok_or_else(|| Error::Validation(format!("simm.{field} missing {class}")))?;
            if !value.is_finite() || value <= 0.0 {
                return Err(Error::Validation(format!(
                    "simm.{field}[{class}] must be finite and positive"
                )));
            }
        }
    }
    let tenors: Vec<&String> = p.ir_delta_weights.keys().collect();
    let mut missing_pairs = Vec::new();
    for (i, tenor_i) in tenors.iter().enumerate() {
        for tenor_j in tenors.iter().skip(i + 1) {
            let key = ordered_tenor_pair(tenor_i, tenor_j);
            if !p.ir_tenor_correlations.contains_key(&key) {
                missing_pairs.push(key);
            }
        }
    }
    if !missing_pairs.is_empty() {
        let sample = missing_pairs
            .iter()
            .take(5)
            .map(|(a, b)| format!("({a},{b})"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Error::Validation(format!(
            "SIMM registry {:?}: ir_tenor_correlations missing {} tenor pair(s) (showing up to 5: {})",
            p.version,
            missing_pairs.len(),
            sample
        )));
    }
    validate_non_negative("simm.cnq_delta_weight", p.cnq_delta_weight)?;
    validate_non_negative("simm.equity_delta_weight", p.equity_delta_weight)?;
    validate_non_negative("simm.fx_delta_weight", p.fx_delta_weight)?;
    if !(-1.0..=1.0).contains(&p.fx_intra_bucket_correlation) {
        return Err(Error::Validation(
            "simm.fx_intra_bucket_correlation must be in [-1,1]".to_string(),
        ));
    }
    validate_non_negative("simm.ir_vega_weight", p.ir_vega_weight)?;
    validate_non_negative("simm.cq_vega_weight", p.cq_vega_weight)?;
    validate_non_negative("simm.cnq_vega_weight", p.cnq_vega_weight)?;
    validate_non_negative("simm.equity_vega_weight", p.equity_vega_weight)?;
    validate_non_negative("simm.fx_vega_weight", p.fx_vega_weight)?;
    validate_non_negative("simm.commodity_vega_weight", p.commodity_vega_weight)?;
    validate_cq_tables(p)?;
    validate_simm_correlations_psd(p)?;
    Ok(())
}

pub(super) fn validate_cq_tables(p: &SimmParams) -> Result<()> {
    let sectors = simm_cq_validation_sectors();
    for sector in sectors {
        let weight = p.cq_bucket_weights.get(&sector).copied().ok_or_else(|| {
            Error::Validation(format!(
                "SIMM registry {:?}: cq_bucket_weights missing {sector:?}",
                p.version
            ))
        })?;
        validate_non_negative(&format!("simm.cq_bucket_weights[{sector:?}]"), weight)?;

        let threshold = p
            .cq_concentration_thresholds
            .get(&sector)
            .copied()
            .ok_or_else(|| {
                Error::Validation(format!(
                    "SIMM registry {:?}: cq_concentration_thresholds missing {sector:?}",
                    p.version
                ))
            })?;
        validate_non_negative(
            &format!("simm.cq_concentration_thresholds[{sector:?}]"),
            threshold,
        )?;
    }

    for (i, &a) in sectors.iter().enumerate() {
        for &b in sectors.iter().skip(i + 1) {
            let key = ordered_pair(a, b);
            let rho = p.cq_inter_bucket_correlations.get(&key).copied();
            if p.version == SimmVersion::V2_6 && rho.is_none() {
                return Err(Error::Validation(format!(
                    "SIMM registry {:?}: cq_inter_bucket_correlations missing ({a:?},{b:?})",
                    p.version
                )));
            }
            if let Some(v) = rho
                && !(-1.0..=1.0).contains(&v)
            {
                return Err(Error::Validation(format!(
                    "simm.cq_inter_bucket_correlations[({a:?},{b:?})] must be in [-1,1]"
                )));
            }
        }
    }
    Ok(())
}

/// Validate that SIMM correlation matrices are positive semi-definite.
///
/// Hand-typed or overlay-supplied correlation matrices can become non-PSD through
/// a single typo (e.g. swapping two adjacent off-diagonals). A non-PSD correlation
/// matrix propagates silently through `K_b = sqrt(Σ WSᵢ² + Σ ρᵢⱼ WSᵢ WSⱼ)` — the
/// sum under the sqrt clamps to zero (or worse, aggregates a negative variance)
/// producing a regulatory miscalculation without an obvious failure signal.
///
/// Checks the three correlation matrices the SIMM calculator consumes:
///
/// 1. `risk_class_correlations` — 6×6 matrix across `SimmRiskClass`.
/// 2. `ir_tenor_correlations` — n×n matrix over the `ir_delta_weights` tenor keys.
/// 3. `cq_inter_bucket_correlations` — full credit-qualifying bucket matrix across
///    `SimmCreditSector`.
///
/// Missing off-diagonal entries are filled with the calculator's own fallback
/// value (see `SimmParams::correlation` / `cq_inter_bucket_correlation`) so the
/// validated matrix is the one the calculator will actually observe at runtime.
pub(super) fn validate_simm_correlations_psd(p: &SimmParams) -> Result<()> {
    use crate::types::SimmRiskClass::{
        Commodity, CreditNonQualifying, CreditQualifying, Equity, Fx, InterestRate,
    };

    // 1. Cross-risk-class matrix (6x6). Missing pairs fall back to 1.0 per the
    //    calculator's `SimmParams::correlation` — validate the effective matrix.
    let risk_classes = [
        InterestRate,
        CreditQualifying,
        CreditNonQualifying,
        Equity,
        Commodity,
        Fx,
    ];
    validate_dense_correlation_matrix(
        &risk_classes,
        "risk_class_correlations",
        p.version,
        |a, b| {
            let key = ordered_pair(*a, *b);
            p.risk_class_correlations.get(&key).copied().unwrap_or(1.0)
        },
    )?;

    // 2. IR tenor correlations — all pairs are guaranteed present by
    //    `validate_simm_params`; the fallback keeps this lower-level matrix
    //    validator independently total.
    let tenors: Vec<&str> = p
        .ir_delta_weights
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut tenors_sorted = tenors.clone();
    tenors_sorted.sort_unstable();
    validate_dense_correlation_matrix(
        &tenors_sorted,
        "ir_tenor_correlations",
        p.version,
        |a, b| {
            let key = ordered_tenor_pair(a, b);
            p.ir_tenor_correlations.get(&key).copied().unwrap_or(1.0)
        },
    )?;

    // 3. CQ inter-bucket correlations over the full v2.6 bucket set.
    //    Missing legacy pairs are treated as uncorrelated.
    let sectors = simm_cq_validation_sectors();
    validate_dense_correlation_matrix(
        &sectors,
        "cq_inter_bucket_correlations",
        p.version,
        |a, b| {
            let key = ordered_pair(*a, *b);
            p.cq_inter_bucket_correlations
                .get(&key)
                .copied()
                .unwrap_or(0.0)
        },
    )?;

    // 4. Commodity inter-bucket correlations (17x17). Already dense in
    //    row-major form, loaded from the SIMM registry, and validated directly
    //    without densification.
    let n = usize::from(crate::types::SIMM_COMMODITY_BUCKET_COUNT);
    let expected_len = n * n;
    if p.commodity_inter_bucket_correlations.len() != expected_len {
        return Err(Error::Validation(format!(
            "SIMM registry {:?}: commodity_inter_bucket_correlations has {} entries, expected {n}x{n} = {expected_len}",
            p.version,
            p.commodity_inter_bucket_correlations.len()
        )));
    }
    finstack_quant_core::math::linalg::validate_correlation_matrix(
        &p.commodity_inter_bucket_correlations,
        n,
    )
    .map_err(|_| {
        Error::Validation(format!(
            "SIMM registry {:?}: commodity_inter_bucket_correlations is not a valid {n}x{n} correlation matrix (failed diagonal / range / symmetry / PSD check)",
            p.version
        ))
    })?;

    Ok(())
}

/// Build a dense symmetric correlation matrix from a sparse lookup and validate it.
///
/// The diagonal is fixed at 1.0; off-diagonals are sourced from `lookup`. The resulting
/// matrix is handed to [`finstack_quant_core::math::linalg::validate_correlation_matrix`],
/// which checks diagonal exactness, off-diagonal range, symmetry, and positive
/// semi-definiteness.
///
/// On failure, the error message names the matrix and the SIMM version so a bad
/// registry overlay can be traced back to the exact parameter set at load time.
pub(super) fn validate_dense_correlation_matrix<T, F>(
    keys: &[T],
    matrix_name: &str,
    version: crate::SimmVersion,
    lookup: F,
) -> Result<()>
where
    F: Fn(&T, &T) -> f64,
{
    let n = keys.len();
    if n < 2 {
        return Ok(());
    }
    let mut matrix = vec![0.0; n * n];
    for (i, a) in keys.iter().enumerate() {
        matrix[i * n + i] = 1.0;
        for (j, b) in keys.iter().enumerate().skip(i + 1) {
            let rho = lookup(a, b);
            matrix[i * n + j] = rho;
            matrix[j * n + i] = rho;
        }
    }
    finstack_quant_core::math::linalg::validate_correlation_matrix(&matrix, n).map_err(|_| {
        Error::Validation(format!(
            "SIMM registry {version:?}: {matrix_name} is not a valid correlation matrix (failed diagonal / range / symmetry / PSD check — see finstack_quant_core::math::linalg::validate_correlation_matrix)"
        ))
    })
}

// Generic numeric range validators live in `registry::validation`.

// Convenience helpers for constructing Money amounts from defaults
