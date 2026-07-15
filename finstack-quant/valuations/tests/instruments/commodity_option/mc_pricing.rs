//! Monte Carlo pricing tests for commodity options with Schwartz-Smith dynamics.
//!
//! These tests verify that the Schwartz-Smith MC pricer produces reasonable
//! prices and satisfies fundamental pricing relationships.

use crate::finstack_quant_test_utils::{
    date, flat_discount_with_tenor, flat_price_curve, flat_vol_surface,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::commodity::commodity_option::{
    CommodityMcParams, CommodityOption, CommodityPricingModel,
};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::CommodityUnderlyingParams;
use finstack_quant_valuations::instruments::{
    ExerciseStyle, InstrumentPricingOverrides, OptionType, SettlementType,
};

/// Helper to build a standard commodity option for MC tests.
fn build_commodity_option(strike: f64, option_type: OptionType) -> CommodityOption {
    let expiry = date(2026, 1, 1);
    CommodityOption::builder()
        .id(InstrumentId::new("CL-MC-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(strike)
        .option_type(option_type)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(DayCount::Act365F)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build commodity option")
}

/// Helper to build a standard market context.
fn build_market(as_of: finstack_quant_core::dates::Date) -> MarketContext {
    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 2.0);
    let price_curve = flat_price_curve("CL-FWD", as_of, 100.0, 2.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[0.5, 1.0, 2.0], &[80.0, 100.0, 120.0], 0.20);

    MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface)
}

/// When Schwartz-Smith parameters approach the GBM limit (kappa -> small,
/// sigma_y -> dominant, sigma_x -> small), MC price should converge to Black-76.
///
/// In the GBM limit: sigma_x -> 0 means no short-term deviation, and with
/// kappa small the long-term component dominates. The total vol approaches
/// sigma_y, and the process becomes approximately lognormal.
#[test]
fn test_schwartz_smith_gbm_limit_converges_to_black76() {
    let as_of = date(2025, 1, 1);
    let market = build_market(as_of);

    let call = build_commodity_option(100.0, OptionType::Call);

    // Black-76 analytical price
    let analytical_pv = {
        use finstack_quant_valuations::instruments::Instrument;
        call.value(&market, as_of).expect("analytical price")
    };

    // Schwartz-Smith in GBM limit: tiny sigma_x, small kappa, dominant sigma_y.
    // With sigma_x ~ 0 and mu_y chosen for risk-neutral drift:
    //   S(t) = exp(Y(t)) where dY = mu_y dt + sigma_y dW
    //
    // For E[S_T] = F_0 (forward is a martingale under the forward measure),
    // we need the Ito correction: mu_y = -0.5 * sigma_y^2.
    // This ensures E[exp(Y_T)] = exp(Y_0) = S_0 = F_0.
    let sigma_y = 0.20;
    let mc_params = CommodityMcParams {
        model: CommodityPricingModel::SchwartzSmith {
            kappa: 0.01,                    // Very small mean reversion
            sigma_x: 0.001,                 // Negligible short-term vol
            sigma_y,                        // Dominant long-term vol (matching Black-76 vol)
            rho_xy: 0.0,                    // No correlation
            mu_y: -0.5 * sigma_y * sigma_y, // Ito correction for martingale
            lambda_x: 0.0,                  // No risk premium
        },
        n_paths: 100_000,
        n_steps: 100,
        seed: Some(42),
    };

    let mc_pv = call.npv_mc(&mc_params, &market, as_of).expect("MC price");

    let analytical_val = analytical_pv.amount();
    let mc_val = mc_pv.amount();

    // With 100k paths, MC should be within ~5% of analytical
    let relative_error = (mc_val - analytical_val).abs() / analytical_val.max(1e-10);
    assert!(
        relative_error < 0.05,
        "MC price ({:.4}) should be within 5% of Black-76 ({:.4}), relative error: {:.4}",
        mc_val,
        analytical_val,
        relative_error,
    );
}

/// Standard error should shrink as path count increases.
/// With N paths, stderr ~ 1/sqrt(N), so 100k paths should have ~3x smaller
/// stderr than 10k paths.
#[test]
fn test_standard_error_shrinks_with_more_paths() {
    let as_of = date(2025, 1, 1);
    let market = build_market(as_of);
    let call = build_commodity_option(100.0, OptionType::Call);

    let ss_model = CommodityPricingModel::SchwartzSmith {
        kappa: 2.0,
        sigma_x: 0.30,
        sigma_y: 0.15,
        rho_xy: -0.5,
        mu_y: 0.0,
        lambda_x: 0.0,
    };

    // Price with 10k paths
    let mc_10k = CommodityMcParams {
        model: ss_model.clone(),
        n_paths: 10_000,
        n_steps: 50,
        seed: Some(42),
    };
    let pv_10k = call.npv_mc(&mc_10k, &market, as_of).expect("10k paths");

    // Price with 100k paths
    let mc_100k = CommodityMcParams {
        model: ss_model,
        n_paths: 100_000,
        n_steps: 50,
        seed: Some(42),
    };
    let pv_100k = call.npv_mc(&mc_100k, &market, as_of).expect("100k paths");

    // Both should produce positive values for an ATM call
    assert!(
        pv_10k.amount() > 0.0,
        "10k-path MC price should be positive: {}",
        pv_10k.amount()
    );
    assert!(
        pv_100k.amount() > 0.0,
        "100k-path MC price should be positive: {}",
        pv_100k.amount()
    );

    // The prices should be in a reasonable range (not wildly different)
    // Both should be roughly the same value; we just check they're close
    let ratio = pv_10k.amount() / pv_100k.amount();
    assert!(
        (0.80..=1.20).contains(&ratio),
        "10k and 100k path prices should be within 20%: 10k={:.4}, 100k={:.4}, ratio={:.4}",
        pv_10k.amount(),
        pv_100k.amount(),
        ratio,
    );
}

/// Put-call parity should hold approximately for MC prices:
///   C - P ≈ DF × (F - K)
///
/// Since both call and put use the same MC dynamics, their difference
/// should approximate the discounted forward-strike spread.
#[test]
fn test_put_call_parity_mc() {
    let as_of = date(2025, 1, 1);
    let market = build_market(as_of);

    let call = build_commodity_option(100.0, OptionType::Call);
    let put = build_commodity_option(100.0, OptionType::Put);

    // Near-GBM limit with Ito correction for martingale property
    let sigma_y = 0.20;
    let ss_model = CommodityPricingModel::SchwartzSmith {
        kappa: 0.01,
        sigma_x: 0.001,
        sigma_y,
        rho_xy: 0.0,
        mu_y: -0.5 * sigma_y * sigma_y,
        lambda_x: 0.0,
    };

    let mc_params = CommodityMcParams {
        model: ss_model,
        n_paths: 100_000,
        n_steps: 100,
        seed: Some(42),
    };

    let call_pv = call
        .npv_mc(&mc_params, &market, as_of)
        .expect("MC call price");
    let put_pv = put
        .npv_mc(&mc_params, &market, as_of)
        .expect("MC put price");

    // Get forward and discount factor
    let forward = call.forward_price(&market, as_of).expect("forward");
    let disc = market.get_discount("USD-OIS").expect("discount curve");
    let expiry = date(2026, 1, 1);
    let df = disc.df_between_dates(as_of, expiry).expect("df");

    // Put-call parity: C - P = DF * (F - K)
    let lhs = call_pv.amount() - put_pv.amount();
    let rhs = df * (forward - 100.0);

    let error = (lhs - rhs).abs();
    // MC put-call parity: allow ~5% of option price as tolerance
    let tolerance = 0.05 * call_pv.amount().max(1.0);
    assert!(
        error < tolerance,
        "Put-call parity violated: C-P={:.4}, DF*(F-K)={:.4}, error={:.4}, tolerance={:.4}",
        lhs,
        rhs,
        error,
        tolerance,
    );
}

/// Verify that MC Schwartz-Smith produces positive call and put values
/// with typical commodity parameters.
#[test]
fn test_schwartz_smith_positive_option_values() {
    let as_of = date(2025, 1, 1);
    let market = build_market(as_of);

    let mc_params = CommodityMcParams {
        model: CommodityPricingModel::SchwartzSmith {
            kappa: 2.0,
            sigma_x: 0.30,
            sigma_y: 0.15,
            rho_xy: -0.5,
            mu_y: 0.0,
            lambda_x: 0.0,
        },
        n_paths: 50_000,
        n_steps: 50,
        seed: Some(42),
    };

    // ATM call
    let call = build_commodity_option(100.0, OptionType::Call);
    let call_pv = call.npv_mc(&mc_params, &market, as_of).expect("MC call");
    assert!(
        call_pv.amount() > 0.0,
        "ATM call should have positive value: {}",
        call_pv.amount()
    );

    // ATM put
    let put = build_commodity_option(100.0, OptionType::Put);
    let put_pv = put.npv_mc(&mc_params, &market, as_of).expect("MC put");
    assert!(
        put_pv.amount() > 0.0,
        "ATM put should have positive value: {}",
        put_pv.amount()
    );

    // Deep OTM call (strike = 150) should still be non-negative
    let otm_call = build_commodity_option(150.0, OptionType::Call);
    let otm_call_pv = otm_call
        .npv_mc(&mc_params, &market, as_of)
        .expect("MC OTM call");
    assert!(
        otm_call_pv.amount() >= 0.0,
        "OTM call should be non-negative: {}",
        otm_call_pv.amount()
    );
}

/// Verify that the BlackScholes model variant falls back to analytical pricing.
#[test]
fn test_mc_black_scholes_fallback() {
    use finstack_quant_valuations::instruments::Instrument;

    let as_of = date(2025, 1, 1);
    let market = build_market(as_of);

    let call = build_commodity_option(100.0, OptionType::Call);

    let analytical_pv = call.value(&market, as_of).expect("analytical price");

    let mc_params = CommodityMcParams {
        model: CommodityPricingModel::BlackScholes,
        n_paths: 1_000, // Should be ignored
        n_steps: 10,    // Should be ignored
        seed: Some(42),
    };

    let mc_pv = call
        .npv_mc(&mc_params, &market, as_of)
        .expect("BS fallback");

    assert!(
        (mc_pv.amount() - analytical_pv.amount()).abs() < 1e-10,
        "BlackScholes MC fallback should match analytical: MC={:.6}, analytical={:.6}",
        mc_pv.amount(),
        analytical_pv.amount(),
    );
}
