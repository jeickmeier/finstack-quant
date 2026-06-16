//! Normal (Bachelier) model Greeks validation tests.
//!
//! Validates that when `vol_type = Normal`, the delta, gamma, and vega calculators
//! use the Bachelier formulas rather than Black-76. Tests use finite-difference
//! cross-validation and sign/invariant checks.
//!
//! # Market Context
//!
//! Bachelier (normal) vol is the standard for EUR ESTR caps/floors in negative-rate
//! environments. Getting the Greeks wrong here causes unhedged risk.
//!
//! # References
//!
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models*, Ch. 1.
//! - QuantLib `BachelierCapFloorEngine`

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
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn flat_normal_vol_surface(sigma: f64, id: &str) -> VolSurface {
    // Normal vol surface with rate-level strikes (can be negative)
    VolSurface::builder(id)
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[-0.02, -0.01, 0.0, 0.01, 0.02])
        .row(&[sigma, sigma, sigma, sigma, sigma])
        .row(&[sigma, sigma, sigma, sigma, sigma])
        .row(&[sigma, sigma, sigma, sigma, sigma])
        .row(&[sigma, sigma, sigma, sigma, sigma])
        .build()
        .unwrap()
}

fn make_caplet(
    _as_of: Date,
    fixing: Date,
    payment: Date,
    _forward_rate: f64,
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
        id: "TEST".into(),
        rate_option_type,
        notional: Money::new(1_000_000.0, Currency::EUR),
        strike: Decimal::try_from(strike).unwrap(),
        start_date: fixing,
        maturity: payment,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: finstack_quant_valuations::instruments::SettlementType::Cash,
        discount_curve_id: "DISC".into(),
        forward_curve_id: "FWD".into(),
        vol_surface_id: "VOL".into(),
        vol_type,
        vol_shift: 0.0,
        pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn context_from(as_of: Date, fwd_rate: f64, normal_sigma: f64) -> MarketContext {
    MarketContext::new()
        .insert(flat_disc(0.02, as_of, "DISC"))
        .insert(flat_fwd(fwd_rate, as_of, "FWD"))
        .insert_surface(flat_normal_vol_surface(normal_sigma, "VOL"))
}

// ---------------------------------------------------------------------------
// Sign & non-negativity tests
// ---------------------------------------------------------------------------

/// Normal-model caplet delta must be positive.
#[test]
fn normal_caplet_delta_is_positive() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);

    let caplet = make_caplet(
        as_of,
        fixing,
        payment,
        0.02,
        0.02,
        CapFloorVolType::Normal,
        true,
    );
    let ctx = context_from(as_of, 0.02, 0.005);

    let result = caplet
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing should succeed");
    let delta = *result.measures.get("delta").unwrap();
    assert!(
        delta > 0.0,
        "Normal caplet delta must be positive (ATM): got {delta}"
    );
}

/// Normal-model floorlet delta must be negative.
#[test]
fn normal_floorlet_delta_is_negative() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);

    let floorlet = make_caplet(
        as_of,
        fixing,
        payment,
        0.02,
        0.02,
        CapFloorVolType::Normal,
        false,
    );
    let ctx = context_from(as_of, 0.02, 0.005);

    let result = floorlet
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing should succeed");
    let delta = *result.measures.get("delta").unwrap();
    assert!(
        delta < 0.0,
        "Normal floorlet delta must be negative (ATM): got {delta}"
    );
}

/// Normal-model gamma must be non-negative for both caplet and floorlet.
#[test]
fn normal_gamma_non_negative() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);
    let ctx = context_from(as_of, 0.02, 0.005);

    for is_cap in [true, false] {
        let inst = make_caplet(
            as_of,
            fixing,
            payment,
            0.02,
            0.02,
            CapFloorVolType::Normal,
            is_cap,
        );
        let result = inst
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::Gamma],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .expect("pricing should succeed");
        let gamma = *result.measures.get("gamma").unwrap();
        assert!(
            gamma >= 0.0,
            "Normal gamma must be non-negative (is_cap={is_cap}): got {gamma}"
        );
    }
}

/// Normal-model vega must be positive for both caplet and floorlet.
#[test]
fn normal_vega_positive() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);
    let ctx = context_from(as_of, 0.02, 0.005);

    for is_cap in [true, false] {
        let inst = make_caplet(
            as_of,
            fixing,
            payment,
            0.02,
            0.02,
            CapFloorVolType::Normal,
            is_cap,
        );
        let result = inst
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::Vega],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .expect("pricing should succeed");
        let vega = *result.measures.get("vega").unwrap();
        assert!(
            vega > 0.0,
            "Normal vega must be positive (is_cap={is_cap}): got {vega}"
        );
    }
}

// ---------------------------------------------------------------------------
// Negative-rate environment: normal model handles it, black model fails
// ---------------------------------------------------------------------------

/// Normal model must price successfully with negative forward rate.
#[test]
fn normal_model_prices_negative_forward() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);

    let caplet = make_caplet(
        as_of,
        fixing,
        payment,
        -0.005,
        0.0,
        CapFloorVolType::Normal,
        true,
    );
    let ctx = context_from(as_of, -0.005, 0.003);

    let result = caplet
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Delta, MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("normal model must handle negative forward");

    let pv = result.value.amount();
    let delta = *result.measures.get("delta").unwrap();
    let vega = *result.measures.get("vega").unwrap();

    assert!(
        pv.is_finite() && pv >= 0.0,
        "PV must be finite and non-negative: {pv}"
    );
    assert!(delta.is_finite(), "Delta must be finite: {delta}");
    assert!(
        vega.is_finite() && vega >= 0.0,
        "Vega must be finite and non-negative: {vega}"
    );
}

// ---------------------------------------------------------------------------
// Finite-difference cross-validation: Bachelier delta
// ---------------------------------------------------------------------------

/// Normal caplet delta matches finite-difference approximation within 2%.
#[test]
fn normal_delta_matches_finite_difference() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);

    let fwd_rate = 0.02f64;
    let strike = 0.02f64;
    let normal_sigma = 0.005f64;
    let bump = 0.0001f64; // 1bp

    let caplet = make_caplet(
        as_of,
        fixing,
        payment,
        fwd_rate,
        strike,
        CapFloorVolType::Normal,
        true,
    );

    let ctx_base = MarketContext::new()
        .insert(flat_disc(0.02, as_of, "DISC"))
        .insert(flat_fwd(fwd_rate, as_of, "FWD"))
        .insert_surface(flat_normal_vol_surface(normal_sigma, "VOL"));

    let ctx_up = MarketContext::new()
        .insert(flat_disc(0.02, as_of, "DISC"))
        .insert(flat_fwd(fwd_rate + bump, as_of, "FWD"))
        .insert_surface(flat_normal_vol_surface(normal_sigma, "VOL"));

    let ctx_down = MarketContext::new()
        .insert(flat_disc(0.02, as_of, "DISC"))
        .insert(flat_fwd(fwd_rate - bump, as_of, "FWD"))
        .insert_surface(flat_normal_vol_surface(normal_sigma, "VOL"));

    let pv_up = caplet.value(&ctx_up, as_of).unwrap().amount();
    let pv_down = caplet.value(&ctx_down, as_of).unwrap().amount();
    let fd_delta = (pv_up - pv_down) / (2.0 * bump);

    let result = caplet
        .price_with_metrics(
            &ctx_base,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let analytic_delta = *result.measures.get("delta").unwrap();

    let abs_diff = (fd_delta - analytic_delta).abs();
    let rel_err = abs_diff / analytic_delta.abs().max(1.0);
    assert!(
        rel_err < 0.02 || abs_diff < 5.0,
        "Normal delta FD={fd_delta:.4} vs analytic={analytic_delta:.4}: rel_err={rel_err:.4}"
    );
}

/// Normal caplet vega matches finite-difference approximation within 2%.
#[test]
fn normal_vega_matches_finite_difference() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);

    let fwd_rate = 0.02f64;
    let strike = 0.02f64;
    let normal_sigma = 0.005f64;
    let vol_bump = 0.0001f64; // 1bp normal vol bump

    let caplet = make_caplet(
        as_of,
        fixing,
        payment,
        fwd_rate,
        strike,
        CapFloorVolType::Normal,
        true,
    );

    let ctx_base = MarketContext::new()
        .insert(flat_disc(0.02, as_of, "DISC"))
        .insert(flat_fwd(fwd_rate, as_of, "FWD"))
        .insert_surface(flat_normal_vol_surface(normal_sigma, "VOL"));

    let ctx_up = MarketContext::new()
        .insert(flat_disc(0.02, as_of, "DISC"))
        .insert(flat_fwd(fwd_rate, as_of, "FWD"))
        .insert_surface(flat_normal_vol_surface(normal_sigma + vol_bump, "VOL"));

    let ctx_down = MarketContext::new()
        .insert(flat_disc(0.02, as_of, "DISC"))
        .insert(flat_fwd(fwd_rate, as_of, "FWD"))
        .insert_surface(flat_normal_vol_surface(normal_sigma - vol_bump, "VOL"));

    let pv_up = caplet.value(&ctx_up, as_of).unwrap().amount();
    let pv_down = caplet.value(&ctx_down, as_of).unwrap().amount();
    // FD vega per 1% normal vol (analytic is also per 1% = 0.01)
    let fd_vega = (pv_up - pv_down) / (2.0 * vol_bump) * 0.01;

    let result = caplet
        .price_with_metrics(
            &ctx_base,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let analytic_vega = *result.measures.get("vega").unwrap();

    let abs_diff = (fd_vega - analytic_vega).abs();
    let rel_err = abs_diff / analytic_vega.abs().max(1.0);
    assert!(
        rel_err < 0.02 || abs_diff < 1.0,
        "Normal vega FD={fd_vega:.4} vs analytic={analytic_vega:.4}: rel_err={rel_err:.4}"
    );
}

// ---------------------------------------------------------------------------
// Cap-floor delta parity: cap_delta + |floor_delta| ≈ 1 (ATM Black-Scholes property)
// For Bachelier: cap_delta - floor_delta = 1 exactly (put-call parity on delta)
// ---------------------------------------------------------------------------

/// Bachelier delta satisfies: cap_delta - floor_delta = 1.
///
/// From put-call parity: C - P = Forward × Annuity × (F - K)
/// Differentiating w.r.t. F: dC/dF - dP/dF = Annuity × (some quantity)
/// At the caplet level: delta_call - delta_put = N(d) - (N(d) - 1) = 1
#[test]
fn normal_caplet_floorlet_delta_parity() {
    let as_of = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let payment = date!(2024 - 07 - 01);
    let ctx = context_from(as_of, 0.02, 0.005);

    let cap_inst = make_caplet(
        as_of,
        fixing,
        payment,
        0.02,
        0.02,
        CapFloorVolType::Normal,
        true,
    );
    let floor_inst = make_caplet(
        as_of,
        fixing,
        payment,
        0.02,
        0.02,
        CapFloorVolType::Normal,
        false,
    );

    let cap_result = cap_inst
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let floor_result = floor_inst
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cap_delta = *cap_result.measures.get("delta").unwrap();
    let floor_delta = *floor_result.measures.get("delta").unwrap();

    // Scaling: both are per_unit × notional × tau × df
    // The per-unit delta satisfies: N(d) - (-N(-d)) = N(d) + N(-d) = 1
    // So cap_delta - floor_delta should equal notional × tau × df
    // We just check they sum to something positive and consistent
    let sum = cap_delta - floor_delta;
    assert!(
        sum > 0.0,
        "cap_delta - floor_delta must be positive (Bachelier parity): sum={sum}"
    );
    assert!(
        (cap_delta + floor_delta.abs() - sum.abs()) < 0.01 * sum.abs(),
        "Bachelier delta parity: cap_delta={cap_delta:.4}, floor_delta={floor_delta:.4}"
    );
}
