//! Vega tests for interest rate options.
//!
//! Validates sensitivity to implied volatility.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurface};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{ExerciseStyle, SettlementType};
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
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
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

fn build_flat_vol_surface(vol: f64, _base_date: Date, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 1.0, 5.0, 10.0])
        .strikes(&[0.01, 0.03, 0.05, 0.07, 0.10])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

fn create_standard_cap(as_of: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
        id: "CAP_TEST".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(strike).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_cap_vega_positive() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = create_standard_cap(as_of, end, 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let vega = *result.measures.get("vega").unwrap();

    // Long option has positive vega
    assert!(vega > 0.0, "Cap vega should be positive: {}", vega);
}

#[test]
fn test_floor_vega_positive() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let floor = CapFloor {
        id: "FLOOR_TEST".into(),
        rate_option_type: RateOptionType::Floor,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = floor
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let vega = *result.measures.get("vega").unwrap();

    // Long floor has positive vega
    assert!(vega > 0.0, "Floor vega should be positive: {}", vega);
}

#[test]
fn test_atm_vega_higher_than_otm() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2027 - 01 - 01);

    let otm_cap = create_standard_cap(as_of, end, 0.10); // Far OTM
    let atm_cap = create_standard_cap(as_of, end, 0.05); // ATM

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let otm_vega = *otm_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    let atm_vega = *atm_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    // ATM options typically have higher vega than OTM
    assert!(
        atm_vega > otm_vega,
        "ATM vega ({}) should be > OTM vega ({})",
        atm_vega,
        otm_vega
    );
}

#[test]
fn test_vega_scales_with_maturity() {
    let as_of = date!(2024 - 01 - 01);

    let short_cap = create_standard_cap(as_of, date!(2025 - 01 - 01), 0.05);
    let long_cap = create_standard_cap(as_of, date!(2034 - 01 - 01), 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let short_vega = *short_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    let long_vega = *long_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    // Longer maturity cap has more caplets, should have higher aggregate vega
    assert!(
        long_vega > short_vega,
        "10Y vega ({}) should be > 1Y vega ({})",
        long_vega,
        short_vega
    );
}

/// Market-quote vega and direct Hull-White σ vega live on distinct axes.
#[test]
fn test_hull_white_1f_market_and_sigma_vegas_are_distinct() {
    use finstack_quant_valuations::pricer::ModelKey;

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface =
        build_flat_vol_surface(0.01, as_of, "USD_CAP_VOL").with_quote_type(VolQuoteType::Normal);

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let opts = finstack_quant_valuations::instruments::PricingOptions::default()
        .with_model(ModelKey::HullWhite1F);

    // Explicit κ/σ make PV independent of the market surface, while direct
    // model-parameter σ sensitivity remains non-zero.
    let mut atm_cap = create_standard_cap(as_of, end, 0.05);
    atm_cap.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.03);
    atm_cap.pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    let result = atm_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega, MetricId::HwSigmaVega],
            opts.clone(),
        )
        .unwrap()
        .measures;
    let market_vega = *result.get("vega").unwrap();
    let sigma_vega = *result.get("hw_sigma_vega").unwrap();
    assert!(
        market_vega.abs() < 1e-8,
        "fixed HW params must have zero market-quote vega, got {market_vega}"
    );
    assert!(
        sigma_vega > 0.0,
        "HW1F direct sigma vega must be positive, got {sigma_vega}"
    );

    // Away from the money, changing σ changes direct model-parameter vega.
    let mut otm_cap = create_standard_cap(as_of, end, 0.08);
    otm_cap.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.03);
    otm_cap.pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    let low_sigma_vega = *otm_cap
        .price_with_metrics(&market, as_of, &[MetricId::HwSigmaVega], opts.clone())
        .unwrap()
        .measures
        .get("hw_sigma_vega")
        .unwrap();

    let mut otm_cap_high = otm_cap;
    otm_cap_high.pricing_overrides.model_config.hw1f_sigma = Some(0.02);
    let high_sigma_vega = *otm_cap_high
        .price_with_metrics(&market, as_of, &[MetricId::HwSigmaVega], opts)
        .unwrap()
        .measures
        .get("hw_sigma_vega")
        .unwrap();

    assert!(
        (high_sigma_vega - low_sigma_vega).abs() > 1e-6,
        "HW1F OTM vega must respond to hw1f_sigma: low_sigma={low_sigma_vega}, high_sigma={high_sigma_vega}"
    );
}

#[test]
fn test_hull_white_1f_surface_shock_moves_pv_and_vega() {
    use finstack_quant_core::market_data::bumps::{
        BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
    };
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::pricer::ModelKey;

    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let mut cap = create_standard_cap(as_of, end, 0.05);
    cap.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.03);
    cap.pricing_overrides.model_config.hw1f_sigma = None;

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface =
        build_flat_vol_surface(0.01, as_of, "USD_CAP_VOL").with_quote_type(VolQuoteType::Normal);
    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);
    let shocked_market = market
        .bump([MarketBump::Curve {
            id: CurveId::from("USD_CAP_VOL"),
            spec: BumpSpec {
                mode: BumpMode::Multiplicative,
                units: BumpUnits::Factor,
                value: 1.25,
                bump_type: BumpType::Parallel,
            },
        }])
        .expect("surface shock");
    let opts = finstack_quant_valuations::instruments::PricingOptions::default()
        .with_model(ModelKey::HullWhite1F);

    let base = cap
        .price_with_metrics(&market, as_of, &[MetricId::Vega], opts.clone())
        .unwrap();
    let shocked = cap
        .price_with_metrics(&shocked_market, as_of, &[MetricId::Vega], opts)
        .unwrap();

    let base_pv = base.value.amount();
    let shocked_pv = shocked.value.amount();
    let vega = *base.measures.get("vega").unwrap();
    assert!(
        vega > 0.0,
        "surface-driven HW vega should be positive: {vega}"
    );
    assert!(
        (shocked_pv - base_pv).abs() > 1e-6,
        "HW cap PV must move under a vol surface shock: base={base_pv}, shocked={shocked_pv}"
    );
}

#[test]
fn test_vega_reasonable_magnitude() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = create_standard_cap(as_of, end, 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let vega = *result.measures.get("vega").unwrap();

    // Vega should be positive and reasonable (per 1% vol change for $1M 5Y cap at 30% vol)
    // Expected ~$5k-$30k for typical ATM cap; 50k is a generous upper sanity bound
    assert!(vega > 0.0, "Vega should be positive");
    assert!(
        vega < 50_000.0,
        "Vega should be reasonable for $1M 5Y cap: {}",
        vega
    );
}
