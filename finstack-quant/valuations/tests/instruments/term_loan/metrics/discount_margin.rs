//! Discount margin tests for callable / non-callable term loans.
//!
//! DM convention under test (Fabozzi; Bloomberg YAS): cashflows are projected
//! at the contractual margin and the DM is a spread added to the discount
//! rate, so PV is strictly decreasing in DM. The solved DM is the full spread
//! over the loan's discount curve, directly comparable to the contractual
//! margin.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_cashflows::builder::FloatingRateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::pricing_overrides::{
    InstrumentPricingOverrides, MarketQuoteOverrides,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

fn build_floating_loan(
    call_schedule: Option<LoanCallSchedule>,
    overrides: InstrumentPricingOverrides,
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
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
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
        .instrument_pricing_overrides(overrides)
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
    let loan = build_floating_loan(None, InstrumentPricingOverrides::default());
    let market = build_market();
    let as_of = date!(2025 - 01 - 01);

    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("DM should succeed for non-callable loan");

    let dm = *result.measures.get("discount_margin").unwrap();
    // Without a quoted price the target is the model PV, so the curve DM
    // (spread over the loan's own discount curve) must solve to ~zero.
    assert!(
        dm.abs() < 1e-6,
        "curve DM vs the model PV must be ~0, got {dm}"
    );
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
    let loan = build_floating_loan(Some(call_schedule), InstrumentPricingOverrides::default());
    let market = build_market();
    let as_of = date!(2025 - 01 - 01);

    let result = loan.price_with_metrics(
        &market,
        as_of,
        &[MetricId::DiscountMargin],
        finstack_quant_valuations::instruments::PricingOptions::default(),
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
    let overrides = InstrumentPricingOverrides {
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
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("DM should succeed when quoted_clean_price is set");

    let dm = *result.measures.get("discount_margin").unwrap();
    // Coupons project at ~7% (4.5% index + 250 bp) while the model discounts
    // at 5%, so the model PV sits well above the 99 quote: the solved DM must
    // be a substantial positive spread, never negative or ~0.
    assert!(
        dm > 0.0 && dm < 0.20,
        "below-par quote must solve to a positive DM, got {dm}"
    );
}

// ---------------------------------------------------------------------------
// Flat, self-consistent market: discount curve == projection curve, both at
// `rate` with quarterly compounding on the Act/360 axis. On this market a
// loan quoted at par must solve to DM == contractual margin.
// ---------------------------------------------------------------------------

fn flat_consistent_market(as_of: time::Date, rate: f64) -> MarketContext {
    // DF(t) = (1 + rate/4)^(-4t): with log-linear interpolation two knots
    // reproduce the exponential exactly, so the quarterly-compounded zero
    // rate equals `rate` at every t.
    let df_10y = (1.0 + rate / 4.0_f64).powf(-40.0);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .interp(InterpStyle::LogLinear)
        .knots([(0.0, 1.0), (10.0, df_10y)])
        .build()
        .expect("flat discount curve should build");
    let fwd = ForwardCurve::builder("USD-SOFR", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .expect("flat forward curve should build");
    MarketContext::new().insert(disc).insert(fwd)
}

/// Non-callable floating loan with a quoted clean price on the flat
/// consistent market (default T+2 settlement; the tiny settlement carry is
/// absorbed by the pinning tolerance).
fn solve_dm_at_clean_price(margin_bp: i64, clean_px: f64) -> f64 {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2028 - 01 - 01);
    let overrides = InstrumentPricingOverrides {
        market_quotes: MarketQuoteOverrides {
            quoted_clean_price: Some(clean_px),
            ..Default::default()
        },
        ..Default::default()
    };
    let loan = TermLoan::builder()
        .id("TL-DM-PAR-PIN".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Floating(FloatingRateSpec {
            index_id: CurveId::from("USD-SOFR"),
            spread_bp: Decimal::from(margin_bp),
            gearing: Decimal::from(1),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
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
        .instrument_pricing_overrides(overrides)
        .attributes(Default::default())
        .build()
        .expect("floating loan construction should succeed");

    let market = flat_consistent_market(as_of, 0.045);
    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("DM metric should converge on the flat consistent market");
    result.measures["discount_margin"]
}

/// Pinned external value (Fabozzi / Bloomberg YAS convention): a floating term
/// loan quoted at par on a flat curve, with the discount curve equal to the
/// projection curve, must solve to a DM equal to its contractual margin — not
/// zero and not its negative. Small tolerance covers day-count / period-
/// compounding residuals (Act/360 quarterly accruals vs the m=4 zero-rate
/// shift).
#[test]
fn test_dm_at_par_equals_contractual_margin_on_flat_consistent_curve() {
    let contractual_margin = 0.025; // 250 bp
    let dm = solve_dm_at_clean_price(250, 100.0);
    assert!(
        (dm - contractual_margin).abs() < 2e-4,
        "loan at par on a flat consistent curve must have DM ~= contractual margin \
         (expected ~{contractual_margin}, got {dm})"
    );
}

/// Sign convention: PV is strictly decreasing in DM, so a loan quoted below
/// par must solve to a DM strictly above its contractual margin, and above
/// par strictly below.
#[test]
fn test_dm_sign_convention_below_and_above_par() {
    let contractual_margin = 0.025; // 250 bp

    let dm_discount = solve_dm_at_clean_price(250, 98.0);
    assert!(
        dm_discount > contractual_margin,
        "loan quoted below par must carry DM strictly above its contractual margin \
         (margin {contractual_margin}, got {dm_discount})"
    );

    let dm_premium = solve_dm_at_clean_price(250, 102.0);
    assert!(
        dm_premium < contractual_margin,
        "loan quoted above par must carry DM strictly below its contractual margin \
         (margin {contractual_margin}, got {dm_premium})"
    );

    // The two quotes must straddle the at-par DM with meaningful width.
    assert!(
        dm_discount - dm_premium > 1e-3,
        "DM spread between 98 and 102 quotes should exceed 10 bp, got {}",
        dm_discount - dm_premium
    );
}
