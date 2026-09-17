//! Call assumptions and clean-up call economics: a deal call redeems every
//! note on the call date at the call price, a tranche call only prices the
//! named class to its refinancing, `TrancheMetrics` keeps the to-maturity
//! figures next to the `*_to_call` twins, and the clean-up call is exercised
//! only when the liquidation proceeds cover the notes.

use finstack_quant_cashflows::builder::{
    DefaultModelSpec, FloatingRateSpec, PrepaymentModelSpec, RecoveryModelSpec,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_metrics, run_simulation, AssetPool, CallAssumption, DealType, PoolAsset,
    ReinvestmentCriteria, ReinvestmentPeriod, StructuredCredit, Tranche, TrancheCashflows,
    TrancheCoupon, TrancheMetrics, TrancheSeniority, TrancheStructure,
};
use time::Month;

use super::instrument_pool_tests::market_with_curves;

const INDEX: &str = "USD-SOFR-3M";

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn close() -> Date {
    d(2024, 1, 1)
}

fn maturity() -> Date {
    d(2032, 1, 1)
}

fn floating(spread_bp: f64) -> FloatingRateSpec {
    let dec = |value: f64| rust_decimal::Decimal::try_from(value).expect("decimal");
    FloatingRateSpec {
        index_id: CurveId::new(INDEX),
        spread_bp: dec(spread_bp),
        gearing: dec(1.0),
        gearing_includes_spread: true,
        index_floor_bp: None,
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        index_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    }
}

/// Quarterly CLO: ten 10M 8% bullet loans, A 60M at SOFR + 150, B 30M at 7%,
/// E 10M, `cpr` prepayments, no defaults, and a reinvestment window to the
/// start of 2027 during which every note is held flat.
fn clo(cpr: f64) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            0.08,
            maturity(),
            DayCount::Act360,
        ));
    }
    pool.reinvestment_period = Some(ReinvestmentPeriod {
        end_date: d(2027, 1, 1),
        is_active: true,
        criteria: ReinvestmentCriteria::default(),
        amortizing_tranches: Vec::new(),
        assumptions: None,
    });
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            60.0,
            TrancheSeniority::Senior,
            usd(60_000_000.0),
            TrancheCoupon::Floating(floating(150.0)),
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "B",
            60.0,
            90.0,
            TrancheSeniority::Mezzanine,
            usd(30_000_000.0),
            TrancheCoupon::Fixed { rate: 0.07 },
            maturity(),
        )
        .expect("B"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(10_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_clo("CLO-CALL", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(cpr);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 0);
    deal
}

fn simulate(deal: &StructuredCredit) -> HashMap<String, TrancheCashflows> {
    run_simulation(deal, &market_with_curves(close()), close()).expect("simulation")
}

fn metrics(deal: &StructuredCredit, id: &str) -> TrancheMetrics {
    calculate_tranche_metrics(deal, id, &market_with_curves(close()), close(), Some(99.0))
        .expect("tranche metrics")
}

fn last_flow_date(results: &HashMap<String, TrancheCashflows>, id: &str) -> Option<Date> {
    results[id].cashflows.last().map(|(date, _)| *date)
}

fn principal_on(results: &HashMap<String, TrancheCashflows>, id: &str, date: Date) -> f64 {
    results[id]
        .principal_flows
        .iter()
        .filter(|(flow_date, _)| *flow_date == date)
        .map(|(_, amount)| amount.amount())
        .sum()
}

/// A deal call on the reinvestment end redeems every note on the first
/// payment date at or after it: par plus stub accrued, a 1% call premium as
/// interest, the collateral residual to equity, and nothing afterwards.
#[test]
fn deal_call_redeems_every_note_on_the_call_date() {
    let mut deal = clo(0.10);
    deal.call_assumption = Some(CallAssumption::new(d(2027, 1, 1), 101.0));

    let results = simulate(&deal);
    let call_date = last_flow_date(&results, "A").expect("A is redeemed");

    assert!(
        call_date >= d(2027, 1, 1) && call_date < d(2027, 4, 1),
        "call date {call_date}"
    );
    for id in ["A", "B", "E"] {
        assert_eq!(
            last_flow_date(&results, id),
            Some(call_date),
            "{id} ends with the call"
        );
    }
    assert!((results["A"].total_principal.amount() - 60_000_000.0).abs() < 1.0);
    assert!((results["B"].total_principal.amount() - 30_000_000.0).abs() < 1.0);
    // 1% premium on 30M of class B lands in interest on the call date.
    let b_call_interest: f64 = results["B"]
        .interest_flows
        .iter()
        .filter(|(date, _)| *date == call_date)
        .map(|(_, amount)| amount.amount())
        .sum();
    assert!(
        b_call_interest > 300_000.0,
        "premium plus accrued: {b_call_interest}"
    );
    // Held flat through the window, the pool still covers the notes: equity
    // receives the liquidation residual.
    assert!(principal_on(&results, "E", call_date) > 0.0);
}

/// Metrics keep the to-maturity WAL and z-spread of the uncalled deal and add
/// the `*_to_call` twins; the floater's discount margin twin is its z-spread.
#[test]
fn tranche_metrics_report_to_call_twins_next_to_unchanged_to_maturity_figures() {
    let base = clo(0.10);
    let mut called = base.clone();
    called.call_assumption = Some(CallAssumption::new(d(2027, 1, 1), 100.0));

    let a_base = metrics(&base, "A");
    let a_called = metrics(&called, "A");
    let b_called = metrics(&called, "B");

    assert_eq!(a_base.wal_to_call, None);
    assert_eq!(
        a_called.wal, a_base.wal,
        "to-maturity WAL is projected without the call"
    );
    assert_eq!(a_called.z_spread_bp, a_base.z_spread_bp);
    let wal_to_call = a_called.wal_to_call.expect("deal call covers A");
    assert!(
        wal_to_call < a_called.wal,
        "{wal_to_call} < {}",
        a_called.wal
    );
    let z_to_call = a_called.z_spread_to_call_bp.expect("z-spread to call");
    assert_ne!(z_to_call, a_called.z_spread_bp);
    assert_eq!(
        a_called.dm_to_call_bp,
        Some(z_to_call),
        "floater DM to call"
    );
    assert!(b_called.wal_to_call.is_some());
    assert_eq!(
        b_called.dm_to_call_bp, None,
        "fixed-rate class has no discount margin"
    );
}

/// A tranche call prices only the named class to its refinancing: the deal's
/// cashflows are unchanged and the other classes carry no twins.
#[test]
fn tranche_call_truncates_only_the_named_class() {
    let base = clo(0.10);
    let mut refinanced = base.clone();
    refinanced.call_assumption = Some(CallAssumption::for_tranche(d(2027, 1, 1), 100.0, "B"));

    let base_results = simulate(&base);
    let refi_results = simulate(&refinanced);
    for id in ["A", "B", "E"] {
        assert_eq!(
            base_results[id].principal_flows,
            refi_results[id].principal_flows
        );
        assert_eq!(
            base_results[id].interest_flows,
            refi_results[id].interest_flows
        );
    }

    let b = metrics(&refinanced, "B");
    let a = metrics(&refinanced, "A");
    let b_wal_to_call = b.wal_to_call.expect("B is priced to its call");
    assert!(b_wal_to_call < b.wal);
    assert!(
        (b_wal_to_call - 3.0).abs() < 0.1,
        "B is repaid in full three years out: {b_wal_to_call}"
    );
    assert_eq!(b.wal, metrics(&base, "B").wal);
    assert_eq!(a.wal_to_call, None);
    assert_eq!(a.z_spread_to_call_bp, None);
}

/// The clean-up call is optional: with the collateral realizable at par the
/// proceeds cover the notes and the deal is called (equity takes the
/// residual, including the reserve); realizable at half of par they do not,
/// the call is skipped and the notes keep amortizing.
#[test]
fn cleanup_call_needs_liquidation_proceeds_to_cover_the_notes() {
    let cleanup = |liquidation: Option<f64>, reserve: f64| {
        let mut deal = clo(0.30);
        deal.pool.reinvestment_period = None;
        deal.cleanup_call_pct = Some(0.30);
        deal.liquidation_price_pct = liquidation;
        deal.pool.reserve_account = usd(reserve);
        deal
    };

    let at_par = simulate(&cleanup(None, 0.0));
    let with_reserve = simulate(&cleanup(None, 5_000_000.0));
    let at_half = simulate(&cleanup(Some(50.0), 0.0));

    let call_date = last_flow_date(&at_par, "B").expect("B redeemed");
    assert!(call_date < maturity());
    assert_eq!(last_flow_date(&at_par, "E"), Some(call_date));
    let residual = principal_on(&at_par, "E", call_date);
    assert!(residual > 0.0, "equity takes the liquidation residual");
    assert!(
        (principal_on(&with_reserve, "E", call_date) - residual - 5_000_000.0).abs() < 1.0,
        "the reserve is part of the proceeds"
    );

    let half_last = last_flow_date(&at_half, "B").expect("B still amortizes");
    assert!(
        half_last > call_date,
        "call skipped: {half_last} vs {call_date}"
    );
    assert_eq!(principal_on(&at_half, "E", call_date), 0.0);
}
