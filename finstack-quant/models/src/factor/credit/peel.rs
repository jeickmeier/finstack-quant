//! Shared single-observation credit hierarchy peel helper.

use std::collections::BTreeMap;

use finstack_quant_core::types::IssuerId;

use super::calibration::BucketWeighting;
use super::hierarchy::IssuerBetas;

/// Spread duration (years) an issuer needs for DTS bucket weighting.
///
/// # Arguments
///
/// * `issuer` - Issuer whose duration is required.
/// * `spread_durations` - Per-issuer spread durations in years.
///
/// # Errors
///
/// Returns a diagnostic string when the duration is missing, non-finite, or
/// `<= 0`.
pub(crate) fn dts_spread_duration(
    issuer: &IssuerId,
    spread_durations: &BTreeMap<IssuerId, f64>,
) -> Result<f64, String> {
    let sd = spread_durations.get(issuer).copied().ok_or_else(|| {
        format!(
            "dts weighting requires spread_duration (years, > 0) for issuer {:?}",
            issuer.as_str()
        )
    })?;
    if !sd.is_finite() || sd <= 0.0 {
        return Err(format!(
            "spread_duration for issuer {:?} must be finite and > 0 years, got {sd}",
            issuer.as_str()
        ));
    }
    Ok(sd)
}

/// Output from peeling one cross-section of issuer spreads.
pub(crate) struct SingleObservationPeel {
    /// Per-level bucket values, in hierarchy level order.
    pub(crate) by_level: Vec<BTreeMap<String, f64>>,
    /// Per-issuer residual after generic and level factors are peeled.
    pub(crate) adder: BTreeMap<IssuerId, f64>,
}

/// Inputs for [`peel_single_observation`].
pub(crate) struct PeelSingleObservation<'a> {
    /// Observed issuer spreads in **bp**.
    pub observed_spreads: &'a BTreeMap<IssuerId, f64>,
    /// Observed generic factor in **bp**.
    pub observed_generic: f64,
    /// Per-issuer betas (unit β for runtime-only names). Keys are borrowed
    /// so callers can share a single unit-β row across runtime issuers.
    pub betas: &'a BTreeMap<&'a IssuerId, &'a IssuerBetas>,
    /// Dotted bucket path per issuer at each hierarchy level.
    pub bucket_paths: &'a BTreeMap<IssuerId, Vec<String>>,
    /// Folded levels per issuer (`true` → skip that level, `β_k = 0`).
    pub folded: &'a BTreeMap<IssuerId, Vec<bool>>,
    /// Hierarchy depth.
    pub num_levels: usize,
    /// Per-issuer bucket weights (equal `1.0` or as-of / current DTS).
    ///
    /// Normalized within each bucket. Missing or non-positive weights skip
    /// that issuer from the bucket mean.
    pub weights: &'a BTreeMap<IssuerId, f64>,
}

/// Peel one observed spread cross-section into hierarchy-level factors.
///
/// This is the common math used by calibration anchoring and decomposition:
/// subtract the generic contribution, compute per-level **weighted** bucket
/// means from the current residuals, subtract each issuer's beta-scaled
/// bucket contribution, and leave the remaining residual as the issuer adder.
pub(crate) fn peel_single_observation(params: PeelSingleObservation<'_>) -> SingleObservationPeel {
    let PeelSingleObservation {
        observed_spreads,
        observed_generic,
        betas,
        bucket_paths,
        folded,
        num_levels,
        weights,
    } = params;

    let mut residuals: BTreeMap<IssuerId, f64> = BTreeMap::new();
    for (issuer, spread) in observed_spreads {
        let beta_pc = betas.get(issuer).map_or(1.0, |row| row.pc);
        residuals.insert(issuer.clone(), spread - beta_pc * observed_generic);
    }

    let mut by_level = Vec::with_capacity(num_levels);
    #[allow(clippy::needless_range_loop)]
    for k in 0..num_levels {
        let mut sums: BTreeMap<&str, (f64, f64)> = BTreeMap::new();
        let mut fallback_sums: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
        for issuer in observed_spreads.keys() {
            if is_folded(folded, issuer, k) {
                continue;
            }
            let Some(paths) = bucket_paths.get(issuer) else {
                continue;
            };
            let Some(path) = paths.get(k) else {
                continue;
            };
            let Some(residual) = residuals.get(issuer).copied() else {
                continue;
            };
            let fb = fallback_sums.entry(path.as_str()).or_insert((0.0, 0));
            fb.0 += residual;
            fb.1 += 1;
            let Some(weight) = weights.get(issuer).copied() else {
                continue;
            };
            if weight <= 0.0 {
                continue;
            }
            let entry = sums.entry(path.as_str()).or_insert((0.0, 0.0));
            entry.0 += residual * weight;
            entry.1 += weight;
        }

        let mut values: BTreeMap<String, f64> = sums
            .into_iter()
            .filter_map(|(bucket, (sum, wsum))| {
                (wsum > 0.0).then_some((bucket.to_owned(), sum / wsum))
            })
            .collect();
        for (bucket, (sum, count)) in fallback_sums {
            if count > 0 && !values.contains_key(bucket) {
                values.insert(bucket.to_owned(), sum / count as f64);
            }
        }

        for issuer in observed_spreads.keys() {
            if is_folded(folded, issuer, k) {
                continue;
            }
            let Some(paths) = bucket_paths.get(issuer) else {
                continue;
            };
            let Some(path) = paths.get(k) else {
                continue;
            };
            let level_value = values.get(path).copied().unwrap_or(0.0);
            let beta_k = betas
                .get(issuer)
                .and_then(|row| row.levels.get(k).copied())
                .unwrap_or(1.0);
            if let Some(prev) = residuals.get_mut(issuer) {
                *prev -= beta_k * level_value;
            }
        }

        by_level.push(values);
    }

    SingleObservationPeel {
        by_level,
        adder: residuals,
    }
}

fn is_folded(folded: &BTreeMap<IssuerId, Vec<bool>>, issuer: &IssuerId, k: usize) -> bool {
    folded
        .get(issuer)
        .and_then(|levels| levels.get(k))
        .copied()
        .unwrap_or(false)
}

/// Per-issuer bucket weights for [`BucketWeighting`].
///
/// # Arguments
///
/// * `weighting` - Equal (`1.0` each) or DTS (`SD_years × spread_bp`).
/// * `issuers` - Universe to weight (typically the observed / history set).
/// * `spread_durations` - Duration in years, required and `> 0` when `Dts`.
/// * `spreads_bp` - Spreads in **bp** used to form DTS.
///
/// # Errors
///
/// Returns a diagnostic string when DTS is selected and a duration is
/// missing, non-finite, or `<= 0`, or when `SD × spread_bp` is not `> 0`.
pub(crate) fn issuer_bucket_weights<'a>(
    weighting: BucketWeighting,
    issuers: impl Iterator<Item = &'a IssuerId>,
    spread_durations: &BTreeMap<IssuerId, f64>,
    spreads_bp: &BTreeMap<IssuerId, f64>,
) -> Result<BTreeMap<IssuerId, f64>, String> {
    let mut weights = BTreeMap::new();
    for issuer in issuers {
        let weight = match weighting {
            BucketWeighting::Equal => 1.0,
            BucketWeighting::Dts => {
                let sd = dts_spread_duration(issuer, spread_durations)?;
                let spread = spreads_bp.get(issuer).copied().ok_or_else(|| {
                    format!(
                        "dts weighting requires a spread (bp) for issuer {:?}",
                        issuer.as_str()
                    )
                })?;
                let dts = sd * spread;
                if !dts.is_finite() || dts <= 0.0 {
                    return Err(format!(
                        "dts for issuer {:?} must be > 0 (spread_duration {sd} × spread_bp \
                         {spread} = {dts})",
                        issuer.as_str()
                    ));
                }
                dts
            }
        };
        weights.insert(issuer.clone(), weight);
    }
    Ok(weights)
}
