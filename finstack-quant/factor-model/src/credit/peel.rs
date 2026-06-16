//! Shared single-observation credit hierarchy peel helper.

use std::collections::BTreeMap;

use finstack_quant_core::types::IssuerId;

use super::hierarchy::IssuerBetas;

/// Output from peeling one cross-section of issuer spreads.
pub(crate) struct SingleObservationPeel {
    /// Per-level bucket values, in hierarchy level order.
    pub(crate) by_level: Vec<BTreeMap<String, f64>>,
    /// Per-issuer residual after generic and level factors are peeled.
    pub(crate) adder: BTreeMap<IssuerId, f64>,
}

/// Peel one observed spread cross-section into hierarchy-level factors.
///
/// This is the common math used by calibration anchoring and decomposition:
/// subtract the generic contribution, compute per-level bucket means from the
/// current residuals, subtract each issuer's beta-scaled bucket contribution,
/// and leave the remaining residual as the issuer adder.
pub(crate) fn peel_single_observation(
    observed_spreads: &BTreeMap<IssuerId, f64>,
    observed_generic: f64,
    betas: &BTreeMap<IssuerId, IssuerBetas>,
    bucket_paths: &BTreeMap<IssuerId, Vec<String>>,
    folded: &BTreeMap<IssuerId, Vec<bool>>,
    num_levels: usize,
) -> SingleObservationPeel {
    let mut residuals: BTreeMap<IssuerId, f64> = BTreeMap::new();
    for (issuer, spread) in observed_spreads {
        let beta_pc = betas.get(issuer).map_or(1.0, |row| row.pc);
        residuals.insert(issuer.clone(), spread - beta_pc * observed_generic);
    }

    let mut by_level = Vec::with_capacity(num_levels);
    #[allow(clippy::needless_range_loop)]
    for k in 0..num_levels {
        let mut sums: BTreeMap<String, (f64, usize)> = BTreeMap::new();
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
            let entry = sums.entry(path.clone()).or_insert((0.0, 0));
            entry.0 += residual;
            entry.1 += 1;
        }

        let values: BTreeMap<String, f64> = sums
            .into_iter()
            .map(|(bucket, (sum, count))| (bucket, sum / count as f64))
            .collect();

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
            if let Some(prev) = residuals.get(issuer).copied() {
                residuals.insert(issuer.clone(), prev - beta_k * level_value);
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
