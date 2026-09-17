//! Config-sensitivity tests: public stochastic knobs must move the price, and
//! the four 2026-09 audit probes stay green on the deterministic engine.
//!
//! Guards against the "silently inert config" class from the 2026-07
//! structured-credit audit: a parameter that passes validation but never
//! reaches the engine produces bit-identical results under a fixed seed, so
//! each test here varies exactly one input and asserts the output changes.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_models::credit::pool::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_metrics, run_simulation_with_diagnostics, AssetPool, CoverageTestSpec,
    DealType, LossAllocationPolicy, PoolAsset, PricingMode, ReinvestmentCriteria,
    ReinvestmentPeriod, SimulationRun, StochasticPricingResult, StructuredCredit, Tranche,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

fn closing_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

fn as_of() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

fn legal_maturity() -> Date {
    Date::from_calendar_date(2026, Month::January, 1).unwrap()
}

fn fixed_market() -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of())
        .knots(vec![(0.0, 1.0), (5.0, 0.90)])
        .build()
        .expect("discount curve");
    MarketContext::new().insert(curve)
}

/// Ten equal names so the systematic/idiosyncratic split has something to
/// correlate: with a single asset, per-name default dispersion is invariant
/// to asset correlation and the sensitivity assertions would be vacuous.
fn pool() -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("A{i}"),
            Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
            0.06,
            legal_maturity(),
            DayCount::Thirty360,
        ));
    }
    pool
}

fn tranches() -> TrancheStructure {
    TrancheStructure::new(vec![
        Tranche::new(
            "SR",
            0.0,
            80.0,
            TrancheSeniority::Senior,
            Money::new(800_000.0, Currency::USD).expect("valid money fixture"),
            TrancheCoupon::Fixed { rate: 0.05 },
            legal_maturity(),
        )
        .unwrap(),
        Tranche::new(
            "EQ",
            80.0,
            100.0,
            TrancheSeniority::Equity,
            Money::new(200_000.0, Currency::USD).expect("valid money fixture"),
            TrancheCoupon::Fixed { rate: 0.0 },
            legal_maturity(),
        )
        .unwrap(),
    ])
    .unwrap()
}

/// Both variants of every test share this deal id, so `derive_seed` gives
/// them identical Monte Carlo draws — any output difference is the config.
fn structured_credit() -> StructuredCredit {
    let mut sc = StructuredCredit::new_abs(
        "ABS-CFG-SENS",
        pool(),
        tranches(),
        closing_date(),
        legal_maturity(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
    sc.credit_model.stochastic_prepay_spec = Some(StochasticPrepaySpec::deterministic(
        sc.credit_model.prepayment_spec.clone(),
    ));
    sc
}

fn price(sc: &StructuredCredit, market: &MarketContext) -> StochasticPricingResult {
    sc.price_stochastic_with_mode(
        market,
        as_of(),
        PricingMode::MonteCarlo {
            num_paths: 512,
            antithetic: false,
        },
    )
    .expect("stochastic pricing")
}

/// The deal-level `CorrelationStructure` must reach the copula: higher asset
/// correlation concentrates defaults on bad factor paths, widening the loss
/// distribution (Vasicek 2002). Exercises the `asset_correlation_override`
/// conduit end-to-end from the public deal type.
#[test]
fn correlation_structure_widens_the_loss_distribution() {
    let market = fixed_market();

    let mut low = structured_credit();
    low.credit_model.stochastic_default_spec =
        Some(StochasticDefaultSpec::gaussian_copula(0.25, 0.10));
    low.credit_model.correlation_structure =
        Some(CorrelationStructure::flat(0.05, 0.0).expect("valid correlation"));

    let mut high = low.clone();
    high.credit_model.correlation_structure =
        Some(CorrelationStructure::flat(0.60, 0.0).expect("valid correlation"));

    let low_result = price(&low, &market);
    let high_result = price(&high, &market);

    assert!(
        high_result.unexpected_loss.amount() > low_result.unexpected_loss.amount() + 1.0,
        "asset correlation must widen the loss distribution: rho=0.05 UL {}, rho=0.60 UL {}",
        low_result.unexpected_loss.amount(),
        high_result.unexpected_loss.amount()
    );
}

/// The intensity-process mean reversion κ must reach the systematic factor:
/// κ sets the factor autocorrelation `φ = e^{−κ/12}`, so κ=0 (persistent
/// factor) and κ=5 (fast mean reversion) must produce different loss
/// distributions. A silently inert κ yields bit-identical results under the
/// shared seed.
#[test]
fn intensity_mean_reversion_changes_the_loss_distribution() {
    let market = fixed_market();

    let mut persistent = structured_credit();
    persistent.credit_model.stochastic_default_spec = Some(
        StochasticDefaultSpec::intensity_process(0.10, 1.0, 0.0, 0.5),
    );

    let mut mean_reverting = persistent.clone();
    mean_reverting.credit_model.stochastic_default_spec = Some(
        StochasticDefaultSpec::intensity_process(0.10, 1.0, 5.0, 0.5),
    );

    let persistent_result = price(&persistent, &market);
    let reverting_result = price(&mean_reverting, &market);

    assert!(
        (persistent_result.unexpected_loss.amount() - reverting_result.unexpected_loss.amount())
            .abs()
            > 1e-6,
        "mean reversion must change the loss distribution: kappa=0 UL {}, kappa=5 UL {}",
        persistent_result.unexpected_loss.amount(),
        reverting_result.unexpected_loss.amount()
    );
}

// ── The four 2026-09 audit probes, kept as permanent sensitivity tests ─────
//
// Each probe reproduces one Blocker of the 2026-09-15 structured-credit
// audit on the deterministic engine and asserts the fixed behavior with a
// one-input change: price basis, loss allocation, coverage-test position and
// deal-level reinvestment.

fn deterministic(deal: &StructuredCredit, market: &MarketContext) -> SimulationRun {
    run_simulation_with_diagnostics(deal, market, as_of()).expect("deterministic run")
}

fn total(flows: &[(Date, Money)]) -> f64 {
    flows.iter().map(|(_, amount)| amount.amount()).sum()
}

fn tranche_mut<'a>(deal: &'a mut StructuredCredit, id: &str) -> &'a mut Tranche {
    deal.tranches
        .tranches
        .iter_mut()
        .find(|t| t.id.as_str() == id)
        .expect("tranche")
}

/// Audit probe 1 — a seasoned note at factor 0.5 has half the PV of the new
/// issue and the SAME price per 100 of CURRENT face (it used to be quoted per
/// original face, i.e. near 50).
#[test]
fn seasoned_note_prices_per_current_face() {
    let market = fixed_market();
    let new_issue = structured_credit();
    let mut seasoned = structured_credit();
    seasoned.pool.assets.truncate(6);
    tranche_mut(&mut seasoned, "SR").current_balance =
        Money::new(400_000.0, Currency::USD).expect("money");

    let fresh = calculate_tranche_metrics(&new_issue, "SR", &market, as_of(), None)
        .expect("new-issue metrics");
    let aged = calculate_tranche_metrics(&seasoned, "SR", &market, as_of(), None)
        .expect("seasoned metrics");
    assert!(
        (aged.pv / fresh.pv - 0.5).abs() < 1e-9,
        "PV scales with the current balance: {} vs {}",
        aged.pv,
        fresh.pv
    );
    assert!(
        (aged.price_pct - fresh.price_pct).abs() < 1e-6,
        "price per current face is unchanged by seasoning: {} vs {}",
        aged.price_pct,
        fresh.price_pct
    );
    assert!(aged.price_pct > 95.0 && aged.price_pct < 110.0);
}

/// Audit probe 2 — the loss-allocation policy reaches the notes: under
/// `WriteDown` a loss beyond the equity is written off the senior note when
/// the default happens, under `ParPreserving` the note keeps its par and the
/// same shortfall is realized only at legal final.
#[test]
fn loss_allocation_policy_changes_the_senior_note_flows() {
    let market = fixed_market();
    let mut base = structured_credit();
    base.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.30);
    base.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);

    let mut written_down = base.clone();
    written_down.loss_allocation = Some(LossAllocationPolicy::WriteDown);
    let mut par_preserving = base;
    par_preserving.loss_allocation = Some(LossAllocationPolicy::ParPreserving);

    let wd = deterministic(&written_down, &market);
    let pp = deterministic(&par_preserving, &market);
    let wd_writedowns = &wd.tranches["SR"].writedown_flows;
    let pp_writedowns = &pp.tranches["SR"].writedown_flows;
    assert!(
        total(wd_writedowns) > 0.0,
        "a 30% CDR exhausts the 20% equity, so the senior note takes a loss"
    );
    assert!(
        (total(wd_writedowns) - total(pp_writedowns)).abs() < 1e-6,
        "the same economic loss is realized either way: {} vs {}",
        total(wd_writedowns),
        total(pp_writedowns)
    );
    let legal_final = pp.tranches["SR"]
        .principal_flows
        .iter()
        .map(|(date, _)| *date)
        .max()
        .expect("principal flows");
    assert!(
        pp_writedowns.iter().all(|(date, _)| *date >= legal_final),
        "par-preserving notes realize the shortfall only at legal final"
    );
    assert!(
        wd_writedowns.iter().any(|(date, _)| *date < legal_final),
        "written-down notes take the loss when the default happens"
    );
    let wd_principal = total(&wd.tranches["SR"].principal_flows);
    let pp_principal = total(&pp.tranches["SR"].principal_flows);
    assert!(
        (wd_principal + total(wd_writedowns) - 800_000.0).abs() < 1e-6,
        "cash principal plus write-downs is the original balance"
    );
    assert!(
        (pp_principal - wd_principal).abs() < 1e-6,
        "the same cash reaches the note either way; only the timing of the write-down differs"
    );
}

/// Audit probe 3 — an OC test on the senior class sits AFTER that class's
/// coupon: a failing test diverts the residual into senior principal, it
/// never traps or defers the senior coupon it protects (the audit found the
/// senior coupon trapped).
#[test]
fn failing_coverage_test_does_not_trap_the_coupon_it_protects() {
    let market = fixed_market();
    let untested = structured_credit();
    let tested = structured_credit()
        .with_coverage_triggers(vec![CoverageTestSpec::oc("SR", 2.0)])
        .expect("coverage test");

    let without = deterministic(&untested, &market);
    let with = deterministic(&tested, &market);
    let senior_with = &with.tranches["SR"];
    let senior_without = &without.tranches["SR"];
    // Same opening balance on the first date, so the same coupon is paid
    // whether or not the test fails there.
    assert!(
        (senior_with.interest_flows[0].1.amount() - senior_without.interest_flows[0].1.amount())
            .abs()
            < 1e-6,
        "the failing OC test leaves the first senior coupon whole: {} vs {}",
        senior_with.interest_flows[0].1.amount(),
        senior_without.interest_flows[0].1.amount()
    );
    assert_eq!(
        total(&senior_with.deferred_flows),
        0.0,
        "nothing owed to the senior class is deferred"
    );
    assert_eq!(total(&senior_with.pik_flows), 0.0);
    // On the first date the residual is diverted into senior principal (over
    // the whole life the faster senior paydown leaves MORE residual, so only
    // the diversion date is a clean comparison).
    let first_date = senior_without.interest_flows[0].0;
    let equity_on_first_date = |run: &SimulationRun| {
        run.tranches["EQ"]
            .cashflows
            .iter()
            .filter(|(date, _)| *date == first_date)
            .map(|(_, amount)| amount.amount())
            .sum::<f64>()
    };
    let equity_with = equity_on_first_date(&with);
    let equity_without = equity_on_first_date(&without);
    assert!(
        equity_with < equity_without - 1.0,
        "the diverted residual repays senior principal: equity {equity_with} vs {equity_without}"
    );
    let senior_principal_on_first_date = |run: &SimulationRun| {
        run.tranches["SR"]
            .principal_flows
            .iter()
            .filter(|(date, _)| *date == first_date)
            .map(|(_, amount)| amount.amount())
            .sum::<f64>()
    };
    assert!(
        senior_principal_on_first_date(&with) > senior_principal_on_first_date(&without) + 1.0,
        "senior principal is repaid earlier under the failing test"
    );
    assert!(
        (total(&senior_with.principal_flows) - 800_000.0).abs() < 1e-6,
        "the senior note is still repaid in full"
    );
}

/// Audit probe 4 — a deal-level reinvestment period is not inert: opening a
/// window retains collateral principal (no senior paydown inside it), and
/// listing the note in `amortizing_tranches` pays it down again.
#[test]
fn reinvestment_window_moves_the_senior_paydown() {
    let market = fixed_market();
    let mut base = structured_credit();
    base.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    let window_end = Date::from_calendar_date(2025, Month::January, 1).expect("date");

    let mut retained = base.clone();
    retained.pool.reinvestment_period = Some(ReinvestmentPeriod {
        end_date: window_end,
        is_active: true,
        criteria: ReinvestmentCriteria::default(),
        amortizing_tranches: Vec::new(),
        assumptions: None,
    });
    let mut amortizing = retained.clone();
    amortizing
        .pool
        .reinvestment_period
        .as_mut()
        .expect("window")
        .amortizing_tranches = vec!["SR".to_string()];

    let inside = |result: &SimulationRun| {
        result.tranches["SR"]
            .principal_flows
            .iter()
            .filter(|(date, _)| *date <= window_end)
            .map(|(_, amount)| amount.amount())
            .sum::<f64>()
    };
    let no_window = deterministic(&base, &market);
    let with_window = deterministic(&retained, &market);
    let with_amortizing = deterministic(&amortizing, &market);
    assert!(
        inside(&no_window) > 1_000.0,
        "prepayments pay the note down without a window"
    );
    assert_eq!(
        inside(&with_window),
        0.0,
        "the window retains principal in the funding account"
    );
    assert!(
        inside(&with_amortizing) > 1_000.0,
        "an amortizing note is paid down inside the window"
    );
    assert!(
        (total(&with_window.tranches["SR"].principal_flows) - 800_000.0).abs() < 1e-6,
        "retained principal reaches the note after the window"
    );
}
