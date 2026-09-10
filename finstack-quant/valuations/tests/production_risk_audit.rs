//! Public source-aware risk regressions for the production audit.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
use finstack_quant_valuations::instruments::fx::fx_option::FxOption;
use finstack_quant_valuations::instruments::fx::fx_spot::FxSpot;
use finstack_quant_valuations::instruments::fx::ndf::{Ndf, NdfQuoteConvention};
use finstack_quant_valuations::instruments::{Instrument, OptionGreeksProvider, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;
use time::macros::date;

fn market() -> MarketContext {
    let as_of = date!(2025 - 01 - 01);
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.1)
        .expect("FX quote");
    let mut market = MarketContext::new().insert_fx(FxMatrix::new(provider));
    for id in ["USD-OIS", "EUR-OIS"] {
        market = market.insert(
            DiscountCurve::builder(id)
                .base_date(as_of)
                .knots([(0.0, 1.0), (10.0, 1.0)])
                .build()
                .expect("curve"),
        );
    }
    market.insert_surface(
        VolSurface::builder("EURUSD-VOL")
            .expiries(&[0.5, 1.5])
            .strikes(&[1.0, 1.4])
            .row(&[0.2, 0.2])
            .row(&[0.2, 0.2])
            .build()
            .expect("surface"),
    )
}

#[test]
fn fx01_follows_active_spot_and_forward_overrides() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);
    let market = market();
    let mut forward = FxForward::example().expect("forward");
    forward.maturity = maturity;
    forward.notional = Money::from((100_i64, Currency::EUR));
    forward.contract_rate = Some(1.0);
    forward.spot_rate_override = Some(1.1);
    let mut spot = FxSpot::new("SPOT".into(), Currency::EUR, Currency::USD)
        .with_rate(1.1)
        .expect("spot")
        .with_settlement(maturity);
    spot.notional = Money::from((100_i64, Currency::EUR));
    let ndf = Ndf::builder()
        .id("NDF".into())
        .base_currency(Currency::EUR)
        .settlement_currency(Currency::USD)
        .fixing_date(date!(2025 - 12 - 30))
        .maturity(maturity)
        .notional(Money::from((100_i64, Currency::EUR)))
        .contract_rate(1.0)
        .quote_convention(NdfQuoteConvention::SettlementPerBase)
        .domestic_discount_curve_id("USD-OIS".into())
        .foreign_discount_curve_id_opt(Some("EUR-OIS".into()))
        .forward_rate_override_opt(Some(1.1))
        .build()
        .expect("NDF");
    for instrument in [&forward as &dyn Instrument, &spot, &ndf] {
        let result = instrument
            .price_with_metrics(&market, as_of, &[MetricId::Fx01], PricingOptions::default())
            .expect("risk");
        assert!(
            (result.measures["fx01"] - 1.1).abs() < 1e-10,
            "{}: {:?}",
            instrument.id(),
            result.measures
        );
    }
    let mut fixed = ndf;
    fixed.fixing_rate = Some(1.1);
    let result = fixed
        .price_with_metrics(&market, as_of, &[MetricId::Fx01], PricingOptions::default())
        .expect("observed fixing risk");
    assert_eq!(result.measures["fx01"], 0.0);

    let bare = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (10.0, 1.0)])
                .build()
                .expect("curve"),
        )
        .insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (10.0, 1.0)])
                .build()
                .expect("curve"),
        );
    for instrument in [&forward as &dyn Instrument, &spot, &fixed] {
        let result = instrument
            .price_with_metrics(&bare, as_of, &[MetricId::Fx01], PricingOptions::default())
            .expect("override requires no FX matrix");
        let expected = if instrument.id() == fixed.id() {
            0.0
        } else {
            1.1
        };
        assert!((result.measures["fx01"] - expected).abs() < 1e-10);
    }
}

#[test]
fn equity_scalar_volatility_risk_is_separate_from_source_nodes() {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_valuations::instruments::EquityOption;
    let as_of = date!(2025 - 01 - 01);
    let mut option = EquityOption::example().expect("option");
    option.expiry = date!(2026 - 01 - 01);
    option.strike = 120.0;
    option.spot_id = "EQ-SPOT".into();
    option.vol_surface_id = "EQ-VOL".into();
    option.div_yield_id = None;
    let bare = market().insert_price("EQ-SPOT", MarketScalar::Unitless(100.0));
    let quoted = bare.clone().insert_surface(
        VolSurface::builder("EQ-VOL")
            .expiries(&[1.0])
            .strikes(&[120.0])
            .row(&[0.2])
            .build()
            .expect("surface"),
    );
    let metrics = [
        MetricId::Vega,
        MetricId::Vanna,
        MetricId::Volga,
        MetricId::BucketedVega,
    ];
    let reference = option
        .price_with_metrics(&quoted, as_of, &metrics, PricingOptions::default())
        .expect("surface risk");
    option.instrument_pricing_overrides = option.instrument_pricing_overrides.with_implied_vol(0.2);
    for market in [&quoted, &bare] {
        let actual = option
            .price_with_metrics(market, as_of, &metrics, PricingOptions::default())
            .expect("scalar risk");
        for key in ["vega", "vanna", "volga"] {
            assert!(reference.measures[key].abs() > 1e-6);
            assert!(
                (reference.measures[key] - actual.measures[key]).abs() < 1e-8,
                "{key}"
            );
        }
        assert_eq!(actual.measures["bucketed_vega"], 0.0);
        assert_eq!(
            MetricId::BucketedVegaResidual.unit(),
            finstack_quant_valuations::metrics::MetricUnit::Currency
        );
        assert_eq!(
            actual.measures["bucketed_vega_residual"],
            actual.measures["vega"]
        );
    }
}

#[test]
fn touch_vega_follows_the_active_scalar_quote() {
    use finstack_quant_valuations::instruments::FxTouchOption;
    let as_of = date!(2025 - 01 - 01);
    let market = market();
    let mut option = FxTouchOption::example().expect("touch");
    option.monitoring_start_date = Some(as_of);
    option.expiry = date!(2026 - 01 - 01);
    let expected = option
        .price_with_metrics(&market, as_of, &[MetricId::Vega], PricingOptions::default())
        .expect("surface risk")
        .measures["vega"];
    assert!(expected.abs() > 1e-6);
    option.instrument_pricing_overrides = option.instrument_pricing_overrides.with_implied_vol(0.2);
    let actual = option
        .price_with_metrics(&market, as_of, &[MetricId::Vega], PricingOptions::default())
        .expect("scalar risk")
        .measures["vega"];
    assert!((actual - expected).abs() < 1e-8, "{actual} vs {expected}");
    option.instrument_pricing_overrides = option.instrument_pricing_overrides.with_implied_vol(0.0);
    let base = option.value(&market, as_of).expect("zero-vol PV").amount();
    let mut up = option.clone();
    up.instrument_pricing_overrides = up.instrument_pricing_overrides.with_implied_vol(0.01);
    let expected = up.value(&market, as_of).expect("one-vol-point PV").amount() - base;
    let actual = option
        .price_with_metrics(&market, as_of, &[MetricId::Vega], PricingOptions::default())
        .expect("zero-vol forward difference")
        .measures["vega"];
    assert!((actual - expected).abs() < 1e-8);
}

#[test]
fn analytical_barrier_price_and_risk_use_the_active_quote() {
    use finstack_quant_core::{market_data::scalars::MarketScalar, types::BarrierType};
    use finstack_quant_valuations::{instruments::BarrierOption, pricer::ModelKey};
    let as_of = date!(2025 - 01 - 01);
    let mut option = BarrierOption::example().expect("barrier");
    option.expiry = date!(2026 - 01 - 01);
    option.strike = 100.0;
    option.barrier = Money::from((80_i64, Currency::USD));
    option.barrier_type = BarrierType::DownAndIn;
    option.observed_barrier_breached = Some(true);
    option.spot_id = "SPOT".into();
    option.vol_surface_id = "VOL".into();
    option.discount_curve_id = "USD-OIS".into();
    option.div_yield_id = None;
    let market = market().insert_price("SPOT", MarketScalar::Unitless(100.0));
    let quoted = market.clone().insert_surface(
        VolSurface::builder("VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.2])
            .build()
            .expect("surface"),
    );
    let options = PricingOptions::default().with_model(ModelKey::BarrierBSContinuous);
    let metrics = [MetricId::Vega, MetricId::Vanna, MetricId::Volga];
    let reference = option
        .price_with_metrics(&quoted, as_of, &metrics, options.clone())
        .expect("surface risk");
    option.instrument_pricing_overrides = option.instrument_pricing_overrides.with_implied_vol(0.2);
    let actual = option
        .price_with_metrics(&market, as_of, &metrics, options)
        .expect("override requires no surface");
    assert_eq!(actual.value, reference.value);
    for metric in metrics {
        assert!((actual.measures[&metric] - reference.measures[&metric]).abs() < 1e-8);
    }
}

#[test]
fn fx_vanna_volga_shift_effective_volatility_at_off_grid_quotes() {
    let as_of = date!(2025 - 01 - 01);
    let market = market();
    let mut option = FxOption::example().expect("option");
    option.expiry = date!(2026 - 01 - 01);
    option.strike = 1.2;
    let mut up = option.clone();
    up.instrument_pricing_overrides = up.instrument_pricing_overrides.with_implied_vol(0.21);
    let mut down = option.clone();
    down.instrument_pricing_overrides = down.instrument_pricing_overrides.with_implied_vol(0.19);
    let up_risk = up
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Vega],
            PricingOptions::default(),
        )
        .expect("up risk");
    let down_risk = down
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Vega],
            PricingOptions::default(),
        )
        .expect("down risk");
    let expected_vanna = (up_risk.measures["delta"] - down_risk.measures["delta"]) / 2.0;
    let expected_volga = (up_risk.measures["vega"] - down_risk.measures["vega"]) / 2.0;
    for use_override in [false, true] {
        if use_override {
            option.instrument_pricing_overrides =
                option.instrument_pricing_overrides.with_implied_vol(0.2);
        }
        let base = option.value(&market, as_of).expect("PV").amount();
        let vanna = option
            .option_vanna(&market, as_of)
            .expect("vanna")
            .expect("some");
        let volga = option
            .option_volga(&market, as_of, base)
            .expect("volga")
            .expect("some");
        assert!(
            (vanna - expected_vanna).abs() < 1e-8,
            "override={use_override}: {vanna} vs {expected_vanna}"
        );
        assert!(
            (volga - expected_volga).abs() < 1e-8,
            "override={use_override}: {volga} vs {expected_volga}"
        );
    }
}
