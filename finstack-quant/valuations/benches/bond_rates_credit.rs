//! Callable bond on the joint rates-credit model.
//!
//! With a rate or hazard volatility the model prices embedded options by
//! least-squares Monte Carlo: 20,000 antithetic estimators (40,000 factor paths)
//! per stage on a daily grid, about 66 million path steps in each of training and
//! pricing for this 4.5-year bond. These benches use the default budget.
//!
//! - `base_pv` is one unquoted PV: one calibration, one policy fit, one pricing
//!   pass.
//! - `oas` solves the OAS to a quoted price: a policy fit and a pricing pass for
//!   every root-search trial.
//!
//! Measure the single-threaded cost, which is what a WASM host pays, with
//! `RAYON_NUM_THREADS=1`.

mod support;

use criterion::{Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, StubKind};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, Rate};
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use time::macros::date;

const AS_OF: Date = date!(2024 - 07 - 15);

fn call(on: Date, price_pct_of_par: f64) -> CallPut {
    CallPut {
        start: on,
        end: on,
        price_pct_of_par,
        make_whole: None,
    }
}

fn callable_credit_bond() -> Bond {
    let mut bond = Bond::fixed(
        "CALLABLE-CREDIT",
        Money::new(1_000_000.0, Currency::USD).expect("notional"),
        Rate::from_decimal(0.0425).expect("coupon"),
        date!(2024 - 01 - 15),
        date!(2029 - 01 - 15),
        StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.credit_curve_id = Some(CurveId::new("ACME-HAZARD"));
    bond.call_put = Some(CallPutSchedule {
        calls: vec![
            call(date!(2026 - 01 - 15), 102.0),
            call(date!(2028 - 01 - 15), 101.0),
        ],
        puts: Vec::new(),
    });
    let model = &mut bond.instrument_pricing_overrides.model_config;
    model.hw1f_sigma = Some(0.01);
    model.hw1f_mean_reversion = Some(0.03);
    bond
}

fn market() -> MarketContext {
    let base = date!(2023 - 12 - 28);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.97), (40.0, 0.3)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("discount curve");
    let hazard = HazardCurve::flat("ACME-HAZARD", base, 0.02, 0.4).expect("hazard curve");
    MarketContext::new().insert(discount).insert(hazard)
}

fn options() -> PricingOptions {
    PricingOptions::default().with_model(ModelKey::RatesCredit)
}

fn bench_rates_credit(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_rates_credit");
    group.bench_function("base_pv", |b| {
        let market = market();
        let unquoted = callable_credit_bond();
        assert!(unquoted.credit_curve_id.is_some());
        assert!(
            unquoted
                .call_put
                .as_ref()
                .is_some_and(CallPutSchedule::has_options)
        );
        assert_eq!(
            unquoted
                .instrument_pricing_overrides
                .model_config
                .hw1f_sigma,
            Some(0.01)
        );
        assert!(
            unquoted
                .instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price_pct
                .is_none()
        );
        support::bench(
            b,
            || {
                unquoted
                    .price_with_metrics(&market, AS_OF, &[], options())
                    .expect("rates-credit base PV")
            },
            |result| {
                assert_eq!(result.value.currency(), Currency::USD);
                assert!(result.value.amount().is_finite());
            },
        );
    });
    group.bench_function("oas", |b| {
        let market = market();
        let mut quoted = callable_credit_bond();
        quoted
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct = Some(101.0);
        support::bench(
            b,
            || {
                quoted
                    .price_with_metrics(&market, AS_OF, &[MetricId::Oas], options())
                    .expect("rates-credit OAS")
            },
            |result| {
                assert_eq!(result.value.currency(), Currency::USD);
                assert!(result.value.amount().is_finite());
                assert!(result.measures["oas"].is_finite());
            },
        );
    });
    group.finish();
}

criterion_group!(benches, bench_rates_credit);
criterion_main!(benches);
