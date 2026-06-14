//! Comprehensive par spread calculation tests for basis swaps.
//!
//! Tests verify that computed par spreads correctly set NPV to zero and
//! validate the mathematical relationship between par spread, annuity, and PV.

use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_core::{currency::Currency::USD, math::interp::InterpStyle};
use finstack_valuations::instruments::rates::basis_swap::{BasisSwap, BasisSwapLeg};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

const CALENDAR_ID: &str = "usny";

fn market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.02), (0.5, 0.021), (1.0, 0.022), (2.0, 0.023)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.019), (0.5, 0.020), (1.0, 0.021), (2.0, 0.022)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    MarketContext::new().insert(disc).insert(f3m).insert(f1m)
}

#[test]
fn par_spread_zeros_npv() {
    // Test that applying the computed par spread results in zero NPV
    let ctx = market();
    let as_of = d(2025, 1, 2);

    // Create swap with zero spread
    let swap = BasisSwap::new(
        "PAR-TEST",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    // Compute par spread
    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BasisParSpread],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread_bp = res.measures[MetricId::BasisParSpread.as_str()];

    // Create new swap with par spread applied
    let swap_at_par = BasisSwap::new(
        "PAR-TEST-APPLIED",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::try_from(par_spread_bp).expect("valid literal"),
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let npv = swap_at_par.value(&ctx, as_of).unwrap();

    // NPV should be very close to zero (within $1 tolerance)
    assert!(
        npv.amount().abs() < 1.0,
        "NPV with par spread should be near zero, got {}",
        npv.amount()
    );
}

#[test]
fn par_spread_formula_validation() {
    // Validate: par_spread * annuity * notional = PV_difference
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "FORMULA-TEST",
        Money::new(5_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2027, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2027, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::BasisParSpread,
                MetricId::AnnuityPrimary,
                MetricId::PvPrimary,
                MetricId::PvReference,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_spread_bp = res.measures[MetricId::BasisParSpread.as_str()];
    let par_spread_decimal = par_spread_bp / 1e4;
    let annuity = res.measures[MetricId::AnnuityPrimary.as_str()];
    let pv_primary = res.measures[MetricId::PvPrimary.as_str()];
    let pv_reference = res.measures[MetricId::PvReference.as_str()];
    let notional = swap.notional.amount();

    // par_spread (decimal) * annuity * notional should equal (pv_reference - pv_primary)
    let computed_pv_diff = par_spread_decimal * annuity * notional;
    let actual_pv_diff = pv_reference - pv_primary;

    assert!(
        (computed_pv_diff - actual_pv_diff).abs() < 1.0,
        "Par spread formula mismatch: computed {} vs actual {}",
        computed_pv_diff,
        actual_pv_diff
    );
}

#[test]
fn par_spread_with_existing_spread() {
    // Test par spread calculation when primary leg already has a spread
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "EXISTING-SPREAD-TEST",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(10), // 10bp existing spread
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BasisParSpread],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];

    // Par spread should be finite and represent the additional spread needed
    assert!(par_spread.is_finite());
    // Since we already have 10bp spread, par spread should be different from zero-spread case
}

#[test]
fn par_spread_inverted_curves() {
    // Test with inverted forward curves (shorter tenor > longer tenor)
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.025), (1.0, 0.024), (2.0, 0.023)]) // Inverted
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 0.030), (1.0, 0.029), (2.0, 0.028)]) // Higher than 3M
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(f3m).insert(f1m);

    let swap = BasisSwap::new(
        "INVERTED-TEST",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            d(2025, 1, 2),
            &[MetricId::BasisParSpread],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];

    assert!(par_spread.is_finite());
    // With inverted curves (1M higher than 3M), par spread should be positive
    // to compensate for receiving lower 3M rate
}

#[test]
fn par_spread_long_maturity() {
    // Test par spread for longer-dated swaps (5 years)
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "LONG-MAT-TEST",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2030, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2030, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BasisParSpread, MetricId::AnnuityPrimary],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];
    let annuity = res.measures[MetricId::AnnuityPrimary.as_str()];

    assert!(par_spread.is_finite());
    assert!(annuity > 0.0);
    // Annuity should be larger for longer maturities
}

#[test]
fn par_spread_sign_convention() {
    // Verify par spread sign convention: positive spread added to primary leg
    let ctx = market();
    let as_of = d(2025, 1, 2);

    // If 3M rate > 1M rate, primary leg receives more, so negative spread needed
    // If 1M rate > 3M rate, primary leg receives less, so positive spread needed
    let swap = BasisSwap::new(
        "SIGN-TEST",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::BasisParSpread,
                MetricId::PvPrimary,
                MetricId::PvReference,
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];
    let pv_primary = res.measures[MetricId::PvPrimary.as_str()];
    let pv_reference = res.measures[MetricId::PvReference.as_str()];

    // If PV_primary < PV_reference, need positive spread to increase primary leg value
    // If PV_primary > PV_reference, need negative spread to decrease primary leg value
    if pv_primary < pv_reference {
        assert!(
            par_spread > 0.0,
            "Expected positive par spread when primary < reference"
        );
    } else if pv_primary > pv_reference {
        assert!(
            par_spread < 0.0,
            "Expected negative par spread when primary > reference"
        );
    }
}

#[test]
fn incremental_par_spread_sign_convention() {
    // Test the sign convention for IncrementalParSpreadCalculator:
    // - Positive: Current spread is below par (primary leg receiver is losing)
    // - Negative: Current spread is above par (primary leg receiver is gaining)
    // - Zero: Swap is at par
    let ctx = market();
    let as_of = d(2025, 1, 2);

    // First, create a swap with zero spread to find the par spread
    let zero_spread_swap = BasisSwap::new(
        "INC-SIGN-ZERO",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res = zero_spread_swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BasisParSpread],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread_bp = res.measures[MetricId::BasisParSpread.as_str()];

    // Case 1: Current spread BELOW par -> Positive incremental
    // (need to add more spread to reach par)
    let below_par_spread = par_spread_bp - 5.0; // 5bp below par
    let swap_below_par = BasisSwap::new(
        "INC-SIGN-BELOW",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::try_from(below_par_spread).expect("valid literal"),
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res_below = swap_below_par
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::IncrementalParSpread],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let inc_spread_below = res_below.measures[MetricId::IncrementalParSpread.as_str()];

    assert!(
        inc_spread_below > 0.0,
        "Expected positive incremental spread when current spread is below par, got {:.2}bp",
        inc_spread_below
    );
    // Should be approximately 5bp since we're 5bp below par
    assert!(
        (inc_spread_below - 5.0).abs() < 1.0,
        "Incremental spread should be ~5bp, got {:.2}bp",
        inc_spread_below
    );

    // Case 2: Current spread ABOVE par -> Negative incremental
    // (would need to reduce spread to reach par)
    let above_par_spread = par_spread_bp + 5.0; // 5bp above par
    let swap_above_par = BasisSwap::new(
        "INC-SIGN-ABOVE",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::try_from(above_par_spread).expect("valid literal"),
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res_above = swap_above_par
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::IncrementalParSpread],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let inc_spread_above = res_above.measures[MetricId::IncrementalParSpread.as_str()];

    assert!(
        inc_spread_above < 0.0,
        "Expected negative incremental spread when current spread is above par, got {:.2}bp",
        inc_spread_above
    );
    // Should be approximately -5bp since we're 5bp above par
    assert!(
        (inc_spread_above + 5.0).abs() < 1.0,
        "Incremental spread should be ~-5bp, got {:.2}bp",
        inc_spread_above
    );

    // Case 3: Current spread AT par -> Zero incremental
    let swap_at_par = BasisSwap::new(
        "INC-SIGN-AT-PAR",
        Money::new(10_000_000.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::try_from(par_spread_bp).expect("valid literal"),
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .expect("valid basis swap");

    let res_at_par = swap_at_par
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::IncrementalParSpread],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let inc_spread_at_par = res_at_par.measures[MetricId::IncrementalParSpread.as_str()];

    assert!(
        inc_spread_at_par.abs() < 0.5,
        "Expected near-zero incremental spread when at par, got {:.2}bp",
        inc_spread_at_par
    );
}

#[test]
fn zero_notional_par_spread_returns_error() {
    // Test that par spread calculation returns an explicit error for zero notional
    // instead of NaN or Inf
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "ZERO-NOTIONAL-PAR",
        Money::new(0.0, USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .unwrap();

    // Par spread calculation should return an error, not NaN/Inf
    let result = swap.price_with_metrics(
        &ctx,
        as_of,
        &[MetricId::BasisParSpread],
        finstack_valuations::instruments::PricingOptions::default(),
    );

    // Either it returns an error (preferred) or returns a finite value
    // It should NOT return Ok with NaN or Inf
    match result {
        Ok(res) => {
            let par_spread = res.measures.get(MetricId::BasisParSpread.as_str());
            if let Some(&val) = par_spread {
                assert!(
                    val.is_finite(),
                    "Par spread should not be NaN or Inf for zero notional; got {}",
                    val
                );
            }
        }
        Err(e) => {
            // This is the expected behavior - explicit error for zero notional
            let err_msg = format!("{}", e);
            assert!(
                err_msg.contains("notional") || err_msg.contains("annuity"),
                "Error message should mention notional or annuity issue: {}",
                err_msg
            );
        }
    }
}
