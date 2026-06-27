//! Public-API example: return-floor (guaranteed minimum MOIC / XIRR).
//!
//! This integration test doubles as a readable example script demonstrating
//! how to declare, price, and verify a private-credit-style loan with a return
//! floor. It uses **only** public APIs — no `pub(crate)` internals.
//!
//! # What is a return floor?
//!
//! A return floor is an **issuer-side, call-protection-only** term. It
//! guarantees that on any early issuer-called or prepaid redemption, the
//! investor's realized MOIC or XIRR (measured from issue date and issue price)
//! meets a stated target. It does **not** guarantee the held-to-maturity return
//! — the maturity path is always unfloored.
//!
//! # Scenario
//!
//! - 5-year private-credit loan, $1,000,000 notional, 10% annual coupon.
//! - 1.25× MOIC floor, active from year 2 onward (2-year no-call period).
//! - Priced flat at a 6% discount rate.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::{
    Bond, ProtectionWindow, ReturnFloorSpec,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

/// Build a flat continuously-compounded discount curve (Act/365F knots).
fn flat_curve(id: &str, rate: f64, as_of: finstack_quant_core::dates::Date) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (3.0, (-rate * 3.0).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

/// Construct a $1M 5-year 10% fixed loan with a 1.25× MOIC floor after year 2.
///
/// The floor is active from 2027-01-01 to maturity (2030-01-01), encoding a
/// standard 2-year no-call ("NC-2") private-credit structure. Any issuer
/// prepayment on or after 2027-01-01 must pay at least the floor-computed
/// redemption price, ensuring the investor earns at least 1.25× their invested
/// capital.
fn build_floored_loan() -> Bond {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let nc2_end = date!(2027 - 01 - 01);

    Bond::fixed(
        "LOAN-FLOOR-001",
        Money::new(1_000_000.0, Currency::USD),
        0.10, // 10% coupon
        issue,
        maturity,
        "USD-OIS",
    )
    .expect("loan construction should succeed")
    .with_return_floor(ReturnFloorSpec::moic(1.25).window(ProtectionWindow::From(nc2_end)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: construction is valid and produces a positive PV
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn return_floor_loan_prices_successfully() {
    let as_of = date!(2025 - 01 - 01);
    let loan = build_floored_loan();
    let market = MarketContext::new().insert(flat_curve("USD-OIS", 0.06, as_of));

    let pv = loan
        .value(&market, as_of)
        .expect("floored loan should price without error");

    println!(
        "[return_floor_example] PV = {:.2} {}",
        pv.amount(),
        pv.currency()
    );

    // A 10%-coupon loan discounted at 6% should price above par.
    assert!(
        pv.amount() > 1_000_000.0,
        "10% coupon / 6% discount rate → should price above par, got {:.2}",
        pv.amount()
    );
    assert_eq!(pv.currency(), Currency::USD);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: return metrics are sane and internally consistent
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn return_floor_metrics_are_sane() {
    let as_of = date!(2025 - 01 - 01);
    let loan = build_floored_loan();
    let market = MarketContext::new().insert(flat_curve("USD-OIS", 0.06, as_of));

    // Request all four return metrics in a single call.
    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Moic,
                MetricId::MoicToWorst,
                MetricId::Xirr,
                MetricId::XirrToWorst,
            ],
            PricingOptions::default(),
        )
        .expect("metrics computation should succeed");

    let moic = *result.measures.get(MetricId::Moic.as_str()).unwrap();
    let moic_tw = *result.measures.get(MetricId::MoicToWorst.as_str()).unwrap();
    let xirr = *result.measures.get(MetricId::Xirr.as_str()).unwrap();
    let xirr_tw = *result.measures.get(MetricId::XirrToWorst.as_str()).unwrap();

    // ── Print values so the test doubles as a readable example ────────────────
    println!("[return_floor_example] MOIC             = {moic:.4}x");
    println!("[return_floor_example] MOIC to worst    = {moic_tw:.4}x");
    println!(
        "[return_floor_example] XIRR             = {:.2}%",
        xirr * 100.0
    );
    println!(
        "[return_floor_example] XIRR to worst    = {:.2}%",
        xirr_tw * 100.0
    );

    // ── Sanity bounds ─────────────────────────────────────────────────────────

    // A 5-year 10% coupon bullet at par: 5 × 10% coupons + 100% notional = 150%
    // total inflow → MOIC = 150/100 = 1.50. Allow some variation from conventions.
    assert!(
        moic > 1.0 && moic < 2.0,
        "MOIC should be in (1.0, 2.0) for a 5y 10% par loan, got {moic:.4}"
    );

    // XIRR of a ~par 10% loan should be close to 10%.
    assert!(
        xirr > 0.08 && xirr < 0.12,
        "XIRR should be near 10% for a 5y 10% coupon bond, got {:.4}",
        xirr
    );

    // ── Floor call-protection semantics ──────────────────────────────────────
    //
    // MoicToWorst is the MINIMUM over ALL exit paths, including the unfloored
    // maturity path. For a bullet bond this equals MOIC (no calls to exercise).
    // With the return floor lowered into a call schedule, early-call paths are
    // floored but maturity is not. For a bullet bond with no contractual calls
    // the floor adds synthetically callable paths; the maturity path still
    // dominates as the "worst" when there are no natural call economics.
    assert!(
        moic_tw <= moic + 1e-9,
        "MoicToWorst ({moic_tw:.4}) must be ≤ Moic ({moic:.4})"
    );
    assert!(
        xirr_tw <= xirr + 1e-9,
        "XirrToWorst ({xirr_tw:.4}) must be ≤ Xirr ({xirr:.4})"
    );

    // Both metrics must be positive (investor is not in a loss position on a
    // par-issued 10% coupon loan).
    assert!(
        moic_tw > 0.0,
        "MoicToWorst must be positive, got {moic_tw:.4}"
    );
    assert!(
        xirr_tw > -1.0,
        "XirrToWorst must be > -100%, got {xirr_tw:.4}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: the min_moic shortcut produces an equivalent spec
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn min_moic_shortcut_equivalent_to_explicit_full_window_spec() {
    let as_of = date!(2025 - 01 - 01);
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let market = MarketContext::new().insert(flat_curve("USD-OIS", 0.06, as_of));

    // Shortcut: .min_moic(1.25) → ProtectionWindow::Full
    let loan_shortcut = Bond::fixed(
        "LOAN-SHORTCUT",
        Money::new(1_000_000.0, Currency::USD),
        0.10,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap()
    .min_moic(1.25);

    // Explicit: ReturnFloorSpec::moic(1.25) with Full window
    let loan_explicit = Bond::fixed(
        "LOAN-EXPLICIT",
        Money::new(1_000_000.0, Currency::USD),
        0.10,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap()
    .with_return_floor(ReturnFloorSpec::moic(1.25));

    let pv_shortcut = loan_shortcut.value(&market, as_of).unwrap();
    let pv_explicit = loan_explicit.value(&market, as_of).unwrap();

    println!(
        "[return_floor_example] min_moic shortcut PV = {:.2}",
        pv_shortcut.amount()
    );
    println!(
        "[return_floor_example] explicit spec PV     = {:.2}",
        pv_explicit.amount()
    );

    assert!(
        (pv_shortcut.amount() - pv_explicit.amount()).abs() < 1e-6,
        "min_moic shortcut and explicit ReturnFloorSpec::moic should give identical PV"
    );
}
