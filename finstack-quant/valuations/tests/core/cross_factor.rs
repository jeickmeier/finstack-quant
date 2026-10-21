//! Cross-factor stencils over a built-in option.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_valuations::metrics::sensitivities::cross_factor::make_vol_bumper;

fn flat_surface(id: &str, vol: f64) -> VolSurface {
    VolSurface::builder(id)
        .expiries(&[1.0])
        .strikes(&[100.0])
        .row(&[vol])
        .build()
        .expect("surface")
}

#[test]
fn near_zero_and_zero_volatility_preserve_bilinear_cross_derivative() {
    use finstack_quant_core::{currency::Currency, money::Money};
    use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
    use finstack_quant_valuations::metrics::MetricContext;
    use std::sync::Arc;
    for vol in [0.0, 0.005, 0.02] {
        let option = EquityOption::example().unwrap();
        let market =
            MarketContext::new().insert_surface(flat_surface(option.vol_surface_id.as_str(), vol));
        let context = MetricContext::new(
            Arc::new(option),
            Arc::new(market.clone()),
            Date::MIN,
            Money::new(0.0, Currency::USD).unwrap(),
            MetricContext::default_config(),
        );
        let bumper = make_vol_bumper(&context).unwrap().unwrap();
        let mut result = 0.0;
        for (ds, ws) in [(-1.0, -0.5), (1.0, 0.5)] {
            for &(dv, wv) in bumper.stencil() {
                let shifted = bumper.bump_market(&market, Date::MIN, dv).unwrap();
                let sigma = shifted.get_surface("EQUITY-VOL").unwrap().vols()[0];
                // V(S,sigma) = S*sigma, expressed per spot and vol point.
                result += ws * wv * (100.0 + ds) * sigma;
            }
        }
        result /= bumper.bump_size();
        assert!((result - 0.01).abs() < 1e-12, "vol={vol}: {result}");
    }
}
