use std::collections::BTreeMap;

use finstack_quant_core::dates::Date;
use finstack_quant_core::types::IssuerId;

use super::config::{
    BetaShrinkage, BucketSizeThresholds, BucketWeighting, CovarianceStrategy,
    CreditCalibrationConfig, PanelFrequency, PanelSpace, VolModelChoice,
};
use super::inputs::HistoryPanel;
use super::statistics::{
    ewma_variance, factor_variances, ledoit_wolf_cov_and_corr, sample_correlation_flat,
};
use super::validation::validate_calibration_config;
use crate::factor::credit::hierarchy::{CreditHierarchySpec, IssuerBetaPolicy};
use crate::factor::FactorId;

/// Audit item #5: on a sparse panel the pairwise-overlap mean differs from
/// each factor's marginal mean. Demeaning the covariance with the marginal
/// mean (the previous behavior) is not a Pearson correlation; a perfectly
/// co-moving pair on the overlap window must come back as ρ = +1.
#[test]
fn sample_correlation_uses_pairwise_overlap_mean_on_sparse_panel() {
    // Factor A: observed on dates 0..4. Factor B: observed on dates 2..4.
    // On the overlap (dates 2,3) the two move identically, so the true
    // pairwise correlation is exactly +1. A's marginal mean (over 4 obs)
    // is far from its overlap mean (over 2 obs) — the bug would produce a
    // biased value, possibly even > 1 before the clamp.
    let a = FactorId::new("credit::generic");
    let b = FactorId::new("credit::bucket::A");
    let mut returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();
    returns.insert(
        a.clone(),
        vec![Some(-5.0), Some(-4.0), Some(1.0), Some(2.0)],
    );
    returns.insert(b.clone(), vec![None, None, Some(1.0), Some(2.0)]);

    let order = vec![a, b];
    let rho = sample_correlation_flat(&order, &returns);

    // 2x2 flat row-major: [aa, ab, ba, bb].
    assert_eq!(rho.len(), 4);
    assert!((rho[0] - 1.0).abs() < 1e-12, "diagonal must be 1.0");
    assert!((rho[3] - 1.0).abs() < 1e-12, "diagonal must be 1.0");
    assert!(
        (rho[1] - 1.0).abs() < 1e-9,
        "perfectly co-moving overlap must give correlation +1, got {}",
        rho[1]
    );
    assert!(
        (rho[1] - rho[2]).abs() < 1e-15,
        "correlation matrix must be symmetric"
    );
}

/// A perfectly anti-correlated overlap must give ρ = −1 regardless of the
/// (different) marginal means.
#[test]
fn sample_correlation_handles_anti_correlated_overlap() {
    let a = FactorId::new("credit::generic");
    let b = FactorId::new("credit::bucket::A");
    let mut returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();
    returns.insert(a.clone(), vec![Some(10.0), Some(1.0), Some(2.0), Some(3.0)]);
    returns.insert(b.clone(), vec![None, Some(-1.0), Some(-2.0), Some(-3.0)]);

    let order = vec![a, b];
    let rho = sample_correlation_flat(&order, &returns);
    assert!(
        (rho[1] - (-1.0)).abs() < 1e-9,
        "perfectly anti-correlated overlap must give -1, got {}",
        rho[1]
    );
}

/// Audit item #12: `factor_variances` must use the unbiased (`n − 1`)
/// sample-variance estimator, not population (`÷ n`).
#[test]
fn factor_variances_use_unbiased_n_minus_one_estimator() {
    // Series {0, 2}: mean 1, Σ(dev²) = 1 + 1 = 2.
    //   population variance = 2/2 = 1.0
    //   unbiased  variance  = 2/1 = 2.0
    let fid = FactorId::new("credit::generic");
    let mut returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();
    returns.insert(fid.clone(), vec![Some(0.0), Some(2.0)]);

    let out = factor_variances(&returns, VolModelChoice::Sample, 1.0);
    let var = *out.get(&fid).expect("variance present");
    assert!(
        (var - 2.0).abs() < 1e-12,
        "expected unbiased variance 2.0 (n-1), got {var}"
    );
}

/// RiskMetrics finite-window EWMA (Longerstaey & Spencer 1996, §5.2),
/// zero-mean convention, normalized weights w_t ∝ λ^{T−1−t}.
///
/// λ = 0.5, returns (oldest → newest) [2, 1]:
///   raw weights (1−λ)·λ^{T−1−t} = [0.25, 0.5], sum = 1 − λ² = 0.75
///   normalized  = [1/3, 2/3]
///   σ² = (1/3)·4 + (2/3)·1 = 2.0
#[test]
fn ewma_variance_matches_hand_worked_recursion() {
    let series = vec![Some(2.0), Some(1.0)];
    let var = ewma_variance(&series, 0.5, 1.0);
    assert!((var - 2.0).abs() < 1e-12, "expected 2.0, got {var}");
}

/// Sparse entries are skipped; annualization multiplies the per-period
/// variance. Same data as above with a gap and ×12 → 24.0.
#[test]
fn ewma_variance_skips_missing_observations_and_annualizes() {
    let series = vec![Some(2.0), None, Some(1.0)];
    let var = ewma_variance(&series, 0.5, 12.0);
    assert!((var - 24.0).abs() < 1e-12, "expected 24.0, got {var}");
}

/// λ = 0.5, returns [1, 1, 3]:
///   raw weights = [0.125, 0.25, 0.5], sum = 1 − 0.5³ = 0.875
///   σ² = (0.125·1 + 0.25·1 + 0.5·9)/0.875 = 4.875/0.875 = 39/7
/// Recency weighting must overweight the large final move relative to the
/// equally-weighted mean of squares (11/3).
#[test]
fn ewma_variance_weights_recent_observations() {
    let series = vec![Some(1.0), Some(1.0), Some(3.0)];
    let var = ewma_variance(&series, 0.5, 1.0);
    assert!((var - 39.0 / 7.0).abs() < 1e-12, "expected 39/7, got {var}");
}

/// Fewer than 2 valid observations → 0.0 (same fallback as
/// `factor_variances`).
#[test]
fn ewma_variance_insufficient_history_is_zero() {
    assert_eq!(ewma_variance(&[Some(5.0)], 0.94, 12.0), 0.0);
    assert_eq!(ewma_variance(&[None, None], 0.94, 12.0), 0.0);
}

/// Golden Ledoit-Wolf adapter test — same panel as the hand-worked example
/// in `core::math::linalg::ledoit_wolf_shrinkage`:
///   per-period Σ* = [[2.5, −1/12], [−1/12, 2.5]], δ* = 17/18.
/// With annualization_factor = 12 the stored covariance is
///   Σ_ann = [[30, −1], [−1, 30]],
/// and the (scale-invariant) correlation is −(1/12)/2.5 = −1/30.
#[test]
fn ledoit_wolf_adapter_matches_hand_worked_example() {
    let a = FactorId::new("credit::generic");
    let b = FactorId::new("credit::bucket::IG");
    let mut returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();
    returns.insert(
        a.clone(),
        vec![Some(1.0), Some(-1.0), Some(2.0), Some(-2.0)],
    );
    returns.insert(
        b.clone(),
        vec![Some(1.0), Some(-1.0), Some(-2.0), Some(2.0)],
    );
    let order = vec![a, b];

    let (corr, cov) = ledoit_wolf_cov_and_corr(&order, &returns, 12.0).expect("dense panel");

    // Row-major flat covariance [aa, ab, ba, bb].
    assert!((cov[0] - 30.0).abs() < 1e-12, "cov[0] = {}", cov[0]);
    assert!((cov[3] - 30.0).abs() < 1e-12, "cov[3] = {}", cov[3]);
    assert!((cov[1] - (-1.0)).abs() < 1e-12, "cov[1] = {}", cov[1]);
    assert!((cov[2] - (-1.0)).abs() < 1e-12, "cov[2] = {}", cov[2]);

    assert!((corr[0][0] - 1.0).abs() < 1e-15);
    assert!((corr[1][1] - 1.0).abs() < 1e-15);
    assert!(
        (corr[0][1] - (-1.0 / 30.0)).abs() < 1e-12,
        "rho = {}",
        corr[0][1]
    );
    assert!((corr[1][0] - (-1.0 / 30.0)).abs() < 1e-12);
}

/// Complete-case construction: with no date where both factors are
/// observed, Ledoit-Wolf must fail with a clean validation error rather
/// than silently imputing.
#[test]
fn ledoit_wolf_adapter_rejects_sparse_overlap() {
    let a = FactorId::new("credit::generic");
    let b = FactorId::new("credit::bucket::IG");
    let mut returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();
    returns.insert(a.clone(), vec![Some(1.0), Some(-1.0), None, None]);
    returns.insert(b.clone(), vec![None, None, Some(1.0), Some(-1.0)]);
    let err =
        ledoit_wolf_cov_and_corr(&[a, b], &returns, 12.0).expect_err("no complete observations");
    assert!(
        err.to_string().contains("complete observation"),
        "unexpected error: {err}"
    );
}

#[test]
fn credit_calibration_config_default_values() {
    let config = CreditCalibrationConfig::default();
    assert_eq!(config.policy, IssuerBetaPolicy::GloballyOff);
    assert!(config.hierarchy.levels.is_empty());
    assert_eq!(config.vol_model, VolModelChoice::Sample);
    // Default must retain sample factor correlation: a Diagonal default
    // silently ignores cross-factor correlation and understates the vol of
    // any correlated long book.
    assert_eq!(
        config.covariance_strategy,
        CovarianceStrategy::FullSampleRepaired
    );
    assert_eq!(config.beta_shrinkage, BetaShrinkage::None);
    assert_eq!(config.use_returns_or_levels, PanelSpace::Returns);
    assert_eq!(config.panel_frequency, PanelFrequency::Monthly);
    assert_eq!(config.bucket_weighting, BucketWeighting::Dts);
}

#[test]
fn panel_space_serde_roundtrip() {
    for variant in [PanelSpace::Returns, PanelSpace::Levels] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: PanelSpace = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, back);
    }
}

#[test]
fn panel_space_default_is_returns() {
    assert_eq!(PanelSpace::default(), PanelSpace::Returns);
}

#[test]
fn vol_model_choice_serde_roundtrip() {
    for variant in [
        VolModelChoice::Sample,
        VolModelChoice::Ewma { lambda: 0.94 },
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: VolModelChoice = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, back);
    }
    assert_eq!(
        serde_json::to_string(&VolModelChoice::Ewma { lambda: 0.94 }).unwrap(),
        r#"{"ewma":{"lambda":0.94}}"#
    );
}

#[test]
fn ewma_lambda_must_be_in_open_unit_interval() {
    for bad in [0.0, 1.0, -0.5, 1.5, f64::NAN] {
        let config = CreditCalibrationConfig {
            vol_model: VolModelChoice::Ewma { lambda: bad },
            ..CreditCalibrationConfig::default()
        };
        assert!(
            validate_calibration_config(&config).is_err(),
            "lambda = {bad} must be rejected"
        );
    }
    let good = CreditCalibrationConfig {
        vol_model: VolModelChoice::Ewma { lambda: 0.94 },
        ..CreditCalibrationConfig::default()
    };
    assert!(validate_calibration_config(&good).is_ok());
}

#[test]
fn garch_and_egarch_json_are_rejected() {
    // Removed in v0.6: an inert config surface is worse than a smaller enum.
    assert!(
        serde_json::from_str::<VolModelChoice>("\"garch\"").is_err(),
        "\"garch\" must no longer deserialize"
    );
    assert!(
        serde_json::from_str::<VolModelChoice>("\"egarch\"").is_err(),
        "\"egarch\" must no longer deserialize"
    );
}

#[test]
fn covariance_strategy_serde_roundtrip() {
    for variant in [
        CovarianceStrategy::Diagonal,
        CovarianceStrategy::Ridge { alpha: 0.05 },
        CovarianceStrategy::FullSampleRepaired,
        CovarianceStrategy::LedoitWolf,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: CovarianceStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, back);
    }
    assert_eq!(
        serde_json::to_string(&CovarianceStrategy::LedoitWolf).unwrap(),
        r#""ledoit_wolf""#
    );
}

#[test]
fn beta_shrinkage_serde_roundtrip() {
    for variant in [BetaShrinkage::None, BetaShrinkage::TowardOne { alpha: 0.1 }] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: BetaShrinkage = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, back);
    }
}

#[test]
fn credit_calibration_config_serde_roundtrip() {
    let config = CreditCalibrationConfig {
        policy: IssuerBetaPolicy::GloballyOff,
        hierarchy: CreditHierarchySpec {
            levels: vec![crate::factor::credit::hierarchy::HierarchyDimension::Rating],
        },
        min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![5] },
        vol_model: VolModelChoice::Sample,
        covariance_strategy: CovarianceStrategy::Diagonal,
        beta_shrinkage: BetaShrinkage::None,
        use_returns_or_levels: PanelSpace::Returns,
        panel_frequency: PanelFrequency::Monthly,
        bucket_weighting: BucketWeighting::Equal,
    };
    let json = serde_json::to_string(&config).unwrap();
    let back: CreditCalibrationConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.policy, back.policy);
    assert_eq!(config.vol_model, back.vol_model);
    assert_eq!(config.covariance_strategy, back.covariance_strategy);
    assert_eq!(config.beta_shrinkage, back.beta_shrinkage);
    assert_eq!(config.use_returns_or_levels, back.use_returns_or_levels);
    assert_eq!(config.panel_frequency, back.panel_frequency);
}

#[test]
fn bucket_size_thresholds_default_for_levels() {
    let thresholds = BucketSizeThresholds::default_for_levels(3);
    assert_eq!(thresholds.per_level, vec![5, 5, 5]);
    assert_eq!(thresholds.threshold_for_level(0), 5);
    assert_eq!(thresholds.threshold_for_level(2), 5);
    assert_eq!(thresholds.threshold_for_level(99), 5);
}

#[test]
fn bucket_size_thresholds_custom_values() {
    let thresholds = BucketSizeThresholds {
        per_level: vec![3, 7],
    };
    assert_eq!(thresholds.threshold_for_level(0), 3);
    assert_eq!(thresholds.threshold_for_level(1), 7);
    assert_eq!(thresholds.threshold_for_level(2), 5);
}

#[test]
fn history_panel_serde_roundtrip() {
    let mut spreads = BTreeMap::new();
    spreads.insert(IssuerId::new("A"), vec![Some(0.0100), Some(0.0101)]);
    spreads.insert(IssuerId::new("B"), vec![Some(0.0200), None]);
    let panel = HistoryPanel {
        dates: vec![
            Date::from_calendar_date(2024, time::Month::January, 31).unwrap(),
            Date::from_calendar_date(2024, time::Month::February, 29).unwrap(),
        ],
        spreads,
    };
    let json = serde_json::to_string(&panel).unwrap();
    let back: HistoryPanel = serde_json::from_str(&json).unwrap();
    assert_eq!(panel, back);
}

// End-to-end calibration fixtures for fold-up, look-ahead, and degenerate-OLS
// regression tests.

mod calibration_pipeline {
    use std::collections::BTreeMap;

    use finstack_quant_core::dates::{create_date, Date};
    use finstack_quant_core::types::IssuerId;
    use time::Month;

    use crate::factor::credit::calibration::{
        BetaShrinkage, BucketSizeThresholds, BucketWeighting, CovarianceStrategy,
        CreditCalibrationConfig, CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries,
        HistoryPanel, IssuerTagPanel, PanelFrequency, PanelSpace, VolModelChoice,
    };
    use crate::factor::credit::hierarchy::{
        CreditFactorModel, CreditHierarchySpec, GenericFactorSpec, HierarchyDimension,
        IssuerBetaOverride, IssuerBetaPolicy, IssuerTags,
    };

    fn monthly_dates(n: usize) -> Vec<Date> {
        let months = [
            Month::January,
            Month::February,
            Month::March,
            Month::April,
            Month::May,
            Month::June,
            Month::July,
            Month::August,
            Month::September,
            Month::October,
            Month::November,
            Month::December,
        ];
        (0..n)
            .map(|i| {
                create_date(2020 + i32::try_from(i / 12).unwrap(), months[i % 12], 28).unwrap()
            })
            .collect()
    }

    fn rating_sector_tags(sector: &str) -> IssuerTags {
        IssuerTags(BTreeMap::from([
            ("rating".to_string(), "IG".to_string()),
            ("sector".to_string(), sector.to_string()),
        ]))
    }

    struct PipelineCase {
        policy: IssuerBetaPolicy,
        thresholds: Vec<usize>,
        n_dates: usize,
        /// `(issuer, sector, spread series)`.
        issuers: Vec<(&'static str, &'static str, Vec<f64>)>,
        generic: Vec<f64>,
        as_of_idx: usize,
    }

    fn calibrate(case: PipelineCase) -> finstack_quant_core::Result<CreditFactorModel> {
        calibrate_with_override(case, None)
    }

    fn calibrate_with_override(
        case: PipelineCase,
        override_vol: Option<f64>,
    ) -> finstack_quant_core::Result<CreditFactorModel> {
        let dates = monthly_dates(case.n_dates);
        let mut tags = BTreeMap::new();
        let mut spreads = BTreeMap::new();
        let mut as_of_spreads = BTreeMap::new();
        for (id, sector, series) in &case.issuers {
            let issuer = IssuerId::new(*id);
            tags.insert(issuer.clone(), rating_sector_tags(sector));
            spreads.insert(issuer.clone(), series.iter().map(|v| Some(*v)).collect());
            as_of_spreads.insert(issuer, series[case.as_of_idx]);
        }
        let config = CreditCalibrationConfig {
            policy: case.policy,
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Sector],
            },
            min_bucket_size_per_level: BucketSizeThresholds {
                per_level: case.thresholds,
            },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Returns,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Equal,
        };
        CreditCalibrator::new(config).calibrate(CreditCalibrationInputs {
            history_panel: HistoryPanel {
                dates: dates.clone(),
                spreads,
            },
            issuer_tags: IssuerTagPanel { tags },
            generic_factor: GenericFactorSeries {
                spec: GenericFactorSpec {
                    name: "CDX IG".into(),
                    series_id: "cdx.ig".into(),
                },
                values: case.generic.clone(),
            },
            as_of: dates[case.as_of_idx],
            as_of_spreads,
            idiosyncratic_overrides: override_vol
                .map(|vol| BTreeMap::from([(IssuerId::new("A"), vol)]))
                .unwrap_or_default(),
            spread_durations: BTreeMap::new(),
        })
    }

    fn wavy(n: usize, base: f64, amp: f64, frequency: f64) -> Vec<f64> {
        (0..n)
            .map(|i| base + amp * ((i as f64) * frequency).sin())
            .collect()
    }

    #[test]
    fn production_credit_audit_override_uses_decimal_spread_volatility() {
        use crate::factor::credit::hierarchy::AdderVolSource;

        let series = wavy(24, 0.01, 0.001, 0.7);
        let model = calibrate_with_override(
            PipelineCase {
                policy: IssuerBetaPolicy::GloballyOff,
                thresholds: vec![1, 1],
                n_dates: 24,
                issuers: vec![("A", "TECH", series.clone())],
                generic: series,
                as_of_idx: 23,
            },
            Some(0.001),
        )
        .expect("calibration");
        let row = &model.issuer_betas[0];
        assert!(
            (row.adder_vol_annualized - 10.0).abs() < 1e-12,
            "0.001 decimal spread per sqrt(year) is 10 bp per sqrt(year), got {}",
            row.adder_vol_annualized
        );
        assert!(matches!(
            row.adder_vol_source,
            AdderVolSource::CallerSupplied
        ));
    }

    /// Fold-up must gate on the full bucket membership. Under the default
    /// `GloballyOff` policy every issuer is `BucketOnly`; two singleton sector
    /// buckets against a threshold of 5 must fold, and the folded sector
    /// factors must not survive into `config.factors`.
    #[test]
    fn fold_up_applies_to_bucket_only_issuers() {
        let n = 30;
        let model = calibrate(PipelineCase {
            policy: IssuerBetaPolicy::GloballyOff,
            thresholds: vec![5, 5],
            n_dates: n,
            issuers: vec![
                ("A", "TECH", wavy(n, 0.010, 0.0010, 1.1)),
                ("B", "ENERGY", wavy(n, 0.015, 0.0008, 2.7)),
            ],
            generic: vec![0.0; n],
            as_of_idx: n - 1,
        })
        .expect("calibration succeeds");

        assert!(
            !model.diagnostics.fold_ups.is_empty(),
            "singleton buckets below threshold must fold even for BucketOnly issuers"
        );
        let factor_ids: Vec<&str> = model.config.factors.iter().map(|f| f.id.as_str()).collect();
        assert!(
            !factor_ids.iter().any(|f| f.contains("TECH")),
            "folded singleton sector bucket must not produce a factor: {factor_ids:?}"
        );
    }

    /// `bucket_sizes_per_level` diagnostics must report the true bucket
    /// occupancy, not the IssuerBeta-only subset (which is 0 under
    /// `GloballyOff` and actively misleading).
    #[test]
    fn bucket_sizes_count_all_members() {
        let n = 30;
        let model = calibrate(PipelineCase {
            policy: IssuerBetaPolicy::GloballyOff,
            thresholds: vec![1, 1],
            n_dates: n,
            issuers: vec![
                ("A", "TECH", wavy(n, 0.010, 0.0010, 1.1)),
                ("B", "TECH", wavy(n, 0.015, 0.0008, 2.7)),
            ],
            generic: vec![0.0; n],
            as_of_idx: n - 1,
        })
        .expect("calibration succeeds");

        assert_eq!(
            model.diagnostics.bucket_sizes_per_level[0]
                .get("IG")
                .copied(),
            Some(2),
            "level-0 bucket size must count both BucketOnly members"
        );
        assert_eq!(
            model.diagnostics.bucket_sizes_per_level[1]
                .get("IG.TECH")
                .copied(),
            Some(2),
            "level-1 bucket size must count both BucketOnly members"
        );
        // Calibrated artifacts must not default to the silent Residual
        // policy: dropped credit exposure should at least surface a warning.
        assert_eq!(
            model.config.unmatched_policy,
            Some(crate::factor::UnmatchedPolicy::Warn),
            "calibrated artifacts must default to the Warn unmatched policy"
        );
    }

    /// A bucket with one IssuerBeta member and five BucketOnly members holds
    /// six real names; a threshold of 5 must NOT fold it.
    #[test]
    fn mixed_mode_bucket_above_threshold_is_not_folded() {
        let n = 30;
        let mut overrides = BTreeMap::new();
        overrides.insert(IssuerId::new("A"), IssuerBetaOverride::ForceIssuerBeta);
        for id in ["B", "C", "D", "E", "F"] {
            overrides.insert(IssuerId::new(id), IssuerBetaOverride::ForceBucketOnly);
        }
        let issuers: Vec<(&'static str, &'static str, Vec<f64>)> = [
            ("A", 1.1),
            ("B", 0.7),
            ("C", 1.9),
            ("D", 2.3),
            ("E", 0.4),
            ("F", 3.1),
        ]
        .into_iter()
        .map(|(id, frequency)| (id, "TECH", wavy(n, 0.010, 0.0010, frequency)))
        .collect();
        let model = calibrate(PipelineCase {
            policy: IssuerBetaPolicy::Dynamic {
                min_history: 5,
                overrides,
            },
            thresholds: vec![5, 5],
            n_dates: n,
            issuers,
            generic: vec![0.0; n],
            as_of_idx: n - 1,
        })
        .expect("calibration succeeds");

        assert!(
            model.diagnostics.fold_ups.is_empty(),
            "six-member bucket must clear a threshold of 5 regardless of member modes: {:?}",
            model.diagnostics.fold_ups
        );
    }

    /// `as_of` earlier than the panel end would let post-`as_of` history leak
    /// into betas, vols, and correlations (look-ahead). Must be rejected.
    #[test]
    fn calibration_rejects_as_of_before_last_panel_date() {
        let n = 30;
        let err = calibrate(PipelineCase {
            policy: IssuerBetaPolicy::GloballyOff,
            thresholds: vec![1, 1],
            n_dates: n,
            issuers: vec![
                ("A", "TECH", wavy(n, 0.010, 0.0010, 1.1)),
                ("B", "ENERGY", wavy(n, 0.015, 0.0008, 2.7)),
            ],
            generic: vec![0.0; n],
            as_of_idx: 5,
        })
        .expect_err("as_of before the panel end must be rejected as look-ahead");
        let msg = err.to_string();
        assert!(
            msg.contains("as_of") && msg.contains("last"),
            "error must explain the look-ahead constraint, got: {msg}"
        );
    }

    /// When a hierarchy level does not refine its parent (all members of a
    /// rating bucket share one sector), the level factor is identically zero
    /// up to float noise, and the OLS slope on it explodes (observed
    /// β ≈ ±2.6e13). The fit must detect the degenerate regressor and fall
    /// back to the unit-beta convention.
    #[test]
    fn degenerate_level_factor_falls_back_to_unit_beta() {
        let n = 40;
        let mut overrides = BTreeMap::new();
        overrides.insert(IssuerId::new("A"), IssuerBetaOverride::ForceIssuerBeta);
        overrides.insert(IssuerId::new("B"), IssuerBetaOverride::ForceIssuerBeta);
        let model = calibrate(PipelineCase {
            policy: IssuerBetaPolicy::Dynamic {
                min_history: 5,
                overrides,
            },
            thresholds: vec![1, 1],
            n_dates: n,
            issuers: vec![
                ("A", "TECH", wavy(n, 0.010, 0.0010, 1.1)),
                ("B", "TECH", wavy(n, 0.010, 0.0010, 2.7)),
            ],
            generic: vec![0.0; n],
            as_of_idx: n - 1,
        })
        .expect("calibration succeeds");

        for row in &model.issuer_betas {
            let beta_sector = row.betas.levels[1];
            assert!(
                (beta_sector - 1.0).abs() < 1e-12,
                "issuer {} sector beta must fall back to 1.0 on a degenerate \
                 (non-refining) level factor, got {beta_sector}",
                row.issuer_id.as_str()
            );
        }
    }
    /// A levels panel must estimate the vol of factor *moves*, not the
    /// dispersion of raw levels. A perfect linear trend (constant monthly
    /// step) has exactly zero change-vol; treating the level dispersion as a
    /// per-period variance and annualizing it produced ~93,000 bp² of
    /// phantom factor variance.
    #[test]
    fn levels_panel_estimates_vol_of_changes_not_levels() {
        let n = 30;
        let trend = |base: f64, step: f64| -> Vec<f64> {
            (0..n).map(|i| base + (i as f64) * step).collect()
        };
        let dates = monthly_dates(n);
        let mut tags = BTreeMap::new();
        let mut spreads = BTreeMap::new();
        let mut as_of_spreads = BTreeMap::new();
        for (id, series) in [("A", trend(0.010, 0.0010)), ("B", trend(0.015, 0.0010))] {
            let issuer = IssuerId::new(id);
            tags.insert(issuer.clone(), rating_sector_tags("TECH"));
            spreads.insert(issuer.clone(), series.iter().map(|v| Some(*v)).collect());
            as_of_spreads.insert(issuer, series[n - 1]);
        }
        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Sector],
            },
            min_bucket_size_per_level: BucketSizeThresholds {
                per_level: vec![1, 1],
            },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Levels,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Equal,
        };
        let model = CreditCalibrator::new(config)
            .calibrate(CreditCalibrationInputs {
                history_panel: HistoryPanel {
                    dates: dates.clone(),
                    spreads,
                },
                issuer_tags: IssuerTagPanel { tags },
                generic_factor: GenericFactorSeries {
                    spec: GenericFactorSpec {
                        name: "CDX IG".into(),
                        series_id: "cdx.ig".into(),
                    },
                    values: vec![0.0; n],
                },
                as_of: dates[n - 1],
                as_of_spreads,
                idiosyncratic_overrides: BTreeMap::new(),
                spread_durations: BTreeMap::new(),
            })
            .expect("calibration succeeds");

        for (fid, vol_model) in &model.vol_state.factors {
            let crate::factor::credit::hierarchy::FactorVolModel::Sample { variance } = vol_model
            else {
                panic!("expected Sample vol model");
            };
            assert!(
                variance.abs() < 1e-18,
                "factor {} must carry zero change-vol on a pure linear trend, got {variance}",
                fid.as_str()
            );
        }
    }

    /// EWMA on a levels panel is now valid because the calibrator estimates
    /// vols over first differences of the level series (zero-mean squared
    /// *moves*, matching the RiskMetrics convention).
    #[test]
    fn ewma_vol_model_accepted_for_levels_panel() {
        let config = CreditCalibrationConfig {
            vol_model: VolModelChoice::Ewma { lambda: 0.94 },
            use_returns_or_levels: PanelSpace::Levels,
            ..CreditCalibrationConfig::default()
        };
        assert!(
            crate::factor::credit::calibration::validation::validate_calibration_config(&config)
                .is_ok(),
            "Ewma + Levels is sound once vols are estimated over differences"
        );
    }

    /// An issuer alone in its deepest bucket has an identically-zero residual
    /// (its own residual IS the bucket mean), which says nothing about its
    /// idiosyncratic risk. The vol cascade must fall through to the bucket
    /// peer proxy instead of recording 0.0 as a `FromHistory` estimate.
    #[test]
    fn singleton_bucket_adder_vol_falls_through_to_peer_proxy() {
        use crate::factor::credit::hierarchy::AdderVolSource;

        let n = 40;
        let model = calibrate(PipelineCase {
            policy: IssuerBetaPolicy::GloballyOff,
            thresholds: vec![1, 1],
            n_dates: n,
            issuers: vec![
                ("A", "TECH", wavy(n, 0.010, 0.0010, 1.1)),
                ("B", "TECH", wavy(n, 0.012, 0.0008, 2.7)),
                ("C", "ENERGY", wavy(n, 0.009, 0.0012, 0.7)),
            ],
            generic: wavy(n, 0.005, 0.0005, 0.4),
            as_of_idx: n - 1,
        })
        .expect("calibration succeeds");

        let row_c = model
            .issuer_betas
            .iter()
            .find(|r| r.issuer_id.as_str() == "C")
            .expect("row for C");
        assert!(
            matches!(
                row_c.adder_vol_source,
                AdderVolSource::BucketPeerProxy { .. }
            ),
            "singleton issuer must receive a peer-proxied adder vol, got {:?}",
            row_c.adder_vol_source
        );
        assert!(
            row_c.adder_vol_annualized > 0.0,
            "peer-proxied adder vol must be positive, got {}",
            row_c.adder_vol_annualized
        );

        // The two-member TECH names keep genuine from-history vols.
        for id in ["A", "B"] {
            let row = model
                .issuer_betas
                .iter()
                .find(|r| r.issuer_id.as_str() == id)
                .expect("row");
            assert!(
                matches!(row.adder_vol_source, AdderVolSource::FromHistory),
                "{id} must keep its from-history vol, got {:?}",
                row.adder_vol_source
            );
        }
    }
    /// IssuerBeta rows must carry per-level fit diagnostics: a level beta
    /// with no recorded fit quality is unauditable (the PC-only `fit_quality`
    /// field was how a 1e13 level beta could ship without any signal).
    #[test]
    fn issuer_beta_rows_record_per_level_fit_quality() {
        let n = 40;
        let mut overrides = BTreeMap::new();
        overrides.insert(IssuerId::new("A"), IssuerBetaOverride::ForceIssuerBeta);
        overrides.insert(IssuerId::new("B"), IssuerBetaOverride::ForceIssuerBeta);
        let model = calibrate(PipelineCase {
            policy: IssuerBetaPolicy::Dynamic {
                min_history: 5,
                overrides,
            },
            thresholds: vec![1, 1],
            n_dates: n,
            issuers: vec![
                ("A", "TECH", wavy(n, 0.010, 0.0010, 1.1)),
                ("B", "ENERGY", wavy(n, 0.012, 0.0008, 2.7)),
            ],
            generic: wavy(n, 0.005, 0.0005, 0.4),
            as_of_idx: n - 1,
        })
        .expect("calibration succeeds");

        for row in &model.issuer_betas {
            assert_eq!(
                row.level_fit_quality.len(),
                2,
                "IssuerBeta row {} must carry one fit-quality slot per level",
                row.issuer_id.as_str()
            );
            let level0 = row.level_fit_quality[0]
                .as_ref()
                .expect("level-0 fit was run and must be recorded");
            assert!(level0.n_obs > 2);
            assert!(level0.r_squared.is_finite());
        }
    }

    /// Inclusive-mean OLS biases β toward 1 by ~1/N. Leave-one-out OLS on
    /// five identical names plus one 2× name must recover β ≈ 2 for the
    /// double-beta issuer.
    #[test]
    fn leave_one_out_ols_recovers_double_beta() {
        let n = 40;
        let dates = monthly_dates(n);
        let base = wavy(n, 0.010, 0.0010, 1.1);
        let double: Vec<f64> = base.iter().map(|v| 2.0 * v).collect();
        let mut overrides = BTreeMap::new();
        for id in ["A", "B", "C", "D", "E", "X2"] {
            overrides.insert(IssuerId::new(id), IssuerBetaOverride::ForceIssuerBeta);
        }
        let mut tags = BTreeMap::new();
        let mut spreads = BTreeMap::new();
        let mut as_of_spreads = BTreeMap::new();
        for (id, series) in [
            ("A", base.clone()),
            ("B", base.clone()),
            ("C", base.clone()),
            ("D", base.clone()),
            ("E", base),
            ("X2", double),
        ] {
            let issuer = IssuerId::new(id);
            tags.insert(
                issuer.clone(),
                IssuerTags(BTreeMap::from([("rating".to_string(), "IG".to_string())])),
            );
            spreads.insert(issuer.clone(), series.iter().map(|v| Some(*v)).collect());
            as_of_spreads.insert(issuer, series[n - 1]);
        }
        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::Dynamic {
                min_history: 5,
                overrides,
            },
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating],
            },
            min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Levels,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Equal,
        };
        let model = CreditCalibrator::new(config)
            .calibrate(CreditCalibrationInputs {
                history_panel: HistoryPanel {
                    dates: dates.clone(),
                    spreads,
                },
                issuer_tags: IssuerTagPanel { tags },
                generic_factor: GenericFactorSeries {
                    spec: GenericFactorSpec {
                        name: "CDX IG".into(),
                        series_id: "cdx.ig".into(),
                    },
                    values: vec![0.0; n],
                },
                as_of: dates[n - 1],
                as_of_spreads,
                idiosyncratic_overrides: BTreeMap::new(),
                spread_durations: BTreeMap::new(),
            })
            .expect("calibration succeeds");

        let row = model
            .issuer_betas
            .iter()
            .find(|r| r.issuer_id.as_str() == "X2")
            .expect("X2 row");
        let beta_rating = row.betas.levels[0];
        assert!(
            (beta_rating - 2.0).abs() < 0.05,
            "LOO OLS must recover β ≈ 2 for the 2× name, got {beta_rating} (pc = {})",
            row.betas.pc
        );
        // Inclusive-mean OLS on the same design is 12/7 ≈ 1.71 — closer to 1.
        assert!(
            (beta_rating - 12.0 / 7.0).abs() > 0.1,
            "LOO β {beta_rating} must be distinguishable from the inclusive-mean 12/7"
        );
    }

    #[test]
    fn calibration_rejects_bp_looking_spreads() {
        let err = calibrate(PipelineCase {
            policy: IssuerBetaPolicy::GloballyOff,
            thresholds: vec![1, 1],
            n_dates: 6,
            issuers: vec![("A", "TECH", vec![100.0; 6]), ("B", "TECH", vec![110.0; 6])],
            generic: vec![0.01; 6],
            as_of_idx: 5,
        })
        .expect_err("100.0 must be rejected as looking like basis points");
        let msg = err.to_string();
        assert!(
            msg.contains("looks like basis points") || msg.contains("decimal"),
            "error must explain the decimal convention, got: {msg}"
        );
    }

    #[test]
    fn calibration_rejects_gap_in_monthly_grid() {
        let dates = vec![
            create_date(2024, Month::January, 31).unwrap(),
            create_date(2024, Month::February, 29).unwrap(),
            create_date(2024, Month::April, 30).unwrap(),
        ];
        let issuer = IssuerId::new("A");
        let mut tags = BTreeMap::new();
        tags.insert(issuer.clone(), rating_sector_tags("TECH"));
        let mut spreads = BTreeMap::new();
        spreads.insert(issuer.clone(), vec![Some(0.01), Some(0.011), Some(0.012)]);
        let mut as_of_spreads = BTreeMap::new();
        as_of_spreads.insert(issuer, 0.012);
        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Sector],
            },
            min_bucket_size_per_level: BucketSizeThresholds {
                per_level: vec![1, 1],
            },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Returns,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Equal,
        };
        let err = CreditCalibrator::new(config)
            .calibrate(CreditCalibrationInputs {
                history_panel: HistoryPanel { dates, spreads },
                issuer_tags: IssuerTagPanel { tags },
                generic_factor: GenericFactorSeries {
                    spec: GenericFactorSpec {
                        name: "CDX IG".into(),
                        series_id: "cdx.ig".into(),
                    },
                    values: vec![0.01, 0.011, 0.012],
                },
                as_of: create_date(2024, Month::April, 30).unwrap(),
                as_of_spreads,
                idiosyncratic_overrides: BTreeMap::new(),
                spread_durations: BTreeMap::new(),
            })
            .expect_err("gap between February and April must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("regular") && msg.contains("Monthly"),
            "error must name the broken monthly grid, got: {msg}"
        );
    }

    #[test]
    fn calibration_rejects_none_in_issuer_series() {
        let dates = monthly_dates(3);
        let issuer = IssuerId::new("A");
        let mut tags = BTreeMap::new();
        tags.insert(issuer.clone(), rating_sector_tags("TECH"));
        let mut spreads = BTreeMap::new();
        spreads.insert(issuer.clone(), vec![Some(0.01), None, Some(0.012)]);
        let mut as_of_spreads = BTreeMap::new();
        as_of_spreads.insert(issuer, 0.012);
        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Sector],
            },
            min_bucket_size_per_level: BucketSizeThresholds {
                per_level: vec![1, 1],
            },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Returns,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Equal,
        };
        let err = CreditCalibrator::new(config)
            .calibrate(CreditCalibrationInputs {
                history_panel: HistoryPanel {
                    dates: dates.clone(),
                    spreads,
                },
                issuer_tags: IssuerTagPanel { tags },
                generic_factor: GenericFactorSeries {
                    spec: GenericFactorSpec {
                        name: "CDX IG".into(),
                        series_id: "cdx.ig".into(),
                    },
                    values: vec![0.01, 0.011, 0.012],
                },
                as_of: dates[2],
                as_of_spreads,
                idiosyncratic_overrides: BTreeMap::new(),
                spread_durations: BTreeMap::new(),
            })
            .expect_err("None in an issuer series must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("None") || msg.contains("missing an observation"),
            "error must explain the missing observation, got: {msg}"
        );
    }

    /// Equal spread duration and as-of spreads 0.01 vs 0.03 must weight the
    /// bucket factor 0.25 / 0.75 (DTS = SD × spread_bp).
    #[test]
    fn dts_weights_two_name_bucket_by_spread() {
        let n = 12;
        let dates = monthly_dates(n);
        let tight = vec![0.01; n];
        let wide = vec![0.03; n];
        let mut tags = BTreeMap::new();
        let mut spreads = BTreeMap::new();
        let mut as_of_spreads = BTreeMap::new();
        let mut spread_durations = BTreeMap::new();
        for (id, series) in [("TIGHT", tight), ("WIDE", wide)] {
            let issuer = IssuerId::new(id);
            tags.insert(
                issuer.clone(),
                IssuerTags(BTreeMap::from([("rating".to_string(), "IG".to_string())])),
            );
            spreads.insert(issuer.clone(), series.iter().map(|v| Some(*v)).collect());
            as_of_spreads.insert(issuer.clone(), series[n - 1]);
            spread_durations.insert(issuer, 5.0);
        }
        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating],
            },
            min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Levels,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Dts,
        };
        let model = CreditCalibrator::new(config)
            .calibrate(CreditCalibrationInputs {
                history_panel: HistoryPanel {
                    dates: dates.clone(),
                    spreads,
                },
                issuer_tags: IssuerTagPanel { tags },
                generic_factor: GenericFactorSeries {
                    spec: GenericFactorSpec {
                        name: "CDX IG".into(),
                        series_id: "cdx.ig".into(),
                    },
                    values: vec![0.0; n],
                },
                as_of: dates[n - 1],
                as_of_spreads: as_of_spreads.clone(),
                idiosyncratic_overrides: BTreeMap::new(),
                spread_durations,
            })
            .expect("DTS calibration succeeds");

        assert_eq!(model.bucket_weighting, BucketWeighting::Dts);
        for row in &model.issuer_betas {
            assert!(
                (row.spread_duration - 5.0).abs() < 1e-15,
                "persisted duration must be 5y, got {}",
                row.spread_duration
            );
        }
        let rating = &model.anchor_state.by_level[0];
        let factor = *rating.values.get("IG").expect("IG bucket factor at anchor");
        // Spreads in bp: 100 and 300. Equal SD → weights 0.25 / 0.75.
        // Factor = 0.25*100 + 0.75*300 = 250.
        assert!(
            (factor - 250.0).abs() < 1e-9,
            "DTS-weighted IG factor must be 250 bp, got {factor}"
        );

        let decomposed = crate::factor::credit::decomposition::decompose_levels(
            &model,
            &as_of_spreads,
            0.0,
            dates[n - 1],
            None,
        )
        .expect("decompose succeeds");
        let decomp_factor = *decomposed.by_level[0]
            .values
            .get("IG")
            .expect("IG bucket in decompose");
        assert!(
            (decomp_factor - 250.0).abs() < 1e-9,
            "decompose must reuse persisted SD × current spread, got {decomp_factor}"
        );
    }

    #[test]
    fn dts_weighting_rejects_missing_spread_duration() {
        let n = 6;
        let dates = monthly_dates(n);
        let issuer = IssuerId::new("A");
        let mut spreads = BTreeMap::new();
        spreads.insert(issuer.clone(), vec![Some(0.01); n]);
        let mut tags = BTreeMap::new();
        tags.insert(issuer.clone(), rating_sector_tags("TECH"));
        let mut as_of_spreads = BTreeMap::new();
        as_of_spreads.insert(issuer, 0.01);
        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Sector],
            },
            min_bucket_size_per_level: BucketSizeThresholds {
                per_level: vec![1, 1],
            },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Returns,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Dts,
        };
        let err = CreditCalibrator::new(config)
            .calibrate(CreditCalibrationInputs {
                history_panel: HistoryPanel {
                    dates: dates.clone(),
                    spreads,
                },
                issuer_tags: IssuerTagPanel { tags },
                generic_factor: GenericFactorSeries {
                    spec: GenericFactorSpec {
                        name: "CDX IG".into(),
                        series_id: "cdx.ig".into(),
                    },
                    values: vec![0.0; n],
                },
                as_of: dates[n - 1],
                as_of_spreads,
                idiosyncratic_overrides: BTreeMap::new(),
                spread_durations: BTreeMap::new(),
            })
            .expect_err("Dts without durations must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("spread_duration"),
            "error must name the missing duration, got: {msg}"
        );
    }

    /// The historical peel must weight each date by contemporaneous DTS, not
    /// the as-of cross-section. WIDE starts at 200 bp and jumps to 300 bp:
    /// the first factor history entry must use the 100/200 DTS split
    /// (`(1·100 + 2·200)/3 ≈ 166.67`), not the as-of 100/300 split
    /// (`0.25·100 + 0.75·200 = 175` — the pre-fix look-ahead value).
    #[test]
    fn dts_historical_peel_uses_contemporaneous_weights() {
        let n = 12;
        let dates = monthly_dates(n);
        let tight = vec![0.01; n];
        let wide: Vec<f64> = (0..n).map(|i| if i == 0 { 0.02 } else { 0.03 }).collect();
        let mut tags = BTreeMap::new();
        let mut spreads = BTreeMap::new();
        let mut as_of_spreads = BTreeMap::new();
        let mut spread_durations = BTreeMap::new();
        for (id, series) in [("TIGHT", tight), ("WIDE", wide)] {
            let issuer = IssuerId::new(id);
            tags.insert(
                issuer.clone(),
                IssuerTags(BTreeMap::from([("rating".to_string(), "IG".to_string())])),
            );
            spreads.insert(issuer.clone(), series.iter().map(|v| Some(*v)).collect());
            as_of_spreads.insert(issuer.clone(), series[n - 1]);
            spread_durations.insert(issuer, 5.0);
        }
        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating],
            },
            min_bucket_size_per_level: BucketSizeThresholds { per_level: vec![1] },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Levels,
            panel_frequency: PanelFrequency::Monthly,
            bucket_weighting: BucketWeighting::Dts,
        };
        let model = CreditCalibrator::new(config)
            .calibrate(CreditCalibrationInputs {
                history_panel: HistoryPanel {
                    dates: dates.clone(),
                    spreads,
                },
                issuer_tags: IssuerTagPanel { tags },
                generic_factor: GenericFactorSeries {
                    spec: GenericFactorSpec {
                        name: "CDX IG".into(),
                        series_id: "cdx.ig".into(),
                    },
                    values: vec![0.0; n],
                },
                as_of: dates[n - 1],
                as_of_spreads,
                idiosyncratic_overrides: BTreeMap::new(),
                spread_durations,
            })
            .expect("DTS calibration succeeds");

        let histories = model.factor_histories.as_ref().expect("histories present");
        let ig = histories
            .values
            .get(&crate::factor::FactorId::new("credit::level0::Rating::IG"))
            .expect("IG bucket factor history");
        // Date 0: spreads (100, 200) bp, equal SD → weights 1/3, 2/3.
        let expected_t0 = (100.0 + 2.0 * 200.0) / 3.0;
        assert!(
            (ig[0] - expected_t0).abs() < 1e-9,
            "date-0 factor must use date-0 DTS weights ({expected_t0}), got {}",
            ig[0]
        );
        // Later dates: spreads (100, 300) bp → weights 0.25 / 0.75 → 250.
        assert!(
            (ig[n - 1] - 250.0).abs() < 1e-9,
            "as-of factor must be 250 bp, got {}",
            ig[n - 1]
        );
        // The anchor uses the as-of cross-section, matching the last date.
        let anchor = *model.anchor_state.by_level[0]
            .values
            .get("IG")
            .expect("IG anchor");
        assert!(
            (anchor - 250.0).abs() < 1e-9,
            "anchor must use as-of DTS (250 bp), got {anchor}"
        );
    }

    fn weekday_dates(n: usize) -> Vec<Date> {
        use finstack_quant_core::dates::DateExt;
        // 2024-01-01 is a Monday.
        let mut dates = Vec::with_capacity(n);
        let mut date = create_date(2024, Month::January, 1).unwrap();
        while dates.len() < n {
            if !date.is_weekend() {
                dates.push(date);
            }
            date = date.next_day().unwrap();
        }
        dates
    }

    fn calibrate_daily(dates: Vec<Date>) -> finstack_quant_core::Result<CreditFactorModel> {
        let n = dates.len();
        let issuer = IssuerId::new("A");
        let mut tags = BTreeMap::new();
        tags.insert(issuer.clone(), rating_sector_tags("TECH"));
        let mut spreads = BTreeMap::new();
        let series: Vec<f64> = (0..n).map(|i| 0.01 + 0.0001 * (i as f64).sin()).collect();
        spreads.insert(issuer.clone(), series.iter().map(|v| Some(*v)).collect());
        let mut as_of_spreads = BTreeMap::new();
        as_of_spreads.insert(issuer, series[n - 1]);
        let config = CreditCalibrationConfig {
            policy: IssuerBetaPolicy::GloballyOff,
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Sector],
            },
            min_bucket_size_per_level: BucketSizeThresholds {
                per_level: vec![1, 1],
            },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            beta_shrinkage: BetaShrinkage::None,
            use_returns_or_levels: PanelSpace::Returns,
            panel_frequency: PanelFrequency::Daily,
            bucket_weighting: BucketWeighting::Equal,
        };
        CreditCalibrator::new(config).calibrate(CreditCalibrationInputs {
            history_panel: HistoryPanel {
                dates: dates.clone(),
                spreads,
            },
            issuer_tags: IssuerTagPanel { tags },
            generic_factor: GenericFactorSeries {
                spec: GenericFactorSpec {
                    name: "CDX IG".into(),
                    series_id: "cdx.ig".into(),
                },
                values: series,
            },
            as_of: dates[n - 1],
            as_of_spreads,
            idiosyncratic_overrides: BTreeMap::new(),
            spread_durations: BTreeMap::new(),
        })
    }

    /// A Daily panel is a business-day series: weekends are skipped and a
    /// mid-week holiday gap within the 7-calendar-day tolerance is accepted.
    #[test]
    fn daily_grid_accepts_business_days_with_holiday_gap() {
        let mut dates = weekday_dates(11);
        // Simulate a market holiday: drop one mid-week observation
        // (Wed 2024-01-10), leaving a Tue → Thu gap of 2 calendar days.
        dates.remove(7);
        calibrate_daily(dates).expect("business-day panel with a holiday gap calibrates");
    }

    #[test]
    fn daily_grid_rejects_weekend_dates() {
        use finstack_quant_core::dates::DateExt;
        let mut dates = weekday_dates(10);
        // Replace the last weekday with the following Saturday.
        let last = *dates.last().unwrap();
        let saturday = (0..7)
            .scan(last, |d, _| {
                *d = d.next_day().unwrap();
                Some(*d)
            })
            .find(|d| d.is_weekend())
            .unwrap();
        *dates.last_mut().unwrap() = saturday;
        let err = calibrate_daily(dates).expect_err("weekend date must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("weekend"),
            "error must name the weekend date, got: {msg}"
        );
    }

    #[test]
    fn daily_grid_rejects_gaps_beyond_holiday_tolerance() {
        let mut dates = weekday_dates(24);
        // Remove two full weeks of observations mid-panel: the surviving
        // neighbors are 15+ calendar days apart, far beyond the tolerance.
        dates.drain(6..16);
        let err = calibrate_daily(dates).expect_err("a two-week gap must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("Daily") && msg.contains("gap"),
            "error must name the oversized daily gap, got: {msg}"
        );
    }
}
