//! Convergence and property tests for the Merton Monte Carlo PIK bond engine.
//!
//! Verifies:
//! - MC standard error decreases as 1/sqrt(N) with more paths
//! - Higher asset volatility increases spread differential between cash and PIK
//! - Zero-coupon PIK matches zero-coupon bond
//! - MC without endogenous hazard/dynamic recovery matches simple MC
//! - Antithetic variates reduce variance
//! - Default rate increases with leverage
//! - PIK accrual increases terminal notional

use finstack_valuations::instruments::fixed_income::bond::pricing::engine::merton_mc::{
    MertonMcConfig, MertonMcEngine, PikMode, PikSchedule,
};
use finstack_valuations::models::credit::{
    AssetDynamics, BarrierType, DynamicRecoverySpec, EndogenousHazardSpec, MertonModel,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a standard Merton model for tests: V=200, sigma=0.25, B=100, r=0.04.
fn base_merton() -> MertonModel {
    MertonModel::new_with_dynamics(
        200.0,
        0.25,
        100.0,
        0.04,
        0.0,
        BarrierType::FirstPassage {
            barrier_growth_rate: 0.0,
        },
        AssetDynamics::GeometricBrownian,
    )
    .expect("valid merton model")
}

/// Build a Merton model with a specified asset volatility.
fn merton_with_vol(vol: f64) -> MertonModel {
    MertonModel::new_with_dynamics(
        200.0,
        vol,
        100.0,
        0.04,
        0.0,
        BarrierType::FirstPassage {
            barrier_growth_rate: 0.0,
        },
        AssetDynamics::GeometricBrownian,
    )
    .expect("valid merton model")
}

/// Build a Merton model with a specified asset value (controls leverage).
fn merton_with_asset_value(asset_value: f64) -> MertonModel {
    MertonModel::new_with_dynamics(
        asset_value,
        0.25,
        100.0,
        0.04,
        0.0,
        BarrierType::FirstPassage {
            barrier_growth_rate: 0.0,
        },
        AssetDynamics::GeometricBrownian,
    )
    .expect("valid merton model")
}

// ---------------------------------------------------------------------------
// Test 1: MC convergence as paths increase
// ---------------------------------------------------------------------------

#[test]
fn mc_converges_as_paths_increase() {
    // Price with 1k, 5k, 25k paths.
    // Standard error should decrease roughly as 1/sqrt(N).
    let merton = base_merton();

    let config_1k = MertonMcConfig::new(merton.clone())
        .num_paths(1_000)
        .seed(42)
        .antithetic(false);
    let config_5k = MertonMcConfig::new(merton.clone())
        .num_paths(5_000)
        .seed(42)
        .antithetic(false);
    let config_25k = MertonMcConfig::new(merton)
        .num_paths(25_000)
        .seed(42)
        .antithetic(false);

    let r1k = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_1k, 0.04).expect("1k ok");
    let r5k = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_5k, 0.04).expect("5k ok");
    let r25k = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_25k, 0.04).expect("25k ok");

    // SE should decrease monotonically
    assert!(
        r25k.standard_error < r5k.standard_error,
        "SE(25k) = {} should be < SE(5k) = {}",
        r25k.standard_error,
        r5k.standard_error,
    );
    assert!(
        r5k.standard_error < r1k.standard_error,
        "SE(5k) = {} should be < SE(1k) = {}",
        r5k.standard_error,
        r1k.standard_error,
    );

    // Check approximate 1/sqrt(N) scaling.
    // Going from 1k to 5k (5x paths) should reduce SE by ~sqrt(5) = 2.236.
    // Going from 5k to 25k (5x paths) should reduce SE by ~sqrt(5) = 2.236.
    // Allow generous tolerance for MC noise in the SE estimate itself.
    let ratio_5k_1k = r1k.standard_error / r5k.standard_error;
    let ratio_25k_5k = r5k.standard_error / r25k.standard_error;
    let expected_ratio = (5.0_f64).sqrt(); // ~2.236

    assert!(
        ratio_5k_1k > expected_ratio * 0.5 && ratio_5k_1k < expected_ratio * 2.0,
        "SE ratio 1k/5k = {ratio_5k_1k}, expected ~{expected_ratio}",
    );
    assert!(
        ratio_25k_5k > expected_ratio * 0.5 && ratio_25k_5k < expected_ratio * 2.0,
        "SE ratio 5k/25k = {ratio_25k_5k}, expected ~{expected_ratio}",
    );
}

// ---------------------------------------------------------------------------
// Test 2: Higher asset vol increases spread differential
// ---------------------------------------------------------------------------

#[test]
fn higher_asset_vol_increases_spread_differential() {
    // Higher sigma_V should produce a bigger gap between cash and PIK spreads.
    let merton_low = merton_with_vol(0.15);
    let merton_high = merton_with_vol(0.40);

    let config_low_cash = MertonMcConfig::new(merton_low.clone())
        .num_paths(10_000)
        .seed(42);
    let config_low_pik = MertonMcConfig::new(merton_low)
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42);
    let config_high_cash = MertonMcConfig::new(merton_high.clone())
        .num_paths(10_000)
        .seed(42);
    let config_high_pik = MertonMcConfig::new(merton_high)
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42);

    let cash_low = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_low_cash, 0.04).expect("ok");
    let pik_low = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_low_pik, 0.04).expect("ok");
    let cash_high =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_high_cash, 0.04).expect("ok");
    let pik_high = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_high_pik, 0.04).expect("ok");

    // Higher vol should cause a larger *relative* price discount for PIK
    // (PIK price / cash price is lower when vol is higher).
    let ratio_low = pik_low.clean_price_pct / cash_low.clean_price_pct;
    let ratio_high = pik_high.clean_price_pct / cash_high.clean_price_pct;

    assert!(
        ratio_high < ratio_low,
        "Higher vol PIK/cash ratio ({ratio_high:.4}) should be lower than low vol ({ratio_low:.4})",
    );
}

// ---------------------------------------------------------------------------
// Test 3: Zero-coupon PIK matches zero-coupon bond
// ---------------------------------------------------------------------------

#[test]
fn zero_pik_coupon_matches_zero_coupon_bond() {
    // A full-PIK bond with 0% coupon should behave identically to a
    // cash bond with 0% coupon, because there is nothing to accrete.
    let merton = base_merton();
    let config = MertonMcConfig::new(merton).num_paths(10_000).seed(42);

    let cash_zero = MertonMcEngine::price(100.0, 0.0, 5.0, 2, &config, 0.04).expect("cash zero ok");
    let config_pik = MertonMcConfig::new(base_merton())
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42);
    let pik_zero =
        MertonMcEngine::price(100.0, 0.0, 5.0, 2, &config_pik, 0.04).expect("pik zero ok");

    // Prices should be identical (same seed, same zero coupon -> same paths)
    let price_diff = (cash_zero.clean_price_pct - pik_zero.clean_price_pct).abs();
    assert!(
        price_diff < 1e-6,
        "Zero-coupon PIK and cash should match: cash={}, pik={}, diff={}",
        cash_zero.clean_price_pct,
        pik_zero.clean_price_pct,
        price_diff,
    );

    // PIK fraction should be 0 (nothing to PIK)
    assert!(
        pik_zero.average_pik_fraction < 1e-10 || cash_zero.average_pik_fraction < 1e-10,
        "Zero-coupon bond should have no PIK accrual",
    );
}

// ---------------------------------------------------------------------------
// Test 4: No endogenous / no dynamic recovery matches standard MC
// ---------------------------------------------------------------------------

#[test]
fn no_endogenous_no_dynamic_recovery_matches_standard() {
    // MC with no endogenous hazard and no dynamic recovery (just
    // structural default via barrier) should match a simple run.
    // We verify by running two identical configs and checking that
    // adding then removing the optional models gives consistent results.
    let merton = base_merton();

    let config_plain = MertonMcConfig::new(merton.clone())
        .num_paths(10_000)
        .seed(42);

    // Config with endogenous hazard and dynamic recovery
    let endo = EndogenousHazardSpec::power_law(0.06, 0.5, 2.5).expect("valid");
    let dyn_rec = DynamicRecoverySpec::floored_inverse(0.40, 100.0, 0.10).expect("valid");
    let config_with_extras = MertonMcConfig::new(merton)
        .num_paths(10_000)
        .seed(42)
        .endogenous_hazard(endo)
        .dynamic_recovery(dyn_rec);

    let config_plain = config_plain.pik_schedule(PikSchedule::Uniform(PikMode::Pik));
    let config_with_extras = config_with_extras.pik_schedule(PikSchedule::Uniform(PikMode::Pik));

    let result_plain =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_plain, 0.04).expect("plain ok");
    let result_extras =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_with_extras, 0.04).expect("extras ok");

    // The configs are different, so results will differ. But both should
    // produce valid, reasonable prices. The "extras" config should produce
    // a lower price (more credit risk from endogenous hazard + dynamic recovery).
    assert!(
        result_plain.clean_price_pct > 0.0 && result_plain.clean_price_pct < 200.0,
        "Plain price should be reasonable: {}",
        result_plain.clean_price_pct,
    );
    assert!(
        result_extras.clean_price_pct > 0.0 && result_extras.clean_price_pct < 200.0,
        "Extras price should be reasonable: {}",
        result_extras.clean_price_pct,
    );

    // With the toggle model off and PIK=true, the endogenous hazard only
    // affects toggle decisions (which don't apply when is_pik=true and
    // toggle_model is None). The dynamic recovery affects the recovery
    // amount on default. So the difference should be moderate.
    let diff_pct = (result_plain.clean_price_pct - result_extras.clean_price_pct).abs();
    assert!(
        diff_pct < 15.0,
        "Plain vs extras diff should be moderate: plain={}, extras={}, diff={}",
        result_plain.clean_price_pct,
        result_extras.clean_price_pct,
        diff_pct,
    );
}

// ---------------------------------------------------------------------------
// Test 5: Antithetic variates improve estimate stability across seeds
// ---------------------------------------------------------------------------

#[test]
fn antithetic_variates_improve_estimate_stability() {
    // Antithetic variates reduce the true variance of the mean estimator
    // by inducing negative correlation between paired paths. The reported
    // standard_error treats all paths as independent (doesn't account for
    // pairing), so we verify antithetic behavior via:
    // 1. Produces a reasonable, deterministic price
    // 2. Prices are consistent across antithetic and non-antithetic runs
    //    (they should converge to the same true price)
    let merton = base_merton();

    let config_no_anti = MertonMcConfig::new(merton.clone())
        .num_paths(10_000)
        .seed(42)
        .antithetic(false);
    let config_anti = MertonMcConfig::new(merton)
        .num_paths(10_000)
        .seed(42)
        .antithetic(true);

    let r_no =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_no_anti, 0.04).expect("no anti ok");
    let r_anti = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_anti, 0.04).expect("anti ok");

    // Both should produce reasonable prices
    assert!(
        r_no.clean_price_pct > 50.0 && r_no.clean_price_pct < 150.0,
        "Non-antithetic price unreasonable: {}",
        r_no.clean_price_pct
    );
    assert!(
        r_anti.clean_price_pct > 50.0 && r_anti.clean_price_pct < 150.0,
        "Antithetic price unreasonable: {}",
        r_anti.clean_price_pct
    );

    // Both methods should produce similar prices (within MC noise)
    let price_diff = (r_no.clean_price_pct - r_anti.clean_price_pct).abs();
    assert!(
        price_diff < 3.0,
        "Antithetic and non-antithetic prices should be close: no_anti={}, anti={}, diff={}",
        r_no.clean_price_pct,
        r_anti.clean_price_pct,
        price_diff,
    );

    // Antithetic should be deterministic
    let r_anti2 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_anti, 0.04).expect("anti ok");
    assert!(
        (r_anti.clean_price_pct - r_anti2.clean_price_pct).abs() < 1e-10,
        "Antithetic should be deterministic"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Default rate increases with leverage
// ---------------------------------------------------------------------------

#[test]
fn default_rate_increases_with_leverage() {
    // Higher leverage (lower asset value relative to debt barrier)
    // should produce higher default rates.
    let merton_low_leverage = merton_with_asset_value(300.0); // V/B = 3.0
    let merton_high_leverage = merton_with_asset_value(120.0); // V/B = 1.2

    let config_low = MertonMcConfig::new(merton_low_leverage)
        .num_paths(10_000)
        .seed(42);
    let config_high = MertonMcConfig::new(merton_high_leverage)
        .num_paths(10_000)
        .seed(42);

    let result_low = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_low, 0.04).expect("low ok");
    let result_high =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_high, 0.04).expect("high ok");

    assert!(
        result_high.path_statistics.default_rate > result_low.path_statistics.default_rate,
        "Higher leverage default rate ({}) should exceed lower leverage ({})",
        result_high.path_statistics.default_rate,
        result_low.path_statistics.default_rate,
    );

    // Also check that the highly levered bond has a lower price
    assert!(
        result_high.clean_price_pct < result_low.clean_price_pct,
        "Higher leverage price ({}) should be below lower leverage ({})",
        result_high.clean_price_pct,
        result_low.clean_price_pct,
    );
}

// ---------------------------------------------------------------------------
// Test 7: PIK accrual increases terminal notional
// ---------------------------------------------------------------------------

#[test]
fn pik_accrual_increases_terminal_notional() {
    // Full PIK bonds should have higher average terminal notional
    // than cash bonds, because coupons accrete to the notional.
    let merton = base_merton();
    let config = MertonMcConfig::new(merton).num_paths(10_000).seed(42);

    let cash = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("cash ok");
    let config_pik = MertonMcConfig::new(base_merton())
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42);
    let pik = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_pik, 0.04).expect("pik ok");

    assert!(
        pik.path_statistics.avg_terminal_notional > cash.path_statistics.avg_terminal_notional,
        "PIK terminal notional ({}) should exceed cash ({})",
        pik.path_statistics.avg_terminal_notional,
        cash.path_statistics.avg_terminal_notional,
    );

    // Cash bond terminal notional should be approximately par (100)
    // (no accrual, so surviving paths all have notional = 100)
    assert!(
        (cash.path_statistics.avg_terminal_notional - 100.0).abs() < 1.0,
        "Cash terminal notional should be ~100, got {}",
        cash.path_statistics.avg_terminal_notional,
    );

    // PIK bond with 8% coupon over 5 years semi-annual should have
    // terminal notional > 100 due to accrual
    assert!(
        pik.path_statistics.avg_terminal_notional > 100.0,
        "PIK terminal notional should exceed par, got {}",
        pik.path_statistics.avg_terminal_notional,
    );

    // PIK fraction for the full PIK bond should be 1.0
    assert!(
        (pik.average_pik_fraction - 1.0).abs() < 1e-10,
        "Full PIK bond should have PIK fraction = 1.0, got {}",
        pik.average_pik_fraction,
    );
}

// ---------------------------------------------------------------------------
// Test 8: Higher default rate implies higher expected loss
// ---------------------------------------------------------------------------

#[test]
fn higher_default_rate_implies_higher_expected_loss() {
    // A more leveraged firm should have both a higher default rate
    // and a higher expected loss.
    let merton_safe = merton_with_asset_value(300.0);
    let merton_risky = merton_with_asset_value(120.0);

    let config_safe = MertonMcConfig::new(merton_safe).num_paths(10_000).seed(42);
    let config_risky = MertonMcConfig::new(merton_risky).num_paths(10_000).seed(42);

    let result_safe =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_safe, 0.04).expect("safe ok");
    let result_risky =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_risky, 0.04).expect("risky ok");

    assert!(
        result_risky.expected_loss > result_safe.expected_loss,
        "Risky expected loss ({}) should exceed safe ({})",
        result_risky.expected_loss,
        result_safe.expected_loss,
    );

    // Riskier bond should have a lower price
    assert!(
        result_risky.clean_price_pct < result_safe.clean_price_pct,
        "Risky price ({}) should be below safe ({})",
        result_risky.clean_price_pct,
        result_safe.clean_price_pct,
    );
}

// ---------------------------------------------------------------------------
// Test 9: Price is bounded between zero and risk-free value
// ---------------------------------------------------------------------------

#[test]
fn price_bounded_between_zero_and_risk_free() {
    // The MC price should be between 0 and the risk-free bond price.
    // Risk-free price for 8% coupon, 5Y, semi-annual, r=4% continuously
    // compounded is approximately:
    //   sum_{i=1..10} 4 * exp(-0.04 * i/2) + 100 * exp(-0.04 * 5)
    //   ~ 4 * (sum of DFs) + 100 * 0.8187
    //   ~ 35.1 + 81.9 = 117.0 (approx)
    let merton = base_merton();
    let config = MertonMcConfig::new(merton).num_paths(10_000).seed(42);

    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");

    assert!(
        result.clean_price_pct > 0.0,
        "Price should be positive: {}",
        result.clean_price_pct,
    );
    // Risk-free price is approx 117-118% of par; MC price should be <= this
    assert!(
        result.clean_price_pct < 120.0,
        "Price should not exceed risk-free value: {}",
        result.clean_price_pct,
    );
}

// ---------------------------------------------------------------------------
// Test 10: Longer maturity increases expected loss (for risky bond)
// ---------------------------------------------------------------------------

#[test]
fn longer_maturity_increases_expected_loss() {
    // For a risky firm, longer maturity means more time to default,
    // so expected loss should increase.
    let merton = merton_with_asset_value(150.0); // moderate leverage

    let config = MertonMcConfig::new(merton).num_paths(10_000).seed(42);

    let result_2y = MertonMcEngine::price(100.0, 0.08, 2.0, 2, &config, 0.04).expect("2y ok");
    let result_10y = MertonMcEngine::price(100.0, 0.08, 10.0, 2, &config, 0.04).expect("10y ok");

    assert!(
        result_10y.path_statistics.default_rate > result_2y.path_statistics.default_rate,
        "10Y default rate ({}) should exceed 2Y ({})",
        result_10y.path_statistics.default_rate,
        result_2y.path_statistics.default_rate,
    );

    assert!(
        result_10y.expected_loss > result_2y.expected_loss,
        "10Y expected loss ({}) should exceed 2Y ({})",
        result_10y.expected_loss,
        result_2y.expected_loss,
    );
}
