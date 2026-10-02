//! Pricing tests for inflation caps/floors.

use crate::instruments::inflation_swap::fixtures::{
    flat_discount, flat_inflation_curve, simple_index,
};
use crate::instruments::test_support::volatility::flat_vol_surface;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::rates::inflation_cap_floor::InflationCapFloor;
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations::instruments::RateOptionType;
use finstack_quant_valuations::pricer::ModelKey;
use rust_decimal::Decimal;
use time::{Duration, Month};

#[test]
fn cap_floor_parity_matches_same_forward_swap_across_quote_volatilities() {
    use finstack_quant_core::market_data::scalars::InflationIndex;
    use finstack_quant_core::market_data::surfaces::VolQuoteType;
    use finstack_quant_valuations::instruments::YoYInflationSwap;
    use time::macros::date;

    let as_of = date!(2025 - 01 - 02);
    for start in [as_of, date!(2026 - 01 - 02)] {
        let end = start.replace_year(start.year() + 1).unwrap();
        let mut cap = InflationCapFloor::example().unwrap();
        cap.start_date = start;
        cap.maturity = end;
        cap.rate_option_type = RateOptionType::Caplet;
        cap.lag = Some(InflationLag::None);
        cap.business_day_convention = BusinessDayConvention::Unadjusted;
        let mut swap = YoYInflationSwap::example().unwrap();
        swap.start_date = start;
        swap.maturity = end;
        swap.lag = Some(InflationLag::None);
        swap.business_day_convention = BusinessDayConvention::Unadjusted;
        for (model, quote_type, sigma) in [
            (ModelKey::Black76, VolQuoteType::BlackLognormal, 0.0),
            (ModelKey::Black76, VolQuoteType::BlackLognormal, 0.2),
            (ModelKey::Black76, VolQuoteType::BlackLognormal, 0.4),
            (ModelKey::Normal, VolQuoteType::Normal, 0.0),
            (ModelKey::Normal, VolQuoteType::Normal, 0.01),
            (ModelKey::Normal, VolQuoteType::Normal, 0.02),
        ] {
            let market = MarketContext::new()
                .insert(flat_discount("USD-OIS", as_of, 0.03).unwrap())
                .insert(flat_inflation_curve("US-CPI", as_of, 300.0, 0.02).unwrap())
                .insert_inflation_index(
                    "US-CPI",
                    InflationIndex::new("US-CPI", vec![(as_of, 300.0)], Currency::USD).unwrap(),
                )
                .insert_surface(
                    flat_vol_surface("USD-INFL-VOL", &[1.0, 2.0], &[0.01, 0.02, 0.03], sigma)
                        .with_quote_type(quote_type)
                        .expect("valid quote convention"),
                );
            for strike in [0.01, 0.02, 0.03] {
                cap.strike = Decimal::try_from(strike).unwrap();
                swap.fixed_rate = cap.strike;
                let mut floor = cap.clone();
                floor.rate_option_type = RateOptionType::Floorlet;
                let parity = cap.npv_raw_with_model(&market, as_of, model).unwrap()
                    - floor.npv_raw_with_model(&market, as_of, model).unwrap();
                let swap_pv = swap.npv_raw(&market, as_of).unwrap();
                assert!(
                    (parity - swap_pv).abs() < 1e-7,
                    "{model:?}, sigma {sigma}, strike {strike}, start {start}: {parity} vs {swap_pv}"
                );
            }
        }
        // A future denominator deliberately uses the same documented
        // forward-ratio approximation as the swap; it is not a joint model.
    }
}

#[test]
fn zero_and_day_lag_caplets_do_not_consume_unpublished_denominators() {
    use finstack_quant_core::market_data::scalars::{InflationIndex, InflationInterpolation};
    use finstack_quant_core::market_data::surfaces::VolQuoteType;
    use finstack_quant_core::market_data::term_structures::InflationCurve;
    use finstack_quant_valuations::instruments::rates::inflation_cap_floor::InflationVolatilityExpiry;
    use time::macros::date;

    let as_of = date!(2026 - 01 - 20);
    let last_release = date!(2026 - 03 - 13);
    let market = MarketContext::new()
        .insert(flat_discount("USD-OIS", as_of, 0.0).unwrap())
        .insert(
            InflationCurve::builder("US-CPI")
                .base_date(date!(2025 - 12 - 01))
                .base_cpi(310.0)
                .knots([(0.0, 310.0), (2.0, 310.0)])
                .build()
                .unwrap(),
        )
        .insert_inflation_index(
            "US-CPI",
            InflationIndex::new(
                "US-CPI",
                vec![
                    (date!(2026 - 01 - 01), 306.0),
                    (date!(2026 - 02 - 01), 900.0),
                ],
                Currency::USD,
            )
            .unwrap()
            .with_publication_dates(vec![
                (date!(2026 - 01 - 01), date!(2026 - 02 - 13)),
                (date!(2026 - 02 - 01), last_release),
            ])
            .unwrap(),
        )
        .insert_surface(
            flat_vol_surface("USD-INFL-VOL", &[0.01, 1.0], &[0.0], 0.01)
                .with_quote_type(VolQuoteType::Normal)
                .expect("valid quote convention"),
        );
    for (lag, start, end) in [
        (
            InflationLag::None,
            date!(2026 - 01 - 01),
            date!(2026 - 02 - 01),
        ),
        (
            InflationLag::Days(10),
            date!(2026 - 01 - 11),
            date!(2026 - 02 - 11),
        ),
    ] {
        let mut cap = InflationCapFloor::example().unwrap();
        cap.start_date = start;
        cap.maturity = end;
        cap.lag = Some(lag);
        cap.interpolation = Some(InflationInterpolation::Step);
        cap.rate_option_type = RateOptionType::Caplet;
        cap.volatility_expiry = InflationVolatilityExpiry::PublicationDate;
        cap.strike = Decimal::ZERO;
        let t = (last_release - as_of).whole_days() as f64 / 365.0;
        let accrual = (end - start).whole_days() as f64 / 365.0;
        // Both unpublished CPI anchors project to 310. The forward is zero,
        // irrespective of the future historical values already in the store.
        let expected =
            1_000_000.0 * accrual * 0.01 * t.sqrt() / (2.0 * std::f64::consts::PI).sqrt();
        let actual = cap
            .npv_raw_with_model(&market, as_of, ModelKey::Normal)
            .unwrap();
        assert!(
            (actual - expected).abs() < 1e-7,
            "{lag:?}: {actual} vs {expected}"
        );
    }
}

#[test]
fn monthly_publication_controls_availability_and_explicit_quote_clock() {
    use finstack_quant_core::market_data::scalars::{InflationIndex, InflationInterpolation};
    use finstack_quant_core::market_data::surfaces::VolQuoteType;
    use finstack_quant_core::market_data::term_structures::InflationCurve;
    use finstack_quant_valuations::instruments::rates::inflation_cap_floor::InflationVolatilityExpiry;
    use finstack_quant_valuations::instruments::{InflationSwap, Instrument, YoYInflationSwap};
    use time::macros::date;

    let as_of = date!(2026 - 01 - 20);
    let release = date!(2026 - 02 - 13);
    let january = date!(2026 - 01 - 01);
    let index = InflationIndex::new(
        "US-CPI",
        vec![
            (date!(2025 - 01 - 01), 300.0),
            (date!(2025 - 12 - 01), 305.0),
        ],
        Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Step)
    .with_publication_dates(vec![(january, release)])
    .unwrap();
    let market = MarketContext::new()
        .insert(flat_discount("USD-OIS", as_of, 0.0).unwrap())
        .insert(
            InflationCurve::builder("US-CPI")
                .base_date(date!(2025 - 12 - 01))
                .base_cpi(306.0)
                .knots([(0.0, 306.0), (2.0, 306.0)])
                .build()
                .unwrap(),
        )
        .insert_inflation_index("US-CPI", index.clone())
        .insert_surface(
            flat_vol_surface("USD-INFL-VOL", &[0.01, 1.0], &[0.02], 0.01)
                .with_quote_type(VolQuoteType::Normal)
                .expect("valid quote convention"),
        );
    let mut cap = InflationCapFloor::example().unwrap();
    cap.start_date = date!(2025 - 04 - 01);
    cap.maturity = date!(2026 - 04 - 01);
    cap.strike = Decimal::try_from(0.02).unwrap();
    cap.rate_option_type = RateOptionType::Caplet;
    cap.interpolation = Some(InflationInterpolation::Step);
    // Existing reference-date quotes cannot be silently reinterpreted after
    // that reference date has passed while the observation is still unknown.
    assert!(cap
        .npv_raw_with_model(&market, as_of, ModelKey::Normal)
        .is_err());
    cap.volatility_expiry = InflationVolatilityExpiry::PublicationDate;
    let pv = cap
        .npv_raw_with_model(&market, as_of, ModelKey::Normal)
        .unwrap();
    let t = (release - as_of).whole_days() as f64 / 365.0;
    let expected = 1_000_000.0 * 0.01 * t.sqrt() / (2.0 * std::f64::consts::PI).sqrt();
    assert!((pv - expected).abs() < 1e-7, "{pv} vs {expected}");

    let mut zc = InflationSwap::example().unwrap();
    zc.start_date = cap.start_date;
    zc.maturity = cap.maturity;
    let mut yoy = YoYInflationSwap::example().unwrap();
    yoy.start_date = cap.start_date;
    yoy.maturity = cap.maturity;
    for valuation_date in [as_of, release - Duration::days(1)] {
        assert!(zc.value(&market, valuation_date).is_ok());
        assert!(yoy.value(&market, valuation_date).is_ok());
        assert!(
            cap.npv_raw_with_model(&market, valuation_date, ModelKey::Normal)
                .unwrap()
                > 0.0
        );
    }
    for valuation_date in [release, release + Duration::days(1)] {
        assert!(zc.value(&market, valuation_date).is_err());
        assert!(yoy.value(&market, valuation_date).is_err());
        assert!(cap
            .npv_raw_with_model(&market, valuation_date, ModelKey::Normal)
            .is_err());
    }
    let mut observations = index.observations();
    observations.push((january, 306.0));
    let published = InflationIndex::new("US-CPI", observations, Currency::USD)
        .unwrap()
        .with_publication_dates(index.get_publication_dates())
        .unwrap();
    let published_market = market.insert_inflation_index("US-CPI", published);
    for valuation_date in [release, release + Duration::days(1)] {
        assert!(zc.value(&published_market, valuation_date).is_ok());
        assert!(yoy.value(&published_market, valuation_date).is_ok());
        let fixed_pv = cap
            .npv_raw_with_model(&published_market, valuation_date, ModelKey::Normal)
            .unwrap();
        assert!(
            fixed_pv.abs() < 1e-7,
            "known ATM payoff must have no time value"
        );
    }
}

#[test]
fn test_caplet_intrinsic_after_fixing() {
    let as_of = Date::from_calendar_date(2025, Month::April, 15).unwrap();
    let start = as_of - Duration::days(60);
    let end = as_of + Duration::days(30);

    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
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
        .rate_option_type(RateOptionType::Caplet)
        .notional(notional)
        .strike(Decimal::try_from(0.02).expect("valid decimal"))
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture"))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .business_day_convention(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .lag_opt(None)
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

    let notional = Money::new(5_000_000.0, Currency::USD).expect("valid money fixture");
    let disc = flat_discount("USD-OIS", as_of, 0.01).unwrap();
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, -0.01).unwrap();
    let index = simple_index("US-CPI-U", as_of, 300.0, Currency::USD, InflationLag::None);
    let vol_surface = flat_vol_surface("US-CPI-VOL", &[1.0], &[0.0], 0.01)
        .with_quote_type(finstack_quant_core::market_data::surfaces::VolQuoteType::Normal)
        .expect("valid quote convention");

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(infl_curve)
        .insert_inflation_index("US-CPI-U", index)
        .insert_surface(vol_surface);

    let floorlet = InflationCapFloor::builder()
        .id("INF-FLOOR".into())
        .rate_option_type(RateOptionType::Floorlet)
        .notional(notional)
        .strike(Decimal::try_from(0.0).expect("valid decimal"))
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(1, TenorUnit::Years).expect("valid tenor fixture"))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .business_day_convention(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .lag_opt(None)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let caplet = InflationCapFloor::builder()
        .id("INF-CAP".into())
        .rate_option_type(RateOptionType::Caplet)
        .notional(notional)
        .strike(Decimal::try_from(0.0).expect("valid decimal"))
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(1, TenorUnit::Years).expect("valid tenor fixture"))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .business_day_convention(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .lag_opt(None)
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

#[test]
fn inflation_cap_floor_uses_rate_option_type_key() {
    let json =
        serde_json::to_value(InflationCapFloor::example().expect("example")).expect("serialize");
    assert_eq!(json["rate_option_type"], "cap");

    let mut retired = json;
    let object = retired.as_object_mut().expect("object");
    object.remove("rate_option_type");
    // schema-rejection-test: `option_type` is now `rate_option_type`
    object.insert("option_type".to_string(), serde_json::json!("cap"));
    assert!(serde_json::from_value::<InflationCapFloor>(retired).is_err());
}
