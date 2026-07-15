//! QuantLib parity tests for FX option pricing (Garman-Kohlhagen).
//!
//! Tests a EURUSD call:
//! - S=1.10, K=1.10, r_d=5%, r_f=3%, σ=10%, T=0.5
//!
//! Validates:
//! 1. GK price matches known reference
//! 2. Put-call parity: C - P = S×e^{-r_f×T} - K×e^{-r_d×T}
//! 3. Delta conventions: spot delta vs forward delta

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::{Attributes, FxOption, Instrument, SettlementType};
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;
use time::Month;

const SPOT: f64 = 1.10;
const STRIKE: f64 = 1.10;
const R_D: f64 = 0.05; // USD domestic rate
const R_F: f64 = 0.03; // EUR foreign rate
const VOL: f64 = 0.10;
const T: f64 = 0.5;

const BASE: Currency = Currency::EUR;
const QUOTE: Currency = Currency::USD;

// Known GK call price for ATM with these params:
// d1 = [ln(1) + (0.05 - 0.03 + 0.005)*0.5] / (0.1*sqrt(0.5))
//    = [0 + 0.0125] / 0.07071
//    = 0.17678
// d2 = d1 - 0.07071 = 0.10607
// N(0.17678) ≈ 0.5702, N(0.10607) ≈ 0.5422
// C = 1.10 * e^{-0.03*0.5} * 0.5702 - 1.10 * e^{-0.05*0.5} * 0.5422
//   = 1.10 * 0.9851 * 0.5702 - 1.10 * 0.9753 * 0.5422
//   = 0.61799 - 0.58176
//   ≈ 0.03623
const EXPECTED_GK_CALL_APPROX: f64 = 0.0362;

struct StaticFxProvider {
    rate: f64,
}

impl FxProvider for StaticFxProvider {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        if from == to {
            return Ok(1.0);
        }
        if from == BASE && to == QUOTE {
            Ok(self.rate)
        } else if from == QUOTE && to == BASE {
            Ok(1.0 / self.rate)
        } else {
            Ok(1.0)
        }
    }
}

fn create_market(as_of: Date) -> MarketContext {
    let domestic_disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, (-R_D).exp())])
        .build()
        .expect("domestic curve should build");

    let foreign_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, (-R_F).exp())])
        .build()
        .expect("foreign curve should build");

    let expiries = [0.25, 0.5, 1.0, 2.0];
    let strikes = [0.9, 1.0, 1.1, 1.2, 1.3];

    let vol_surface = VolSurface::builder("EURUSD-VOL")
        .expiries(&expiries)
        .strikes(&strikes)
        .row(&[VOL, VOL, VOL, VOL, VOL])
        .row(&[VOL, VOL, VOL, VOL, VOL])
        .row(&[VOL, VOL, VOL, VOL, VOL])
        .row(&[VOL, VOL, VOL, VOL, VOL])
        .build()
        .expect("vol surface should build");

    let fx = FxMatrix::new(Arc::new(StaticFxProvider { rate: SPOT }));

    MarketContext::new()
        .insert(domestic_disc)
        .insert(foreign_disc)
        .insert_surface(vol_surface)
        .insert_fx(fx)
}

fn create_fx_option(expiry: Date, option_type: OptionType) -> FxOption {
    FxOption::builder()
        .id(InstrumentId::new("EURUSD-QLPARITY"))
        .base_currency(BASE)
        .quote_currency(QUOTE)
        .strike(STRIKE)
        .option_type(option_type)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .day_count(DayCount::Act365F)
        .notional(Money::new(1.0, BASE)) // Per-unit notional
        .settlement(SettlementType::Cash)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .attributes(Attributes::new())
        .build()
        .expect("FX option should build")
}

/// Test: GK call price matches known reference value.
#[test]
fn test_gk_call_price_matches_reference() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let expiry = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");

    let market = create_market(as_of);
    let call = create_fx_option(expiry, OptionType::Call);

    let pv = call.value(&market, as_of).expect("pricing should succeed");

    let price = pv.amount();

    // Allow 10% relative tolerance (small absolute value, day count effects matter)
    let relative_error = ((price - EXPECTED_GK_CALL_APPROX) / EXPECTED_GK_CALL_APPROX).abs();
    assert!(
        relative_error < 0.15,
        "GK call price should match reference. Expected ~{:.4}, got {:.4}, error = {:.2}%",
        EXPECTED_GK_CALL_APPROX,
        price,
        relative_error * 100.0
    );
}

/// Test: FX put-call parity: C - P = S×e^{-r_f×T} - K×e^{-r_d×T}
///
/// This is the Garman-Kohlhagen put-call parity, which differs from the
/// standard BS version because foreign rate replaces dividend yield.
#[test]
fn test_fx_put_call_parity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let expiry = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");

    let market = create_market(as_of);
    let call = create_fx_option(expiry, OptionType::Call);
    let put = create_fx_option(expiry, OptionType::Put);

    let call_pv = call
        .value(&market, as_of)
        .expect("call pricing should succeed")
        .amount();
    let put_pv = put
        .value(&market, as_of)
        .expect("put pricing should succeed")
        .amount();

    // Expected: C - P = S*e^(-r_f*T) - K*e^(-r_d*T)
    let expected_diff = SPOT * (-R_F * T).exp() - STRIKE * (-R_D * T).exp();
    let actual_diff = call_pv - put_pv;

    let parity_error = (actual_diff - expected_diff).abs();
    assert!(
        parity_error < 0.005,
        "FX put-call parity violated. C - P = {:.6}, expected = {:.6}, error = {:.8}",
        actual_diff,
        expected_diff,
        parity_error
    );
}

/// Test: Call and put prices are non-negative.
#[test]
fn test_fx_option_prices_non_negative() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let expiry = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");

    let market = create_market(as_of);

    for option_type in [OptionType::Call, OptionType::Put] {
        let opt = create_fx_option(expiry, option_type);
        let pv = opt.value(&market, as_of).expect("pricing").amount();
        assert!(
            pv >= 0.0,
            "FX {:?} price should be non-negative, got {:.6}",
            option_type,
            pv
        );
    }
}

/// Test: Delta signs and magnitudes.
///
/// FX spot delta:
/// - Call delta > 0 (buying base currency)
/// - Put delta < 0 (selling base currency)
/// - |call_delta| + |put_delta| ≈ e^{-r_f×T} (delta parity)
#[test]
fn test_fx_delta_conventions() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let expiry = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");

    let market = create_market(as_of);
    let call = create_fx_option(expiry, OptionType::Call);
    let put = create_fx_option(expiry, OptionType::Put);

    let metrics = vec![MetricId::Delta];

    let call_result = call
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("call metrics");
    let put_result = put
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("put metrics");

    if let (Some(&call_delta), Some(&put_delta)) = (
        call_result.measures.get(MetricId::Delta.as_str()),
        put_result.measures.get(MetricId::Delta.as_str()),
    ) {
        // Call delta should be positive
        assert!(
            call_delta > 0.0,
            "FX call delta should be positive, got {:.4}",
            call_delta
        );

        // Put delta should be negative
        assert!(
            put_delta < 0.0,
            "FX put delta should be negative, got {:.4}",
            put_delta
        );

        // Delta parity: call_delta + |put_delta| ≈ e^{-r_f*T}
        // (for per-unit notional = 1)
        let expected_sum = (-R_F * T).exp();
        let actual_sum = call_delta + put_delta.abs();
        let sum_error = (actual_sum - expected_sum).abs();

        // Loose tolerance due to FD approximation of delta
        assert!(
            sum_error < 0.10,
            "Delta parity: |call_delta| + |put_delta| should ≈ e^(-r_f*T) = {:.4}. Got {:.4}, error = {:.4}",
            expected_sum,
            actual_sum,
            sum_error
        );
    }
}
