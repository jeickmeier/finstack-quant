use finstack_core::market_data::hierarchy::{
    HierarchyNode, HierarchyTarget, MarketDataHierarchy, ResolutionMode, TagFilter, TagPredicate,
};
use finstack_core::types::CurveId;

#[test]
fn empty_hierarchy_has_no_roots() {
    let h = MarketDataHierarchy::new();
    assert!(h.roots().is_empty());
}

#[test]
fn hierarchy_node_stores_name_and_curves() {
    let node = HierarchyNode::new("USD");
    assert_eq!(node.name(), "USD");
    assert!(node.curve_ids().is_empty());
    assert!(node.children().is_empty());
    assert!(node.tags().is_empty());
}

#[test]
fn node_path_is_vec_of_strings() {
    let path: Vec<String> = vec!["Rates".into(), "USD".into()];
    assert_eq!(path.len(), 2);
    assert_eq!(path[0], "Rates");
}

#[test]
fn builder_creates_hierarchy_with_slash_paths() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .add_node("Rates/USD/Forward/SOFR")
        .curve_ids(&["USD-SOFR-3M", "USD-SOFR-6M"])
        .add_node("Credit/US/IG/Financials")
        .tag("sector", "Financials")
        .tag("rating", "A")
        .curve_ids(&["JPM-5Y", "GS-5Y"])
        .build()
        .unwrap();

    // Check structure
    assert_eq!(h.roots().len(), 2); // Rates, Credit
    assert!(h.roots().contains_key("Rates"));
    assert!(h.roots().contains_key("Credit"));

    // Check deep path
    let path: Vec<String> = vec!["Rates".into(), "USD".into(), "OIS".into()];
    let node = h.get_node(&path).unwrap();
    assert_eq!(node.curve_ids().len(), 1);
    assert_eq!(node.curve_ids()[0], CurveId::from("USD-OIS"));

    // Check tags
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
fn builder_rejects_duplicate_curve_ids() {
    let result = MarketDataHierarchy::builder()
        .add_node("Rates/USD")
        .curve_ids(&["USD-OIS"])
        .add_node("Credit/US")
        .curve_ids(&["USD-OIS"]) // duplicate!
        .build();

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("USD-OIS"),
        "Error should mention the duplicate: {err}"
    );
}

#[test]
fn builder_rejects_invalid_paths_without_panicking() {
    for path in ["", "Rates/", "Rates//USD"] {
        let result = std::panic::catch_unwind(|| {
            MarketDataHierarchy::builder()
                .add_node(path)
                .curve_ids(&["USD-OIS"])
                .build()
        });

        let build_result = result.unwrap_or_else(|_| {
            panic!("builder should reject invalid path {path:?} with an error, not panic")
        });
        assert!(
            build_result.is_err(),
            "builder should reject invalid path {path:?}"
        );
    }
}

#[test]
fn all_curve_ids_collects_entire_tree() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .add_node("Rates/EUR/ESTR")
        .curve_ids(&["EUR-ESTR"])
        .add_node("Credit/US/IG")
        .curve_ids(&["JPM-5Y"])
        .build()
        .unwrap();

    let all = h.all_curve_ids();
    assert_eq!(all.len(), 3);
}

#[test]
fn path_for_curve_finds_correct_location() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .add_node("Credit/US/IG/Financials")
        .curve_ids(&["JPM-5Y"])
        .build()
        .unwrap();

    let path = h.path_for_curve(&CurveId::from("JPM-5Y")).unwrap();
    assert_eq!(path, vec!["Credit", "US", "IG", "Financials"]);

    assert!(h.path_for_curve(&CurveId::from("NONEXISTENT")).is_none());
}

#[test]
fn serde_round_trip() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .add_node("Credit/US/IG/Financials")
        .tag("sector", "Financials")
        .curve_ids(&["JPM-5Y", "GS-5Y"])
        .build()
        .unwrap();

    let json = serde_json::to_string_pretty(&h).unwrap();
    let deserialized: MarketDataHierarchy = serde_json::from_str(&json).unwrap();

    // Verify structure preserved
    assert_eq!(deserialized.roots().len(), h.roots().len());
    assert_eq!(deserialized.all_curve_ids().len(), h.all_curve_ids().len());

    let path: Vec<String> = vec![
        "Credit".into(),
        "US".into(),
        "IG".into(),
        "Financials".into(),
    ];
    let node = deserialized.get_node(&path).unwrap();
    assert_eq!(node.tags().get("sector").unwrap(), "Financials");
    assert_eq!(node.curve_ids().len(), 2);
}

#[test]
fn insert_and_remove_curve() {
    let mut h = MarketDataHierarchy::builder()
        .add_node("Rates/USD")
        .curve_ids(&["USD-OIS"])
        .build()
        .unwrap();

    h.insert_curve("Rates/USD", "USD-SOFR-3M").unwrap();
    let path: Vec<String> = vec!["Rates".into(), "USD".into()];
    assert_eq!(h.get_node(&path).unwrap().curve_ids().len(), 2);

    assert!(h.remove_curve(&CurveId::from("USD-OIS")));
    assert_eq!(h.get_node(&path).unwrap().curve_ids().len(), 1);
    assert!(!h.remove_curve(&CurveId::from("NONEXISTENT")));
}

// ─── Resolution engine tests ─────────────────────────────────────────────────

/// Build a hierarchy where both `Credit` and `Credit/US` match the same tag
/// filter, so inherited-subtree matching produces duplicate occurrences for the
/// descendant curve without requiring invalid duplicate placements in the tree.
fn hierarchy_with_overlapping_tag_matches() -> MarketDataHierarchy {
    MarketDataHierarchy::builder()
        .add_node("Credit")
        .tag("scope", "all")
        .curve_ids(&[])
        .add_node("Credit/US")
        .tag("scope", "us")
        .curve_ids(&[])
        .add_node("Credit/US/IG")
        .curve_ids(&["JPM-5Y"])
        .build()
        .unwrap()
}

#[test]
fn resolve_most_specific_wins_deduplicates_by_depth() {
    let h = hierarchy_with_overlapping_tag_matches();

    let target = HierarchyTarget {
        path: vec!["Credit".into()],
        tag_filter: Some(TagFilter {
            predicates: vec![TagPredicate::Exists {
                key: "scope".into(),
            }],
        }),
    };
    let mut ids = h.resolve(&target, ResolutionMode::MostSpecificWins);
    ids.sort();

    // `Credit` and `Credit/US` both match the filter, so inherited-subtree
    // matching sees JPM-5Y twice. MostSpecificWins should keep only the deeper
    // `Credit/US` match.
    assert_eq!(
        ids.len(),
        1,
        "expected 1 distinct curve, got {}: {:?}",
        ids.len(),
        ids
    );
    assert_eq!(ids[0], CurveId::from("JPM-5Y"));
}

#[test]
fn resolve_cumulative_returns_all_occurrences() {
    let h = hierarchy_with_overlapping_tag_matches();

    let target = HierarchyTarget {
        path: vec!["Credit".into()],
        tag_filter: Some(TagFilter {
            predicates: vec![TagPredicate::Exists {
                key: "scope".into(),
            }],
        }),
    };
    let ids = h.resolve(&target, ResolutionMode::Cumulative);

    // `Credit` and `Credit/US` both match the filter, so JPM-5Y is collected
    // once from each matching subtree.
    assert_eq!(
        ids.len(),
        2,
        "expected 2 total curve entries (including duplicates), got {}: {:?}",
        ids.len(),
        ids
    );
    assert!(ids.iter().all(|id| *id == "JPM-5Y"));
}

#[test]
fn resolve_returns_empty_for_nonexistent_path() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD")
        .curve_ids(&["USD-OIS"])
        .build()
        .unwrap();

    let target = HierarchyTarget {
        path: vec!["Nonexistent".into()],
        tag_filter: None,
    };
    assert!(h
        .resolve(&target, ResolutionMode::MostSpecificWins)
        .is_empty());
    assert!(h.resolve(&target, ResolutionMode::Cumulative).is_empty());
}

#[test]
fn query_by_tags_finds_curves_where_node_tag_matches() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG/Financials")
        .tag("sector", "Financials")
        .curve_ids(&["JPM-5Y", "GS-5Y"])
        .add_node("Credit/US/IG/Technology")
        .tag("sector", "Technology")
        .curve_ids(&["MSFT-5Y"])
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::Equals {
            key: "sector".into(),
            value: "Financials".into(),
        }],
    };

    let mut ids = h.query_by_tags(&filter);
    ids.sort();

    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&CurveId::from("JPM-5Y")));
    assert!(ids.contains(&CurveId::from("GS-5Y")));
    assert!(!ids.contains(&CurveId::from("MSFT-5Y")));
    assert!(!ids.contains(&CurveId::from("USD-OIS")));
}

#[test]
fn query_by_tags_includes_descendant_curves_of_matching_parent() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US")
        .tag("region", "us")
        .curve_ids(&[])
        .add_node("Credit/US/IG")
        .curve_ids(&["JPM-5Y"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::Equals {
            key: "region".into(),
            value: "us".into(),
        }],
    };

    let ids = h.query_by_tags(&filter);
    assert_eq!(ids, vec![CurveId::from("JPM-5Y")]);
}

#[test]
fn tag_predicate_equals_matches_exact_value_only() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG")
        .tag("rating", "A")
        .curve_ids(&["JPM-5Y"])
        .add_node("Credit/EU/HY")
        .tag("rating", "BB")
        .curve_ids(&["PEUGEOT-3Y"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::Equals {
            key: "rating".into(),
            value: "A".into(),
        }],
    };
    let ids = h.query_by_tags(&filter);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], CurveId::from("JPM-5Y"));

    // A predicate for a value that doesn't exist should return nothing.
    let filter_none = TagFilter {
        predicates: vec![TagPredicate::Equals {
            key: "rating".into(),
            value: "AAA".into(),
        }],
    };
    assert!(h.query_by_tags(&filter_none).is_empty());
}

#[test]
fn tag_predicate_in_matches_any_of_the_given_values() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG")
        .tag("rating", "A")
        .curve_ids(&["JPM-5Y"])
        .add_node("Credit/EU/IG")
        .tag("rating", "AA")
        .curve_ids(&["SIEMENS-5Y"])
        .add_node("Credit/EU/HY")
        .tag("rating", "BB")
        .curve_ids(&["PEUGEOT-3Y"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::In {
            key: "rating".into(),
            values: vec!["A".into(), "AA".into()],
        }],
    };
    let mut ids = h.query_by_tags(&filter);
    ids.sort();

    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&CurveId::from("JPM-5Y")));
    assert!(ids.contains(&CurveId::from("SIEMENS-5Y")));
    assert!(!ids.contains(&CurveId::from("PEUGEOT-3Y")));
}

#[test]
fn tag_predicate_exists_matches_key_regardless_of_value() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US/IG")
        .tag("sector", "Financials")
        .curve_ids(&["JPM-5Y"])
        .add_node("Credit/EU/IG")
        .tag("sector", "Industrials")
        .curve_ids(&["BASF-5Y"])
        .add_node("Rates/USD/OIS")
        // No sector tag
        .curve_ids(&["USD-OIS"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::Exists {
            key: "sector".into(),
        }],
    };
    let mut ids = h.query_by_tags(&filter);
    ids.sort();

    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&CurveId::from("JPM-5Y")));
    assert!(ids.contains(&CurveId::from("BASF-5Y")));
    assert!(!ids.contains(&CurveId::from("USD-OIS")));
}

#[test]
fn tag_predicate_exists_returns_empty_when_key_absent_from_all_nodes() {
    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::Exists {
            key: "nonexistent-key".into(),
        }],
    };
    assert!(h.query_by_tags(&filter).is_empty());
}

#[test]
fn resolve_cumulative_with_tag_filter_scopes_to_subtree() {
    // Build a hierarchy with two regions (USD and EUR), each with children
    // tagged with `asset_class`. Only the USD subtree nodes tagged as "rates"
    // should appear in the result.
    //
    // Structure:
    //   USD
    //   USD/Rates        (asset_class=rates)  → curves: USD-OIS, USD-SOFR
    //   USD/Credit       (asset_class=credit) → curves: JPM-5Y
    //   EUR
    //   EUR/Rates        (asset_class=rates)  → curves: EUR-ESTR
    let mut h = MarketDataHierarchy::new();
    h.insert_curve("USD/Rates", "USD-OIS").unwrap();
    h.insert_curve("USD/Rates", "USD-SOFR").unwrap();
    h.insert_curve("USD/Credit", "JPM-5Y").unwrap();
    h.insert_curve("EUR/Rates", "EUR-ESTR").unwrap();

    // Tag the nodes via the builder on a fresh hierarchy (insert_curve does not
    // attach tags). Rebuild using builder to attach tags properly.
    let h = MarketDataHierarchy::builder()
        .add_node("USD/Rates")
        .tag("asset_class", "rates")
        .curve_ids(&["USD-OIS", "USD-SOFR"])
        .add_node("USD/Credit")
        .tag("asset_class", "credit")
        .curve_ids(&["JPM-5Y"])
        .add_node("EUR/Rates")
        .tag("asset_class", "rates")
        .curve_ids(&["EUR-ESTR"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::Equals {
            key: "asset_class".into(),
            value: "rates".into(),
        }],
    };

    // Resolve within the USD subtree with the tag filter in Cumulative mode.
    let target = HierarchyTarget {
        path: vec!["USD".into()],
        tag_filter: Some(filter),
    };
    let mut ids = h.resolve(&target, ResolutionMode::Cumulative);
    ids.sort();

    // Only USD/Rates curves should appear — EUR-ESTR is excluded (wrong path),
    // and JPM-5Y is excluded (wrong tag).
    assert_eq!(
        ids.len(),
        2,
        "expected 2 curves (USD-OIS, USD-SOFR), got {}: {:?}",
        ids.len(),
        ids
    );
    assert!(ids.contains(&CurveId::from("USD-OIS")));
    assert!(ids.contains(&CurveId::from("USD-SOFR")));
    assert!(
        !ids.contains(&CurveId::from("EUR-ESTR")),
        "EUR curve must be excluded by path scoping"
    );
    assert!(
        !ids.contains(&CurveId::from("JPM-5Y")),
        "credit curve must be excluded by tag filter"
    );
}

#[test]
fn resolve_cumulative_with_tag_filter_includes_descendants_of_matching_parent() {
    let h = MarketDataHierarchy::builder()
        .add_node("Credit/US")
        .tag("region", "us")
        .curve_ids(&[])
        .add_node("Credit/US/IG")
        .curve_ids(&["JPM-5Y"])
        .add_node("Credit/EU/IG")
        .curve_ids(&["SIE-5Y"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::Equals {
            key: "region".into(),
            value: "us".into(),
        }],
    };

    let target = HierarchyTarget {
        path: vec!["Credit".into()],
        tag_filter: Some(filter),
    };

    let ids = h.resolve(&target, ResolutionMode::Cumulative);
    assert_eq!(ids, vec![CurveId::from("JPM-5Y")]);
}

#[test]
fn resolve_most_specific_wins_with_tag_filter() {
    // Build a hierarchy where a tagged parent and a tagged child both match.
    // Inherited-subtree matching should see descendant curves for both nodes,
    // and MostSpecificWins should keep only the deeper match.
    //
    // Structure:
    //   Rates                (asset_class=rates)
    //   Rates/USD            (asset_class=rates) → SHARED-RATE, USD-OIS
    let h = MarketDataHierarchy::builder()
        .add_node("Rates")
        .tag("asset_class", "rates")
        .curve_ids(&[])
        .add_node("Rates/USD")
        .tag("asset_class", "rates")
        .curve_ids(&["SHARED-RATE", "USD-OIS"])
        .build()
        .unwrap();

    let filter = TagFilter {
        predicates: vec![TagPredicate::Equals {
            key: "asset_class".into(),
            value: "rates".into(),
        }],
    };

    let target = HierarchyTarget {
        path: vec!["Rates".into()],
        tag_filter: Some(filter),
    };
    let mut ids = h.resolve(&target, ResolutionMode::MostSpecificWins);
    ids.sort();

    // Rates and Rates/USD both match, so inherited-subtree matching sees both
    // descendant curves twice. MostSpecificWins keeps the deeper Rates/USD hit.
    assert_eq!(
        ids.len(),
        2,
        "expected 2 distinct curves (SHARED-RATE, USD-OIS), got {}: {:?}",
        ids.len(),
        ids
    );
    assert!(ids.contains(&CurveId::from("SHARED-RATE")));
    assert!(ids.contains(&CurveId::from("USD-OIS")));
}

// ─── Completeness tracking tests ─────────────────────────────────────────────

#[test]
fn completeness_report_returns_none_without_hierarchy() {
    use finstack_core::market_data::context::MarketContext;
    let market = MarketContext::new();
    assert!(market.completeness_report().is_none());
}

#[test]
fn completeness_report_detects_missing_and_unclassified() {
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use time::Month;

    let base = finstack_core::dates::Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .add_node("Rates/EUR/ESTR")
        .curve_ids(&["EUR-ESTR"]) // will be missing from MarketContext
        .build()
        .unwrap();

    // Build MarketContext with only USD-OIS (EUR-ESTR is missing)
    // Also add an unclassified curve not in hierarchy
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .build()
        .unwrap();
    let extra_curve = DiscountCurve::builder("GBP-SONIA")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.96)])
        .build()
        .unwrap();

    let mut market = MarketContext::new().insert(usd_curve).insert(extra_curve);
    market.set_hierarchy(h);

    let report = market.completeness_report().unwrap();

    // EUR-ESTR is declared but missing
    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].1, CurveId::from("EUR-ESTR"));

    // GBP-SONIA is present but not in hierarchy
    assert_eq!(report.unclassified.len(), 1);
    assert_eq!(report.unclassified[0], CurveId::from("GBP-SONIA"));

    // Coverage: Rates root has 2 expected, 1 present = 50%
    assert!(!report.coverage.is_empty());
}

#[test]
fn completeness_report_does_not_count_collateral_alias_as_present() {
    use finstack_core::market_data::context::MarketContext;

    let h = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .build()
        .unwrap();

    let mut market = MarketContext::new().map_collateral("USD-CSA", CurveId::from("USD-OIS"));
    market.set_hierarchy(h);

    let report = market.completeness_report().unwrap();
    assert_eq!(report.missing.len(), 1);
    assert_eq!(
        report.missing[0],
        (
            vec!["Rates".to_string(), "USD".to_string(), "OIS".to_string()],
            CurveId::from("USD-OIS"),
        )
    );
    assert!(report.unclassified.is_empty());
}
