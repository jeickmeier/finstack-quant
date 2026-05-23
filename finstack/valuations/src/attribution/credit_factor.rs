//! Linear hierarchy-level decomposition of credit P&L.
//!
//! Given a calibrated [`CreditFactorModel`], a per-position list of CS01s and
//! issuer ids, and the per-period factor moves produced by
//! [`finstack_factor_model::decompose_period`], [`compute_credit_factor_attribution`]
//! returns a [`CreditFactorAttribution`] that obeys
//!
//! ```text
//! generic_pnl + Σ_levels(level.total) + adder_pnl_total ≡ Σ_i (-CS01_i × ΔS_i)
//! ```
//!
//! at absolute tolerance `1e-8`.
//!
//! # Math
//!
//! For each position `i` with issuer `g_i` and credit-curve sensitivity `CS01_i`:
//!
//! ```text
//! ΔS_i = β_i^PC · ΔF_PC + Σ_k β_i^level_k · ΔF_level_k(g_i^k) + Δadder_i
//! ```
//!
//! Multiplying through by `-CS01_i` and summing over positions:
//!
//! ```text
//! generic_pnl  = -Σ_i CS01_i · β_i^PC · ΔF_PC
//! level_k.bucket(g) = -Σ_{i in g} CS01_i · β_i^level_k · ΔF_level_k(g)
//! adder_pnl_total   = -Σ_i CS01_i · Δadder_i
//! ```
//!
//! # Determinism
//!
//! All keyed maps are [`BTreeMap`]; the function performs no I/O.

use std::collections::BTreeMap;

use finstack_core::factor_model::credit_hierarchy::{
    dimension_key, CreditFactorModel, HierarchyDimension, IssuerBetaRow,
};
use finstack_core::money::Money;
use finstack_core::types::IssuerId;
use finstack_core::Error;
use serde::{Deserialize, Serialize};

use super::types::{CreditFactorAttribution, LevelPnl};
use finstack_factor_model::PeriodDecomposition;

/// Options controlling the level of detail emitted by
/// [`compute_credit_factor_attribution`].
///
/// Defaults: per-issuer adder breakdown OFF (large-portfolio payload control);
/// per-bucket breakdown ON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct CreditFactorDetailOptions {
    /// When true, populate `CreditFactorAttribution.adder_pnl_by_issuer`.
    /// Defaults to `false` to keep payload small for big portfolios.
    pub include_per_issuer_adder: bool,
    /// When true, populate `LevelPnl.by_bucket` for every level. When false,
    /// only `LevelPnl.total` is populated. Defaults to `true`.
    pub include_per_bucket_breakdown: bool,
    /// **Deprecated and ignored.** Formerly the minimum fraction of the
    /// absolute (L1) hazard-curve move the signed move had to reach for the
    /// synthetic back-solved-CS01 decomposition to be considered
    /// well-conditioned.
    ///
    /// `compute_credit_factor_detail` no longer back-solves a synthetic CS01:
    /// it measures a real CS01 and attributes the non-parallel (twist /
    /// curve-shape) residual to `curve_shape_pnl`. There is no longer an
    /// ill-conditioned `−credit_pnl / Δs̄` divide, so no threshold to tune —
    /// a twisted curve simply lands in `curve_shape_pnl`.
    ///
    /// The field is retained (the struct is `#[serde(deny_unknown_fields)]`)
    /// only so attribution JSON written before the refactor still deserializes.
    pub parallel_fraction_floor: f64,
}

impl Default for CreditFactorDetailOptions {
    fn default() -> Self {
        Self {
            include_per_issuer_adder: false,
            include_per_bucket_breakdown: true,
            parallel_fraction_floor: 1e-3,
        }
    }
}

/// Reference to a [`CreditFactorModel`] inside an [`crate::attribution::AttributionSpec`].
///
/// The model artifact is embedded inline; boxing keeps `AttributionSpec` small
/// on the stack — `CreditFactorModel` is large.
pub type CreditFactorModelRef = Box<CreditFactorModel>;

/// Per-position input to [`compute_credit_factor_attribution`].
///
/// `cs01` is signed money (typical convention: long credit risk → negative
/// CS01, since price falls as spread widens). `delta_spread` is the observed
/// raw `ΔS_i` for the issuer's curve — kept on the struct as a sanity-check
/// hook, not used by the linear decomposition itself (the period
/// decomposition already encodes it via `β·ΔF + Δadder`).
#[derive(Debug, Clone)]
pub struct CreditAttributionInput {
    /// Caller-supplied position identifier (free-form string; not validated).
    pub position_id: String,
    /// Issuer this position is exposed to.
    pub issuer_id: IssuerId,
    /// Credit-curve sensitivity in money units.
    pub cs01: Money,
    /// Observed `ΔS_i` for this issuer (informational only).
    pub delta_spread: f64,
}

/// Stable, deterministic identifier for a [`CreditFactorModel`].
///
/// Defined as `"{as_of}/{fnv1a64(serde_json::to_string(model))}"` (16-char
/// lowercase hex). The model is serialized via `serde_json` (which uses
/// `BTreeMap`-stable order) so two byte-identical models produce the same id.
///
/// FNV-1a is used to avoid a new external crypto dependency; the id is for
/// traceability, not security.
#[allow(clippy::expect_used)] // CreditFactorModel has no non-serializable fields
pub fn credit_factor_model_id(model: &CreditFactorModel) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let json = serde_json::to_string(model).expect("CreditFactorModel is always serializable");
    let mut hash: u64 = FNV_OFFSET;
    for b in json.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{}/{:016x}", model.as_of, hash)
}

/// Compute hierarchy-level credit P&L from per-position CS01s and an already-
/// decomposed period.
///
/// # Errors
///
/// Returns [`Error::Validation`] when:
///
/// - the period decomposition shape (level count / dimension order) does not
///   match the model's hierarchy;
/// - a position references an issuer that has no [`IssuerBetaRow`] in the
///   model and whose tags cannot be derived (positions with unknown issuers
///   are silently dropped *only* if the model lists them as `BucketOnly`-eligible
///   — for unmapped issuers we still drop with a structured note from the
///   caller). For PR-7 we surface this as a hard error so misconfigurations
///   are caught.
pub fn compute_credit_factor_attribution(
    model: &CreditFactorModel,
    options: &CreditFactorDetailOptions,
    positions: &[CreditAttributionInput],
    period: &PeriodDecomposition,
) -> Result<CreditFactorAttribution, Error> {
    // ------------------------------------------------------------------
    // Shape check: period decomposition must agree with model hierarchy.
    // ------------------------------------------------------------------
    let num_levels = model.hierarchy.levels.len();
    if period.by_level.len() != num_levels {
        return Err(Error::Validation(format!(
            "credit_factor_attribution: period has {} levels but model hierarchy has {}",
            period.by_level.len(),
            num_levels
        )));
    }
    for (k, (period_lvl, model_dim)) in period
        .by_level
        .iter()
        .zip(model.hierarchy.levels.iter())
        .enumerate()
    {
        if period_lvl.level_index != k || &period_lvl.dimension != model_dim {
            return Err(Error::Validation(format!(
                "credit_factor_attribution: level {} dimension mismatch (period={:?}, model={:?})",
                k, period_lvl.dimension, model_dim
            )));
        }
    }

    // Determine output currency. If positions is empty there is nothing to do
    // — return an all-zero result in USD just to keep types consistent. (Caller
    // should not invoke us with no positions.)
    let ccy = positions
        .first()
        .map(|p| p.cs01.currency())
        .unwrap_or(finstack_core::currency::Currency::USD);

    // ------------------------------------------------------------------
    // Index issuer beta rows by id for O(log n) lookup.
    // ------------------------------------------------------------------
    let mut beta_idx: BTreeMap<&IssuerId, &IssuerBetaRow> = BTreeMap::new();
    for row in &model.issuer_betas {
        beta_idx.insert(&row.issuer_id, row);
    }

    // Pre-fill level totals and per-bucket maps (zeroed).
    let mut level_totals: Vec<f64> = vec![0.0; num_levels];
    let mut level_by_bucket: Vec<BTreeMap<String, f64>> =
        (0..num_levels).map(|_| BTreeMap::new()).collect();

    let mut generic_pnl_amt = 0.0_f64;
    let mut adder_total_amt = 0.0_f64;
    let mut adder_by_issuer_amt: BTreeMap<IssuerId, f64> = BTreeMap::new();

    let d_generic = period.d_generic;

    for input in positions {
        let cs01 = input.cs01.amount();
        if cs01 == 0.0 {
            continue;
        }
        let Some(row) = beta_idx.get(&input.issuer_id) else {
            tracing::warn!(
                issuer_id = %input.issuer_id.as_str(),
                "Credit factor attribution skipped issuer not found in CreditFactorModel.issuer_betas"
            );
            continue;
        };
        if row.betas.levels.len() != num_levels {
            return Err(Error::Validation(format!(
                "credit_factor_attribution: issuer {:?} betas.levels.len() = {}, expected {}",
                input.issuer_id.as_str(),
                row.betas.levels.len(),
                num_levels
            )));
        }

        // Generic PC contribution.
        generic_pnl_amt += -cs01 * row.betas.pc * d_generic;

        // Per-level: locate this issuer's bucket at level k; if the period
        // doesn't contain that bucket (e.g. one-sided in decompose_period) the
        // contribution falls into the adder via the period's d_adder.
        for k in 0..num_levels {
            let bucket = match model.hierarchy.bucket_path(&row.tags, k) {
                Some(p) => p,
                None => continue,
            };
            let d_level = match period.by_level[k].deltas.get(&bucket) {
                Some(v) => *v,
                None => continue,
            };
            let beta_k = row.betas.levels[k];
            let contribution = -cs01 * beta_k * d_level;
            level_totals[k] += contribution;
            if options.include_per_bucket_breakdown {
                *level_by_bucket[k].entry(bucket).or_insert(0.0) += contribution;
            }
        }

        // Adder contribution. Issuers absent from period.d_adder contribute 0.
        if let Some(d_adder) = period.d_adder.get(&input.issuer_id) {
            let contribution = -cs01 * d_adder;
            adder_total_amt += contribution;
            if options.include_per_issuer_adder {
                *adder_by_issuer_amt
                    .entry(input.issuer_id.clone())
                    .or_insert(0.0) += contribution;
            }
        }
    }

    // Materialize levels.
    let mut levels = Vec::with_capacity(num_levels);
    for (k, total) in level_totals.iter().enumerate() {
        let dim = &model.hierarchy.levels[k];
        let level_name = match dim {
            HierarchyDimension::Custom(s) => s.clone(),
            _ => dimension_key(dim),
        };
        let by_bucket = if options.include_per_bucket_breakdown {
            level_by_bucket[k]
                .iter()
                .map(|(k, v)| (k.clone(), Money::new(*v, ccy)))
                .collect()
        } else {
            BTreeMap::new()
        };
        levels.push(LevelPnl {
            level_name,
            total: Money::new(*total, ccy),
            by_bucket,
        });
    }

    let adder_pnl_by_issuer = if options.include_per_issuer_adder {
        Some(
            adder_by_issuer_amt
                .into_iter()
                .map(|(k, v)| (k, Money::new(v, ccy)))
                .collect(),
        )
    } else {
        None
    };

    Ok(CreditFactorAttribution {
        model_id: credit_factor_model_id(model),
        generic_pnl: Money::new(generic_pnl_amt, ccy),
        levels,
        adder_pnl_total: Money::new(adder_total_amt, ccy),
        // This portfolio-level decomposition works purely from a
        // `PeriodDecomposition` of scalar spread levels; it carries no
        // hazard-curve term structure, so the non-parallel curve-shape
        // component is identically zero here.
        curve_shape_pnl: Money::new(0.0, ccy),
        adder_pnl_by_issuer,
        adder_magnitude: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::dates::create_date;
    use finstack_core::factor_model::credit_hierarchy::{
        AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
        FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
        IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
    };
    use finstack_core::factor_model::{
        FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
    };
    use finstack_factor_model::{decompose_levels, decompose_period};
    use std::collections::BTreeMap;
    use time::Month;

    fn make_tags(rating: &str, region: &str) -> IssuerTags {
        let mut m = BTreeMap::new();
        m.insert("rating".into(), rating.into());
        m.insert("region".into(), region.into());
        IssuerTags(m)
    }

    fn empty_factor_model_config() -> FactorModelConfig {
        FactorModelConfig {
            factors: vec![],
            covariance: FactorCovarianceMatrix::new(vec![], vec![]).unwrap(),
            matching: MatchingConfig::MappingTable(vec![]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: Default::default(),
            bump_size: None,
            unmatched_policy: None,
        }
    }

    fn issuer_row(id: &str, rating: &str, region: &str, pc: f64, lv: Vec<f64>) -> IssuerBetaRow {
        IssuerBetaRow {
            issuer_id: IssuerId::new(id),
            tags: make_tags(rating, region),
            mode: IssuerBetaMode::IssuerBeta,
            betas: IssuerBetas { pc, levels: lv },
            adder_at_anchor: 0.0,
            adder_vol_annualized: 0.01,
            adder_vol_source: AdderVolSource::Default,
            fit_quality: None,
        }
    }

    fn model_two_levels() -> CreditFactorModel {
        CreditFactorModel {
            schema_version: CreditFactorModel::SCHEMA_VERSION.into(),
            as_of: create_date(2024, Month::March, 29).unwrap(),
            calibration_window: DateRange {
                start: create_date(2022, Month::March, 29).unwrap(),
                end: create_date(2024, Month::March, 29).unwrap(),
            },
            policy: IssuerBetaPolicy::GloballyOff,
            generic_factor: GenericFactorSpec {
                name: "CDX IG 5Y".into(),
                series_id: "cdx.ig.5y".into(),
            },
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Region],
            },
            config: empty_factor_model_config(),
            issuer_betas: vec![
                issuer_row("ISSUER-A", "IG", "EU", 1.1, vec![0.9, 1.05]),
                issuer_row("ISSUER-B", "IG", "EU", 1.2, vec![0.95, 1.0]),
                issuer_row("ISSUER-C", "HY", "NA", 0.8, vec![1.05, 0.92]),
            ],
            anchor_state: LevelsAtAnchor {
                pc: 0.0,
                by_level: vec![],
            },
            static_correlation: FactorCorrelationMatrix::identity(vec![]),
            vol_state: VolState {
                factors: BTreeMap::new(),
                idiosyncratic: BTreeMap::new(),
            },
            factor_histories: None,
            diagnostics: CalibrationDiagnostics {
                mode_counts: BTreeMap::new(),
                bucket_sizes_per_level: vec![],
                fold_ups: vec![],
                r_squared_histogram: None,
                tag_taxonomy: BTreeMap::new(),
            },
        }
    }

    fn make_period_from_spreads(
        model: &CreditFactorModel,
        s_t0: BTreeMap<IssuerId, f64>,
        g_t0: f64,
        s_t1: BTreeMap<IssuerId, f64>,
        g_t1: f64,
    ) -> PeriodDecomposition {
        let from = decompose_levels(
            model,
            &s_t0,
            g_t0,
            create_date(2025, Month::January, 1).unwrap(),
            None,
        )
        .unwrap();
        let to = decompose_levels(
            model,
            &s_t1,
            g_t1,
            create_date(2025, Month::January, 31).unwrap(),
            None,
        )
        .unwrap();
        decompose_period(&from, &to).unwrap()
    }

    #[test]
    fn reconciles_to_sum_of_minus_cs01_times_delta_spread() {
        let model = model_two_levels();

        let mut s_t0 = BTreeMap::new();
        s_t0.insert(IssuerId::new("ISSUER-A"), 100.0);
        s_t0.insert(IssuerId::new("ISSUER-B"), 110.0);
        s_t0.insert(IssuerId::new("ISSUER-C"), 350.0);
        let mut s_t1 = BTreeMap::new();
        s_t1.insert(IssuerId::new("ISSUER-A"), 105.0);
        s_t1.insert(IssuerId::new("ISSUER-B"), 118.0);
        s_t1.insert(IssuerId::new("ISSUER-C"), 360.0);
        let period = make_period_from_spreads(&model, s_t0.clone(), 80.0, s_t1.clone(), 85.0);

        let positions = vec![
            CreditAttributionInput {
                position_id: "P1".into(),
                issuer_id: IssuerId::new("ISSUER-A"),
                cs01: Money::new(-1500.0, Currency::USD),
                delta_spread: 5.0,
            },
            CreditAttributionInput {
                position_id: "P2".into(),
                issuer_id: IssuerId::new("ISSUER-B"),
                cs01: Money::new(-2000.0, Currency::USD),
                delta_spread: 8.0,
            },
            CreditAttributionInput {
                position_id: "P3".into(),
                issuer_id: IssuerId::new("ISSUER-C"),
                cs01: Money::new(-500.0, Currency::USD),
                delta_spread: 10.0,
            },
        ];

        let opts = CreditFactorDetailOptions::default();
        let detail = compute_credit_factor_attribution(&model, &opts, &positions, &period).unwrap();

        // Expected sum: -Σ CS01_i × ΔS_i. Note ΔS_i comes from the period itself
        // (s_t1 - s_t0) — we drive both sides with the same numbers.
        let expected: f64 = positions
            .iter()
            .map(|p| {
                let ds = s_t1[&p.issuer_id] - s_t0[&p.issuer_id];
                -p.cs01.amount() * ds
            })
            .sum();
        let attributed = detail.generic_pnl.amount()
            + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
            + detail.adder_pnl_total.amount();
        assert!(
            (attributed - expected).abs() < 1e-8,
            "reconciliation failed: attributed={}, expected={}",
            attributed,
            expected
        );
    }

    #[test]
    fn per_issuer_adder_is_omitted_by_default() {
        let model = model_two_levels();
        let mut s_t0 = BTreeMap::new();
        s_t0.insert(IssuerId::new("ISSUER-A"), 100.0);
        s_t0.insert(IssuerId::new("ISSUER-B"), 110.0);
        let mut s_t1 = BTreeMap::new();
        s_t1.insert(IssuerId::new("ISSUER-A"), 105.0);
        s_t1.insert(IssuerId::new("ISSUER-B"), 118.0);
        let period = make_period_from_spreads(&model, s_t0, 80.0, s_t1, 85.0);

        let positions = vec![CreditAttributionInput {
            position_id: "P1".into(),
            issuer_id: IssuerId::new("ISSUER-A"),
            cs01: Money::new(-1000.0, Currency::USD),
            delta_spread: 5.0,
        }];

        let opts = CreditFactorDetailOptions::default();
        let detail = compute_credit_factor_attribution(&model, &opts, &positions, &period).unwrap();
        assert!(detail.adder_pnl_by_issuer.is_none());
    }

    #[test]
    fn per_bucket_breakdown_can_be_disabled() {
        let model = model_two_levels();
        let mut s_t0 = BTreeMap::new();
        s_t0.insert(IssuerId::new("ISSUER-A"), 100.0);
        s_t0.insert(IssuerId::new("ISSUER-B"), 110.0);
        let mut s_t1 = BTreeMap::new();
        s_t1.insert(IssuerId::new("ISSUER-A"), 105.0);
        s_t1.insert(IssuerId::new("ISSUER-B"), 118.0);
        let period = make_period_from_spreads(&model, s_t0, 80.0, s_t1, 85.0);

        let positions = vec![CreditAttributionInput {
            position_id: "P1".into(),
            issuer_id: IssuerId::new("ISSUER-A"),
            cs01: Money::new(-1000.0, Currency::USD),
            delta_spread: 5.0,
        }];

        let opts = CreditFactorDetailOptions {
            include_per_issuer_adder: false,
            include_per_bucket_breakdown: false,
            parallel_fraction_floor: 1e-3,
        };
        let detail = compute_credit_factor_attribution(&model, &opts, &positions, &period).unwrap();
        for level in &detail.levels {
            assert!(level.by_bucket.is_empty());
        }
    }
}
