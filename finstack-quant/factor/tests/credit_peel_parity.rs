//! Regression coverage for credit peel calibration and decomposition parity.

use std::collections::BTreeMap;

use finstack_quant_core::dates::create_date;
use finstack_quant_core::types::IssuerId;
use finstack_quant_factor::credit::calibration::{
    BetaShrinkage, BucketSizeThresholds, BucketWeighting, CovarianceStrategy,
    CreditCalibrationConfig, CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries,
    HistoryPanel, IssuerTagPanel, PanelFrequency, PanelSpace, VolModelChoice,
};
use finstack_quant_factor::credit::decomposition::decompose_levels;
use finstack_quant_factor::credit::hierarchy::{
    CreditHierarchySpec, GenericFactorSpec, HierarchyDimension, IssuerBetaPolicy, IssuerTags,
};
use time::Month;

#[test]
fn calibrated_anchor_matches_decompose_levels_for_single_level_case() {
    let issuer = IssuerId::new("ACME");
    let dates = vec![
        create_date(2024, Month::January, 31).expect("valid date"),
        create_date(2024, Month::February, 29).expect("valid date"),
        create_date(2024, Month::March, 31).expect("valid date"),
    ];
    let as_of = dates[2];
    let hierarchy = CreditHierarchySpec {
        levels: vec![HierarchyDimension::Rating],
    };
    let mut tags = BTreeMap::new();
    tags.insert(
        issuer.clone(),
        IssuerTags(BTreeMap::from([("rating".to_string(), "IG".to_string())])),
    );
    let mut spreads = BTreeMap::new();
    spreads.insert(issuer.clone(), vec![Some(0.010), Some(0.011), Some(0.012)]);
    let mut as_of_spreads = BTreeMap::new();
    as_of_spreads.insert(issuer, 0.012);
    let config = CreditCalibrationConfig {
        policy: IssuerBetaPolicy::GloballyOff,
        hierarchy,
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
            history_panel: HistoryPanel { dates, spreads },
            issuer_tags: IssuerTagPanel { tags },
            generic_factor: GenericFactorSeries {
                spec: GenericFactorSpec {
                    name: "CDX IG".to_string(),
                    series_id: "cdx.ig".to_string(),
                },
                values: vec![0.0010, 0.0011, 0.0012],
            },
            as_of,
            as_of_spreads: as_of_spreads.clone(),
            idiosyncratic_overrides: BTreeMap::new(),
            spread_durations: BTreeMap::new(),
        })
        .expect("calibration succeeds");

    let decomposed =
        decompose_levels(&model, &as_of_spreads, 0.0012, as_of, None).expect("decompose succeeds");

    assert!((decomposed.generic - model.anchor_state.pc).abs() < 1e-10);
    assert_eq!(decomposed.by_level.len(), model.anchor_state.by_level.len());
    for (decomposed_level, anchor_level) in
        decomposed.by_level.iter().zip(&model.anchor_state.by_level)
    {
        assert_eq!(decomposed_level.level_index, anchor_level.level_index);
        assert_eq!(decomposed_level.dimension, anchor_level.dimension);
        for (bucket, anchor_value) in &anchor_level.values {
            let decomposed_value = decomposed_level
                .values
                .get(bucket)
                .expect("bucket value exists");
            assert!((decomposed_value - anchor_value).abs() < 1e-10);
        }
    }
}
