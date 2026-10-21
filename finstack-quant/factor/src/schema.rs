//! JSON Schema generation helpers for factor-model contracts.

use crate::credit::calibration::{CreditCalibrationConfig, CreditCalibrationInputs};
use crate::credit::decomposition::{LevelsAtDate, PeriodDecomposition};
use crate::credit::hierarchy::CreditFactorModel;
use crate::risk::{
    PositionRiskDecomposition, RiskBudgetResult, RiskDecomposition, StressAttribution,
};
use crate::FactorModelConfigEnvelope;
use crate::MarketDependency;

/// The three-issuer, six-date panel behind both credit examples.
///
/// Small enough to read end to end, and shared so the published inputs and the
/// published model are demonstrably the same calibration.
fn example_credit_inputs() -> finstack_quant_core::Result<CreditCalibrationInputs> {
    use crate::credit::calibration::{GenericFactorSeries, HistoryPanel, IssuerTagPanel};
    use crate::credit::hierarchy::{GenericFactorSpec, IssuerTags};
    use std::collections::BTreeMap;

    let months = [
        time::Month::January,
        time::Month::February,
        time::Month::March,
        time::Month::April,
        time::Month::May,
        time::Month::June,
    ];
    let dates = months
        .into_iter()
        .map(|month| {
            finstack_quant_core::dates::Date::from_calendar_date(2024, month, 1).map_err(|error| {
                finstack_quant_core::Error::Internal(format!("example date: {error}"))
            })
        })
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;

    let issuers = [
        (
            "ACME",
            "ig",
            [0.0120, 0.0125, 0.0118, 0.0130, 0.0127, 0.0122],
        ),
        (
            "BOREAS",
            "ig",
            [0.0095, 0.0099, 0.0092, 0.0104, 0.0101, 0.0097],
        ),
        (
            "CYGNUS",
            "hy",
            [0.0340, 0.0355, 0.0331, 0.0372, 0.0364, 0.0349],
        ),
    ];

    let mut spreads = BTreeMap::new();
    let mut tags = BTreeMap::new();
    let mut as_of_spreads = BTreeMap::new();
    for (issuer, rating, series) in issuers {
        let id = finstack_quant_core::types::IssuerId::from(issuer);
        spreads.insert(
            id.clone(),
            series.iter().map(|value| Some(*value)).collect(),
        );
        let mut issuer_tags = BTreeMap::new();
        issuer_tags.insert("rating".to_string(), rating.to_string());
        tags.insert(id.clone(), IssuerTags(issuer_tags));
        as_of_spreads.insert(id, series[series.len() - 1]);
    }

    Ok(CreditCalibrationInputs {
        history_panel: HistoryPanel {
            dates: dates.clone(),
            spreads,
        },
        issuer_tags: IssuerTagPanel { tags },
        generic_factor: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: "credit_pc1".to_string(),
                series_id: "CREDIT-PC1".to_string(),
            },
            values: vec![0.0100, 0.0104, 0.0098, 0.0109, 0.0106, 0.0102],
        },
        as_of: dates[dates.len() - 1],
        as_of_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
        spread_durations: BTreeMap::from([
            (finstack_quant_core::types::IssuerId::from("ACME"), 5.0),
            (finstack_quant_core::types::IssuerId::from("BOREAS"), 5.0),
            (finstack_quant_core::types::IssuerId::from("CYGNUS"), 4.0),
        ]),
    })
}

/// Canonical `CreditCalibrationInputs`: the shared panel.
fn credit_calibration_inputs_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let value = serde_json::to_value(example_credit_inputs()?).map_err(|error| {
        finstack_quant_core::Error::Internal(format!(
            "serialize credit calibration inputs example: {error}"
        ))
    })?;
    Ok(vec![value])
}

/// Canonical `CreditFactorModel`: the calibrator's own output for that panel.
///
/// Produced by running the calibration rather than hand-writing a result, so
/// the published example is exactly what a caller would get back. `CreditCalibrator`
/// is deterministic, which is what lets a computed artifact be checked in.
fn credit_factor_model_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let config = crate::credit::calibration::CreditCalibrationConfig::default();
    let model = crate::credit::calibration::CreditCalibrator::new(config)
        .calibrate(example_credit_inputs()?)?;
    Ok(vec![json_fixed_point_value(&model)?])
}

/// Serialize `value` to canonical JSON, reload, and repeat until the bytes
/// are a parse → emit fixed point so schema examples match the checked-in file.
fn json_fixed_point_value<T>(value: &T) -> finstack_quant_core::Result<serde_json::Value>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    use finstack_quant_core::canonical::to_canonical_bytes;
    use finstack_quant_core::Error;

    let mut bytes = to_canonical_bytes(value)?;
    for _ in 0..8 {
        let reloaded: T = serde_json::from_slice(&bytes)
            .map_err(|error| Error::Internal(format!("reload schema example JSON: {error}")))?;
        let next = to_canonical_bytes(&reloaded)?;
        if next == bytes {
            return serde_json::from_slice(&bytes).map_err(|error| {
                Error::Internal(format!("parse stabilized schema example: {error}"))
            });
        }
        bytes = next;
    }
    Err(Error::Internal(
        "schema example did not reach a JSON f64 fixed point".to_owned(),
    ))
}

/// A canonical one-factor model: a parallel USD rates factor.
///
/// One factor keeps the covariance matrix 1x1 and the mapping table a single
/// rule, which is enough to show how factors, their market mapping and the
/// dependency matcher fit together.
fn factor_model_config_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    use crate::{
        FactorDefinition, FactorId, FactorType, MappingRule, MarketMapping, MatchingConfig,
    };
    use finstack_quant_core::market_data::bumps::BumpUnits;

    let factor_id = FactorId::new("rates::usd::parallel");
    let factor = FactorDefinition {
        id: factor_id.clone(),
        factor_type: FactorType::Rates,
        market_mapping: MarketMapping::CurveParallel {
            curve_ids: vec!["USD-OIS".into()],
            units: BumpUnits::RateBp,
        },
        description: Some("Parallel shift of the USD OIS curve.".to_string()),
    };
    let covariance = crate::FactorCovarianceMatrix::new(vec![factor_id.clone()], vec![0.000_1])?;
    let matching = MatchingConfig::MappingTable(vec![MappingRule {
        dependency_filter: Default::default(),
        attribute_filter: Default::default(),
        factor_id,
    }]);

    let config = crate::FactorModelConfig {
        factors: vec![factor],
        covariance,
        matching,

        risk_measure: Default::default(),
        bump_config: None,
        unmatched_policy: None,
    };
    let value =
        serde_json::to_value(crate::FactorModelConfigEnvelope::new(config)).map_err(|error| {
            finstack_quant_core::Error::Internal(format!(
                "serialize factor-model config example: {error}"
            ))
        })?;
    Ok(vec![value])
}

/// Canonical `CreditCalibrationConfig`: the shipped defaults.
fn credit_calibration_config_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let value =
        serde_json::to_value(crate::credit::calibration::CreditCalibrationConfig::default())
            .map_err(|error| {
                finstack_quant_core::Error::Internal(format!(
                    "serialize credit calibration config example: {error}"
                ))
            })?;
    Ok(vec![value])
}

/// The crate's complete schema registry, sorted by artifact path.
///
/// This lives in the library, not the generator binary, so the generator, the
/// contract tests and the bindings all render from one definition. Render an
/// entry with [`finstack_quant_core::schema::SchemaArtifact::generate`];
/// `generated_schema` produces only the raw derived document.
pub const ARTIFACTS: &[finstack_quant_core::schema::SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
    CreditCalibrationConfig,
    "factor_model",
    "credit_calibration_config",
    Component,
    "Configuration for the deterministic credit factor-model calibrator."
)
    .with_summary("Estimator choice and bounds for credit factor calibration.")
    .with_examples(credit_calibration_config_examples),
    finstack_quant_core::schema_artifact!(
    CreditCalibrationInputs,
    "factor_model",
    "credit_calibration_inputs",
    Component,
    "Typed issuer histories, tags, generic factor series, anchor date, and overrides for one credit calibration run."
)
    .with_summary("Observed spread panel feeding credit factor calibration.")
    .with_examples(credit_calibration_inputs_examples),
    finstack_quant_core::schema_artifact!(
    CreditFactorModel,
    "factor_model",
    "credit_factor_model",
    Input,
    "Fully self-contained credit factor hierarchy model artifact."
)
    .with_summary("Calibrated credit factor hierarchy, loadable as pricing input.")
    .with_examples(credit_factor_model_examples),
    finstack_quant_core::schema_artifact!(
    FactorModelConfigEnvelope,
    "factor_model",
    "factor_model_config",
    Input,
    "Versioned factor-model configuration with typed factors, covariance, matching, pricing, and risk settings.",
    title = "Finstack Quant Factor Model Configuration"
)
    .with_summary("Factor definitions, matchers and covariance settings.")
    .with_examples(factor_model_config_examples),
];

/// Factor-owned contracts published in the `models` schema family.
///
/// These result and dependency types were registered with the models crate
/// before factor modelling became a crate of its own, so their artifacts stay
/// at `schemas/models/1/` and keep their `$id`s. The umbrella crate's
/// `gen_models_schemas` binary writes them together with
/// `finstack_quant_models::schema::ARTIFACTS`, the only generator that sees both.
pub const MODELS_ARTIFACTS: &[finstack_quant_core::schema::SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
        LevelsAtDate,
        "models",
        "levels_at_date",
        Component,
        "Snapshot of all hierarchy-level factor values at a single date, produced from observed issuer spreads."
    ),
    finstack_quant_core::schema_artifact!(
        MarketDependency,
        "models",
        "market_dependency",
        Component,
        "A single market dependency extracted from an instrument."
    ),
    finstack_quant_core::schema_artifact!(
        PeriodDecomposition,
        "models",
        "period_decomposition",
        Output,
        "Difference between two `LevelsAtDate` snapshots."
    ),
    finstack_quant_core::schema_artifact!(
        PositionRiskDecomposition,
        "models",
        "position_risk_decomposition",
        Output,
        "Complete position-level risk decomposition of a portfolio."
    ),
    finstack_quant_core::schema_artifact!(
        RiskBudgetResult,
        "models",
        "risk_budget_result",
        Output,
        "Result of comparing actual risk decomposition against a risk budget."
    ),
    finstack_quant_core::schema_artifact!(
        RiskDecomposition,
        "models",
        "risk_decomposition",
        Output,
        "Portfolio-level decomposition of total risk across common factors and residuals."
    ),
    finstack_quant_core::schema_artifact!(
        StressAttribution,
        "models",
        "stress_attribution",
        Output,
        "Per-position attribution of portfolio losses in tail scenarios."
    ),
];
