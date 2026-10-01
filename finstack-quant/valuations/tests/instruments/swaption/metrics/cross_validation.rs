//! Cross-validation tests between analytical and numerical Greeks

use crate::instruments::swaption::common::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::prelude::ToPrimitive;

#[test]
fn test_delta_gamma_consistency() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);

    // Compute delta and gamma
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();
    let gamma = *result.measures.get("gamma").unwrap();

    // For ATM options, both should be positive and reasonable
    assert!(delta > 0.0, "Delta should be positive");
    assert!(gamma >= 0.0, "Gamma should be non-negative");

    // Gamma should be smaller in magnitude than delta (typically)
    // This is a rough heuristic check - can be violated for scaled cash greeks
    assert!(gamma.abs() < delta.abs() * 100.0, "Gamma magnitude check");
}

#[test]
fn test_metric_stability_across_maturities() {
    let as_of = time::macros::date!(2024 - 01 - 01);
    let swap_start = time::macros::date!(2025 - 01 - 01);
    let swap_end = time::macros::date!(2030 - 01 - 01);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let metrics = vec![MetricId::Delta, MetricId::Vega, MetricId::Dv01];

    // Test across different expiries
    for months in [3, 6, 12, 24] {
        let expiry = as_of
            .checked_add(time::Duration::days(months * 30))
            .unwrap();
        if expiry <= swap_start {
            let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);

            let result = swaption
                .price_with_metrics(
                    &market,
                    as_of,
                    &metrics,
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap();

            // All metrics should be finite and reasonable
            for (name, value) in &result.measures {
                assert!(
                    value.is_finite(),
                    "{} should be finite for {}M expiry, got: {}",
                    name,
                    months,
                    value
                );
            }
        }
    }
}

/// A live option keeps its forward-rate Greeks across the former 1/252 cutoff.
/// Both analytic identities and bumps hold the settlement annuity fixed.
#[test]
fn positive_time_greeks_match_frozen_annuity_formulas_and_bumps() {
    use finstack_quant_core::math::{norm_cdf, norm_pdf};
    use finstack_quant_models::closed_form::{
        bachelier_call, bachelier_put, black_call, black_put,
    };
    use finstack_quant_models::volatility::SabrParameters;
    use finstack_quant_valuations::instruments::{
        InstrumentPricingOverrides, OptionType, VolatilityModel,
    };
    use rust_decimal::Decimal;
    use time::macros::date;

    let as_of = date!(2025 - 01 - 02);
    let market = create_flat_market(as_of, 0.05, 0.20);
    for (model, shift, sigma) in [
        (VolatilityModel::Normal, 0.0, 0.005),
        (VolatilityModel::Black, 0.0, 0.20),
        (VolatilityModel::Black, 0.02, 0.20),
    ] {
        for days in [1, 2, 3] {
            let expiry = as_of + time::Duration::days(days);
            for offset in [-0.0002, 0.0, 0.0002] {
                for side in [OptionType::Call, OptionType::Put] {
                    // Keep the underlying exactly five years for each expiry;
                    // the standard fixture has no stub convention.
                    let maturity = expiry.replace_year(expiry.year() + 5).unwrap();
                    let mut option = create_standard_payer_swaption(expiry, expiry, maturity, 0.05);
                    option.vol_model = model;
                    option.option_type = side;
                    option.instrument_pricing_overrides =
                        InstrumentPricingOverrides::default().with_implied_volatility(sigma);
                    if shift > 0.0 {
                        option.sabr_params = Some(SabrParameters {
                            alpha: 0.20,
                            beta: 1.0,
                            rho: 0.0,
                            nu: 0.0,
                            shift: Some(shift),
                        });
                    }
                    let forward = option.forward_swap_rate(&market, as_of).unwrap();
                    option.underlying_fixed_leg.rate = Decimal::try_from(forward + offset).unwrap();
                    let strike = option.get_strike().to_f64().unwrap();
                    let inputs = option.greek_inputs(&market, as_of).unwrap().unwrap();
                    let scale = inputs.annuity * option.notional.amount();
                    let t = days as f64 / 365.0;
                    let sqrt_t = t.sqrt();
                    let f = forward + shift;
                    let k = strike + shift;
                    let normal = model == VolatilityModel::Normal;
                    let d = if normal {
                        (f - k) / (sigma * sqrt_t)
                    } else {
                        ((f / k).ln() + 0.5 * sigma * sigma * t) / (sigma * sqrt_t)
                    };
                    let expected_delta =
                        scale * (norm_cdf(d) - if side == OptionType::Put { 1.0 } else { 0.0 });
                    let expected_gamma =
                        scale * norm_pdf(d) / (sigma * sqrt_t * if normal { 1.0 } else { f });
                    let expected_vega =
                        scale * norm_pdf(d) * sqrt_t * if normal { 0.01 } else { f * 0.01 };
                    let result = option
                        .price_with_metrics(
                            &market,
                            as_of,
                            &[MetricId::Delta, MetricId::Gamma, MetricId::Vega],
                            Default::default(),
                        )
                        .unwrap();
                    for (name, expected) in [
                        ("delta", expected_delta),
                        ("gamma", expected_gamma),
                        ("vega", expected_vega),
                    ] {
                        let actual = result.measures.get(name).unwrap();
                        assert!((actual-expected).abs() <= expected.abs().max(1.0)*1e-10,
                            "{model:?}/{shift}/{days}/{offset}/{side:?} {name}: actual={actual}, expected={expected}");
                    }
                    let price = |forward: f64, volatility: f64| {
                        scale
                            * match (normal, side) {
                                (true, OptionType::Call) => {
                                    bachelier_call(forward, k, volatility, t)
                                }
                                (true, OptionType::Put) => bachelier_put(forward, k, volatility, t),
                                (false, OptionType::Call) => black_call(forward, k, volatility, t),
                                (false, OptionType::Put) => black_put(forward, k, volatility, t),
                            }
                    };
                    for bump in [1e-7, 5e-8] {
                        let delta =
                            (price(f + bump, sigma) - price(f - bump, sigma)) / (2.0 * bump);
                        let gamma = (price(f + bump, sigma) - 2.0 * price(f, sigma)
                            + price(f - bump, sigma))
                            / (bump * bump);
                        assert!(
                            (delta - expected_delta).abs() < expected_delta.abs().max(1.0) * 1e-5
                        );
                        assert!(
                            (gamma - expected_gamma).abs() < expected_gamma.abs().max(1.0) * 1e-3
                        );
                    }
                    let bump = sigma * 1e-4;
                    let vega =
                        (price(f, sigma + bump) - price(f, sigma - bump)) / (2.0 * bump) * 0.01;
                    assert!((vega - expected_vega).abs() < expected_vega.abs().max(1.0) * 1e-5);
                }
            }
        }
    }
}

#[test]
fn zero_volatility_and_expired_greeks_have_explicit_finite_limits() {
    use finstack_quant_valuations::instruments::{
        InstrumentPricingOverrides, OptionType, VolatilityModel,
    };
    use rust_decimal::Decimal;
    use time::macros::date;
    let as_of = date!(2025 - 01 - 02);
    let expiry = date!(2025 - 01 - 03);
    let market = create_flat_market(as_of, 0.0, 0.0);
    for side in [OptionType::Call, OptionType::Put] {
        let mut option = create_standard_payer_swaption(expiry, expiry, date!(2030 - 01 - 03), 0.0);
        option.vol_model = VolatilityModel::Normal;
        option.option_type = side;
        option.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_implied_volatility(0.0);
        for strike in [Decimal::new(-1, 2), Decimal::ZERO, Decimal::new(1, 2)] {
            option.underlying_fixed_leg.rate = strike;
            let inputs = option.greek_inputs(&market, as_of).unwrap().unwrap();
            assert_eq!(inputs.forward, 0.0);
            let expected_call_delta = if strike <= Decimal::ZERO { 1.0 } else { 0.0 };
            let expected_delta = (expected_call_delta
                - if side == OptionType::Put { 1.0 } else { 0.0 })
                * inputs.annuity
                * option.notional.amount();
            let result = option
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::Delta, MetricId::Gamma, MetricId::Vega],
                    Default::default(),
                )
                .unwrap();
            assert_eq!(*result.measures.get("delta").unwrap(), expected_delta);
            assert_eq!(*result.measures.get("gamma").unwrap(), 0.0);
            assert_eq!(*result.measures.get("vega").unwrap(), 0.0);
        }
        let expired = option
            .price_with_metrics(
                &market,
                expiry + time::Duration::days(1),
                &[MetricId::Delta, MetricId::Gamma, MetricId::Vega],
                Default::default(),
            )
            .unwrap();
        for name in ["delta", "gamma", "vega"] {
            assert_eq!(*expired.measures.get(name).unwrap(), 0.0);
        }
    }
}
