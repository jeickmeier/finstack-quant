//! Comparable company and relative value analysis.
//!
//! Provides peer set construction, valuation multiples computation,
//! cross-sectional statistical analysis, and composite rich/cheap scoring.
//!
//! Start with [`PeerSet::from_universe`] to build a filtered peer group,
//! then use [`compute_peer_multiples`] for multiples and
//! [`score_relative_value`] for the composite rich/cheap signal.

mod multiples;
mod peer_set;
mod scoring;
mod stats;
mod types;

pub use multiples::{compute_multiple, compute_peer_multiples};
pub use peer_set::{PeerFilter, PeerSet};
pub use scoring::{
    score_relative_value, DimensionScore, MetricExtractor, RelativeValueResult, ScoreDirection,
    ScoringDimension,
};
pub use stats::{
    peer_stats, percentile_rank, regression_fair_value, z_score, PeerStats, RegressionResult,
};
pub use types::{CompanyId, CompanyMetrics, Multiple, PeriodBasis};

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::types::Attributes;
    use indexmap::IndexMap;

    // -----------------------------------------------------------------------
    // Helper: build a CompanyMetrics with common fields populated
    // -----------------------------------------------------------------------

    fn make_company(
        id: &str,
        sector: &str,
        rating: &str,
        country: &str,
        leverage: f64,
        oas_bps: f64,
        market_cap: f64,
    ) -> CompanyMetrics {
        CompanyMetrics {
            id: CompanyId::new(id),
            attributes: Attributes::default()
                .with_meta("gics_sector", sector)
                .with_meta("rating", rating)
                .with_meta("country", country),
            enterprise_value: Some(market_cap * 1.5),
            market_cap: Some(market_cap),
            share_price: Some(50.0),
            oas_bps: Some(oas_bps),
            yield_pct: Some(oas_bps / 100.0 + 3.0),
            ebitda: Some(market_cap * 0.15),
            revenue: Some(market_cap * 0.5),
            ebit: Some(market_cap * 0.12),
            ufcf: Some(market_cap * 0.08),
            lfcf: Some(market_cap * 0.06),
            net_income: Some(market_cap * 0.07),
            book_value: Some(market_cap * 0.6),
            tangible_book_value: Some(market_cap * 0.45),
            dividends_per_share: Some(2.0),
            leverage: Some(leverage),
            interest_coverage: Some(8.0 / leverage),
            revenue_growth: Some(0.05),
            ebitda_margin: Some(0.30),
            custom: IndexMap::new(),
        }
    }

    fn make_universe() -> Vec<CompanyMetrics> {
        vec![
            make_company("A", "Energy", "BB", "US", 3.0, 250.0, 5_000.0),
            make_company("B", "Energy", "BB", "US", 4.0, 320.0, 8_000.0),
            make_company("C", "Energy", "B", "US", 5.5, 450.0, 3_000.0),
            make_company("D", "Energy", "BB", "CA", 3.5, 280.0, 6_000.0),
            make_company("E", "Tech", "BBB", "US", 2.0, 150.0, 20_000.0),
            make_company("F", "Energy", "BB", "US", 4.5, 380.0, 7_000.0),
            make_company("G", "Energy", "CCC", "US", 6.0, 550.0, 2_000.0),
            make_company("H", "Financials", "A", "US", 1.5, 100.0, 50_000.0),
            make_company("I", "Energy", "BB", "US", 3.8, 310.0, 9_000.0),
            make_company("J", "Energy", "B", "CA", 5.0, 420.0, 4_000.0),
        ]
    }

    // -----------------------------------------------------------------------
    // Percentile rank tests
    // -----------------------------------------------------------------------

    #[test]
    fn percentile_rank_correctness() {
        let values = [100.0, 200.0, 300.0, 400.0, 500.0];

        // 250 is > 100, 200 but not 300, 400, 500 => 2/5 = 0.4
        assert_eq!(percentile_rank(&values, 250.0), Some(0.4));

        // Value below all => 0/5 = 0.0
        assert_eq!(percentile_rank(&values, 50.0), Some(0.0));

        // Value at max => 5/5 = 1.0
        assert_eq!(percentile_rank(&values, 500.0), Some(1.0));

        // Value at min => 1/5 = 0.2
        assert_eq!(percentile_rank(&values, 100.0), Some(0.2));

        // Value above all => 5/5 = 1.0
        assert_eq!(percentile_rank(&values, 600.0), Some(1.0));

        // Empty slice
        assert_eq!(percentile_rank(&[], 100.0), None);
    }

    // -----------------------------------------------------------------------
    // Z-score tests
    // -----------------------------------------------------------------------

    #[test]
    fn z_score_known_values() {
        // mean = 3.0, std_dev = sqrt(var) where var = 2.5 (sample variance)
        let values = [1.0, 2.0, 3.0, 4.0, 5.0];
        let z = z_score(&values, 3.0).expect("z-score of mean should be defined");
        assert!(z.abs() < 1e-10, "z-score of the mean should be ~0, got {z}");

        // Value 1 standard deviation above
        let sd = (2.5_f64).sqrt(); // sample std dev of [1,2,3,4,5]
        let z_above = z_score(&values, 3.0 + sd).expect("z-score should be defined");
        assert!(
            (z_above - 1.0).abs() < 1e-10,
            "z-score should be 1.0, got {z_above}"
        );
    }

    #[test]
    fn z_score_edge_cases() {
        // Fewer than 2 values
        assert_eq!(z_score(&[1.0], 1.0), None);
        assert_eq!(z_score(&[], 1.0), None);

        // All identical values (zero std dev)
        assert_eq!(z_score(&[5.0, 5.0, 5.0], 5.0), None);
    }

    // -----------------------------------------------------------------------
    // Regression tests
    // -----------------------------------------------------------------------

    #[test]
    fn regression_r_squared_positive_for_correlated_data() {
        // y = 2x + 1 (perfect linear relationship)
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [3.0, 5.0, 7.0, 9.0, 11.0];

        let result = regression_fair_value(&x, &y, 3.0, 7.0).expect("regression should succeed");

        assert!(
            result.r_squared > 0.99,
            "R-squared should be ~1 for perfect linear data, got {}",
            result.r_squared
        );
        assert!(
            (result.slope - 2.0).abs() < 1e-8,
            "Slope should be ~2.0, got {}",
            result.slope
        );
        assert!(
            (result.intercept - 1.0).abs() < 1e-8,
            "Intercept should be ~1.0, got {}",
            result.intercept
        );
        assert!(
            result.residual.abs() < 1e-8,
            "Residual should be ~0 for point on line, got {}",
            result.residual
        );
    }

    #[test]
    fn regression_residual_sign() {
        // y = 2x + 1
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [3.0, 5.0, 7.0, 9.0, 11.0];

        // Subject above the line: actual 10 vs fitted 7 => positive residual (cheap)
        let above = regression_fair_value(&x, &y, 3.0, 10.0).expect("regression");
        assert!(
            above.residual > 0.0,
            "Subject above line should have positive residual"
        );

        // Subject below the line: actual 4 vs fitted 7 => negative residual (rich)
        let below = regression_fair_value(&x, &y, 3.0, 4.0).expect("regression");
        assert!(
            below.residual < 0.0,
            "Subject below line should have negative residual"
        );
    }

    #[test]
    fn regression_with_noise() {
        // y ~ 50x + 100 + noise
        let x = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let y = [
            155.0, 198.0, 252.0, 305.0, 348.0, 402.0, 455.0, 498.0, 552.0, 605.0,
        ];

        let result = regression_fair_value(&x, &y, 5.5, 375.0).expect("regression");
        assert!(
            result.r_squared > 0.99,
            "R-squared should be high for near-linear data, got {}",
            result.r_squared
        );
        assert!(
            (result.slope - 50.0).abs() < 2.0,
            "Slope should be approximately 50, got {}",
            result.slope
        );
    }

    #[test]
    fn regression_too_few_points() {
        assert!(regression_fair_value(&[1.0, 2.0], &[3.0, 5.0], 1.5, 4.0).is_none());
    }

    // -----------------------------------------------------------------------
    // Peer stats tests
    // -----------------------------------------------------------------------

    #[test]
    fn peer_stats_correctness() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = peer_stats(&values).expect("peer_stats should succeed");

        assert_eq!(stats.count, 5);
        assert!((stats.mean - 3.0).abs() < 1e-10);
        assert!((stats.median - 3.0).abs() < 1e-10);
        assert!((stats.min - 1.0).abs() < 1e-10);
        assert!((stats.max - 5.0).abs() < 1e-10);
        assert!((stats.q1 - 2.0).abs() < 1e-10);
        assert!((stats.q3 - 4.0).abs() < 1e-10);
        assert!(stats.std_dev > 0.0);
    }

    #[test]
    fn peer_stats_empty() {
        assert!(peer_stats(&[]).is_none());
    }

    #[test]
    fn peer_stats_single_value() {
        let stats = peer_stats(&[42.0]).expect("single value stats");
        assert_eq!(stats.count, 1);
        assert!((stats.mean - 42.0).abs() < 1e-10);
        assert!((stats.median - 42.0).abs() < 1e-10);
        assert!((stats.min - 42.0).abs() < 1e-10);
        assert!((stats.max - 42.0).abs() < 1e-10);
    }

    // -----------------------------------------------------------------------
    // PeerFilter tests
    // -----------------------------------------------------------------------

    #[test]
    fn peer_filter_by_sector() {
        let universe = make_universe();
        let filter = PeerFilter {
            gics_sectors: vec!["Energy".to_string()],
            ..Default::default()
        };

        let accepted: Vec<_> = universe.iter().filter(|c| filter.accepts(c)).collect();
        assert_eq!(accepted.len(), 8); // A, B, C, D, F, G, I, J
        for c in &accepted {
            assert_eq!(c.attributes.get_meta("gics_sector"), Some("Energy"),);
        }
    }

    #[test]
    fn peer_filter_by_sector_and_rating() {
        let universe = make_universe();
        let filter = PeerFilter {
            gics_sectors: vec!["Energy".to_string()],
            ratings: vec!["BB".to_string()],
            ..Default::default()
        };

        let accepted: Vec<_> = universe.iter().filter(|c| filter.accepts(c)).collect();
        // A(BB), B(BB), D(BB), F(BB), I(BB) - all Energy + BB
        assert_eq!(accepted.len(), 5);
    }

    #[test]
    fn peer_filter_by_market_cap_range() {
        let universe = make_universe();
        let filter = PeerFilter {
            market_cap_min: Some(5_000.0),
            market_cap_max: Some(10_000.0),
            ..Default::default()
        };

        let accepted: Vec<_> = universe.iter().filter(|c| filter.accepts(c)).collect();
        // A(5k), B(8k), D(6k), F(7k), I(9k) = 5
        assert_eq!(accepted.len(), 5);
    }

    #[test]
    fn peer_filter_by_country() {
        let universe = make_universe();
        let filter = PeerFilter {
            countries: vec!["CA".to_string()],
            ..Default::default()
        };

        let accepted: Vec<_> = universe.iter().filter(|c| filter.accepts(c)).collect();
        assert_eq!(accepted.len(), 2); // D and J
    }

    #[test]
    fn peer_filter_empty_accepts_all() {
        let universe = make_universe();
        let filter = PeerFilter::default();

        let accepted: Vec<_> = universe.iter().filter(|c| filter.accepts(c)).collect();
        assert_eq!(accepted.len(), universe.len());
    }

    #[test]
    fn peer_filter_with_tags() {
        let mut universe = make_universe();
        universe[0].attributes = universe[0].attributes.clone().with_tag("high_yield");
        universe[1].attributes = universe[1].attributes.clone().with_tag("high_yield");

        let filter = PeerFilter {
            required_tags: vec!["high_yield".to_string()],
            ..Default::default()
        };

        let accepted: Vec<_> = universe.iter().filter(|c| filter.accepts(c)).collect();
        assert_eq!(accepted.len(), 2);
    }

    #[test]
    fn peer_filter_excluded_tags() {
        let mut universe = make_universe();
        universe[0].attributes = universe[0].attributes.clone().with_tag("exclude_me");

        let filter = PeerFilter {
            excluded_tags: vec!["exclude_me".to_string()],
            ..Default::default()
        };

        let accepted: Vec<_> = universe.iter().filter(|c| filter.accepts(c)).collect();
        assert_eq!(accepted.len(), universe.len() - 1);
    }

    // -----------------------------------------------------------------------
    // PeerSet construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn peer_set_excludes_subject() {
        let universe = make_universe();
        let subject = universe[0].clone(); // Company A
        let filter = PeerFilter::default();

        let peer_set = PeerSet::from_universe(subject, &universe, &filter, PeriodBasis::Ltm);

        // Subject should not appear in peers
        assert!(!peer_set.peers.iter().any(|p| p.id.as_str() == "A"));
        assert_eq!(peer_set.peer_count(), universe.len() - 1);
    }

    #[test]
    fn peer_set_with_filter() {
        let universe = make_universe();
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 330.0, 6_500.0);
        let filter = PeerFilter {
            gics_sectors: vec!["Energy".to_string()],
            ratings: vec!["BB".to_string()],
            ..Default::default()
        };

        let peer_set = PeerSet::from_universe(subject, &universe, &filter, PeriodBasis::Ltm);
        // Energy + BB: A, B, D, F, I
        assert_eq!(peer_set.peer_count(), 5);
    }

    // -----------------------------------------------------------------------
    // Multiples computation tests
    // -----------------------------------------------------------------------

    #[test]
    fn compute_ev_ebitda() {
        let c = make_company("TEST", "Energy", "BB", "US", 4.0, 300.0, 10_000.0);
        let ev_ebitda = compute_multiple(&c, Multiple::EvEbitda);
        assert!(ev_ebitda.is_some());
        // EV = 15000, EBITDA = 1500 => 10.0x
        let val = ev_ebitda.unwrap();
        assert!(
            (val - 10.0).abs() < 1e-10,
            "EV/EBITDA should be 10.0x, got {val}"
        );
    }

    #[test]
    fn compute_multiple_missing_data() {
        let mut c = CompanyMetrics::new("EMPTY");
        c.enterprise_value = Some(1000.0);
        // EBITDA is None
        assert!(compute_multiple(&c, Multiple::EvEbitda).is_none());
    }

    #[test]
    fn compute_multiple_negative_denominator() {
        let mut c = CompanyMetrics::new("NEG");
        c.enterprise_value = Some(1000.0);
        c.ebitda = Some(-100.0); // Negative EBITDA
        assert!(compute_multiple(&c, Multiple::EvEbitda).is_none());
    }

    #[test]
    fn compute_peer_multiples_filters_missing() {
        let universe = make_universe();
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 330.0, 6_500.0);
        let peer_set = PeerSet::new(subject, universe, PeriodBasis::Ltm);

        let multiples = compute_peer_multiples(&peer_set, Multiple::EvEbitda);
        assert_eq!(multiples.len(), 10); // All peers have EV and EBITDA
    }

    // -----------------------------------------------------------------------
    // Rich/cheap scoring tests
    // -----------------------------------------------------------------------

    #[test]
    fn scoring_single_dimension_univariate() {
        let universe = make_universe();
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 330.0, 6_500.0);
        let peer_set = PeerSet::new(subject, universe, PeriodBasis::Ltm);

        let dimensions = vec![ScoringDimension {
            label: "Spread Level".to_string(),
            y_extractor: MetricExtractor::Named("oas_bps".to_string()),
            x_extractors: vec![],
            weight: 1.0,
            direction: ScoreDirection::default(),
        }];

        let result = score_relative_value(&peer_set, &dimensions).expect("scoring should succeed");

        assert_eq!(result.company_id.as_str(), "SUBJECT");
        assert_eq!(result.dimensions.len(), 1);
        assert_eq!(result.peer_count, 10);

        let dim = &result.dimensions[0];
        assert!(dim.percentile >= 0.0 && dim.percentile <= 1.0);
        assert!(dim.regression_residual.is_none()); // No X extractor
    }

    #[test]
    fn scoring_regression_dimension() {
        let universe = make_universe();
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 330.0, 6_500.0);
        let peer_set = PeerSet::new(subject, universe, PeriodBasis::Ltm);

        let dimensions = vec![ScoringDimension {
            label: "Spread vs Leverage".to_string(),
            y_extractor: MetricExtractor::Named("oas_bps".to_string()),
            x_extractors: vec![MetricExtractor::Named("leverage".to_string())],
            weight: 1.0,
            direction: ScoreDirection::default(),
        }];

        let result = score_relative_value(&peer_set, &dimensions).expect("scoring should succeed");

        let dim = &result.dimensions[0];
        assert!(
            dim.regression_residual.is_some(),
            "Regression residual should be present"
        );
        assert!(dim.r_squared.is_some(), "R-squared should be present");
        assert!(result.confidence > 0.0, "Confidence should be positive");
    }

    #[test]
    fn scoring_regression_positive_residual_is_cheap() {
        let peers = vec![
            make_company("A", "Energy", "BB", "US", 1.0, 100.0, 5_000.0),
            make_company("B", "Energy", "BB", "US", 2.0, 200.0, 5_000.0),
            make_company("C", "Energy", "BB", "US", 3.0, 300.0, 5_000.0),
        ];
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 2.0, 250.0, 5_000.0);
        let peer_set = PeerSet::new(subject, peers, PeriodBasis::Ltm);

        let dimensions = vec![ScoringDimension {
            label: "Spread vs Leverage".to_string(),
            y_extractor: MetricExtractor::Named("oas_bps".to_string()),
            x_extractors: vec![MetricExtractor::Named("leverage".to_string())],
            weight: 1.0,
            direction: ScoreDirection::default(),
        }];

        let result = score_relative_value(&peer_set, &dimensions).expect("scoring should succeed");
        let dim = &result.dimensions[0];

        assert_eq!(dim.regression_residual, Some(50.0));
        assert!(
            result.composite_score > 0.0,
            "positive residual should produce a cheap positive score"
        );
    }

    #[test]
    fn scoring_empty_dimensions_error() {
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 330.0, 6_500.0);
        let peer_set = PeerSet::new(subject, vec![], PeriodBasis::Ltm);

        let result = score_relative_value(&peer_set, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn scoring_multi_dimension_weights() {
        let universe = make_universe();
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 330.0, 6_500.0);
        let peer_set = PeerSet::new(subject, universe, PeriodBasis::Ltm);

        let dimensions = vec![
            ScoringDimension {
                label: "Spread vs Leverage".to_string(),
                y_extractor: MetricExtractor::Named("oas_bps".to_string()),
                x_extractors: vec![MetricExtractor::Named("leverage".to_string())],
                weight: 0.5,
                direction: ScoreDirection::default(),
            },
            ScoringDimension {
                label: "Spread Level".to_string(),
                y_extractor: MetricExtractor::Named("oas_bps".to_string()),
                x_extractors: vec![],
                weight: 0.3,
                direction: ScoreDirection::default(),
            },
            ScoringDimension {
                label: "EV/EBITDA".to_string(),
                y_extractor: MetricExtractor::Multiple(Multiple::EvEbitda),
                x_extractors: vec![],
                weight: 0.2,
                direction: ScoreDirection::default(),
            },
        ];

        let result = score_relative_value(&peer_set, &dimensions).expect("scoring should succeed");

        assert_eq!(result.dimensions.len(), 3);
        // Verify weights are preserved
        assert!((result.dimensions[0].weight - 0.5).abs() < 1e-10);
        assert!((result.dimensions[1].weight - 0.3).abs() < 1e-10);
        assert!((result.dimensions[2].weight - 0.2).abs() < 1e-10);
    }

    // -----------------------------------------------------------------------
    // Serde round-trip tests
    // -----------------------------------------------------------------------

    #[test]
    fn serde_company_metrics_roundtrip() {
        let c = make_company("TEST", "Energy", "BB", "US", 4.0, 300.0, 10_000.0);
        let json = serde_json::to_string(&c).expect("serialize CompanyMetrics");
        let deserialized: CompanyMetrics =
            serde_json::from_str(&json).expect("deserialize CompanyMetrics");
        assert_eq!(deserialized.id, c.id);
        assert_eq!(deserialized.leverage, c.leverage);
        assert_eq!(deserialized.oas_bps, c.oas_bps);
    }

    #[test]
    fn serde_peer_set_roundtrip() {
        let universe = make_universe();
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 330.0, 6_500.0);
        let peer_set = PeerSet::new(subject, universe, PeriodBasis::Ltm);

        let json = serde_json::to_string(&peer_set).expect("serialize PeerSet");
        let deserialized: PeerSet = serde_json::from_str(&json).expect("deserialize PeerSet");
        assert_eq!(deserialized.subject.id, peer_set.subject.id);
        assert_eq!(deserialized.peers.len(), peer_set.peers.len());
    }

    #[test]
    fn serde_peer_filter_roundtrip() {
        let filter = PeerFilter {
            gics_sectors: vec!["Energy".to_string()],
            ratings: vec!["BB".to_string(), "B".to_string()],
            market_cap_min: Some(1_000.0),
            ..Default::default()
        };

        let json = serde_json::to_string(&filter).expect("serialize PeerFilter");
        let deserialized: PeerFilter = serde_json::from_str(&json).expect("deserialize PeerFilter");
        assert_eq!(deserialized.gics_sectors, filter.gics_sectors);
        assert_eq!(deserialized.ratings, filter.ratings);
        assert_eq!(deserialized.market_cap_min, filter.market_cap_min);
    }

    #[test]
    fn serde_relative_value_result_roundtrip() {
        let universe = make_universe();
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 330.0, 6_500.0);
        let peer_set = PeerSet::new(subject, universe, PeriodBasis::Ltm);

        let dimensions = vec![ScoringDimension {
            label: "Spread vs Leverage".to_string(),
            y_extractor: MetricExtractor::Named("oas_bps".to_string()),
            x_extractors: vec![MetricExtractor::Named("leverage".to_string())],
            weight: 1.0,
            direction: ScoreDirection::default(),
        }];

        let result = score_relative_value(&peer_set, &dimensions).expect("scoring");

        let json = serde_json::to_string(&result).expect("serialize RelativeValueResult");
        let deserialized: RelativeValueResult =
            serde_json::from_str(&json).expect("deserialize RelativeValueResult");
        assert_eq!(deserialized.company_id, result.company_id);
        assert!((deserialized.composite_score - result.composite_score).abs() < 1e-10);
        assert_eq!(deserialized.dimensions.len(), result.dimensions.len());
    }

    // -----------------------------------------------------------------------
    // Property-like tests
    // -----------------------------------------------------------------------

    #[test]
    fn percentile_rank_always_bounded() {
        let values = [10.0, 20.0, 30.0, 40.0, 50.0];
        for test_val in [0.0, 10.0, 25.0, 50.0, 100.0] {
            let p = percentile_rank(&values, test_val).unwrap();
            assert!(
                (0.0..=1.0).contains(&p),
                "Percentile {p} out of [0,1] for value {test_val}"
            );
        }
    }

    #[test]
    fn z_score_of_mean_is_zero() {
        let values = [10.0, 20.0, 30.0, 40.0, 50.0];
        let m = finstack_core::math::stats::mean(&values);
        let z = z_score(&values, m).expect("z-score of mean");
        assert!(z.abs() < 1e-10, "z-score of mean should be ~0, got {z}");
    }

    // -----------------------------------------------------------------------
    // Regression alignment / standardization / direction tests
    // -----------------------------------------------------------------------

    /// Four peers with both metrics plus one peer missing leverage but
    /// carrying a poison OAS value: misaligned x/y pairing would drag the
    /// regression toward the poison value; pairwise extraction must drop
    /// that peer entirely and reproduce the hand-computed aligned OLS.
    #[test]
    fn scoring_regression_aligns_pairs_when_peer_missing_one_metric() {
        let mut peers = vec![
            make_company("P1", "Energy", "BB", "US", 1.0, 100.0, 5_000.0),
            make_company("P2", "Energy", "BB", "US", 2.0, 200.0, 5_000.0),
            make_company("P3", "Energy", "BB", "US", 3.0, 310.0, 5_000.0),
            make_company("P4", "Energy", "BB", "US", 4.0, 380.0, 5_000.0),
            make_company("P5", "Energy", "BB", "US", 9.9, 9_999.0, 5_000.0),
        ];
        // P5 has the y metric (oas) but is missing the x metric (leverage).
        peers[4].leverage = None;

        let subject = make_company("SUBJECT", "Energy", "BB", "US", 2.5, 300.0, 5_000.0);
        let peer_set = PeerSet::new(subject, peers, PeriodBasis::Ltm);

        let dimensions = vec![ScoringDimension {
            label: "Spread vs Leverage".to_string(),
            y_extractor: MetricExtractor::Named("oas_bps".to_string()),
            x_extractors: vec![MetricExtractor::Named("leverage".to_string())],
            weight: 1.0,
            direction: ScoreDirection::default(),
        }];

        let result = score_relative_value(&peer_set, &dimensions).expect("scoring");
        let dim = &result.dimensions[0];

        // Hand-computed OLS over the aligned pairs (1,100),(2,200),(3,310),(4,380):
        // slope = 95, intercept = 10, fitted(2.5) = 247.5, residual = 52.5.
        let residual = dim.regression_residual.expect("regression should run");
        assert!(
            (residual - 52.5).abs() < 1e-9,
            "aligned OLS residual should be 52.5, got {residual}"
        );
    }

    /// The composite uses the *standardized* residual: subject residual
    /// divided by the sample std dev of peer residuals around the fitted
    /// line (here sqrt(350/3)).
    #[test]
    fn scoring_composite_is_standardized_residual() {
        let peers = vec![
            make_company("P1", "Energy", "BB", "US", 1.0, 100.0, 5_000.0),
            make_company("P2", "Energy", "BB", "US", 2.0, 200.0, 5_000.0),
            make_company("P3", "Energy", "BB", "US", 3.0, 310.0, 5_000.0),
            make_company("P4", "Energy", "BB", "US", 4.0, 380.0, 5_000.0),
        ];
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 2.5, 300.0, 5_000.0);
        let peer_set = PeerSet::new(subject, peers, PeriodBasis::Ltm);

        let dimensions = vec![ScoringDimension {
            label: "Spread vs Leverage".to_string(),
            y_extractor: MetricExtractor::Named("oas_bps".to_string()),
            x_extractors: vec![MetricExtractor::Named("leverage".to_string())],
            weight: 1.0,
            direction: ScoreDirection::default(),
        }];

        let result = score_relative_value(&peer_set, &dimensions).expect("scoring");

        // Peer residuals from the fitted line y = 10 + 95x: [-5, 0, 15, -10];
        // sample std = sqrt(350/3). Subject residual = 52.5.
        let expected = 52.5 / (350.0_f64 / 3.0).sqrt();
        assert!(
            (result.composite_score - expected).abs() < 1e-9,
            "composite should be the standardized residual {expected}, got {}",
            result.composite_score
        );
    }

    /// The direction flag must apply consistently to both scoring paths:
    /// univariate (z-score) and regression (standardized residual).
    #[test]
    fn scoring_direction_flag_flips_sign_consistently() {
        let universe = make_universe();
        let subject = make_company("SUBJECT", "Energy", "BB", "US", 4.0, 500.0, 6_500.0);

        for x_extractors in [vec![], vec![MetricExtractor::Named("leverage".to_string())]] {
            let peer_set = PeerSet::new(subject.clone(), universe.clone(), PeriodBasis::Ltm);
            let dims_cheap = vec![ScoringDimension {
                label: "dim".to_string(),
                y_extractor: MetricExtractor::Named("oas_bps".to_string()),
                x_extractors: x_extractors.clone(),
                weight: 1.0,
                direction: ScoreDirection::HigherIsCheap,
            }];
            let dims_rich = vec![ScoringDimension {
                label: "dim".to_string(),
                y_extractor: MetricExtractor::Named("oas_bps".to_string()),
                x_extractors: x_extractors.clone(),
                weight: 1.0,
                direction: ScoreDirection::HigherIsRich,
            }];

            let cheap = score_relative_value(&peer_set, &dims_cheap).expect("scoring");
            let rich = score_relative_value(&peer_set, &dims_rich).expect("scoring");

            assert!(
                cheap.composite_score != 0.0,
                "test setup should produce a non-zero signal"
            );
            assert!(
                (cheap.composite_score + rich.composite_score).abs() < 1e-12,
                "directions must negate each other: cheap={} rich={}",
                cheap.composite_score,
                rich.composite_score
            );
        }
    }

    #[test]
    fn scoring_dimension_direction_serde_default_and_names() {
        // Old payloads without `direction` deserialize to the default.
        let json = r#"{
            "label": "dim",
            "y_extractor": {"Named": "oas_bps"},
            "x_extractors": [],
            "weight": 1.0
        }"#;
        let dim: ScoringDimension = serde_json::from_str(json).expect("deserialize");
        assert_eq!(dim.direction, ScoreDirection::HigherIsCheap);

        // Stable lowercase serde names.
        assert_eq!(
            serde_json::to_string(&ScoreDirection::HigherIsCheap).expect("serialize"),
            "\"higher_is_cheap\""
        );
        assert_eq!(
            serde_json::to_string(&ScoreDirection::HigherIsRich).expect("serialize"),
            "\"higher_is_rich\""
        );
    }

    // -----------------------------------------------------------------------
    // NaN hardening and PeerStats.iqr
    // -----------------------------------------------------------------------

    #[test]
    fn peer_stats_filters_non_finite_and_reports_iqr() {
        let values = [1.0, f64::NAN, 2.0, 3.0, f64::INFINITY, 4.0, 5.0];
        let stats = peer_stats(&values).expect("stats over finite subset");

        assert_eq!(stats.count, 5);
        assert!((stats.mean - 3.0).abs() < 1e-12);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert!((stats.iqr - (stats.q3 - stats.q1)).abs() < 1e-12);
        assert!(stats.std_dev.is_finite());

        // All-NaN input has no usable observations.
        assert!(peer_stats(&[f64::NAN, f64::NAN]).is_none());
    }

    #[test]
    fn z_score_and_percentile_rank_ignore_non_finite_peers() {
        let clean = [1.0, 2.0, 3.0, 4.0, 5.0];
        let dirty = [1.0, f64::NAN, 2.0, 3.0, 4.0, f64::NEG_INFINITY, 5.0];

        assert_eq!(z_score(&clean, 4.0), z_score(&dirty, 4.0));
        assert_eq!(percentile_rank(&clean, 4.0), percentile_rank(&dirty, 4.0));

        // Non-finite subject value is undefined.
        assert_eq!(z_score(&clean, f64::NAN), None);
        assert_eq!(percentile_rank(&clean, f64::NAN), None);
    }
}
