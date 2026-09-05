use std::collections::{BTreeMap, BTreeSet};

use finstack_quant_core::types::IssuerId;
use finstack_quant_core::Result;

use super::config::BucketSizeThresholds;
use super::validation::validation_err;
use crate::factor::credit::hierarchy::{
    dimension_key, CreditHierarchySpec, FoldUpRecord, IssuerBetaMode, IssuerTags,
};

/// Inventory built in step 3, before any fold-up.
pub(super) struct BucketInventory {
    /// `bucket_paths[issuer][k]` = bucket path at level k (or error).
    pub(super) bucket_paths: BTreeMap<IssuerId, Vec<String>>,
    /// `bucket_sizes_per_level[k][bucket]` = count of **all** issuers in that
    /// bucket, regardless of [`IssuerBetaMode`]. Bucket factors are
    /// cross-sectional means over every member, so occupancy and the fold-up
    /// threshold both use the full membership. (Counting only `IssuerBeta`
    /// members made the threshold inert under the default `GloballyOff`
    /// policy, where every issuer is `BucketOnly`.)
    pub(super) bucket_sizes_per_level: Vec<BTreeMap<String, usize>>,
    /// Membership keyed by (level_index, bucket_path) → set of all member
    /// issuer IDs. Used by fold-up to decide whether to mark members as folded.
    bucket_members: Vec<BTreeMap<String, BTreeSet<IssuerId>>>,
    /// Observed values per dimension (for diagnostics).
    pub(super) tag_taxonomy: BTreeMap<String, BTreeSet<String>>,
}

pub(super) fn build_bucket_inventory(
    hierarchy: &CreditHierarchySpec,
    tags: &BTreeMap<IssuerId, IssuerTags>,
    modes: &BTreeMap<IssuerId, IssuerBetaMode>,
) -> Result<BucketInventory> {
    let num_levels = hierarchy.levels.len();
    let mut bucket_paths: BTreeMap<IssuerId, Vec<String>> = BTreeMap::new();
    let mut bucket_sizes_per_level: Vec<BTreeMap<String, usize>> =
        vec![BTreeMap::new(); num_levels];
    let mut bucket_members: Vec<BTreeMap<String, BTreeSet<IssuerId>>> =
        vec![BTreeMap::new(); num_levels];
    let mut tag_taxonomy: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    // Initialize tag taxonomy with the canonical dimension keys so that
    // dimensions present in the hierarchy but unseen still appear (with an
    // empty set) — useful for diagnostics consumers.
    for dim in &hierarchy.levels {
        tag_taxonomy
            .entry(dimension_key(dim).to_owned())
            .or_default();
    }

    // Every issuer in the panel is inventoried; `modes` supplies the universe
    // (its keys are the panel issuers) and is otherwise unused now that bucket
    // occupancy counts all member modes.
    for issuer in modes.keys() {
        let issuer_tags = tags.get(issuer).cloned().unwrap_or_default();
        // Tag values used by hierarchy dimensions must not contain the '.'
        // bucket-path separator: a dotted value would mis-segment
        // `synth_tags_from_path`, the fold-up parent computation, and the
        // matcher's factor IDs, silently corrupting factor identity.
        for dim in &hierarchy.levels {
            let key = dimension_key(dim);
            let Some(v) = issuer_tags.0.get(key) else {
                continue;
            };
            if v.contains('.') {
                return Err(validation_err(format!(
                    "CreditCalibrator: issuer {:?} tag {key:?} = {v:?} contains '.', \
                         which is reserved as the bucket-path separator",
                    issuer.as_str()
                )));
            }
            tag_taxonomy
                .entry(key.to_owned())
                .or_default()
                .insert(v.clone());
        }
        let paths = hierarchy.bucket_paths(&issuer_tags).map_err(|missing| {
            validation_err(format!(
                "CreditCalibrator: issuer {:?} is missing tag for dimension {:?}",
                issuer.as_str(),
                missing
            ))
        })?;
        for (k, path) in paths.iter().enumerate() {
            *bucket_sizes_per_level[k].entry(path.clone()).or_insert(0) += 1;
            bucket_members[k]
                .entry(path.clone())
                .or_default()
                .insert(issuer.clone());
        }
        bucket_paths.insert(issuer.clone(), paths);
    }

    Ok(BucketInventory {
        bucket_paths,
        bucket_sizes_per_level,
        bucket_members,
        tag_taxonomy,
    })
}

/// Mark which (issuer, level) pairs are folded up.
///
/// Returns `(folded, fold_up_records)` where `folded[issuer][k] == true` iff
/// the issuer's bucket at level `k` was below threshold.
///
/// Folding sets `β_k = 0`: the sparse bucket gets no factor and its common
/// variation flows into each member's idiosyncratic adder (see
/// [`FoldUpRecord`] for the correlation-understatement caveat). `folded_to`
/// names the deepest surviving ancestor for diagnostics only.
pub(super) fn apply_fold_up(
    inventory: &BucketInventory,
    thresholds: &BucketSizeThresholds,
) -> (BTreeMap<IssuerId, Vec<bool>>, Vec<FoldUpRecord>) {
    let num_levels = inventory.bucket_sizes_per_level.len();
    let mut folded: BTreeMap<IssuerId, Vec<bool>> = BTreeMap::new();
    for issuer in inventory.bucket_paths.keys() {
        folded.insert(issuer.clone(), vec![false; num_levels]);
    }
    let mut records: Vec<FoldUpRecord> = Vec::new();

    for k in 0..num_levels {
        let threshold = thresholds.threshold_for_level(k);
        for (bucket, members) in &inventory.bucket_members[k] {
            // Full bucket occupancy: the bucket factor is a cross-sectional
            // mean over every member, so the sparsity gate must see them all.
            let count = members.len();
            if count < threshold {
                let folded_to = if k == 0 {
                    "<root>".to_owned()
                } else {
                    // Strip the trailing "." segment — the parent path.
                    bucket
                        .rsplit_once('.')
                        .map(|x| x.0)
                        .unwrap_or("<root>")
                        .to_owned()
                };
                let reason = format!("fewer than {threshold} members ({count})");
                for member in members {
                    if let Some(flags) = folded.get_mut(member) {
                        flags[k] = true;
                    }
                    records.push(FoldUpRecord {
                        issuer_id: member.clone(),
                        level_index: k,
                        original_bucket: bucket.clone(),
                        folded_to: folded_to.clone(),
                        reason: reason.clone(),
                    });
                }
            }
        }
    }

    // Sort records for determinism: by (level_index, issuer_id).
    records.sort_by(|a, b| {
        a.level_index
            .cmp(&b.level_index)
            .then_with(|| a.issuer_id.as_str().cmp(b.issuer_id.as_str()))
    });

    (folded, records)
}
