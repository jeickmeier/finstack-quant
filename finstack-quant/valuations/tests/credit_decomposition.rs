//! Integration tests for [`finstack_quant_factor_model::credit::decomposition`].
//!
//! Covers the six PR-3 acceptance tests plus structural error handling.

use std::collections::BTreeMap;

use finstack_quant_core::dates::create_date;
use finstack_quant_core::types::IssuerId;
use finstack_quant_factor_model::credit::hierarchy::{
    AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
    FactorCorrelationMatrix, FoldUpRecord, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
    IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
};
use finstack_quant_factor_model::{
    decompose_levels, decompose_period, DecompositionError, LevelsAtDate,
};
use finstack_quant_factor_model::{
    FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
};
use time::Month;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

const TOL: f64 = 1e-10;

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

fn base_model(levels: Vec<HierarchyDimension>) -> CreditFactorModel {
    CreditFactorModel {
        schema_version: CreditFactorModel::SCHEMA_VERSION.to_owned(),
        as_of: create_date(2024, Month::March, 29).unwrap(),
        calibration_window: DateRange {
            start: create_date(2022, Month::March, 29).unwrap(),
            end: create_date(2024, Month::March, 29).unwrap(),
        },
        policy: IssuerBetaPolicy::GloballyOff,
        generic_factor: GenericFactorSpec {
            name: "CDX IG 5Y".to_owned(),
            series_id: "cdx.ig.5y".to_owned(),
        },
        hierarchy: CreditHierarchySpec { levels },
        config: empty_factor_model_config(),
        issuer_betas: vec![],
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

fn tags_3d(rating: &str, region: &str, sector: &str) -> IssuerTags {
    let mut t = BTreeMap::new();
    t.insert("rating".to_owned(), rating.to_owned());
    t.insert("region".to_owned(), region.to_owned());
    t.insert("sector".to_owned(), sector.to_owned());
    IssuerTags(t)
}

fn issuer_row(id: &str, tags: IssuerTags, betas: IssuerBetas) -> IssuerBetaRow {
    IssuerBetaRow {
        issuer_id: IssuerId::new(id),
        tags,
        mode: IssuerBetaMode::IssuerBeta,
        betas,
        adder_at_anchor: 0.0,
        adder_vol_annualized: 0.01,
        adder_vol_source: AdderVolSource::Default,
        fit_quality: None,
    }
}

fn betas(pc: f64, levels: Vec<f64>) -> IssuerBetas {
    IssuerBetas { pc, levels }
}

/// 4-issuer sample model for reconciliation invariants.
///
/// Levels: rating → region → sector. Two rating buckets (IG, HY), two regions
/// (EU, NA), two sectors (FIN, TECH). Distinct beta vectors so mistakes
/// in the level-peel arithmetic show up.
fn sample_model() -> CreditFactorModel {
    let mut m = base_model(vec![
        HierarchyDimension::Rating,
        HierarchyDimension::Region,
        HierarchyDimension::Sector,
    ]);
    m.issuer_betas = vec![
        issuer_row(
            "ISSUER-A",
            tags_3d("IG", "EU", "FIN"),
            betas(1.10, vec![0.95, 1.05, 1.00]),
        ),
        issuer_row(
            "ISSUER-B",
            tags_3d("IG", "EU", "FIN"),
            betas(0.90, vec![1.05, 0.95, 1.00]),
        ),
        issuer_row(
            "ISSUER-C",
            tags_3d("IG", "NA", "TECH"),
            betas(1.00, vec![1.00, 1.10, 0.90]),
        ),
        issuer_row(
            "ISSUER-D",
            tags_3d("HY", "NA", "FIN"),
            betas(1.20, vec![1.20, 0.80, 1.10]),
        ),
    ];
    m
}

fn spread_map(entries: &[(&str, f64)]) -> BTreeMap<IssuerId, f64> {
    entries
        .iter()
        .map(|(id, s)| (IssuerId::new(*id), *s))
        .collect()
}

/// Reconstruct an issuer's spread from a `LevelsAtDate` snapshot using the
/// reconciliation identity. Returns `None` if the issuer is missing from the
/// adder map.
fn reconstruct_spread(
    levels_snapshot: &LevelsAtDate,
    issuer: &IssuerId,
    issuer_betas: &IssuerBetas,
    bucket_paths: &[String],
) -> Option<f64> {
    let adder = *levels_snapshot.adder.get(issuer)?;
    let mut s = issuer_betas.pc * levels_snapshot.generic;
    for (k, b) in issuer_betas.levels.iter().enumerate() {
        let path = &bucket_paths[k];
        let lk = *levels_snapshot.by_level[k].values.get(path)?;
        s += b * lk;
    }
    s += adder;
    Some(s)
}

fn bucket_paths_for(tags: &IssuerTags, levels: &[HierarchyDimension]) -> Vec<String> {
    let mut paths = Vec::with_capacity(levels.len());
    for k in 0..levels.len() {
        let mut parts = Vec::with_capacity(k + 1);
        for dim in levels.iter().take(k + 1) {
            let key = match dim {
                HierarchyDimension::Rating => "rating".to_owned(),
                HierarchyDimension::Region => "region".to_owned(),
                HierarchyDimension::Sector => "sector".to_owned(),
                HierarchyDimension::Custom(s) => s.clone(),
            };
            parts.push(tags.0.get(&key).cloned().unwrap());
        }
        paths.push(parts.join("."));
    }
    paths
}

// ---------------------------------------------------------------------------
// Test 1 (and zero-th sanity check): decompose_levels reconciles each issuer
// ---------------------------------------------------------------------------

#[test]
fn decompose_levels_reconciles_each_issuer() {
    let model = sample_model();
    let spreads = spread_map(&[
        ("ISSUER-A", 1.50),
        ("ISSUER-B", 1.30),
        ("ISSUER-C", 1.10),
        ("ISSUER-D", 3.40),
    ]);
    let generic = 0.80_f64;
    let as_of = create_date(2024, Month::April, 30).unwrap();

    let snap = decompose_levels(&model, &spreads, generic, as_of, None).unwrap();
    assert_eq!(snap.date, as_of);
    assert_eq!(snap.generic, generic);
    assert_eq!(snap.by_level.len(), 3);

    let mut max_err = 0.0_f64;
    for row in &model.issuer_betas {
        if !spreads.contains_key(&row.issuer_id) {
            continue;
        }
        let paths = bucket_paths_for(&row.tags, &model.hierarchy.levels);
        let recon = reconstruct_spread(&snap, &row.issuer_id, &row.betas, &paths).unwrap();
        let observed = spreads[&row.issuer_id];
        let err = (recon - observed).abs();
        assert!(
            err < TOL,
            "issuer {:?}: recon = {recon}, observed = {observed}, err = {err}",
            row.issuer_id.as_str()
        );
        if err > max_err {
            max_err = err;
        }
    }
    // Sanity: max error is well under tolerance.
    assert!(max_err < TOL);
}

// ---------------------------------------------------------------------------
// Test 2: PR-3 named test — period reconciliation
// ---------------------------------------------------------------------------

#[test]
fn decompose_period_reconciles_each_issuer() {
    let model = sample_model();

    let s1 = spread_map(&[
        ("ISSUER-A", 1.50),
        ("ISSUER-B", 1.30),
        ("ISSUER-C", 1.10),
        ("ISSUER-D", 3.40),
    ]);
    let s2 = spread_map(&[
        ("ISSUER-A", 1.65),
        ("ISSUER-B", 1.42),
        ("ISSUER-C", 1.22),
        ("ISSUER-D", 3.80),
    ]);
    let g1 = 0.80_f64;
    let g2 = 0.90_f64;
    let d1 = create_date(2024, Month::April, 30).unwrap();
    let d2 = create_date(2024, Month::May, 31).unwrap();

    let snap1 = decompose_levels(&model, &s1, g1, d1, None).unwrap();
    let snap2 = decompose_levels(&model, &s2, g2, d2, None).unwrap();
    let period = decompose_period(&snap1, &snap2).unwrap();

    let mut max_err = 0.0_f64;
    for row in &model.issuer_betas {
        if !s1.contains_key(&row.issuer_id) || !s2.contains_key(&row.issuer_id) {
            continue;
        }
        let paths = bucket_paths_for(&row.tags, &model.hierarchy.levels);
        let mut delta_s = row.betas.pc * period.d_generic;
        for (k, b) in row.betas.levels.iter().enumerate() {
            let dk = *period.by_level[k].deltas.get(&paths[k]).unwrap();
            delta_s += b * dk;
        }
        delta_s += *period.d_adder.get(&row.issuer_id).unwrap();

        let observed = s2[&row.issuer_id] - s1[&row.issuer_id];
        let err = (delta_s - observed).abs();
        assert!(
            err < TOL,
            "issuer {:?}: ΔS recon = {delta_s}, observed = {observed}, err = {err}",
            row.issuer_id.as_str()
        );
        if err > max_err {
            max_err = err;
        }
    }
    assert!(max_err < TOL);
}

// ---------------------------------------------------------------------------
// Test 3: PR-3 named test — runtime issuer with tags is BucketOnly
// ---------------------------------------------------------------------------

#[test]
fn new_issuer_with_tags_is_bucket_only() {
    // Model has only A, B, C; we observe a brand-new D-prime that's not in
    // the calibrated artifact but is supplied via runtime_tags.
    let mut model = base_model(vec![HierarchyDimension::Rating, HierarchyDimension::Region]);
    model.issuer_betas = vec![
        issuer_row(
            "ISSUER-A",
            tags_3d("IG", "EU", "FIN"),
            betas(1.0, vec![1.0, 1.0]),
        ),
        issuer_row(
            "ISSUER-B",
            tags_3d("IG", "EU", "FIN"),
            betas(1.0, vec![1.0, 1.0]),
        ),
    ];

    let mut runtime_tags = BTreeMap::new();
    runtime_tags.insert(
        IssuerId::new("ISSUER-NEW"),
        tags_3d("IG", "EU", "FIN"), // sector is irrelevant — only first two levels.
    );

    let spreads = spread_map(&[("ISSUER-A", 1.10), ("ISSUER-B", 1.20), ("ISSUER-NEW", 1.30)]);
    let snap = decompose_levels(
        &model,
        &spreads,
        0.50,
        create_date(2024, Month::April, 30).unwrap(),
        Some(&runtime_tags),
    )
    .unwrap();

    // Reconstruct with β=1 for the new issuer.
    let unit = betas(1.0, vec![1.0, 1.0]);
    let new_id = IssuerId::new("ISSUER-NEW");
    let new_tags = tags_3d("IG", "EU", "FIN");
    let paths = bucket_paths_for(&new_tags, &model.hierarchy.levels);
    let recon = reconstruct_spread(&snap, &new_id, &unit, &paths).unwrap();
    assert!(
        (recon - 1.30).abs() < TOL,
        "new issuer recon {recon} != observed 1.30"
    );
}

// ---------------------------------------------------------------------------
// Test 4: PR-3 named test — missing tag returns error
// ---------------------------------------------------------------------------

#[test]
fn missing_required_tag_returns_error() {
    let mut model = base_model(vec![
        HierarchyDimension::Rating,
        HierarchyDimension::Region,
        HierarchyDimension::Sector,
    ]);
    // ISSUER-A in the model is missing 'sector'.
    let mut bad_tags = BTreeMap::new();
    bad_tags.insert("rating".to_owned(), "IG".to_owned());
    bad_tags.insert("region".to_owned(), "EU".to_owned());
    model.issuer_betas = vec![issuer_row(
        "ISSUER-A",
        IssuerTags(bad_tags),
        betas(1.0, vec![1.0, 1.0, 1.0]),
    )];

    let spreads = spread_map(&[("ISSUER-A", 1.0)]);
    let err = decompose_levels(
        &model,
        &spreads,
        0.5,
        create_date(2024, Month::April, 30).unwrap(),
        None,
    )
    .unwrap_err();

    match err {
        DecompositionError::MissingTag {
            issuer_id,
            dimension,
        } => {
            assert_eq!(issuer_id.as_str(), "ISSUER-A");
            assert_eq!(dimension, "sector");
        }
        other => panic!("expected MissingTag, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 5: PR-3 named test — empty bucket at as_of degrades gracefully
// ---------------------------------------------------------------------------

#[test]
fn empty_bucket_at_as_of_degrades_to_zero_level() {
    // Model has issuers spanning 'IG' and 'HY' rating buckets, but observed
    // spreads at as_of include only IG issuers. The HY bucket should simply
    // be absent from the resulting LevelsAtDate (as documented).
    let mut model = base_model(vec![HierarchyDimension::Rating]);
    model.issuer_betas = vec![
        issuer_row(
            "ISSUER-A",
            tags_3d("IG", "EU", "FIN"),
            betas(1.0, vec![1.0]),
        ),
        issuer_row(
            "ISSUER-B",
            tags_3d("HY", "EU", "FIN"),
            betas(1.0, vec![1.0]),
        ),
    ];

    let spreads = spread_map(&[("ISSUER-A", 1.0)]);
    let snap = decompose_levels(
        &model,
        &spreads,
        0.5,
        create_date(2024, Month::April, 30).unwrap(),
        None,
    )
    .unwrap();

    assert_eq!(snap.by_level.len(), 1);
    assert!(snap.by_level[0].values.contains_key("IG"));
    assert!(
        !snap.by_level[0].values.contains_key("HY"),
        "expected HY bucket omitted (no current issuers); got {:?}",
        snap.by_level[0].values
    );
}

// ---------------------------------------------------------------------------
// Test 6: PR-3 named test — empty hierarchy
// ---------------------------------------------------------------------------

#[test]
fn empty_hierarchy_decomposes_to_generic_and_adder() {
    let mut model = base_model(vec![]);
    model.issuer_betas = vec![
        issuer_row("ISSUER-A", IssuerTags(BTreeMap::new()), betas(1.10, vec![])),
        issuer_row("ISSUER-B", IssuerTags(BTreeMap::new()), betas(0.90, vec![])),
    ];

    let spreads = spread_map(&[("ISSUER-A", 1.50), ("ISSUER-B", 1.10)]);
    let generic = 0.80_f64;
    let snap = decompose_levels(
        &model,
        &spreads,
        generic,
        create_date(2024, Month::April, 30).unwrap(),
        None,
    )
    .unwrap();

    assert!(snap.by_level.is_empty());
    let a = snap.adder[&IssuerId::new("ISSUER-A")];
    let b = snap.adder[&IssuerId::new("ISSUER-B")];
    // adder = S - β_PC · generic
    assert!((a - (1.50 - 1.10 * 0.80)).abs() < TOL);
    assert!((b - (1.10 - 0.90 * 0.80)).abs() < TOL);
}

// ---------------------------------------------------------------------------
// Test 7: round-trip — decompose_levels twice, then decompose_period
// ---------------------------------------------------------------------------

#[test]
fn decompose_levels_then_decompose_period_round_trip() {
    let model = sample_model();
    let s1 = spread_map(&[
        ("ISSUER-A", 1.30),
        ("ISSUER-B", 1.50),
        ("ISSUER-C", 1.05),
        ("ISSUER-D", 3.10),
    ]);
    let s2 = spread_map(&[
        ("ISSUER-A", 1.42),
        ("ISSUER-B", 1.66),
        ("ISSUER-C", 1.20),
        ("ISSUER-D", 3.45),
    ]);
    let g1 = 0.75_f64;
    let g2 = 0.92_f64;
    let d1 = create_date(2024, Month::April, 30).unwrap();
    let d2 = create_date(2024, Month::May, 31).unwrap();

    let snap1 = decompose_levels(&model, &s1, g1, d1, None).unwrap();
    let snap2 = decompose_levels(&model, &s2, g2, d2, None).unwrap();
    let period = decompose_period(&snap1, &snap2).unwrap();

    // Each issuer's ΔS reconstructs from the period decomposition.
    for row in &model.issuer_betas {
        let paths = bucket_paths_for(&row.tags, &model.hierarchy.levels);
        let mut delta_s = row.betas.pc * period.d_generic;
        for (k, b) in row.betas.levels.iter().enumerate() {
            delta_s += b * period.by_level[k].deltas[&paths[k]];
        }
        delta_s += period.d_adder[&row.issuer_id];

        let observed = s2[&row.issuer_id] - s1[&row.issuer_id];
        assert!(
            (delta_s - observed).abs() < TOL,
            "round-trip ΔS mismatch for {:?}: recon = {delta_s}, observed = {observed}",
            row.issuer_id.as_str()
        );
    }
}

// ---------------------------------------------------------------------------
// Defensive structural tests — model inconsistency, snapshot shape mismatch
// ---------------------------------------------------------------------------

#[test]
fn decompose_levels_rejects_inconsistent_betas_length() {
    let mut model = base_model(vec![HierarchyDimension::Rating, HierarchyDimension::Region]);
    // Wrong length: only one level beta when hierarchy has two.
    model.issuer_betas = vec![issuer_row(
        "ISSUER-A",
        tags_3d("IG", "EU", "FIN"),
        betas(1.0, vec![1.0]),
    )];
    let spreads = spread_map(&[("ISSUER-A", 1.0)]);
    let err = decompose_levels(
        &model,
        &spreads,
        0.5,
        create_date(2024, Month::April, 30).unwrap(),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, DecompositionError::ModelInconsistent { .. }));
}

#[test]
fn decompose_levels_rejects_unknown_issuer_without_runtime_tags() {
    let model = base_model(vec![HierarchyDimension::Rating]);
    // ISSUER-X is in spreads but not in the model.
    let spreads = spread_map(&[("ISSUER-X", 1.0)]);
    let err = decompose_levels(
        &model,
        &spreads,
        0.5,
        create_date(2024, Month::April, 30).unwrap(),
        None,
    )
    .unwrap_err();
    match err {
        DecompositionError::UnknownIssuer { issuer_id } => {
            assert_eq!(issuer_id.as_str(), "ISSUER-X");
        }
        other => panic!("expected UnknownIssuer, got {other:?}"),
    }
}

#[test]
fn decompose_period_rejects_swapped_dates() {
    let model = sample_model();
    let s = spread_map(&[
        ("ISSUER-A", 1.0),
        ("ISSUER-B", 1.0),
        ("ISSUER-C", 1.0),
        ("ISSUER-D", 1.0),
    ]);
    let early = decompose_levels(
        &model,
        &s,
        0.5,
        create_date(2024, Month::April, 30).unwrap(),
        None,
    )
    .unwrap();
    let late = decompose_levels(
        &model,
        &s,
        0.5,
        create_date(2024, Month::May, 31).unwrap(),
        None,
    )
    .unwrap();
    let err = decompose_period(&late, &early).unwrap_err();
    assert!(matches!(
        err,
        DecompositionError::DateMismatchInPeriod { .. }
    ));
}

#[test]
fn decompose_period_rejects_shape_mismatch() {
    let model_a = base_model(vec![HierarchyDimension::Rating]);
    let mut model_a = model_a;
    model_a.issuer_betas = vec![issuer_row(
        "ISSUER-A",
        tags_3d("IG", "EU", "FIN"),
        betas(1.0, vec![1.0]),
    )];

    let mut model_b = base_model(vec![HierarchyDimension::Rating, HierarchyDimension::Region]);
    model_b.issuer_betas = vec![issuer_row(
        "ISSUER-A",
        tags_3d("IG", "EU", "FIN"),
        betas(1.0, vec![1.0, 1.0]),
    )];

    let s = spread_map(&[("ISSUER-A", 1.0)]);
    let snap_a = decompose_levels(
        &model_a,
        &s,
        0.5,
        create_date(2024, Month::April, 30).unwrap(),
        None,
    )
    .unwrap();
    let snap_b = decompose_levels(
        &model_b,
        &s,
        0.5,
        create_date(2024, Month::May, 31).unwrap(),
        None,
    )
    .unwrap();
    let err = decompose_period(&snap_a, &snap_b).unwrap_err();
    assert!(matches!(
        err,
        DecompositionError::SnapshotShapeMismatch { .. }
    ));
}

#[test]
fn decompose_levels_excludes_folded_issuers_from_bucket_means() {
    let mut model = base_model(vec![HierarchyDimension::Rating]);
    model.issuer_betas = vec![
        issuer_row(
            "ISSUER-FOLDED",
            tags_3d("IG", "EU", "FIN"),
            betas(1.0, vec![0.0]),
        ),
        issuer_row(
            "ISSUER-ACTIVE",
            tags_3d("IG", "EU", "FIN"),
            betas(1.0, vec![1.0]),
        ),
    ];
    model.diagnostics.fold_ups = vec![FoldUpRecord {
        issuer_id: IssuerId::new("ISSUER-FOLDED"),
        level_index: 0,
        original_bucket: "IG".to_owned(),
        folded_to: "<root>".to_owned(),
        reason: "test fold-up".to_owned(),
    }];

    let spreads = spread_map(&[("ISSUER-FOLDED", 100.0), ("ISSUER-ACTIVE", 10.0)]);
    let snap = decompose_levels(
        &model,
        &spreads,
        0.0,
        create_date(2024, Month::April, 30).unwrap(),
        None,
    )
    .unwrap();

    assert_eq!(snap.by_level[0].values.get("IG").copied(), Some(10.0));
    assert_eq!(
        snap.adder.get(&IssuerId::new("ISSUER-FOLDED")).copied(),
        Some(100.0)
    );
    assert_eq!(
        snap.adder.get(&IssuerId::new("ISSUER-ACTIVE")).copied(),
        Some(0.0)
    );
}

// ---------------------------------------------------------------------------
// non-finite inputs are rejected before they poison bucket means
// ---------------------------------------------------------------------------

#[test]
fn decompose_levels_rejects_non_finite_inputs() {
    let model = sample_model();
    let as_of = model.as_of;

    // NaN spread for one issuer must be a typed error, not a poisoned cascade.
    let mut spreads = spread_map(&[("ISSUER-A", 120.0), ("ISSUER-B", 95.0)]);
    spreads.insert(IssuerId::new("ISSUER-A"), f64::NAN);
    let err = decompose_levels(&model, &spreads, 100.0, as_of, None)
        .expect_err("NaN spread must be rejected");
    assert!(
        err.to_string().contains("ISSUER-A"),
        "error must name the offending issuer: {err}"
    );

    // Non-finite generic is rejected too.
    let spreads = spread_map(&[("ISSUER-A", 120.0)]);
    let err = decompose_levels(&model, &spreads, f64::INFINITY, as_of, None)
        .expect_err("non-finite generic must be rejected");
    assert!(
        err.to_string().contains("observed_generic"),
        "error must name the generic input: {err}"
    );
}
