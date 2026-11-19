//! Integration tests for Cash Flow Waterfall & Sweep Mechanics

use finstack_quant_cashflows::builder::CouponType;
use finstack_quant_cashflows::builder::{AmortizationSpec, RateSpec};
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
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use time::Month;

#[test]
fn test_ecf_sweep_basic() {
    // Create a simple model with a term loan and ECF sweep
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let model = {
        let mut model_builder = ModelBuilder::new("ecf_test");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid periods");
        model_builder.value(
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
        );
        model_builder.value(
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
        );
        model_builder.value(
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
        );
        model_builder.value(
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
        );
        model_builder.add_debt(
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
                    .rate(RateSpec::Fixed { rate: 0.05 })
                    .frequency(Tenor::quarterly())
                    .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
                    .day_count(DayCount::Act360)
                    .yield_basis(finstack_quant_valuations::metrics::returns::YieldBasis::Act360)
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
        );
        model_builder.waterfall(WaterfallSpec {
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".to_string(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::Input {
                        expression: "taxes".to_string(),
                    },
                    crate::capital_structure::EcfTerm::Input {
                        expression: "capex".to_string(),
                    },
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 0.5,
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
                payment_lag_days: None,
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
        });
        model_builder.build().expect("model should build")
    };

    // Create market context
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve should build");
    let market_ctx = MarketContext::new()
        .try_insert(disc_curve)
        .expect("insert market curve");

    // Evaluate model
    let mut evaluator = Evaluator::new();
    let results = evaluator
        .evaluate_with_market(
            &model,
            &market_ctx,
            issue,
            crate::evaluator::ObservationCutoff::FullSnapshot,
        )
        .expect("evaluation should succeed");

    // Verify that EBITDA values are present
    assert!(
        results
            .get(
                "ebitda",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            )
            .is_some()
    );
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
    use crate::capital_structure::waterfall::execute_waterfall_same_period;
    use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::cashflow::{CashFlow, CashFlowParams};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Period, PeriodId};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_statements::capital_structure::{
        CapitalStructureState, WaterfallSpec, calculate_period_flows,
    };
    use finstack_quant_statements::evaluator::EvaluationContext;
    use finstack_quant_statements::types::NodeId;
    use finstack_quant_valuations::instruments::Instrument;
    use indexmap::IndexMap;
    use std::sync::Arc;
    use time::Month;

    use crate::support::ScheduleInstrument;

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
        let mut ctx = EvaluationContext::new(
            period,
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            Arc::new(node_to_column),
            Arc::new(IndexMap::new()),
        );
        for (name, value) in values {
            ctx.set_value(name, *value).expect("context accepts value");
        }
        ctx
    }

    fn schedule(flows: Vec<CashFlow>, notional: f64, issue_date: Date) -> CashFlowSchedule {
        CashFlowSchedule::from_flows(
            flows,
            Notional::par(notional, Currency::USD).expect("valid notional fixture"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue_date),
                ..CashFlowMeta::default()
            },
        )
    }

    /// Conservation through the full pipeline: an amortizing loan's fees +
    /// cash interest + principal + equity must equal available cash.
    #[test]
    fn waterfall_with_contractual_flows_conserves_available_cash() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let period = quarter_period(2025, 1);

        let instrument = ScheduleInstrument::new(schedule(
            vec![
                CashFlowParams::new(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    None,
                    Money::new(10_000.0, Currency::USD).expect("valid money fixture"),
                    CFKind::Fixed,
                    0.25,
                    Some(0.04),
                )
                .build()
                .expect("valid cashflow"),
                {
                    let cash = Money::new(50_000.0, Currency::USD).expect("valid money fixture");
                    CashFlowParams::new(
                        Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
                        None,
                        cash,
                        CFKind::Amortization,
                        0.0,
                        None,
                    )
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(
                        Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
                    )
                    .build()
                    .expect("valid cashflow")
                },
            ],
            1_000_000.0,
            issue,
        ));

        let market_ctx = MarketContext::new();
        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "TL-1".to_string(),
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        );

        let opening = state.opening_balances["TL-1"];
        let (breakdown, _, _) =
            calculate_period_flows(&instrument, &period, opening, &market_ctx, issue, None)
                .expect("period flows");

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
        let result = execute_waterfall_same_period(&ctx, &waterfall, &mut state, &contractual)
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
        let instrument: Arc<dyn Instrument> = Arc::new(ScheduleInstrument::new(schedule(
            vec![
                {
                    let cash =
                        Money::new(-1_000_000.0, Currency::USD).expect("valid money fixture");
                    CashFlowParams::new(issue, None, cash, CFKind::Notional, 0.0, None)
                        .with_principal_delta(cash.checked_neg())
                        .with_principal_date(issue)
                        .build()
                        .expect("valid cashflow")
                },
                CashFlowParams::new(
                    Date::from_calendar_date(2025, Month::November, 15).expect("valid date"),
                    None,
                    Money::new(20_000.0, Currency::USD).expect("valid money fixture"),
                    CFKind::Fixed,
                    0.25,
                    Some(0.08),
                )
                .build()
                .expect("valid cashflow"),
            ],
            // The dated draw establishes the principal; an opening notional
            // that also held it would double the schedule's balance.
            0.0,
            issue,
        )));

        let mut instruments: IndexMap<String, Arc<dyn Instrument>> = IndexMap::new();
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
                    .get_component("debt_balance", "DDTL-1", &pre)
                    .expect("balance present"),
                0.0,
                "pre-issuance period {pre} must report zero debt balance"
            );
        }
        assert_eq!(
            cashflows
                .get_component("debt_balance", "DDTL-1", &q3)
                .expect("balance present"),
            1_000_000.0,
            "post-issuance balance must equal the funded notional"
        );
    }
}
