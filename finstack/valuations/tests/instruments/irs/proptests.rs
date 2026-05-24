//! Property-based tests for Interest Rate Swap instruments.
//!
//! Uses proptest to verify invariants and edge cases across a wide range
//! of randomly generated inputs. Tests cover numerical stability, monotonicity,
//! and fundamental swap relationships.

use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::{FixedLegSpec, FloatLegSpec};
use proptest::prelude::*;
use time::macros::date;

/// Helper to build a flat discount curve for property tests.
fn build_test_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
            (30.0, (-rate * 30.0).exp()),
        ])
        // For property tests we prefer robustness over strict Hagan–West
        // monotone-convex behaviour; linear interpolation is sufficient and
        // handles flat/zero-rate curves gracefully.
        .interp(InterpStyle::Linear);

    // For zero or negative rates, allow non-monotonic DFs
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().unwrap()
}

/// Helper to build a flat forward curve for property tests.
fn build_test_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id, 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate), (30.0, rate)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

proptest! {
    /// Property: Receiver swap NPV should increase as fixed rate increases (holding curves constant).
    ///
    /// For a receiver swap (receive fixed, pay floating), the NPV should be
    /// monotonically increasing in the fixed rate, as higher coupons increase
    /// the present value of the fixed leg received.
    #[test]
    fn receiver_npv_increases_with_fixed_rate(
        fixed_rate_1 in 0.01f64..0.10,
        fixed_rate_2 in 0.01f64..0.10,
        notional in 1_000_000.0..100_000_000.0f64,
        tenor_years in 1i32..10,
    ) {
        // Ensure rate_1 < rate_2
        let (rate_low, rate_high) = if fixed_rate_1 < fixed_rate_2 {
            (fixed_rate_1, fixed_rate_2)
        } else {
            (fixed_rate_2, fixed_rate_1)
        };

        // Skip if rates are too close
        prop_assume!((rate_high - rate_low).abs() >= 1e-6);

        let base_date = date!(2024-01-01);
        let start = date!(2024-01-01);
        let end = Date::from_ordinal_date(start.year() + tenor_years, 1).unwrap();

        // Build market curves
        let disc = build_test_discount_curve(0.03, base_date, "USD-OIS");
        let fwd = build_test_forward_curve(0.03, base_date, "USD-SOFR-3M");
        let context = MarketContext::new().insert(disc).insert(fwd);

        // Create two receiver swaps with different fixed rates
        let irs_low = InterestRateSwap::builder()
            .id(InstrumentId::new("IRS-LOW"))
            .notional(Money::new(notional, Currency::USD))
            .side(PayReceive::Receive)
            .fixed(FixedLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                rate: rust_decimal::Decimal::from_f64_retain(rate_low).unwrap_or_default(),
                frequency: Tenor::semi_annual(),
                day_count: DayCount::Thirty360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::None,
                start,
                end,
                par_method: None,
                compounding_simple: true,
                payment_lag_days: 0,
                end_of_month: false,            })
            .float(FloatLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                fixing_calendar_id: None,
                stub: StubKind::None,
                reset_lag_days: 0,  // Use 0 for spot-starting swaps to avoid needing historical fixings
                start,
                end,
                compounding: Default::default(),
                payment_lag_days: 0,
                end_of_month: false,            })
            .build()?;

        let irs_high = InterestRateSwap::builder()
            .id(InstrumentId::new("IRS-HIGH"))
            .notional(Money::new(notional, Currency::USD))
            .side(PayReceive::Receive)
            .fixed(FixedLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                rate: rust_decimal::Decimal::from_f64_retain(rate_high).unwrap_or_default(),
                frequency: Tenor::semi_annual(),
                day_count: DayCount::Thirty360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::None,
                start,
                end,
                par_method: None,
                compounding_simple: true,
                payment_lag_days: 0,
                end_of_month: false,            })
            .float(FloatLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                fixing_calendar_id: None,
                stub: StubKind::None,
                reset_lag_days: 0,  // Use 0 for spot-starting swaps to avoid needing historical fixings
                start,
                end,
                compounding: Default::default(),
                payment_lag_days: 0,
                end_of_month: false,            })
            .build()?;

        let npv_low = irs_low.value(&context, base_date)?;
        let npv_high = irs_high.value(&context, base_date)?;

        // Higher fixed rate should yield higher NPV for receiver swap
        prop_assert!(
            npv_high.amount() > npv_low.amount(),
            "Receiver swap NPV should increase with fixed rate: {} (low) vs {} (high) at rates {} vs {}",
            npv_low.amount(), npv_high.amount(), rate_low, rate_high
        );
    }

    /// Property: Payer and receiver swaps at same rate should have opposite NPVs.
    ///
    /// For identical swap terms, a payer swap (pay fixed) should have
    /// NPV = -1 * receiver swap NPV, as they are mirror positions.
    #[test]
    fn payer_receiver_symmetry(
        fixed_rate in 0.01f64..0.10,
        notional in 1_000_000.0..100_000_000.0f64,
        tenor_years in 2i32..10,
    ) {
        let base_date = date!(2024-01-01);
        let start = date!(2024-01-01);
        let end = Date::from_ordinal_date(start.year() + tenor_years, 1).unwrap();

        // Build market curves
        let disc = build_test_discount_curve(0.04, base_date, "USD-OIS");
        let fwd = build_test_forward_curve(0.04, base_date, "USD-SOFR-3M");
        let context = MarketContext::new().insert(disc).insert(fwd);

        let payer = InterestRateSwap::builder()
            .id(InstrumentId::new("IRS-PAYER"))
            .notional(Money::new(notional, Currency::USD))
            .side(PayReceive::Pay)
            .fixed(FixedLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                rate: rust_decimal::Decimal::from_f64_retain(fixed_rate).unwrap_or_default(),
                frequency: Tenor::semi_annual(),
                day_count: DayCount::Thirty360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::None,
                start,
                end,
                par_method: None,
                compounding_simple: true,
                payment_lag_days: 0,
                end_of_month: false,            })
            .float(FloatLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                fixing_calendar_id: None,
                stub: StubKind::None,
                reset_lag_days: 0,  // Use 0 for spot-starting swaps to avoid needing historical fixings
                start,
                end,
                compounding: Default::default(),
                payment_lag_days: 0,
                end_of_month: false,            })
            .build()?;

        let receiver = InterestRateSwap::builder()
            .id(InstrumentId::new("IRS-RECEIVER"))
            .notional(Money::new(notional, Currency::USD))
            .side(PayReceive::Receive)
            .fixed(FixedLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                rate: rust_decimal::Decimal::from_f64_retain(fixed_rate).unwrap_or_default(),
                frequency: Tenor::semi_annual(),
                day_count: DayCount::Thirty360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::None,
                start,
                end,
                par_method: None,
                compounding_simple: true,
                payment_lag_days: 0,
                end_of_month: false,            })
            .float(FloatLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                fixing_calendar_id: None,
                stub: StubKind::None,
                reset_lag_days: 0,  // Use 0 for spot-starting swaps to avoid needing historical fixings
                start,
                end,
                compounding: Default::default(),
                payment_lag_days: 0,
                end_of_month: false,            })
            .build()?;

        let npv_payer = payer.value(&context, base_date)?;
        let npv_receiver = receiver.value(&context, base_date)?;

        // Payer and receiver should be exact opposites
        let sum = npv_payer.amount() + npv_receiver.amount();
        prop_assert!(
            sum.abs() < 1e-6,
            "Payer and receiver NPVs should sum to zero: payer={}, receiver={}, sum={}",
            npv_payer.amount(), npv_receiver.amount(), sum
        );
    }

    /// Property: NPV should not overflow or underflow for extreme (but valid) rates.
    ///
    /// Tests numerical stability across a wide range of rates, from deeply
    /// negative to very high positive rates. All calculations should complete
    /// without errors and produce finite results.
    #[test]
    fn numerical_stability_extreme_rates(
        fixed_rate in -0.05f64..0.20,
        curve_rate in -0.05f64..0.20,
        notional in 1_000_000.0..10_000_000.0f64,
    ) {
        let base_date = date!(2024-01-01);
        let start = date!(2024-01-01);
        let end = date!(2029-01-01);

        // Build market curves with potentially extreme rates. Some combinations
        // of parameters may violate curve builder invariants (e.g., forward
        // positivity constraints). Those are treated as outside the property
        // domain and discarded via `prop_assume!`.
        let disc_res = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 1.0),
                (1.0, (-curve_rate).exp()),
                (5.0, (-curve_rate * 5.0).exp()),
                (10.0, (-curve_rate * 10.0).exp()),
                (30.0, (-curve_rate * 30.0).exp()),
            ])
            .interp(InterpStyle::Linear)
            .build();

        let fwd_res = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots([(0.0, curve_rate), (10.0, curve_rate), (30.0, curve_rate)])
            .interp(InterpStyle::Linear)
            .build();

        prop_assume!(disc_res.is_ok() && fwd_res.is_ok());
        let disc = disc_res.unwrap();
        let fwd = fwd_res.unwrap();

        let context = MarketContext::new().insert(disc).insert(fwd);

        let irs = InterestRateSwap::builder()
            .id(InstrumentId::new("IRS-EXTREME"))
            .notional(Money::new(notional, Currency::USD))
            .side(PayReceive::Pay)
            .fixed(FixedLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                rate: rust_decimal::Decimal::from_f64_retain(fixed_rate).unwrap_or_default(),
                frequency: Tenor::semi_annual(),
                day_count: DayCount::Thirty360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::None,
                start,
                end,
                par_method: None,
                compounding_simple: true,
                payment_lag_days: 0,
                end_of_month: false,            })
            .float(FloatLegSpec {
                discount_curve_id: CurveId::new("USD-OIS"),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                fixing_calendar_id: None,
                stub: StubKind::None,
                reset_lag_days: 0,  // Use 0 for spot-starting swaps to avoid needing historical fixings
                start,
                end,
                compounding: Default::default(),
                payment_lag_days: 0,
                end_of_month: false,            })
            .build()?;

        let npv = irs.value(&context, base_date)?;

        // NPV should be finite (not NaN or infinite)
        prop_assert!(
            npv.amount().is_finite(),
            "NPV should be finite for extreme rates: fixed_rate={}, curve_rate={}, npv={}",
            fixed_rate, curve_rate, npv.amount()
        );

        // NPV should be bounded (rough sanity check: within 2x notional * tenor)
        let max_reasonable_npv = notional * 5.0 * 2.0; // 5 years * 2x factor
        prop_assert!(
            npv.amount().abs() < max_reasonable_npv,
            "NPV should be reasonable for extreme rates: {}, max expected: {}",
            npv.amount(), max_reasonable_npv
        );
    }
}
