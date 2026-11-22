//! Pricing tests for inflation caps/floors.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationExpectedRelease, InflationLag, InflationObservation, SeriesInterpolation,
};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_rates::cap_floor::RateOptionType;
use finstack_quant_valuations_rates::inflation_cap_floor::InflationCapFloor;
use finstack_quant_valuations_test_support::inflation::{flat_inflation_curve, simple_index};
use finstack_quant_valuations_test_support::test_helpers::flat_discount_curve_with_day_count;
use finstack_quant_valuations_test_support::volatility::flat_vol_surface;
use rust_decimal::Decimal;
use time::{Duration, Month};

#[test]
fn cap_floor_parity_matches_same_forward_swap_across_quote_volatilities() {
    use finstack_quant_core::market_data::surfaces::VolQuoteType;
    use finstack_quant_valuations_rates::inflation_swap::YoYInflationSwap;
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
                .try_insert(flat_discount_curve_with_day_count(
                    0.03,
                    as_of,
                    "USD-OIS",
                    DayCount::Act365F,
                ))
                .expect("insert market curve")
                .try_insert(flat_inflation_curve("US-CPI", as_of, 300.0, 0.02).unwrap())
                .expect("insert market curve")
                .insert_inflation_index(simple_index("US-CPI", as_of, 300.0, Currency::USD))
                .insert_surface(
                    finstack_quant_valuations_test_support::volatility::relabel_quote_type(
                        &flat_vol_surface("USD-INFL-VOL", &[1.0, 2.0], &[0.01, 0.02, 0.03], sigma),
                        quote_type,
                    )
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
    use finstack_quant_core::market_data::scalars::{InflationIndex, SeriesInterpolation};
    use finstack_quant_core::market_data::surfaces::VolQuoteType;
    use finstack_quant_core::market_data::term_structures::InflationCurve;
    use finstack_quant_valuations_rates::inflation_cap_floor::InflationVolatilityExpiry;
    use time::macros::date;

    let as_of = date!(2026 - 01 - 20);
    let last_release = date!(2026 - 03 - 13);
    let market = MarketContext::new()
        .try_insert(flat_discount_curve_with_day_count(
            0.0,
            as_of,
            "USD-OIS",
            DayCount::Act365F,
        ))
        .expect("insert market curve")
        .try_insert(
            InflationCurve::builder("US-CPI")
                .base_date(date!(2025 - 12 - 01))
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::LogLinear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .knots([(0.0, 310.0), (2.0, 310.0)])
                .build()
                .unwrap(),
        )
        .expect("insert market curve")
        .insert_inflation_index(
            InflationIndex::new("US-CPI", vec![], Currency::USD, as_of)
                .unwrap()
                .with_expected_releases(vec![
                    InflationExpectedRelease {
                        reference_month: date!(2026 - 01 - 01),
                        expected_release: date!(2026 - 02 - 13),
                        known_on: as_of,
                    },
                    InflationExpectedRelease {
                        reference_month: date!(2026 - 02 - 01),
                        expected_release: last_release,
                        known_on: as_of,
                    },
                ])
                .unwrap(),
        )
        .insert_surface(
            finstack_quant_valuations_test_support::volatility::flat_vol_surface_with_quote_type(
                "USD-INFL-VOL",
                &[0.01, 1.0],
                &[0.0],
                0.01,
                VolQuoteType::Normal,
            ),
        );
    for (lag, start, end) in [
        (
            InflationLag::None,
            date!(2026 - 01 - 01),
            date!(2026 - 02 - 01),
        ),
        (
            InflationLag::days(10),
            date!(2026 - 01 - 11),
            date!(2026 - 02 - 11),
        ),
    ] {
        let mut cap = InflationCapFloor::example().unwrap();
        cap.start_date = start;
        cap.maturity = end;
        cap.lag = Some(lag);
        cap.interpolation = Some(SeriesInterpolation::Step);
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
    use finstack_quant_core::market_data::scalars::{InflationIndex, SeriesInterpolation};
    use finstack_quant_core::market_data::surfaces::VolQuoteType;
    use finstack_quant_core::market_data::term_structures::InflationCurve;
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_rates::inflation_cap_floor::InflationVolatilityExpiry;
    use finstack_quant_valuations_rates::inflation_swap::{InflationSwap, YoYInflationSwap};
    use time::macros::date;

    let as_of = date!(2026 - 01 - 20);
    let release = date!(2026 - 02 - 13);
    let january = date!(2026 - 01 - 01);
    let index = InflationIndex::new(
        "US-CPI",
        vec![
            InflationObservation {
                reference_month: date!(2025 - 01 - 01),
                value: 300.0,
                available_on: date!(2025 - 02 - 13),
            },
            InflationObservation {
                reference_month: date!(2025 - 12 - 01),
                value: 305.0,
                available_on: date!(2026 - 01 - 13),
            },
        ],
        Currency::USD,
        as_of,
    )
    .unwrap()
    .with_expected_releases(vec![InflationExpectedRelease {
        reference_month: january,
        expected_release: release,
        known_on: as_of,
    }])
    .unwrap();
    let market = MarketContext::new()
        .try_insert(flat_discount_curve_with_day_count(
            0.0,
            as_of,
            "USD-OIS",
            DayCount::Act365F,
        ))
        .expect("insert market curve")
        .try_insert(
            InflationCurve::builder("US-CPI")
                .base_date(date!(2025 - 12 - 01))
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::LogLinear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .knots([(0.0, 306.0), (2.0, 306.0)])
                .build()
                .unwrap(),
        )
        .expect("insert market curve")
        .insert_inflation_index(index.clone())
        .insert_surface(
            finstack_quant_valuations_test_support::volatility::relabel_quote_type(
                &flat_vol_surface("USD-INFL-VOL", &[0.01, 1.0], &[0.02], 0.01),
                VolQuoteType::Normal,
            )
            .expect("valid quote convention"),
        );
    let mut cap = InflationCapFloor::example().unwrap();
    cap.start_date = date!(2025 - 04 - 01);
    cap.maturity = date!(2026 - 04 - 01);
    cap.strike = Decimal::try_from(0.02).unwrap();
    cap.rate_option_type = RateOptionType::Caplet;
    cap.interpolation = Some(SeriesInterpolation::Step);
    // Existing reference-date quotes cannot be silently reinterpreted after
    // that reference date has passed while the observation is still unknown.
    assert!(
        cap.npv_raw_with_model(&market, as_of, ModelKey::Normal)
            .is_err()
    );
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
        assert!(zc.value(&market, valuation_date).is_ok());
        assert!(yoy.value(&market, valuation_date).is_ok());
        assert!(
            cap.npv_raw_with_model(&market, valuation_date, ModelKey::Normal)
                .is_err()
        );
    }
    let mut observations = index.observations().to_vec();
    observations.push(InflationObservation {
        reference_month: january,
        value: 306.0,
        available_on: release,
    });
    let published = InflationIndex::new("US-CPI", observations, Currency::USD, release)
        .unwrap()
        .with_expected_releases(index.get_expected_releases().to_vec())
        .unwrap();
    let published_market = market.insert_inflation_index(published);
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
    let disc = flat_discount_curve_with_day_count(0.02, as_of, "USD-OIS", DayCount::Act365F);
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.02).unwrap();
    let index = simple_index("US-CPI-U", as_of, 300.0, Currency::USD);
    let vol_surface = flat_vol_surface("US-CPI-VOL", &[0.25], &[0.02], 0.20);

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(infl_curve)
        .expect("insert market curve")
        .insert_inflation_index(index)
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
        .lag(InflationLag::months(3))
        .interpolation(SeriesInterpolation::Linear)
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let idx = ctx.get_inflation_index("US-CPI-U").unwrap();
    let reference = |date| {
        finstack_quant_valuations_core::instruments::pricing::inflation::resolve_reference_inflation(
        &ctx, "US-CPI-U", date, idx.knowledge_date(), InflationLag::months(3), SeriesInterpolation::Linear,
    ).unwrap().value
    };
    let cpi_start = reference(start);
    let cpi_end = reference(end);
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
    let df = disc_curve.df(t_pay).expect("df");
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
    let disc = flat_discount_curve_with_day_count(0.01, as_of, "USD-OIS", DayCount::Act365F);
    let infl_curve = flat_inflation_curve("US-CPI-U", as_of, 300.0, -0.01).unwrap();
    let index = simple_index("US-CPI-U", as_of, 300.0, Currency::USD);
    let vol_surface =
        finstack_quant_valuations_test_support::volatility::flat_vol_surface_with_quote_type(
            "US-CPI-VOL",
            &[1.0],
            &[0.0],
            0.01,
            finstack_quant_core::market_data::surfaces::VolQuoteType::Normal,
        );

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(infl_curve)
        .expect("insert market curve")
        .insert_inflation_index(index)
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
        .lag(InflationLag::None)
        .interpolation(SeriesInterpolation::Linear)
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
        .lag(InflationLag::None)
        .interpolation(SeriesInterpolation::Linear)
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
