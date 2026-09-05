//! Capital structure integration tests for spec builders.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{build_periods, Date};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_statements::capital_structure::build_instrument_from_spec;
use finstack_quant_statements::types::{DebtInstrumentSpec, FinancialStatementInstrument};
use finstack_quant_valuations::instruments::{fixed_income::bond::Bond, PayReceive};
use time::Month;

#[path = "support/period_flows.rs"]
mod support;

#[path = "support/rates.rs"]
mod rates_support;

use rates_support::usd_irs_swap;

#[test]
fn test_build_instrument_from_bond_spec() {
    let bond = Bond::fixed(
        InstrumentId::new("BOND-001"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
        Date::from_calendar_date(2030, Month::January, 15).expect("valid date"),
        finstack_quant_core::dates::StubKind::ShortFront,
        CurveId::new("USD-OIS"),
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let spec = DebtInstrumentSpec {
        id: "BOND-001".to_string(),
        spec: FinancialStatementInstrument::Bond(bond),
    };

    let instrument = build_instrument_from_spec(&spec).expect("bond should build");
    let notional = instrument
        .notional()
        .expect("valid amount")
        .expect("bond exposes notional");
    assert_eq!(notional.currency(), Currency::USD);
}

#[test]
fn test_build_instrument_from_swap_spec() {
    let swap = usd_irs_swap(
        InstrumentId::new("SWAP-001"),
        Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
        0.04,
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
        Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
        PayReceive::Pay,
    )
    .expect("swap should build");

    let spec = DebtInstrumentSpec {
        id: "SWAP-001".to_string(),
        spec: FinancialStatementInstrument::InterestRateSwap(swap),
    };

    let instrument = build_instrument_from_spec(&spec).expect("swap should build");
    let notional = instrument
        .notional()
        .expect("valid amount")
        .expect("swap exposes notional");
    assert_eq!(notional.currency(), Currency::USD);
}

#[test]
fn test_reporting_totals_sum_without_fx_when_same_currency() {
    use indexmap::IndexMap;
    use std::sync::Arc;

    let market_ctx = MarketContext::new();
    let periods = build_periods("2025M1..M1", None)
        .expect("valid periods")
        .periods;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let bond_1 = Bond::fixed(
        InstrumentId::new("BOND-1"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        CurveId::new("USD-OIS"),
    )
    .expect("bond_1");

    let bond_2 = Bond::fixed(
        InstrumentId::new("BOND-2"),
        Money::new(2_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        CurveId::new("USD-OIS"),
    )
    .expect("bond_2");

    let mut instruments: IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>> =
        IndexMap::new();
    instruments.insert("BOND-1".to_string(), Arc::new(bond_1));
    instruments.insert("BOND-2".to_string(), Arc::new(bond_2));

    let cashflows = support::aggregate_period_flows(&instruments, &periods, &market_ctx, issue)
        .expect("aggregate cashflows");

    let period_id =
        finstack_quant_core::dates::PeriodId::month(2025, 1).expect("valid period fixture");

    // Debt balance totals should sum across instruments even without FX matrix present.
    let total_balance = cashflows
        .get_total_debt_balance(&period_id)
        .expect("total debt balance");
    assert_eq!(total_balance, 3_000_000.0);

    // Accrued interest totals should be consistent with per-instrument values.
    let a1 = cashflows
        .get_accrued_interest("BOND-1", &period_id)
        .expect("accrued 1");
    let a2 = cashflows
        .get_accrued_interest("BOND-2", &period_id)
        .expect("accrued 2");
    let total_accrued = cashflows
        .get_total_accrued_interest(&period_id)
        .expect("total accrued");
    assert!(
        (total_accrued - (a1 + a2)).abs() < 1e-9,
        "total accrued should sum per-instrument accrued. total={}, sum={}",
        total_accrued,
        a1 + a2
    );
}

#[test]
fn test_capital_structure_builds_revolving_credit() {
    // RevolvingCredit had no typed DebtInstrumentSpec variant and was absent
    // from the old `Generic` brute-force list (Bond/IRS/TermLoan/Deposit/FRA/
    // Repo). Routing through the canonical registry makes it constructible.
    use finstack_quant_valuations::instruments::RevolvingCredit;

    let rcf = RevolvingCredit::example().expect("example RevolvingCredit");
    let spec = DebtInstrumentSpec {
        id: "RCF-001".to_string(),
        spec: FinancialStatementInstrument::RevolvingCredit(rcf),
    };

    build_instrument_from_spec(&spec)
        .expect("revolving credit must build via the canonical registry");
}
