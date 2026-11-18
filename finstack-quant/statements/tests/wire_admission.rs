//! Qualification of the one model loading path: limits at and above their
//! bounds, exact version and number admission, identity only after
//! validation, the runnable schema fixture, and the document set every host
//! must load to the same result or the same structured error.

use finstack_quant_core::contract::{ContractError, DEFAULT_MAX_BYTES, LoadLimits};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{Evaluator, MonteCarloConfig};
use finstack_quant_statements::types::{FinancialModelSpec, NodeType};
use serde_json::{Value, json};

fn quarter(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid quarter")
}

fn model() -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("wire");
    builder.periods("2025Q1..Q2", None).expect("periods");
    builder.value_money(
        "revenue",
        &[
            (quarter(1), Money::new(100.0, Currency::USD).expect("money")),
            (quarter(2), Money::new(110.0, Currency::USD).expect("money")),
        ],
    );
    builder.compute("profit", "revenue * 0.4").expect("formula");
    builder.build().expect("model")
}

fn document() -> Value {
    serde_json::to_value(model()).expect("model serializes")
}

/// `value` nested in `depth` further arrays.
fn nested(depth: usize) -> Value {
    (0..depth).fold(json!(0), |inner, _| json!([inner]))
}

/// The model document whose deepest JSON nesting is exactly `depth`.
fn document_of_depth(depth: usize) -> String {
    let mut document = document();
    // The model object is level 1 and `meta` level 2, so a `meta` entry
    // nested `depth - 2` arrays deep reaches `depth`.
    document["meta"] = json!({"deep": nested(depth - 2)});
    document.to_string()
}

fn limit_exceeded(error: &ContractError) -> (&'static str, usize, usize) {
    match error {
        ContractError::LimitExceeded { what, found, limit } => (what, *found, *limit),
        other => panic!("expected a limit failure, got {other:?}"),
    }
}

#[test]
fn encoded_input_is_admitted_at_64_mib_and_rejected_one_byte_above() {
    assert_eq!(DEFAULT_MAX_BYTES, 64 * 1024 * 1024);
    let mut text = document().to_string();
    text.push_str(&" ".repeat(DEFAULT_MAX_BYTES - text.len()));
    assert_eq!(text.len(), DEFAULT_MAX_BYTES);
    FinancialModelSpec::from_json(&text).expect("an input of exactly 64 MiB loads");

    text.push(' ');
    let error = FinancialModelSpec::from_json(&text).expect_err("64 MiB + 1 byte is refused");
    assert_eq!(
        limit_exceeded(&error),
        ("bytes", DEFAULT_MAX_BYTES + 1, DEFAULT_MAX_BYTES)
    );
}

#[test]
fn nesting_is_admitted_at_depth_96_and_rejected_at_97() {
    assert_eq!(LoadLimits::default().max_depth, 96);
    FinancialModelSpec::from_json(&document_of_depth(96)).expect("depth 96 loads");
    let error = FinancialModelSpec::from_json(&document_of_depth(97)).expect_err("depth 97");
    assert_eq!(limit_exceeded(&error), ("JSON depth", 97, 96));
}

#[test]
fn retained_diagnostics_stop_at_the_limit() {
    assert_eq!(LoadLimits::default().max_diagnostics, 256);
    let report = |max_diagnostics| {
        let limits = LoadLimits {
            max_diagnostics,
            ..LoadLimits::default()
        };
        match FinancialModelSpec::from_slice_strict(b"{\"id\": ", &limits) {
            Err(ContractError::Report(report)) => report,
            other => panic!("expected a report, got {other:?}"),
        }
    };
    // A load reports one decode failure: retained within the limit, and
    // dropped with the truncation flag set when the limit is below it.
    let within = report(1);
    assert_eq!(within.diagnostics.len(), 1);
    assert!(!within.truncated);
    let beyond = report(0);
    assert!(beyond.diagnostics.is_empty());
    assert!(beyond.truncated);
    assert!(beyond.has_errors());
}

#[test]
fn schema_version_is_the_exact_integer_one() {
    let with_version = |version: Option<Value>| {
        let mut document = document();
        let object = document.as_object_mut().expect("model object");
        match version {
            Some(version) => object.insert("schema_version".to_string(), version),
            None => object.remove("schema_version"),
        };
        FinancialModelSpec::from_json(&document.to_string())
    };
    with_version(Some(json!(1))).expect("the integer 1 is the supported version");
    for wrong_token in [json!("1"), json!(1.0), json!(true), json!(-1)] {
        let error = with_version(Some(wrong_token.clone())).expect_err("not the integer 1");
        let ContractError::Report(report) = &error else {
            panic!("{wrong_token}: expected a report, got {error:?}");
        };
        assert_eq!(
            report.diagnostics[0].pointer.as_deref(),
            Some("/schema_version")
        );
    }
    for (version, code) in [
        (Some(json!(2)), "contract/version-unsupported"),
        (None, "contract/version-missing"),
    ] {
        let error = with_version(version).expect_err("not version 1");
        let ContractError::Report(report) = &error else {
            panic!("expected a report, got {error:?}");
        };
        assert_eq!(report.diagnostics[0].code, code);
        assert_eq!(
            report.diagnostics[0].pointer.as_deref(),
            Some("/schema_version")
        );
    }
}

#[test]
fn duplicate_keys_unknown_fields_and_inexact_numbers_are_rejected() {
    let text = document().to_string();
    let first_code = |text: &str| match FinancialModelSpec::from_json(text) {
        Err(ContractError::Report(report)) => (
            report.diagnostics[0].code.clone(),
            report.diagnostics[0].pointer.clone(),
        ),
        other => panic!("expected a report, got {other:?}"),
    };

    // A repeated key, at the top level and inside the node map.
    let duplicate = text.replacen("{\"id\":", "{\"id\":\"shadow\",\"id\":", 1);
    assert_ne!(duplicate, text);
    assert_eq!(
        first_code(&duplicate),
        ("contract/parse-error".to_string(), Some("/id".to_string()))
    );

    // An unknown field in the closed model object.
    let mut unknown = document();
    unknown["surprise"] = json!(1);
    assert_eq!(
        first_code(&unknown.to_string()).0,
        "contract/structure-invalid"
    );

    // Money is a decimal-string amount with its currency: a JSON number is
    // not an amount.
    let money = document()["nodes"]["revenue"]["values"]["2025Q1"].clone();
    assert_eq!(money, json!({"amount": "100", "currency": "USD"}));
    let mut float_money = document();
    float_money["nodes"]["revenue"]["values"]["2025Q1"]["amount"] = json!(100.0);
    assert_eq!(
        first_code(&float_money.to_string()).0,
        "contract/structure-invalid"
    );

    // A number outside the finite double range is not a number.
    let mut overflow = document();
    overflow["meta"] = json!({"big": "@"});
    let overflow = overflow.to_string().replace("\"@\"", "1e999");
    assert_eq!(first_code(&overflow).0, "contract/parse-error");
}

/// Node order is part of a model: a loaded model keeps the document's order,
/// whatever the alphabetical order of its node identifiers.
#[test]
fn a_loaded_model_keeps_its_node_order() {
    let mut builder = ModelBuilder::new("order");
    builder.periods("2025Q1..Q1", None).expect("periods");
    for node in ["zeta", "alpha", "mid"] {
        builder.value_scalar(node, &[(quarter(1), 1.0)]);
    }
    let text = serde_json::to_string(&builder.build().expect("model")).expect("json");
    let loaded = FinancialModelSpec::from_json(&text).expect("loads");
    let order: Vec<&str> = loaded.nodes.keys().map(|id| id.as_str()).collect();
    assert_eq!(order, ["zeta", "alpha", "mid"]);
}

#[test]
fn monte_carlo_seed_is_a_canonical_decimal_string() {
    let config = MonteCarloConfig::new(10, u64::MAX, ["revenue"]);
    let wire = serde_json::to_value(&config).expect("config serializes");
    assert_eq!(wire["seed"], json!("18446744073709551615"));
    let restored: MonteCarloConfig = serde_json::from_value(wire.clone()).expect("round trip");
    assert_eq!(restored.seed, u64::MAX);

    for not_canonical in [
        json!(42),
        json!("042"),
        json!("+42"),
        json!("4.2e1"),
        json!(""),
    ] {
        let mut wire = wire.clone();
        wire["seed"] = not_canonical.clone();
        serde_json::from_value::<MonteCarloConfig>(wire)
            .expect_err(&format!("{not_canonical} is not a canonical decimal seed"));
    }
}

#[test]
fn a_model_has_an_identity_only_once_it_validates() {
    // Not runnable: a formula over a node that does not exist.
    let mut broken = model();
    broken.nodes.get_mut("profit").expect("profit").formula_text = Some("missing * 2".to_string());
    broken
        .content_hash()
        .expect_err("a model that does not validate has no identity");

    // Runnable but not yet normalized: validation infers the value node's
    // units, and the identity is that of the validated model either way.
    let validated = model();
    let mut raw = validated.clone();
    let revenue = raw.nodes.get_mut("revenue").expect("revenue");
    assert_eq!(revenue.node_type, NodeType::Value);
    assert!(revenue.value_type.take().is_some());
    let identity = validated.content_hash().expect("validated identity");
    assert_eq!(raw.content_hash().expect("raw identity"), identity);

    let loaded = FinancialModelSpec::from_json(&serde_json::to_string(&raw).expect("json"))
        .expect("the raw document loads");
    assert_eq!(loaded.content_hash().expect("loaded identity"), identity);

    // A run of the raw model records the same identity, so its results stay
    // the validated model's results.
    let run = Evaluator::new().evaluate(&raw).expect("evaluates");
    assert_eq!(
        run.provenance.expect("provenance").model_content_hash,
        identity
    );
}

#[test]
fn run_identity_is_separate_from_model_identity() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_statements::evaluator::ObservationCutoff;

    let mut model = model();
    model.source_snapshot = Some("close-2025-03-31".to_string());
    let identity = model.content_hash().expect("identity");
    let market = MarketContext::new();
    let as_of = time::macros::date!(2025 - 03 - 31);
    let run = |cutoff| {
        Evaluator::new()
            .evaluate_with_market(&model, &market, as_of, cutoff)
            .expect("evaluates")
            .provenance
            .expect("provenance")
    };
    let full = run(ObservationCutoff::FullSnapshot);
    let dated = run(ObservationCutoff::Through(as_of));
    // One model under two cutoffs: the same content identity, two runs.
    assert_eq!(full.model_content_hash, identity);
    assert_eq!(dated.model_content_hash, identity);
    assert_ne!(full.observation_cutoff, dated.observation_cutoff);
    assert_ne!(full.run_id, dated.run_id);
}

#[test]
fn the_published_model_example_is_runnable_with_a_value_and_a_formula_node() {
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/statements/1/financial_model_spec.schema.json"
    ))
    .expect("schema parses");
    let example = &schema["examples"][0];
    let model = FinancialModelSpec::from_json(&example.to_string()).expect("the example loads");
    let kinds: Vec<NodeType> = model.nodes.values().map(|node| node.node_type).collect();
    assert!(kinds.contains(&NodeType::Value), "{kinds:?}");
    assert!(kinds.contains(&NodeType::Calculated), "{kinds:?}");
    let result = Evaluator::new().evaluate(&model).expect("the example runs");
    assert_eq!(
        result.get(
            "gross_profit",
            &PeriodId::quarter(2024, 2).expect("quarter")
        ),
        Some(44.0)
    );
}

/// One host-independent view of a load: the content identity, or the error's
/// kind, code and JSON pointer.
fn outcome(document: &str) -> Value {
    match FinancialModelSpec::from_json(document) {
        Ok(model) => json!({"content_hash": model.content_hash().expect("identity")}),
        Err(error) => {
            let (kind, code, pointer) = match &error {
                ContractError::Core(core) => (core.kind().as_str(), None, None),
                ContractError::Report(report) => (
                    "validation",
                    report.diagnostics.first().map(|d| d.code.clone()),
                    report.diagnostics.first().and_then(|d| d.pointer.clone()),
                ),
                ContractError::LimitExceeded { .. } => {
                    ("validation", Some("limit_exceeded".to_string()), None)
                }
                other => panic!("unexpected load failure {other:?}"),
            };
            json!({"kind": kind, "code": code, "pointer": pointer})
        }
    }
}

/// The shared document set: Python and WASM load the same file and compare
/// the same outcomes. Regenerate with `UPDATE_WIRE_ADMISSION=1`.
#[test]
fn every_host_document_loads_to_its_recorded_outcome() {
    let text = document().to_string();
    let edited = |edit: &dyn Fn(&mut Value)| {
        let mut document = document();
        edit(&mut document);
        document.to_string()
    };
    let documents: Vec<(&str, String)> = vec![
        ("valid", text.clone()),
        (
            "duplicate_key",
            text.replacen("{\"id\":", "{\"id\":\"shadow\",\"id\":", 1),
        ),
        ("unknown_field", edited(&|d| d["surprise"] = json!(1))),
        (
            "version_as_string",
            edited(&|d| d["schema_version"] = json!("1")),
        ),
        (
            "version_as_float",
            edited(&|d| d["schema_version"] = json!(1.0)),
        ),
        (
            "missing_version",
            edited(&|d| {
                d.as_object_mut().expect("object").remove("schema_version");
            }),
        ),
        ("depth_97", document_of_depth(97)),
        (
            "numeric_money_amount",
            edited(&|d| d["nodes"]["revenue"]["values"]["2025Q1"]["amount"] = json!(100.0)),
        ),
        (
            "number_out_of_range",
            edited(&|d| d["meta"] = json!({"big": "@"})).replace("\"@\"", "1e999"),
        ),
        ("malformed_json", "{\"id\": ".to_string()),
        (
            "unknown_node_reference",
            edited(&|d| d["nodes"]["profit"]["formula_text"] = json!("missing * 2")),
        ),
        (
            "dependency_cycle",
            edited(&|d| d["nodes"]["profit"]["formula_text"] = json!("profit + revenue")),
        ),
        (
            "empty_timeline",
            edited(&|d| d["periods"]["periods"] = json!([])),
        ),
    ];
    let cases: Vec<Value> = documents
        .iter()
        .map(|(name, document)| {
            json!({"name": name, "document": document, "expect": outcome(document)})
        })
        .collect();
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&json!({"cases": cases})).expect("cases serialize")
    );

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/wire_admission.json"
    );
    if std::env::var_os("UPDATE_WIRE_ADMISSION").is_some() {
        std::fs::write(path, &rendered).expect("write the shared cases");
    }
    let recorded = std::fs::read_to_string(path).expect("shared cases exist");
    assert_eq!(
        recorded, rendered,
        "the shared load cases drifted; rerun with UPDATE_WIRE_ADMISSION=1"
    );
}
