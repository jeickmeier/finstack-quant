//! Accrued interest calculator tests via the metrics framework.
//!
//! These tests validate accrued interest calculation through the `MetricId::Accrued`
//! interface. For direct tests of the underlying `accrual::accrued_interest_amount()`
//! function (including Linear vs Compounded, ex-coupon, and amortizing scenarios),
//! see `../bond_accrued_interest.rs`.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::{Instrument, PricingOverrides};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn create_curve(base_date: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (5.0, 0.80)])
        .build()
        .unwrap();
    MarketContext::new().insert(curve)
}

#[test]
fn test_accrued_at_issue() {
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "ACCR1",
        Money::new(100.0, Currency::USD),
        0.06,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    let market = create_curve(as_of);
    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Accrued],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let accrued = *result.measures.get("accrued").unwrap();
    assert_eq!(accrued, 0.0);
}

#[test]
fn test_accrued_mid_period() {
    let issue = date!(2025 - 01 - 01);
    let mid = date!(2025 - 04 - 01); // ~3 months later
    let bond = Bond::fixed(
        "ACCR2",
        Money::new(100.0, Currency::USD),
        0.06,
        issue,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    let market = create_curve(mid);
    let result = bond
        .price_with_metrics(
            &market,
            mid,
            &[MetricId::Accrued],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let accrued = *result.measures.get("accrued").unwrap();
    assert!(accrued > 0.0 && accrued < 3.0); // Semi-annual 6% = 3% per period
}

#[test]
fn test_quoted_price_accrued_uses_settlement_date() {
    let issue = date!(2025 - 01 - 01);
    let as_of = date!(2025 - 04 - 01);
    let mut bond = Bond::fixed(
        "ACCR-QUOTE",
        Money::new(1_000.0, Currency::USD),
        0.06,
        issue,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(98.5);

    let market = create_curve(as_of);
    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Accrued,
                MetricId::CleanPrice,
                MetricId::DirtyPrice,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let accrued = *result.measures.get("accrued").unwrap();
    let clean = *result.measures.get("clean_price").unwrap();
    let dirty = *result.measures.get("dirty_price").unwrap();

    assert!(
        (accrued - (dirty - clean)).abs() < 1e-9,
        "quoted bond accrued should be settlement-date accrued: accrued={accrued}, dirty-clean={}",
        dirty - clean
    );
}

#[test]
fn test_accrued_frn_uses_forward_rate() {
    // FRN accrued should use forward rate from the last reset period
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let issue = date!(2025 - 01 - 01);
    let as_of = date!(2025 - 03 - 15); // mid-quarter

    // Build market with discount and forward curve
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (1.0, 0.99)])
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(issue)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (10.0, 0.05)]) // 5% flat
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc).insert(fwd);

    // Floating-rate bond with SOFR 3M (using new CashflowSpec)
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    use finstack_quant_valuations::instruments::pricing_overrides::PricingOverrides;
    use finstack_quant_valuations::instruments::Attributes;
    let bond = Bond::builder()
        .id("FRN1".into())
        .notional(Money::new(100.0, Currency::USD))
        .issue_date(issue)
        .maturity(date!(2026 - 01 - 01))
        .cashflow_spec(
            CashflowSpec::floating_with_reset_lag(
                CurveId::new("USD-SOFR-3M"),
                0.0,
                Tenor::quarterly(),
                DayCount::Act360,
                0,
            )
            .expect("finite test rate"),
        )
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id_opt(None)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Accrued],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let accrued = *result.measures.get("accrued").unwrap();

    // Expect non-zero accrued consistent with ~5% * fraction of quarter * notional
    assert!(accrued > 0.0, "FRN accrued should be > 0");
}

#[test]
fn test_clean_dirty_ex_coupon_parity() {
    // Clean = Dirty - Accrued; around ex-coupon date accrued should reset
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    let issue = date!(2025 - 01 - 01);
    // let coup = date!(2025 - 07 - 01);
    let as_of_before = date!(2025 - 06 - 28);
    let as_of_after = date!(2025 - 07 - 02);

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(curve);

    let bond = Bond::fixed(
        "B",
        Money::new(100.0, Currency::USD),
        0.06,
        issue,
        date!(2026 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    // Before coupon date: accrued > 0
    let res_before = bond
        .price_with_metrics(
            &market,
            as_of_before,
            &[
                MetricId::Accrued,
                MetricId::DirtyPrice,
                MetricId::CleanPrice,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let acc_before = *res_before.measures.get("accrued").unwrap();
    // Use base value as dirty price when no quoted clean is provided
    let dirty_before = res_before.value.amount();
    let clean_before = *res_before.measures.get("clean_price").unwrap();
    assert!((clean_before - (dirty_before - acc_before)).abs() < 1e-6); // clean = dirty - accrued identity

    // After coupon date: check parity and ensure accrued decreased
    let res_after = bond
        .price_with_metrics(
            &market,
            as_of_after,
            &[MetricId::Accrued, MetricId::CleanPrice],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let acc_after = *res_after.measures.get("accrued").unwrap();
    let clean_after = *res_after.measures.get("clean_price").unwrap();
    let dirty_after = res_after.value.amount();
    assert!((clean_after - (dirty_after - acc_after)).abs() < 1e-6); // clean = dirty - accrued identity
    assert!(
        acc_after < acc_before,
        "Accrued should decrease after coupon"
    );
}
