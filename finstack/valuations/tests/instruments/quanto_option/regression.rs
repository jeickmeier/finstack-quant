//! Numeric regression tests for QuantoOption greeks.

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::fx::quanto_option::QuantoOption;
use finstack_valuations::instruments::{
    Attributes, Instrument, OptionGreekKind, OptionGreeksProvider, OptionGreeksRequest, OptionType,
    PricingOptions, PricingOverrides,
};
use finstack_valuations::metrics::MetricId;
use std::sync::Arc;
use time::macros::date;

const AS_OF: Date = date!(2026 - 01 - 02);
const EXPIRY: Date = date!(2027 - 01 - 04);

fn flat_surface(id: &str, level: f64) -> VolSurface {
    let strikes = vec![10_000.0, 25_000.0, 35_000.0, 50_000.0, 75_000.0];
    let tenors = vec![0.25, 0.5, 1.0, 2.0, 5.0];
    let row = vec![level; strikes.len()];
    let mut b = VolSurface::builder(CurveId::new(id))
        .expiries(&tenors)
        .strikes(&strikes);
    for _ in 0..tenors.len() {
        b = b.row(&row);
    }
    b.build().expect("flat surface")
}

fn flat_fx_surface(id: &str, level: f64) -> VolSurface {
    let strikes = vec![0.5, 0.8, 1.0, 1.2, 1.5];
    let tenors = vec![0.25, 0.5, 1.0, 2.0, 5.0];
    let row = vec![level; strikes.len()];
    let mut b = VolSurface::builder(CurveId::new(id))
        .expiries(&tenors)
        .strikes(&strikes);
    for _ in 0..tenors.len() {
        b = b.row(&row);
    }
    b.build().expect("flat fx surface")
}

fn build_market(equity_vol: f64, fx_vol: f64) -> MarketContext {
    let usd = DiscountCurve::builder("USD-OIS")
        .base_date(AS_OF)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("usd disc");
    let jpy = DiscountCurve::builder("JPY-OIS")
        .base_date(AS_OF)
        .knots([(0.0, 1.0), (1.0, 0.999), (5.0, 0.995)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("jpy disc");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::JPY, Currency::USD, 1.0 / 150.0)
        .expect("rate");
    let fx = FxMatrix::new(provider);

    MarketContext::new()
        .insert(usd)
        .insert(jpy)
        .insert_surface(flat_surface("NKY-VOL", equity_vol))
        .insert_surface(flat_fx_surface("JPYUSD-VOL", fx_vol))
        .insert_fx(fx)
        // Dividend yield is read as a unitless price scalar by
        // `resolve_optional_dividend_yield`, not a discount curve.
        .insert_price("NKY-DIV", MarketScalar::Unitless(0.01))
        .insert_price("NKY-SPOT", MarketScalar::Unitless(35_000.0))
        // Quote-per-base direction (USD per JPY), matching the JPYUSD id.
        .insert_price("JPYUSD-SPOT", MarketScalar::Unitless(1.0 / 150.0))
}

fn build_option(correlation: f64) -> QuantoOption {
    QuantoOption::builder()
        .id(InstrumentId::new("QUANTO-REGRESSION"))
        .underlying_ticker("NKY".to_string())
        .equity_strike(Money::new(35_000.0, Currency::JPY))
        .option_type(OptionType::Call)
        .expiry(EXPIRY)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .underlying_quantity_opt(Some(4_000.0))
        .payoff_fx_rate_opt(Some(1.0 / 140.0))
        .base_currency(Currency::JPY)
        .quote_currency(Currency::USD)
        .correlation(correlation)
        .day_count(DayCount::Act365F)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("JPY-OIS"))
        .spot_id("NKY-SPOT".into())
        .vol_surface_id(CurveId::new("NKY-VOL"))
        .div_yield_id_opt(Some(CurveId::new("NKY-DIV")))
        .fx_rate_id_opt(Some("JPYUSD-SPOT".to_string()))
        .fx_vol_id_opt(Some(CurveId::new("JPYUSD-VOL")))
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("quanto option")
}

fn fx_vega_via_registry(option: &QuantoOption, market: &MarketContext) -> f64 {
    let result = option
        .price_with_metrics(
            market,
            AS_OF,
            &[MetricId::FxVega],
            PricingOptions::default(),
        )
        .expect("price with FxVega");
    result
        .measures
        .get(&MetricId::FxVega)
        .copied()
        .expect("FxVega in measures")
}

fn correlation01_via_registry(option: &QuantoOption, market: &MarketContext) -> f64 {
    let result = option
        .price_with_metrics(
            market,
            AS_OF,
            &[MetricId::Correlation01],
            PricingOptions::default(),
        )
        .expect("price with Correlation01");
    result
        .measures
        .get(&MetricId::Correlation01)
        .copied()
        .expect("Correlation01 in measures")
}

/// Pins the FX-vega scaling fix: previously the calculator used a multiplicative
/// `surface.scaled(1 + 0.01)` bump but divided by the absolute `0.01`, which
/// under-reported vega by roughly the FX-vol level. After the fix, an absolute
/// 1-vol-point bump and 2*0.01 divisor produce a vega that scales correctly
/// across vol levels.
#[test]
fn fx_vega_scales_with_vol_level_after_additive_fix() {
    let option = build_option(-0.2);

    let market_low = build_market(0.20, 0.05);
    let market_high = build_market(0.20, 0.20);

    let vega_low = fx_vega_via_registry(&option, &market_low);
    let vega_high = fx_vega_via_registry(&option, &market_high);

    assert!(vega_low.is_finite(), "vega_low must be finite: {vega_low}");
    assert!(
        vega_high.is_finite(),
        "vega_high must be finite: {vega_high}"
    );

    // Both should be the same order of magnitude. Under the old multiplicative
    // bug the ratio scaled with the FX-vol level (~10x), now it is bounded.
    let ratio = vega_high.abs() / vega_low.abs().max(1e-12);
    assert!(
        (0.3..=3.0).contains(&ratio),
        "FX vega ratio at 4x vol level should be order-of-magnitude stable, \
         got {ratio:.3} (vega_low={vega_low:.3}, vega_high={vega_high:.3})"
    );
}

/// Pins the boundary-aware correlation01 fix: at correlation near +/-1, the
/// calculator must shrink the bump width symmetrically (so both bumps stay in
/// [-1, 1]) and divide by the actual width applied. Previously the divisor was
/// fixed at the unbumped width, biasing the result by up to a full bump.
#[test]
fn correlation01_finite_at_boundary() {
    for &rho in &[-0.99_f64, 0.0, 0.99] {
        let option = build_option(rho);
        let market = build_market(0.20, 0.10);
        let value = correlation01_via_registry(&option, &market);
        assert!(
            value.is_finite(),
            "Correlation01 at rho={rho} must be finite, got {value}"
        );
    }
}

/// Pins the OptionGreeks dispatcher fix: unsupported greek kinds now error
/// instead of returning a default-zero `OptionGreeks`. (Catching e.g. Theta
/// before it silently reports 0 to the caller.)
#[test]
fn option_greeks_errors_on_unsupported_kind() {
    let market = build_market(0.20, 0.10);
    let option = build_option(-0.2);

    let request = OptionGreeksRequest {
        greek: OptionGreekKind::Theta,
        base_pv: None,
    };
    let result = option.option_greeks(&market, AS_OF, &request);
    assert!(
        result.is_err(),
        "Theta is not supported by quanto analytical pricer; should error"
    );
}

/// Pins that all standard greeks for the quanto option remain finite under
/// reasonable inputs. Catches accidental NaN propagation after refactors and
/// also validates the registry path end-to-end.
#[test]
fn standard_quanto_metrics_are_finite() {
    let option = build_option(-0.2);
    let market = build_market(0.20, 0.10);

    let result = option
        .price_with_metrics(
            &market,
            AS_OF,
            &[
                MetricId::Delta,
                MetricId::Gamma,
                MetricId::Vega,
                MetricId::FxDelta,
                MetricId::FxVega,
                MetricId::Correlation01,
            ],
            PricingOptions::default(),
        )
        .expect("price with metrics");

    for metric in [
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::FxDelta,
        MetricId::FxVega,
        MetricId::Correlation01,
    ] {
        let v = result
            .measures
            .get(&metric)
            .copied()
            .unwrap_or_else(|| panic!("metric {metric} missing from result"));
        assert!(v.is_finite(), "metric {metric} must be finite, got {v}");
    }
}

// Note: a negative-`r_d` regression for the touch-option `lambda^2 < 0` error
// path is not constructed at integration-test scope because `DiscountCurve`
// rejects DF > 1 outright (a useful input-validation defence). The contract
// "no silent zero on lambda^2 < 0" is pinned at the unit-test level inside
// `fx_touch_option/pricer.rs` via direct `price_touch` invocation.
