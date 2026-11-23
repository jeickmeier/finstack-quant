use super::*;
use crate::capital_structure::{
    CapitalStructureState, CashflowBreakdown, EcfSweepSpec, PaymentClassSpec, PaymentPriority,
    PikToggleSpec, WaterfallSpec,
};
use crate::evaluator::{CapitalStructureWarning, EvalWarning, EvaluationContext};
use finstack_quant_core::currency::Currency;
#[cfg(test)]
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;

/// Cash pool used by fixtures that predate the required
/// `available_cash_node`. Large enough that every scheduled flow is funded,
/// so those tests keep asserting the same numbers — the difference is that
/// the funding is now stated rather than assumed.
const AMPLE_CASH: f64 = 1e12;

fn build_context(period: PeriodId, values: &[(&str, f64)]) -> EvaluationContext {
    let mut values = values.to_vec();
    if !values.iter().any(|(name, _)| *name == "cash") {
        values.push(("cash", AMPLE_CASH));
    }
    let values = values.as_slice();
    let mut node_to_column = IndexMap::new();
    for (idx, (name, _)) in values.iter().enumerate() {
        node_to_column.insert(crate::types::NodeId::new(*name), idx);
    }
    let mut ctx = EvaluationContext::new(
        period,
        &finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            Vec::new(),
        )
        .expect("empty plan"),
        std::sync::Arc::new(node_to_column),
        std::sync::Arc::new(IndexMap::new()),
    );
    for (name, value) in values {
        ctx.set_value(name, *value)
            .expect("sample context should accept provided node values");
    }
    ctx
}

#[test]
fn test_execute_waterfall_applies_ecf_sweep_and_updates_state() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(
        period,
        &[
            ("ebitda", 1_000_000.0),
            ("taxes", 200_000.0),
            ("capex", 50_000.0),
        ],
    );

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut tl_breakdown = CashflowBreakdown::with_currency(Currency::USD);
    tl_breakdown.principal_payment = Money::from((100_000_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), tl_breakdown);

    let mut state = CapitalStructureState::new();
    state.opening_balances.insert(
        "TL-1".to_string(),
        Money::from((10_000_000_i64, Currency::USD)),
    );

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::Input {
                    expression: "taxes".into(),
                },
                crate::capital_structure::EcfTerm::Input {
                    expression: "capex".into(),
                },
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 0.5,
            facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            payment_lag_days: None,
        }),
        pik_toggle: None,
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    // Corrected ECF convention (per standard LPA / S&P LCD definitions):
    // scheduled amortization ranked ahead of the sweep is deducted from
    // ECF before applying the sweep percentage.
    // ECF = 1,000,000 - 200,000 - 50,000 - 100,000 (sched amort) = 650,000
    // Sweep = 650,000 × 0.5 = 325,000; total principal = 100,000 + 325,000.
    let tl_result = results.flows.get("TL-1").expect("instrument exists");
    assert_eq!(tl_result.principal_payment.amount(), 425_000.0);
    assert_eq!(
        state
            .closing_balances
            .get("TL-1")
            .expect("closing balance")
            .amount(),
        10_000_000.0 - 425_000.0
    );
}

/// A negative cash pool is floored to zero for allocation, but the
/// operating shortfall must be surfaced, not silently erased: every
/// creditor gets shorted and the only diagnostic would otherwise be
/// downstream shortfall warnings that never mention the negative pool.
#[test]
fn negative_available_cash_is_warned_before_flooring() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash", -500.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let result =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("a negative pool floors to zero and still executes");

    assert!(
        result.warnings.iter().any(|w| matches!(
            w,
            EvalWarning::CapitalStructure {
                warning: CapitalStructureWarning::NegativeAvailableCashFloored { .. },
                ..
            }
        )),
        "the negative cash pool must be surfaced: {:?}",
        result.warnings
    );
}

/// Cash conservation under an available-cash cap: every cash outflow
/// (fees + cash interest + cash principal) plus the equity residual must
/// sum exactly to the available pool when claims exceed it.
#[test]
fn capped_waterfall_conserves_available_cash_identity() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash", 260.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    breakdown.principal_payment = Money::from((200_i64, Currency::USD));
    breakdown.fees = Money::from((50_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let result =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let flows = result.flows.get("TL-1").expect("instrument exists");
    let cash_out = flows.fees.amount()
        + flows.interest_expense_cash.amount()
        + flows.principal_payment.amount();
    let equity = result
        .equity_distribution
        .map_or(0.0, |money| money.amount());

    // Claims total 350 against a 260 pool: waterfall order pays fees 50,
    // interest 100, principal 110; nothing reaches equity.
    assert!(
        (cash_out + equity - 260.0).abs() < 1e-9,
        "cash out ({cash_out}) + equity ({equity}) must equal the 260 pool"
    );
    assert_eq!(flows.fees.amount(), 50.0);
    assert_eq!(flows.interest_expense_cash.amount(), 100.0);
    assert_eq!(flows.principal_payment.amount(), 110.0);
}

/// A non-finite `available_cash_node` must be a hard error, not a silent
/// zero. `NaN.max(0.0)` evaluates to 0.0, which would short every creditor
/// against an empty cash pool while reporting only downstream shortfall
/// warnings — no diagnostic pointing at the broken formula.
#[test]
fn available_cash_node_rejects_an_input_without_a_value() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    // A lag with no prior period has no value.
    let context = build_context(period, &[("cash", 0.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "lag(cash, 1)".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let err = execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
        .expect_err("available cash without a value must error rather than read as zero");
    let msg = err.to_string();
    assert!(
        msg.contains("has no value") && msg.contains("lag(cash, 1)"),
        "error must name the offending expression: {msg}"
    );
}

/// Typed inputs keep their units: money in another currency is refused,
/// not allocated as if it were the waterfall currency.
#[test]
fn a_cash_pool_in_another_currency_is_refused() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let mut context = build_context(period, &[]);
    context
        .set_cell(
            "cash",
            crate::evaluator::cell::Cell::Money(Money::from((500_i64, Currency::EUR))),
        )
        .expect("cell");
    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    contractual_flows.insert(
        "TL-1".to_string(),
        CashflowBreakdown::with_currency(Currency::USD),
    );
    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));
    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ..Default::default()
    };
    let msg = execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
        .expect_err("a EUR pool cannot pay a USD waterfall")
        .to_string();
    assert!(msg.contains("EUR") && msg.contains("USD"), "{msg}");
}

/// `available_cash_node` is the pre-waterfall cash pool. Deducting
/// `cs.*` debt service in that formula double-pays interest / principal.
#[test]
fn available_cash_node_rejects_cs_debt_service_formula() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(
        period,
        &[("ebitda", 1_000_000.0), ("cash_available", 500_000.0)],
    );

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let stack = vec![
        PaymentPriority::Fees,
        PaymentPriority::Interest,
        PaymentPriority::Amortization,
        PaymentPriority::Equity,
    ];
    let forbidden = WaterfallSpec {
        priority_of_payments: stack.clone(),
        available_cash_node: "ebitda - cs.interest_expense_cash.total".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };
    let err = execute_waterfall_same_period(&context, &forbidden, &mut state, &contractual_flows)
        .expect_err("deducting cs debt service from available cash must error");
    let msg = err.to_string();
    assert!(
        msg.contains("before debt service") && msg.contains("cs.interest_expense"),
        "error must name the pre-debt-service contract: {msg}"
    );

    let allowed = WaterfallSpec {
        priority_of_payments: stack,
        available_cash_node: "cash_available".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };
    execute_waterfall_same_period(&context, &allowed, &mut state, &contractual_flows)
        .expect("a standalone cash_available node must still be accepted");
}

/// A finite-but-astronomical cash value must error rather than panic.
/// `Money::new` asserts a `rust_decimal`-representable amount (~7.9e28) and
/// panics beyond it, which would abort evaluation from ordinary model data.
#[test]
fn available_cash_node_rejects_amount_beyond_decimal_range() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash", 1e30)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let err = execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
        .expect_err("out-of-range available cash must error rather than panic");
    let msg = err.to_string();
    assert!(
        msg.contains("1e30") || msg.contains("Decimal") || msg.contains("representable"),
        "error must describe the out-of-range amount, not an unrelated spec problem: {msg}"
    );
}

/// Warnings raised inside an inline waterfall expression must survive.
///
/// The expression runs against a scratch copy of the context, and the copy
/// (with its warnings) was dropped. A guarded expression returns a finite
/// value, so the bad arithmetic inside it left no trace at all.
#[test]
fn a_guarded_division_by_zero_still_fails() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash", 500.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        // `coalesce` recovers an unavailable value, not invalid arithmetic.
        available_cash_node: "coalesce(cash / 0, cash)".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let err = execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
        .expect_err("a division by zero is an error even inside coalesce");
    let msg = err.to_string();
    assert!(
        msg.contains("division by zero") && msg.contains("coalesce(cash / 0, cash)"),
        "error must name the cause and the expression: {msg}"
    );
}

/// An internally inconsistent input breakdown must error, not panic.
///
/// `waterfall_currency` compares only `interest_expense_cash` *across*
/// instruments, so a breakdown whose other legs carry a different currency
/// passed the entry check and reached `Money`'s asserting `AddAssign`,
/// aborting evaluation. `execute_waterfall` is public and
/// `CashflowBreakdown`'s fields are `pub`, so this is reachable input.
#[test]
fn inconsistent_breakdown_currency_errors_rather_than_panicking() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash", 1_000.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    // A caller-built breakdown with one leg in the wrong currency.
    breakdown.interest_expense_pik = Money::from((50_i64, Currency::EUR));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    // A firing PIK toggle reaches `interest_expense_pik += moved` in Step
    // 4b, where `Money`'s asserting `AddAssign` aborts on the mismatch.
    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: None,
        pik_toggle: Some(PikToggleSpec {
            liquidity_metric: "cash".into(),
            threshold: 1_000_000.0, // always below => PIK on
            elections: vec![crate::capital_structure::PikElectionSpec {
                instrument_id: "TL-1".to_string(),
                pik_fraction: rust_decimal::Decimal::ONE,
                min_coupon_periods: 0,
                notice_days: 0,
            }],
        }),
        ..Default::default()
    };

    let err = execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
        .expect_err("an inconsistent breakdown must error rather than panic");
    assert!(
        err.to_string().contains("Currency mismatch"),
        "expected a currency-mismatch diagnostic naming the field: {err}"
    );
}

/// A non-finite PIK liquidity metric must error rather than silently
/// evaluate `NaN < threshold` as false (i.e. "PIK not triggered").
#[test]
fn pik_liquidity_metric_rejects_non_finite_value() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("liquidity", 0.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Amortization,
            PaymentPriority::Interest,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: None,
        pik_toggle: Some(PikToggleSpec {
            liquidity_metric: "liquidity / 0".into(),
            threshold: 1.0,
            elections: vec![crate::capital_structure::PikElectionSpec {
                instrument_id: "TL-1".to_string(),
                pik_fraction: rust_decimal::Decimal::ONE,
                min_coupon_periods: 0,
                notice_days: 0,
            }],
        }),
        ..Default::default()
    };

    let err = execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
        .expect_err("an invalid PIK metric must error rather than read as 'not triggered'");
    assert!(
        err.to_string().contains("division by zero"),
        "error must flag the invalid metric: {err}"
    );
}

#[test]
fn test_ecf_sweep_deducts_fees_before_sweep_percentage() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("ebitda", 1_000.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.fees = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Amortization,
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 0.5,
            facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            payment_lag_days: None,
        }),
        pik_toggle: None,
        ..Default::default()
    };

    let result =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = result.flows.get("TL-1").expect("TL-1");
    assert_eq!(
        tl.principal_payment.amount(),
        450.0,
        "fees paid ahead of sweep reduce ECF before the sweep percentage: (1000 - 100) * 50%"
    );
}

/// The swept fraction is rounded to the minor unit, half away from zero:
/// 50% of 100.03 is 50.015, requested and paid as 50.02.
#[test]
fn test_ecf_sweep_request_is_rounded_to_the_minor_unit() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("ebitda", 100.03)]);
    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    contractual_flows.insert(
        "TL-1".to_string(),
        CashflowBreakdown::with_currency(Currency::USD),
    );
    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));
    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Amortization,
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        available_cash_node: "ebitda".into(),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: Vec::new(),
            credits: Vec::new(),
            sweep_percentage: 0.5,
            facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            payment_lag_days: None,
        }),
        pik_toggle: None,
        ..Default::default()
    };
    let result =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");
    let paid = result.flows.get("TL-1").expect("TL-1").principal_payment;
    assert_eq!(paid.amount_decimal().to_string(), "50.02");
}

#[test]
fn test_execute_waterfall_conserves_sweep_across_multiple_instruments() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(
        period,
        &[
            ("ebitda", 1_000_000.0),
            ("taxes", 200_000.0),
            ("capex", 50_000.0),
        ],
    );

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    contractual_flows.insert(
        "TL-1".to_string(),
        CashflowBreakdown::with_currency(Currency::USD),
    );
    contractual_flows.insert(
        "TL-2".to_string(),
        CashflowBreakdown::with_currency(Currency::USD),
    );

    let mut state = CapitalStructureState::new();
    state.opening_balances.insert(
        "TL-1".to_string(),
        Money::from((200_000_i64, Currency::USD)),
    );
    state.opening_balances.insert(
        "TL-2".to_string(),
        Money::from((300_000_i64, Currency::USD)),
    );

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::Input {
                    expression: "taxes".into(),
                },
                crate::capital_structure::EcfTerm::Input {
                    expression: "capex".into(),
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
        pik_toggle: None,
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let total_principal = results
        .flows
        .values()
        .map(|breakdown| breakdown.principal_payment.amount())
        .sum::<f64>();
    let tl1 = results.flows.get("TL-1").expect("TL-1 result");
    let tl2 = results.flows.get("TL-2").expect("TL-2 result");

    assert_eq!(total_principal, 375_000.0);
    assert!((tl1.principal_payment.amount() - 150_000.0).abs() < 1e-9);
    assert!((tl2.principal_payment.amount() - 225_000.0).abs() < 1e-9);
}

#[test]
fn test_available_cash_caps_scheduled_payments_by_priority() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash_available", 150.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.fees = Money::from((20_i64, Currency::USD));
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    breakdown.principal_payment = Money::from((200_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((1_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash_available".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = results.flows.get("TL-1").expect("TL-1");
    assert_eq!(tl.fees.amount(), 20.0);
    assert_eq!(tl.interest_expense_cash.amount(), 100.0);
    assert_eq!(tl.principal_payment.amount(), 30.0);
}

/// A dated schedule cannot reduce principal below zero: scheduled
/// principal beyond the balance is rejected, not clamped.
#[test]
fn scheduled_principal_beyond_the_balance_is_rejected() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash", 1_000.0)]);
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.principal_payment = Money::from((300_i64, Currency::USD));
    let contractual_flows = IndexMap::from([("TL-1".to_string(), breakdown)]);
    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((200_i64, Currency::USD)));
    let waterfall = WaterfallSpec {
        available_cash_node: "cash".into(),
        ..Default::default()
    };

    let error = execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
        .expect_err("300 scheduled against 200 outstanding");
    assert!(error.to_string().contains("negative"), "{error}");
    assert!(state.closing_balances.is_empty());
}

// Regression (C5): a revolver with an in-period draw must keep the drawn
// balance at close. Before the fix, Step 6 recomputed closing as
// `opening - principal` (no draw term), wiping the draw to zero.
#[test]
fn test_revolver_in_period_draw_preserved_at_close() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    // In-period repayment of 30k against a freshly-drawn balance.
    breakdown.principal_payment = Money::from((30_000_i64, Currency::USD));
    contractual_flows.insert("REVOLVER".to_string(), breakdown);

    // Fully-swept revolver: the ledger opens at zero and the dated
    // schedule draws 100k and repays 30k in the period, so its economic
    // close is 70k.
    contractual_flows["REVOLVER"].debt_balance = Money::from((70_000_i64, Currency::USD));
    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("REVOLVER".to_string(), Money::from((0_i64, Currency::USD)));
    let dated = finstack_quant_core::dates::Date::MIN;
    state
        .set_period_principal_flows(
            "REVOLVER",
            &finstack_quant_core::dates::Period {
                id: period,
                start: dated,
                end: finstack_quant_core::dates::Date::MAX,
                is_actual: false,
            },
            vec![crate::capital_structure::PrincipalClaim::at_par(
                dated,
                dated,
                Money::from((30_000_i64, Currency::USD)),
            )],
        )
        .expect("dated repayment");

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let rev = results.flows.get("REVOLVER").expect("REVOLVER");
    // closing = opening(0) + draws(100k) - principal(30k) = 70k.
    assert_eq!(rev.debt_balance.amount(), 70_000.0);
    assert_eq!(
        state
            .closing_balances
            .get("REVOLVER")
            .expect("closing balance")
            .amount(),
        70_000.0
    );
}

#[test]
fn test_sweep_before_amortization_does_not_produce_negative_balance() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("ebitda", 5_000.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.principal_payment = Money::from((300_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((500_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Sweep,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 1.0,
            facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            payment_lag_days: None,
        }),
        pik_toggle: None,
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = results.flows.get("TL-1").expect("TL-1");
    assert!(
        tl.debt_balance.amount() >= 0.0,
        "debt balance must never go negative, got {}",
        tl.debt_balance.amount()
    );
}

#[test]
fn test_ecf_defaults_cash_interest_from_contractual_flows() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(
        period,
        &[("ebitda", 1_000.0), ("taxes", 100.0), ("capex", 50.0)],
    );

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((200_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::Input {
                    expression: "taxes".into(),
                },
                crate::capital_structure::EcfTerm::Input {
                    expression: "capex".into(),
                },
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 0.5,
            facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            payment_lag_days: None,
        }),
        pik_toggle: None,
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = results.flows.get("TL-1").expect("TL-1");
    assert_eq!(tl.principal_payment.amount(), 325.0);
}

#[test]
fn a_negative_interest_claim_is_rejected() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(
        period,
        &[("ebitda", 1_000.0), ("taxes", 100.0), ("capex", 50.0)],
    );

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((-200_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::Input {
                    expression: "taxes".into(),
                },
                crate::capital_structure::EcfTerm::Input {
                    expression: "capex".into(),
                },
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 0.5,
            facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            payment_lag_days: None,
        }),
        pik_toggle: None,
        ..Default::default()
    };

    // A negative outflow claim is a sign error upstream: it is rejected,
    // not read as zero, and the state is left untouched.
    let error = execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
        .expect_err("negative cash interest claim")
        .to_string();
    assert!(error.contains("TL-1"), "{error}");
    assert!(error.contains("negative cash interest claim"), "{error}");
    assert!(state.closing_balances.is_empty());
}

/// Unpaid interest under an available-cash cap must not evaporate: it
/// accrues, raises a structured warning, and is carried as a claim in the
/// next period's interest category.
#[test]
fn test_cash_shortfall_accrues_and_carries_forward() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((1_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash_available".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    // Period 1: only 60 of cash against a 100 coupon.
    let ctx_short = build_context(period, &[("cash_available", 60.0)]);
    let result_short =
        execute_waterfall_same_period(&ctx_short, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = result_short.flows.get("TL-1").expect("TL-1");
    assert_eq!(tl.interest_expense_cash.amount(), 60.0);
    // The unpaid coupon was due: it is arrears, not accrued interest.
    assert_eq!(tl.interest_arrears.map(|a| a.amount()), Some(40.0));
    assert_eq!(tl.accrued_interest.amount(), 0.0);
    assert_eq!(
        state
            .interest_shortfall
            .get("TL-1")
            .expect("shortfall recorded")
            .amount(),
        40.0
    );
    assert_eq!(
        result_short.warnings.len(),
        1,
        "shortfall must surface a structured warning, got {:?}",
        result_short.warnings
    );
    assert_eq!(
        result_short
            .equity_distribution
            .expect("equity populated")
            .amount(),
        0.0
    );

    // Period 2: 200 of cash funds the 100 coupon plus the carried 40 claim.
    state.advance_period();
    let ctx_recovered = build_context(period, &[("cash_available", 200.0)]);
    let result_recovered =
        execute_waterfall_same_period(&ctx_recovered, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = result_recovered.flows.get("TL-1").expect("TL-1");
    assert_eq!(
        tl.interest_expense_cash.amount(),
        140.0,
        "carried claim is paid in the next period's interest category"
    );
    assert_eq!(tl.interest_arrears.map(|a| a.amount()), Some(0.0));
    assert!(
        state.interest_shortfall.get("TL-1").is_none(),
        "shortfall cleared once paid"
    );
    assert!(result_recovered.warnings.is_empty());
    assert_eq!(
        result_recovered
            .equity_distribution
            .expect("equity populated")
            .amount(),
        60.0
    );
}

#[test]
fn test_principal_shortfall_carries_forward_as_amortization_claim() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.principal_payment = Money::from((100_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((1_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash_available".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let ctx_short = build_context(period, &[("cash_available", 60.0)]);
    let result_short =
        execute_waterfall_same_period(&ctx_short, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = result_short.flows.get("TL-1").expect("TL-1");
    assert_eq!(tl.principal_payment.amount(), 60.0);
    assert_eq!(
        state
            .principal_shortfall
            .get("TL-1")
            .expect("principal shortfall recorded")
            .iter()
            .map(|claim| claim.amount.amount())
            .sum::<f64>(),
        40.0
    );
    assert!(
        result_short.warnings.iter().any(|warning| matches!(
            warning,
            EvalWarning::CapitalStructure {
                warning: CapitalStructureWarning::PrincipalShortfall { .. },
                ..
            }
        )),
        "principal shortfall must surface a structured warning"
    );

    state.advance_period();
    let ctx_recovered = build_context(period, &[("cash_available", 200.0)]);
    let result_recovered =
        execute_waterfall_same_period(&ctx_recovered, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = result_recovered.flows.get("TL-1").expect("TL-1");
    assert_eq!(
        tl.principal_payment.amount(),
        140.0,
        "carried principal claim is paid in the next period's amortization category"
    );
    assert!(
        state.principal_shortfall.get("TL-1").is_none(),
        "principal shortfall clears once paid"
    );
    assert_eq!(
        result_recovered
            .equity_distribution
            .expect("equity populated")
            .amount(),
        60.0
    );
}

/// Conservation: fees + cash interest + principal + equity == available cash.
#[test]
fn test_equity_distribution_conserves_available_cash() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash_available", 1_000.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
    breakdown.fees = Money::from((20_i64, Currency::USD));
    breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
    breakdown.principal_payment = Money::from((200_i64, Currency::USD));
    contractual_flows.insert("TL-1".to_string(), breakdown);

    let mut state = CapitalStructureState::new();
    state
        .opening_balances
        .insert("TL-1".to_string(), Money::from((5_000_i64, Currency::USD)));

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash_available".into(),
        ecf_sweep: None,
        pik_toggle: None,
        ..Default::default()
    };

    let result =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = result.flows.get("TL-1").expect("TL-1");
    let equity = result
        .equity_distribution
        .expect("equity populated")
        .amount();
    assert_eq!(equity, 680.0);
    let conserved = tl.fees.amount()
        + tl.interest_expense_cash.amount()
        + tl.principal_payment.amount()
        + equity;
    assert!(
        (conserved - 1_000.0).abs() < 1e-9,
        "fees + interest + principal + equity must equal available cash, got {conserved}"
    );
}

#[test]
fn payment_classes_pay_senior_interest_before_junior() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(period, &[("cash_available", 50_000.0)]);

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    let mut first_lien = CashflowBreakdown::with_currency(Currency::USD);
    first_lien.interest_expense_cash = Money::from((50_000_i64, Currency::USD));
    contractual_flows.insert("1L".to_string(), first_lien);
    let mut second_lien = CashflowBreakdown::with_currency(Currency::USD);
    second_lien.interest_expense_cash = Money::from((50_000_i64, Currency::USD));
    contractual_flows.insert("2L".to_string(), second_lien);

    let mut state = CapitalStructureState::new();
    state.opening_balances.insert(
        "1L".to_string(),
        Money::from((1_000_000_i64, Currency::USD)),
    );
    state.opening_balances.insert(
        "2L".to_string(),
        Money::from((1_000_000_i64, Currency::USD)),
    );

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash_available".into(),
        payment_classes: vec![
            PaymentClassSpec {
                id: "first-lien".into(),
                rank: 0,
                instrument_ids: vec!["1L".into()],
            },
            PaymentClassSpec {
                id: "second-lien".into(),
                rank: 1,
                instrument_ids: vec!["2L".into()],
            },
        ],
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let first = results.flows.get("1L").expect("1L");
    let second = results.flows.get("2L").expect("2L");
    assert!(
        (first.interest_expense_cash.amount() - 50_000.0).abs() < 1e-9,
        "1L interest should be paid in full, got {}",
        first.interest_expense_cash.amount()
    );
    assert!(
        second.interest_expense_cash.amount().abs() < 1e-9,
        "2L interest should be unpaid, got {}",
        second.interest_expense_cash.amount()
    );
    assert!(
        results.warnings.iter().any(|w| matches!(
            w,
            EvalWarning::CapitalStructure {
                warning: CapitalStructureWarning::InterestShortfall {
                    instrument_id,
                    ..
                },
                ..
            } if instrument_id == "2L"
        )),
        "2L should record an interest shortfall, not a pro-rata split"
    );
}

#[test]
fn mandatory_and_sweep_rungs_apply_independently() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(
        period,
        &[
            ("ebitda", 300_000.0),
            ("mandatory", 200_000.0),
            ("cash_available", 1_000_000.0),
        ],
    );

    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    contractual_flows.insert(
        "TL-1".to_string(),
        CashflowBreakdown::with_currency(Currency::USD),
    );

    let mut state = CapitalStructureState::new();
    state.opening_balances.insert(
        "TL-1".to_string(),
        Money::from((10_000_000_i64, Currency::USD)),
    );

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::MandatoryPrepayment,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash_available".into(),
        mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
            amount: "mandatory".into(),
            facilities: crate::capital_structure::FacilitySelection::AllEligible,
        }),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 1.0,
            facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            payment_lag_days: None,
        }),
        pik_toggle: None,
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = results.flows.get("TL-1").expect("TL-1");
    assert!(
        (tl.principal_payment.amount() - 500_000.0).abs() < 1e-6,
        "mandatory 200k + sweep 300k should both apply, got {}",
        tl.principal_payment.amount()
    );
}

/// Scheduled amortization, a mandatory prepayment, the ECF sweep and a
/// voluntary prepayment all land in `principal_payment`; the breakdown
/// keeps each source separately and they add back to the total.
#[test]
fn principal_components_sum_to_principal_payment() {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let context = build_context(
        period,
        &[
            ("ebitda", 300_000.0),
            ("mandatory", 200_000.0),
            ("voluntary", 50_000.0),
            ("cash_available", 1_000_000.0),
        ],
    );

    let mut contractual = CashflowBreakdown::with_currency(Currency::USD);
    contractual.interest_expense_cash = Money::from((80_000_i64, Currency::USD));
    contractual.fees = Money::from((5_000_i64, Currency::USD));
    contractual.principal_payment = Money::from((100_000_i64, Currency::USD));
    let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
    contractual_flows.insert("TL-1".to_string(), contractual);

    let mut state = CapitalStructureState::new();
    state.opening_balances.insert(
        "TL-1".to_string(),
        Money::from((10_000_000_i64, Currency::USD)),
    );

    let waterfall = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::MandatoryPrepayment,
            PaymentPriority::Sweep,
            PaymentPriority::VoluntaryPrepayment,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash_available".into(),
        mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
            amount: "mandatory".into(),
            facilities: crate::capital_structure::FacilitySelection::AllEligible,
        }),
        voluntary_prepayment: Some(crate::capital_structure::PrepaymentSpec {
            amount: "voluntary".into(),
            facilities: crate::capital_structure::FacilitySelection::AllEligible,
        }),
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 1.0,
            facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            payment_lag_days: None,
        }),
        pik_toggle: None,
        ..Default::default()
    };

    let results =
        execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
            .expect("waterfall should execute");

    let tl = results.flows.get("TL-1").expect("TL-1");
    let amount = |money: Option<Money>| money.expect("recorded by the waterfall").amount();
    let scheduled = amount(tl.scheduled_principal);
    let mandatory = amount(tl.mandatory_prepayment);
    let sweep = amount(tl.sweep_prepayment);
    let voluntary = amount(tl.voluntary_prepayment);

    assert_eq!(scheduled, 100_000.0);
    assert_eq!(mandatory, 200_000.0);
    // ECF = EBITDA 300k - interest 80k - fees 5k - scheduled principal 100k.
    assert_eq!(sweep, 115_000.0);
    assert_eq!(voluntary, 50_000.0);
    assert_eq!(
        scheduled + mandatory + sweep + voluntary,
        tl.principal_payment.amount()
    );
    assert_eq!(tl.principal_payment.amount(), 465_000.0);

    // Opening balance less principal paid is the closing balance here
    // (no draws, no PIK).
    assert_eq!(amount(tl.opening_balance), 10_000_000.0);
    assert_eq!(
        amount(tl.opening_balance) - tl.principal_payment.amount(),
        tl.debt_balance.amount()
    );

    // Sources and uses of cash for the period reconcile.
    let available = results.available_cash.expect("waterfall ran").amount();
    let equity = results.equity_distribution.expect("residual").amount();
    assert_eq!(available, 1_000_000.0);
    assert_eq!(
        tl.fees.amount()
            + tl.interest_expense_cash.amount()
            + tl.principal_payment.amount()
            + equity,
        available
    );
    tl.validate_currency_invariant()
        .expect("components share the breakdown currency");
}

/// A breakdown written before the principal components existed still
/// deserializes, with the components unrecorded.
#[test]
fn breakdown_without_principal_components_deserializes() {
    let breakdown = CashflowBreakdown::with_currency(Currency::USD);
    let json = serde_json::to_value(&breakdown).expect("serialize");
    for field in [
        "opening_balance",
        "scheduled_principal",
        "mandatory_prepayment",
        "sweep_prepayment",
        "voluntary_prepayment",
    ] {
        assert!(
            json.get(field).is_none(),
            "{field} is skipped when unrecorded"
        );
    }
    let back: CashflowBreakdown = serde_json::from_value(json).expect("deserialize");
    assert!(back.scheduled_principal.is_none());
    assert!(back.opening_balance.is_none());
}
#[test]
fn waterfall_errors_preserve_state_and_retry_matches_a_clean_run() {
    let period = PeriodId::quarter(2025, 1).unwrap();
    let mut flow = CashflowBreakdown::with_currency(Currency::USD);
    flow.interest_expense_cash = Money::from((100_i64, Currency::USD));
    flow.principal_payment = Money::from((50_i64, Currency::USD));
    flow.debt_balance = Money::from((950_i64, Currency::USD));
    let contractual = IndexMap::from([("TL-1".to_string(), flow)]);
    let mut initial = CapitalStructureState::new();
    initial
        .opening_balances
        .insert("TL-1".into(), Money::from((1000_i64, Currency::USD)));
    initial
        .interest_shortfall
        .insert("TL-1".into(), Money::from((20_i64, Currency::USD)));
    initial
        .fee_shortfall
        .insert("TL-1".into(), Money::from((10_i64, Currency::USD)));
    let valid_spec = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::MandatoryPrepayment,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        pik_toggle: Some(PikToggleSpec {
            liquidity_metric: "liquidity".into(),
            threshold: 100.0,
            elections: vec![crate::capital_structure::PikElectionSpec {
                instrument_id: "TL-1".into(),
                pik_fraction: rust_decimal::Decimal::ONE,
                min_coupon_periods: 3,
                notice_days: 0,
            }],
        }),
        mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
            amount: "0".into(),
            facilities: crate::capital_structure::FacilitySelection::AllEligible,
        }),
        ..Default::default()
    };
    let valid_context = build_context(period, &[("liquidity", 50.0), ("cash", 1000.0)]);
    for late in [false, true] {
        let mut spec = valid_spec.clone();
        let context = if late {
            spec.mandatory_prepayment = Some(crate::capital_structure::PrepaymentSpec {
                amount: "1 / 0".into(),
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
            });
            valid_context.clone()
        } else {
            build_context(period, &[("liquidity", 50.0), ("cash", 1e100)])
        };
        let mut state = initial.clone();
        let before = format!("{state:?}");
        assert!(execute_waterfall_same_period(&context, &spec, &mut state, &contractual).is_err());
        assert_eq!(
            format!("{state:?}"),
            before,
            "failed waterfall changed runtime state"
        );
        let retry =
            execute_waterfall_same_period(&valid_context, &valid_spec, &mut state, &contractual)
                .unwrap();
        let mut clean_state = initial.clone();
        let clean = execute_waterfall_same_period(
            &valid_context,
            &valid_spec,
            &mut clean_state,
            &contractual,
        )
        .unwrap();
        assert_eq!(format!("{state:?}"), format!("{clean_state:?}"));
        assert_eq!(format!("{retry:?}"), format!("{clean:?}"));
    }
}

/// The waterfall reports the principal it changed in exact money and the
/// PIK toggle's request; the evaluator rebuilds the schedules from them.
#[test]
fn the_waterfall_reports_its_postings_and_pik_request() {
    use finstack_quant_core::dates::{Date, Period};
    let id = PeriodId::quarter(2025, 1).unwrap();
    let period = Period {
        id,
        start: Date::MIN,
        end: Date::MAX,
        is_actual: false,
    };
    let usd = |amount: i64| Money::from((amount, Currency::USD));
    let mut flow = CashflowBreakdown::with_currency(Currency::USD);
    flow.interest_expense_cash = usd(30);
    flow.debt_balance = usd(1000);
    let contractual = IndexMap::from([("TL-1".to_string(), flow)]);
    let mut initial = CapitalStructureState::new();
    initial.opening_balances.insert("TL-1".into(), usd(1000));
    initial
        .set_period_principal_flows("TL-1", &period, Vec::new())
        .unwrap();
    let spec = WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::MandatoryPrepayment,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        pik_toggle: Some(PikToggleSpec {
            liquidity_metric: "liquidity".into(),
            threshold: 100.0,
            elections: vec![crate::capital_structure::PikElectionSpec {
                instrument_id: "TL-1".into(),
                pik_fraction: rust_decimal::Decimal::ONE,
                min_coupon_periods: 0,
                notice_days: 0,
            }],
        }),
        mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
            amount: "200".into(),
            facilities: crate::capital_structure::FacilitySelection::AllEligible,
        }),
        ..Default::default()
    };
    let context = build_context(id, &[("liquidity", 50.0), ("cash", 1000.0)]);

    // 200 prepaid. The toggle only records its request: the 30 coupon
    // due now is paid in cash, not capitalized.
    let mut state = initial;
    let result =
        execute_waterfall(&context, &spec, &mut state, &contractual, None).expect("waterfall");
    assert_eq!(result.postings["TL-1"], usd(-200));
    assert_eq!(result.pik_request, Some(true));
    assert_eq!(state.closing_balances["TL-1"], usd(800));
}
