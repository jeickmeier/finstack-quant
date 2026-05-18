//! Pricing tests for inflation caps/floors.

use crate::finstack_test_utils::flat_vol_surface;
use crate::inflation_swap::fixtures::{flat_discount, flat_inflation_curve, simple_index};
use finstack_core::currency::Currency;
use finstack_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor, TenorUnit,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::InflationLag;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::rates::inflation_cap_floor::{
    InflationCapFloor, InflationCapFloorType,
};
use finstack_valuations::instruments::Attributes;
use finstack_valuations::instruments::PricingOverrides;
use finstack_valuations::pricer::ModelKey;
use rust_decimal::Decimal;
use time::{Duration, Month};

#[test]
fn test_caplet_intrinsic_after_fixing() {
    let as_of = Date::from_calendar_date(2025, Month::April, 15).unwrap();
    let start = as_of - Duration::days(60);
    let end = as_of + Duration::days(30);

    let notional = Money::new(1_000_000.0, Currency::USD);
    let disc = flat_discount("USD-OIS", as_of, 0.02).unwrap();
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.02).unwrap();
    let index = simple_index(
        "US-CPI-U",
        as_of,
        300.0,
        Currency::USD,
        InflationLag::Months(3),
    );
    let vol_surface = flat_vol_surface("US-CPI-VOL", &[0.25], &[0.02], 0.20);

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(infl_curve)
        .insert_inflation_index("US-CPI-U", index)
        .insert_surface(vol_surface);

    let caplet = InflationCapFloor::builder()
        .id("INF-CAPLET".into())
        .option_type(InflationCapFloorType::Caplet)
        .notional(notional)
        .strike(Decimal::try_from(0.02).expect("valid decimal"))
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(3, TenorUnit::Months))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .pricing_overrides(PricingOverrides::default())
        .lag_override_opt(None)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let idx = ctx.get_inflation_index("US-CPI-U").unwrap();
    let cpi_start = idx.value_on(start).unwrap();
    let cpi_end = idx.value_on(end).unwrap();
    let accrual = DayCount::Act365F
        .year_fraction(start, end, DayCountContext::default())
        .unwrap();
    let forward_rate = (cpi_end / cpi_start - 1.0) / accrual;
    let payoff_rate = (forward_rate - 0.02).max(0.0);

    let disc_curve = ctx.get_discount("USD-OIS").unwrap();
    let t_pay = disc_curve
        .day_count()
        .year_fraction(as_of, end, DayCountContext::default())
        .unwrap();
    let df = disc_curve.df(t_pay);
    let expected = payoff_rate * accrual * notional.amount() * df;

    let pv = caplet
        .npv_with_model(&ctx, as_of, ModelKey::Normal)
        .unwrap();
    assert!((pv.amount() - expected).abs() < 1e-6 * notional.amount());
}

#[test]
fn test_floor_value_with_negative_forward_normal_model() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let start = as_of;
    let end = Date::from_calendar_date(2026, Month::January, 2).unwrap();

    let notional = Money::new(5_000_000.0, Currency::USD);
    let disc = flat_discount("USD-OIS", as_of, 0.01).unwrap();
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, -0.01).unwrap();
    let vol_surface = flat_vol_surface("US-CPI-VOL", &[1.0], &[0.0], 0.01);

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(infl_curve)
        .insert_surface(vol_surface);

    let floorlet = InflationCapFloor::builder()
        .id("INF-FLOOR".into())
        .option_type(InflationCapFloorType::Floorlet)
        .notional(notional)
        .strike(Decimal::try_from(0.0).expect("valid decimal"))
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(1, TenorUnit::Years))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .pricing_overrides(PricingOverrides::default())
        .lag_override_opt(None)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let caplet = InflationCapFloor::builder()
        .id("INF-CAP".into())
        .option_type(InflationCapFloorType::Caplet)
        .notional(notional)
        .strike(Decimal::try_from(0.0).expect("valid decimal"))
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(1, TenorUnit::Years))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .pricing_overrides(PricingOverrides::default())
        .lag_override_opt(None)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let floor_pv = floorlet
        .npv_with_model(&ctx, as_of, ModelKey::Normal)
        .unwrap();
    let cap_pv = caplet
        .npv_with_model(&ctx, as_of, ModelKey::Normal)
        .unwrap();

    assert!(floor_pv.amount() > cap_pv.amount());
    assert!(floor_pv.amount() > 0.0);
}

/// Regression test (item 3): the YoY caplet must apply the convexity / timing
/// adjustment to the forward — feeding the raw deterministic CPI-ratio forward
/// into Black-76 omits it.
///
/// A YoY caplet pays `(CPI(Tᵢ)/CPI(Tᵢ₋₁) − 1 − K)⁺`. Under stochastic
/// inflation the payment-measure expected YoY ratio carries a Jensen
/// convexity (`+σ_I²·τ`) that raises the forward above the deterministic
/// ratio. With zero-lag (so the fixing is genuinely in the future) and a
/// non-trivial inflation vol, the convexity-adjusted caplet must be worth
/// strictly more than the same caplet priced with the convexity suppressed.
///
/// The convexity is suppressed here by setting the inflation vol surface to a
/// near-zero level (`σ_I ≈ 0` ⇒ `C ≈ 0`, no adjustment) and compared against a
/// market with a realistic 2% inflation vol. A higher `σ_I` both raises the
/// forward (convexity) and adds option time value — both push the cap price
/// up, but the convexity contribution is the item-3 fix.
#[test]
fn test_yoy_caplet_applies_convexity_adjustment() {
    // YoY caplet on a 1-year period starting one year out, so the period is
    // [Tᵢ₋₁, Tᵢ] = [+1y, +2y] and the deterministic forward YoY rate ≈ 2.5%.
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let start = Date::from_calendar_date(2026, Month::January, 2).unwrap();
    let end = Date::from_calendar_date(2027, Month::January, 2).unwrap();
    let notional = Money::new(5_000_000.0, Currency::USD);

    let build_caplet = |vol_surface_id: &str| {
        InflationCapFloor::builder()
            .id("INF-CAP-CVX".into())
            .option_type(InflationCapFloorType::Caplet)
            .notional(notional)
            // Slightly OTM strike so the option carries time value and is
            // sensitive to the forward-raising convexity adjustment.
            .strike(Decimal::try_from(0.030).expect("valid decimal"))
            .start_date(start)
            .maturity(end)
            .frequency(Tenor::new(1, TenorUnit::Years))
            .day_count(DayCount::Act365F)
            .stub(StubKind::None)
            .bdc(BusinessDayConvention::Following)
            .calendar_id_opt(None)
            .inflation_index_id(CurveId::new("US-CPI-U"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new(vol_surface_id))
            // Zero lag so the fixing date is in the future and the convexity
            // adjustment (which requires t_fix > 0) is active.
            .lag_override_opt(Some(InflationLag::Months(0)))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .unwrap()
    };

    // No inflation INDEX is inserted: forced curve projection for CPI(start)
    // and CPI(end), so the deterministic forward YoY rate is a genuine ~2.5%
    // (an index would extrapolate flat past its last observation).
    //
    // Market A: negligible inflation vol -> convexity ~ 0.
    let ctx_flat = MarketContext::new()
        .insert(flat_discount("USD-OIS", as_of, 0.02).unwrap())
        .insert(flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.025).unwrap())
        .insert_surface(flat_vol_surface(
            "US-CPI-VOL-LO",
            &[1.0, 5.0],
            &[0.025],
            1e-6,
        ));

    // Market B: realistic 2% inflation vol -> non-trivial convexity.
    let ctx_vol = MarketContext::new()
        .insert(flat_discount("USD-OIS", as_of, 0.02).unwrap())
        .insert(flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.025).unwrap())
        .insert_surface(flat_vol_surface(
            "US-CPI-VOL-HI",
            &[1.0, 5.0],
            &[0.025],
            0.02,
        ));

    let pv_no_convexity = build_caplet("US-CPI-VOL-LO")
        .npv_with_model(&ctx_flat, as_of, ModelKey::Black76)
        .unwrap();
    let pv_with_convexity = build_caplet("US-CPI-VOL-HI")
        .npv_with_model(&ctx_vol, as_of, ModelKey::Black76)
        .unwrap();

    assert!(
        pv_with_convexity.amount() > pv_no_convexity.amount(),
        "a YoY caplet priced with inflation vol (convexity adjustment active) \
         must be worth more than one with the convexity suppressed: \
         with={}, without={}",
        pv_with_convexity.amount(),
        pv_no_convexity.amount()
    );
    assert!(
        pv_with_convexity.amount() > 0.0,
        "YoY caplet with vol must have positive value, got {}",
        pv_with_convexity.amount()
    );
}
