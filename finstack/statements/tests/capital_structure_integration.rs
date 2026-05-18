//! Capital structure integration tests for spec builders.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use finstack_cashflows::CashflowProvider;
use finstack_core::currency::Currency;
use finstack_core::dates::{build_periods, BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_statements::capital_structure::aggregate_instrument_cashflows;
use finstack_statements::capital_structure::integration::build_any_instrument_from_spec;
use finstack_statements::types::CapitalStructureSpec;
use finstack_statements::types::DebtInstrumentSpec;
use finstack_valuations::instruments::rates::irs::{FloatingLegCompounding, InterestRateSwap};
use finstack_valuations::instruments::{fixed_income::bond::Bond, PayReceive};
use finstack_valuations::instruments::{FixedLegSpec, FloatLegSpec};
use rust_decimal::Decimal;
use time::Month;

fn usd_irs_swap(
    id: InstrumentId,
    notional: Money,
    fixed_rate: f64,
    start: Date,
    end: Date,
    side: PayReceive,
) -> finstack_core::Result<InterestRateSwap> {
    let rate_decimal = Decimal::try_from(fixed_rate).map_err(|_| {
        finstack_core::Error::Validation(format!(
            "Invalid fixed rate: {} cannot be converted to Decimal. \
             Check for NaN, infinity, or values exceeding Decimal range.",
            fixed_rate
        ))
    })?;

    let fixed = FixedLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        rate: rate_decimal,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Thirty360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".to_string()),
        stub: StubKind::None,
        start,
        end,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,
    };

    let float = FloatLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: Decimal::ZERO,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".to_string()),
        stub: StubKind::None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        start,
        end,
        compounding: FloatingLegCompounding::Simple,
        payment_lag_days: 0,
        end_of_month: false,
    };

    let swap = InterestRateSwap::builder()
        .id(id)
        .notional(notional)
        .side(side)
        .fixed(fixed)
        .float(float)
        .build()?;

    swap.validate()?;
    Ok(swap)
}

#[test]
fn test_build_any_instrument_from_bond_spec() {
    let bond = Bond::fixed(
        InstrumentId::new("BOND-001"),
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
        Date::from_calendar_date(2030, Month::January, 15).expect("valid date"),
        CurveId::new("USD-OIS"),
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let spec = DebtInstrumentSpec {
        id: "BOND-001".to_string(),
        spec: serde_json::to_value(finstack_valuations::instruments::InstrumentJson::Bond(bond))
            .expect("bond should serialize"),
    };

    let instrument = build_any_instrument_from_spec(&spec).expect("bond should deserialize");
    let notional = instrument.notional().expect("bond exposes notional");
    assert_eq!(notional.currency(), Currency::USD);
}

#[test]
fn test_build_any_instrument_from_swap_spec() {
    let swap = usd_irs_swap(
        InstrumentId::new("SWAP-001"),
        Money::new(5_000_000.0, Currency::USD),
        0.04,
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
        Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
        PayReceive::PayFixed,
    )
    .expect("swap should build");

    let spec = DebtInstrumentSpec {
        id: "SWAP-001".to_string(),
        spec: serde_json::to_value(
            finstack_valuations::instruments::InstrumentJson::InterestRateSwap(swap),
        )
        .expect("swap should serialize"),
    };

    let instrument = build_any_instrument_from_spec(&spec).expect("swap should deserialize");
    let notional = instrument.notional().expect("swap exposes notional");
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
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        CurveId::new("USD-OIS"),
    )
    .expect("bond_1");

    let bond_2 = Bond::fixed(
        InstrumentId::new("BOND-2"),
        Money::new(2_000_000.0, Currency::USD),
        0.06,
        issue,
        maturity,
        CurveId::new("USD-OIS"),
    )
    .expect("bond_2");

    let mut instruments: IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>> =
        IndexMap::new();
    instruments.insert("BOND-1".to_string(), Arc::new(bond_1));
    instruments.insert("BOND-2".to_string(), Arc::new(bond_2));

    let spec = CapitalStructureSpec {
        debt_instruments: vec![],
        equity_instruments: vec![],
        meta: IndexMap::new(),
        reporting_currency: Some(Currency::USD),
        fx_policy: None,
        waterfall: None,
    };

    let cashflows =
        aggregate_instrument_cashflows(&spec, &instruments, &periods, &market_ctx, issue)
            .expect("aggregate cashflows");

    let period_id = finstack_core::dates::PeriodId::month(2025, 1);

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
    use finstack_valuations::instruments::{InstrumentJson, RevolvingCredit};

    let rcf = RevolvingCredit::example().expect("example RevolvingCredit");
    let spec = DebtInstrumentSpec {
        id: "RCF-001".to_string(),
        spec: serde_json::to_value(InstrumentJson::RevolvingCredit(rcf))
            .expect("revolving credit should serialize"),
    };

    build_any_instrument_from_spec(&spec)
        .expect("revolving credit must build via the canonical registry");
}
