//! Pure decomposition of issuer spreads into hierarchy-level factor values.
//!
//! Given a calibrated [`CreditFactorModel`] and observed issuer spreads at a
//! point in time, [`decompose_levels`] peels off the generic (PC) component
//! and each hierarchy level in turn, producing a [`LevelsAtDate`] snapshot.
//!
//! [`decompose_period`] then differences two [`LevelsAtDate`] snapshots into
//! a [`PeriodDecomposition`] that preserves the linear reconciliation
//! invariant
//!
//! ```text
//! ΔS_i ≡ β_i^PC · Δgeneric
//!        + Σ_k β_i^level_k · ΔL_level_k(g_i^k)
//!        + Δadder_i
//! ```
//!
//! to absolute tolerance `1e-10` for every issuer present in both snapshots.
//!
//! # Determinism
//!
//! All keyed maps are [`BTreeMap`] so the output is byte-stable for byte-stable
//! input. The function performs no I/O and reads no global state.

use std::collections::{BTreeMap, BTreeSet};

use finstack_core::dates::Date;
use finstack_core::factor_model::credit_hierarchy::{
    CreditFactorModel, HierarchyDimension, IssuerBetaRow, IssuerBetas, IssuerTags,
};
use finstack_core::types::IssuerId;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Per-level bucket values produced by a single decomposition.
///
/// The `values` map is keyed by the dotted bucket path (e.g. `"IG.EU.FIN"`),
/// matching the convention used elsewhere in the credit hierarchy artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelValuesAtDate {
    /// Zero-based index of this level inside [`finstack_core::factor_model::credit_hierarchy::CreditHierarchySpec::levels`].
    pub level_index: usize,
    /// Dimension identifier for this level, copied from the hierarchy spec.
    pub dimension: HierarchyDimension,
    /// Bucket → mean residual at this level, computed across all issuers
    /// in the input spread set whose tags placed them in that bucket.
    pub values: BTreeMap<String, f64>,
}

/// Snapshot of all hierarchy-level factor values at a single date,
/// produced from observed issuer spreads.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelsAtDate {
    /// Date the spreads were observed.
    pub date: Date,
    /// Generic (PC) factor value at `date`. Equal to the input
    /// `observed_generic` and propagated unchanged.
    pub generic: f64,
    /// Per-level bucket values, in hierarchy spec order.
    pub by_level: Vec<LevelValuesAtDate>,
    /// Per-issuer residual after peeling generic + every level.
    pub adder: BTreeMap<IssuerId, f64>,
}

/// Per-level bucket-value deltas produced by [`decompose_period`].
#[derive(Debug, Clone, PartialEq)]
pub struct LevelValuesDelta {
    /// Zero-based level index, mirroring [`LevelValuesAtDate::level_index`].
    pub level_index: usize,
    /// Dimension for this level.
    pub dimension: HierarchyDimension,
    /// Bucket → `(to.values[bucket] - from.values[bucket])`. Only buckets
    /// present in **both** snapshots are included.
    pub deltas: BTreeMap<String, f64>,
}

/// Difference between two [`LevelsAtDate`] snapshots.
///
/// Only issuers and buckets present in **both** snapshots are included so that
/// the linear reconciliation invariant on `ΔS_i` holds for every entry.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodDecomposition {
    /// Earlier snapshot date.
    pub from: Date,
    /// Later snapshot date.
    pub to: Date,
    /// `to.generic - from.generic`.
    pub d_generic: f64,
    /// Per-level bucket deltas. Same length and same `level_index` /
    /// `dimension` ordering as [`LevelsAtDate::by_level`].
    pub by_level: Vec<LevelValuesDelta>,
    /// Per-issuer adder deltas (`to - from`), restricted to issuers present
    /// in both snapshots.
    pub d_adder: BTreeMap<IssuerId, f64>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Failure modes for the decomposition routines.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum DecompositionError {
    /// An issuer appeared in `observed_spreads` but had no
    /// [`IssuerBetaRow`] in the model and no entry in `runtime_tags`.
    #[error(
        "issuer {issuer_id:?} is not in CreditFactorModel.issuer_betas and no \
         runtime_tags entry was supplied"
    )]
    UnknownIssuer {
        /// The issuer that could not be resolved.
        issuer_id: IssuerId,
    },
    /// An issuer (model-resident or runtime-supplied) was missing the tag for
    /// a hierarchy dimension.
    #[error("issuer {issuer_id:?} is missing the tag for dimension {dimension:?}")]
    MissingTag {
        /// The issuer with the missing tag.
        issuer_id: IssuerId,
        /// Canonical key (per [`finstack_core::factor_model::credit_hierarchy::dimension_key`]) of the missing dimension.
        dimension: String,
    },
    /// The model is internally inconsistent — typically `IssuerBetas.levels`
    /// or `LevelsAtAnchor.by_level` does not match `hierarchy.levels.len()`.
    #[error("CreditFactorModel is inconsistent: {reason}")]
    ModelInconsistent {
        /// Free-form diagnostic explaining the mismatch.
        reason: String,
    },
    /// The two snapshots passed to [`decompose_period`] disagree on the number
    /// of hierarchy levels or on the dimension assigned to each level.
    #[error("snapshot shape mismatch: {reason}")]
    SnapshotShapeMismatch {
        /// Free-form diagnostic explaining the mismatch.
        reason: String,
    },
    /// The two snapshots passed to [`decompose_period`] are out of order
    /// (`from.date > to.date`).
    #[error("decompose_period requires from.date <= to.date, got from={from:?} to={to:?}")]
    DateMismatchInPeriod {
        /// Earlier-but-supplied-second date.
        from: Date,
        /// Later-but-supplied-first date.
        to: Date,
    },
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Indexes every issuer beta row by `issuer_id` for O(log n) lookup.
fn index_issuer_betas(model: &CreditFactorModel) -> BTreeMap<&IssuerId, &IssuerBetaRow> {
    let mut idx = BTreeMap::new();
    for row in &model.issuer_betas {
        idx.insert(&row.issuer_id, row);
    }
    idx
}

/// Default unit-beta values used when an issuer is decomposed under
/// "bucket-only" semantics (no model row, runtime tags only).
fn unit_betas(num_levels: usize) -> IssuerBetas {
    IssuerBetas {
        pc: 1.0,
        levels: vec![1.0; num_levels],
    }
}

// ---------------------------------------------------------------------------
// decompose_levels
// ---------------------------------------------------------------------------

/// Decompose observed issuer spreads at `as_of` into per-level factor values
/// and per-issuer residual adders.
///
/// # Algorithm
///
/// 1. Validate the model: every `IssuerBetas.levels` must have length equal to
///    `hierarchy.levels.len()`.
/// 2. **PC peel.** For each issuer `i` with observed spread `S_i`, compute
///    `r_1_i = S_i - β_i^PC · observed_generic`.
/// 3. **Level peel.** For each level `k` in `0..L`:
///    - Group issuers by their bucket path at level `k`
///      (e.g. `"IG.EU"` for level 1).
///    - Compute `L_k(g) = mean over issuers in g of r_(k+1)_i`.
///    - Update `r_(k+2)_i = r_(k+1)_i - β_i^level_k · L_k(g_i^k)`.
/// 4. The remaining residual `r_(L+1)_i` is the per-issuer adder.
///
/// # Universe-difference handling (design §5.4)
///
/// - An issuer in `observed_spreads` but absent from `model.issuer_betas` is
///   accepted iff `runtime_tags` contains its tag map. Such an issuer is
///   treated as `BucketOnly` (`β = 1`).
/// - An issuer in `model.issuer_betas` but absent from `observed_spreads` is
///   silently skipped.
/// - A hierarchy bucket present in `model.anchor_state` but with no current
///   issuers is omitted from the output (rather than emitting `0.0`),
///   so that callers can distinguish "no data" from "data, value 0".
///
/// Note: `decompose_levels` only processes `observed_spreads`; an issuer in
/// `observed_spreads` is always in some bucket of size ≥ 1 (its own residual).
/// The §5.4 "β=0 fallback for empty bucket" applies at attribution time
/// (PR-7/8), not here.
///
/// # Errors
///
/// - [`DecompositionError::ModelInconsistent`] when an issuer's `betas.levels`
///   length disagrees with `hierarchy.levels.len()`.
/// - [`DecompositionError::UnknownIssuer`] when an issuer is in
///   `observed_spreads` but neither in the model nor in `runtime_tags`.
/// - [`DecompositionError::MissingTag`] when an issuer's tags do not cover
///   every hierarchy dimension.
pub fn decompose_levels(
    model: &CreditFactorModel,
    observed_spreads: &BTreeMap<IssuerId, f64>,
    observed_generic: f64,
    as_of: Date,
    runtime_tags: Option<&BTreeMap<IssuerId, IssuerTags>>,
) -> Result<LevelsAtDate, DecompositionError> {
    let num_levels = model.hierarchy.levels.len();
    let beta_idx = index_issuer_betas(model);

    // ------------------------------------------------------------------
    // Step 0 — defensive shape check on model.issuer_betas. Cheap: O(N).
    // ------------------------------------------------------------------
    for row in &model.issuer_betas {
        if row.betas.levels.len() != num_levels {
            return Err(DecompositionError::ModelInconsistent {
                reason: format!(
                    "issuer {:?}: betas.levels.len() = {}, expected {num_levels}",
                    row.issuer_id.as_str(),
                    row.betas.levels.len()
                ),
            });
        }
    }
    let mut folded_pairs: BTreeSet<(IssuerId, usize)> = BTreeSet::new();
    for record in &model.diagnostics.fold_ups {
        if record.level_index >= num_levels {
            return Err(DecompositionError::ModelInconsistent {
                reason: format!(
                    "fold-up record for issuer {:?} has level_index {}, expected < {num_levels}",
                    record.issuer_id.as_str(),
                    record.level_index
                ),
            });
        }
        folded_pairs.insert((record.issuer_id.clone(), record.level_index));
    }

    // Pre-resolve, for every issuer in the input spread map, the
    // (betas, tags) pair we need. Either the model row supplies both, or
    // runtime_tags supplies tags and we synthesize unit betas.
    //
    // We materialize the unit-beta vector once and share it via reference
    // to avoid an allocation per runtime issuer.
    let unit = unit_betas(num_levels);

    struct Resolved<'a> {
        betas: &'a IssuerBetas,
        tags: &'a IssuerTags,
    }

    let mut resolved: BTreeMap<&IssuerId, Resolved<'_>> = BTreeMap::new();
    for issuer in observed_spreads.keys() {
        if let Some(row) = beta_idx.get(issuer) {
            resolved.insert(
                issuer,
                Resolved {
                    betas: &row.betas,
                    tags: &row.tags,
                },
            );
        } else if let Some(tags) = runtime_tags.and_then(|m| m.get(issuer)) {
            resolved.insert(issuer, Resolved { betas: &unit, tags });
        } else {
            return Err(DecompositionError::UnknownIssuer {
                issuer_id: issuer.clone(),
            });
        }
    }

    // Pre-compute every issuer's bucket path at every level. A missing tag at
    // any level is a hard error here; we surface the canonical dimension key.
    //
    // bucket_paths[issuer][k] = "IG.EU.FIN" or analogous.
    let mut bucket_paths: BTreeMap<&IssuerId, Vec<String>> = BTreeMap::new();
    for (issuer, r) in &resolved {
        let mut paths = Vec::with_capacity(num_levels);
        for k in 0..num_levels {
            match model.hierarchy.bucket_path(r.tags, k) {
                Some(p) => paths.push(p),
                None => {
                    // Find the first missing dimension key for the diagnostic.
                    use finstack_core::factor_model::credit_hierarchy::dimension_key;
                    let missing_key = model.hierarchy.levels[..=k]
                        .iter()
                        .find(|dim| !r.tags.0.contains_key(&dimension_key(dim)))
                        .map(dimension_key)
                        .unwrap_or_else(|| format!("level_{k}"));
                    return Err(DecompositionError::MissingTag {
                        issuer_id: (*issuer).clone(),
                        dimension: missing_key,
                    });
                }
            }
        }
        bucket_paths.insert(*issuer, paths);
    }

    // ------------------------------------------------------------------
    // Step 1 — PC peel.
    // ------------------------------------------------------------------
    let mut residuals: BTreeMap<&IssuerId, f64> = BTreeMap::new();
    for (issuer, spread) in observed_spreads {
        let r = &resolved[issuer];
        residuals.insert(issuer, spread - r.betas.pc * observed_generic);
    }

    // ------------------------------------------------------------------
    // Step 2 — per-level peel.
    // ------------------------------------------------------------------
    let mut by_level: Vec<LevelValuesAtDate> = Vec::with_capacity(num_levels);
    // We index into `bucket_paths[issuer][k]` and `r.betas.levels[k]` inside
    // this loop, so a range-based loop is clearer than `enumerate()`-iterating
    // over one of the structures and indexing into the other.
    #[allow(clippy::needless_range_loop)]
    for k in 0..num_levels {
        // Aggregate: bucket → (sum, count) over the current residuals.
        let mut sums: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        for issuer in observed_spreads.keys() {
            if folded_pairs.contains(&(issuer.clone(), k)) {
                continue;
            }
            let path = &bucket_paths[issuer][k];
            let r_k = residuals[issuer];
            let entry = sums.entry(path.clone()).or_insert((0.0, 0));
            entry.0 += r_k;
            entry.1 += 1;
        }
        // Convert to means.
        let mut values: BTreeMap<String, f64> = BTreeMap::new();
        for (bucket, (sum, count)) in sums {
            // count is always >= 1 because every issuer contributes
            // exactly one bucket entry above.
            #[allow(clippy::cast_precision_loss)]
            let mean = sum / count as f64;
            values.insert(bucket, mean);
        }

        // Subtract β_i^level_k · L_k(g_i^k) from each issuer's residual.
        for issuer in observed_spreads.keys() {
            if folded_pairs.contains(&(issuer.clone(), k)) {
                continue;
            }
            let r = &resolved[issuer];
            let path = &bucket_paths[issuer][k];
            let level_value = values[path];
            let beta_k = r.betas.levels[k];
            let prev = residuals[issuer];
            residuals.insert(issuer, prev - beta_k * level_value);
        }

        by_level.push(LevelValuesAtDate {
            level_index: k,
            dimension: model.hierarchy.levels[k].clone(),
            values,
        });
    }

    // ------------------------------------------------------------------
    // Step 3 — final residuals are the per-issuer adders.
    // ------------------------------------------------------------------
    let adder: BTreeMap<IssuerId, f64> =
        residuals.into_iter().map(|(k, v)| (k.clone(), v)).collect();

    Ok(LevelsAtDate {
        date: as_of,
        generic: observed_generic,
        by_level,
        adder,
    })
}

// ---------------------------------------------------------------------------
// decompose_period
// ---------------------------------------------------------------------------

/// Difference two [`LevelsAtDate`] snapshots component-wise.
///
/// Output buckets and issuers are restricted to those present in **both**
/// snapshots — a one-sided entry would not satisfy the reconciliation
/// invariant on `ΔS_i` and is therefore omitted rather than emitted with an
/// implicit `0.0`.
///
/// # Errors
///
/// - [`DecompositionError::DateMismatchInPeriod`] when `from.date > to.date`.
/// - [`DecompositionError::SnapshotShapeMismatch`] when the two snapshots
///   disagree on hierarchy depth or on the dimension at any level.
pub fn decompose_period(
    from: &LevelsAtDate,
    to: &LevelsAtDate,
) -> Result<PeriodDecomposition, DecompositionError> {
    if from.date > to.date {
        return Err(DecompositionError::DateMismatchInPeriod {
            from: from.date,
            to: to.date,
        });
    }
    if from.by_level.len() != to.by_level.len() {
        return Err(DecompositionError::SnapshotShapeMismatch {
            reason: format!(
                "from.by_level.len() = {}, to.by_level.len() = {}",
                from.by_level.len(),
                to.by_level.len()
            ),
        });
    }
    for (a, b) in from.by_level.iter().zip(to.by_level.iter()) {
        if a.level_index != b.level_index || a.dimension != b.dimension {
            return Err(DecompositionError::SnapshotShapeMismatch {
                reason: format!(
                    "level {}/{}: dimensions disagree (from={:?}, to={:?})",
                    a.level_index, b.level_index, a.dimension, b.dimension
                ),
            });
        }
    }

    let d_generic = to.generic - from.generic;

    let mut by_level = Vec::with_capacity(from.by_level.len());
    for (a, b) in from.by_level.iter().zip(to.by_level.iter()) {
        let mut deltas: BTreeMap<String, f64> = BTreeMap::new();
        for (bucket, v_from) in &a.values {
            if let Some(v_to) = b.values.get(bucket) {
                deltas.insert(bucket.clone(), v_to - v_from);
            }
        }
        by_level.push(LevelValuesDelta {
            level_index: a.level_index,
            dimension: a.dimension.clone(),
            deltas,
        });
    }

    let mut d_adder: BTreeMap<IssuerId, f64> = BTreeMap::new();
    for (issuer, v_from) in &from.adder {
        if let Some(v_to) = to.adder.get(issuer) {
            d_adder.insert(issuer.clone(), v_to - v_from);
        }
    }

    Ok(PeriodDecomposition {
        from: from.date,
        to: to.date,
        d_generic,
        by_level,
        d_adder,
    })
}
