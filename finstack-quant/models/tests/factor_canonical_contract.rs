//! Canonical bytes, hash, and insertion-order invariance for factor-model envelopes.

use std::collections::BTreeMap;

use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::dates::{create_date, Date, DateExt};
use finstack_quant_core::types::IssuerId;
use finstack_quant_core::{content_hash, to_canonical_bytes};
use finstack_quant_models::factor::credit::calibration::{
    BucketSizeThresholds, BucketWeighting, CovarianceStrategy, CreditCalibrationConfig,
    CreditCalibrationInputs, CreditCalibrator, GenericFactorSeries, HistoryPanel, IssuerTagPanel,
    PanelFrequency, PanelSpace, VolModelChoice,
};
use finstack_quant_models::factor::credit::hierarchy::{
    CreditFactorModel, CreditHierarchySpec, GenericFactorSpec, HierarchyDimension,
    IssuerBetaPolicy, IssuerTags,
};
use finstack_quant_models::factor::FactorModelConfigEnvelope;
use time::Month;

fn config_bytes(overrides: &str) -> Vec<u8> {
    include_str!("data/canonical/factor_model_config.json")
        .replace(
            r#""overrides":{"credit:acme":1.0,"rates:usd":2.0}"#,
            &format!(r#""overrides":{overrides}"#),
        )
        .into_bytes()
}

#[test]
fn factor_model_envelope_has_golden_canonical_bytes_hash_and_order_invariance() {
    let first_bytes = config_bytes(r#"{"rates:usd":2.0,"credit:acme":1.0}"#);
    let second_bytes = config_bytes(r#"{"credit:acme":1.0,"rates:usd":2.0}"#);
    FactorModelConfigEnvelope::from_slice_strict(&first_bytes, &LoadLimits::default())
        .expect("first factor model is strictly valid");
    FactorModelConfigEnvelope::from_slice_strict(&second_bytes, &LoadLimits::default())
        .expect("second factor model is strictly valid");
    let first: FactorModelConfigEnvelope =
        serde_json::from_slice(&first_bytes).expect("first envelope parses");
    let second: FactorModelConfigEnvelope =
        serde_json::from_slice(&second_bytes).expect("second envelope parses");

    let canonical = to_canonical_bytes(&first).expect("factor model canonicalizes");
    assert_eq!(
        canonical,
        include_bytes!("data/canonical/factor_model_config.json")
    );
    assert_eq!(
        first.content_hash().expect("factor model hashes"),
        include_str!("data/canonical/factor_model_config.sha256").trim()
    );
    assert_eq!(
        canonical,
        to_canonical_bytes(&second).expect("reverse-order factor model canonicalizes")
    );
}

fn eom_months_before(end: Date, steps: i32) -> Date {
    let stepped = end.add_months(-steps).expect("valid date shift");
    stepped.end_of_month()
}

/// Reload until `to_canonical_bytes(from_slice_strict(bytes)) == bytes`.
///
/// Some f64 values are not a fixed point of JSON parse → Ryu shortest
/// emit. The checked-in golden must be that fixed point so the load path
/// is byte-stable.
fn stabilize_canonical_bytes(model: &CreditFactorModel) -> Vec<u8> {
    let mut bytes = to_canonical_bytes(model).expect("credit factor model canonicalizes");
    for _ in 0..8 {
        let (reloaded, _) = CreditFactorModel::from_slice_strict(&bytes, &LoadLimits::default())
            .expect("reload while stabilizing canonical bytes");
        let next = to_canonical_bytes(&reloaded).expect("reloaded model canonicalizes");
        if next == bytes {
            return bytes;
        }
        bytes = next;
    }
    panic!("credit factor model canonical bytes did not reach a JSON f64 fixed point")
}

fn build_canonical_credit_model() -> CreditFactorModel {
    let n = 24usize;
    let end = create_date(2024, Month::March, 31).expect("end date");
    let dates: Vec<Date> = (0..n)
        .map(|i| eom_months_before(end, i32::try_from(n - 1 - i).expect("fit")))
        .collect();
    let generic: Vec<f64> = (0..n)
        .map(|i| 0.0100 + 0.00005 * (i as f64).sin())
        .collect();
    let specs = [
        ("ISSUER-A", "IG", "EU"),
        ("ISSUER-B", "IG", "NA"),
        ("ISSUER-C", "IG", "APAC"),
        ("ISSUER-D", "HY", "EU"),
        ("ISSUER-E", "HY", "NA"),
        ("ISSUER-F", "HY", "APAC"),
    ];
    let mut tags = BTreeMap::new();
    let mut spreads = BTreeMap::new();
    let mut as_of_spreads = BTreeMap::new();
    let mut spread_durations = BTreeMap::new();
    for (idx, (id, rating, region)) in specs.into_iter().enumerate() {
        let issuer = IssuerId::new(id);
        let base = 0.0100 + (idx as f64) * 0.0025;
        let beta_pc = 0.7 + 0.05 * (idx as f64);
        let series: Vec<f64> = (0..n)
            .map(|i| {
                base + beta_pc * (generic[i] - 0.0100)
                    + 0.00001 * (idx as f64 + i as f64 * 0.5).cos()
            })
            .collect();
        tags.insert(
            issuer.clone(),
            IssuerTags(BTreeMap::from([
                ("rating".to_string(), rating.to_string()),
                ("region".to_string(), region.to_string()),
            ])),
        );
        spreads.insert(issuer.clone(), series.iter().map(|v| Some(*v)).collect());
        as_of_spreads.insert(issuer.clone(), series[n - 1]);
        spread_durations.insert(issuer, 5.0);
    }
    let config = CreditCalibrationConfig {
        policy: IssuerBetaPolicy::Dynamic {
            min_history: 12,
            overrides: BTreeMap::new(),
        },
        hierarchy: CreditHierarchySpec {
            levels: vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        },
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: vec![1, 1],
        },
        vol_model: VolModelChoice::Sample,
        covariance_strategy: CovarianceStrategy::Diagonal,
        beta_shrinkage: finstack_quant_models::factor::credit::calibration::BetaShrinkage::None,
        use_returns_or_levels: PanelSpace::Returns,
        panel_frequency: PanelFrequency::Monthly,
        bucket_weighting: BucketWeighting::Dts,
    };
    CreditCalibrator::new(config)
        .calibrate(CreditCalibrationInputs {
            history_panel: HistoryPanel {
                dates: dates.clone(),
                spreads,
            },
            issuer_tags: IssuerTagPanel { tags },
            generic_factor: GenericFactorSeries {
                spec: GenericFactorSpec {
                    name: "CDX IG 5Y".to_string(),
                    series_id: "cdx.ig.5y".to_string(),
                },
                values: generic,
            },
            as_of: dates[n - 1],
            as_of_spreads,
            idiosyncratic_overrides: BTreeMap::new(),
            spread_durations,
        })
        .expect("canonical credit model calibrates")
}

#[test]
fn credit_factor_model_has_exact_canonical_bytes_and_hash() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("canonical");
    let json_path = directory.join("credit_factor_model.json");
    let hash_path = directory.join("credit_factor_model.sha256");

    if std::env::var_os("FQ_UPDATE_CANONICAL_GOLDENS").is_some() {
        let model = build_canonical_credit_model();
        let canonical = stabilize_canonical_bytes(&model);
        let (reloaded, _) =
            CreditFactorModel::from_slice_strict(&canonical, &LoadLimits::default())
                .expect("reload stabilized model");
        let hash = content_hash(&reloaded).expect("credit factor model hashes");
        std::fs::create_dir_all(&directory).expect("create canonical fixture directory");
        std::fs::write(&json_path, &canonical)
            .expect("write credit factor model canonical fixture");
        std::fs::write(&hash_path, format!("{hash}\n"))
            .expect("write credit factor model hash fixture");
    }

    let source = std::fs::read(&json_path).expect("read credit factor model canonical fixture");
    let live = stabilize_canonical_bytes(&build_canonical_credit_model());
    assert_eq!(
        live, source,
        "checked-in credit factor model must match a fresh calibration"
    );
    let (model, report) = CreditFactorModel::from_slice_strict(&source, &LoadLimits::default())
        .expect("strict credit factor model");
    assert!(!report.has_errors());
    let canonical = to_canonical_bytes(&model).expect("credit factor model canonicalizes");
    let hash = content_hash(&model).expect("credit factor model hashes");

    assert_eq!(canonical, source);
    assert_eq!(
        hash,
        std::fs::read_to_string(&hash_path)
            .expect("read credit factor model hash fixture")
            .trim()
    );
}
