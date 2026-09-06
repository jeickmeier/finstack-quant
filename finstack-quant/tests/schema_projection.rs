//! Whole-corpus properties of the LLM projection.
//!
//! The per-pass behaviour is unit-tested in `finstack_quant_core::schema`;
//! these are the properties that only hold once every crate's registry is
//! visible at once. Rendering and projecting the full corpus is too slow
//! for the default suite; every test here is
//! `#[ignore = "slow: covered by mise rust-test-slow"]`.

use finstack_quant_core::schema::{project_llm, LlmProfile, RESOLVES_FROM_KEYWORD};
use serde_json::Value;

fn corpus() -> (
    std::collections::BTreeMap<String, Value>,
    Vec<&'static finstack_quant_core::schema::SchemaArtifact>,
) {
    (
        finstack_quant::schema::documents_by_id().expect("corpus renders"),
        finstack_quant::schema::artifacts(),
    )
}

fn count_absolute_refs(node: &Value) -> usize {
    match node {
        Value::Object(object) => {
            let here = usize::from(
                object
                    .get("$ref")
                    .and_then(Value::as_str)
                    .is_some_and(|target| !target.starts_with('#')),
            );
            here + object.values().map(count_absolute_refs).sum::<usize>()
        }
        Value::Array(items) => items.iter().map(count_absolute_refs).sum(),
        _ => 0,
    }
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn every_artifact_projects_and_becomes_self_contained() {
    // The whole point: published schemas reference an unresolvable host, so an
    // unprojected artifact cannot be handed to anything that will not fetch.
    let (documents, artifacts) = corpus();
    let resolve = |id: &str| documents.get(id).cloned();
    let profile = LlmProfile::default();

    let mut before = 0usize;
    let mut after = 0usize;
    for artifact in &artifacts {
        let canonical = artifact.generate().expect("artifact renders");
        before += count_absolute_refs(&canonical);
        let projected = project_llm(&canonical, &resolve, &profile).expect("artifact projects");
        after += count_absolute_refs(&projected);
    }

    assert!(
        before > 1_000,
        "the corpus really does lean on absolute refs: {before}"
    );
    assert_eq!(after, 0, "projection must leave nothing to fetch");
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn projection_is_deterministic() {
    let (documents, artifacts) = corpus();
    let resolve = |id: &str| documents.get(id).cloned();
    let profile = LlmProfile::default();

    for artifact in artifacts.iter().take(12) {
        let canonical = artifact.generate().expect("renders");
        let first = project_llm(&canonical, &resolve, &profile).expect("projects");
        let second = project_llm(&canonical, &resolve, &profile).expect("projects");
        assert_eq!(
            first, second,
            "{} projected differently",
            artifact.relative_path
        );
    }
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn currency_collapses_from_a_union_of_consts_to_a_flat_enum() {
    // The single largest avoidable cost in the corpus: 159 branches, inlined
    // wherever Money appears.
    let (documents, artifacts) = corpus();
    let resolve = |id: &str| documents.get(id).cloned();

    let money = artifacts
        .iter()
        .find(|artifact| {
            artifact
                .relative_path
                .ends_with("common/1/money.schema.json")
        })
        .expect("money artifact");
    let canonical = money.generate().expect("renders");
    let projected = project_llm(&canonical, &resolve, &LlmProfile::default()).expect("projects");

    let currency = &projected["$defs"]["Currency"];
    assert!(currency.get("oneOf").is_none(), "the union is gone");
    let values = currency["enum"].as_array().expect("flat enum");
    assert!(
        values.len() > 100,
        "every ISO code survives: {}",
        values.len()
    );
    assert!(values.contains(&Value::String("USD".to_string())));

    let canonical_size = serde_json::to_vec(&canonical["$defs"]["Currency"])
        .expect("ser")
        .len();
    let projected_size = serde_json::to_vec(currency).expect("ser").len();
    assert!(
        projected_size * 4 < canonical_size,
        "expected a large reduction, got {canonical_size} -> {projected_size}"
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn an_oversized_reference_becomes_a_handle_rather_than_dominating() {
    // A portfolio bundle names the instrument union; inlining it would make a
    // small contract inherit all seventy branches.
    let (documents, artifacts) = corpus();
    let resolve = |id: &str| documents.get(id).cloned();

    let bundle = artifacts
        .iter()
        .find(|artifact| {
            artifact
                .relative_path
                .ends_with("portfolio_materialization.schema.json")
        })
        .expect("materialization artifact");
    let projected = project_llm(
        &bundle.generate().expect("renders"),
        &resolve,
        &LlmProfile::default(),
    )
    .expect("projects");

    let text = serde_json::to_string(&projected).expect("ser");
    assert!(
        text.contains(RESOLVES_FROM_KEYWORD),
        "the union is referenced by handle"
    );
    assert!(
        text.len() < 64 * 1024,
        "handle substitution keeps the bundle small: {} bytes",
        text.len()
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn projection_is_not_a_validator() {
    // It is deliberately both stricter and looser than the runtime contract, so
    // a caller that reached for it as a schema would silently mis-validate.
    let (documents, artifacts) = corpus();
    let resolve = |id: &str| documents.get(id).cloned();

    let bond = artifacts
        .iter()
        .find(|artifact| {
            artifact
                .relative_path
                .ends_with("fixed_income/bond.schema.json")
        })
        .expect("bond artifact");
    let canonical = bond.generate().expect("renders");
    let projected = project_llm(&canonical, &resolve, &LlmProfile::default()).expect("projects");

    let example = canonical["examples"][0].clone();
    let validator = jsonschema::options()
        .with_resources(
            documents
                .iter()
                .map(|(id, doc)| {
                    (
                        id.as_str(),
                        jsonschema::Resource::from_contents(doc.clone()).expect("resource"),
                    )
                })
                .collect::<Vec<_>>()
                .into_iter(),
        )
        .build(&canonical)
        .expect("canonical compiles");
    assert!(
        validator.validate(&example).is_ok(),
        "the canonical artifact is the validator"
    );

    let projected_validator = jsonschema::validator_for(&projected).expect("projected compiles");
    assert!(
        projected_validator.validate(&example).is_err(),
        "a payload the contract accepts is rejected by the projection, which is why \
         the projection must never be used to validate"
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn every_projected_artifact_is_still_a_valid_schema_document() {
    // The projection rewrites structure, so it can produce something that is
    // no longer JSON Schema at all - an array-form `items` was exactly that.
    // Compiling every projected artifact is the guard.
    let (documents, artifacts) = corpus();
    let resolve = |id: &str| documents.get(id).cloned();
    let profile = LlmProfile::default();

    let mut failures = Vec::new();
    for artifact in &artifacts {
        let projected = project_llm(&artifact.generate().expect("renders"), &resolve, &profile)
            .expect("projects");
        if let Err(error) = jsonschema::validator_for(&projected) {
            failures.push(format!("{}: {error}", artifact.relative_path));
        }
    }
    assert!(
        failures.is_empty(),
        "projected artifacts must compile:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn the_inline_budget_separates_primitives_from_pricing_bags() {
    // The default ceiling is a boundary in the corpus, not a tuning knob: every
    // shared primitive must stay inline, and the override bags - which drag the
    // whole pricing-model configuration in behind them - must not.
    let (documents, _) = corpus();
    let size = |name: &str| {
        let id = format!("https://finstack_quant.dev/schemas/common/1/{name}.schema.json");
        serde_json::to_vec(documents.get(&id).expect(name))
            .expect("ser")
            .len()
    };
    let budget = finstack_quant_core::schema::DEFAULT_MAX_INLINE_BYTES;

    for primitive in ["money", "currency", "day_count", "date", "decimal", "tenor"] {
        assert!(
            size(primitive) < budget,
            "{primitive} is {}B and must stay inline under a {budget}B budget",
            size(primitive)
        );
    }
    for bag in ["instrument_pricing_overrides", "metric_pricing_overrides"] {
        assert!(
            size(bag) > budget,
            "{bag} is {}B and must become a handle under a {budget}B budget",
            size(bag)
        );
    }
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn a_projected_instrument_keeps_its_primitives_and_defers_its_override_bags() {
    let (documents, artifacts) = corpus();
    let resolve = |id: &str| documents.get(id).cloned();

    let forward = artifacts
        .iter()
        .find(|artifact| {
            artifact
                .relative_path
                .ends_with("fx/fx_forward.schema.json")
        })
        .expect("fx_forward");
    let projected = project_llm(
        &forward.generate().expect("renders"),
        &resolve,
        &LlmProfile::default(),
    )
    .expect("projects");

    let defs = projected["$defs"].as_object().expect("definitions");
    let currency = defs.get("Currency").expect("Currency stays inline");
    assert!(
        currency["enum"].is_array(),
        "a payload author needs the codes in front of them"
    );

    let overrides = defs
        .get("InstrumentPricingOverrides")
        .expect("the override bag is still referenced");
    assert_eq!(
        overrides[RESOLVES_FROM_KEYWORD],
        serde_json::json!(
            "https://finstack_quant.dev/schemas/common/1/instrument_pricing_overrides.schema.json"
        ),
        "the bag is deferred, not carried"
    );
    assert!(
        overrides.get("properties").is_none(),
        "its contents are not inlined"
    );

    let bytes = serde_json::to_vec(&projected).expect("ser").len();
    assert!(
        bytes < 24 * 1024,
        "an FX forward should be small: {bytes} bytes"
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn every_published_artifact_carries_a_valid_example() {
    // An example is the cheapest thing a payload author can copy, and a schema
    // without one makes them guess. This is also the guard that a *new*
    // artifact cannot ship without one.
    let (documents, artifacts) = corpus();
    let resources: Vec<(&str, jsonschema::Resource)> = documents
        .iter()
        .map(|(id, doc)| {
            (
                id.as_str(),
                jsonschema::Resource::from_contents(doc.clone()).expect("resource"),
            )
        })
        .collect();

    let mut missing = Vec::new();
    let mut invalid = Vec::new();
    let mut payloads = 0usize;
    for artifact in &artifacts {
        let schema = artifact.generate().expect("renders");
        let examples = schema.get("examples").and_then(Value::as_array);
        let Some(examples) = examples.filter(|entries| !entries.is_empty()) else {
            missing.push(artifact.relative_path);
            continue;
        };
        let validator = jsonschema::options()
            .with_resources(resources.iter().map(|(id, r)| (*id, r.clone())))
            .build(&schema)
            .expect("compiles");
        for example in examples {
            payloads += 1;
            if let Err(error) = validator.validate(example) {
                invalid.push(format!("{}: {error}", artifact.relative_path));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "artifacts without an example:\n{}",
        missing.join("\n")
    );
    assert!(
        invalid.is_empty(),
        "examples that do not validate:\n{}",
        invalid.join("\n")
    );
    assert!(
        !artifacts.is_empty(),
        "the published registry must not be empty"
    );
    assert!(
        payloads >= artifacts.len(),
        "{payloads} payloads across {} artifacts",
        artifacts.len()
    );
}
