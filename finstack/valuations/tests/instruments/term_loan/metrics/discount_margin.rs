//! Discount margin tests for callable / non-callable term loans.

use finstack_cashflows::builder::specs::CouponType;
use finstack_cashflows::builder::FloatingRateSpec;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, RateSpec, TermLoan,
};
use finstack_valuations::instruments::pricing_overrides::{MarketQuoteOverrides, PricingOverrides};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

fn build_floating_loan(
    call_schedule: Option<LoanCallSchedule>,
    overrides: PricingOverrides,
) -> TermLoan {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2028 - 01 - 01);
    TermLoan::builder()
        .id("TL-DM-TEST".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Floating(FloatingRateSpec {
            index_id: CurveId::from("USD-SOFR"),
            spread_bp: Decimal::from(250),
            gearing: Decimal::from(1),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
            payment_lag_days: 0,
        }))
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(overrides)
        .call_schedule_opt(call_schedule)
        .attributes(Default::default())
        .build()
        .expect("floating loan construction should succeed")
}

fn build_market() -> MarketContext {
    let as_of = date!(2025 - 01 - 01);
    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = ForwardCurve::builder("USD-SOFR", 0.25)
        .base_date(as_of)
        .knots([(0.0, 0.045), (3.0, 0.045), (10.0, 0.045)])
        .build()
        .expect("forward curve");
    MarketContext::new().insert(disc_curve).insert(fwd_curve)
}

/// DM should work on non-callable floating-rate loans without a quoted price.
#[test]
fn test_dm_non_callable_succeeds() {
    let loan = build_floating_loan(None, PricingOverrides::default());
    let market = build_market();
    let as_of = date!(2025 - 01 - 01);

    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("DM should succeed for non-callable loan");

    let dm = *result.measures.get("discount_margin").unwrap();
    assert!(dm.is_finite(), "DM should be finite, got {dm}");
}

/// DM should reject callable floating loans without quoted_clean_price.
#[test]
fn test_dm_callable_without_price_rejects() {
    let call_schedule = LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2026 - 07 - 01),
            price_pct_of_par: 101.0,
            call_type: Default::default(),
        }],
    };
    let loan = build_floating_loan(Some(call_schedule), PricingOverrides::default());
    let market = build_market();
    let as_of = date!(2025 - 01 - 01);

    let result = loan.price_with_metrics(
        &market,
        as_of,
        &[MetricId::DiscountMargin],
        finstack_valuations::instruments::PricingOptions::default(),
    );

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("DiscountMargin requires quoted_clean_price"),
                "Error should mention callable + quoted_clean_price, got: {msg}"
            );
        }
        Ok(r) => {
            // If the metric itself errored but pricing succeeded,
            // the error may be in the measures map or the result may be Ok
            // but missing the DM key. Either way, DM should not silently succeed.
            assert!(
                r.measures.get("discount_margin").is_none(),
                "DM should not silently succeed for callable loan without quoted price"
            );
        }
    }
}

/// DM should work on callable floating loans when quoted_clean_price is set.
#[test]
fn test_dm_callable_with_quoted_price_succeeds() {
    let call_schedule = LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2026 - 07 - 01),
            price_pct_of_par: 101.0,
            call_type: Default::default(),
        }],
    };
    let overrides = PricingOverrides {
        market_quotes: MarketQuoteOverrides {
            quoted_clean_price: Some(99.0),
            ..Default::default()
        },
        ..Default::default()
    };
    let loan = build_floating_loan(Some(call_schedule), overrides);
    let market = build_market();
    let as_of = date!(2025 - 01 - 01);

    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("DM should succeed when quoted_clean_price is set");

    let dm = *result.measures.get("discount_margin").unwrap();
    assert!(dm.is_finite(), "DM should be finite, got {dm}");
}
