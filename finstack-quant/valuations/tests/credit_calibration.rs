//! Integration tests for [`finstack_quant_models::factor::credit::calibration`].
//!
//! Implements the seven required PR-4 tests from the design.

use std::collections::{BTreeMap, BTreeSet};

use finstack_quant_core::dates::{create_date, Date, DateExt};
use finstack_quant_core::types::IssuerId;
use finstack_quant_models::factor::credit::calibration::{
    BetaShrinkage, BucketSizeThresholds, BucketWeighting, CovarianceStrategy,
    CreditCalibrationConfig, CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries,
    HistoryPanel, IssuerTagPanel, PanelFrequency, PanelSpace, VolModelChoice,
};
use finstack_quant_models::factor::credit::hierarchy::{
    AdderVolSource, CreditFactorModel, CreditHierarchySpec, FactorVolModel, GenericFactorSpec,
    HierarchyDimension, IdiosyncraticVolModel, IssuerBetaMode, IssuerBetaOverride,
    IssuerBetaPolicy, IssuerTags,
};
use time::Month;

fn d(year: i32, month: Month, day: u8) -> Date {
    create_date(year, month, day).expect("valid date")
}

/// 24-month grid of monthly dates ending at as_of.
fn monthly_dates(n: usize, end: Date) -> Vec<Date> {
    let mut out = Vec::with_capacity(n);
    let mut current = end;
    for _ in 0..n {
        out.push(current);
        current = if current == current.end_of_month() {
            current.add_months(-1).end_of_month()
        } else {
            current.add_months(-1)
        };
    }
    out.reverse();
    out
}

fn tags_for(rating: &str, region: &str) -> IssuerTags {
    let mut t = BTreeMap::new();
    t.insert("rating".to_owned(), rating.to_owned());
    t.insert("region".to_owned(), region.to_owned());
    IssuerTags(t)
}

fn tags_for_sector(rating: &str, region: &str, sector: &str) -> IssuerTags {
    let mut t = tags_for(rating, region).0;
    t.insert("sector".to_owned(), sector.to_owned());
    IssuerTags(t)
}

/// Convert a basis-point fixture number to the decimal spread the calibrator
/// requires (`100.0` → `0.01`).
fn bp(value: f64) -> f64 {
    value / 10_000.0
}

/// Synthesize a deterministic 24-month panel with 6 issuers in 2 ratings × 3 regions.
fn fixture_panel() -> CalibrationFixture {
    let n = 24;
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);

    // Generic factor: simple deterministic increments.
    let generic_values: Vec<f64> = (0..n)
        .map(|i| 0.0100 + 0.00005 * (i as f64).sin())
        .collect();

    // 6 issuers — 3 IG (across regions) + 3 HY (across regions).
    let issuer_specs = [
        ("ISSUER-A", "IG", "EU"),
        ("ISSUER-B", "IG", "NA"),
        ("ISSUER-C", "IG", "APAC"),
        ("ISSUER-D", "HY", "EU"),
        ("ISSUER-E", "HY", "NA"),
        ("ISSUER-F", "HY", "APAC"),
    ];

    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut tags: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut as_of_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    for (idx, (id, rating, region)) in issuer_specs.iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        let base = 0.0100 + (idx as f64) * 0.0025;
        let beta_pc = 0.7 + 0.05 * (idx as f64);
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                let val = base
                    + beta_pc * (generic_values[i] - 0.0100)
                    + 0.00001 * ((idx as f64) + (i as f64) * 0.5).cos();
                Some(val)
            })
            .collect();
        as_of_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
        spreads.insert(issuer_id.clone(), series);
        tags.insert(issuer_id, tags_for(rating, region));
    }

    CalibrationFixture {
        history: HistoryPanel { dates, spreads },
        tags: IssuerTagPanel { tags },
        generic: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: "CDX IG 5Y".to_owned(),
                series_id: "cdx.ig.5y".to_owned(),
            },
            values: generic_values,
        },
        as_of,
        as_of_spreads,
    }
}

struct CalibrationFixture {
    history: HistoryPanel,
    tags: IssuerTagPanel,
    generic: GenericFactorSeries,
    as_of: Date,
    as_of_spreads: BTreeMap<IssuerId, f64>,
}

impl CalibrationFixture {
    fn into_inputs(self) -> CreditCalibrationInputs {
        CreditCalibrationInputs {
            history_panel: self.history,
            issuer_tags: self.tags,
            generic_factor: self.generic,
            as_of: self.as_of,
            as_of_spreads: self.as_of_spreads,
            idiosyncratic_overrides: BTreeMap::new(),
            spread_durations: BTreeMap::new(),
        }
    }
}

fn config_with(
    policy: IssuerBetaPolicy,
    levels: Vec<HierarchyDimension>,
) -> CreditCalibrationConfig {
    CreditCalibrationConfig {
        policy,
        hierarchy: CreditHierarchySpec {
            levels: levels.clone(),
        },
        min_bucket_size_per_level: BucketSizeThresholds::default_for_levels(levels.len()),
        vol_model: VolModelChoice::Sample,
        covariance_strategy: CovarianceStrategy::Diagonal,
        beta_shrinkage: BetaShrinkage::None,
        use_returns_or_levels: PanelSpace::Returns,
        panel_frequency: PanelFrequency::Monthly,
        bucket_weighting: BucketWeighting::Equal,
    }
}

// PR-4 Test 1: bit-identical determinism

#[test]
fn calibration_is_bit_identical_for_same_inputs() {
    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 12,
        overrides: BTreeMap::new(),
    };
    let cfg = config_with(
        policy,
        vec![HierarchyDimension::Rating, HierarchyDimension::Region],
    );
    // Lower bucket-size thresholds so the test fixture (1 issuer per leaf
    // bucket) doesn't hit fold-up by accident.
    let cfg_a = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..cfg
    };
    let cfg_b = cfg_a.clone();

    let inputs_a = fixture_panel().into_inputs();
    let inputs_b = fixture_panel().into_inputs();

    let model_a = CreditCalibrator::new(cfg_a)
        .calibrate(inputs_a)
        .expect("calibration A succeeds");
    let model_b = CreditCalibrator::new(cfg_b)
        .calibrate(inputs_b)
        .expect("calibration B succeeds");

    let json_a = serde_json::to_string(&model_a).expect("serialize A");
    let json_b = serde_json::to_string(&model_b).expect("serialize B");
    assert_eq!(json_a, json_b, "calibration must be bit-identical");

    // Validation must still pass.
    model_a.validate().expect("validate model A");
}

// PR-4 Test 2: GloballyOff sets all betas to 1.0

#[test]
fn globally_off_sets_all_betas_to_one() {
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };
    let inputs = fixture_panel().into_inputs();
    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    assert!(!model.issuer_betas.is_empty());
    for row in &model.issuer_betas {
        assert!(
            matches!(row.mode, IssuerBetaMode::BucketOnly),
            "mode must be BucketOnly under GloballyOff"
        );
        assert!(
            (row.betas.pc - 1.0).abs() < 1e-12,
            "pc beta must be 1.0; got {}",
            row.betas.pc
        );
        for (k, b) in row.betas.levels.iter().enumerate() {
            assert!(
                (b - 1.0).abs() < 1e-12,
                "level {k} beta must be 1.0; got {b}"
            );
        }
        assert!(row.fit_quality.is_none());
    }
}

// PR-4 Test 3: Dynamic policy classifies short history as BucketOnly

#[test]
fn dynamic_policy_classifies_short_history_as_bucket_only() {
    // Set min_history above the fixture's 24 months so every issuer fails the
    // gate.
    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 36,
        overrides: BTreeMap::new(),
    };
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            policy,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };
    let inputs = fixture_panel().into_inputs();
    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    for row in &model.issuer_betas {
        assert!(
            matches!(row.mode, IssuerBetaMode::BucketOnly),
            "issuer {:?} should be BucketOnly with insufficient history",
            row.issuer_id.as_str()
        );
        assert!((row.betas.pc - 1.0).abs() < 1e-12);
        for b in &row.betas.levels {
            assert!((b - 1.0).abs() < 1e-12);
        }
    }
}

// PR-4 Test 4: ForceIssuerBeta override wins over short-history rule

#[test]
fn override_force_issuer_beta_wins() {
    let mut overrides = BTreeMap::new();
    overrides.insert(
        IssuerId::new("ISSUER-A"),
        IssuerBetaOverride::ForceIssuerBeta,
    );
    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 100, // way above the fixture's 24
        overrides,
    };
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            policy,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };
    let inputs = fixture_panel().into_inputs();
    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    let row_a = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "ISSUER-A")
        .expect("ISSUER-A row present");
    assert!(
        matches!(row_a.mode, IssuerBetaMode::IssuerBeta),
        "ForceIssuerBeta override must produce IssuerBeta mode despite short history"
    );

    // All others must remain BucketOnly because they hit the min_history gate.
    for row in &model.issuer_betas {
        if row.issuer_id.as_str() == "ISSUER-A" {
            continue;
        }
        assert!(matches!(row.mode, IssuerBetaMode::BucketOnly));
    }
}

// PR-4 Test 5: Sparse bucket folds to parent

#[test]
fn sparse_bucket_folds_to_parent() {
    // Threshold = 5 at level 0 means each rating bucket needs ≥ 5 IssuerBeta
    // issuers. The fixture has 3 IG + 3 HY → both buckets fold up.
    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 12,
        overrides: BTreeMap::new(),
    };
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![5, 5],
        },
        ..config_with(
            policy,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };
    let inputs = fixture_panel().into_inputs();
    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    // Folded issuers must have β = 0 at level 0.
    for row in &model.issuer_betas {
        if matches!(row.mode, IssuerBetaMode::IssuerBeta) {
            assert!(
                (row.betas.levels[0]).abs() < 1e-12,
                "issuer {:?} level0 beta should be 0 after fold-up; got {}",
                row.issuer_id.as_str(),
                row.betas.levels[0]
            );
        }
    }

    // FoldUpRecord must be populated.
    assert!(
        !model.diagnostics.fold_ups.is_empty(),
        "diagnostics.fold_ups must record the fold-ups"
    );
    let any_level0 = model
        .diagnostics
        .fold_ups
        .iter()
        .any(|f| f.level_index == 0);
    assert!(any_level0, "fold-up at level 0 must be recorded");
}

// PR-4 Test 6: Single-level hierarchy → expected factor IDs

#[test]
fn single_level_hierarchy_builds_expected_factor_ids() {
    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 12,
        overrides: BTreeMap::new(),
    };
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(policy, vec![HierarchyDimension::Rating])
    };
    let inputs = fixture_panel().into_inputs();
    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    let factor_ids: Vec<String> = model
        .config
        .factors
        .iter()
        .map(|f| f.id.as_str().to_owned())
        .collect();

    let expected = vec![
        "credit::generic".to_owned(),
        "credit::level0::Rating::HY".to_owned(),
        "credit::level0::Rating::IG".to_owned(),
    ];
    assert_eq!(factor_ids, expected);

    // M-4: verify tag_taxonomy contains the expected dimension and observed values.
    let taxonomy = &model.diagnostics.tag_taxonomy;
    assert!(
        taxonomy.contains_key("rating"),
        "tag_taxonomy must contain dimension key 'rating'"
    );
    let rating_values = &taxonomy["rating"];
    assert_eq!(
        *rating_values,
        BTreeSet::from(["IG".to_owned(), "HY".to_owned()]),
        "rating dimension must observe exactly IG and HY"
    );
    // Single-level hierarchy: only 'rating' should appear as a key.
    assert_eq!(
        taxonomy.len(),
        1,
        "single-level Rating hierarchy must produce exactly one taxonomy key"
    );
}

// PR-4 Test 7: All-BucketOnly calibration succeeds

#[test]
fn all_bucket_only_calibration_succeeds() {
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };
    let inputs = fixture_panel().into_inputs();
    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    // Every issuer is BucketOnly.
    for row in &model.issuer_betas {
        assert!(matches!(row.mode, IssuerBetaMode::BucketOnly));
    }

    // The bucket factor at level 0 (Rating) must equal the cross-sectional
    // mean of issuer residuals after PC peel — since all PC betas = 1.0, the
    // residual is `r_i = ΔS_i - Δgeneric` per period, and the bucket factor
    // series equals the simple average.
    // We don't recompute it numerically here, but ensure validate() holds and
    // each surviving bucket factor has a sample-variance entry.
    model.validate().expect("validate succeeds");
    assert!(model.factor_histories.is_some());
    let fh = model.factor_histories.as_ref().unwrap();
    assert!(fh
        .values
        .contains_key(&finstack_quant_models::factor::FactorId::new(
            "credit::generic"
        )));
    assert!(!model.vol_state.factors.is_empty());

    // M-4: verify tag_taxonomy for a two-level Rating × Region hierarchy.
    let taxonomy = &model.diagnostics.tag_taxonomy;
    assert!(
        taxonomy.contains_key("rating"),
        "tag_taxonomy must contain dimension key 'rating'"
    );
    assert!(
        taxonomy.contains_key("region"),
        "tag_taxonomy must contain dimension key 'region'"
    );
    assert_eq!(
        taxonomy["rating"],
        BTreeSet::from(["IG".to_owned(), "HY".to_owned()]),
        "rating dimension must observe exactly IG and HY"
    );
    assert_eq!(
        taxonomy["region"],
        BTreeSet::from(["EU".to_owned(), "NA".to_owned(), "APAC".to_owned()]),
        "region dimension must observe exactly EU, NA, and APAC"
    );
}

// I-2: sparse bucket emits None for empty dates (factor variance excludes gap)

/// A panel with a `None` hole is rejected: calibration requires a fully
/// aligned grid (no missing issuer observations).
#[test]
fn sparse_bucket_emits_none_for_empty_dates() {
    let n = 12usize; // 12-month panel
    let as_of = d(2024, Month::December, 31);
    let dates = monthly_dates(n, as_of);

    let generic_values: Vec<f64> = (0..n).map(|i| 0.01 * (i as f64).sin()).collect();

    // Two issuers: IG (sole member of its bucket) and HY (sole member of its bucket).
    // IG is missing on date index 5.
    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut issuer_tags_map: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut as_of_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    let ig_id = IssuerId::new("ISSUER-IG");
    let hy_id = IssuerId::new("ISSUER-HY");

    // IG series: present on all dates except index 5.
    let ig_series: Vec<Option<f64>> = (0..n)
        .map(|i| {
            if i == 5 {
                None
            } else {
                Some(bp(100.0) + 0.8 * generic_values[i] + 0.05 * (i as f64).cos())
            }
        })
        .collect();
    as_of_spreads.insert(ig_id.clone(), ig_series[n - 1].unwrap());
    spreads.insert(ig_id.clone(), ig_series);
    let mut ig_tags_map = BTreeMap::new();
    ig_tags_map.insert("rating".to_owned(), "IG".to_owned());
    issuer_tags_map.insert(ig_id, IssuerTags(ig_tags_map));

    // HY series: fully observed.
    let hy_series: Vec<Option<f64>> = (0..n)
        .map(|i| Some(bp(200.0) + 1.2 * generic_values[i] + 0.03 * (i as f64).sin()))
        .collect();
    as_of_spreads.insert(hy_id.clone(), hy_series[n - 1].unwrap());
    spreads.insert(hy_id.clone(), hy_series);
    let mut hy_tags_map = BTreeMap::new();
    hy_tags_map.insert("rating".to_owned(), "HY".to_owned());
    issuer_tags_map.insert(hy_id, IssuerTags(hy_tags_map));

    let inputs = CreditCalibrationInputs {
        history_panel: HistoryPanel { dates, spreads },
        issuer_tags: IssuerTagPanel {
            tags: issuer_tags_map,
        },
        generic_factor: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: "CDX".to_owned(),
                series_id: "cdx".to_owned(),
            },
            values: generic_values,
        },
        as_of,
        as_of_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
        spread_durations: BTreeMap::new(),
    };

    // Use GloballyOff so betas=1 and the IG factor series = issuer's residual mean.
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };

    let err = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect_err("a None hole must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("missing an observation") && msg.contains("ISSUER-IG"),
        "rejection must name the hole, got {msg}"
    );
}

// Additional: unsupported PR-5a/b features error cleanly

/// PR-5b: Ridge covariance is now supported (replaces the old rejection test).
#[test]
fn ridge_covariance_accepted() {
    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::Ridge { alpha: 0.1 },
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };
    let inputs = fixture_panel().into_inputs();
    assert!(
        CreditCalibrator::new(cfg).calibrate(inputs).is_ok(),
        "Ridge covariance strategy must succeed in PR-5b"
    );
}

/// EWMA vol model calibrates and persists the estimator in vol_state.
#[test]
fn ewma_vol_model_calibrates_and_persists_estimator() {
    let cfg = CreditCalibrationConfig {
        vol_model: VolModelChoice::Ewma { lambda: 0.94 },
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };
    let model = CreditCalibrator::new(cfg)
        .calibrate(fixture_panel().into_inputs())
        .expect("EWMA calibration succeeds");

    assert!(!model.vol_state.factors.is_empty());
    for (fid, vol_model) in &model.vol_state.factors {
        let FactorVolModel::Ewma { lambda, variance } = vol_model else {
            panic!("factor {fid} must persist the EWMA estimator, got {vol_model:?}");
        };
        assert!((lambda - 0.94).abs() < 1e-15);
        assert!(variance.is_finite() && *variance >= 0.0);
    }

    // JSON round-trip preserves the EWMA vol state. Compared field-wise
    // (rather than via the derived `PartialEq` on the whole `vol_state`)
    // because serde_json's default f64 parser does not guarantee a
    // bit-exact round trip; that guarantee is only available behind the
    // opt-in `float_roundtrip` feature, which this crate deliberately does
    // not enable (see .claude/rules/rust/testing-standards.md on documented
    // float tolerances). `lambda` is an input value echoed straight through,
    // so it must match exactly; `variance` is a computed quantity and is
    // compared with a tight relative tolerance instead.
    let json = serde_json::to_string(&model).expect("serialize");
    let back: CreditFactorModel = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        model.vol_state.factors.len(),
        back.vol_state.factors.len(),
        "round-trip must preserve the same number of factor vol entries"
    );
    for (fid, orig_vol_model) in &model.vol_state.factors {
        let back_vol_model = back
            .vol_state
            .factors
            .get(fid)
            .unwrap_or_else(|| panic!("round-tripped vol_state missing factor {fid}"));
        let FactorVolModel::Ewma {
            lambda: orig_lambda,
            variance: orig_variance,
        } = orig_vol_model
        else {
            panic!("factor {fid} must persist the EWMA estimator, got {orig_vol_model:?}");
        };
        let FactorVolModel::Ewma {
            lambda: back_lambda,
            variance: back_variance,
        } = back_vol_model
        else {
            panic!(
                "round-tripped factor {fid} must persist the EWMA estimator, got {back_vol_model:?}"
            );
        };
        assert_eq!(
            orig_lambda, back_lambda,
            "lambda is an echoed input and must round-trip exactly"
        );
        let rel_diff = (orig_variance - back_variance).abs() / orig_variance.abs().max(1e-300);
        assert!(
            rel_diff < 1e-9,
            "round-tripped EWMA variance drifted beyond tolerance: {orig_variance} vs {back_variance}"
        );
    }

    // Verify idiosyncratic vol_state round-trip. Cascade-assigned idiosyncratic
    // vols must also be stamped Ewma when configured; this block mirrors the
    // factors verification above.
    assert_eq!(
        model.vol_state.idiosyncratic.len(),
        back.vol_state.idiosyncratic.len(),
        "round-trip must preserve the same number of idiosyncratic vol entries"
    );
    for (issuer_id, orig_vol_model) in &model.vol_state.idiosyncratic {
        let back_vol_model = back
            .vol_state
            .idiosyncratic
            .get(issuer_id)
            .unwrap_or_else(|| {
                panic!("round-tripped vol_state missing idiosyncratic for issuer {issuer_id}")
            });
        let IdiosyncraticVolModel::Ewma {
            lambda: orig_lambda,
            variance: orig_variance,
        } = orig_vol_model
        else {
            panic!("issuer {issuer_id} must persist the EWMA estimator, got {orig_vol_model:?}");
        };
        let IdiosyncraticVolModel::Ewma {
            lambda: back_lambda,
            variance: back_variance,
        } = back_vol_model
        else {
            panic!(
                "round-tripped idiosyncratic {issuer_id} must persist the EWMA estimator, got {back_vol_model:?}"
            );
        };
        assert_eq!(
            orig_lambda, back_lambda,
            "lambda is an echoed input and must round-trip exactly"
        );
        let rel_diff = (orig_variance - back_variance).abs() / orig_variance.abs().max(1e-300);
        assert!(
            rel_diff < 1e-9,
            "round-tripped EWMA variance drifted beyond tolerance: {orig_variance} vs {back_variance}"
        );
    }
}

/// Ledoit-Wolf covariance strategy calibrates end-to-end: positive diagonal,
/// Cauchy-Schwarz-consistent off-diagonals, and a PSD matrix (enforced by
/// FactorCovarianceMatrix::new inside the calibrator).
#[test]
fn ledoit_wolf_covariance_accepted() {
    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::LedoitWolf,
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };
    let model = CreditCalibrator::new(cfg)
        .calibrate(fixture_panel().into_inputs())
        .expect("Ledoit-Wolf calibration succeeds");

    let cov = &model.config.covariance;
    let ids = cov.factor_ids().to_vec();
    assert!(
        ids.len() >= 2,
        "expected generic + bucket factors, got {}",
        ids.len()
    );
    for i in &ids {
        assert!(cov.variance(i) > 0.0, "diagonal must be positive for {i}");
    }
    for i in &ids {
        for j in &ids {
            let bound = (cov.variance(i) * cov.variance(j)).sqrt();
            assert!(
                cov.covariance(i, j).abs() <= bound + 1e-12,
                "|Σ({i},{j})| must satisfy the Cauchy-Schwarz bound"
            );
        }
    }
}

/// Pins the Ewma vol_model + LedoitWolf covariance_strategy composition: both
/// settings are independently supported, but this is the only test exercising
/// them together end-to-end. Asserts vol_state persists the Ewma variant and
/// the LedoitWolf covariance still satisfies the Cauchy-Schwarz bound.
#[test]
fn ewma_vol_model_composes_with_ledoit_wolf_covariance() {
    let cfg = CreditCalibrationConfig {
        vol_model: VolModelChoice::Ewma { lambda: 0.94 },
        covariance_strategy: CovarianceStrategy::LedoitWolf,
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };
    let model = CreditCalibrator::new(cfg)
        .calibrate(fixture_panel().into_inputs())
        .expect("Ewma + LedoitWolf calibration succeeds");

    assert!(!model.vol_state.factors.is_empty());
    for (fid, vol_model) in &model.vol_state.factors {
        let FactorVolModel::Ewma { lambda, variance } = vol_model else {
            panic!("factor {fid} must persist the Ewma variant, got {vol_model:?}");
        };
        assert!((lambda - 0.94).abs() < 1e-15);
        assert!(variance.is_finite() && *variance >= 0.0);
    }

    let cov = &model.config.covariance;
    let ids = cov.factor_ids().to_vec();
    assert!(
        ids.len() >= 2,
        "expected generic + bucket factors, got {}",
        ids.len()
    );
    for i in &ids {
        for j in &ids {
            let bound = (cov.variance(i) * cov.variance(j)).sqrt();
            assert!(
                cov.covariance(i, j).abs() <= bound + 1e-12,
                "|Σ({i},{j})| must satisfy the Cauchy-Schwarz bound"
            );
        }
    }
}

/// The LedoitWolf covariance is estimated from the factor-return panel alone, so
/// it must be bit-identical whether the vols come from the `Sample` or `Ewma`
/// estimator. This pins that independence directly: the Cauchy-Schwarz bound in
/// [`ewma_vol_model_composes_with_ledoit_wolf_covariance`] holds for *any* PSD
/// matrix and so would not catch vol-model state leaking into the stored `Σ`.
///
/// The final assertion is the anti-vacuity guard: it proves the fixture actually
/// distinguishes the two estimators, so "the covariances match" is a real
/// invariant rather than an artifact of both paths producing the same vols.
#[test]
fn ledoit_wolf_covariance_is_independent_of_vol_model() {
    let base = || CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::LedoitWolf,
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };

    let sample_model = CreditCalibrator::new(CreditCalibrationConfig {
        vol_model: VolModelChoice::Sample,
        ..base()
    })
    .calibrate(fixture_panel().into_inputs())
    .expect("Sample + LedoitWolf calibration succeeds");
    let ewma_model = CreditCalibrator::new(CreditCalibrationConfig {
        vol_model: VolModelChoice::Ewma { lambda: 0.94 },
        ..base()
    })
    .calibrate(fixture_panel().into_inputs())
    .expect("Ewma + LedoitWolf calibration succeeds");

    let sample_cov = &sample_model.config.covariance;
    let ewma_cov = &ewma_model.config.covariance;
    let ids = sample_cov.factor_ids().to_vec();
    assert_eq!(
        ids,
        ewma_cov.factor_ids().to_vec(),
        "both calibrations must produce the same factor set"
    );
    assert!(ids.len() >= 2, "expected generic + bucket factors");

    for i in &ids {
        for j in &ids {
            assert_eq!(
                sample_cov.covariance(i, j),
                ewma_cov.covariance(i, j),
                "Σ({i},{j}) must not depend on the vol model under LedoitWolf"
            );
        }
    }

    let vols_differ = ids.iter().any(|fid| {
        let sample_var = match sample_model.vol_state.factors.get(fid) {
            Some(FactorVolModel::Sample { variance }) => *variance,
            other => panic!("factor {fid} must persist the Sample variant, got {other:?}"),
        };
        let ewma_var = match ewma_model.vol_state.factors.get(fid) {
            Some(FactorVolModel::Ewma { variance, .. }) => *variance,
            other => panic!("factor {fid} must persist the Ewma variant, got {other:?}"),
        };
        (sample_var - ewma_var).abs() > 1e-12
    });
    assert!(
        vols_differ,
        "fixture must distinguish the Sample and Ewma estimators, otherwise the \
         covariance-equality assertion above is vacuous"
    );
}

/// Out-of-range lambda is rejected before any estimation runs.
#[test]
fn ewma_lambda_out_of_range_rejected_by_calibrate() {
    let cfg = CreditCalibrationConfig {
        vol_model: VolModelChoice::Ewma { lambda: 1.0 },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };
    let err = CreditCalibrator::new(cfg)
        .calibrate(fixture_panel().into_inputs())
        .expect_err("lambda = 1.0 must be rejected");
    assert!(err.to_string().contains("ewma lambda"));
}

/// PR-5b I3: Ridge must reject negative alpha.
#[test]
fn ridge_covariance_rejects_negative_alpha() {
    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::Ridge { alpha: -0.01 },
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };
    let inputs = fixture_panel().into_inputs();
    assert!(
        CreditCalibrator::new(cfg).calibrate(inputs).is_err(),
        "Ridge covariance with negative alpha must return an error"
    );
}

#[test]
fn calibration_rejects_non_finite_generic_values() {
    let cfg = config_with(
        IssuerBetaPolicy::GloballyOff,
        vec![HierarchyDimension::Rating],
    );
    let mut inputs = fixture_panel().into_inputs();
    inputs.generic_factor.values[0] = f64::NAN;

    assert!(
        CreditCalibrator::new(cfg).calibrate(inputs).is_err(),
        "calibration must reject NaN generic factor inputs"
    );
}

#[test]
fn calibration_rejects_non_finite_spread_values() {
    let cfg = config_with(
        IssuerBetaPolicy::GloballyOff,
        vec![HierarchyDimension::Rating],
    );
    let mut inputs = fixture_panel().into_inputs();
    let series = inputs
        .history_panel
        .spreads
        .get_mut(&IssuerId::new("ISSUER-A"))
        .expect("fixture issuer exists");
    series[0] = Some(f64::INFINITY);

    assert!(
        CreditCalibrator::new(cfg).calibrate(inputs).is_err(),
        "calibration must reject infinite issuer spread inputs"
    );
}

#[test]
fn calibration_rejects_invalid_numeric_config_values() {
    let cfg = CreditCalibrationConfig {
        vol_model: VolModelChoice::Ewma { lambda: 0.0 },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };

    assert!(
        CreditCalibrator::new(cfg)
            .calibrate(fixture_panel().into_inputs())
            .is_err(),
        "ewma lambda must be in (0, 1)"
    );
}

// PR-5a Test 1: caller override wins over IssuerBeta history

/// An idiosyncratic override supplied for an `IssuerBeta` issuer must win over
/// the vol computed from that issuer's residual history, and the source must
/// record `CallerSupplied`.
#[test]
fn idiosyncratic_override_wins_over_history() {
    // Use Dynamic policy with low min_history so ISSUER-A gets IssuerBeta mode.
    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 12,
        overrides: BTreeMap::new(),
    };
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            policy,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    let fixture = fixture_panel();
    let override_vol = 0.9999_f64;
    let mut overrides = BTreeMap::new();
    // Caller inputs are decimal spread volatility; model rows retain bp units.
    overrides.insert(IssuerId::new("ISSUER-A"), override_vol / 10_000.0);

    let inputs = CreditCalibrationInputs {
        idiosyncratic_overrides: overrides,
        ..fixture.into_inputs()
    };

    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    let row_a = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "ISSUER-A")
        .expect("ISSUER-A row present");

    assert!(
        matches!(row_a.mode, IssuerBetaMode::IssuerBeta),
        "ISSUER-A must be IssuerBeta"
    );
    assert!(
        (row_a.adder_vol_annualized - override_vol).abs() < 1e-12,
        "adder_vol_annualized must equal override; got {}",
        row_a.adder_vol_annualized
    );
    assert!(
        matches!(row_a.adder_vol_source, AdderVolSource::CallerSupplied),
        "adder_vol_source must be CallerSupplied; got {:?}",
        row_a.adder_vol_source
    );
}

// PR-5a Test 2: caller override wins over BucketOnly peer proxy

/// An idiosyncratic override supplied for a `BucketOnly` issuer must win over
/// the peer-proxy fallback, and the source must record `CallerSupplied`.
#[test]
fn idiosyncratic_override_wins_over_bucket_only_peer_proxy() {
    // GloballyOff → all issuers are BucketOnly.
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    let fixture = fixture_panel();
    let override_vol = 0.7777_f64;
    let mut overrides = BTreeMap::new();
    // Caller inputs are decimal spread volatility; model rows retain bp units.
    overrides.insert(IssuerId::new("ISSUER-D"), override_vol / 10_000.0);

    let inputs = CreditCalibrationInputs {
        idiosyncratic_overrides: overrides,
        ..fixture.into_inputs()
    };

    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    let row_d = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "ISSUER-D")
        .expect("ISSUER-D row present");

    assert!(
        matches!(row_d.mode, IssuerBetaMode::BucketOnly),
        "ISSUER-D must be BucketOnly"
    );
    assert!(
        (row_d.adder_vol_annualized - override_vol).abs() < 1e-12,
        "adder_vol_annualized must equal override; got {}",
        row_d.adder_vol_annualized
    );
    assert!(
        matches!(row_d.adder_vol_source, AdderVolSource::CallerSupplied),
        "adder_vol_source must be CallerSupplied; got {:?}",
        row_d.adder_vol_source
    );
}

// PR-5a Test 3: BucketOnly uses peer proxy at deepest level

/// Fixture: 1 BucketOnly issuer X tagged `{rating: IG, region: EU, sector: TECH}`
/// plus 2 IssuerBeta peers tagged `{rating: IG, region: EU, sector: BANK}`.
/// X is alone in `IG.EU.TECH` so its residual is degenerate and the cascade
/// walks up to `IG.EU`, the deepest bucket that has FromHistory peers.
#[test]
fn bucket_only_uses_peer_proxy_at_deepest_level() {
    let n = 24usize;
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);

    let generic_values: Vec<f64> = (0..n).map(|i| 0.01 * (i as f64).sin()).collect();

    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut issuer_tags_map: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut as_of_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    // 2 IssuerBeta peers in IG.EU.BANK.
    for (idx, id) in ["PEER-1", "PEER-2"].iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                Some(
                    bp(100.0)
                        + (idx as f64) * bp(20.0)
                        + 0.8 * generic_values[i]
                        + 0.1 * ((idx as f64) + (i as f64) * 0.3).sin(),
                )
            })
            .collect();
        as_of_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
        spreads.insert(issuer_id.clone(), series);
        issuer_tags_map.insert(issuer_id, tags_for_sector("IG", "EU", "BANK"));
    }

    // BucketOnly issuer X in IG.EU.TECH — singleton at the deepest level, so
    // the residual is identically zero and the peer-proxy cascade engages.
    let x_id = IssuerId::new("ISSUER-X");
    let x_series: Vec<Option<f64>> = (0..n)
        .map(|i| Some(bp(150.0) + 0.9 * generic_values[i] + 0.05 * ((i as f64) * 0.7).cos()))
        .collect();
    as_of_spreads.insert(x_id.clone(), x_series[n - 1].unwrap());
    spreads.insert(x_id.clone(), x_series);
    issuer_tags_map.insert(x_id.clone(), tags_for_sector("IG", "EU", "TECH"));

    // Policy: peers are IssuerBeta, X is BucketOnly via ForceIssuerBeta +
    // ForceBucketOnly overrides.
    let mut overrides = BTreeMap::new();
    overrides.insert(IssuerId::new("PEER-1"), IssuerBetaOverride::ForceIssuerBeta);
    overrides.insert(IssuerId::new("PEER-2"), IssuerBetaOverride::ForceIssuerBeta);
    overrides.insert(x_id, IssuerBetaOverride::ForceBucketOnly);

    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 12,
        overrides,
    };
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1, 1],
        },
        ..config_with(
            policy,
            vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Region,
                HierarchyDimension::Sector,
            ],
        )
    };

    let inputs = CreditCalibrationInputs {
        history_panel: HistoryPanel { dates, spreads },
        issuer_tags: IssuerTagPanel {
            tags: issuer_tags_map,
        },
        generic_factor: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: "CDX".to_owned(),
                series_id: "cdx".to_owned(),
            },
            values: generic_values,
        },
        as_of,
        as_of_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
        spread_durations: BTreeMap::new(),
    };

    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    // Get peer vols from the model.
    let peer1_vol = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "PEER-1")
        .map(|r| r.adder_vol_annualized)
        .expect("PEER-1 row");
    let peer2_vol = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "PEER-2")
        .map(|r| r.adder_vol_annualized)
        .expect("PEER-2 row");

    let expected_mean = (peer1_vol + peer2_vol) / 2.0;

    let row_x = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "ISSUER-X")
        .expect("ISSUER-X row present");

    assert!(
        matches!(row_x.mode, IssuerBetaMode::BucketOnly),
        "ISSUER-X must be BucketOnly"
    );
    assert!(
        (row_x.adder_vol_annualized - expected_mean).abs() < 1e-9,
        "adder_vol must equal mean of IG.EU peers ({expected_mean}); got {}",
        row_x.adder_vol_annualized
    );
    // peer_bucket must be "IG.EU" (the deepest level path where peers exist).
    assert!(
        matches!(
            &row_x.adder_vol_source,
            AdderVolSource::BucketPeerProxy { peer_bucket }
            if peer_bucket == "IG.EU"
        ),
        "adder_vol_source must be BucketPeerProxy {{ peer_bucket: \"IG.EU\" }}; got {:?}",
        row_x.adder_vol_source
    );
}

// PR-5a Test 4: peer proxy falls back to parent bucket level

/// Fixture: BucketOnly issuer X tagged `{rating: IG, region: APAC}` but there
/// are no IG.APAC IssuerBeta peers. There ARE IG.EU IssuerBeta peers.
/// X must proxy from IG level (level-0, the coarsest), not IG.APAC.
/// `peer_bucket = "IG"`.
#[test]
fn bucket_peer_proxy_falls_back_to_parent() {
    let n = 24usize;
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);

    let generic_values: Vec<f64> = (0..n).map(|i| 0.01 * (i as f64).sin()).collect();

    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut issuer_tags_map: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut as_of_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    // 2 IssuerBeta peers in IG.EU bucket (different region from X).
    for (idx, id) in ["PEER-EU-1", "PEER-EU-2"].iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                Some(
                    bp(100.0)
                        + (idx as f64) * bp(20.0)
                        + 0.8 * generic_values[i]
                        + 0.1 * ((idx as f64) + (i as f64) * 0.3).sin(),
                )
            })
            .collect();
        as_of_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
        spreads.insert(issuer_id.clone(), series);
        issuer_tags_map.insert(issuer_id, tags_for("IG", "EU"));
    }

    // BucketOnly issuer X in IG.APAC — no IG.APAC IssuerBeta peers.
    let x_id = IssuerId::new("ISSUER-X");
    // Full panel: X is alone in IG.APAC, so the residual is degenerate and
    // the peer-proxy cascade walks up to IG.
    let x_series: Vec<Option<f64>> = (0..n)
        .map(|i| Some(bp(150.0) + 0.9 * generic_values[i] + 0.05 * ((i as f64) * 0.7).cos()))
        .collect();
    as_of_spreads.insert(x_id.clone(), x_series[n - 1].unwrap());
    spreads.insert(x_id.clone(), x_series);
    issuer_tags_map.insert(x_id.clone(), tags_for("IG", "APAC"));

    let mut overrides = BTreeMap::new();
    overrides.insert(
        IssuerId::new("PEER-EU-1"),
        IssuerBetaOverride::ForceIssuerBeta,
    );
    overrides.insert(
        IssuerId::new("PEER-EU-2"),
        IssuerBetaOverride::ForceIssuerBeta,
    );
    overrides.insert(x_id, IssuerBetaOverride::ForceBucketOnly);

    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 12,
        overrides,
    };
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            policy,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    let inputs = CreditCalibrationInputs {
        history_panel: HistoryPanel { dates, spreads },
        issuer_tags: IssuerTagPanel {
            tags: issuer_tags_map,
        },
        generic_factor: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: "CDX".to_owned(),
                series_id: "cdx".to_owned(),
            },
            values: generic_values,
        },
        as_of,
        as_of_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
        spread_durations: BTreeMap::new(),
    };

    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    let peer1_vol = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "PEER-EU-1")
        .map(|r| r.adder_vol_annualized)
        .expect("PEER-EU-1 row");
    let peer2_vol = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "PEER-EU-2")
        .map(|r| r.adder_vol_annualized)
        .expect("PEER-EU-2 row");

    let expected_mean = (peer1_vol + peer2_vol) / 2.0;

    let row_x = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "ISSUER-X")
        .expect("ISSUER-X row present");

    assert!(
        (row_x.adder_vol_annualized - expected_mean).abs() < 1e-9,
        "adder_vol must equal mean of IG-level peers ({expected_mean}); got {}",
        row_x.adder_vol_annualized
    );
    // Level-1 bucket IG.APAC has no peers → fell back to level-0 bucket "IG".
    assert!(
        matches!(
            &row_x.adder_vol_source,
            AdderVolSource::BucketPeerProxy { peer_bucket }
            if peer_bucket == "IG"
        ),
        "adder_vol_source must be BucketPeerProxy {{ peer_bucket: \"IG\" }}; got {:?}",
        row_x.adder_vol_source
    );
}

// PR-5a Test 5: peer proxy cascade falls back to global mean

/// Fixture: BucketOnly issuer X tagged `{rating: HY, region: APAC}` but there
/// are NO HY.APAC or HY IssuerBeta peers anywhere. There are IG IssuerBeta
/// peers in a completely different rating bucket. X's vol must equal the global
/// mean of all IssuerBeta vols. Source = `Default`.
#[test]
fn peer_proxy_cascade_falls_back_to_global() {
    let n = 24usize;
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);

    let generic_values: Vec<f64> = (0..n).map(|i| 0.01 * (i as f64).sin()).collect();

    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut issuer_tags_map: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut as_of_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    // 2 IssuerBeta peers in IG.EU bucket (different rating bucket from X).
    for (idx, id) in ["IG-PEER-1", "IG-PEER-2"].iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                Some(
                    bp(100.0)
                        + (idx as f64) * bp(20.0)
                        + 0.8 * generic_values[i]
                        + 0.1 * ((idx as f64) + (i as f64) * 0.3).sin(),
                )
            })
            .collect();
        as_of_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
        spreads.insert(issuer_id.clone(), series);
        issuer_tags_map.insert(issuer_id, tags_for("IG", "EU"));
    }

    // BucketOnly issuer X in HY.APAC — no HY IssuerBeta peers at any level.
    let x_id = IssuerId::new("ISSUER-X");
    // Full panel: X is alone in HY.APAC with no HY IssuerBeta peers, so the
    // cascade lands on the global Default mean.
    let x_series: Vec<Option<f64>> = (0..n)
        .map(|i| Some(bp(250.0) + 1.2 * generic_values[i] + 0.08 * ((i as f64) * 0.4).cos()))
        .collect();
    as_of_spreads.insert(x_id.clone(), x_series[n - 1].unwrap());
    spreads.insert(x_id.clone(), x_series);
    issuer_tags_map.insert(x_id.clone(), tags_for("HY", "APAC"));

    let mut overrides = BTreeMap::new();
    overrides.insert(
        IssuerId::new("IG-PEER-1"),
        IssuerBetaOverride::ForceIssuerBeta,
    );
    overrides.insert(
        IssuerId::new("IG-PEER-2"),
        IssuerBetaOverride::ForceIssuerBeta,
    );
    overrides.insert(x_id, IssuerBetaOverride::ForceBucketOnly);

    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 12,
        overrides,
    };
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            policy,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    let inputs = CreditCalibrationInputs {
        history_panel: HistoryPanel { dates, spreads },
        issuer_tags: IssuerTagPanel {
            tags: issuer_tags_map,
        },
        generic_factor: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: "CDX".to_owned(),
                series_id: "cdx".to_owned(),
            },
            values: generic_values,
        },
        as_of,
        as_of_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
        spread_durations: BTreeMap::new(),
    };

    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    // Compute the global mean of IssuerBeta from-history vols.
    let ig_peer_vols: Vec<f64> = ["IG-PEER-1", "IG-PEER-2"]
        .iter()
        .map(|id| {
            model
                .issuer_betas
                .iter()
                .find(|r| r.issuer_id.as_str() == *id)
                .map(|r| r.adder_vol_annualized)
                .unwrap_or(0.0)
        })
        .collect();
    let global_mean = ig_peer_vols.iter().sum::<f64>() / (ig_peer_vols.len() as f64);

    let row_x = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id.as_str() == "ISSUER-X")
        .expect("ISSUER-X row present");

    assert!(
        (row_x.adder_vol_annualized - global_mean).abs() < 1e-9,
        "adder_vol must equal global mean ({global_mean}); got {}",
        row_x.adder_vol_annualized
    );
    assert!(
        matches!(row_x.adder_vol_source, AdderVolSource::Default),
        "adder_vol_source must be Default (global fallback); got {:?}",
        row_x.adder_vol_source
    );
}

// PR-5a Test 6: GloballyOff issuers get FromHistory adder vols

/// Under `GloballyOff` every issuer is `BucketOnly`, but adder vols are still
/// estimated from each issuer's own residual history (quant-review M3: the
/// old IssuerBeta-only gate silently zeroed every idiosyncratic vol under the
/// default policy). In this fixture every issuer is alone in its deepest
/// (rating × region) bucket, so the final-level peel absorbs the residual
/// completely and the residual series is identically zero. A self-mean
/// carries no information about idiosyncratic risk, so such series are
/// excluded from `FromHistory`; with *every* issuer a singleton there are no
/// peers to proxy from either, and the cascade lands on the honest
/// `Default` 0.0 ("no data"), not a `FromHistory` "estimate" of 0.0.
#[test]
fn globally_off_issuers_get_from_history_adder_vols() {
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    let inputs = fixture_panel().into_inputs();
    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    for row in &model.issuer_betas {
        assert!(
            matches!(row.mode, IssuerBetaMode::BucketOnly),
            "mode must be BucketOnly"
        );
        assert!(
            matches!(row.adder_vol_source, AdderVolSource::Default),
            "identically-zero singleton residuals must fall through to the \
             Default source (no usable history, no peers anywhere); \
             got {:?} for {:?}",
            row.adder_vol_source,
            row.issuer_id.as_str()
        );
        assert!(
            row.adder_vol_annualized.abs() < 1e-9,
            "with no from-history vols anywhere the Default fallback is 0.0; \
             got {} for {:?}",
            row.adder_vol_annualized,
            row.issuer_id.as_str()
        );
    }
}

/// The `Default` 0.0 fallback now only applies when no issuer anywhere has
/// enough residual history for a `FromHistory` estimate (fewer than 2 usable
/// observations in the working panel).
#[test]
fn adder_vol_defaults_to_zero_when_history_too_short_everywhere() {
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(2, as_of);
    let generic_values = vec![bp(100.0), bp(100.5)];

    let issuer = IssuerId::new("ISSUER-A");
    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    spreads.insert(issuer.clone(), vec![Some(bp(120.0)), Some(bp(121.0))]);
    let mut tags: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    tags.insert(issuer.clone(), tags_for("IG", "EU"));
    let mut as_of_spreads = BTreeMap::new();
    as_of_spreads.insert(issuer, bp(121.0));

    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    let inputs = CreditCalibrationInputs {
        history_panel: HistoryPanel { dates, spreads },
        issuer_tags: IssuerTagPanel { tags },
        generic_factor: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: "CDX".to_owned(),
                series_id: "cdx".to_owned(),
            },
            values: generic_values,
        },
        as_of,
        as_of_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
        spread_durations: BTreeMap::new(),
    };

    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("calibration succeeds");

    let row = &model.issuer_betas[0];
    assert!(
        row.adder_vol_annualized.abs() < 1e-12,
        "one usable return is below the FromHistory minimum; vol must be 0.0"
    );
    assert!(
        matches!(row.adder_vol_source, AdderVolSource::Default),
        "adder_vol_source must be Default; got {:?}",
        row.adder_vol_source
    );
}

// PR-5b Test 1: Ridge covariance adds alpha to diagonal

/// Verify that `CovarianceStrategy::Ridge { alpha }` produces Σ where:
/// - off-diagonal entries equal D·ρ·D (sample covariance off-diagonals)
/// - diagonal entries equal D·ρ·D diagonal + alpha
#[test]
fn ridge_covariance_adds_alpha_to_diagonal() {
    let alpha = 0.25_f64;
    let cfg_ridge = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::Ridge { alpha },
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::Dynamic {
                min_history: 12,
                overrides: BTreeMap::new(),
            },
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };
    // Baseline with alpha=0 to get the unridged D·ρ·D covariance.
    let cfg_zero = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::Ridge { alpha: 0.0 },
        ..cfg_ridge.clone()
    };

    let model_ridge = CreditCalibrator::new(cfg_ridge)
        .calibrate(fixture_panel().into_inputs())
        .expect("ridge calibration succeeds");
    let model_zero = CreditCalibrator::new(cfg_zero)
        .calibrate(fixture_panel().into_inputs())
        .expect("zero-alpha ridge calibration succeeds");

    let cov_ridge = &model_ridge.config.covariance;
    let cov_zero = &model_zero.config.covariance;

    let n = cov_ridge.factor_ids().len();
    assert!(n > 0, "at least one factor must exist");

    let data_ridge = cov_ridge.as_slice();
    let data_zero = cov_zero.as_slice();

    for i in 0..n {
        for j in 0..n {
            let val_ridge = data_ridge[i * n + j];
            let val_zero = data_zero[i * n + j];
            if i == j {
                let diff = val_ridge - val_zero;
                assert!(
                    (diff - alpha).abs() < 1e-12,
                    "diagonal [{i}]: ridge - zero should equal alpha={alpha}; got diff={diff}"
                );
            } else {
                // Off-diagonal must be unchanged.
                assert!(
                    (val_ridge - val_zero).abs() < 1e-12,
                    "off-diagonal [{i}][{j}]: ridge and zero should be equal; ridge={val_ridge} zero={val_zero}"
                );
            }
        }
    }

    // The static_correlation must record Pearson ρ (not identity).
    let corr = &model_ridge.static_correlation;
    assert_eq!(
        corr.factor_ids.len(),
        n,
        "static_correlation must have same factor count"
    );

    // For a highly correlated fixture the correlation matrix should NOT be identity
    // (at least one off-diagonal entry should deviate from 0.0).
    let has_nonzero_offdiag = corr.data.iter().enumerate().any(|(i, row)| {
        row.iter()
            .enumerate()
            .any(|(j, &v)| i != j && v.abs() > 1e-6)
    });
    assert!(
        has_nonzero_offdiag,
        "Ridge static_correlation should be sample Pearson ρ, not identity"
    );
}

// PR-5b Test 2: FullSampleRepaired covariance is PSD

/// Fixture where n_factors > n_obs makes naive sample covariance non-PSD.
/// Verify that `CovarianceStrategy::FullSampleRepaired` produces a covariance
/// matrix with all non-negative eigenvalues.
#[test]
fn full_sample_repaired_covariance_is_psd() {
    use finstack_quant_core::math::linalg::symmetric_eigen;

    // Build a very sparse panel: 3 dates → 2 return observations, but 6
    // factors from a 2-level hierarchy. n_factors=7 (generic+6 buckets) > n_obs=2
    // causes the sample correlation to be rank-deficient and non-PSD.
    let n = 3usize; // 3 dates → 2 returns
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);
    let generic_values: Vec<f64> = vec![0.0, 0.01, -0.004];

    let issuer_specs = [
        ("ISSUER-A", "IG", "EU"),
        ("ISSUER-B", "IG", "NA"),
        ("ISSUER-C", "IG", "APAC"),
        ("ISSUER-D", "HY", "EU"),
        ("ISSUER-E", "HY", "NA"),
        ("ISSUER-F", "HY", "APAC"),
    ];

    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut tags: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut as_of_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    for (idx, (id, rating, region)) in issuer_specs.iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        // Deterministic series with slight variation to avoid exact collinearity
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                Some(
                    bp(100.0)
                        + (idx as f64) * bp(10.0)
                        + generic_values[i]
                        + 0.01 * ((idx * n + i) as f64).sin(),
                )
            })
            .collect();
        as_of_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
        spreads.insert(issuer_id.clone(), series);
        tags.insert(issuer_id, tags_for(rating, region));
    }

    let inputs = CreditCalibrationInputs {
        history_panel: HistoryPanel { dates, spreads },
        issuer_tags: IssuerTagPanel { tags },
        generic_factor: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: "CDX".to_owned(),
                series_id: "cdx".to_owned(),
            },
            values: generic_values,
        },
        as_of,
        as_of_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
        spread_durations: BTreeMap::new(),
    };

    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::FullSampleRepaired,
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    // Capture before inputs is consumed by calibrate().
    let n_dates = inputs.history_panel.dates.len();

    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("FullSampleRepaired calibration must succeed");

    // Structural sanity: n_factors > n_obs guarantees the unrepaired sample
    // correlation is rank-deficient, exercising the repair branch.
    let n_factors = model.config.factors.len();
    let n_obs = n_dates - 1; // returns = dates - 1
    assert!(
        n_factors > n_obs,
        "fixture must have n_factors ({n_factors}) > n_obs ({n_obs}) to exercise repair"
    );

    model.validate().expect("model must validate");

    // Extract the covariance data and verify all eigenvalues ≥ 0.
    let cov = &model.config.covariance;
    let n_f = cov.factor_ids().len();
    assert!(n_f > 0, "at least one factor must be present");

    let (eigenvalues, _) =
        symmetric_eigen(cov.as_slice(), n_f).expect("symmetric_eigen must succeed on covariance");

    let min_eig = eigenvalues.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(
        min_eig >= -1e-10,
        "FullSampleRepaired covariance must have all eigenvalues ≥ 0; min = {min_eig}"
    );
}

// PR-5b Test 3: Ridge covariance preserves determinism

#[test]
fn ridge_covariance_preserves_determinism() {
    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::Ridge { alpha: 0.05 },
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::Dynamic {
                min_history: 12,
                overrides: BTreeMap::new(),
            },
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    let model_a = CreditCalibrator::new(cfg.clone())
        .calibrate(fixture_panel().into_inputs())
        .expect("calibration A");
    let model_b = CreditCalibrator::new(cfg)
        .calibrate(fixture_panel().into_inputs())
        .expect("calibration B");

    let json_a = serde_json::to_string(&model_a).expect("serialize A");
    let json_b = serde_json::to_string(&model_b).expect("serialize B");
    assert_eq!(
        json_a, json_b,
        "Ridge calibration must be bit-identical for same inputs"
    );
}

// PR-5b Test 4: FullSampleRepaired preserves determinism

#[test]
fn full_sample_repaired_preserves_determinism() {
    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::FullSampleRepaired,
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        ..config_with(
            IssuerBetaPolicy::Dynamic {
                min_history: 12,
                overrides: BTreeMap::new(),
            },
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };

    let model_a = CreditCalibrator::new(cfg.clone())
        .calibrate(fixture_panel().into_inputs())
        .expect("calibration A");
    let model_b = CreditCalibrator::new(cfg)
        .calibrate(fixture_panel().into_inputs())
        .expect("calibration B");

    let json_a = serde_json::to_string(&model_a).expect("serialize A");
    let json_b = serde_json::to_string(&model_b).expect("serialize B");
    assert_eq!(
        json_a, json_b,
        "FullSampleRepaired calibration must be bit-identical for same inputs"
    );
}

// PR-5b Test 5: Golden artifact regression test

const REGEN_CREDIT_FACTOR_MODEL_GOLDEN_ENV: &str = "FQ_UPDATE_CANONICAL_GOLDENS";

fn calibrate_canonical_credit_model() -> CreditFactorModel {
    let mut inputs = fixture_panel().into_inputs();
    inputs.spread_durations = inputs
        .as_of_spreads
        .keys()
        .cloned()
        .map(|id| (id, 5.0))
        .collect();
    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::Diagonal,
        vol_model: VolModelChoice::Sample,
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        bucket_weighting: BucketWeighting::Dts,
        ..config_with(
            IssuerBetaPolicy::Dynamic {
                min_history: 12,
                overrides: BTreeMap::new(),
            },
            vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        )
    };
    CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("canonical credit model calibrates")
}

fn stabilize_canonical_bytes(model: &CreditFactorModel) -> Vec<u8> {
    let mut bytes =
        finstack_quant_core::to_canonical_bytes(model).expect("credit factor model canonicalizes");
    for _ in 0..8 {
        let (reloaded, _) = CreditFactorModel::from_slice_strict(
            &bytes,
            &finstack_quant_core::contract::LoadLimits::default(),
        )
        .expect("reload while stabilizing canonical bytes");
        let next = finstack_quant_core::to_canonical_bytes(&reloaded)
            .expect("reloaded model canonicalizes");
        if next == bytes {
            return bytes;
        }
        bytes = next;
    }
    panic!("credit factor model canonical bytes did not reach a JSON f64 fixed point")
}

/// Generate (or regenerate) the factor-model-owned canonical artifact and hash.
/// Run manually with:
/// `FQ_UPDATE_CANONICAL_GOLDENS=1 cargo test -p finstack-quant-valuations --test credit_calibration generate_golden_artifact -- --nocapture`
#[test]
fn generate_golden_artifact() {
    if std::env::var_os(REGEN_CREDIT_FACTOR_MODEL_GOLDEN_ENV).is_none() {
        return;
    }

    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../models/tests/data/canonical/credit_factor_model.json"
    );
    let hash_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../models/tests/data/canonical/credit_factor_model.sha256"
    );

    let model = calibrate_canonical_credit_model();
    let json = stabilize_canonical_bytes(&model);
    let (reloaded, _) = CreditFactorModel::from_slice_strict(
        &json,
        &finstack_quant_core::contract::LoadLimits::default(),
    )
    .expect("reload stabilized golden");
    let hash = finstack_quant_core::content_hash(&reloaded).expect("hash canonical model JSON");
    std::fs::create_dir_all(std::path::Path::new(golden_path).parent().unwrap())
        .expect("create golden dir");
    std::fs::write(golden_path, &json).expect("write golden file");
    std::fs::write(hash_path, format!("{hash}\n")).expect("write golden hash");
    println!("Canonical fixture written to {golden_path}");
}

/// Calibrate with the canonical fixture (Diagonal + Sample), serialize to
/// pretty-printed JSON, and compare byte-for-byte against the checked-in
/// factor-model fixture at
/// `../models/tests/data/canonical/credit_factor_model.json`.
///
/// On first run after generating the golden file, this test confirms the
/// file matches a fresh calibration. On subsequent runs it catches any
/// accidental changes to the serialization or calibration math.
#[test]
fn golden_credit_factor_model_matches_checked_in_json() {
    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../models/tests/data/canonical/credit_factor_model.json"
    );

    let model = calibrate_canonical_credit_model();
    let produced = stabilize_canonical_bytes(&model);

    let golden = std::fs::read(golden_path).unwrap_or_else(|e| {
        panic!(
            "Golden file not found at {golden_path}: {e}\n\
             Bootstrap it by running:\n  \
             {REGEN_CREDIT_FACTOR_MODEL_GOLDEN_ENV}=1 cargo nextest run -p finstack-quant-models \
             --test factor_canonical_contract credit_factor_model_has_exact_canonical_bytes_and_hash"
        )
    });

    assert_eq!(
        produced, golden,
        "Calibration output does not match golden file at {golden_path}.\n\
         If this change is intentional, regenerate the golden file."
    );
}

// Serde round-trip tests (PR-9 Fix 1)

/// `CreditCalibrationConfig` must round-trip through JSON without loss.
///
/// Tests the default config as well as non-default variants of each enum
/// field to confirm the serde derives are correct and match the schema
/// (snake_case unit variants, externally-tagged struct variants).
#[test]
fn calibration_config_round_trips_through_json() {
    // 1. Default config (the simple case: all unit-variant enums).
    let default_cfg = CreditCalibrationConfig::default();
    let json = serde_json::to_string(&default_cfg).expect("serialize default config");
    let back: CreditCalibrationConfig =
        serde_json::from_str(&json).expect("deserialize default config");
    // Compare field-by-field via Debug since CreditCalibrationConfig doesn't impl PartialEq.
    assert_eq!(format!("{:?}", default_cfg), format!("{:?}", back));

    // 2. Config with struct-variant enums (Ridge, TowardOne, Ewma).
    let complex_cfg = CreditCalibrationConfig {
        policy: IssuerBetaPolicy::GloballyOff,
        hierarchy: CreditHierarchySpec {
            levels: vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        },
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![3, 5],
        },
        vol_model: VolModelChoice::Ewma { lambda: 0.94 },
        covariance_strategy: CovarianceStrategy::Ridge { alpha: 0.01 },
        beta_shrinkage: BetaShrinkage::TowardOne { alpha: 0.2 },
        use_returns_or_levels: PanelSpace::Returns,
        panel_frequency: PanelFrequency::Monthly,
        bucket_weighting: BucketWeighting::Equal,
    };
    let json2 = serde_json::to_string(&complex_cfg).expect("serialize complex config");
    let back2: CreditCalibrationConfig =
        serde_json::from_str(&json2).expect("deserialize complex config");
    assert_eq!(format!("{:?}", complex_cfg), format!("{:?}", back2));

    // Spot-check that struct variants serialize in schema-compatible form.
    let v: serde_json::Value = serde_json::from_str(&json2).expect("parse complex config as Value");
    assert_eq!(
        v["vol_model"],
        serde_json::json!({"ewma": {"lambda": 0.94}}),
        "VolModelChoice::Ewma must serialize as {{\"ewma\": {{\"lambda\": ...}}}}"
    );
    assert_eq!(
        v["covariance_strategy"],
        serde_json::json!({"ridge": {"alpha": 0.01}}),
        "CovarianceStrategy::Ridge must serialize as {{\"ridge\": {{\"alpha\": ...}}}}"
    );
    assert_eq!(
        v["beta_shrinkage"],
        serde_json::json!({"toward_one": {"alpha": 0.2}}),
        "BetaShrinkage::TowardOne must serialize as {{\"toward_one\": {{\"alpha\": ...}}}}"
    );
}

/// Serialize a default `CreditCalibrationConfig` and validate the JSON against
/// `credit_calibration_config.schema.json`.
#[test]
fn calibration_config_serialization_matches_schema() {
    let schema = finstack_quant_models::factor::schema::credit_calibration_config_schema()
        .expect("embedded schema must be valid JSON");

    // Use a non-trivial config so validation exercises required fields.
    let cfg = CreditCalibrationConfig {
        policy: IssuerBetaPolicy::GloballyOff,
        hierarchy: CreditHierarchySpec {
            levels: vec![HierarchyDimension::Rating],
        },
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![5] },
        vol_model: VolModelChoice::Sample,
        covariance_strategy: CovarianceStrategy::Diagonal,
        beta_shrinkage: BetaShrinkage::None,
        use_returns_or_levels: PanelSpace::Returns,
        panel_frequency: PanelFrequency::Monthly,
        bucket_weighting: BucketWeighting::Equal,
    };

    let instance: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&cfg).expect("serialize config"))
            .expect("re-parse as Value");

    let validator = jsonschema::validator_for(schema).expect("schema must compile");
    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|e| {
            let path = e.instance_path.to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{path}: {e}")
            }
        })
        .collect();
    assert!(
        errors.is_empty(),
        "CreditCalibrationConfig serialization failed schema validation:\n  {}",
        errors.join("\n  ")
    );
}

// as_of_spreads must cover exactly the calibrated universe

/// A history issuer missing from `as_of_spreads` previously got a silent
/// `adder_at_anchor = 0.0` and shifted every bucket peer's anchor mean.
#[test]
fn calibration_rejects_asof_spreads_missing_a_history_issuer() {
    let mut inputs = fixture_panel().into_inputs();
    inputs.as_of_spreads.remove(&IssuerId::new("ISSUER-B"));

    let cfg = config_with(
        IssuerBetaPolicy::GloballyOff,
        vec![HierarchyDimension::Rating, HierarchyDimension::Region],
    );
    let err = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect_err("missing as_of spread must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("ISSUER-B") && msg.contains("as_of_spreads"),
        "error must name the missing issuer: {msg}"
    );
}

/// An asof-only issuer would silently enter anchor bucket means with unit
/// betas while receiving no artifact row.
#[test]
fn calibration_rejects_asof_only_issuer() {
    let mut inputs = fixture_panel().into_inputs();
    inputs
        .as_of_spreads
        .insert(IssuerId::new("ISSUER-GHOST"), 175.0);

    let cfg = config_with(
        IssuerBetaPolicy::GloballyOff,
        vec![HierarchyDimension::Rating, HierarchyDimension::Region],
    );
    let err = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect_err("asof-only issuer must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("ISSUER-GHOST"),
        "error must name the anchor-only issuer: {msg}"
    );
}

// '.' in hierarchy tag values corrupts factor identity

#[test]
fn calibration_rejects_dotted_tag_values() {
    let mut inputs = fixture_panel().into_inputs();
    // "Cons.Disc" would mis-segment bucket paths in synth_tags_from_path,
    // the fold-up parent computation, and the matcher's factor IDs.
    inputs.issuer_tags.tags.insert(
        IssuerId::new("ISSUER-A"),
        IssuerTags(BTreeMap::from([
            ("rating".to_owned(), "IG".to_owned()),
            ("region".to_owned(), "Cons.Disc".to_owned()),
        ])),
    );

    let cfg = config_with(
        IssuerBetaPolicy::GloballyOff,
        vec![HierarchyDimension::Rating, HierarchyDimension::Region],
    );
    let err = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect_err("dotted tag value must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("Cons.Disc") && msg.contains("separator"),
        "error must name the offending tag value: {msg}"
    );
}

// unsorted/duplicated date grids are rejected

#[test]
fn calibration_rejects_unsorted_or_duplicated_dates() {
    let mut inputs = fixture_panel().into_inputs();
    let n = inputs.history_panel.dates.len();
    inputs.history_panel.dates.swap(0, 1);

    let cfg = config_with(
        IssuerBetaPolicy::GloballyOff,
        vec![HierarchyDimension::Rating, HierarchyDimension::Region],
    );
    let err = CreditCalibrator::new(cfg.clone())
        .calibrate(inputs)
        .expect_err("unsorted dates must be rejected");
    assert!(err.to_string().contains("strictly increasing"));

    let mut inputs = fixture_panel().into_inputs();
    inputs.history_panel.dates[1] = inputs.history_panel.dates[0];
    assert_eq!(inputs.history_panel.dates.len(), n);
    let err = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect_err("duplicated dates must be rejected");
    assert!(err.to_string().contains("strictly increasing"));
}
