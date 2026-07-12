//! Integration tests for Cash Flow Waterfall & Sweep Mechanics

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{EcfSweepSpec, WaterfallSpec};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::AmountOrScalar;
use time::Month;

#[test]
fn test_ecf_sweep_basic() {
    // Create a simple model with a term loan and ECF sweep
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let model = ModelBuilder::new("ecf_test")
        .periods("2025Q1..2025Q2", None)
        .expect("valid periods")
        .value(
            "ebitda",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        )
        .value(
            "taxes",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(220_000.0),
                ),
            ],
        )
        .value(
            "capex",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(100_000.0),
                ),
            ],
        )
        .add_bond(
            "BOND-001",
            Money::new(10_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("valid bond")
        .waterfall(WaterfallSpec {
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".to_string(),
                taxes_node: Some("taxes".to_string()),
                capex_node: Some("capex".to_string()),
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 0.5,      // 50% sweep
                target_instrument_id: None, // Apply to all
            }),
            ..WaterfallSpec::default()
        })
        .build()
        .expect("model should build");

    // Create market context
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .expect("curve should build");
    let market_ctx = MarketContext::new().insert(disc_curve);

    // Evaluate model
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate_with_market(&model, &market_ctx, issue)
        .expect("evaluation should succeed");

    // Verify that EBITDA values are present
    assert!(results.get("ebitda", &PeriodId::quarter(2025, 1)).is_some());
    assert_eq!(
        results.get("ebitda", &PeriodId::quarter(2025, 1)),
        Some(1_000_000.0)
    );
}

// ============================================================================
// calculate_period_flows + execute_waterfall integration
//
// The in-module waterfall tests use synthetic flows that bypass
// `calculate_period_flows`; the tests below exercise the full per-period
// pipeline (contractual flow extraction → waterfall → state advance).
// ============================================================================

mod period_flow_waterfall_integration {
    use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    use finstack_quant_cashflows::primitives::CFKind;
    use finstack_quant_cashflows::CashflowProvider;
    use finstack_quant_core::cashflow::CashFlow;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Period, PeriodId};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_statements::capital_structure::{
        calculate_period_flows, execute_waterfall, CapitalStructureState, PaymentPriority,
        PikToggleSpec, WaterfallSpec,
    };
    use finstack_quant_statements::evaluator::EvaluationContext;
    use finstack_quant_statements::types::NodeId;
    use indexmap::IndexMap;
    use std::sync::Arc;
    use time::Month;

    struct ScheduleInstrument {
        schedule: CashFlowSchedule,
    }

    impl CashflowProvider for ScheduleInstrument {
        fn cashflow_schedule(
            &self,
            _curves: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            Ok(self.schedule.clone())
        }
    }

    fn quarter_period(year: i32, q: u8) -> Period {
        let month = |q: u8| match q {
            1 => Month::January,
            2 => Month::April,
            3 => Month::July,
            _ => Month::October,
        };
        let start = Date::from_calendar_date(year, month(q), 1).expect("valid date");
        let end = if q == 4 {
            Date::from_calendar_date(year + 1, Month::January, 1).expect("valid date")
        } else {
            Date::from_calendar_date(year, month(q + 1), 1).expect("valid date")
        };
        Period {
            id: PeriodId::quarter(year, q),
            start,
            end,
            is_actual: false,
        }
    }

    fn context_with(period: PeriodId, values: &[(&str, f64)]) -> EvaluationContext {
        let mut node_to_column = IndexMap::new();
        for (idx, (name, _)) in values.iter().enumerate() {
            node_to_column.insert(NodeId::new(*name), idx);
        }
        let mut ctx =
            EvaluationContext::new(period, Arc::new(node_to_column), Arc::new(IndexMap::new()));
        for (name, value) in values {
            ctx.set_value(name, *value).expect("context accepts value");
        }
        ctx
    }

    /// PIK compounding regression (review: SCALE_CLAMP_MAX froze PIK after
    /// ~5 quarters): a toggled-PIK loan at 2%/quarter must compound for
    /// 8 quarters. The schedule's coupon stays at 2% of the original
    /// notional; the stateful balance compounds, and the toggled-PIK
    /// exclusion keeps the scale clamp from freezing interest at 1.10×.
    #[test]
    fn toggled_pik_interest_compounds_across_eight_quarters() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let notional = 1_000_000.0;
        let rate_q = 0.02;

        // 8 quarterly coupons of 2% of the (non-compounding) scheduled
        // notional, mid-quarter so they fall inside each period.
        let mut flows = Vec::new();
        for i in 0..8u8 {
            let year = 2025 + i32::from(i / 4);
            let month = match i % 4 {
                0 => Month::February,
                1 => Month::May,
                2 => Month::August,
                _ => Month::November,
            };
            flows.push(CashFlow {
                date: Date::from_calendar_date(year, month, 15).expect("valid date"),
                reset_date: None,
                amount: Money::new(-notional * rate_q, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: Some(rate_q * 4.0),
            });
        }
        let instrument = ScheduleInstrument {
            schedule: CashFlowSchedule {
                flows,
                notional: Notional::par(notional, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(issue),
                    ..CashFlowMeta::default()
                },
            },
        };

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                target_instrument_ids: Some(vec!["TL-PIK".into()]),
                min_periods_in_pik: 0,
            }),
        };

        let market_ctx = MarketContext::new();
        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-PIK".to_string(), Money::new(notional, Currency::USD));

        let mut last_pik = 0.0;
        for i in 0..8u8 {
            let year = 2025 + i32::from(i / 4);
            let q = (i % 4) + 1;
            let period = quarter_period(year, q);

            let opening = state
                .opening_balances
                .get("TL-PIK")
                .copied()
                .expect("opening balance");
            let toggled_pik = state
                .cumulative_toggled_pik
                .get("TL-PIK")
                .copied()
                .unwrap_or_else(|| Money::new(0.0, Currency::USD));

            let (breakdown, _, _, warnings) = calculate_period_flows(
                &instrument,
                &period,
                opening,
                toggled_pik,
                &market_ctx,
                issue,
            )
            .expect("period flows");
            assert!(
                warnings.is_empty(),
                "PIK compounding must not trigger the scale clamp in quarter {i}, got {warnings:?}"
            );

            let mut contractual: IndexMap<
                String,
                finstack_quant_statements::capital_structure::CashflowBreakdown,
            > = IndexMap::new();
            contractual.insert("TL-PIK".to_string(), breakdown);

            // Liquidity below the threshold keeps PIK active every period.
            let ctx = context_with(period.id, &[("liquidity", 10.0)]);
            let result = execute_waterfall(&period.id, &ctx, &waterfall, &mut state, &contractual)
                .expect("waterfall");
            last_pik = result.flows["TL-PIK"].interest_expense_pik.amount();

            state.advance_period();
        }

        // Quarter 8 PIK coupon = 2% of the balance compounded for 7 quarters.
        let expected_q8 = notional * rate_q * (1.0 + rate_q).powi(7);
        assert!(
            (last_pik - expected_q8).abs() < 1e-6,
            "Q8 PIK interest should compound to {expected_q8}, got {last_pik}"
        );
        // The old clamp froze interest at 1.10 × the original coupon (22,000).
        assert!(
            last_pik > notional * rate_q * 1.10 + 1e-9,
            "PIK interest must compound beyond the old 1.10 clamp ceiling, got {last_pik}"
        );

        let closing = state
            .opening_balances
            .get("TL-PIK")
            .expect("balance after 8 quarters")
            .amount();
        let expected_balance = notional * (1.0 + rate_q).powi(8);
        assert!(
            (closing - expected_balance).abs() < 1e-6,
            "balance should compound to {expected_balance}, got {closing}"
        );
    }

    /// Conservation through the full pipeline: an amortizing loan's fees +
    /// cash interest + principal + equity must equal available cash.
    #[test]
    fn waterfall_with_contractual_flows_conserves_available_cash() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let period = quarter_period(2025, 1);

        let instrument = ScheduleInstrument {
            schedule: CashFlowSchedule {
                flows: vec![
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::February, 15)
                            .expect("valid date"),
                        reset_date: None,
                        amount: Money::new(-10_000.0, Currency::USD),
                        kind: CFKind::Fixed,
                        accrual_factor: 0.25,
                        rate: Some(0.04),
                    },
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
                        reset_date: None,
                        amount: Money::new(50_000.0, Currency::USD),
                        kind: CFKind::Amortization,
                        accrual_factor: 0.0,
                        rate: None,
                    },
                ],
                notional: Notional::par(1_000_000.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(issue),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(1_000_000.0, Currency::USD));

        let opening = state.opening_balances["TL-1"];
        let (breakdown, _, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            opening,
            Money::new(0.0, Currency::USD),
            &market_ctx,
            issue,
        )
        .expect("period flows");
        assert!(warnings.is_empty());

        let mut contractual: IndexMap<
            String,
            finstack_quant_statements::capital_structure::CashflowBreakdown,
        > = IndexMap::new();
        contractual.insert("TL-1".to_string(), breakdown);

        let waterfall = WaterfallSpec {
            available_cash_node: Some("cash_available".into()),
            ..WaterfallSpec::default()
        };

        let available = 100_000.0;
        let ctx = context_with(period.id, &[("cash_available", available)]);
        let result = execute_waterfall(&period.id, &ctx, &waterfall, &mut state, &contractual)
            .expect("waterfall");

        let tl = &result.flows["TL-1"];
        let equity = result
            .equity_distribution
            .expect("equity populated")
            .amount();
        let conserved = tl.fees.amount()
            + tl.interest_expense_cash.amount()
            + tl.principal_payment.amount()
            + equity;
        assert!(
            (conserved - available).abs() < 1e-9,
            "fees + interest + principal + equity ({conserved}) must equal available cash ({available})"
        );
        assert!((tl.interest_expense_cash.amount() - 10_000.0).abs() < 1e-9);
        assert!((tl.principal_payment.amount() - 50_000.0).abs() < 1e-9);
        assert!((equity - 40_000.0).abs() < 1e-9);
    }

    /// Forward-dated instruments must report a zero balance before issuance
    /// (review: pre-issue periods fell back to the first *future* outstanding
    /// entry, i.e. the full notional).
    #[test]
    fn forward_dated_instrument_reports_zero_balance_before_issuance() {
        use finstack_quant_statements::capital_structure::aggregate_instrument_cashflows;
        use finstack_quant_statements::types::CapitalStructureSpec;

        let issue = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Instrument issued mid-horizon (Q3): funding draw on Jul 1, one
        // coupon in Q4.
        let instrument: Arc<dyn CashflowProvider + Send + Sync> = Arc::new(ScheduleInstrument {
            schedule: CashFlowSchedule {
                flows: vec![
                    CashFlow {
                        date: issue,
                        reset_date: None,
                        amount: Money::new(-1_000_000.0, Currency::USD),
                        kind: CFKind::Notional,
                        accrual_factor: 0.0,
                        rate: None,
                    },
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::November, 15)
                            .expect("valid date"),
                        reset_date: None,
                        amount: Money::new(-20_000.0, Currency::USD),
                        kind: CFKind::Fixed,
                        accrual_factor: 0.25,
                        rate: Some(0.08),
                    },
                ],
                notional: Notional::par(1_000_000.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(issue),
                    ..CashFlowMeta::default()
                },
            },
        });

        let mut instruments: IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>> =
            IndexMap::new();
        instruments.insert("DDTL-1".to_string(), instrument);

        let periods: Vec<Period> = (1..=4).map(|q| quarter_period(2025, q)).collect();
        let spec = CapitalStructureSpec {
            debt_instruments: vec![],
            equity_instruments: vec![],
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: None,
        };

        let cashflows = aggregate_instrument_cashflows(
            &spec,
            &instruments,
            &periods,
            &MarketContext::new(),
            as_of,
        )
        .expect("aggregation");

        let q1 = PeriodId::quarter(2025, 1);
        let q2 = PeriodId::quarter(2025, 2);
        let q3 = PeriodId::quarter(2025, 3);
        for pre in [q1, q2] {
            assert_eq!(
                cashflows
                    .get_debt_balance("DDTL-1", &pre)
                    .expect("balance present"),
                0.0,
                "pre-issuance period {pre} must report zero debt balance"
            );
        }
        assert_eq!(
            cashflows
                .get_debt_balance("DDTL-1", &q3)
                .expect("balance present"),
            1_000_000.0,
            "post-issuance balance must equal the funded notional"
        );
    }
}
