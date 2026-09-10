//! Tests for `CapFloorVolType::Auto` model selection.
//!
//! `Auto` treats the supplied surface as a **lognormal** surface: it prices with
//! Black-76 where the model is well-defined (`forward > 0` and `strike > 0`) and
//! otherwise converts the lognormal vol to an equivalent normal vol and uses
//! Bachelier. This keeps a single, consistent interpretation of the surface
//! across every caplet, so `Auto` behaves identically to `Lognormal` (which uses
//! the same fallback) rather than feeding the raw surface number to Bachelier.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::cap_floor::{
    CapFloor, CapFloorVolType, RateOptionType,
};
use finstack_quant_valuations::instruments::ExerciseStyle;
use finstack_quant_valuations::instruments::Instrument;
use rust_decimal::Decimal;
use time::macros::date;

fn flat_fwd(rate: f64, base: Date, id: &str) -> ForwardCurve {
    ForwardCurve::builder(id, 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (5.0, rate)])
        .build()
        .unwrap()
}

fn flat_disc(rate: f64, base: Date, id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, (-rate).exp()), (5.0, (-rate * 5.0).exp())])
        .build()
        .unwrap()
}

fn flat_vol_surface(sigma: f64, id: &str) -> VolSurface {
    VolSurface::builder(id)
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[-0.02, -0.01, 0.0, 0.01, 0.02, 0.03, 0.05])
        .row(&[sigma; 7])
        .row(&[sigma; 7])
        .row(&[sigma; 7])
        .row(&[sigma; 7])
        .build()
        .unwrap()
}

fn make_caplet(
    fixing: Date,
    payment: Date,
    strike: f64,
    vol_type: CapFloorVolType,
    is_cap: bool,
) -> CapFloor {
    let rate_option_type = if is_cap {
        RateOptionType::Caplet
    } else {
        RateOptionType::Floorlet
    };
    CapFloor {
        id: "TEST-AUTO".into(),
        rate_option_type,
        notional: Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        strike: Decimal::try_from(strike).unwrap(),
        start_date: fixing,
        maturity: payment,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: finstack_quant_valuations::instruments::SettlementType::Cash,
        discount_curve_id: "DISC".into(),
        forward_curve_id: "FWD".into(),
        vol_surface_id: "VOL".into(),
        vol_type,
        vol_shift: 0.0,
        overnight_coupon: None,
        premium: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn context_from(as_of: Date, fwd_rate: f64, sigma: f64) -> MarketContext {
    MarketContext::new()
        .insert(flat_disc(fwd_rate.max(0.001), as_of, "DISC"))
        .insert(flat_fwd(fwd_rate, as_of, "FWD"))
        .insert_surface(flat_vol_surface(sigma, "VOL"))
}

// Auto selects Black for positive rates

#[test]
fn auto_selects_black_for_positive_rates() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);
    let fwd_rate = 0.03; // positive
    let strike = 0.03; // positive
    let sigma = 0.20; // lognormal vol

    let auto_cap = make_caplet(fixing, payment, strike, CapFloorVolType::Auto, true);
    let black_cap = make_caplet(fixing, payment, strike, CapFloorVolType::Lognormal, true);

    let ctx = context_from(as_of, fwd_rate, sigma);

    let auto_pv = auto_cap.value(&ctx, as_of).expect("Auto should succeed");
    let black_pv = black_cap.value(&ctx, as_of).expect("Black should succeed");

    // Auto should produce the same result as Black for positive rates
    let diff = (auto_pv.amount() - black_pv.amount()).abs();
    assert!(
        diff < 1e-10,
        "Auto should match Black for positive rates: auto={}, black={}, diff={}",
        auto_pv.amount(),
        black_pv.amount(),
        diff
    );
}

#[test]
fn auto_follows_normal_metadata_for_negative_forward() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);
    let ctx = context_from(as_of, -0.005, 0.005).insert_surface(
        flat_vol_surface(0.005, "VOL")
            .with_quote_type(finstack_quant_core::market_data::surfaces::VolQuoteType::Normal),
    );
    let auto = make_caplet(fixing, payment, -0.002, CapFloorVolType::Auto, true);
    let normal = make_caplet(fixing, payment, -0.002, CapFloorVolType::Normal, true);
    let auto_pv = auto.value(&ctx, as_of).expect("auto normal quote").amount();
    let normal_pv = normal.value(&ctx, as_of).expect("normal quote").amount();
    assert!(auto_pv > 0.0);
    assert!((auto_pv - normal_pv).abs() < 1e-10);
}

#[test]
fn black_metadata_requires_positive_rates() {
    let as_of = date!(2024 - 01 - 01);
    let ctx = context_from(as_of, -0.005, 0.20);
    for model in [CapFloorVolType::Auto, CapFloorVolType::Lognormal] {
        let cap = make_caplet(
            date!(2024 - 04 - 01),
            date!(2024 - 07 - 01),
            0.0,
            model,
            true,
        );
        let error = cap
            .value(&ctx, as_of)
            .expect_err("invalid Black coordinates");
        assert!(error.to_string().contains("positive forward and strike"));
    }
}

// Serde round-trip

#[test]
fn auto_vol_type_serde_round_trip() {
    let json = serde_json::to_string(&CapFloorVolType::Auto).unwrap();
    assert_eq!(json, "\"auto\"");

    let deserialized: CapFloorVolType = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, CapFloorVolType::Auto);
}

#[test]
fn auto_vol_type_from_str() {
    let parsed: CapFloorVolType = "auto".parse().unwrap();
    assert_eq!(parsed, CapFloorVolType::Auto);
}

#[test]
fn auto_vol_type_display() {
    assert_eq!(CapFloorVolType::Auto.to_string(), "auto");
}
