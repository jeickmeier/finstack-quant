//! Attachment points derived from balances: a note built without points takes
//! its balance share in payment-priority order, declared points must still
//! reconcile with the shares, and a derived structure prices exactly like
//! its declared twin (deterministic flows and stochastic tranche reporting).

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AssetPool, DealType, PoolAsset, PricingMode, StructuredCredit, Tranche,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn maturity() -> Date {
    d(2032, 1, 1)
}

fn note(id: &str, seniority: TrancheSeniority, balance: f64, rate: f64) -> Tranche {
    Tranche::from_balance(
        id,
        seniority,
        usd(balance),
        TrancheCoupon::Fixed { rate },
        maturity(),
    )
    .expect("tranche")
}

fn declared(
    id: &str,
    a: f64,
    b: f64,
    seniority: TrancheSeniority,
    balance: f64,
    rate: f64,
) -> Tranche {
    Tranche::new(
        id,
        a,
        b,
        seniority,
        usd(balance),
        TrancheCoupon::Fixed { rate },
        maturity(),
    )
    .expect("tranche")
}

fn points(structure: &TrancheStructure) -> Vec<(String, f64, f64)> {
    structure
        .tranches
        .iter()
        .map(|t| (t.id.to_string(), t.attachment_pct(), t.detachment_pct()))
        .collect()
}

#[test]
fn derived_points_follow_balance_shares_in_priority_order() {
    let structure = TrancheStructure::new(vec![
        note("A", TrancheSeniority::Senior, 60_000_000.0, 0.05),
        note("B", TrancheSeniority::Mezzanine, 30_000_000.0, 0.07),
        note("E", TrancheSeniority::Equity, 10_000_000.0, 0.0),
    ])
    .expect("structure");

    assert_eq!(
        points(&structure),
        vec![
            ("A".to_string(), 40.0, 100.0),
            ("B".to_string(), 10.0, 40.0),
            ("E".to_string(), 0.0, 10.0),
        ]
    );
    assert!(structure.tranches[2].is_first_loss());
    assert_eq!(structure.tranches[0].thickness(), 60.0);

    // Input order does not matter: the shares stack by payment priority.
    let shuffled = TrancheStructure::new(vec![
        note("E", TrancheSeniority::Equity, 10_000_000.0, 0.0),
        note("A", TrancheSeniority::Senior, 60_000_000.0, 0.05),
        note("B", TrancheSeniority::Mezzanine, 30_000_000.0, 0.07),
    ])
    .expect("structure");
    assert_eq!(
        points(&shuffled),
        vec![
            ("E".to_string(), 0.0, 10.0),
            ("A".to_string(), 40.0, 100.0),
            ("B".to_string(), 10.0, 40.0),
        ]
    );

    // Pari-passu seniors stack in input order, and the top detaches at 100
    // even when the shares do not sum exactly.
    let pari = TrancheStructure::new(vec![
        note("A1", TrancheSeniority::Senior, 33_333_333.0, 0.05),
        note("A2", TrancheSeniority::Senior, 33_333_333.0, 0.05),
        note("E", TrancheSeniority::Equity, 33_333_334.0, 0.0),
    ])
    .expect("structure");
    let resolved = points(&pari);
    assert_eq!(resolved[2].1, 0.0);
    assert!((resolved[1].1 - resolved[2].2).abs() < 1e-12);
    assert!((resolved[0].1 - resolved[1].2).abs() < 1e-12);
    assert_eq!(resolved[0].2, 100.0);
}

#[test]
fn from_balances_discards_declared_points_and_missing_points_are_filled() {
    // Declared points that do not match the balance shares are rejected ...
    let err = TrancheStructure::new(vec![
        declared(
            "A",
            50.0,
            100.0,
            TrancheSeniority::Senior,
            60_000_000.0,
            0.05,
        ),
        declared("E", 0.0, 50.0, TrancheSeniority::Equity, 40_000_000.0, 0.0),
    ])
    .expect_err("thickness 50% vs balance share 60%");
    assert!(err.to_string().contains("balance shares"), "{err}");

    // ... unless the caller asks for balance-derived points outright.
    let derived = TrancheStructure::from_balances(vec![
        declared(
            "A",
            50.0,
            100.0,
            TrancheSeniority::Senior,
            60_000_000.0,
            0.05,
        ),
        declared("E", 0.0, 50.0, TrancheSeniority::Equity, 40_000_000.0, 0.0),
    ])
    .expect("derived");
    assert_eq!(
        points(&derived),
        vec![("A".to_string(), 40.0, 100.0), ("E".to_string(), 0.0, 40.0)]
    );

    // A declared note next to derived ones is validated against the shares.
    let mixed = TrancheStructure::new(vec![
        declared(
            "A",
            40.0,
            100.0,
            TrancheSeniority::Senior,
            60_000_000.0,
            0.05,
        ),
        note("B", TrancheSeniority::Mezzanine, 30_000_000.0, 0.07),
        note("E", TrancheSeniority::Equity, 10_000_000.0, 0.0),
    ])
    .expect("mixed");
    assert_eq!(points(&mixed)[1], ("B".to_string(), 10.0, 40.0));
    assert!(TrancheStructure::new(vec![
        declared(
            "A",
            45.0,
            100.0,
            TrancheSeniority::Senior,
            60_000_000.0,
            0.05
        ),
        note("B", TrancheSeniority::Mezzanine, 30_000_000.0, 0.07),
        note("E", TrancheSeniority::Equity, 10_000_000.0, 0.0),
    ])
    .is_err());

    // A builder with only one of the two points is rejected.
    assert!(Tranche::builder()
        .id("X")
        .attachment_detachment(0.0, 10.0)
        .seniority(TrancheSeniority::Equity)
        .balance(usd(1.0))
        .coupon(TrancheCoupon::Fixed { rate: 0.0 })
        .maturity(maturity())
        .build()
        .is_ok());
    let alone = Tranche::builder()
        .id("X")
        .seniority(TrancheSeniority::Equity)
        .balance(usd(1.0))
        .coupon(TrancheCoupon::Fixed { rate: 0.0 })
        .maturity(maturity())
        .build()
        .expect("no points");
    assert_eq!(alone.attachment_point, None);

    // JSON without points round-trips through the derivation as well.
    let json = serde_json::to_string(&derived).expect("serialize");
    let parsed: TrancheStructure = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(points(&parsed), points(&derived));
}

fn deal(tranches: TrancheStructure) -> StructuredCredit {
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
    let mut deal = StructuredCredit::new_clo(
        "CLO-AP",
        pool,
        tranches,
        d(2024, 1, 1),
        maturity(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.10);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.03);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 12);
    deal
}

fn market() -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(d(2024, 1, 1))
        .knots((0..=12).map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp())))
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// The derived structure and its declared twin are the same deal: identical
/// deterministic tranche flows and identical stochastic attachment /
/// detachment reporting.
#[test]
fn derived_structure_prices_like_the_declared_one() {
    let declared_structure = TrancheStructure::new(vec![
        declared("E", 0.0, 10.0, TrancheSeniority::Equity, 10_000_000.0, 0.0),
        declared(
            "B",
            10.0,
            40.0,
            TrancheSeniority::Mezzanine,
            30_000_000.0,
            0.07,
        ),
        declared(
            "A",
            40.0,
            100.0,
            TrancheSeniority::Senior,
            60_000_000.0,
            0.05,
        ),
    ])
    .expect("declared");
    let derived_structure = TrancheStructure::new(vec![
        note("E", TrancheSeniority::Equity, 10_000_000.0, 0.0),
        note("B", TrancheSeniority::Mezzanine, 30_000_000.0, 0.07),
        note("A", TrancheSeniority::Senior, 60_000_000.0, 0.05),
    ])
    .expect("derived");
    assert_eq!(points(&declared_structure), points(&derived_structure));

    let as_of = d(2024, 1, 1);
    let market = market();
    let declared_flows =
        run_simulation(&deal(declared_structure.clone()), &market, as_of).expect("run");
    let derived_flows =
        run_simulation(&deal(derived_structure.clone()), &market, as_of).expect("run");
    for id in ["A", "B", "E"] {
        assert_eq!(
            declared_flows[id].cashflows, derived_flows[id].cashflows,
            "{id}"
        );
    }

    let mode = PricingMode::MonteCarlo {
        num_paths: 16,
        antithetic: true,
    };
    let mut declared_deal = deal(declared_structure);
    declared_deal
        .enable_stochastic_defaults()
        .expect("stochastic");
    let mut derived_deal = deal(derived_structure);
    derived_deal
        .enable_stochastic_defaults()
        .expect("stochastic");
    let declared_result = declared_deal
        .price_stochastic_with_mode(&market, as_of, mode.clone())
        .expect("price");
    let derived_result = derived_deal
        .price_stochastic_with_mode(&market, as_of, mode)
        .expect("price");
    for (left, right) in declared_result
        .tranche_results
        .iter()
        .zip(&derived_result.tranche_results)
    {
        assert_eq!(left.tranche_id, right.tranche_id);
        assert_eq!(left.attachment, right.attachment);
        assert_eq!(left.detachment, right.detachment);
        assert_eq!(left.npv, right.npv);
    }
    assert_eq!(declared_result.npv, derived_result.npv);
}
