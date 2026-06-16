//! Replay tests for portfolio.

mod replay_tests {
    use finstack_quant_attribution::AttributionMethod;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_portfolio::position::{Position, PositionUnit};
    use finstack_quant_portfolio::replay::{ReplayConfig, ReplayMode, ReplayTimeline};
    use finstack_quant_portfolio::types::Entity;
    use finstack_quant_portfolio::Portfolio;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    fn empty_market() -> MarketContext {
        MarketContext::new()
    }

    #[test]
    fn timeline_rejects_empty() {
        let result = ReplayTimeline::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn timeline_accepts_single_snapshot() {
        let result = ReplayTimeline::new(vec![(date!(2024 - 01 - 01), empty_market())]);
        assert!(result.is_ok());
        let tl = result.unwrap();
        assert_eq!(tl.len(), 1);
        assert!(!tl.is_empty());
        let (start, end) = tl.date_range();
        assert_eq!(start, date!(2024 - 01 - 01));
        assert_eq!(end, date!(2024 - 01 - 01));
    }

    #[test]
    fn timeline_accepts_sorted_dates() {
        let result = ReplayTimeline::new(vec![
            (date!(2024 - 01 - 01), empty_market()),
            (date!(2024 - 01 - 02), empty_market()),
            (date!(2024 - 01 - 03), empty_market()),
        ]);
        assert!(result.is_ok());
        let tl = result.unwrap();
        assert_eq!(tl.len(), 3);
        let (start, end) = tl.date_range();
        assert_eq!(start, date!(2024 - 01 - 01));
        assert_eq!(end, date!(2024 - 01 - 03));
    }

    #[test]
    fn timeline_rejects_unsorted_dates() {
        let result = ReplayTimeline::new(vec![
            (date!(2024 - 01 - 02), empty_market()),
            (date!(2024 - 01 - 01), empty_market()),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn timeline_rejects_duplicate_dates() {
        let result = ReplayTimeline::new(vec![
            (date!(2024 - 01 - 01), empty_market()),
            (date!(2024 - 01 - 01), empty_market()),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn timeline_iter_yields_all_snapshots() {
        let tl = ReplayTimeline::new(vec![
            (date!(2024 - 01 - 01), empty_market()),
            (date!(2024 - 01 - 02), empty_market()),
        ])
        .unwrap();
        let dates: Vec<_> = tl.iter().map(|(d, _)| *d).collect();
        assert_eq!(dates, vec![date!(2024 - 01 - 01), date!(2024 - 01 - 02)]);
    }

    fn build_test_portfolio() -> Portfolio {
        let as_of = date!(2024 - 01 - 01);
        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(DayCount::Act360)
            .quote_rate_opt(Some(rust_decimal::Decimal::try_from(0.045).unwrap()))
            .discount_curve_id("USD".into())
            .build()
            .unwrap();

        let position = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1M",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .unwrap();

        Portfolio::builder("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .unwrap()
    }

    fn market_at_rate(as_of: time::Date, rate_bp: f64) -> MarketContext {
        let rate = rate_bp / 10_000.0;
        let curve = DiscountCurve::builder("USD")
            .base_date(as_of)
            .knots(vec![
                (0.0, 1.0),
                (1.0, (-rate * 1.0_f64).exp()),
                (5.0, (-rate * 5.0_f64).exp()),
            ])
            .interp(InterpStyle::Linear)
            .validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            )
            .build()
            .unwrap();
        MarketContext::new().insert(curve)
    }

    #[test]
    fn replay_pv_only_produces_steps_for_each_date() {
        let portfolio = build_test_portfolio();
        let timeline = ReplayTimeline::new(vec![
            (
                date!(2024 - 01 - 01),
                market_at_rate(date!(2024 - 01 - 01), 0.0),
            ),
            (
                date!(2024 - 01 - 02),
                market_at_rate(date!(2024 - 01 - 02), 50.0),
            ),
            (
                date!(2024 - 01 - 03),
                market_at_rate(date!(2024 - 01 - 03), 100.0),
            ),
        ])
        .unwrap();

        let config = ReplayConfig {
            mode: ReplayMode::PvOnly,
            attribution_method: Default::default(),
            valuation_options: Default::default(),
            on_error: Default::default(),
        };

        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &timeline,
            &config,
            &FinstackConfig::default(),
        )
        .unwrap();

        assert_eq!(result.steps.len(), 3);

        // Step 0 has no P&L
        assert!(result.steps[0].daily_pnl.is_none());
        assert!(result.steps[0].cumulative_pnl.is_none());
        assert!(result.steps[0].attribution.is_none());

        // All steps in PvOnly have no P&L fields
        for step in &result.steps {
            assert!(step.daily_pnl.is_none());
            assert!(step.cumulative_pnl.is_none());
            assert!(step.attribution.is_none());
        }

        // Dates match timeline
        assert_eq!(result.steps[0].date, date!(2024 - 01 - 01));
        assert_eq!(result.steps[1].date, date!(2024 - 01 - 02));
        assert_eq!(result.steps[2].date, date!(2024 - 01 - 03));
        assert_eq!(
            result.steps[1].valuation.as_of,
            date!(2024 - 01 - 02),
            "M-14: replay must value each snapshot at its snapshot date"
        );
        assert_eq!(
            result.steps[2].valuation.as_of,
            date!(2024 - 01 - 03),
            "M-14: replay must value each snapshot at its snapshot date"
        );

        // Summary
        assert_eq!(result.summary.num_steps, 3);
        assert_eq!(result.summary.start_date, date!(2024 - 01 - 01));
        assert_eq!(result.summary.end_date, date!(2024 - 01 - 03));
    }

    #[test]
    fn replay_pv_and_pnl_computes_daily_and_cumulative() {
        let portfolio = build_test_portfolio();
        let timeline = ReplayTimeline::new(vec![
            (
                date!(2024 - 01 - 01),
                market_at_rate(date!(2024 - 01 - 01), 0.0),
            ),
            (
                date!(2024 - 01 - 02),
                market_at_rate(date!(2024 - 01 - 02), 50.0),
            ),
            (
                date!(2024 - 01 - 03),
                market_at_rate(date!(2024 - 01 - 03), 100.0),
            ),
        ])
        .unwrap();

        let config = ReplayConfig {
            mode: ReplayMode::PvAndPnl,
            attribution_method: Default::default(),
            valuation_options: Default::default(),
            on_error: Default::default(),
        };

        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &timeline,
            &config,
            &FinstackConfig::default(),
        )
        .unwrap();

        // Step 0: no P&L
        assert!(result.steps[0].daily_pnl.is_none());
        assert!(result.steps[0].cumulative_pnl.is_none());

        // Steps 1+: has P&L, no attribution
        for step in &result.steps[1..] {
            assert!(step.daily_pnl.is_some());
            assert!(step.cumulative_pnl.is_some());
            assert!(step.attribution.is_none());
        }

        // Cumulative at last step equals total_pnl in summary
        let last_cum = result.steps.last().unwrap().cumulative_pnl.unwrap();
        let diff = (last_cum.amount() - result.summary.total_pnl.amount()).abs();
        assert!(diff < 1e-6, "cumulative P&L should match summary total_pnl");
    }

    #[test]
    fn replay_full_attribution_produces_attribution_at_each_step() {
        let portfolio = build_test_portfolio();
        let timeline = ReplayTimeline::new(vec![
            (
                date!(2024 - 01 - 01),
                market_at_rate(date!(2024 - 01 - 01), 450.0),
            ),
            (
                date!(2024 - 01 - 02),
                market_at_rate(date!(2024 - 01 - 02), 460.0),
            ),
        ])
        .unwrap();

        let config = ReplayConfig {
            mode: ReplayMode::FullAttribution,
            attribution_method: AttributionMethod::Parallel,
            valuation_options: Default::default(),
            on_error: Default::default(),
        };

        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &timeline,
            &config,
            &FinstackConfig::default(),
        )
        .unwrap();

        // Step 0: no attribution
        assert!(result.steps[0].attribution.is_none());

        // Step 1: has attribution with factor breakdown
        let attr = result.steps[1]
            .attribution
            .as_ref()
            .expect("step 1 should have attribution");
        assert!(
            !attr.by_position.is_empty(),
            "should have per-position breakdown"
        );

        // Also has P&L in FullAttribution mode
        assert!(result.steps[1].daily_pnl.is_some());
        assert!(result.steps[1].cumulative_pnl.is_some());
    }

    #[test]
    fn replay_summary_tracks_max_drawdown() {
        let portfolio = build_test_portfolio();
        // Rates: 0bp -> 200bp (value drops) -> 100bp (partial recovery)
        let timeline = ReplayTimeline::new(vec![
            (
                date!(2024 - 01 - 01),
                market_at_rate(date!(2024 - 01 - 01), 0.0),
            ),
            (
                date!(2024 - 01 - 02),
                market_at_rate(date!(2024 - 01 - 02), 200.0),
            ),
            (
                date!(2024 - 01 - 03),
                market_at_rate(date!(2024 - 01 - 03), 100.0),
            ),
        ])
        .unwrap();

        let config = ReplayConfig {
            mode: ReplayMode::PvAndPnl,
            attribution_method: Default::default(),
            valuation_options: Default::default(),
            on_error: Default::default(),
        };

        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &timeline,
            &config,
            &FinstackConfig::default(),
        )
        .unwrap();

        // Max drawdown should be positive (a loss amount)
        assert!(result.summary.max_drawdown.amount() >= 0.0);
        // Peak should be at step 0 (rates started at 0)
        assert_eq!(result.summary.max_drawdown_peak_date, date!(2024 - 01 - 01));
        // Trough should be at step 1 (highest rates)
        assert_eq!(
            result.summary.max_drawdown_trough_date,
            date!(2024 - 01 - 02)
        );
    }

    #[test]
    fn replay_config_roundtrips_via_json() {
        let config = ReplayConfig {
            mode: ReplayMode::FullAttribution,
            attribution_method: AttributionMethod::Parallel,
            valuation_options: Default::default(),
            on_error: Default::default(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ReplayConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized.mode, ReplayMode::FullAttribution));
    }

    #[test]
    fn replay_result_serializes_to_json() {
        let portfolio = build_test_portfolio();
        let timeline = ReplayTimeline::new(vec![
            (
                date!(2024 - 01 - 01),
                market_at_rate(date!(2024 - 01 - 01), 0.0),
            ),
            (
                date!(2024 - 01 - 02),
                market_at_rate(date!(2024 - 01 - 02), 50.0),
            ),
        ])
        .unwrap();

        let config = ReplayConfig {
            mode: ReplayMode::PvAndPnl,
            attribution_method: Default::default(),
            valuation_options: Default::default(),
            on_error: Default::default(),
        };

        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &timeline,
            &config,
            &FinstackConfig::default(),
        )
        .unwrap();

        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.is_empty());

        // Deserialize back
        let deserialized: finstack_quant_portfolio::replay::ReplayResult =
            serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.steps.len(), 2);
        assert_eq!(deserialized.summary.num_steps, 2);
    }
}
