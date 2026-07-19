//! Replay tests for portfolio.

mod replay_tests {
    use finstack_quant_attribution::AttributionMethod;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_portfolio::position::{Position, PositionUnit};
    use finstack_quant_portfolio::replay::{
        ReplayConfig, ReplayErrorPolicy, ReplayMode, ReplayTimeline,
    };
    use finstack_quant_portfolio::types::Entity;
    use finstack_quant_portfolio::Portfolio;
    use finstack_quant_portfolio::{PortfolioValuationOptions, RequestedMetrics};
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use finstack_quant_valuations::instruments::{Attributes, Instrument, PricingOptions};
    use finstack_quant_valuations::metrics::MetricId;
    use finstack_quant_valuations::pricer::InstrumentType;
    use finstack_quant_valuations::results::ValuationResult;
    use indexmap::IndexMap;
    use std::any::Any;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
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
    fn replay_metrics_attribution_preserves_only_valuation_metrics() {
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
            attribution_method: AttributionMethod::MetricsBased,
            valuation_options: PortfolioValuationOptions {
                strict_risk: false,
                metrics: RequestedMetrics::Only(Vec::new()),
            },
            on_error: Default::default(),
        };

        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &timeline,
            &config,
            &FinstackConfig::default(),
        )
        .unwrap();

        assert!(result.steps[1].attribution.is_some());
        for step in &result.steps {
            let valuation = step.valuation.position_values["POS_001"]
                .valuation_result
                .as_ref()
                .expect("phase-A valuation should contain a valuation result");
            assert!(
                valuation.measures.is_empty(),
                "the replay valuation must retain the caller's exact Only metrics"
            );
        }
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

    #[derive(Clone)]
    struct ReplayProbeInstrument {
        id: String,
        attributes: Attributes,
        base_calls: Arc<AtomicUsize>,
        pv_only_calls: Arc<AtomicUsize>,
        metric_calls: Arc<AtomicUsize>,
        fail_base_dates: Arc<Vec<Date>>,
        fail_metrics: bool,
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        ReplayProbeInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl ReplayProbeInstrument {
        fn value_for_date(as_of: Date) -> Money {
            Money::new(f64::from(as_of.day()) * 100.0, Currency::USD)
        }

        fn check_snapshot(&self, as_of: Date) -> finstack_quant_core::Result<()> {
            if self.fail_base_dates.contains(&as_of) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "configured replay snapshot failure on {as_of}"
                )));
            }
            Ok(())
        }
    }

    impl Instrument for ReplayProbeInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Basket
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            self.base_calls.fetch_add(1, Ordering::SeqCst);
            self.check_snapshot(as_of)?;
            Ok(Self::value_for_date(as_of))
        }

        fn price_with_metrics(
            &self,
            _market: &MarketContext,
            as_of: Date,
            metrics: &[MetricId],
            options: PricingOptions,
        ) -> finstack_quant_core::Result<ValuationResult> {
            if metrics.is_empty() {
                self.pv_only_calls.fetch_add(1, Ordering::SeqCst);
            } else {
                self.metric_calls.fetch_add(1, Ordering::SeqCst);
            }
            self.check_snapshot(as_of)?;
            if self.fail_metrics && !metrics.is_empty() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "configured replay metric failure on {as_of}"
                )));
            }
            let config = options.config.as_deref().ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "replay test expected the executor request config".to_string(),
                )
            })?;
            let measures: IndexMap<MetricId, f64> = metrics
                .iter()
                .cloned()
                .map(|metric| (metric, 0.0))
                .collect();
            Ok(ValuationResult::stamped_with_config(
                self.id(),
                as_of,
                Self::value_for_date(as_of),
                config,
            )
            .with_measures(measures))
        }
    }

    fn build_replay_probe_portfolio(
        fail_base_dates: Vec<Date>,
        fail_metrics: bool,
    ) -> (
        Portfolio,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
    ) {
        let base_calls = Arc::new(AtomicUsize::new(0));
        let pv_only_calls = Arc::new(AtomicUsize::new(0));
        let metric_calls = Arc::new(AtomicUsize::new(0));
        let instrument: Arc<dyn Instrument> = Arc::new(ReplayProbeInstrument {
            id: "REPLAY_PROBE".to_string(),
            attributes: Attributes::new(),
            base_calls: Arc::clone(&base_calls),
            pv_only_calls: Arc::clone(&pv_only_calls),
            metric_calls: Arc::clone(&metric_calls),
            fail_base_dates: Arc::new(fail_base_dates),
            fail_metrics,
        });
        let portfolio = Portfolio::builder("REPLAY_PROBE_PORTFOLIO")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .entity(Entity::new("ENTITY_A"))
            .position(
                Position::new(
                    "REPLAY_PROBE_POSITION",
                    "ENTITY_A",
                    "REPLAY_PROBE",
                    instrument,
                    1.0,
                    PositionUnit::Units,
                )
                .expect("valid replay-probe position"),
            )
            .build()
            .expect("valid replay-probe portfolio");
        (portfolio, base_calls, pv_only_calls, metric_calls)
    }

    fn empty_timeline(snapshot_count: usize) -> ReplayTimeline {
        ReplayTimeline::new(
            (0..snapshot_count)
                .map(|index| {
                    (
                        date!(2024 - 01 - 01) + time::Duration::days(index as i64),
                        MarketContext::new(),
                    )
                })
                .collect(),
        )
        .expect("ordered replay-probe timeline")
    }

    fn pv_only_replay_config(mode: ReplayMode, on_error: ReplayErrorPolicy) -> ReplayConfig {
        ReplayConfig {
            mode,
            attribution_method: AttributionMethod::MetricsBased,
            valuation_options: PortfolioValuationOptions {
                strict_risk: true,
                metrics: RequestedMetrics::Only(Vec::new()),
            },
            on_error,
        }
    }

    #[test]
    fn replay_best_effort_uses_the_previous_surviving_snapshot() {
        let failed_date = date!(2024 - 01 - 02);
        let (portfolio, _, _, _) = build_replay_probe_portfolio(vec![failed_date], false);
        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &empty_timeline(3),
            &pv_only_replay_config(ReplayMode::PvAndPnl, ReplayErrorPolicy::BestEffort),
            &FinstackConfig::default(),
        )
        .expect("best-effort replay");

        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].date, date!(2024 - 01 - 01));
        assert_eq!(result.steps[1].date, date!(2024 - 01 - 03));
        assert_eq!(result.skipped_dates.len(), 1);
        assert_eq!(result.skipped_dates[0].0, failed_date);
        assert_eq!(
            result.steps[1]
                .daily_pnl
                .expect("surviving step daily P&L")
                .amount(),
            200.0,
            "P&L must bridge from Jan 1 directly to the next surviving snapshot"
        );
    }

    #[test]
    fn replay_state_parallel_threshold_matches_the_serial_prefix() {
        let (portfolio, _, _, _) = build_replay_probe_portfolio(Vec::new(), false);
        let config = pv_only_replay_config(ReplayMode::PvAndPnl, ReplayErrorPolicy::Strict);
        let seven = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &empty_timeline(7),
            &config,
            &FinstackConfig::default(),
        )
        .expect("seven-state serial replay");
        let eight = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &empty_timeline(8),
            &config,
            &FinstackConfig::default(),
        )
        .expect("eight-state parallel replay");

        for (serial_step, parallel_step) in seven.steps.iter().zip(&eight.steps) {
            assert_eq!(parallel_step.date, serial_step.date);
            assert_eq!(
                parallel_step.valuation.total_base_ccy,
                serial_step.valuation.total_base_ccy
            );
            assert_eq!(parallel_step.daily_pnl, serial_step.daily_pnl);
            assert_eq!(parallel_step.cumulative_pnl, serial_step.cumulative_pnl);
        }
    }

    #[test]
    fn replay_metrics_attribution_reuses_complete_phase_a_endpoints() {
        let (portfolio, base_calls, pv_only_calls, metric_calls) =
            build_replay_probe_portfolio(Vec::new(), false);
        let config = ReplayConfig {
            mode: ReplayMode::FullAttribution,
            attribution_method: AttributionMethod::MetricsBased,
            valuation_options: PortfolioValuationOptions::default(),
            on_error: ReplayErrorPolicy::Strict,
        };
        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &empty_timeline(3),
            &config,
            &FinstackConfig::default(),
        )
        .expect("metrics-attribution replay");

        assert_eq!(result.steps.len(), 3);
        assert_eq!(
            metric_calls.load(Ordering::SeqCst),
            3,
            "complete StandardPlus Phase-A metrics must satisfy attribution without repricing"
        );
        assert_eq!(pv_only_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            base_calls.load(Ordering::SeqCst),
            0,
            "exact metric endpoints must not fall back to a scalar PV pass"
        );
    }

    #[test]
    fn replay_explicit_only_uses_bounded_strict_batches_without_duplicate_endpoints() {
        let (portfolio, base_calls, pv_only_calls, metric_calls) =
            build_replay_probe_portfolio(Vec::new(), false);
        let snapshot_count = 20;
        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &empty_timeline(snapshot_count),
            &pv_only_replay_config(ReplayMode::FullAttribution, ReplayErrorPolicy::Strict),
            &FinstackConfig::default(),
        )
        .expect("explicit-Only metrics attribution replay");

        assert_eq!(
            pv_only_calls.load(Ordering::SeqCst),
            snapshot_count,
            "Phase A must retain the caller's exact PV-only profile"
        );
        assert_eq!(base_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            metric_calls.load(Ordering::SeqCst),
            snapshot_count,
            "bounded strict batches must not reprice overlapping endpoints"
        );
        for step in result.steps {
            assert!(step.valuation.position_values["REPLAY_PROBE_POSITION"]
                .valuation_result
                .as_ref()
                .expect("phase-A result")
                .measures
                .is_empty());
        }
    }

    #[test]
    fn replay_degraded_metrics_endpoint_is_strictly_retried_and_propagated() {
        let (portfolio, base_calls, pv_only_calls, metric_calls) =
            build_replay_probe_portfolio(Vec::new(), true);
        let config = ReplayConfig {
            mode: ReplayMode::FullAttribution,
            attribution_method: AttributionMethod::MetricsBased,
            valuation_options: PortfolioValuationOptions::default(),
            on_error: ReplayErrorPolicy::BestEffort,
        };
        let error = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &empty_timeline(2),
            &config,
            &FinstackConfig::default(),
        )
        .expect_err("strict attribution retry must surface its metric failure");

        assert!(error
            .to_string()
            .contains("configured replay metric failure on 2024-01-01"));
        assert_eq!(base_calls.load(Ordering::SeqCst), 2);
        assert_eq!(pv_only_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            metric_calls.load(Ordering::SeqCst),
            4,
            "two degraded Phase-A calls and two strict endpoint retries are expected"
        );
    }

    #[test]
    fn replay_state_parallel_failures_are_reported_in_timeline_order() {
        let (portfolio, _, _, _) =
            build_replay_probe_portfolio(vec![date!(2024 - 01 - 02), date!(2024 - 01 - 06)], false);
        let config = pv_only_replay_config(ReplayMode::PvOnly, ReplayErrorPolicy::Strict);

        for _ in 0..8 {
            let error = finstack_quant_portfolio::replay::replay_portfolio(
                &portfolio,
                &empty_timeline(8),
                &config,
                &FinstackConfig::default(),
            )
            .expect_err("configured replay must fail");
            let message = error.to_string();
            assert!(message.contains("2024-01-02"), "{message}");
            assert!(!message.contains("2024-01-06"), "{message}");
        }
    }

    fn assert_same_portfolio_decomposition(
        actual: &finstack_quant_portfolio::attribution::PortfolioAttribution,
        expected: &finstack_quant_portfolio::attribution::PortfolioAttribution,
    ) {
        assert_eq!(actual.total_pnl, expected.total_pnl);
        assert_eq!(actual.carry, expected.carry);
        assert_eq!(actual.rates_curves_pnl, expected.rates_curves_pnl);
        assert_eq!(actual.credit_curves_pnl, expected.credit_curves_pnl);
        assert_eq!(actual.inflation_curves_pnl, expected.inflation_curves_pnl);
        assert_eq!(actual.correlations_pnl, expected.correlations_pnl);
        assert_eq!(actual.fx_pnl, expected.fx_pnl);
        assert_eq!(actual.fx_translation_pnl, expected.fx_translation_pnl);
        assert_eq!(actual.cross_factor_pnl, expected.cross_factor_pnl);
        assert_eq!(actual.vol_pnl, expected.vol_pnl);
        assert_eq!(actual.model_params_pnl, expected.model_params_pnl);
        assert_eq!(actual.market_scalars_pnl, expected.market_scalars_pnl);
        assert_eq!(actual.residual, expected.residual);
        assert_eq!(actual.result_invalid, expected.result_invalid);
    }

    #[test]
    fn replay_repricing_methods_reuse_each_compatible_endpoint_once() {
        let methods = [
            AttributionMethod::Parallel,
            AttributionMethod::Waterfall(finstack_quant_attribution::default_waterfall_order()),
            AttributionMethod::Taylor(
                finstack_quant_attribution::TaylorAttributionConfig::default(),
            ),
        ];

        for method in methods {
            let (portfolio, _, pv_only_calls, _) = build_replay_probe_portfolio(Vec::new(), false);
            let timeline = empty_timeline(3);
            let config = ReplayConfig {
                mode: ReplayMode::FullAttribution,
                attribution_method: method.clone(),
                valuation_options: PortfolioValuationOptions {
                    strict_risk: false,
                    metrics: RequestedMetrics::Only(Vec::new()),
                },
                on_error: ReplayErrorPolicy::Strict,
            };
            let finstack_config = FinstackConfig::default();
            let result = finstack_quant_portfolio::replay::replay_portfolio(
                &portfolio,
                &timeline,
                &config,
                &finstack_config,
            )
            .expect("repricing-attribution replay");

            assert_eq!(
                pv_only_calls.load(Ordering::SeqCst),
                timeline.len(),
                "{method} must reuse Phase-A endpoints across adjacent intervals"
            );

            let snapshots: Vec<_> = timeline.iter().collect();
            for index in 1..snapshots.len() {
                let (as_of_t0, market_t0) = snapshots[index - 1];
                let (as_of_t1, market_t1) = snapshots[index];
                let expected = finstack_quant_portfolio::attribution::attribute_portfolio_pnl(
                    &portfolio,
                    market_t0,
                    market_t1,
                    *as_of_t0,
                    *as_of_t1,
                    &finstack_config,
                    method.clone(),
                )
                .expect("standalone attribution");
                let actual = result.steps[index]
                    .attribution
                    .as_ref()
                    .expect("replay attribution");
                assert_same_portfolio_decomposition(actual, &expected);
            }
        }
    }

    #[test]
    fn replay_repricing_method_prepares_incompatible_endpoints_once_per_state() {
        let snapshot_count = 20;
        let (portfolio, _, pv_only_calls, metric_calls) =
            build_replay_probe_portfolio(Vec::new(), false);
        let config = ReplayConfig {
            mode: ReplayMode::FullAttribution,
            attribution_method: AttributionMethod::Parallel,
            valuation_options: PortfolioValuationOptions::default(),
            on_error: ReplayErrorPolicy::Strict,
        };

        finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &empty_timeline(snapshot_count),
            &config,
            &FinstackConfig::default(),
        )
        .expect("repricing-attribution replay");

        assert_eq!(
            pv_only_calls.load(Ordering::SeqCst),
            snapshot_count,
            "bounded endpoint batches must prepare each state once, not once per interval"
        );
        assert!(
            metric_calls.load(Ordering::SeqCst) >= snapshot_count,
            "Phase A must retain the caller's Standard metric profile"
        );
    }
}
