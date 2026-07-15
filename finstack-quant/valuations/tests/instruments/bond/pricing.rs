//! Bond pricing engine tests.
//!
//! Tests the core pricing functionality including:
//! - Basic present value calculation
//! - Settlement date conventions
//! - Theta (time decay)
//! - Price sensitivity to curve shifts

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

use crate::common::test_helpers::tolerances;

/// Expected deviation from par for bonds with semi-annual coupon priced on
/// continuous compounding curve.
///
/// **Reason for deviation:** A 5% coupon bond pays semi-annual coupons at
/// 2.5% per period, which corresponds to an effective annual yield of
/// (1.025)^2 - 1 = 5.0625%. When discounted on a continuously compounded 5%
/// curve (effective yield = e^0.05 - 1 = 5.127%), there's a ~12bp mismatch
/// that compounds over the bond's life.
///
/// **This is intentional behavior.** The test validates the pricing engine,
/// not convention alignment. For fully convention-aligned tests, construct
/// curves with matching compounding conventions.
const PAR_BOND_COMPOUNDING_DEVIATION: f64 = 3.0; // ~0.3% on $1000 notional

fn create_flat_curve(rate: f64, base_date: Date, id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

#[test]
fn test_bond_basic_pricing() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond = Bond::fixed(
        "PRICE_TEST",
        Money::new(1000.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv = bond.value(&market, as_of).unwrap();

    // At 5% curve with 5% coupon, should be near par.
    // See PAR_BOND_COMPOUNDING_DEVIATION for explanation of expected deviation.
    assert!(
        (pv.amount() - 1000.0).abs() < PAR_BOND_COMPOUNDING_DEVIATION,
        "Par bond should price near par, got {} (expected deviation < {:.1})",
        pv.amount(),
        PAR_BOND_COMPOUNDING_DEVIATION
    );
    assert_eq!(pv.currency(), Currency::USD);
}

#[test]
fn test_bond_price_vs_yield() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond = Bond::fixed(
        "YIELD_TEST",
        Money::new(1000.0, Currency::USD),
        0.06,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Low yield → high price
    let low_curve = create_flat_curve(0.04, as_of, "USD-OIS");
    let market_low = MarketContext::new().insert(low_curve);
    let pv_low = bond.value(&market_low, as_of).unwrap();

    // High yield → low price
    let high_curve = create_flat_curve(0.08, as_of, "USD-OIS");
    let market_high = MarketContext::new().insert(high_curve);
    let pv_high = bond.value(&market_high, as_of).unwrap();

    // Verify inverse relationship
    assert!(pv_low.amount() > pv_high.amount());
}

#[test]
fn test_bond_price_coupon_relationship() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    // Bond with above-market coupon
    let high_coupon = Bond::fixed(
        "HIGH_COUPON",
        Money::new(1000.0, Currency::USD),
        0.08,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Bond with below-market coupon
    let low_coupon = Bond::fixed(
        "LOW_COUPON",
        Money::new(1000.0, Currency::USD),
        0.03,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv_high = high_coupon.value(&market, as_of).unwrap();
    let pv_low = low_coupon.value(&market, as_of).unwrap();

    // Higher coupon → higher price
    assert!(pv_high.amount() > pv_low.amount());

    // High coupon above market rate → premium
    assert!(pv_high.amount() > 1000.0);

    // Low coupon below market rate → discount
    assert!(pv_low.amount() < 1000.0);
}

#[test]
fn test_bond_price_maturity_relationship() {
    let as_of = date!(2025 - 01 - 01);

    let bond_2y = Bond::fixed(
        "2Y",
        Money::new(1000.0, Currency::USD),
        0.05,
        as_of,
        date!(2027 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let bond_10y = Bond::fixed(
        "10Y",
        Money::new(1000.0, Currency::USD),
        0.05,
        as_of,
        date!(2035 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv_2y = bond_2y.value(&market, as_of).unwrap();
    let pv_10y = bond_10y.value(&market, as_of).unwrap();

    // At flat curve with coupon = yield, both should be near par.
    // Using CURVE_PRICING tolerance (0.5%) for 2Y to account for semi-annual vs continuous
    // compounding convention mismatch between bond cashflows and discount curve.
    // For longer-dated bonds (10Y), the mismatch accumulates more, so we use a slightly
    // higher tolerance (0.6%).
    let notional = 1000.0;
    let tolerance_2y = notional * tolerances::CURVE_PRICING;
    let tolerance_10y = notional * 6e-3; // 0.6% for longer-dated bonds
    assert!(
        (pv_2y.amount() - notional).abs() < tolerance_2y,
        "2Y par bond deviation {:.2} exceeds {:.1}%",
        (pv_2y.amount() - notional).abs(),
        tolerances::CURVE_PRICING * 100.0
    );
    assert!(
        (pv_10y.amount() - notional).abs() < tolerance_10y,
        "10Y par bond deviation {:.2} exceeds {:.1}%",
        (pv_10y.amount() - notional).abs(),
        0.6
    );
}

#[test]
fn test_bond_price_zero_coupon() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond = Bond::fixed(
        "ZERO",
        Money::new(1000.0, Currency::USD),
        0.0,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv = bond.value(&market, as_of).unwrap();

    // Zero coupon bond should be priced at discount
    assert!(pv.amount() < 1000.0);

    // Expected: 1000 * exp(-0.05 * 5) = 778.8007...
    // Note: The test curve is constructed using continuous compounding (exp),
    // so the expected value uses the same logic. This validates that the
    // pricing engine correctly retrieves the discount factor from the curve.
    // Market standard zero-coupon bonds may use discrete compounding (e.g. simple interest),
    // which would require a curve constructed with those conventions.
    let expected = 1000.0 * (-0.05 * 5.0_f64).exp();
    assert!(
        (pv.amount() - expected).abs() < 1.0,
        "Zero coupon bond should price near {:.2}, got {:.2}",
        expected,
        pv.amount()
    );
}

#[test]
fn test_bond_theta_time_decay() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond = Bond::fixed(
        "THETA_TEST",
        Money::new(1000.0, Currency::USD),
        0.06,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Price at T
    let pv_t0 = bond.value(&market, as_of).unwrap();

    // Price at T+1 day
    let tomorrow = date!(2025 - 01 - 02);
    let curve_t1 = create_flat_curve(0.05, tomorrow, "USD-OIS");
    let market_t1 = MarketContext::new().insert(curve_t1);
    let pv_t1 = bond.value(&market_t1, tomorrow).unwrap();

    // Premium bond should decay toward par (price decrease)
    // But positive carry from coupon can offset this
    // Just verify prices are finite and reasonable
    assert!(pv_t0.amount().is_finite());
    assert!(pv_t1.amount().is_finite());
    assert!((pv_t0.amount() - pv_t1.amount()).abs() < 1.0); // Daily carry on $1000 bond is ~$0.15
}

#[test]
fn test_bond_settlement_date_impact() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    // No settlement lag
    let bond_t0 = Bond::builder()
        .id("SETTLE_T0".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(
                0.05,
                finstack_quant_core::dates::Tenor::semi_annual(),
                DayCount::Act365F,
            )
            .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .settlement_convention_opt(None)
        .build()
        .unwrap();

    // T+2 settlement
    let bond_t2 = Bond::builder()
        .id("SETTLE_T2".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(
                0.05,
                finstack_quant_core::dates::Tenor::semi_annual(),
                DayCount::Act365F,
            )
            .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .settlement_convention_opt(Some(
            finstack_quant_valuations::instruments::fixed_income::bond::BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            },
        ))
        .build()
        .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv_t0 = bond_t0.value(&market, as_of).unwrap();
    let pv_t2 = bond_t2.value(&market, as_of).unwrap();

    // T+2 settlement should result in slightly higher PV (less discounting)
    assert!(pv_t2.amount() >= pv_t0.amount());
}

#[test]
fn test_bond_matured_or_near_zero_value() {
    let as_of = date!(2025 - 01 - 01);
    let issue = date!(2020 - 01 - 01);
    let maturity = date!(2024 - 01 - 01); // Already matured

    let bond = Bond::fixed(
        "MATURED",
        Money::new(1000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Matured bonds have no future cashflows — returns zero PV.
    let result = bond.value(&market, as_of).unwrap();
    assert!(
        result.amount().abs() < 1e-12,
        "Matured bond should have zero PV, got {}",
        result.amount()
    );
}

#[test]
fn test_bond_near_maturity_pricing() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2025 - 02 - 01); // 1 month away

    let bond = Bond::fixed(
        "NEAR_MAT",
        Money::new(1000.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv = bond.value(&market, as_of).unwrap();

    // Very close to maturity, should be near par plus accrued.
    // Use CURVE_PRICING tolerance (0.5% of notional = 5.0 for $1000).
    let notional = 1000.0;
    assert!(
        (pv.amount() - notional).abs() < notional * tolerances::CURVE_PRICING,
        "Near-maturity bond deviation {:.2} exceeds tolerance",
        (pv.amount() - notional).abs()
    );
}

#[test]
fn test_bond_curve_parallel_shift() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond = Bond::fixed(
        "SHIFT_TEST",
        Money::new(1000.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Base curve at 5%
    let curve_base = create_flat_curve(0.05, as_of, "USD-OIS");
    let market_base = MarketContext::new().insert(curve_base);
    let pv_base = bond.value(&market_base, as_of).unwrap();

    // Curve shifted down 100bp to 4%
    let curve_down = create_flat_curve(0.04, as_of, "USD-OIS");
    let market_down = MarketContext::new().insert(curve_down);
    let pv_down = bond.value(&market_down, as_of).unwrap();

    // Curve shifted up 100bp to 6%
    let curve_up = create_flat_curve(0.06, as_of, "USD-OIS");
    let market_up = MarketContext::new().insert(curve_up);
    let pv_up = bond.value(&market_up, as_of).unwrap();

    // Price increases when rates fall
    assert!(pv_down.amount() > pv_base.amount());

    // Price decreases when rates rise
    assert!(pv_up.amount() < pv_base.amount());

    // Verify reasonable magnitudes
    let delta_down = pv_down.amount() - pv_base.amount();
    let delta_up = pv_base.amount() - pv_up.amount();

    // Due to convexity, down move has larger impact
    assert!(delta_down > delta_up * 0.9); // Allow some tolerance
}

#[test]
fn test_bond_price_consistency() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond = Bond::fixed(
        "CONSISTENT",
        Money::new(1000.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Price multiple times should give same result
    let pv1 = bond.value(&market, as_of).unwrap();
    let pv2 = bond.value(&market, as_of).unwrap();
    let pv3 = bond.value(&market, as_of).unwrap();

    assert_eq!(pv1.amount(), pv2.amount());
    assert_eq!(pv2.amount(), pv3.amount());
}

#[test]
fn test_bond_notional_scaling() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond_100 = Bond::fixed(
        "N100",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let bond_1000 = Bond::fixed(
        "N1000",
        Money::new(1000.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv_100 = bond_100.value(&market, as_of).unwrap();
    let pv_1000 = bond_1000.value(&market, as_of).unwrap();

    // Price should scale linearly with notional
    assert!((pv_1000.amount() / pv_100.amount() - 10.0).abs() < 0.01);
}

#[test]
fn test_bond_different_day_counts() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let day_counts = vec![
        DayCount::Thirty360,
        DayCount::Act360,
        DayCount::Act365F,
        DayCount::ActAct,
    ];

    for dc in day_counts {
        let bond = Bond::builder()
            .id(format!("DC_{:?}", dc).into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, finstack_quant_core::dates::Tenor::semi_annual(), dc)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        let disc_curve = create_flat_curve(0.05, as_of, "USD-OIS");
        let market = MarketContext::new().insert(disc_curve);

        let pv = bond.value(&market, as_of).unwrap();

        // All should produce valid, finite prices
        assert!(pv.amount().is_finite());
        assert!(pv.amount() > 0.0);
    }
}
