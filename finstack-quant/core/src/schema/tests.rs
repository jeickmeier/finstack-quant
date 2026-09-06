use super::externalize::*;
use super::generator::*;
use super::llm::*;
use super::registry::*;
use super::*;
use serde_json::json;
use serde_json::Map;

#[allow(dead_code)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
struct ExternalText(String);

#[allow(dead_code)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct SuffixProbe {
    value: String,
}

#[allow(dead_code)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct RecursiveProbe {
    value: String,
    child: Option<Box<RecursiveProbe>>,
}

#[allow(dead_code)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct RecursiveEnvelope {
    probe: RecursiveProbe,
}

#[allow(dead_code)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct RecursiveContainer {
    envelope: RecursiveEnvelope,
}

#[test]
fn packaging_externalizes_refs_and_prunes_defs() {
    let mut schema = json!({
        "properties": {
            "money": {
                "$ref": "#/$defs/Money"
            },
            "local": {
                "$ref": "#/$defs/Local"
            }
        },
        "$defs": {
            "Money": {
                "type": "string"
            },
            "Local": {
                "$ref": "#/$defs/Nested"
            },
            "Nested": {
                "type": "string"
            },
            "Unused": {
                "type": "boolean"
            }
        }
    });

    externalize_schema_definitions(
        &mut schema,
        &[ExternalSchemaDefinition::new::<ExternalText>(
            "Money",
            "https://example.test/money.schema.json",
        )],
    )
    .expect("equivalent definition externalizes");

    assert_eq!(
        schema["properties"]["money"]["$ref"],
        "https://example.test/money.schema.json"
    );
    assert!(schema["$defs"].get("Money").is_none());
    assert!(schema["$defs"].get("Unused").is_none());
    assert!(schema["$defs"].get("Local").is_some());
    assert!(schema["$defs"].get("Nested").is_some());
}

#[test]
fn packaging_decodes_nested_definition_name_and_preserves_suffix() {
    let mut schema = json!({
        "properties": {
            "nested": {
                "$ref": "#/$defs/Foo~1Bar~0Baz/properties/value"
            }
        },
        "$defs": {
            "Foo/Bar~Baz": {
                "properties": {
                    "value": {
                        "type": "string"
                    }
                },
                "required": ["value"],
                "type": "object"
            }
        }
    });

    externalize_schema_definitions(
        &mut schema,
        &[ExternalSchemaDefinition::new::<SuffixProbe>(
            "Foo/Bar~Baz",
            "https://example.test/foo.schema.json",
        )],
    )
    .expect("equivalent escaped definition externalizes");

    assert_eq!(
        schema["properties"]["nested"]["$ref"],
        "https://example.test/foo.schema.json#/properties/value"
    );
    assert!(schema.get("$defs").is_none());
}

#[test]
fn packaging_keeps_escaped_nested_local_definition_reachable() {
    let mut schema = json!({
        "properties": {
            "nested": {
                "$ref": "#/$defs/Foo~1Bar~0Baz/properties/value"
            }
        },
        "$defs": {
            "Foo/Bar~Baz": {
                "properties": {
                    "value": {
                        "type": "string"
                    }
                },
                "type": "object"
            },
            "Unused": {
                "type": "boolean"
            }
        }
    });

    externalize_schema_definitions(&mut schema, &[])
        .expect("empty external definition list is valid");

    assert!(schema["$defs"].get("Foo/Bar~Baz").is_some());
    assert!(schema["$defs"].get("Unused").is_none());
    assert_eq!(
        schema["properties"]["nested"]["$ref"],
        "#/$defs/Foo~1Bar~0Baz/properties/value"
    );
}

#[test]
fn packaging_rejects_name_only_shape_substitution() {
    let mut schema = json!({
        "$ref": "#/$defs/Money",
        "$defs": {
            "Money": { "type": "object" }
        }
    });
    let original = schema.clone();
    let error = externalize_schema_definitions(
        &mut schema,
        &[ExternalSchemaDefinition::new::<ExternalText>(
            "Money",
            "https://example.test/money.schema.json",
        )],
    )
    .expect_err("same definition name with different assertions must fail");
    assert!(error.to_string().contains("not assertion-equivalent"));
    assert_eq!(
        schema, original,
        "failed packaging must not mutate the schema"
    );
}

#[test]
fn packaging_compares_recursive_typed_definition_graphs() {
    let mut schema = serde_json::to_value(schemars::schema_for!(RecursiveContainer))
        .expect("recursive derived schema serializes");

    externalize_schema_definitions(
        &mut schema,
        &[ExternalSchemaDefinition::new::<RecursiveEnvelope>(
            "RecursiveEnvelope",
            "https://example.test/recursive_envelope.schema.json",
        )],
    )
    .expect("equivalent recursive definition externalizes");

    assert!(
        schema
            .to_string()
            .contains("https://example.test/recursive_envelope.schema.json"),
        "the recursive edge must use the typed external schema"
    );
    assert!(schema.get("$defs").is_none());
}

#[test]
fn generated_schema_preserves_derived_assertions() {
    #[allow(dead_code)]
    #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct DerivedProbe {
        value: String,
    }

    let raw = serde_json::to_value(schemars::schema_for!(DerivedProbe))
        .expect("derived schema serializes");
    let schema = generated_schema::<DerivedProbe>(
        "https://example.test/schema/",
        "probe.schema.json",
        "Derived probe",
        "Exercises assertion-preserving schema metadata.",
    )
    .expect("probe schema generates");

    assert_eq!(schema["type"], raw["type"]);
    assert_eq!(schema["properties"], raw["properties"]);
    assert_eq!(schema["required"], raw["required"]);
}

const INDEX_PROBE_ARTIFACTS: &[SchemaArtifact] = &[
    SchemaArtifact::new::<SuffixProbe>(
        "schemas/probe/1/second.schema.json",
        "https://finstack_quant.dev/schemas/probe/1/second.schema.json",
        "Second",
        "Registered second but sorts first by path.",
    )
    .with_kind(SchemaKind::Output),
    SchemaArtifact::new::<SuffixProbe>(
        "schemas/probe/1/first.schema.json",
        "https://finstack_quant.dev/schemas/probe/1/first.schema.json",
        "First",
        "Falls back to this description because no summary is set.",
    )
    .with_kind(SchemaKind::Input)
    .with_summary("An explicit one-line summary."),
];

#[test]
fn schema_index_is_sorted_by_path_and_carries_kind_and_summary() {
    let index = build_schema_index(INDEX_PROBE_ARTIFACTS).expect("index builds");
    let rows = index["artifacts"].as_array().expect("artifacts array");

    assert_eq!(index["schema_index_version"], json!(SCHEMA_INDEX_VERSION));
    // Registration order is second-then-first; the index must not depend on it.
    assert_eq!(rows[0]["path"], json!("schemas/probe/1/first.schema.json"));
    assert_eq!(rows[1]["path"], json!("schemas/probe/1/second.schema.json"));

    assert_eq!(rows[0]["kind"], json!("input"));
    assert_eq!(rows[0]["summary"], json!("An explicit one-line summary."));
    assert_eq!(rows[1]["kind"], json!("output"));
    assert_eq!(
        rows[1]["summary"],
        json!("Registered second but sorts first by path."),
        "an artifact without a summary must fall back to its description"
    );
    assert!(
        rows[0]["bytes"].as_u64().is_some_and(|bytes| bytes > 0),
        "each row reports the rendered artifact size"
    );
}

#[test]
fn single_branch_union_collapses_and_keeps_the_wrapper_annotation() {
    let mut schema = json!({
        "description": "Field-level documentation.",
        "oneOf": [{
            "type": "object",
            "description": "Branch documentation.",
            "properties": {"spec": {"type": "string"}},
            "required": ["spec"],
            "additionalProperties": false
        }]
    });
    collapse_single_branch_unions(&mut schema);

    assert!(schema.get("oneOf").is_none(), "the wrapper is removed");
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["required"], json!(["spec"]));
    assert_eq!(schema["additionalProperties"], json!(false));
    assert_eq!(
        schema["description"],
        json!("Field-level documentation."),
        "the wrapper's annotation wins over the branch's"
    );
}

#[test]
fn single_branch_union_is_kept_when_the_wrapper_asserts_anything() {
    // `type` is an assertion, so dropping the wrapper could change meaning.
    let mut schema = json!({
        "type": "object",
        "oneOf": [{"required": ["spec"]}]
    });
    let before = schema.clone();
    collapse_single_branch_unions(&mut schema);
    assert_eq!(schema, before);
}

#[test]
fn multi_branch_unions_are_untouched() {
    let mut schema = json!({"oneOf": [{"const": "a"}, {"const": "b"}]});
    let before = schema.clone();
    collapse_single_branch_unions(&mut schema);
    assert_eq!(schema, before);
}

#[test]
fn single_branch_unions_collapse_at_every_depth() {
    let mut schema = json!({
        "properties": {
            "outer": {"oneOf": [{"properties": {"inner": {"oneOf": [{"type": "integer"}]}}}]}
        }
    });
    collapse_single_branch_unions(&mut schema);
    assert_eq!(
        schema["properties"]["outer"]["properties"]["inner"]["type"],
        json!("integer")
    );
}

fn no_resolver(_: &str) -> Option<Value> {
    None
}

#[test]
fn projection_flattens_a_const_union_and_folds_short_variant_notes() {
    let schema = json!({
        "type": "object",
        "properties": {
            "stub": {"oneOf": [
                {"const": "short_front", "type": "string", "description": "Short first period."},
                {"const": "long_back", "type": "string", "description": "Long final period."}
            ]}
        }
    });
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");
    let stub = &projected["properties"]["stub"];

    assert_eq!(stub["type"], json!("string"));
    assert_eq!(stub["enum"], json!(["short_front", "long_back"]));
    assert!(stub.get("oneOf").is_none());
    let described = stub["description"].as_str().expect("folded notes");
    assert!(
        described.contains("`short_front`: Short first period."),
        "{described}"
    );
}

#[test]
fn projection_keeps_a_union_whose_branches_carry_structure() {
    // Tagged unions are the contract, not an enum; only all-`const` unions
    // may collapse.
    let schema = json!({"oneOf": [
        {"properties": {"fixed": {"type": "number"}}, "required": ["fixed"]},
        {"properties": {"floating": {"type": "number"}}, "required": ["floating"]}
    ]});
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");
    assert!(projected["oneOf"].is_array());
    assert!(projected.get("enum").is_none());
}

#[test]
fn projection_strips_rust_prose_but_keeps_domain_references() {
    let description = concat!(
        "Act/360 day count.\n\n",
        "# Examples\n```rust\nlet convention = DayCount::Act360;\n```\n\n",
        "# Standards Reference\nISDA 2006 Definitions, Section 4.16(d).\n\n",
        "See [`DayCount`] for the full set."
    );
    let schema = json!({"type": "string", "description": description});
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");
    let text = projected["description"]
        .as_str()
        .expect("description survives");

    assert!(text.contains("Act/360 day count."));
    assert!(
        text.contains("ISDA 2006 Definitions"),
        "domain grounding is kept: {text}"
    );
    assert!(!text.contains("```"), "code fences are dropped: {text}");
    assert!(
        !text.contains("let convention"),
        "Rust examples are dropped: {text}"
    );
    assert!(
        text.contains("`DayCount`") && !text.contains("[`DayCount`]"),
        "links flatten: {text}"
    );
}

#[test]
fn oversized_local_definition_becomes_a_handle_and_its_dependents_are_pruned() {
    // A container instrument embeds the whole instrument union locally, so
    // the union definition is the one over budget and everything it
    // references exists only to serve it.
    let union_branches: Vec<Value> = (0..200)
        .map(|index| json!({"$ref": format!("#/$defs/Leaf{index}")}))
        .collect();
    let mut defs = Map::new();
    defs.insert(
        "InstrumentJson".to_string(),
        json!({
            "description": "Any instrument payload.",
            "type": "object",
            "oneOf": union_branches,
        }),
    );
    for index in 0..200 {
        defs.insert(
            format!("Leaf{index}"),
            json!({"type": "object", "properties": {"id": {"type": "string"}}}),
        );
    }
    defs.insert(
        "Kept".to_string(),
        json!({"type": "string", "description": "Reachable from the root."}),
    );
    let schema = json!({
        "type": "object",
        "properties": {
            "holding": {"$ref": "#/$defs/InstrumentJson"},
            "label": {"$ref": "#/$defs/Kept"},
        },
        "$defs": Value::Object(defs),
    });

    // Pinned rather than inherited from the default so the test states the
    // budget it depends on.
    let profile = LlmProfile {
        max_inline_bytes: 2048,
        ..LlmProfile::default()
    };
    let projected = project_llm(&schema, &no_resolver, &profile).expect("projects");
    let projected_defs = projected["$defs"].as_object().expect("defs survive");

    assert!(
        projected_defs.contains_key("Kept"),
        "definitions reachable from the root survive"
    );
    assert!(
        !projected_defs.contains_key("Leaf0"),
        "definitions reachable only through the handle are pruned"
    );
    let handle = &projected_defs["InstrumentJson"];
    assert!(
        handle.get("oneOf").is_none(),
        "the oversized union is stood down to a handle"
    );
    assert!(
        handle["description"]
            .as_str()
            .is_some_and(|text| text.contains("Any instrument payload.")),
        "the handle keeps the original summary: {handle}"
    );
    assert!(
        serde_json::to_vec(&projected).expect("serializes").len()
            < serde_json::to_vec(&schema).expect("serializes").len() / 4,
        "the projection is dramatically smaller than the source"
    );
}

#[test]
fn definitions_reachable_only_through_other_definitions_are_kept() {
    let schema = json!({
        "type": "object",
        "properties": {"outer": {"$ref": "#/$defs/Outer"}},
        "$defs": {
            "Outer": {"type": "object", "properties": {"inner": {"$ref": "#/$defs/Inner"}}},
            "Inner": {"type": "string", "description": "Two hops from the root."},
            "Orphan": {"type": "string", "description": "Referenced by nothing."},
        },
    });

    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");
    let defs = projected["$defs"].as_object().expect("defs survive");

    assert!(defs.contains_key("Outer"), "direct reference survives");
    assert!(defs.contains_key("Inner"), "transitive reference survives");
    assert!(
        !defs.contains_key("Orphan"),
        "unreferenced definition is pruned"
    );
}

#[test]
fn short_descriptions_survive_the_character_budget_untouched() {
    let schema = json!({
        "type": "string",
        "description": "Annualized coupon rate as a decimal fraction, not a percentage.",
    });
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");

    assert_eq!(
        projected["description"],
        json!("Annualized coupon rate as a decimal fraction, not a percentage.")
    );
}

#[test]
fn long_description_keeps_its_leading_paragraph_and_drops_elaboration() {
    let description = concat!(
        "Notional amount the schedule accrues on.\n\n",
        "The remaining paragraphs elaborate at length on amortization interactions, ",
        "sinking-fund mechanics, and the treatment of partial prepayments, none of which ",
        "a payload author needs in order to fill in a single number for this field, and ",
        "all of which cost tokens in every projected document that references it.",
    );
    let schema = json!({"type": "string", "description": description});
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");
    let text = projected["description"]
        .as_str()
        .expect("description survives");

    assert_eq!(text, "Notional amount the schedule accrues on.");
}

#[test]
fn overlong_leading_paragraph_is_cut_on_a_sentence_boundary() {
    let sentence = "Rates are decimal fractions rather than percentages. ";
    let description = sentence.repeat(12);
    let schema = json!({"type": "string", "description": description});
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");
    let text = projected["description"]
        .as_str()
        .expect("description survives");

    assert!(
        text.chars().count() <= DEFAULT_MAX_DESCRIPTION_CHARS,
        "budget respected, got {} chars",
        text.chars().count()
    );
    assert!(
        text.ends_with("percentages."),
        "cut lands on a sentence boundary: {text}"
    );
}

#[test]
fn a_single_overlong_sentence_is_kept_whole_rather_than_cut_mid_clause() {
    let description = format!("Day-count basis {} and nothing else", "x".repeat(400));
    let schema = json!({"type": "string", "description": description});
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");

    assert_eq!(
        projected["description"].as_str().expect("survives"),
        description,
        "no sentence boundary exists, so the text is left intact"
    );
}

#[test]
fn standards_citations_survive_the_character_budget() {
    // Which ISDA section a convention implements is contract, not prose: a
    // payload author cannot confirm they picked the right day count without
    // it, so length alone must never drop it.
    let description = concat!(
        "Actual/360 day count convention.\n\n",
        "Year fraction = (actual days between dates) / 360\n\n",
        "# Standards Reference\n\n",
        "- **ISDA**: 2006 ISDA Definitions, Section 4.16(d)\n",
        "- **ISO 20022**: Day Count Fraction Code \"Actual/360\" (A004)\n\n",
        "# Usage\n\n",
        "Standard for USD money market deposits, EUR money market instruments, ",
        "short-term rate derivatives such as SOFR and \u{20ac}STR, and FX swaps and ",
        "forwards, across a range of desks that each have their own further ",
        "conventions layered on top of the basic accrual rule described above.",
    );
    let schema = json!({"type": "string", "description": description});
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");
    let text = projected["description"]
        .as_str()
        .expect("description survives");

    assert!(text.contains("Actual/360 day count convention."));
    assert!(text.contains("ISDA"), "standards body survives: {text}");
    assert!(text.contains("4.16(d)"), "section number survives: {text}");
    assert!(text.contains("A004"), "ISO code survives: {text}");
    assert!(
        !text.contains("# Usage"),
        "elaboration without a citation is dropped: {text}"
    );
}

#[test]
fn zero_budget_disables_description_shortening() {
    let description = concat!(
        "Leading summary sentence.\n\n",
        "Elaboration that a zero budget must preserve verbatim for callers that want ",
        "the full projected prose rather than the summary alone.",
    );
    let schema = json!({"type": "string", "description": description});
    let profile = LlmProfile {
        max_description_chars: 0,
        ..LlmProfile::default()
    };
    let projected = project_llm(&schema, &no_resolver, &profile).expect("projects");

    assert!(
        projected["description"]
            .as_str()
            .expect("survives")
            .contains("Elaboration that a zero budget"),
        "zero disables the cap"
    );
}

#[test]
fn projection_closes_objects_and_requires_every_property() {
    let schema = json!({
        "type": "object",
        "properties": {"id": {"type": "string"}, "note": {"type": "string"}},
        "required": ["id"]
    });
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");

    assert_eq!(projected["additionalProperties"], json!(false));
    assert_eq!(projected["required"], json!(["id", "note"]));
}

#[test]
fn projection_rewrites_tuples_and_drops_non_portable_keywords() {
    let schema = json!({
        "type": "array",
        "prefixItems": [{"type": "string"}, {"type": "number"}],
        "uniqueItems": true,
        "format": "double"
    });
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");

    assert!(projected.get("prefixItems").is_none());
    // `items` must stay a schema: draft 2020-12 has no array form.
    assert!(projected["items"].is_object(), "{}", projected["items"]);
    assert!(
        projected["items"]["anyOf"].is_array(),
        "differing positions become a union"
    );
    assert_eq!(projected["minItems"], json!(2));
    assert_eq!(projected["maxItems"], json!(2));
    assert!(projected.get("uniqueItems").is_none());
    assert!(projected.get("format").is_none());
}

#[test]
fn projection_inlines_a_resolvable_reference() {
    let schema = json!({
        "type": "object",
        "properties": {"amount": {"$ref": "https://finstack_quant.dev/schemas/common/1/decimal.schema.json"}}
    });
    let resolve = |id: &str| {
        (id == "https://finstack_quant.dev/schemas/common/1/decimal.schema.json")
            .then(|| json!({"$id": id, "type": "string", "pattern": "^-?\\d+$"}))
    };
    let projected = project_llm(&schema, &resolve, &LlmProfile::default()).expect("projects");

    assert_eq!(
        projected["properties"]["amount"]["$ref"],
        json!("#/$defs/Decimal")
    );
    assert_eq!(projected["$defs"]["Decimal"]["pattern"], json!("^-?\\d+$"));
    assert!(
        projected["$defs"]["Decimal"].get("$id").is_none(),
        "identity is not carried into a definition"
    );
}

#[test]
fn projection_leaves_an_unresolvable_reference_alone() {
    // A missing shared definition must surface as a dangling reference, not
    // as a silently different contract.
    let schema = json!({"$ref": "https://finstack_quant.dev/schemas/common/1/gone.schema.json"});
    let projected = project_llm(&schema, &no_resolver, &LlmProfile::default()).expect("projects");
    assert_eq!(
        projected["$ref"],
        json!("https://finstack_quant.dev/schemas/common/1/gone.schema.json")
    );
}

#[test]
fn projection_substitutes_a_handle_for_an_oversized_reference() {
    let big = json!({
        "$id": "https://finstack_quant.dev/schemas/instrument/1/instrument.schema.json",
        "type": "object",
        "description": "Every supported instrument.",
        "properties": {"filler": {"type": "string", "description": "x".repeat(4096)}}
    });
    let schema = json!({
        "type": "object",
        "properties": {"instrument": {"$ref": "https://finstack_quant.dev/schemas/instrument/1/instrument.schema.json"}}
    });
    let resolve = |id: &str| (id.ends_with("instrument.schema.json")).then(|| big.clone());
    let profile = LlmProfile {
        max_inline_bytes: 512,
        ..LlmProfile::default()
    };
    let projected = project_llm(&schema, &resolve, &profile).expect("projects");

    let handle = &projected["$defs"]["Instrument"];
    assert_eq!(
        handle[RESOLVES_FROM_KEYWORD],
        json!("https://finstack_quant.dev/schemas/instrument/1/instrument.schema.json")
    );
    assert_eq!(
        handle["type"],
        json!("object"),
        "a handle keeps the target's own type"
    );
    assert!(
        handle.get("properties").is_none(),
        "the target's internals are not carried"
    );
}

#[test]
fn projection_rejects_a_non_object_document() {
    assert!(project_llm(&json!([1, 2]), &no_resolver, &LlmProfile::default()).is_err());
}

#[test]
fn schema_index_defaults_to_component() {
    const COMPONENT: &[SchemaArtifact] = &[SchemaArtifact::new::<SuffixProbe>(
        "schemas/probe/1/component.schema.json",
        "https://finstack_quant.dev/schemas/probe/1/component.schema.json",
        "Component",
        "Referenced by roots, never submitted alone.",
    )];
    let index = build_schema_index(COMPONENT).expect("index builds");
    assert_eq!(index["artifacts"][0]["kind"], json!("component"));
}

#[test]
fn registry_selection_and_index_use_the_published_contract() {
    const REGISTRY: &[SchemaArtifact] = &[SchemaArtifact::new::<SuffixProbe>(
        "schemas/probe/1/component.schema.json",
        "https://finstack_quant.dev/schemas/probe/1/component.schema.json",
        "Component",
        "A component schema.",
    )];
    let artifact = &REGISTRY[0];
    for selector in [artifact.id, artifact.relative_path, "component.schema.json"] {
        assert_eq!(
            super::find_schema_artifact(REGISTRY, selector)
                .expect("match")
                .id,
            artifact.id
        );
    }
    for selector in ["", "json", ".schema.json", "ponent.schema.json"] {
        assert!(
            super::find_schema_artifact(REGISTRY, selector).is_err(),
            "{selector:?}"
        );
    }
    let schema = artifact.generate().expect("schema");
    let index = build_schema_index(REGISTRY).expect("index");
    assert_eq!(
        index["artifacts"][0]["bytes"],
        json!(super::deterministic_json_bytes(&schema)
            .expect("bytes")
            .len())
    );
}
