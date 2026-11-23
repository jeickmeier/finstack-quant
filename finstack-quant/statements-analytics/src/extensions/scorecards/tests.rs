use super::extension::MetricScore;
use super::*;
use finstack_quant_core::rating_scales::RatingScaleRegistry;

fn registry() -> RatingScaleRegistry {
    RatingScaleRegistry::from_json(
        r#"{"schema":"finstack_quant.rating_scales/2","rating_scales":[{
            "id":"internal","source":"test policy",
            "scale":{"scale_name":"Internal","ratings":[
                {"name":"AAA","score":100.0,"min_score":96.0},
                {"name":"AA+","score":95.0,"min_score":91.0},
                {"name":"AA","score":90.0,"min_score":0.0}
            ]}}]}"#,
    )
    .expect("valid registry")
}

fn extension_with(thresholds: &[(&str, (f64, f64))]) -> CreditScorecardExtension {
    CreditScorecardExtension::new(
        ScorecardConfig {
            rating_scale: "internal".into(),
            metrics: vec![ScorecardMetric {
                name: "m".into(),
                formula: "1".into(),
                weight: 1.0,
                thresholds: thresholds
                    .iter()
                    .map(|(label, bounds)| ((*label).to_string(), *bounds))
                    .collect(),
                description: None,
            }],
            min_rating: None,
            period: None,
        },
        &registry(),
    )
    .expect("valid extension")
}

fn score(extension: &CreditScorecardExtension, value: f64) -> Option<f64> {
    extension.matching_threshold_score(value, &extension.thresholds[0])
}

#[test]
fn unmatched_value_returns_no_score() {
    let extension = extension_with(&[("AAA", (0.0, 1.0))]);
    assert_eq!(
        score(&extension, 2.5),
        None,
        "unmatched value must not score a fallback"
    );
}

#[test]
fn unknown_scale_threshold_and_min_rating_labels_are_rejected() {
    let config = |scale: &str, label: &str, min: Option<&str>| ScorecardConfig {
        rating_scale: scale.into(),
        metrics: vec![ScorecardMetric {
            name: "m".into(),
            formula: "1".into(),
            weight: 1.0,
            thresholds: [(label.to_string(), (0.0, 1.0))].into_iter().collect(),
            description: None,
        }],
        min_rating: min.map(str::to_string),
        period: None,
    };
    let registry = registry();
    assert!(CreditScorecardExtension::new(config("sp", "AAA", None), &registry).is_err());
    assert!(CreditScorecardExtension::new(config("internal", "BBB", None), &registry).is_err());
    assert!(
        CreditScorecardExtension::new(config("internal", "AAA", Some("NR")), &registry).is_err()
    );
    assert!(
        CreditScorecardExtension::new(config(" INTERNAL ", " aa+ ", Some("aa")), &registry).is_ok()
    );
    let mut repeated = config("internal", "AAA", None);
    repeated.metrics[0]
        .thresholds
        .insert(" aaa ".into(), (1.0, 2.0));
    assert!(CreditScorecardExtension::new(repeated, &registry).is_err());
}

#[test]
fn calculate_weighted_score_treats_sub_epsilon_weights_as_zero() {
    let extension = extension_with(&[]);
    let scores = vec![MetricScore {
        metric_name: "leverage".to_string(),
        value: 2.5,
        score: 80.0,
        weight: f64::EPSILON / 4.0,
    }];

    assert_eq!(extension.calculate_weighted_score(&scores), None);
}

// Scorecard boundary convention
//
// Buckets are matched in scale order (best band first) and every bucket
// is closed `[min, max]`, so a value on a shared boundary always resolves
// to the *better* of the two adjacent bands — for both higher-is-better
// (coverage) and lower-is-better (leverage) metrics.

#[test]
fn scorecard_top_boundary_value_lands_in_best_bucket() {
    let extension = extension_with(&[("AAA", (95.0, 100.0)), ("AA+", (90.0, 95.0))]);
    assert_eq!(
        score(&extension, 95.0),
        Some(100.0),
        "top-bucket upper bound is inclusive"
    );
    assert_eq!(score(&extension, 100.0), Some(100.0));
}

#[test]
fn scorecard_non_top_shared_boundary_goes_to_better_rating() {
    let extension = extension_with(&[
        ("AAA", (95.0, 100.0)),
        ("AA+", (90.0, 95.0)),
        ("AA", (85.0, 90.0)),
    ]);
    assert_eq!(
        score(&extension, 90.0),
        Some(95.0),
        "shared boundary 90.0 between AA+ and AA must resolve to AA+ (better)"
    );
    assert_eq!(
        score(&extension, 85.0),
        Some(90.0),
        "lower-bound value must land in the bucket it bounds"
    );
}

#[test]
fn scorecard_leverage_boundary_goes_to_better_rating() {
    let extension = extension_with(&[("AAA", (0.0, 1.0)), ("AA+", (1.0, 2.0)), ("AA", (2.0, 3.0))]);
    assert_eq!(
        score(&extension, 2.0),
        Some(95.0),
        "leverage 2.0x on the AA+/AA boundary must resolve to AA+ (better)"
    );
    assert_eq!(score(&extension, 1.0), Some(100.0));
}
