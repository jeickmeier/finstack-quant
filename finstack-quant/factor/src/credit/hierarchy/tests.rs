use super::*;
use crate::credit::calibration::{BucketWeighting, PanelFrequency, PanelSpace};
use crate::matching::MatchingConfig;
use crate::{
    FactorCovarianceMatrix, FactorDefinition, FactorId, FactorModelConfig, FactorType,
    MarketMapping,
};
use finstack_quant_core::dates::create_date;
use finstack_quant_core::types::IssuerId;
use std::collections::BTreeMap;
use time::Month;

fn empty_factor_model_config() -> FactorModelConfig {
    FactorModelConfig {
        factors: vec![],
        covariance: FactorCovarianceMatrix::new(vec![], vec![]).unwrap(),
        matching: MatchingConfig::MappingTable(vec![]),

        risk_measure: Default::default(),
        bump_config: None,
        unmatched_policy: None,
    }
}

fn minimal_model() -> CreditFactorModel {
    CreditFactorModel {
        schema: CreditFactorModelSchema::CURRENT,
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
        hierarchy: CreditHierarchySpec {
            levels: vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Region,
                HierarchyDimension::Sector,
            ],
        },
        panel_frequency: PanelFrequency::Monthly,
        use_returns_or_levels: PanelSpace::Returns,
        bucket_weighting: BucketWeighting::Equal,
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
        diagnostics: CreditCalibrationDiagnostics {
            mode_counts: BTreeMap::new(),
            bucket_sizes_per_level: vec![],
            fold_ups: vec![],
            r_squared_histogram: None,
            tag_taxonomy: BTreeMap::new(),
        },
    }
}

fn issuer_row(id: &str, mode: IssuerBetaMode) -> IssuerBetaRow {
    IssuerBetaRow {
        issuer_id: IssuerId::new(id),
        tags: IssuerTags(BTreeMap::new()),
        mode,
        betas: IssuerBetas {
            pc: 1.0,
            levels: vec![1.0, 1.0, 1.0],
        },
        adder_at_anchor: 0.0,
        adder_vol_annualized: 0.01,
        adder_vol_source: AdderVolSource::Default,
        fit_quality: None,
        level_fit_quality: vec![],
        spread_duration: 1.0,
    }
}

#[test]
fn from_json_validates_and_to_json_round_trips() {
    let model = minimal_model();
    let json = model.to_json().unwrap();
    let back = CreditFactorModel::from_json(&json).unwrap();
    assert_eq!(back.to_json().unwrap(), json);

    let error = CreditFactorModel::from_json("{").unwrap_err();
    assert!(error.to_string().contains("invalid CreditFactorModel JSON"));

    // Well-formed JSON that fails `validate()` is rejected too.
    let mut invalid = model;
    invalid.hierarchy.levels = vec![HierarchyDimension::Rating, HierarchyDimension::Rating];
    let error =
        CreditFactorModel::from_json(&serde_json::to_string(&invalid).unwrap()).unwrap_err();
    assert!(error.to_string().contains("duplicate hierarchy dimension"));
}

#[test]
fn credit_factor_model_round_trips_json() {
    let model = minimal_model();
    let json = serde_json::to_string(&model).unwrap();
    let back: CreditFactorModel = serde_json::from_str(&json).unwrap();
    assert_eq!(back.schema, CreditFactorModelSchema::CreditFactorModel);
    assert_eq!(back.as_of, model.as_of);
    assert_eq!(back.hierarchy.levels, model.hierarchy.levels);
    assert_eq!(back.issuer_betas.len(), 0);
    // Second serialization must be byte-identical (determinism)
    let json2 = serde_json::to_string(&back).unwrap();
    assert_eq!(json, json2);
}

#[test]
fn strict_loader_fuses_schema_deserialization_and_validation() {
    let model = minimal_model();
    let bytes = serde_json::to_vec(&model).expect("serialize model");
    let (loaded, report) = CreditFactorModel::from_slice_strict(
        &bytes,
        &finstack_quant_core::contract::LoadLimits::default(),
    )
    .expect("valid model");
    assert_eq!(loaded.schema, CreditFactorModelSchema::CreditFactorModel);
    assert!(report.diagnostics.is_empty());

    let base = serde_json::to_value(model).expect("serialize fixture");
    for schema in [
        None,
        Some("finstack_quant.credit_factor_model/0"),
        Some("finstack_quant.credit_factor_model/2"),
        Some("finstack_quant.credit_factor_model/not-a-version"),
    ] {
        let mut value = base.clone();
        match schema {
            Some(schema) => value["schema"] = serde_json::json!(schema),
            None => {
                value
                    .as_object_mut()
                    .expect("model object")
                    .remove("schema");
            }
        }
        assert!(
            CreditFactorModel::from_slice_strict(
                &serde_json::to_vec(&value).expect("serialize fixture"),
                &finstack_quant_core::contract::LoadLimits::default(),
            )
            .is_err(),
            "invalid schema must fail"
        );
    }

    let mut invalid = base;
    invalid["hierarchy"]["levels"] = serde_json::json!(["rating", "rating"]);
    assert!(
        CreditFactorModel::from_slice_strict(
            &serde_json::to_vec(&invalid).expect("serialize fixture"),
            &finstack_quant_core::contract::LoadLimits::default(),
        )
        .is_err(),
        "semantic validation must run"
    );
}

// INVARIANTS.md §8 contract: the root artifact is closed
// (`deny_unknown_fields`), so an unknown root key must FAIL to
// deserialize; `CreditCalibrationDiagnostics` is an open extension point, so
// an unknown diagnostics key must deserialize successfully. Adding a
// root key therefore requires a coordinated v1 contract change.
#[test]
fn unknown_root_key_is_rejected_but_diagnostics_extension_is_accepted() {
    let model = minimal_model();
    let json = serde_json::to_string(&model).unwrap();

    // Unknown root key: closed root must reject (forward-compat break).
    let mut root = serde_json::from_str::<serde_json::Value>(&json).unwrap();
    root.as_object_mut()
        .unwrap()
        .insert("future_root_field".to_owned(), serde_json::json!(42));
    let with_root_key = serde_json::to_string(&root).unwrap();
    assert!(
        serde_json::from_str::<CreditFactorModel>(&with_root_key).is_err(),
        "unknown root key must fail: root uses deny_unknown_fields, so \
         adding a root field is a coordinated v1 contract change"
    );

    // Unknown diagnostics key: open extension point must accept.
    let mut diag = serde_json::from_str::<serde_json::Value>(&json).unwrap();
    diag.as_object_mut()
        .unwrap()
        .get_mut("diagnostics")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("future_diagnostic".to_owned(), serde_json::json!("extra"));
    let with_diag_key = serde_json::to_string(&diag).unwrap();
    let parsed = serde_json::from_str::<CreditFactorModel>(&with_diag_key);
    assert!(
        parsed.is_ok(),
        "unknown CreditCalibrationDiagnostics key must deserialize: diagnostics \
         is a declared open extension point (no deny_unknown_fields)"
    );
}

#[test]
fn wrong_schema_marker_is_rejected_during_deserialization() {
    let mut value = serde_json::to_value(minimal_model()).expect("serialize model");
    value["schema"] = serde_json::json!("finstack_quant.credit_factor_model/999");
    serde_json::from_value::<CreditFactorModel>(value)
        .expect_err("unsupported schema marker must fail during deserialization");
}

#[test]
fn credit_factor_model_rejects_duplicate_issuers() {
    let mut model = minimal_model();
    model
        .issuer_betas
        .push(issuer_row("ISSUER-A", IssuerBetaMode::BucketOnly));
    model
        .issuer_betas
        .push(issuer_row("ISSUER-A", IssuerBetaMode::BucketOnly));
    assert!(model.validate().is_err());
}

#[test]
fn credit_hierarchy_custom_dimensions_serialize_deterministically() {
    let spec = CreditHierarchySpec {
        levels: vec![
            HierarchyDimension::Rating,
            HierarchyDimension::Custom("Currency".to_owned()),
            HierarchyDimension::Custom("AssetType".to_owned()),
        ],
    };
    let json1 = serde_json::to_string(&spec).unwrap();
    let back: CreditHierarchySpec = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string(&back).unwrap();
    assert_eq!(json1, json2);
    assert_eq!(back.levels, spec.levels);
}

#[test]
fn credit_factor_ids_are_stable_for_same_hierarchy() {
    // Two models with the same hierarchy spec and same factor IDs in config
    // should produce the same JSON for the config block.
    let make_model = || {
        let factor_id = FactorId::new("credit::generic");
        let factor_def = FactorDefinition {
            id: factor_id.clone(),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![finstack_quant_core::types::CurveId::new("CDX.IG")],
                units: finstack_quant_core::market_data::bumps::BumpUnits::RateBp,
            },
            description: None,
        };
        let covariance = FactorCovarianceMatrix::new(vec![factor_id], vec![0.0001]).unwrap();
        let config = FactorModelConfig {
            factors: vec![factor_def],
            covariance,
            matching: MatchingConfig::MappingTable(vec![]),

            risk_measure: Default::default(),
            bump_config: None,
            unmatched_policy: None,
        };
        let mut model = minimal_model();
        model.config = config;
        model
    };

    let json_a = serde_json::to_string(&make_model()).unwrap();
    let json_b = serde_json::to_string(&make_model()).unwrap();
    assert_eq!(json_a, json_b);
}

#[test]
fn empty_hierarchy_is_valid() {
    let mut model = minimal_model();
    model.hierarchy = CreditHierarchySpec { levels: vec![] };
    assert!(model.validate().is_ok());
    let json = serde_json::to_string(&model).unwrap();
    let back: CreditFactorModel = serde_json::from_str(&json).unwrap();
    assert!(back.validate().is_ok());
    assert!(back.hierarchy.levels.is_empty());
}

// Additional: duplicate hierarchy dimensions are rejected
#[test]
fn validate_rejects_duplicate_hierarchy_dimensions() {
    let mut model = minimal_model();
    model.hierarchy = CreditHierarchySpec {
        levels: vec![HierarchyDimension::Rating, HierarchyDimension::Rating],
    };
    assert!(model.validate().is_err());
}

// Additional: FactorCorrelationMatrix constructors
#[test]
fn factor_correlation_matrix_identity_roundtrips() {
    let fids = vec![FactorId::new("f1"), FactorId::new("f2")];
    let m = FactorCorrelationMatrix::identity(fids.clone());
    assert_eq!(m.data[0][0], 1.0);
    assert_eq!(m.data[0][1], 0.0);
    assert_eq!(m.data[1][0], 0.0);
    assert_eq!(m.data[1][1], 1.0);

    let json = serde_json::to_string(&m).unwrap();
    let back: FactorCorrelationMatrix = serde_json::from_str(&json).unwrap();
    assert_eq!(back.factor_ids, fids);
    assert_eq!(back.data, m.data);
}

#[test]
fn factor_correlation_matrix_rejects_non_unit_diagonal() {
    let fids = vec![FactorId::new("f1")];
    let result = FactorCorrelationMatrix::new(fids, vec![vec![0.9]]);
    assert!(result.is_err());
}

#[test]
fn factor_correlation_matrix_rejects_asymmetric() {
    let fids = vec![FactorId::new("f1"), FactorId::new("f2")];
    let result = FactorCorrelationMatrix::new(fids, vec![vec![1.0, 0.5], vec![0.6, 1.0]]);
    assert!(result.is_err());
}

// Additional: IssuerTags deterministic order
#[test]
fn issuer_tags_serialize_in_btree_order() {
    // BTreeMap guarantees alphabetical key order, so "rating" < "region" < "sector"
    let mut tags = IssuerTags(BTreeMap::new());
    tags.0.insert("sector".to_owned(), "FIN".to_owned());
    tags.0.insert("rating".to_owned(), "IG".to_owned());
    tags.0.insert("region".to_owned(), "EU".to_owned());

    let json = serde_json::to_string(&tags).unwrap();
    // Keys must appear in alphabetical order in the serialized JSON
    let rating_pos = json.find("rating").unwrap();
    let region_pos = json.find("region").unwrap();
    let sector_pos = json.find("sector").unwrap();
    assert!(rating_pos < region_pos);
    assert!(region_pos < sector_pos);
}

// Additional: FactorHistories length mismatch is rejected by validate
#[test]
fn validate_rejects_mismatched_factor_history_lengths() {
    let mut model = minimal_model();
    let mut values = BTreeMap::new();
    values.insert(FactorId::new("credit::generic"), vec![1.0, 2.0, 3.0]);
    model.factor_histories = Some(FactorHistories {
        dates: vec![
            create_date(2024, Month::January, 1).unwrap(),
            create_date(2024, Month::February, 1).unwrap(),
        ],
        values,
    });
    assert!(model.validate().is_err());
}

// Additional: Dynamic policy round-trips
#[test]
fn dynamic_policy_round_trips_json() {
    let mut overrides = BTreeMap::new();
    overrides.insert(
        IssuerId::new("ISSUER-X"),
        IssuerBetaOverride::ForceIssuerBeta,
    );
    overrides.insert(
        IssuerId::new("ISSUER-Y"),
        IssuerBetaOverride::ForceBucketOnly,
    );
    let policy = IssuerBetaPolicy::Dynamic {
        min_history: 24,
        overrides,
    };
    let json = serde_json::to_string(&policy).unwrap();
    let back: IssuerBetaPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(policy, back);
}

// Fix 1 test: FactorCorrelationMatrix rejects duplicate factor IDs
#[test]
fn factor_correlation_matrix_rejects_duplicate_factor_ids() {
    let fid_a = FactorId::new("f1");
    let result =
        FactorCorrelationMatrix::new(vec![fid_a.clone(), fid_a], vec![vec![1.0], vec![1.0]]);
    assert!(result.is_err());
}

// Fix 2 test: validate() rejects a corrupt static_correlation
#[test]
fn validate_rejects_corrupt_static_correlation() {
    let mut model = minimal_model();
    // Bypass new() by assigning directly to the public field.
    // This matrix has a non-unit diagonal — structurally invalid.
    model.static_correlation = FactorCorrelationMatrix {
        factor_ids: vec![FactorId::new("f1")],
        data: vec![vec![0.5]], // diagonal != 1.0
    };
    assert!(model.validate().is_err());
}

// CreditHierarchySpec::bucket_path — unit tests (Fix A)

fn tags_rrs(rating: &str, region: &str, sector: &str) -> IssuerTags {
    let mut m = BTreeMap::new();
    m.insert("rating".to_owned(), rating.to_owned());
    m.insert("region".to_owned(), region.to_owned());
    m.insert("sector".to_owned(), sector.to_owned());
    IssuerTags(m)
}

fn spec_rating_region_sector() -> CreditHierarchySpec {
    CreditHierarchySpec {
        levels: vec![
            HierarchyDimension::Rating,
            HierarchyDimension::Region,
            HierarchyDimension::Sector,
        ],
    }
}

#[test]
fn bucket_path_full_tags_all_levels() {
    let spec = spec_rating_region_sector();
    let tags = tags_rrs("IG", "EU", "FIN");
    // Level 0: just rating value
    assert_eq!(spec.bucket_path(&tags, 0), Some("IG".to_owned()));
    // Level 1: rating.region
    assert_eq!(spec.bucket_path(&tags, 1), Some("IG.EU".to_owned()));
    // Level 2: full path
    assert_eq!(spec.bucket_path(&tags, 2), Some("IG.EU.FIN".to_owned()));
}

#[test]
fn bucket_path_missing_tag_at_level_1_returns_none() {
    let spec = spec_rating_region_sector();
    // Tags has rating and sector, but no region.
    let mut m = BTreeMap::new();
    m.insert("rating".to_owned(), "IG".to_owned());
    m.insert("sector".to_owned(), "FIN".to_owned());
    let tags = IssuerTags(m);
    // Level 0 still works (only needs rating).
    assert_eq!(spec.bucket_path(&tags, 0), Some("IG".to_owned()));
    // Level 1 requires region — returns None.
    assert_eq!(spec.bucket_path(&tags, 1), None);
    // Level 2 also requires region — returns None.
    assert_eq!(spec.bucket_path(&tags, 2), None);
}

#[test]
fn bucket_path_custom_dimension_uses_verbatim_key() {
    let spec = CreditHierarchySpec {
        levels: vec![
            HierarchyDimension::Rating,
            HierarchyDimension::Custom("Currency".to_owned()),
        ],
    };
    let mut m = BTreeMap::new();
    m.insert("rating".to_owned(), "HY".to_owned());
    m.insert("Currency".to_owned(), "USD".to_owned()); // exact key used verbatim
    let tags = IssuerTags(m);
    assert_eq!(spec.bucket_path(&tags, 0), Some("HY".to_owned()));
    assert_eq!(spec.bucket_path(&tags, 1), Some("HY.USD".to_owned()));
}

#[test]
fn bucket_path_k_beyond_levels_returns_none() {
    let spec = spec_rating_region_sector();
    let tags = tags_rrs("IG", "EU", "FIN");
    // k == levels.len() is out of bounds.
    assert_eq!(spec.bucket_path(&tags, 3), None);
    assert_eq!(spec.bucket_path(&tags, 99), None);
}

#[test]
fn bucket_path_empty_hierarchy_returns_none() {
    let spec = CreditHierarchySpec { levels: vec![] };
    let tags = tags_rrs("IG", "EU", "FIN");
    // Any k is out of bounds for an empty hierarchy.
    assert_eq!(spec.bucket_path(&tags, 0), None);
}
#[test]
fn validate_rejects_covariance_factor_id_mismatch() {
    use crate::{FactorDefinition, FactorType, MarketMapping};
    use finstack_quant_core::market_data::bumps::BumpUnits;

    let declared = FactorId::new("credit::generic");
    let uncovered = FactorId::new("credit::level0::Rating::IG");
    let mut model = minimal_model();
    model.config.factors = [&declared, &uncovered]
        .into_iter()
        .map(|id| FactorDefinition {
            id: id.clone(),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        })
        .collect();
    // Covariance only covers one of the two declared factors; the other
    // would silently contribute zero risk at lookup time.
    model.config.covariance =
        FactorCovarianceMatrix::new(vec![declared.clone()], vec![400.0]).unwrap();
    model.static_correlation = FactorCorrelationMatrix::identity(vec![declared.clone()]);
    model
        .vol_state
        .factors
        .insert(declared, FactorVolModel::Sample { variance: 400.0 });

    let err = model
        .validate()
        .expect_err("covariance missing a declared factor must be rejected");
    assert!(
        err.to_string().contains("credit::level0::Rating::IG"),
        "error must name the uncovered factor: {err}"
    );
}

#[test]
fn validate_rejects_static_correlation_and_vol_state_mismatch() {
    use crate::{FactorDefinition, FactorType, MarketMapping};
    use finstack_quant_core::market_data::bumps::BumpUnits;

    let declared = FactorId::new("credit::generic");
    let mut model = minimal_model();
    model.config.factors = vec![FactorDefinition {
        id: declared.clone(),
        factor_type: FactorType::Credit,
        market_mapping: MarketMapping::CurveParallel {
            curve_ids: vec![],
            units: BumpUnits::RateBp,
        },
        description: None,
    }];
    model.config.covariance =
        FactorCovarianceMatrix::new(vec![declared.clone()], vec![400.0]).unwrap();

    // Correlation matrix over a disjoint id set.
    model.static_correlation = FactorCorrelationMatrix::identity(vec![FactorId::new("unrelated")]);
    model
        .vol_state
        .factors
        .insert(declared.clone(), FactorVolModel::Sample { variance: 400.0 });
    assert!(
        model.validate().is_err(),
        "static_correlation over a different factor-id set must be rejected"
    );

    // Fix the correlation; break vol_state instead.
    model.static_correlation = FactorCorrelationMatrix::identity(vec![declared.clone()]);
    model.vol_state.factors.clear();
    model.vol_state.factors.insert(
        FactorId::new("unrelated"),
        FactorVolModel::Sample { variance: 1.0 },
    );
    assert!(
        model.validate().is_err(),
        "vol_state keyed by a different factor-id set must be rejected"
    );

    // Aligned everywhere: validate() must pass.
    model.vol_state.factors.clear();
    model
        .vol_state
        .factors
        .insert(declared, FactorVolModel::Sample { variance: 400.0 });
    assert!(model.validate().is_ok(), "aligned model must validate");
}

#[test]
fn validate_rejects_builtin_and_custom_dimension_key_collision() {
    let mut model = minimal_model();
    // `Rating` and `Custom("rating")` read the SAME tag key at runtime
    // (`dimension_key` maps both to "rating"), so the two levels are the
    // same information counted twice.
    model.hierarchy = CreditHierarchySpec {
        levels: vec![
            HierarchyDimension::Rating,
            HierarchyDimension::Custom("rating".to_owned()),
        ],
    };
    let err = model
        .validate()
        .expect_err("colliding dimension keys must be rejected");
    assert!(
        err.to_string().contains("rating"),
        "error must name the colliding key: {err}"
    );
}
/// A `BucketOnly` row's betas are `1.0` by convention (`0.0` only as the
/// folded-level sentinel). A hand-edited row claiming `BucketOnly` with
/// fitted-looking betas is contradictory: consumers that branch on the
/// mode and consumers that read the betas would disagree.
#[test]
fn validate_rejects_bucket_only_row_with_non_unit_betas() {
    let mut model = minimal_model();
    let mut row = issuer_row("ACME", IssuerBetaMode::BucketOnly);
    row.betas.pc = 5.0;
    model.issuer_betas = vec![row];
    let err = model
        .validate()
        .expect_err("BucketOnly with non-unit pc beta must be rejected");
    assert!(
        err.to_string().contains("ACME"),
        "error must name the issuer: {err}"
    );

    // Folded levels (0.0) and unit betas are the legitimate shapes.
    let mut ok_row = issuer_row("ACME", IssuerBetaMode::BucketOnly);
    ok_row.betas.levels = vec![1.0, 0.0, 1.0];
    model.issuer_betas = vec![ok_row];
    assert!(model.validate().is_ok(), "unit/folded betas must validate");
}
