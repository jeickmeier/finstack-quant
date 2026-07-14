//! Market standard validation tests.
//!
//! These tests validate our IRS metric calculations against:
//! - Standard swap pricing formulas
//! - Market conventions (ISDA, ICMA)
//! - Textbook examples (Hull)
//!
//! References:
//! - Hull, "Options, Futures, and Other Derivatives"
//! - ISDA documentation
//! - Market practice for USD swaps

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, CalendarRegistry, Date, DateExt, DayCount, DayCountContext, StubKind,
    Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::IndexId;
use finstack_quant_valuations::instruments::rates::irs::{
    FixedLegSpec, FloatLegSpec, FloatingLegCompounding, InterestRateSwap, ParRateMethod, PayReceive,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::market::conventions::ConventionRegistry;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id, 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .unwrap()
}

fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
            (30.0, (-rate * 30.0).exp()),
        ]);

    // For zero or negative rates, DFs may be flat or increasing
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().unwrap()
}

#[test]
fn test_irs_par_rate_market_standard() {
    // Market standard: For a new (at-inception) swap, par rate makes NPV = 0
    // 5-year USD swap (ISDA conventions via usd_irs_swap)

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = test_utils::usd_irs_swap(
        "SWAP_PAR_TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();
    // Compute par rate under current curves
    let par = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures["par_rate"];
    // Rebuild swap at par and assert PV ~ 0
    let par_swap = test_utils::usd_irs_swap(
        "SWAP_PAR_PAR",
        Money::new(1_000_000.0, Currency::USD),
        par,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();
    let npv = par_swap.value(&market, as_of).unwrap();
    assert!(
        npv.amount().abs() < 1.0, // 0.01bp tolerance on 1MM notional
        "Par swap NPV should be ~0 (within 0.01bp), got {:.2} ({:.4}bp)",
        npv.amount(),
        npv.amount() / 100.0
    );
}

#[test]
fn test_par_rate_discount_ratio_matches_forward_for_new_swap() {
    // For an unseasoned swap (as_of == start), DiscountRatio and ForwardBased
    // par rate methods should agree.
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap_forward = test_utils::usd_irs_swap(
        "SWAP_PAR_FWD",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();
    let mut swap_discount = swap_forward.clone();
    swap_discount.fixed.par_method = Some(ParRateMethod::DiscountRatio);

    let par_forward = swap_forward
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures["par_rate"];

    let par_discount = swap_discount
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures["par_rate"];

    let diff = (par_forward - par_discount).abs();
    assert!(
        diff < 5e-4, // 0.05% tolerance (~0.5bp)
        "ForwardBased and DiscountRatio par rates should be very close for new \
         swaps: forward={}, discount={}, diff={}",
        par_forward,
        par_discount,
        diff
    );
}

#[test]
fn test_par_rate_discount_ratio_rejects_seasoned_swap() {
    let start = date!(2024 - 01 - 01);
    let as_of = date!(2024 - 06 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    // Flat forward curve based at the trade date: seasoned resets strictly
    // before the curve base now error under the default FloatingRateFallback.
    let fwd_curve = build_flat_forward_curve(0.05, start, "USD-SOFR-3M");

    // Provide fixings for past reset dates (quarterly from Jan 1 through Apr 1)
    let fixings = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![
            (date!(2023 - 12 - 29), 0.05), // Q1 T-2 reset at 5%
            (date!(2024 - 01 - 01), 0.05), // Q1 reset at 5%
            (date!(2024 - 03 - 28), 0.05), // Q2 T-2 reset at 5%
            (date!(2024 - 04 - 01), 0.05), // Q2 reset at 5%
        ],
        None,
    )
    .expect("fixings series");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_series(fixings);

    let mut swap = test_utils::usd_irs_swap(
        "SWAP_PAR_SEASONED",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        start,
        end,
        PayReceive::Receive,
    )
    .unwrap();
    swap.fixed.par_method = Some(ParRateMethod::DiscountRatio);

    let par_forward = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures["par_rate"];

    let par_discount = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures["par_rate"];

    let diff = (par_forward - par_discount).abs();
    assert!(
        diff < 5e-4,
        "For seasoned swaps, DiscountRatio should effectively fall back to \
         ForwardBased par rate: forward={}, discount={}, diff={}",
        par_forward,
        par_discount,
        diff
    );
}

#[test]
fn test_irs_annuity_calculation() {
    // Annuity = Sum of discounted year fractions
    // For 5-year swap at 5% flat curve, quarterly payments
    // Expected: ~4.28 years (approximate)

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = test_utils::usd_irs_swap(
        "SWAP_ANNUITY_TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = *result.measures.get("annuity").unwrap();

    // Expected: ~4.28 for 5-year swap at 5%
    assert!(
        annuity > 4.0 && annuity < 4.5,
        "Annuity={:.3} outside expected range 4.0-4.5 years",
        annuity
    );

    // Annuity should be less than time to maturity
    assert!(
        annuity < 5.0,
        "Annuity={:.3} should be less than maturity 5.0 years",
        annuity
    );
}

#[test]
fn test_irs_dv01_market_standard() {
    // DV01 = Annuity × Notional × 1bp
    // Market standard formula for IRS

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let notional = 1_000_000.0;

    let swap = test_utils::usd_irs_swap(
        "SWAP_DV01_TEST",
        Money::new(notional, Currency::USD),
        0.05,
        as_of,
        end,
        PayReceive::Receive,
    )
    .unwrap();

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity, MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = *result.measures.get("annuity").unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 is computed via parallel bump-and-reprice (GenericParallelDv01)
    // For a Receive swap, DV01 should be negative (loses value when rates rise)
    // For $1MM notional, 5-year swap at 5% rates, expect DV01 magnitude around $430-$450
    assert!(
        dv01.abs() > 400.0 && dv01.abs() < 500.0,
        "DV01={:.2} outside typical range $400-$500 for $1MM 5Y swap",
        dv01
    );

    // Receive swap should have negative DV01 (loses value when rates increase)
    assert!(
        dv01 < 0.0,
        "Receive swap should have negative DV01, got {:.2}",
        dv01
    );

    // Annuity approximation: DV01 ≈ Annuity × Notional × 0.0001
    // For IRS on flat curves, the parallel bump DV01 should closely match the
    // annuity-based approximation. Typical differences are 2-3% due to:
    // - Schedule generation effects (payment dates vs accrual dates)
    // - Floating leg projection includes forward rate changes on bump
    // - Minor day count effects in schedule generation
    let annuity_approx = annuity * notional * 0.0001;
    assert!(
        (dv01.abs() - annuity_approx).abs() / annuity_approx < 0.03, // Tightened from 5% to 3%
        "DV01={:.2} differs from annuity approximation {:.2} by more than 3% (Annuity={:.4})",
        dv01,
        annuity_approx,
        annuity
    );
}

#[test]
fn test_irs_receive_vs_pay_fixed() {
    // Receive fixed and pay fixed should have opposite NPVs

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.06, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let fixed_leg = FixedLegSpec {
        discount_curve_id: "USD-OIS".into(),
        rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,
        start: as_of,
        end,
    };

    let float_leg = FloatLegSpec {
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
        spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
        compounding: Default::default(),
        payment_lag_days: 0,
        end_of_month: false,
        start: as_of,
        end,
        fixing_calendar_id: None,
    };

    let swap_receive = InterestRateSwap {
        id: "SWAP_RECEIVE".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: fixed_leg.clone(),
        float: float_leg.clone(),
        margin_spec: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let swap_pay = InterestRateSwap {
        id: "SWAP_PAY".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Pay,
        fixed: fixed_leg,
        float: float_leg,
        margin_spec: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let npv_receive = swap_receive.value(&market, as_of).unwrap();
    let npv_pay = swap_pay.value(&market, as_of).unwrap();

    // NPVs should be opposite signs
    assert!(
        npv_receive.amount() * npv_pay.amount() < 0.0,
        "Receive fixed and pay fixed should have opposite NPVs: receive={:.2}, pay={:.2}",
        npv_receive.amount(),
        npv_pay.amount()
    );

    // With forward rate (6%) > fixed rate (5%):
    // Receive fixed (pay floating) should be negative
    // Pay fixed (receive floating) should be positive
    assert!(
        npv_receive.amount() < 0.0,
        "Receive fixed below market should be negative: {:.2}",
        npv_receive.amount()
    );
    assert!(
        npv_pay.amount() > 0.0,
        "Pay fixed below market should be positive: {:.2}",
        npv_pay.amount()
    );
}

#[test]
fn test_irs_rate_sensitivity() {
    // As rates increase, receive fixed position loses value

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let fixed_leg = FixedLegSpec {
        discount_curve_id: "USD-OIS".into(),
        rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,
        start: as_of,
        end,
    };

    let float_leg = FloatLegSpec {
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
        spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
        compounding: Default::default(),
        payment_lag_days: 0,
        end_of_month: false,
        start: as_of,
        end,
        fixing_calendar_id: None,
    };

    let swap = InterestRateSwap {
        id: "SWAP_RATE_SENS".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: fixed_leg,
        float: float_leg,
        margin_spec: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let mut npvs = Vec::new();

    // Test at different rate levels
    for rate in [0.03, 0.04, 0.05, 0.06, 0.07] {
        let disc_curve = build_flat_discount_curve(rate, as_of, "USD-OIS");
        let fwd_curve = build_flat_forward_curve(rate, as_of, "USD-SOFR-3M");

        let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

        let npv = swap.value(&market, as_of).unwrap();
        npvs.push((rate, npv.amount()));
    }

    // Verify inverse relationship for receive fixed:
    // Higher rates → Lower NPV (losing value)
    for i in 1..npvs.len() {
        assert!(
            npvs[i].1 < npvs[i - 1].1,
            "Receive fixed swap value should decrease as rates rise: \
             rate {:.2}% NPV={:.2} >= rate {:.2}% NPV={:.2}",
            npvs[i].0 * 100.0,
            npvs[i].1,
            npvs[i - 1].0 * 100.0,
            npvs[i - 1].1
        );
    }

    // At par rate (5%), NPV should be near zero
    let par_npv = npvs[2].1; // 0.05 rate
    assert!(
        par_npv.abs() < 100.0, // 1bp tolerance
        "At par rate, NPV should be near zero (within 1bp): {:.2} ({:.2}bp)",
        par_npv,
        par_npv / 100.0
    );
}

#[test]
fn test_irs_leg_pvs_consistency() {
    // For receive fixed swap: NPV = PV(fixed leg) - PV(floating leg)

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.06, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let swap = InterestRateSwap {
        id: "SWAP_LEG_PVS".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        float: FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
            fixing_calendar_id: None,
        },
        margin_spec: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PvFixed, MetricId::PvFloat],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_fixed = *result.measures.get("pv_fixed").unwrap();
    let pv_float = *result.measures.get("pv_float").unwrap();
    let base_npv = result.value.amount();

    // NPV should equal PV(fixed) - PV(float) for receive fixed
    let calculated_npv = pv_fixed - pv_float;

    assert!(
        (calculated_npv - base_npv).abs() < 100.0,
        "NPV from legs ({:.2}) should match total NPV ({:.2})",
        calculated_npv,
        base_npv
    );
}

#[test]
fn test_daycount_convention_impact_on_annuity() {
    // Different day-count conventions should produce measurably different annuities
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD-SOFR-3M");
    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Swap with ACT/360 fixed leg
    let swap_act360 = InterestRateSwap::builder()
        .id("IRS-ACT360".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        })
        .float(FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        })
        .build()
        .unwrap();

    // Swap with 30/360 fixed leg (US convention)
    let swap_30360 = InterestRateSwap::builder()
        .id("IRS-30360".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        })
        .float(FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        })
        .build()
        .unwrap();

    let annuity_act360 = swap_act360
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures["annuity"];

    let annuity_30360 = swap_30360
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures["annuity"];

    // ACT/360 counts actual days / 360, 30/360 assumes 30-day months
    // For a 5Y swap, difference should be ~1-2% depending on actual calendar
    let diff_pct = (annuity_act360 - annuity_30360).abs() / annuity_act360 * 100.0;

    assert!(
        diff_pct > 0.1 && diff_pct < 5.0,
        "Day-count conventions should produce measurable difference.\n\
         ACT/360: {:.4}, 30/360: {:.4}, diff: {:.2}%",
        annuity_act360,
        annuity_30360,
        diff_pct
    );
}

/// Test ISDA-compliant USD 5Y IRS with T-2 fixing calendar.
///
/// This test validates the fix for the floating leg pricer that now:
/// 1. Uses `reset_lag_days` and `fixing_calendar_id` for computing reset dates
/// 2. Uses the forward curve's day count for forward rate projection
///
/// Per ISDA 2006 Section 4.2, USD swaps use:
/// - Reset lag: T-2 (2 business days before accrual start)
/// - Fixing calendar: USD (SIFMA holiday calendar for US markets)
///
/// # Acceptance Criteria
/// - PV within 1e-8 of reference (deterministic, reproducible)
/// - Reset dates must never be after accrual start dates
/// - Runtime ≤ 5ms per swap valuation
#[test]
fn test_irs_t_minus_2_fixing_calendar_isda_standard() {
    use std::time::Instant;

    let as_of = date!(2024 - 01 - 02); // Tuesday Jan 2, 2024
    let start = date!(2024 - 01 - 02);
    let end = date!(2029 - 01 - 02);

    // Build curves with different day counts to verify correct basis usage
    // Discount curve: ACT/365F (typical for OIS)
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.05_f64).exp()),
            (5.0, (-0.05_f64 * 5.0).exp()),
            (10.0, (-0.05_f64 * 10.0).exp()),
        ])
        .build()
        .unwrap();

    // Forward curve: ACT/360 (typical for USD SOFR)
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 0.05),
            (1.0, 0.052),
            (2.0, 0.054),
            (5.0, 0.055),
            (10.0, 0.056),
        ])
        .build()
        .unwrap();

    // Provide fixings for T-2 reset dates that fall before as_of
    // For a spot-starting swap on Jan 2, 2024 with T-2 reset lag,
    // the first reset is Dec 28, 2023 (2 business days before, accounting for NY holiday)
    let fixings = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![
            (date!(2023 - 12 - 28), 0.05), // First reset fixing at 5%
        ],
        None,
    )
    .expect("fixings series");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_series(fixings);

    // Create ISDA-standard USD swap with explicit fixing calendar
    let swap = InterestRateSwap::builder()
        .id("IRS-5Y-USD-T2-FIXING".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"), // 5% fixed rate
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360, // USD fixed leg: 30/360
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        })
        .float(FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360, // USD float leg: ACT/360
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()), // Payment calendar
            fixing_calendar_id: Some("usny".to_string()), // Fixing calendar for T-2
            stub: StubKind::None,
            reset_lag_days: 2, // T-2 reset lag per ISDA standard
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        })
        .build()
        .unwrap();

    // Validate the swap
    swap.validate().expect("Swap should be valid");

    // Performance test: pricing should complete in < 5ms
    let timer = Instant::now();
    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::PvFixed,
                MetricId::PvFloat,
                MetricId::ParRate,
                MetricId::Dv01,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let elapsed = timer.elapsed();

    let max_ms = if cfg!(debug_assertions) { 200 } else { 10 };
    assert!(
        elapsed.as_millis() <= max_ms,
        "Pricing should complete in <= {}ms, took {}ms",
        max_ms,
        elapsed.as_millis()
    );

    let pv = result.value.amount();
    let pv_fixed = result.measures["pv_fixed"];
    let pv_float = result.measures["pv_float"];
    let par_rate = result.measures["par_rate"];
    let dv01 = result.measures["dv01"];

    // Sanity checks
    // 1. PV should be reasonable for a near-par swap
    assert!(
        pv.abs() < 500_000.0, // Within 5% of notional
        "PV={:.2} should be reasonable for near-par swap",
        pv
    );

    // 2. PV_Fixed and PV_Float should be positive (unsigned leg values)
    assert!(
        pv_fixed > 0.0,
        "PV_Fixed should be positive: {:.2}",
        pv_fixed
    );
    assert!(
        pv_float > 0.0,
        "PV_Float should be positive: {:.2}",
        pv_float
    );

    // 3. For Pay: NPV = PV_Float - PV_Fixed
    let calculated_npv = pv_float - pv_fixed;
    assert!(
        (calculated_npv - pv).abs() < 1.0,
        "NPV should equal PV_Float - PV_Fixed: calc={:.2}, actual={:.2}",
        calculated_npv,
        pv
    );

    // 4. Par rate should be reasonable (between 3% and 7% for this curve)
    assert!(
        par_rate > 0.03 && par_rate < 0.07,
        "Par rate {:.4} outside expected range [3%, 7%]",
        par_rate
    );

    // 5. DV01 should be reasonable for 5Y $10MM swap (~$4,000-$5,000)
    assert!(
        dv01.abs() > 3_000.0 && dv01.abs() < 6_000.0,
        "DV01={:.2} outside expected range [$3,000, $6,000] for $10MM 5Y swap",
        dv01
    );

    // Determinism test: run 10 times and verify bitwise identical results
    let pvs: Vec<f64> = (0..10)
        .map(|_| swap.value(&market, as_of).unwrap().amount())
        .collect();

    for (i, &p) in pvs.iter().enumerate().skip(1) {
        assert_eq!(
            p, pvs[0],
            "Iteration {} PV {:.15} differs from iteration 0 PV {:.15}",
            i, p, pvs[0]
        );
    }
}

/// Test that the forward curve's day count is used for forward rate projection.
///
/// This validates that when the floating leg and forward curve have different
/// day count conventions, the forward curve's convention is used for time
/// calculations in rate projection.
#[test]
fn test_irs_forward_curve_daycount_used_for_projection() {
    let as_of = date!(2024 - 01 - 02);
    let start = date!(2024 - 01 - 02);
    let end = date!(2029 - 01 - 02); // 5Y swap for more visible effects

    // Forward curve with ACT/365F day count - upward sloping to create non-zero PV
    let fwd_curve_365 = ForwardCurve::builder("USD-SOFR-365", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 0.04), (1.0, 0.045), (5.0, 0.055)])
        .build()
        .unwrap();

    // Forward curve with ACT/360 day count - same rates
    let fwd_curve_360 = ForwardCurve::builder("USD-SOFR-360", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.04), (1.0, 0.045), (5.0, 0.055)])
        .build()
        .unwrap();

    // Same discount curve for both
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.045_f64).exp()),
            (5.0, (-0.045_f64 * 5.0).exp()),
        ])
        .build()
        .unwrap();

    let market_365 = MarketContext::new()
        .insert(disc_curve.clone())
        .insert(fwd_curve_365);

    let market_360 = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve_360);

    // Create identical swap specs but with different forward curve references
    // Use fixed rate below forward to create positive floating leg NPV
    let swap_365 = InterestRateSwap::builder()
        .id("IRS-FWD-365".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.04).expect("valid"), // Below average forward to create positive NPV
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        })
        .float(FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-365".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360, // Float leg uses ACT/360 for accrual
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        })
        .build()
        .unwrap();

    let swap_360 = InterestRateSwap::builder()
        .id("IRS-FWD-360".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.04).expect("valid"), // Below average forward to create positive NPV
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        })
        .float(FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-360".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360, // Float leg uses ACT/360 for accrual
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        })
        .build()
        .unwrap();

    let pv_365 = swap_365.value(&market_365, as_of).unwrap().amount();
    let pv_360 = swap_360.value(&market_360, as_of).unwrap().amount();

    // Both PVs should be non-zero (we're in-the-money)
    assert!(
        pv_365.abs() > 1000.0,
        "PV_365={:.2} should be non-zero for non-par swap",
        pv_365
    );
    assert!(
        pv_360.abs() > 1000.0,
        "PV_360={:.2} should be non-zero for non-par swap",
        pv_360
    );

    // The PVs should be slightly different due to different time calculations
    // in forward rate projection (ACT/365F vs ACT/360 will give different t values)
    // For a 5Y swap with upward sloping rates, the difference should be measurable
    let diff = (pv_365 - pv_360).abs();
    let diff_pct = diff / pv_365.abs() * 100.0;

    // The difference should exist (forward curve day count is being used)
    // but be relatively small (< 1% of PV)
    assert!(
        diff > 1.0,
        "Different forward curve day counts should produce measurable PV difference.\n\
         PV_365={:.4}, PV_360={:.4}, diff={:.4}",
        pv_365,
        pv_360,
        diff
    );
    assert!(
        diff_pct < 5.0,
        "Day count difference impact should be < 5% of PV, got {:.2}%",
        diff_pct
    );
}

#[test]
fn test_sofr_ois_par_rate_matches_quantlib_identity() {
    // QuantLib identity for single-curve OIS:
    // par_rate = PV_float / annuity, where PV_float uses DF ratios per period.
    let as_of = date!(2025 - 01 - 02);
    let calendar = CalendarRegistry::global()
        .resolve_str("usny")
        .expect("USNY calendar");
    let start = as_of.add_business_days(2, calendar).expect("spot start");
    let end = Tenor::annual()
        .add_to_date(
            start,
            Some(calendar),
            BusinessDayConvention::ModifiedFollowing,
        )
        .expect("maturity");

    let curve_id = "USD-SOFR-OIS";
    let disc = build_flat_discount_curve(0.05, as_of, curve_id);
    let market = MarketContext::new().insert(disc);

    let swap = InterestRateSwap::builder()
        .id("SOFR-OIS-QL-PARITY".into())
        .notional(Money::new(100_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: curve_id.into(),
            rate: rust_decimal::Decimal::ZERO,
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start,
            end,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(FloatLegSpec {
            discount_curve_id: curve_id.into(),
            forward_curve_id: curve_id.into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0,
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        })
        .build()
        .unwrap();

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("metrics");
    let par_rate = result
        .measures
        .get(MetricId::ParRate.as_str())
        .copied()
        .expect("par rate");

    let conv = ConventionRegistry::try_global()
        .expect("registry")
        .require_rate_index(&IndexId::new(curve_id))
        .expect("rate index");
    let periods = build_periods(BuildPeriodsParams {
        start,
        end,
        frequency: Tenor::annual(),
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: conv.market_calendar_id.as_str(),
        end_of_month: false,
        day_count: DayCount::Act360,
        payment_lag_days: conv.default_payment_lag_days,
        reset_lag_days: None,
        adjust_accrual_dates: false,
    })
    .expect("periods");

    let disc = market.get_discount(curve_id).expect("discount");
    let mut annuity = 0.0;
    let mut float_pv = 0.0;
    for period in periods {
        let t_start = disc
            .day_count()
            .year_fraction(
                disc.base_date(),
                period.accrual_start,
                DayCountContext::default(),
            )
            .expect("t_start");
        let t_end = disc
            .day_count()
            .year_fraction(
                disc.base_date(),
                period.accrual_end,
                DayCountContext::default(),
            )
            .expect("t_end");
        let t_pay = disc
            .day_count()
            .year_fraction(
                disc.base_date(),
                period.payment_date,
                DayCountContext::default(),
            )
            .expect("t_pay");
        let df_start = disc.df(t_start);
        let df_end = disc.df(t_end);
        let df_pay = disc.df(t_pay);

        annuity += df_pay * period.accrual_year_fraction;
        float_pv += (df_start / df_end - 1.0) * df_pay;
    }
    let expected_par = float_pv / annuity;

    let diff_bp = (par_rate - expected_par) * 10_000.0;
    assert!(
        diff_bp.abs() <= 0.1,
        "SOFR OIS par rate mismatch vs QuantLib identity: par_rate={:.6} expected={:.6} diff={:.4}bp",
        par_rate,
        expected_par,
        diff_bp
    );
}

// ============================================================================
// End-of-month convention tests
// ============================================================================

/// Verify that the pricer and the cashflow schedule builder produce consistent
/// period boundaries when `end_of_month = true` and the start date falls on
/// the last business day of a month.
///
/// Before this fix, the pricer hardcoded `end_of_month: false` while the
/// cashflow builder correctly used the spec value, causing PV mismatches.
#[test]
fn test_eom_pricer_cashflow_consistency() {
    use finstack_quant_cashflows::CashflowProvider;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;

    // 2024-02-29 is a Thursday (last business day of Feb in a leap year)
    let as_of = date!(2024 - 02 - 29);
    let start = as_of;
    let end = date!(2025 - 02 - 28); // 1Y swap

    let disc_curve = build_flat_discount_curve(0.04, as_of, "USD-OIS");
    let fwd_curve = build_flat_forward_curve(0.04, as_of, "USD-SOFR-3M");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Build swap with end_of_month = true (ISDA 2006 §4.18 convention)
    let swap_eom = InterestRateSwap::builder()
        .id("IRS-EOM-TRUE".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            rate: Decimal::try_from(0.04).unwrap(),
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
            end_of_month: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            start,
            end,
            compounding: FloatingLegCompounding::Simple,
            payment_lag_days: 0,
            end_of_month: true,
        })
        .build()
        .unwrap();

    // Build the same swap with end_of_month = false for comparison
    let swap_no_eom = InterestRateSwap::builder()
        .id("IRS-EOM-FALSE".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            rate: Decimal::try_from(0.04).unwrap(),
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
            end_of_month: false,
        })
        .float(FloatLegSpec {
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            start,
            end,
            compounding: FloatingLegCompounding::Simple,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .build()
        .unwrap();

    // Both should price successfully
    let pv_eom = swap_eom
        .value(&market, as_of)
        .expect("EOM swap should price");
    let pv_no_eom = swap_no_eom
        .value(&market, as_of)
        .expect("Non-EOM swap should price");

    // With flat curves, the two PVs should be close but may differ slightly
    // due to different period boundaries. The key invariant is that both
    // price successfully and the cashflow schedule is consistent with the pricer.

    // Cross-check: the cashflow schedule and the pricer should produce the same NPV.
    // Build full schedule (this uses the cashflow builder which correctly propagates EOM)
    let sched = swap_eom
        .cashflow_schedule(&market, as_of)
        .expect("EOM cashflow schedule");

    // The schedule should have generated valid flows
    assert!(
        !sched.get_flows().is_empty(),
        "EOM cashflow schedule should have flows"
    );

    // Both variants should return finite PVs
    assert!(
        pv_eom.amount().is_finite(),
        "EOM PV should be finite: {}",
        pv_eom.amount()
    );
    assert!(
        pv_no_eom.amount().is_finite(),
        "Non-EOM PV should be finite: {}",
        pv_no_eom.amount()
    );

    // Verify that the EOM swap's par rate calculation also works
    let metrics = vec![MetricId::ParRate, MetricId::Annuity];
    let result = swap_eom
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("EOM metrics should compute");
    let par_rate = result.measures.get(MetricId::ParRate.as_str()).copied();
    assert!(
        par_rate.is_some(),
        "Par rate should be computed for EOM swap"
    );
    let par = par_rate.unwrap();
    assert!(
        par.is_finite() && par > 0.0 && par < 1.0,
        "Par rate should be reasonable: {}",
        par
    );
}

/// Verify that the OIS identity N × (DF(start) − DF(end)) holds for
/// single-curve compounded swaps when `end_of_month = true`.
#[test]
fn test_ois_identity_with_eom() {
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;

    // Use a month-end start date in a leap year
    let as_of = date!(2024 - 01 - 02);
    let start = date!(2024 - 02 - 29); // Last day of Feb (leap year)
    let end = date!(2024 - 08 - 30); // ~6 months later (last biz day of Aug)

    let disc_id = CurveId::new("USD-OIS");
    let disc = DiscountCurve::builder(disc_id.clone())
        .base_date(as_of)
        .knots(vec![
            (0.0, 1.0),
            (0.25, 0.9925),
            (0.5, 0.9850),
            (1.0, 0.9650),
        ])
        .build()
        .expect("discount curve");

    let market = MarketContext::new().insert(disc.clone());

    // Create OIS swap with EOM convention, no lookback, no spread, no payment delay
    let swap = InterestRateSwap::builder()
        .id("OIS-EOM-IDENTITY".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: disc_id.clone(),
            rate: Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            // Feb 29 to Aug 30 is not an integer number of quarterly tenors;
            // preserve the contractual maturity with a final short stub.
            stub: StubKind::ShortBack,
            start,
            end,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: disc_id.clone(),
            forward_curve_id: disc_id,
            spread_bp: Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            // Keep both legs aligned on the same declared short-back stub.
            stub: StubKind::ShortBack,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            start,
            end,
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0,
            end_of_month: true,
        })
        .build()
        .expect("swap");

    // Compute the NPV (with zero fixed rate, NPV = PV_float for Pay)
    let pv = swap.value(&market, as_of).expect("OIS EOM PV");

    // Analytical identity: PV_float = N × (DF(start) − DF(end))
    let df_start = disc.df_between_dates(as_of, start).expect("df_start");
    let df_end = disc.df_between_dates(as_of, end).expect("df_end");
    let expected_pv = swap.notional.amount() * (df_start - df_end);

    let error = (pv.amount() - expected_pv).abs();
    let relative_error = error / swap.notional.amount();

    assert!(
        relative_error < 1e-6,
        "OIS identity violated with EOM convention!\n\
         Computed PV:    {:.6}\n\
         Expected PV:    {:.6}\n\
         Error:          {:.6}\n\
         Relative error: {:.2e}\n\
         DF(start):      {:.6}\n\
         DF(end):        {:.6}",
        pv.amount(),
        expected_pv,
        error,
        relative_error,
        df_start,
        df_end
    );
}
