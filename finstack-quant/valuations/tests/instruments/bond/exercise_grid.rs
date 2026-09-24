//! Exercise-date grid gate (W3.1).
//!
//! Callable, puttable and return-floor bonds used to be exercisable on every
//! calendar day of their windows. The exercise set is now the window ends,
//! schedule dates and month-ends inside the window (plus protection and
//! contractual-call breakpoints for return floors). These fixtures pin that
//! the candidate set prices within 0.01 per 100 of the daily set.
//!
//! The references are the daily-enumeration prices measured on commit
//! a4e5cc95e (the last commit with daily exercise). Without month-end anchors
//! (window ends and coupon dates only) the same fixtures moved by
//! +0.031, +0.034, -0.034, +0.078, +0.051 and +0.094 per 100.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::{
    Bond, CallPut, CallPutSchedule, ProtectionWindow, ReturnFloorSpec,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentPricingOverrides};
use time::macros::date;

fn market() -> MarketContext {
    let base = date!(2025 - 01 - 01);
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .expect("curve");
    let hazard = HazardCurve::builder("USD-HAZARD")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(0.0, 0.02), (10.0, 0.02)])
        .build()
        .expect("hazard");
    MarketContext::new().insert(curve).insert(hazard)
}

fn bond(id: &str, coupon: f64, maturity: Date) -> Bond {
    let mut bond = Bond::fixed(
        id,
        Money::new(100.0, Currency::USD).expect("money"),
        finstack_quant_core::types::Rate::from_decimal(coupon).expect("rate"),
        date!(2025 - 01 - 01),
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_implied_vol(0.01);
    bond
}

fn right(start: Date, end: Date, price: f64) -> CallPut {
    CallPut {
        start_date: start,
        end_date: end,
        price_pct_of_par: price,
        make_whole: None,
    }
}

fn fixtures() -> Vec<(&'static str, Bond)> {
    let mut par_call = bond("PAR-CALL-WINDOW", 0.05, date!(2035 - 01 - 01));
    par_call.call_put = Some(CallPutSchedule {
        calls: vec![right(date!(2030 - 01 - 01), date!(2035 - 01 - 01), 100.0)],
        puts: Vec::new(),
    });

    let mut step_down = bond("STEP-DOWN-CALLS", 0.06, date!(2035 - 01 - 01));
    step_down.call_put = Some(CallPutSchedule {
        calls: vec![
            right(date!(2027 - 01 - 01), date!(2028 - 01 - 01), 102.0),
            right(date!(2028 - 01 - 01), date!(2029 - 01 - 01), 101.0),
            right(date!(2029 - 01 - 01), date!(2035 - 01 - 01), 100.0),
        ],
        puts: Vec::new(),
    });

    let mut put = bond("PUT-WINDOW", 0.03, date!(2035 - 01 - 01));
    put.call_put = Some(CallPutSchedule {
        calls: Vec::new(),
        puts: vec![right(date!(2028 - 01 - 01), date!(2030 - 01 - 01), 100.0)],
    });

    let moic = bond("MOIC-FLOOR", 0.03, date!(2030 - 01 - 01)).with_return_floor(
        ReturnFloorSpec::moic(1.10).window(ProtectionWindow::Between {
            start: date!(2026 - 01 - 01),
            end: date!(2029 - 01 - 01),
        }),
    );

    let xirr = bond("XIRR-FLOOR", 0.04, date!(2032 - 01 - 01)).with_return_floor(
        ReturnFloorSpec::xirr(finstack_quant_core::types::Rate::from_decimal(0.06).expect("rate"))
            .window(ProtectionWindow::From(date!(2027 - 01 - 01))),
    );

    let mut rates_credit = bond("RATES-CREDIT-CALL", 0.05, date!(2030 - 01 - 01));
    rates_credit.credit_curve_id = Some(CurveId::new("USD-HAZARD"));
    rates_credit.call_put = Some(CallPutSchedule {
        calls: vec![right(date!(2027 - 01 - 01), date!(2029 - 01 - 01), 100.0)],
        puts: Vec::new(),
    });
    let model = &mut rates_credit.instrument_pricing_overrides.model_config;
    model.hw1f_sigma = Some(0.01);
    model.hazard_volatility = Some(0.02);
    model.rate_credit_correlation = Some(0.25);
    model.mc_paths = Some(64);
    model.mc_antithetic = Some(true);

    vec![
        ("par_call", par_call),
        ("step_down", step_down),
        ("put", put),
        ("moic_floor", moic),
        ("xirr_floor", xirr),
        ("rates_credit", rates_credit),
    ]
}

#[test]
fn exercise_candidate_grid_prices_within_one_cent_of_daily_exercise() {
    let market = market();
    // Mid-coupon valuation date so window starts and accrued are non-trivial.
    let as_of = date!(2025 - 03 - 17);
    let daily = [
        ("par_call", 109.6201800332),
        ("step_down", 109.6190775850),
        ("put", 105.0235559707),
        ("moic_floor", 101.7835213357),
        ("xirr_floor", 105.2871011458),
        ("rates_credit", 102.9137436949),
    ];
    for ((name, bond), (reference_name, reference)) in fixtures().into_iter().zip(daily) {
        assert_eq!(name, reference_name);
        let pv = bond.value(&market, as_of).expect("price").amount();
        assert!(
            (pv - reference).abs() < 0.01,
            "{name}: candidate-grid price {pv} vs daily-exercise {reference}"
        );
    }
}
