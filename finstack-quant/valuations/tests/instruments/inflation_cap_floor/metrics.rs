//! Metric tests for inflation caps/floors.

use crate::instruments::inflation_swap::fixtures::{flat_discount, flat_inflation_curve};
use crate::instruments::test_support::volatility::flat_vol_surface;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::rates::inflation_cap_floor::InflationCapFloor;
use finstack_quant_valuations::instruments::RateOptionType;
use finstack_quant_valuations::instruments::{
    Attributes, Instrument, InstrumentPricingOverrides, PricingOptions,
};
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

fn build_caplet() -> InflationCapFloor {
    let start = Date::from_calendar_date(2026, Month::January, 2).unwrap();
    let end = Date::from_calendar_date(2027, Month::January, 2).unwrap();
    InflationCapFloor::builder()
        .id("INF-CAP-VEGA".into())
        .rate_option_type(RateOptionType::Caplet)
        .notional(Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"))
        .strike(Decimal::try_from(0.025).expect("valid decimal"))
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
        .unwrap()
}

fn build_market(as_of: Date, vol: f64) -> MarketContext {
    MarketContext::new()
        .insert(flat_discount("USD-OIS", as_of, 0.02).unwrap())
        .insert(flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.025).unwrap())
        .insert_surface(flat_vol_surface("US-CPI-VOL", &[1.0, 5.0], &[0.025], vol))
}

#[test]
fn risk_uses_selected_model_and_declared_bump_units() {
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_core::market_data::surfaces::VolQuoteType;
    use finstack_quant_valuations::pricer::ModelKey;
    use time::macros::date;

    let as_of = date!(2025 - 01 - 02);
    for (model, quote_type, inflation, strike, sigma) in [
        (ModelKey::Normal, VolQuoteType::Normal, -0.01, 0.0, 0.02),
        (
            ModelKey::Black76,
            VolQuoteType::BlackLognormal,
            0.025,
            0.025,
            0.20,
        ),
    ] {
        let market_at_vol = |vol| {
            MarketContext::new()
                .insert(flat_discount("USD-OIS", as_of, 0.02).unwrap())
                .insert(flat_inflation_curve("US-CPI-U", as_of, 300.0, inflation).unwrap())
                .insert_surface(
                    flat_vol_surface("US-CPI-VOL", &[1.0, 5.0], &[strike], vol)
                        .with_quote_type(quote_type)
                        .expect("valid quote convention"),
                )
        };
        let market = market_at_vol(sigma);
        let mut option = build_caplet();
        option.strike = Decimal::try_from(strike).unwrap();
        let options = PricingOptions::default().with_model(model);
        let result = option
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Vega, MetricId::Inflation01, MetricId::Gamma],
                options,
            )
            .unwrap();
        let price_only = option
            .price_with_metrics(
                &market,
                as_of,
                &[],
                PricingOptions::default().with_model(model),
            )
            .unwrap();
        assert_eq!(result.value, price_only.value);

        let price =
            |market: &MarketContext| option.npv_raw_with_model(market, as_of, model).unwrap();
        let vega =
            (price(&market_at_vol(sigma + 0.01)) - price(&market_at_vol(sigma - 0.01))) / 2.0;
        let bumped = |amount| {
            market
                .bump([MarketBump::Curve {
                    id: option.inflation_index_id.clone(),
                    spec: BumpSpec::inflation_shift_pct(amount),
                }])
                .unwrap()
        };
        let up = price(&bumped(0.01));
        let down = price(&bumped(-0.01));
        let inflation01 = (up - down) / 2.0;
        let gamma = (up - 2.0 * price(&market) + down) / 1e-8;
        for (metric, expected) in [
            (MetricId::Vega, vega),
            (MetricId::Inflation01, inflation01),
            (MetricId::Gamma, gamma),
        ] {
            let actual = result.measures[metric.as_str()];
            assert!(
                (actual - expected).abs() < 1e-8 * expected.abs().max(1.0),
                "{model:?} {metric:?}: {actual} vs {expected}"
            );
        }
    }
}

#[test]
fn risk_retains_custom_registered_pricing_dispatch() {
    use finstack_quant_core::market_data::surfaces::VolQuoteType;
    use finstack_quant_valuations::pricer::{
        standard_pricer_registry, InstrumentType, ModelKey, Pricer, PricerKey, PricingError,
        PricingErrorContext,
    };
    use finstack_quant_valuations::results::ValuationResult;
    use time::macros::date;

    struct ScaledNormalPricer;
    impl Pricer for ScaledNormalPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::InflationCapFloor, ModelKey::Normal)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            market: &MarketContext,
            as_of: Date,
        ) -> Result<ValuationResult, PricingError> {
            let amount = self.price_raw_dyn(instrument, market, as_of)?;
            Ok(ValuationResult::stamped(
                instrument.id(),
                as_of,
                Money::new(amount, Currency::USD).unwrap(),
            ))
        }

        fn price_raw_dyn(
            &self,
            instrument: &dyn Instrument,
            market: &MarketContext,
            as_of: Date,
        ) -> Result<f64, PricingError> {
            let option = instrument
                .as_any()
                .downcast_ref::<InflationCapFloor>()
                .unwrap();
            option
                .npv_raw_with_model(market, as_of, ModelKey::Normal)
                .map(|amount| 3.0 * amount)
                .map_err(|error| {
                    PricingError::model_failure_with_context(
                        error.to_string(),
                        PricingErrorContext::default(),
                    )
                })
        }
    }

    let as_of = date!(2025 - 01 - 02);
    let market = build_market(as_of, 0.02).insert_surface(
        flat_vol_surface("US-CPI-VOL", &[1.0, 5.0], &[0.025], 0.02)
            .with_quote_type(VolQuoteType::Normal)
            .expect("valid quote convention"),
    );
    let option = build_caplet();
    let metrics = [MetricId::Vega, MetricId::Inflation01, MetricId::Gamma];
    let reference = option
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            PricingOptions::default().with_model(ModelKey::Normal),
        )
        .unwrap();
    let mut registry = standard_pricer_registry().clone();
    registry.replace(ScaledNormalPricer);
    let custom = registry
        .price_with_metrics(
            &option,
            ModelKey::Normal,
            &market,
            as_of,
            &metrics,
            PricingOptions::default(),
        )
        .unwrap();
    assert!((custom.value.amount() - 3.0 * reference.value.amount()).abs() < 1e-7);
    for metric in metrics {
        let expected = 3.0 * reference.measures[metric.as_str()];
        let actual = custom.measures[metric.as_str()];
        assert!((actual - expected).abs() < 1e-7 * expected.abs().max(1.0));
    }
}

/// Vega must be expressed per vol point (per 1% = 0.01 absolute vol change),
/// matching the workspace-wide convention (swaption `VOL_PCT_SCALE`, nominal
/// cap/floor, CMS, fd_greeks). For a flat surface the reference value is the
/// central difference of PV under ±1 vol point, divided by 2 vol points:
/// `(PV(σ+0.01) − PV(σ−0.01)) / 2`.
#[test]
fn vega_is_per_vol_point() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let vol = 0.02;
    let market = build_market(as_of, vol);
    let caplet = build_caplet();

    let result = caplet
        .price_with_metrics(&market, as_of, &[MetricId::Vega], PricingOptions::default())
        .unwrap();
    let vega = *result.measures.get("vega").unwrap();

    let bump = 0.01;
    let pv_up = caplet
        .value(&build_market(as_of, vol + bump), as_of)
        .unwrap()
        .amount();
    let pv_down = caplet
        .value(&build_market(as_of, vol - bump), as_of)
        .unwrap()
        .amount();
    let expected = (pv_up - pv_down) / 2.0;

    assert!(
        expected.abs() > 1.0,
        "test setup must have non-trivial vega, got reference {expected}"
    );
    assert!(
        (vega - expected).abs() <= 1e-6 * expected.abs().max(1.0),
        "vega must be per vol point: metric={vega}, reference={expected} \
         (a ~100x mismatch means the per-unit-vol scaling regressed)"
    );
}

/// Build a 1Y inflation cap/floor of the given type and strike (otherwise
/// identical to `build_caplet`).
fn build_option(rate_option_type: RateOptionType, strike: f64) -> InflationCapFloor {
    let start = Date::from_calendar_date(2026, Month::January, 2).unwrap();
    let end = Date::from_calendar_date(2027, Month::January, 2).unwrap();
    InflationCapFloor::builder()
        .id("INF-CF".into())
        .rate_option_type(rate_option_type)
        .notional(Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"))
        .strike(Decimal::try_from(strike).expect("valid decimal"))
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
        .unwrap()
}

/// `Gamma` is registered but was previously unexercised. A long inflation cap
/// is convex in the underlying inflation rate, so its gamma is positive.
#[test]
fn test_gamma_positive_for_long_cap() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let market = build_market(as_of, 0.02);
    let caplet = build_caplet();

    let gamma = *caplet
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    assert!(gamma.is_finite(), "gamma should be finite, got {gamma}");
    assert!(
        gamma > 0.0,
        "long cap gamma should be positive, got {gamma}"
    );
}

/// `Dv01` and `BucketedDv01` are registered but were previously unexercised.
/// Both must be finite and the bucketed key-rate DV01s must reconcile with the
/// parallel DV01.
#[test]
fn test_dv01_and_bucketed_dv01_reconcile() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let market = build_market(as_of, 0.02);
    let caplet = build_caplet();

    let result = caplet
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::BucketedDv01],
            PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();
    let bucketed = *result.measures.get("bucketed_dv01").unwrap();

    assert!(dv01.is_finite(), "DV01 should be finite, got {dv01}");
    assert!(
        bucketed.is_finite(),
        "BucketedDv01 aggregate should be finite, got {bucketed}"
    );
    assert!(
        (bucketed - dv01).abs() <= 1.0 + 0.05 * dv01.abs(),
        "BucketedDv01 ({bucketed}) should reconcile with parallel DV01 ({dv01})"
    );
}

/// Strike monotonicity: a cap (call on inflation) loses value as its strike
/// rises, while a floor (put on inflation) gains value. A fundamental
/// no-arbitrage property that must hold regardless of the pricing details.
#[test]
fn test_cap_value_falls_and_floor_rises_with_strike() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let market = build_market(as_of, 0.02);

    let cap_low = build_option(RateOptionType::Caplet, 0.02)
        .value(&market, as_of)
        .unwrap()
        .amount();
    let cap_high = build_option(RateOptionType::Caplet, 0.03)
        .value(&market, as_of)
        .unwrap()
        .amount();
    let floor_low = build_option(RateOptionType::Floorlet, 0.02)
        .value(&market, as_of)
        .unwrap()
        .amount();
    let floor_high = build_option(RateOptionType::Floorlet, 0.03)
        .value(&market, as_of)
        .unwrap()
        .amount();

    assert!(
        cap_low > cap_high,
        "cap value should fall as strike rises: K=2% {cap_low}, K=3% {cap_high}"
    );
    assert!(
        floor_high > floor_low,
        "floor value should rise as strike rises: K=2% {floor_low}, K=3% {floor_high}"
    );
}

/// Cap−Floor put-call parity holds within the model: `Cap(K) − Floor(K) =
/// DF·N·τ·(F − K)`, where `F` is the CPI-curve ratio forward supplied to the
/// quote model. Option quote volatility does not change that forward.
///
/// The clean, model-agnostic way to confirm parity (i.e. that the optionality
/// time value cancels and only the linear forward leg survives) is to difference
/// across strikes: the shared `F` cancels, leaving
/// `[Cap(K1) − Floor(K1)] − [Cap(K2) − Floor(K2)] = DF·N·τ·(K2 − K1)`, which is
/// independent of volatility. We assert that this strike-difference matches
/// across two very different vols.
#[test]
fn test_cap_floor_parity_strike_difference_is_vol_independent() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let (k1, k2) = (0.02, 0.03);

    let strike_diff = |vol: f64| -> f64 {
        let market = build_market(as_of, vol);
        let v = |ty: RateOptionType, k: f64| {
            build_option(ty, k).value(&market, as_of).unwrap().amount()
        };
        let parity_k1 = v(RateOptionType::Caplet, k1) - v(RateOptionType::Floorlet, k1);
        let parity_k2 = v(RateOptionType::Caplet, k2) - v(RateOptionType::Floorlet, k2);
        parity_k1 - parity_k2
    };

    let diff_low_vol = strike_diff(0.02);
    let diff_high_vol = strike_diff(0.05);

    // = DF·N·τ·(K2 − K1) > 0; the common forward and option time value cancel.
    assert!(
        diff_low_vol.abs() > 1.0,
        "strike-difference should be a material forward-leg value, got {diff_low_vol}"
    );
    assert!(
        (diff_low_vol - diff_high_vol).abs() <= 1e-6 * diff_low_vol.abs().max(1.0),
        "Cap−Floor put-call parity: the strike-difference must be vol-independent \
         2% vol {diff_low_vol}, 5% vol {diff_high_vol}"
    );
}
