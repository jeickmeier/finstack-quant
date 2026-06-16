//! Amortizing bond integration tests.

use finstack_quant_cashflows::builder::AmortizationSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

fn create_curve(base_date: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .unwrap();
    MarketContext::new().insert(curve)
}

#[test]
fn test_linear_amortization() {
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::builder()
        .id("AMORT_LINEAR".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .cashflow_spec(CashflowSpec::amortizing(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
            AmortizationSpec::LinearTo {
                final_notional: Money::new(400.0, Currency::USD),
            },
        ))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let market = create_curve(as_of);
    let pv = bond.value(&market, as_of);

    // Amortizing bond should price successfully
    assert!(pv.is_ok(), "Amortizing bond should price without error");

    let pv_amount = pv.unwrap().amount();

    // PV should be finite
    assert!(pv_amount.is_finite(), "PV should be finite");
}

#[test]
fn test_full_amortization() {
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::builder()
        .id("AMORT_FULL".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .cashflow_spec(CashflowSpec::amortizing(
            CashflowSpec::fixed(0.06, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
            AmortizationSpec::LinearTo {
                final_notional: Money::new(0.0, Currency::USD),
            },
        ))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let market = create_curve(as_of);
    let pv = bond.value(&market, as_of);

    // Fully amortizing bond should price successfully
    assert!(
        pv.is_ok(),
        "Fully amortizing bond should price without error"
    );

    let pv_amount = pv.unwrap().amount();

    // PV should be finite
    assert!(pv_amount.is_finite(), "PV should be finite");
}
