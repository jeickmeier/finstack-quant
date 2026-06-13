//! Integration tests for [`finstack_factor_model::credit::calibration`].
//!
//! Implements the seven required PR-4 tests from the design.

use std::collections::{BTreeMap, BTreeSet};

use finstack_core::dates::{create_date, Date};
use finstack_core::types::IssuerId;
use finstack_factor_model::credit::hierarchy::{
    AdderVolSource, CreditFactorModel, CreditHierarchySpec, FactorVolModel, GenericFactorSpec,
    HierarchyDimension, IssuerBetaMode, IssuerBetaOverride, IssuerBetaPolicy, IssuerTags,
};
use finstack_factor_model::{
    BetaShrinkage, BucketSizeThresholds, CovarianceStrategy, CreditCalibrationConfig,
    CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries, HistoryPanel, IssuerTagPanel,
    PanelSpace, VolModelChoice,
};
use time::Month;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn d(year: i32, month: Month, day: u8) -> Date {
    create_date(year, month, day).expect("valid date")
}

/// 24-month grid of monthly dates ending at as_of.
fn monthly_dates(n: usize, end: Date) -> Vec<Date> {
    let mut out = Vec::with_capacity(n);
    let mut current = end;
    for _ in 0..n {
        out.push(current);
        // Naively step back by ~30 days. Calendar-month exactness doesn't
        // matter for the calibration math (only ordering does).
        for _ in 0..30 {
            current = current.previous_day().expect("date in range");
        }
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

/// Synthesize a deterministic 24-month panel with 6 issuers in 2 ratings × 3 regions.
fn fixture_panel() -> CalibrationFixture {
    let n = 24;
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);

    // Generic factor: simple deterministic increments.
    let generic_values: Vec<f64> = (0..n).map(|i| 100.0 + 0.5 * (i as f64).sin()).collect();

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
    let mut asof_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    for (idx, (id, rating, region)) in issuer_specs.iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        let base = 100.0 + (idx as f64) * 25.0;
        let beta_pc = 0.7 + 0.05 * (idx as f64);
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                let val = base
                    + beta_pc * (generic_values[i] - 100.0)
                    + 0.1 * ((idx as f64) + (i as f64) * 0.5).cos();
                Some(val)
            })
            .collect();
        asof_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
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
        asof_spreads,
    }
}

struct CalibrationFixture {
    history: HistoryPanel,
    tags: IssuerTagPanel,
    generic: GenericFactorSeries,
    as_of: Date,
    asof_spreads: BTreeMap<IssuerId, f64>,
}

impl CalibrationFixture {
    fn into_inputs(self) -> CreditCalibrationInputs {
        CreditCalibrationInputs {
            history_panel: self.history,
            issuer_tags: self.tags,
            generic_factor: self.generic,
            as_of: self.as_of,
            asof_spreads: self.asof_spreads,
            idiosyncratic_overrides: BTreeMap::new(),
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
        annualization_factor: 12.0,
    }
}

// ---------------------------------------------------------------------------
// PR-4 Test 1: bit-identical determinism
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-4 Test 2: GloballyOff sets all betas to 1.0
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-4 Test 3: Dynamic policy classifies short history as BucketOnly
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-4 Test 4: ForceIssuerBeta override wins over short-history rule
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-4 Test 5: Sparse bucket folds to parent
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-4 Test 6: Single-level hierarchy → expected factor IDs
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-4 Test 7: All-BucketOnly calibration succeeds
// ---------------------------------------------------------------------------

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
        .contains_key(&finstack_factor_model::FactorId::new("credit::generic")));
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

// ---------------------------------------------------------------------------
// I-2: sparse bucket emits None for empty dates (factor variance excludes gap)
// ---------------------------------------------------------------------------

/// Fixture with 2 issuers, each the sole member of its Rating bucket.
/// On date index 5 (0-based), ISSUER-IG has no observation (spread = None).
/// That date should produce a `None` factor observation for the IG bucket,
/// which must be excluded from the annualized variance calculation.
#[test]
fn sparse_bucket_emits_none_for_empty_dates() {
    let n = 12usize; // 12-month panel
    let as_of = d(2024, Month::December, 31);
    let dates = monthly_dates(n, as_of);

    let generic_values: Vec<f64> = (0..n).map(|i| 0.5 * (i as f64).sin()).collect();

    // Two issuers: IG (sole member of its bucket) and HY (sole member of its bucket).
    // IG is missing on date index 5.
    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut issuer_tags_map: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut asof_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    let ig_id = IssuerId::new("ISSUER-IG");
    let hy_id = IssuerId::new("ISSUER-HY");

    // IG series: present on all dates except index 5.
    let ig_series: Vec<Option<f64>> = (0..n)
        .map(|i| {
            if i == 5 {
                None
            } else {
                Some(100.0 + 0.8 * generic_values[i] + 0.05 * (i as f64).cos())
            }
        })
        .collect();
    asof_spreads.insert(ig_id.clone(), ig_series[n - 1].unwrap());
    spreads.insert(ig_id.clone(), ig_series);
    let mut ig_tags_map = BTreeMap::new();
    ig_tags_map.insert("rating".to_owned(), "IG".to_owned());
    issuer_tags_map.insert(ig_id, IssuerTags(ig_tags_map));

    // HY series: fully observed.
    let hy_series: Vec<Option<f64>> = (0..n)
        .map(|i| Some(200.0 + 1.2 * generic_values[i] + 0.03 * (i as f64).sin()))
        .collect();
    asof_spreads.insert(hy_id.clone(), hy_series[n - 1].unwrap());
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
        asof_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
    };

    // Use GloballyOff so betas=1 and the IG factor series = issuer's residual mean.
    let cfg = CreditCalibrationConfig {
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };

    let model = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect("sparse-panel calibration succeeds");
    model.validate().expect("model validates");

    // The IG factor history is in returns space (dates[1..]). A missing spread
    // at index 5 makes both return[4] (spread[5]-spread[4]) and return[5]
    // (spread[6]-spread[5]) uncomputable. The empty-bucket entries are stored
    // as 0.0 (the dense-compatible sentinel) so that FactorHistories can
    // round-trip through JSON without serde type errors.
    let fh = model
        .factor_histories
        .as_ref()
        .expect("factor_histories present");
    let ig_factor_id = finstack_factor_model::FactorId::new("credit::level0::Rating::IG");
    let ig_history = fh
        .values
        .get(&ig_factor_id)
        .expect("IG factor history present");

    // No NaN values must appear in the stored history (ensuring JSON round-trip).
    assert!(
        ig_history.iter().all(|v| v.is_finite()),
        "IG factor history must contain no NaN or Inf (JSON round-trip requirement)"
    );

    // Exactly 2 zero-sentinel entries must appear (the two return periods that
    // straddle the missing spread at index 5).
    let zero_count = ig_history.iter().filter(|&&v| v == 0.0).count();
    assert_eq!(
        zero_count, 2,
        "IG factor history must contain exactly 2 zero sentinels (empty-bucket dates)"
    );

    // The factor variance is computed before flattening, over only the observed
    // (Some) values. It must be strictly positive (the IG series is non-constant).
    let vol_entry = model
        .vol_state
        .factors
        .get(&ig_factor_id)
        .expect("IG factor vol present");
    let FactorVolModel::Sample { variance } = vol_entry;
    assert!(
        *variance > 0.0,
        "IG factor variance must be positive (computed over non-zero-sentinel dates)"
    );

    // Round-trip serialization must succeed without error.
    let json = serde_json::to_string(&model).expect("serialize succeeds");
    let model2: CreditFactorModel = serde_json::from_str(&json).expect("deserialize succeeds");
    assert_eq!(model.as_of, model2.as_of, "round-trip preserves as_of");
}

// ---------------------------------------------------------------------------
// Additional: unsupported PR-5a/b features error cleanly
// ---------------------------------------------------------------------------

#[test]
fn rejects_garch_vol_model() {
    let cfg = CreditCalibrationConfig {
        vol_model: VolModelChoice::Garch,
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };
    let inputs = fixture_panel().into_inputs();
    assert!(CreditCalibrator::new(cfg).calibrate(inputs).is_err());
}

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
        annualization_factor: 0.0,
        ..config_with(
            IssuerBetaPolicy::GloballyOff,
            vec![HierarchyDimension::Rating],
        )
    };

    assert!(
        CreditCalibrator::new(cfg)
            .calibrate(fixture_panel().into_inputs())
            .is_err(),
        "annualization_factor must be positive and finite"
    );
}

// ---------------------------------------------------------------------------
// PR-5a Test 1: caller override wins over IssuerBeta history
// ---------------------------------------------------------------------------

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
    overrides.insert(IssuerId::new("ISSUER-A"), override_vol);

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

// ---------------------------------------------------------------------------
// PR-5a Test 2: caller override wins over BucketOnly peer proxy
// ---------------------------------------------------------------------------

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
    overrides.insert(IssuerId::new("ISSUER-D"), override_vol);

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

// ---------------------------------------------------------------------------
// PR-5a Test 3: BucketOnly uses peer proxy at deepest level
// ---------------------------------------------------------------------------

/// Fixture: 1 BucketOnly issuer X with tags `{rating: IG, region: EU}` plus
/// 2 IssuerBeta peers also tagged `{rating: IG, region: EU}`.
/// X's adder vol must equal the mean of those 2 peers' vols, and the
/// `peer_bucket` must be `"IG.EU"` (the deepest level = level-1 path).
#[test]
fn bucket_only_uses_peer_proxy_at_deepest_level() {
    let n = 24usize;
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);

    let generic_values: Vec<f64> = (0..n).map(|i| 0.5 * (i as f64).sin()).collect();

    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut issuer_tags_map: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut asof_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    // 2 IssuerBeta peers in IG.EU bucket.
    for (idx, id) in ["PEER-1", "PEER-2"].iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                Some(
                    100.0
                        + (idx as f64) * 20.0
                        + 0.8 * generic_values[i]
                        + 0.1 * ((idx as f64) + (i as f64) * 0.3).sin(),
                )
            })
            .collect();
        asof_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
        spreads.insert(issuer_id.clone(), series);
        issuer_tags_map.insert(issuer_id, tags_for("IG", "EU"));
    }

    // BucketOnly issuer X in the same IG.EU bucket.
    let x_id = IssuerId::new("ISSUER-X");
    // Only the last two dates are observed -> a single usable return, which
    // is below the 2-observation minimum for a FromHistory adder vol, so the
    // peer-proxy cascade must engage (issuers with sufficient residual
    // history now always take their own FromHistory vol, any mode).
    let x_series: Vec<Option<f64>> = (0..n)
        .map(|i| {
            (i >= n - 2).then(|| 150.0 + 0.9 * generic_values[i] + 0.05 * ((i as f64) * 0.7).cos())
        })
        .collect();
    asof_spreads.insert(x_id.clone(), x_series[n - 1].unwrap());
    spreads.insert(x_id.clone(), x_series);
    issuer_tags_map.insert(x_id.clone(), tags_for("IG", "EU"));

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
        asof_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
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

// ---------------------------------------------------------------------------
// PR-5a Test 4: peer proxy falls back to parent bucket level
// ---------------------------------------------------------------------------

/// Fixture: BucketOnly issuer X tagged `{rating: IG, region: APAC}` but there
/// are no IG.APAC IssuerBeta peers. There ARE IG.EU IssuerBeta peers.
/// X must proxy from IG level (level-0, the coarsest), not IG.APAC.
/// `peer_bucket = "IG"`.
#[test]
fn bucket_peer_proxy_falls_back_to_parent() {
    let n = 24usize;
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);

    let generic_values: Vec<f64> = (0..n).map(|i| 0.5 * (i as f64).sin()).collect();

    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut issuer_tags_map: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut asof_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    // 2 IssuerBeta peers in IG.EU bucket (different region from X).
    for (idx, id) in ["PEER-EU-1", "PEER-EU-2"].iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                Some(
                    100.0
                        + (idx as f64) * 20.0
                        + 0.8 * generic_values[i]
                        + 0.1 * ((idx as f64) + (i as f64) * 0.3).sin(),
                )
            })
            .collect();
        asof_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
        spreads.insert(issuer_id.clone(), series);
        issuer_tags_map.insert(issuer_id, tags_for("IG", "EU"));
    }

    // BucketOnly issuer X in IG.APAC — no IG.APAC IssuerBeta peers.
    let x_id = IssuerId::new("ISSUER-X");
    // Only the last two dates are observed -> a single usable return, which
    // is below the 2-observation minimum for a FromHistory adder vol, so the
    // peer-proxy cascade must engage (issuers with sufficient residual
    // history now always take their own FromHistory vol, any mode).
    let x_series: Vec<Option<f64>> = (0..n)
        .map(|i| {
            (i >= n - 2).then(|| 150.0 + 0.9 * generic_values[i] + 0.05 * ((i as f64) * 0.7).cos())
        })
        .collect();
    asof_spreads.insert(x_id.clone(), x_series[n - 1].unwrap());
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
        asof_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
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

// ---------------------------------------------------------------------------
// PR-5a Test 5: peer proxy cascade falls back to global mean
// ---------------------------------------------------------------------------

/// Fixture: BucketOnly issuer X tagged `{rating: HY, region: APAC}` but there
/// are NO HY.APAC or HY IssuerBeta peers anywhere. There are IG IssuerBeta
/// peers in a completely different rating bucket. X's vol must equal the global
/// mean of all IssuerBeta vols. Source = `Default`.
#[test]
fn peer_proxy_cascade_falls_back_to_global() {
    let n = 24usize;
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);

    let generic_values: Vec<f64> = (0..n).map(|i| 0.5 * (i as f64).sin()).collect();

    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut issuer_tags_map: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    let mut asof_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    // 2 IssuerBeta peers in IG.EU bucket (different rating bucket from X).
    for (idx, id) in ["IG-PEER-1", "IG-PEER-2"].iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                Some(
                    100.0
                        + (idx as f64) * 20.0
                        + 0.8 * generic_values[i]
                        + 0.1 * ((idx as f64) + (i as f64) * 0.3).sin(),
                )
            })
            .collect();
        asof_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
        spreads.insert(issuer_id.clone(), series);
        issuer_tags_map.insert(issuer_id, tags_for("IG", "EU"));
    }

    // BucketOnly issuer X in HY.APAC — no HY IssuerBeta peers at any level.
    let x_id = IssuerId::new("ISSUER-X");
    // Only the last two dates are observed -> a single usable return, which
    // is below the 2-observation minimum for a FromHistory adder vol, so the
    // peer-proxy cascade must engage (issuers with sufficient residual
    // history now always take their own FromHistory vol, any mode).
    let x_series: Vec<Option<f64>> = (0..n)
        .map(|i| {
            (i >= n - 2).then(|| 250.0 + 1.2 * generic_values[i] + 0.08 * ((i as f64) * 0.4).cos())
        })
        .collect();
    asof_spreads.insert(x_id.clone(), x_series[n - 1].unwrap());
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
        asof_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
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

// ---------------------------------------------------------------------------
// PR-5a Test 6: GloballyOff issuers get FromHistory adder vols
// ---------------------------------------------------------------------------

/// Under `GloballyOff` every issuer is `BucketOnly`, but adder vols are still
/// estimated from each issuer's own residual history (quant-review M3: the
/// old IssuerBeta-only gate silently zeroed every idiosyncratic vol under the
/// default policy). In this fixture every issuer is alone in its deepest
/// (rating × region) bucket, so the final-level peel absorbs the residual
/// completely and the FromHistory vol is exactly 0.0 — the bucket factor
/// carries that risk instead. The source must still be `FromHistory`, not the
/// `Default` fallback.
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
            matches!(row.adder_vol_source, AdderVolSource::FromHistory),
            "adder_vol_source must be FromHistory under GloballyOff with full history; \
             got {:?} for {:?}",
            row.adder_vol_source,
            row.issuer_id.as_str()
        );
        assert!(
            row.adder_vol_annualized.abs() < 1e-9,
            "singleton-bucket residuals are fully absorbed by the deepest level, \
             so the FromHistory vol must be 0.0; got {} for {:?}",
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
    let generic_values = vec![100.0, 100.5];

    let issuer = IssuerId::new("ISSUER-A");
    let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    spreads.insert(issuer.clone(), vec![Some(120.0), Some(121.0)]);
    let mut tags: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    tags.insert(issuer.clone(), tags_for("IG", "EU"));
    let mut asof_spreads = BTreeMap::new();
    asof_spreads.insert(issuer, 121.0);

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
        asof_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
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

// ---------------------------------------------------------------------------
// PR-5b Test 1: Ridge covariance adds alpha to diagonal
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-5b Test 2: FullSampleRepaired covariance is PSD
// ---------------------------------------------------------------------------

/// Fixture where n_factors > n_obs makes naive sample covariance non-PSD.
/// Verify that `CovarianceStrategy::FullSampleRepaired` produces a covariance
/// matrix with all non-negative eigenvalues.
#[test]
fn full_sample_repaired_covariance_is_psd() {
    use finstack_core::math::linalg::symmetric_eigen;

    // Build a very sparse panel: 3 dates → 2 return observations, but 6
    // factors from a 2-level hierarchy. n_factors=7 (generic+6 buckets) > n_obs=2
    // causes the sample correlation to be rank-deficient and non-PSD.
    let n = 3usize; // 3 dates → 2 returns
    let as_of = d(2024, Month::March, 31);
    let dates = monthly_dates(n, as_of);
    let generic_values: Vec<f64> = vec![0.0, 1.0, -0.5];

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
    let mut asof_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

    for (idx, (id, rating, region)) in issuer_specs.iter().enumerate() {
        let issuer_id = IssuerId::new(*id);
        // Deterministic series with slight variation to avoid exact collinearity
        let series: Vec<Option<f64>> = (0..n)
            .map(|i| {
                Some(
                    100.0
                        + (idx as f64) * 10.0
                        + generic_values[i]
                        + 0.01 * ((idx * n + i) as f64).sin(),
                )
            })
            .collect();
        asof_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
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
        asof_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
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

// ---------------------------------------------------------------------------
// PR-5b Test 3: Ridge covariance preserves determinism
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-5b Test 4: FullSampleRepaired preserves determinism
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// PR-5b Test 5: Golden artifact regression test
// ---------------------------------------------------------------------------

const REGEN_CREDIT_FACTOR_MODEL_GOLDEN_ENV: &str = "FINSTACK_REGEN_CREDIT_FACTOR_MODEL_GOLDEN";

/// Generate (or regenerate) the golden artifact file.
/// Run manually with:
/// `FINSTACK_REGEN_CREDIT_FACTOR_MODEL_GOLDEN=1 cargo test -p finstack-valuations --test credit_calibration generate_golden_artifact -- --nocapture`
#[test]
fn generate_golden_artifact() {
    if std::env::var_os(REGEN_CREDIT_FACTOR_MODEL_GOLDEN_ENV).is_none() {
        return;
    }

    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/schema_fixtures/credit_factor_model_v1.json"
    );

    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::Diagonal,
        vol_model: VolModelChoice::Sample,
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

    let model = CreditCalibrator::new(cfg)
        .calibrate(fixture_panel().into_inputs())
        .expect("golden fixture calibration must succeed");

    let json = serde_json::to_string_pretty(&model).expect("serialize to pretty JSON");
    std::fs::create_dir_all(std::path::Path::new(golden_path).parent().unwrap())
        .expect("create golden dir");
    std::fs::write(golden_path, &json).expect("write golden file");
    println!("Golden file written to {golden_path}");
}

/// Calibrate with the canonical fixture (Diagonal + Sample), serialize to
/// pretty-printed JSON, and compare byte-for-byte against the checked-in
/// schema fixture at `tests/schema_fixtures/credit_factor_model_v1.json`.
///
/// On first run after generating the golden file, this test confirms the
/// file matches a fresh calibration. On subsequent runs it catches any
/// accidental changes to the serialization or calibration math.
#[test]
fn golden_credit_factor_model_matches_checked_in_json() {
    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/schema_fixtures/credit_factor_model_v1.json"
    );

    let cfg = CreditCalibrationConfig {
        covariance_strategy: CovarianceStrategy::Diagonal,
        vol_model: VolModelChoice::Sample,
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

    let model = CreditCalibrator::new(cfg)
        .calibrate(fixture_panel().into_inputs())
        .expect("golden fixture calibration must succeed");

    let produced = serde_json::to_string_pretty(&model).expect("serialize to pretty JSON");

    // Read the checked-in golden file. If it doesn't exist yet, the test fails
    // with a clear message telling the developer how to bootstrap it by running
    // the env-gated generator test.
    let golden = std::fs::read_to_string(golden_path).unwrap_or_else(|e| {
        panic!(
            "Golden file not found at {golden_path}: {e}\n\
             Bootstrap it by running:\n  \
             {REGEN_CREDIT_FACTOR_MODEL_GOLDEN_ENV}=1 cargo test -p finstack-valuations \
             --test credit_calibration generate_golden_artifact -- --nocapture"
        )
    });

    // Parse both as serde_json::Value for stable comparison regardless of
    // trailing whitespace or insignificant formatting differences; then
    // re-serialize to canonical pretty form for a clean diff on failure.
    let produced_val: serde_json::Value =
        serde_json::from_str(&produced).expect("produced JSON is valid");
    let golden_val: serde_json::Value =
        serde_json::from_str(&golden).expect("golden JSON is valid");

    assert_eq!(
        serde_json::to_string_pretty(&produced_val).expect("re-serialize produced"),
        serde_json::to_string_pretty(&golden_val).expect("re-serialize golden"),
        "Calibration output does not match golden file at {golden_path}.\n\
         If this change is intentional, regenerate the golden file."
    );
}

// ---------------------------------------------------------------------------
// Serde round-trip tests (PR-9 Fix 1)
// ---------------------------------------------------------------------------

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
        annualization_factor: 12.0,
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
    let schema_content =
        include_str!("../schemas/factor_model/1/credit_calibration_config.schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(schema_content).expect("schema must be valid JSON");

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
        annualization_factor: 12.0,
    };

    let instance: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&cfg).expect("serialize config"))
            .expect("re-parse as Value");

    let validator = jsonschema::validator_for(&schema).expect("schema must compile");
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

// ---------------------------------------------------------------------------
// asof_spreads must cover exactly the calibrated universe
// ---------------------------------------------------------------------------

/// A history issuer missing from `asof_spreads` previously got a silent
/// `adder_at_anchor = 0.0` and shifted every bucket peer's anchor mean.
#[test]
fn calibration_rejects_asof_spreads_missing_a_history_issuer() {
    let mut inputs = fixture_panel().into_inputs();
    inputs.asof_spreads.remove(&IssuerId::new("ISSUER-B"));

    let cfg = config_with(
        IssuerBetaPolicy::GloballyOff,
        vec![HierarchyDimension::Rating, HierarchyDimension::Region],
    );
    let err = CreditCalibrator::new(cfg)
        .calibrate(inputs)
        .expect_err("missing as_of spread must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("ISSUER-B") && msg.contains("asof_spreads"),
        "error must name the missing issuer: {msg}"
    );
}

/// An asof-only issuer would silently enter anchor bucket means with unit
/// betas while receiving no artifact row.
#[test]
fn calibration_rejects_asof_only_issuer() {
    let mut inputs = fixture_panel().into_inputs();
    inputs
        .asof_spreads
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

// ---------------------------------------------------------------------------
// '.' in hierarchy tag values corrupts factor identity
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// unsorted/duplicated date grids are rejected
// ---------------------------------------------------------------------------

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
