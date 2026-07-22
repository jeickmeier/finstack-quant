//! Registry-level metric tests for `IrFutureOption`.
//!
//! The economically load-bearing check is DV01: the futures price is an
//! exogenous market quote (not derived from a curve), so a generic curve-bump
//! DV01 only sees premium discounting and misses the dominant delta channel
//! (price = 100 − rate). The registered DV01 must match a full finite
//! difference that bumps BOTH the futures price (−0.01 per +1bp) and the
//! discount curve (+1bp).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::{IrFutureOption, OptionType};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::prelude::Instrument;
use time::macros::date;

const FLAT_RATE: f64 = 0.04;

fn flat_market(as_of: time::Date, bump_bp: f64) -> MarketContext {
    let rate = FLAT_RATE + bump_bp * 1e-4;
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0_f64).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("curve"),
    )
}

fn sample_option(futures_price: f64) -> IrFutureOption {
    IrFutureOption::builder()
        .id(InstrumentId::new("IRFO-METRICS"))
        .futures_price(futures_price)
        .strike(95.25)
        .expiry(date!(2025 - 06 - 16))
        .option_type(OptionType::Call)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .tick_size(0.0025)
        .tick_value(6.25)
        .volatility(0.20)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("build")
}

/// The registered DV01 metric must capture the futures-price delta channel,
/// not just premium discounting: it must match the full finite difference
/// (futures price −0.01 AND curve +1bp) within 1%.
#[test]
fn registered_dv01_matches_full_finite_difference() {
    let as_of = date!(2025 - 01 - 15);
    let market = flat_market(as_of, 0.0);
    let option = sample_option(95.50);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("price with metrics");
    let dv01 = *result.measures.get("dv01").expect("dv01 present");

    let pv_base = option.npv(&market, as_of).expect("pv");
    let bumped_market = flat_market(as_of, 1.0);
    let pv_bumped = sample_option(95.50 - 0.01)
        .npv(&bumped_market, as_of)
        .expect("pv bumped");
    let fd = pv_bumped - pv_base;

    assert!(
        dv01 < 0.0,
        "call on the future loses when rates rise: dv01={dv01}"
    );
    assert!(
        (dv01 - fd).abs() < 0.01 * fd.abs(),
        "registered DV01 must include the futures delta channel: dv01={dv01}, full FD={fd}"
    );
}
