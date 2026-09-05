//! Integration tests for Cash Flow Waterfall & Sweep Mechanics

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, PeriodId, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{EcfSweepSpec, WaterfallSpec};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, FinancialStatementInstrument};
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
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
            "cash",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000_000.0),
                ),
            ],
        )
        .value(
            "ebitda",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        )
        .value(
            "taxes",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(200_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(220_000.0),
                ),
            ],
        )
        .value(
            "capex",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
            ],
        )
        .add_debt(
            "TL-001",
            FinancialStatementInstrument::TermLoan(
                TermLoan::builder()
                    .id("TL-001".into())
                    .currency(Currency::USD)
                    .notional_limit(
                        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
                    )
                    .issue_date(issue)
                    .maturity(maturity)
                    .rate(RateSpec::Fixed { rate_bp: 500 })
                    .frequency(Tenor::quarterly())
                    .day_count(DayCount::Act360)
                    .business_day_convention(BusinessDayConvention::ModifiedFollowing)
                    .calendar_id_opt(None)
                    .stub(StubKind::None)
                    .discount_curve_id(CurveId::from("USD-OIS"))
                    .amortization(AmortizationSpec::None)
                    .coupon_type(CouponType::Cash)
                    .upfront_fee_opt(None)
                    .ddtl_opt(None)
                    .covenants_opt(None)
                    .instrument_pricing_overrides(Default::default())
                    .attributes(Default::default())
                    .build()
                    .expect("valid term loan"),
            ),
        )
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
            priority_of_payments: vec![
                finstack_quant_statements::capital_structure::PaymentPriority::Fees,
                finstack_quant_statements::capital_structure::PaymentPriority::Interest,
                finstack_quant_statements::capital_structure::PaymentPriority::Amortization,
                finstack_quant_statements::capital_structure::PaymentPriority::Sweep,
                finstack_quant_statements::capital_structure::PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            pik_toggle: None,
            ..Default::default()
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
    assert!(results
        .get(
            "ebitda",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        )
        .is_some());
    assert_eq!(
        results.get(
            "ebitda",
            &PeriodId::quarter(2025, 1).expect("valid period fixture")
        ),
        Some(1_000_000.0)
    );
}

// calculate_period_flows + execute_waterfall integration
//
// The in-module waterfall tests use synthetic flows that bypass
// `calculate_period_flows`; the tests below exercise the full per-period
// pipeline (contractual flow extraction → waterfall → state advance).

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
        calculate_period_flows, execute_waterfall, CapitalStructureState, EcfSweepSpec,
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

    impl finstack_quant_cashflows::CashflowScheduleSource for ScheduleInstrument {
        fn raw_cashflow_schedule(
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
            id: PeriodId::quarter(year, q).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        }
    }

    /// Cash pool for fixtures that predate the required `available_cash_node`:
    /// ample, so the scheduled flows are funded exactly as before.
    const AMPLE_CASH: f64 = 1e12;

    fn context_with(period: PeriodId, values: &[(&str, f64)]) -> EvaluationContext {
        let mut values = values.to_vec();
        if !values.iter().any(|(name, _)| *name == "cash") {
            values.push(("cash", AMPLE_CASH));
        }
        let values = values.as_slice();
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

    fn schedule(flows: Vec<CashFlow>, notional: f64, issue_date: Date) -> CashFlowSchedule {
        CashFlowSchedule::from_parts(
            flows,
            Notional::par(notional, Currency::USD).expect("valid notional fixture"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue_date),
                ..CashFlowMeta::default()
            },
        )
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
            flows.push(CashFlow::new(
                Date::from_calendar_date(year, month, 15).expect("valid date"),
                None,
                Money::new(-notional * rate_q, Currency::USD).expect("valid money fixture"),
                CFKind::Fixed,
                0.25,
                Some(rate_q * 4.0),
            ));
        }
        let instrument = ScheduleInstrument {
            schedule: schedule(flows, notional, issue),
        };

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                finstack_quant_statements::capital_structure::PaymentPriority::Fees,
                finstack_quant_statements::capital_structure::PaymentPriority::Interest,
                finstack_quant_statements::capital_structure::PaymentPriority::Amortization,
                finstack_quant_statements::capital_structure::PaymentPriority::Sweep,
                finstack_quant_statements::capital_structure::PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                target_instrument_ids: Some(vec!["TL-PIK".into()]),
                min_periods_in_pik: 0,
            }),
            ..Default::default()
        };

        let market_ctx = MarketContext::new();
        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "TL-PIK".to_string(),
            Money::new(notional, Currency::USD).expect("valid money fixture"),
        );
        state
            .residual_schedules
            .insert("TL-PIK".to_string(), instrument.schedule.clone());

        let mut last_pik = 0.0;
        let mut capitalizations: Vec<(Date, f64)> = Vec::new();
        let mut expected_balance = notional;
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
                .unwrap_or_else(|| Money::new(0.0, Currency::USD).expect("valid money fixture"));

            let residual = state.residual_schedules.get("TL-PIK");
            let (breakdown, _, _, warnings) = calculate_period_flows(
                &instrument,
                &period,
                opening,
                toggled_pik,
                &market_ctx,
                issue,
                residual,
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
            let coupon = &instrument.schedule.get_flows()[i as usize];
            let start = if i == 0 {
                issue
            } else {
                instrument.schedule.get_flows()[i as usize - 1].date
            };
            let days = (coupon.date - start).whole_days() as f64;
            let expected = notional * rate_q
                + capitalizations
                    .iter()
                    .map(|(date, amount)| {
                        amount
                            * rate_q
                            * (coupon.date - (*date).max(start)).whole_days().max(0) as f64
                            / days
                    })
                    .sum::<f64>();
            assert!(
                (last_pik - expected).abs() < 1e-6,
                "quarter {i}: {last_pik} versus {expected}"
            );
            expected_balance += expected;
            capitalizations.push((period.end, expected));

            let snapshot = period.end - time::Duration::days(1);
            state
                .rebuild_residuals(snapshot)
                .expect("rebuild residual after PIK capitalize");
            state.advance_period();
        }

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
        assert!(
            (closing - expected_balance).abs() < 1e-6,
            "balance should compound to {expected_balance}, got {closing}"
        );
    }

    /// A mid-accrual sweep reduces only interest earned after the sweep date.
    #[test]
    fn sweep_rebuilds_next_period_coupon_on_new_outstanding() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let notional = 1_000_000.0;
        let q1 = quarter_period(2025, 1);
        let q2 = quarter_period(2025, 2);

        let instrument = ScheduleInstrument {
            schedule: schedule(
                vec![
                    CashFlow::new(
                        Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                        None,
                        Money::new(-20_000.0, Currency::USD).expect("valid money fixture"),
                        CFKind::Fixed,
                        0.25,
                        Some(0.08),
                    ),
                    CashFlow::new(
                        Date::from_calendar_date(2025, Month::May, 15).expect("valid date"),
                        None,
                        Money::new(-20_000.0, Currency::USD).expect("valid money fixture"),
                        CFKind::Fixed,
                        0.25,
                        Some(0.08),
                    ),
                ],
                notional,
                issue,
            ),
        };

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                finstack_quant_statements::capital_structure::PaymentPriority::Fees,
                finstack_quant_statements::capital_structure::PaymentPriority::Interest,
                finstack_quant_statements::capital_structure::PaymentPriority::Amortization,
                finstack_quant_statements::capital_structure::PaymentPriority::Sweep,
                finstack_quant_statements::capital_structure::PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "sweep_cash".into(),
                taxes_node: None,
                capex_node: None,
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 1.0,
                target_instrument_id: Some("TL-1".into()),
            }),
            pik_toggle: None,
            ..Default::default()
        };

        let market_ctx = MarketContext::new();
        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "TL-1".to_string(),
            Money::new(notional, Currency::USD).expect("valid money fixture"),
        );
        state
            .residual_schedules
            .insert("TL-1".to_string(), instrument.schedule.clone());

        let residual = state.residual_schedules.get("TL-1");
        let (breakdown, _, _, warnings) = calculate_period_flows(
            &instrument,
            &q1,
            Money::new(notional, Currency::USD).expect("valid money fixture"),
            Money::new(0.0, Currency::USD).expect("valid money fixture"),
            &market_ctx,
            issue,
            residual,
        )
        .expect("q1 flows");
        assert!(
            warnings.is_empty(),
            "q1 must book at scale 1, got {warnings:?}"
        );

        let mut contractual = IndexMap::new();
        contractual.insert("TL-1".to_string(), breakdown);
        // ECF = 500k, sweep 100% → 500k extra principal after 20k cash interest.
        let ctx = context_with(q1.id, &[("sweep_cash", 520_000.0)]);
        execute_waterfall(&q1.id, &ctx, &waterfall, &mut state, &contractual)
            .expect("q1 waterfall");
        state
            .rebuild_residuals(q1.end - time::Duration::days(1))
            .expect("rebuild after sweep");

        let coupon_date = Date::from_calendar_date(2025, Month::May, 15).unwrap();
        let prior_coupon = Date::from_calendar_date(2025, Month::February, 15).unwrap();
        let remaining_fraction = (coupon_date - q1.end).whole_days() as f64
            / (coupon_date - prior_coupon).whole_days() as f64;
        let expected_coupon = 20_000.0 - 10_000.0 * remaining_fraction;
        let rebuilt_q2: f64 = state.residual_schedules["TL-1"]
            .get_flows()
            .iter()
            .filter(|flow| flow.date == coupon_date)
            .map(|flow| flow.amount.amount().abs())
            .sum();
        assert!(
            (rebuilt_q2 - expected_coupon).abs() < 1e-6,
            "preserve interest earned before the sweep"
        );

        state.advance_period();
        let opening = state.opening_balances["TL-1"];
        let residual = state.residual_schedules.get("TL-1");
        let (q2_flows, _, _, q2_warnings) = calculate_period_flows(
            &instrument,
            &q2,
            opening,
            Money::new(0.0, Currency::USD).expect("valid money fixture"),
            &market_ctx,
            issue,
            residual,
        )
        .expect("q2 flows");
        assert!(
            q2_warnings.is_empty(),
            "rebuilt residual must not emit a scale warning, got {q2_warnings:?}"
        );
        assert!(
            (q2_flows.interest_expense_cash.amount() - expected_coupon).abs() < 1e-6,
            "q2 cash interest must preserve the pre-sweep accrual, got {}",
            q2_flows.interest_expense_cash.amount()
        );
    }

    /// Conservation through the full pipeline: an amortizing loan's fees +
    /// cash interest + principal + equity must equal available cash.
    #[test]
    fn waterfall_with_contractual_flows_conserves_available_cash() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let period = quarter_period(2025, 1);

        let instrument = ScheduleInstrument {
            schedule: schedule(
                vec![
                    CashFlow::new(
                        Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                        None,
                        Money::new(-10_000.0, Currency::USD).expect("valid money fixture"),
                        CFKind::Fixed,
                        0.25,
                        Some(0.04),
                    ),
                    CashFlow::new(
                        Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
                        None,
                        Money::new(50_000.0, Currency::USD).expect("valid money fixture"),
                        CFKind::Amortization,
                        0.0,
                        None,
                    ),
                ],
                1_000_000.0,
                issue,
            ),
        };

        let market_ctx = MarketContext::new();
        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "TL-1".to_string(),
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        );

        let opening = state.opening_balances["TL-1"];
        let (breakdown, _, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            opening,
            Money::new(0.0, Currency::USD).expect("valid money fixture"),
            &market_ctx,
            issue,
            None,
        )
        .expect("period flows");
        assert!(warnings.is_empty());

        let mut contractual: IndexMap<
            String,
            finstack_quant_statements::capital_structure::CashflowBreakdown,
        > = IndexMap::new();
        contractual.insert("TL-1".to_string(), breakdown);

        let waterfall = WaterfallSpec {
            available_cash_node: "cash_available".into(),
            priority_of_payments: vec![
                finstack_quant_statements::capital_structure::PaymentPriority::Fees,
                finstack_quant_statements::capital_structure::PaymentPriority::Interest,
                finstack_quant_statements::capital_structure::PaymentPriority::Amortization,
                finstack_quant_statements::capital_structure::PaymentPriority::Equity,
            ],
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
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
        let issue = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Instrument issued mid-horizon (Q3): funding draw on Jul 1, one
        // coupon in Q4.
        let instrument: Arc<dyn CashflowProvider + Send + Sync> = Arc::new(ScheduleInstrument {
            schedule: schedule(
                vec![
                    CashFlow::new(
                        issue,
                        None,
                        Money::new(-1_000_000.0, Currency::USD).expect("valid money fixture"),
                        CFKind::Notional,
                        0.0,
                        None,
                    ),
                    CashFlow::new(
                        Date::from_calendar_date(2025, Month::November, 15).expect("valid date"),
                        None,
                        Money::new(-20_000.0, Currency::USD).expect("valid money fixture"),
                        CFKind::Fixed,
                        0.25,
                        Some(0.08),
                    ),
                ],
                1_000_000.0,
                issue,
            ),
        });

        let mut instruments: IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>> =
            IndexMap::new();
        instruments.insert("DDTL-1".to_string(), instrument);

        let periods: Vec<Period> = (1..=4).map(|q| quarter_period(2025, q)).collect();
        let cashflows = crate::support::aggregate_period_flows(
            &instruments,
            &periods,
            &MarketContext::new(),
            as_of,
        )
        .expect("aggregation");

        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let q3 = PeriodId::quarter(2025, 3).expect("valid period fixture");
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
