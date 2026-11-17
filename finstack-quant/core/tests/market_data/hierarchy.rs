//! Tests for `MarketDataHierarchy`, builder, strict validation, and preorder resolution.

use finstack_quant_core::market_data::bumps::MarketDataTarget;
use finstack_quant_core::market_data::hierarchy::{
    HierarchyTarget, MarketDataHierarchy, ResolvedTargetMatch, TagFilter, TagPredicate,
};
use finstack_quant_core::types::CurveId;

fn resolved_matches(h: &MarketDataHierarchy, target: &HierarchyTarget) -> Vec<ResolvedTargetMatch> {
    h.resolve_matches(target)
        .expect("target resolution should succeed")
}

fn resolved_ids(h: &MarketDataHierarchy, target: &HierarchyTarget) -> Vec<CurveId> {
    resolved_matches(h, target)
        .into_iter()
        .map(|matched| matched.target.id().clone())
        .collect()
}

fn root_names(h: &MarketDataHierarchy) -> Vec<String> {
    serde_json::to_value(h).unwrap()["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap().to_string())
        .collect()
}

fn all_targets(h: &MarketDataHierarchy) -> Vec<MarketDataTarget> {
    root_names(h)
        .into_iter()
        .flat_map(|root| {
            let target = HierarchyTarget::unfiltered(vec![root]).unwrap();
            resolved_matches(h, &target).into_iter().map(|m| m.target)
        })
        .collect()
}

#[test]
fn empty_hierarchy_has_no_roots() {
    let h = MarketDataHierarchy::default();
    assert!(root_names(&h).is_empty());
    assert!(h.roots().is_empty());
}

#[test]
fn hierarchy_node_built_from_a_path_starts_empty() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD")
        .build()
        .expect("valid hierarchy");
    let node = h
        .get_node(&["Rates".to_string(), "USD".to_string()])
        .expect("USD node");
    assert!(node.targets().is_empty());
    assert!(node.children().is_empty());
    assert!(node.tags().is_empty());
}

#[test]
fn builder_creates_hierarchy_with_slash_paths_and_typed_targets() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .add_node("Rates/USD/Forward/SOFR")
        .targets([
            MarketDataTarget::ForwardCurve(CurveId::from("USD-SOFR-3M")),
            MarketDataTarget::ForwardCurve(CurveId::from("USD-SOFR-6M")),
        ])
        .add_node("Credit/US/IG/Financials")
        .tag("sector", "Financials")
        .tag("rating", "A")
        .targets([
            MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")),
            MarketDataTarget::HazardCurve(CurveId::from("GS-5Y")),
        ])
        .build()
        .unwrap();

    let roots = root_names(&h);
    assert_eq!(roots, vec!["Rates".to_string(), "Credit".to_string()]);

    let path: Vec<String> = vec!["Rates".into(), "USD".into(), "OIS".into()];
    let node = h.get_node(&path).unwrap();
    assert_eq!(
        node.targets(),
        &[MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS"))]
    );

    let credit_path: Vec<String> = vec![
        "Credit".into(),
        "US".into(),
        "IG".into(),
        "Financials".into(),
    ];
    let credit_node = h.get_node(&credit_path).unwrap();
    assert_eq!(credit_node.tags().get("sector").unwrap(), "Financials");
    assert_eq!(credit_node.tags().get("rating").unwrap(), "A");
}

#[test]
fn builder_rejects_duplicate_exact_targets() {
    let result = MarketDataHierarchy::builder()
        .add_node("Rates/USD")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .add_node("Credit/US")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .build();

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("USD-OIS"),
        "Error should mention the duplicate target: {err}"
    );
}

#[test]
fn same_raw_id_allowed_across_distinct_target_namespaces() {
    let h = MarketDataHierarchy::builder()
        .add_node("Equity/SPX")
        .targets([
            MarketDataTarget::PriceScalar(CurveId::from("SPX")),
            MarketDataTarget::VolSurface(CurveId::from("SPX")),
        ])
        .build()
        .expect("distinct namespaces with same raw ID should coexist");

    let matches = resolved_matches(
        &h,
        &HierarchyTarget::unfiltered(vec!["Equity".into()]).unwrap(),
    );
    assert_eq!(matches.len(), 2);
    assert_eq!(
        matches[0].target,
        MarketDataTarget::PriceScalar(CurveId::from("SPX"))
    );
    assert_eq!(
        matches[1].target,
        MarketDataTarget::VolSurface(CurveId::from("SPX"))
    );
}

#[test]
fn builder_rejects_invalid_paths_and_is_sticky() {
    for path in ["", "Rates/", "Rates//USD", "   ", "Rates/   "] {
        let build_result = MarketDataHierarchy::builder()
            .add_node(path)
            .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
            .build();
        assert!(
            build_result.is_err(),
            "builder should reject invalid path {path:?}"
        );
    }

    // Subsequent valid add_node calls must not clear earlier sticky errors.
    let sticky = MarketDataHierarchy::builder()
        .add_node("Rates//USD")
        .add_node("Rates/USD")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .build();
    assert!(sticky.is_err());

    // Setting tags or targets without an active node must error rather than no-op.
    assert!(
        MarketDataHierarchy::builder()
            .tag("desk", "rates")
            .build()
            .is_err()
    );
    assert!(
        MarketDataHierarchy::builder()
            .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
            .build()
            .is_err()
    );
    assert!(
        MarketDataHierarchy::builder()
            .add_node("Rates/USD")
            .tag("   ", "rates")
            .build()
            .is_err()
    );
}

#[test]
fn builder_preserves_non_blank_whitespace_in_segments() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/ US ")
        .target(MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")))
        .build()
        .expect("segment with surrounding spaces must be preserved verbatim");

    assert!(
        h.get_node(&["Credit".into(), "US".into()]).is_none(),
        "segments must not be trimmed"
    );
    let node = h
        .get_node(&["Credit".into(), " US ".into()])
        .expect("exact untrimmed segment lookup");
    assert_eq!(node.targets().len(), 1);
}

#[test]
fn root_resolution_collects_entire_tree() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .add_node("Rates/EUR/ESTR")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("EUR-ESTR")))
        .add_node("Credit/US/IG")
        .target(MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")))
        .build()
        .unwrap();

    assert_eq!(all_targets(&h).len(), 3);
}

#[test]
fn numeric_root_and_child_names_preserve_authored_order() {
    let h = MarketDataHierarchy::builder()
        .add_node("2/10")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("C-2-10")))
        .add_node("2/1")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("C-2-1")))
        .add_node("1")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("C-1")))
        .build()
        .unwrap();

    let json = h.to_json().unwrap();
    let restored = MarketDataHierarchy::from_json(&json).unwrap();
    assert_eq!(root_names(&restored), vec!["2", "1"]);

    let children_of_2: Vec<&str> = restored
        .get_node(&["2".into()])
        .unwrap()
        .children()
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(children_of_2, vec!["10", "1"]);
}

#[test]
fn serde_round_trip() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .add_node("Credit/US/IG/Financials")
        .tag("sector", "Financials")
        .targets([
            MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")),
            MarketDataTarget::HazardCurve(CurveId::from("GS-5Y")),
        ])
        .build()
        .unwrap();

    let json = h.to_json().unwrap();
    let deserialized = MarketDataHierarchy::from_json(&json).unwrap();

    assert_eq!(root_names(&deserialized), root_names(&h));
    assert_eq!(all_targets(&deserialized), all_targets(&h));

    let path: Vec<String> = vec![
        "Credit".into(),
        "US".into(),
        "IG".into(),
        "Financials".into(),
    ];
    let node = deserialized.get_node(&path).unwrap();
    assert_eq!(node.tags().get("sector").unwrap(), "Financials");
    assert_eq!(node.targets().len(), 2);
}

#[test]
fn json_loader_rejects_duplicate_siblings_and_duplicate_keys() {
    let dup_roots = r#"{
        "roots": [
            {"name": "Rates", "node": {}},
            {"name": "Rates", "node": {}}
        ]
    }"#;
    assert!(MarketDataHierarchy::from_json(dup_roots).is_err());

    let dup_children = r#"{
        "roots": [{
            "name": "Rates",
            "node": {
                "children": [
                    {"name": "USD", "node": {}},
                    {"name": "USD", "node": {}}
                ]
            }
        }]
    }"#;
    assert!(MarketDataHierarchy::from_json(dup_children).is_err());

    let dup_tags = r#"{
        "roots": [{
            "name": "Rates",
            "node": {
                "tags": {"desk": "a", "desk": "b"}
            }
        }]
    }"#;
    assert!(MarketDataHierarchy::from_json(dup_tags).is_err());
    assert!(serde_json::from_str::<MarketDataHierarchy>(dup_tags).is_err());
}

// ─── Resolution engine tests ─────────────────────────────────────────────────

#[test]
fn resolve_emits_unique_targets_in_preorder_with_deepest_matching_ancestor() {
    // Both `Credit` and `Credit/US` match `Exists(scope)`. JPM-5Y must be emitted
    // once (not twice), with deepest matching ancestor `Credit/US` (depth 2).
    // Meanwhile `Credit/B` matches at `Credit` (depth 1) and must keep preorder after `Credit/US`.
    let h = MarketDataHierarchy::builder()
        .add_node("Credit")
        .tag("scope", "all")
        .target(MarketDataTarget::HazardCurve(CurveId::from("ROOT-CDS")))
        .add_node("Credit/US")
        .tag("scope", "us")
        .add_node("Credit/US/IG")
        .target(MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")))
        .add_node("Credit/B")
        .target(MarketDataTarget::HazardCurve(CurveId::from("B-CDS")))
        .build()
        .unwrap();

    let target = HierarchyTarget::new(
        vec!["Credit".into()],
        Some(TagFilter {
            predicates: vec![TagPredicate::Exists {
                key: "scope".into(),
            }],
        }),
    )
    .unwrap();

    let matches = resolved_matches(&h, &target);
    assert_eq!(
        matches,
        vec![
            ResolvedTargetMatch {
                target: MarketDataTarget::HazardCurve(CurveId::from("ROOT-CDS")),
                matched_depth: 1,
                matched_path: vec!["Credit".into()],
            },
            ResolvedTargetMatch {
                target: MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")),
                matched_depth: 2,
                matched_path: vec!["Credit".into(), "US".into()],
            },
            ResolvedTargetMatch {
                target: MarketDataTarget::HazardCurve(CurveId::from("B-CDS")),
                matched_depth: 1,
                matched_path: vec!["Credit".into()],
            },
        ]
    );
}

#[test]
fn resolve_treats_empty_tag_filter_identically_to_none() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG")
        .target(MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")))
        .build()
        .unwrap();

    let none_target = HierarchyTarget {
        path: vec!["Credit".into()],
        tag_filter: None,
    };
    let empty_target = HierarchyTarget {
        path: vec!["Credit".into()],
        tag_filter: Some(TagFilter { predicates: vec![] }),
    };

    assert_eq!(none_target, empty_target);
    assert_eq!(
        serde_json::to_string(&none_target).unwrap(),
        serde_json::to_string(&empty_target).unwrap()
    );
    assert_eq!(
        resolved_matches(&h, &none_target),
        resolved_matches(&h, &empty_target)
    );
    assert_eq!(resolved_matches(&h, &empty_target)[0].matched_depth, 1);

    let from_json =
        HierarchyTarget::from_json(r#"{"path":["Credit"],"tag_filter":{"predicates":[]}}"#)
            .unwrap();
    assert!(from_json.tag_filter.is_none());
}

#[test]
fn resolve_errors_on_nonexistent_path_and_zero_matches() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD")
        .target(MarketDataTarget::DiscountCurve(CurveId::from("USD-OIS")))
        .build()
        .unwrap();

    let missing = HierarchyTarget::unfiltered(vec!["Nonexistent".into()]).unwrap();
    assert!(h.resolve_matches(&missing).is_err());

    let no_tag_match = HierarchyTarget::new(
        vec!["Rates".into()],
        Some(TagFilter {
            predicates: vec![TagPredicate::Exists {
                key: "nonexistent".into(),
            }],
        }),
    )
    .unwrap();
    assert!(h.resolve_matches(&no_tag_match).is_err());
}

#[test]
fn resolve_with_tag_filter_includes_descendants_of_matching_parent_even_if_child_differs() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US")
        .tag("region", "us")
        .add_node("Credit/US/Sub")
        .tag("region", "eu")
        .target(MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")))
        .build()
        .unwrap();

    let target = HierarchyTarget::new(
        vec!["Credit".into()],
        Some(TagFilter {
            predicates: vec![TagPredicate::Equals {
                key: "region".into(),
                value: "us".into(),
            }],
        }),
    )
    .unwrap();

    // Subtree inclusion under matching parent `Credit/US` retains descendants.
    let matches = resolved_matches(&h, &target);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].target.id(), &CurveId::from("JPM-5Y"));
    assert_eq!(matches[0].matched_path, vec!["Credit", "US"]);
}

#[test]
fn multi_predicate_filter_evaluates_locally_per_node() {
    // Parent has region=us, child has only rating=A; neither node satisfies BOTH locally.
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US")
        .tag("region", "us")
        .add_node("Credit/US/IG")
        .tag("rating", "A")
        .target(MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")))
        .build()
        .unwrap();

    let target = HierarchyTarget::new(
        vec!["Credit".into()],
        Some(TagFilter {
            predicates: vec![
                TagPredicate::Equals {
                    key: "region".into(),
                    value: "us".into(),
                },
                TagPredicate::Equals {
                    key: "rating".into(),
                    value: "A".into(),
                },
            ],
        }),
    )
    .unwrap();

    assert!(h.resolve_matches(&target).is_err());
}

#[test]
fn tag_predicates_and_in_empty_semantics() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG")
        .tag("rating", "A")
        .tag("sector", "Financials")
        .target(MarketDataTarget::HazardCurve(CurveId::from("JPM-5Y")))
        .add_node("Credit/EU/IG")
        .tag("rating", "AA")
        .tag("sector", "Industrials")
        .target(MarketDataTarget::HazardCurve(CurveId::from("SIEMENS-5Y")))
        .add_node("Credit/EU/HY")
        .tag("rating", "BB")
        .target(MarketDataTarget::HazardCurve(CurveId::from("PEUGEOT-3Y")))
        .build()
        .unwrap();

    let eq_target = HierarchyTarget::new(
        vec!["Credit".into()],
        Some(TagFilter {
            predicates: vec![TagPredicate::Equals {
                key: "rating".into(),
                value: "A".into(),
            }],
        }),
    )
    .unwrap();
    assert_eq!(resolved_ids(&h, &eq_target), vec![CurveId::from("JPM-5Y")]);

    let in_target = HierarchyTarget::new(
        vec!["Credit".into()],
        Some(TagFilter {
            predicates: vec![TagPredicate::In {
                key: "rating".into(),
                values: vec!["A".into(), "AA".into()],
            }],
        }),
    )
    .unwrap();
    assert_eq!(
        resolved_ids(&h, &in_target),
        vec![CurveId::from("JPM-5Y"), CurveId::from("SIEMENS-5Y")]
    );

    let in_empty = HierarchyTarget::new(
        vec!["Credit".into()],
        Some(TagFilter {
            predicates: vec![TagPredicate::In {
                key: "rating".into(),
                values: vec![],
            }],
        }),
    )
    .unwrap();
    assert!(h.resolve_matches(&in_empty).is_err());

    let exists_target = HierarchyTarget::new(
        vec!["Credit".into()],
        Some(TagFilter {
            predicates: vec![TagPredicate::Exists {
                key: "sector".into(),
            }],
        }),
    )
    .unwrap();
    assert_eq!(
        resolved_ids(&h, &exists_target),
        vec![CurveId::from("JPM-5Y"), CurveId::from("SIEMENS-5Y")]
    );
}

#[test]
fn hierarchy_target_validation_rejects_invalid_shapes() {
    assert!(HierarchyTarget::unfiltered(vec![]).is_err());
    assert!(HierarchyTarget::unfiltered(vec!["  ".into()]).is_err());
    assert!(HierarchyTarget::unfiltered(vec!["Credit/US".into()]).is_err());
    assert!(
        HierarchyTarget::from_json(
            r#"{"path":["Credit"],"tag_filter":{"predicates":[{"equals":{"key":" ","value":"a"}}]}}"#
        )
        .is_err()
    );
    assert!(
        HierarchyTarget::from_json(
            r#"{"path":["Credit"],"tag_filter":{"predicates":[{"equals":{"key":"k","value":"v","extra":1}}]}}"#
        )
        .is_err()
    );
    assert!(HierarchyTarget::from_json(r#"{"path":["Credit"],"unknown":1}"#).is_err());
    assert!(HierarchyTarget::from_json(r#"{"path":["Credit"],"path":["Rates"]}"#).is_err());
}

mod strict_raw_fields {
    use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;

    #[test]
    fn market_hierarchy_rejects_unknown_raw_fields_and_legacy_map_shape() {
        let top_level = serde_json::json!({
            "roots": [],
            "__nonexistent__": true
        });
        let nested = serde_json::json!({
            "roots": [{
                "name": "Rates",
                "node": {
                    "targets": [],
                    "__nonexistent__": true
                }
            }]
        });
        let legacy_map = serde_json::json!({
            "roots": {
                "Rates": {
                    "curves": []
                }
            }
        });

        for value in [top_level, nested] {
            let error = serde_json::from_value::<MarketDataHierarchy>(value)
                .expect_err("unknown raw hierarchy fields must fail");
            assert!(error.to_string().contains("unknown field"), "{error}");
        }
        assert!(
            serde_json::from_value::<MarketDataHierarchy>(legacy_map).is_err(),
            "legacy object-map roots shape must be rejected"
        );
    }
}
