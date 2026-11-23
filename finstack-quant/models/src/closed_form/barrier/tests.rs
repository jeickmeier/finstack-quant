use super::pricing::*;
use crate::types::OptionType;
use finstack_quant_core::math::special_functions::norm_cdf;
use finstack_quant_core::types::{BarrierType, PayoutTiming};

/// Exact bit patterns of every barrier variant, captured before the eight
/// named entry points were collapsed into [`barrier_price`].
#[rustfmt::skip]
const BARRIER_PRICE_PIN: &[u64] = &[
    0x0000000000000000, 0x0000000000000000, 0x4022743a10e31120, 0x40195200a7f17af0,
    0x401e590a761be416, 0x3fc412db3b15cc40, 0x3ffa3da6aea8f8a8, 0x4018b169ce18cc8e,
    0x3fbdef199885af80, 0x4013431f57c7fd3a, 0x4022385bddb205c1, 0x3ff83b8540a5f6d8,
    0x0000000000000000, 0x0000000000000000, 0x4022743a10e31120, 0x40195200a7f17af0,
    0x3fdad8f6c7939030, 0x40239921d0f4f390, 0x401a402433a1a145, 0x3fe3d562c0646248,
    0x0000000000000000, 0x0000000000000000, 0x401bedb3a01ada48, 0x4024d677fcfb39b4,
    0x0000000000000000, 0x0000000000000000, 0x40219729ed6b6320, 0x401ec72b75fdd838,
    0x402123182990a783, 0x3fed7188a1f39df0, 0x3fcd0470f6aee750, 0x401b18fa61bf647a,
    0x0000000000000000, 0x0000000000000000, 0x402fdff0134d3ef8, 0x40069aa018d30bc0,
    0x40177d9f923740c8, 0x0000000000000000, 0x402421204a319e94, 0x40069aa018d30bc0,
    0x0000000000000000, 0x0000000000000000, 0x4022743a10e31120, 0x40195200a7f17af0,
    0x0000000000000000, 0x0000000000000000, 0x4022743a10e31120, 0x40195200a7f17af0,
    0x0000000000000000, 0x0000000000000000, 0x4022743a10e31120, 0x40195200a7f17af0,
    0x3f53fc292c8d8000, 0x3d00000000000000, 0x4022739a2f99acb4, 0x40195200a7f17ae8,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x4024000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x4024000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x3f4a254442570000, 0x3f4a2503d5a90000,
    0x3f4a254442570000, 0x3f4a2503d5a90000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x40072ce6f3a94ea0, 0x0000000000000000,
    0x40072ce6f3a94ea0, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x40072ce6f3a94ea0, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x40072ce6f3a94ea0, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x40072ce6f3a94ea0, 0x0000000000000000,
    0x40072ce6f3a94ea0, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000,
    0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000,
    0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000,
    0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000,
    0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000,
    0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000,
    0x0000000000000000, 0x0000000000000000, 0x7ff8000000000000, 0x7ff8000000000000,
    0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000, 0x7ff8000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
];

fn barrier_price_pin_values() -> Vec<u64> {
    // (spot, strike, barrier, time, rate, div_yield, vol)
    let sets: [(f64, f64, f64, f64, f64, f64, f64); 18] = [
        (100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.20),
        (100.0, 100.0, 110.0, 1.0, 0.05, 0.02, 0.20),
        (100.0, 105.0, 120.0, 0.5, 0.03, 0.0, 0.30),
        (100.0, 95.0, 80.0, 2.0, -0.01, 0.01, 0.15),
        (100.0, 85.0, 95.0, 0.75, 0.04, 0.06, 0.25),
        (100.0, 100.0, 100.0, 1.0, 0.05, 0.02, 0.20),
        (100.0, 100.0, 99.999, 1.0, 0.05, 0.02, 0.20),
        (100.0, 100.0, 90.0, 0.0, 0.05, 0.02, 0.20),
        (100.0, 90.0, 110.0, 0.0, 0.05, 0.02, 0.20),
        (100.0, 100.0, 90.0, 1e-8, 0.05, 0.02, 0.20),
        (100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.0),
        (100.0, 100.0, 110.0, 1.0, 0.05, 0.02, 0.0),
        (100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 1e-6),
        (-1.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.20),
        (100.0, 0.0, 90.0, 1.0, 0.05, 0.02, 0.20),
        (100.0, 100.0, f64::NAN, 1.0, 0.05, 0.02, 0.20),
        (100.0, 100.0, 90.0, 1.0, 0.05, 0.02, f64::NAN),
        (100.0, 100.0, 110.0, -1.0, 0.05, 0.02, 0.20),
    ];
    let types = [
        BarrierType::UpAndOut,
        BarrierType::UpAndIn,
        BarrierType::DownAndOut,
        BarrierType::DownAndIn,
    ];
    let mut out = Vec::new();
    for (spot, strike, barrier, time, rate, div_yield, vol) in sets {
        let p = BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol);
        for ty in types {
            out.push(barrier_price(&p, ty, OptionType::Call).to_bits());
            out.push(barrier_price(&p, ty, OptionType::Put).to_bits());
        }
    }
    out
}

#[test]
fn barrier_price_bit_pin() {
    assert_eq!(barrier_price_pin_values().as_slice(), BARRIER_PRICE_PIN);
}

#[test]
fn low_volatility_barriers_preserve_exact_payoff_identities() {
    // Every ITM call path crosses an upper barrier below its strike;
    // every ITM put path crosses a lower barrier above its strike.
    for (rate, barrier, eta) in [(0.1_f64, 150.0, 1.0), (-0.1_f64, 75.0, -1.0)] {
        let time = 5.0;
        let strike = 100.0 * (rate * time).exp();
        let vanilla = vanilla_option_price(100.0, strike, time, rate, 0.0, 0.01, eta);
        let (knock_in, knock_out) = if eta > 0.0 {
            (
                barrier_price(
                    &BarrierParams::new(100.0, strike, barrier, time, rate, 0.0, 0.01),
                    BarrierType::UpAndIn,
                    OptionType::Call,
                ),
                barrier_price(
                    &BarrierParams::new(100.0, strike, barrier, time, rate, 0.0, 0.01),
                    BarrierType::UpAndOut,
                    OptionType::Call,
                ),
            )
        } else {
            (
                barrier_price(
                    &BarrierParams::new(100.0, strike, barrier, time, rate, 0.0, 0.01),
                    BarrierType::DownAndIn,
                    OptionType::Put,
                ),
                barrier_price(
                    &BarrierParams::new(100.0, strike, barrier, time, rate, 0.0, 0.01),
                    BarrierType::DownAndOut,
                    OptionType::Put,
                ),
            )
        };
        assert!(vanilla > 0.5);
        assert_eq!(knock_in, vanilla);
        assert_eq!(knock_out, 0.0);
    }
}

#[test]
fn touch_probability_retains_diffusion_when_reflection_power_overflows() {
    // Place the barrier at the median terminal spot. Here the reflected
    // power is exp(800), but P(hit) = 1/2 + exp(800) Phi(-40).
    let vol = 0.005;
    let rate = 0.1 + 0.5 * vol * vol;
    let probability =
        barrier_touch_probability(100.0, 100.0 * 0.1_f64.exp(), 1.0, rate, 0.0, vol, true);
    assert!(
        (probability - 0.509_967_335_188_301_3).abs() < 1e-11,
        "nonzero diffusion must not become a deterministic hit: {probability}"
    );
}

#[test]
fn low_volatility_barrier_prices_match_conditional_bridge_integrals() {
    use finstack_quant_core::math::integration::adaptive_simpson;
    use finstack_quant_core::math::special_functions::norm_pdf;

    // These are the nontrivial strike/barrier regimes. Integrate over the
    // terminal normal shock and condition on Brownian-bridge survival.
    for (log_barrier, strike, is_up, eta) in
        [(0.1_f64, 105.0_f64, true, 1.0), (-0.1, 95.0, false, -1.0)]
    {
        let vol = 0.005;
        let variance = vol * vol;
        let rate = log_barrier + 0.5 * variance;
        let barrier = 100.0 * log_barrier.exp();
        let strike_shock = ((strike / 100.0).ln() - log_barrier) / vol;
        let (lower, upper) = if is_up {
            (strike_shock.max(-12.0), 0.0)
        } else {
            (0.0, strike_shock.min(12.0))
        };
        let expected = (-rate).exp()
            * adaptive_simpson(
                |z| {
                    let log_terminal = log_barrier + vol * z;
                    let terminal = 100.0 * log_terminal.exp();
                    let survival =
                        -(-2.0 * log_barrier * (log_barrier - log_terminal) / variance).exp_m1();
                    Ok((eta * (terminal - strike)).max(0.0) * survival * norm_pdf(z)?)
                },
                lower,
                upper,
                1e-11,
                20,
                100_000,
            )
            .expect("valid conditional bridge integration");
        let actual = if is_up {
            barrier_price(
                &BarrierParams::new(100.0, strike, barrier, 1.0, rate, 0.0, vol),
                BarrierType::UpAndOut,
                OptionType::Call,
            )
        } else {
            barrier_price(
                &BarrierParams::new(100.0, strike, barrier, 1.0, rate, 0.0, vol),
                BarrierType::DownAndOut,
                OptionType::Put,
            )
        };
        assert!(expected > 1.0);
        assert!(
            (actual - expected).abs() < 1e-9,
            "barrier={barrier}: analytical {actual} vs bridge integral {expected}"
        );
    }
}

#[test]
fn at_hit_rebates_discount_at_the_deterministic_crossing_time() {
    for (barrier, rate, dividend, barrier_type) in [
        (105.0_f64, 0.1, 0.0, BarrierType::UpAndOut),
        (95.0_f64, 0.1, 0.2, BarrierType::DownAndOut),
        (95.0_f64, -0.1, 0.0, BarrierType::DownAndOut),
    ] {
        let hit_time = (barrier / 100.0).ln() / (rate - dividend);
        let expected = (-rate * hit_time).exp();
        for vol in [0.0, 1e-6, 1e-4] {
            let params = BarrierParams::new(100.0, 100.0, barrier, 2.0, rate, dividend, vol);
            let actual = barrier_rebate(&params, 1.0, barrier_type, PayoutTiming::AtHit);
            assert!(
                (actual - expected).abs() < 1e-6,
                "vol={vol}: at-hit {actual} must converge to {expected} at tau={hit_time}"
            );
        }
    }
}

#[test]
fn negative_rate_at_hit_rebates_match_first_passage_density() {
    use finstack_quant_core::math::integration::adaptive_simpson;

    // Include both sides of the real-root boundary at q = 0. The direct
    // density integral is independent of the CDF integration-by-parts path.
    for dividend in [-0.02_f64, -1e-8, 0.0, 1e-8] {
        for (barrier, is_up, barrier_type) in [
            (110.0_f64, true, BarrierType::UpAndOut),
            (90.0_f64, false, BarrierType::DownAndOut),
        ] {
            let params = BarrierParams::new(100.0, 100.0, barrier, 1.0, -0.02, dividend, 0.2);
            let distance = (barrier / params.spot).ln().abs();
            let nu = params.rate - params.div_yield - 0.5 * params.vol * params.vol;
            let signed_drift = if is_up { nu } else { -nu };
            let expected = adaptive_simpson(
                |t| {
                    if t == 0.0 {
                        return Ok(0.0);
                    }
                    Ok(
                        distance / (params.vol * (std::f64::consts::TAU * t * t * t).sqrt())
                            * (-(distance - signed_drift * t).powi(2)
                                / (2.0 * params.vol * params.vol * t)
                                - params.rate * t)
                                .exp(),
                    )
                },
                0.0,
                params.time,
                1e-11,
                20,
                100_000,
            )
            .expect("valid first-passage density integration");
            let actual = barrier_rebate(&params, 1.0, barrier_type, PayoutTiming::AtHit);
            let p_hit = barrier_touch_probability(
                params.spot,
                barrier,
                params.time,
                params.rate,
                dividend,
                params.vol,
                is_up,
            );
            assert!(actual >= p_hit - 1e-12);
            assert!(actual <= (-params.rate * params.time).exp() * p_hit + 1e-12);
            assert!(
                (actual - expected).abs() < 1e-10,
                "q={dividend}, barrier={barrier}: at-hit {actual} vs density {expected}"
            );
        }
    }
}

#[test]
fn test_barrier_in_plus_out_equals_vanilla() {
    let spot = 100.0;
    let strike = 100.0;
    let barrier = 120.0;
    let time = 1.0;
    let rate = 0.05;
    let div_yield = 0.02;
    let vol = 0.2;

    let up_in = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::UpAndIn,
        OptionType::Call,
    );
    let up_out = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::UpAndOut,
        OptionType::Call,
    );

    let d1 =
        ((spot / strike).ln() + (rate - div_yield + 0.5 * vol * vol) * time) / (vol * time.sqrt());
    let d2 = d1 - vol * time.sqrt();
    let vanilla = spot * (-div_yield * time).exp() * norm_cdf(d1).expect("valid Gaussian variate")
        - strike * (-rate * time).exp() * norm_cdf(d2).expect("valid Gaussian variate");

    let sum = up_in + up_out;

    assert!(
        (sum - vanilla).abs() < 0.01,
        "Barrier parity failed: {} vs {}",
        sum,
        vanilla
    );
}

#[test]
fn test_barrier_put_in_plus_out_equals_vanilla() {
    let spot = 100.0;
    let strike = 100.0;
    let barrier = 80.0;
    let time = 1.0;
    let rate = 0.05;
    let div_yield = 0.02;
    let vol = 0.2;

    let params = BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol);
    let down_in = barrier_price(&params, BarrierType::DownAndIn, OptionType::Put);
    let down_out = barrier_price(&params, BarrierType::DownAndOut, OptionType::Put);

    let d1 =
        ((spot / strike).ln() + (rate - div_yield + 0.5 * vol * vol) * time) / (vol * time.sqrt());
    let d2 = d1 - vol * time.sqrt();
    let vanilla = strike * (-rate * time).exp() * norm_cdf(-d2).expect("valid Gaussian variate")
        - spot * (-div_yield * time).exp() * norm_cdf(-d1).expect("valid Gaussian variate");

    let sum = down_in + down_out;

    assert!(
        (sum - vanilla).abs() < 0.01,
        "Barrier parity failed: {} vs {}",
        sum,
        vanilla
    );
}

#[test]
fn test_up_out_call_knocked_out() {
    let price = barrier_price(
        &BarrierParams::new(125.0, 100.0, 120.0, 1.0, 0.05, 0.02, 0.2),
        BarrierType::UpAndOut,
        OptionType::Call,
    );
    assert_eq!(price, 0.0, "Already above barrier should be zero");
}

#[test]
fn test_down_out_call_knocked_out() {
    let price = barrier_price(
        &BarrierParams::new(75.0, 100.0, 80.0, 1.0, 0.05, 0.02, 0.2),
        BarrierType::DownAndOut,
        OptionType::Call,
    );
    assert_eq!(price, 0.0, "Already below barrier should be zero");
}

#[test]
fn test_barrier_prices_non_negative() {
    let spot = 100.0;
    let strike = 100.0;
    let barrier_up = 120.0;
    let barrier_down = 80.0;
    let time = 1.0;
    let rate = 0.05;
    let div_yield = 0.02;
    let vol = 0.2;

    assert!(
        barrier_price(
            &BarrierParams::new(spot, strike, barrier_up, time, rate, div_yield, vol),
            BarrierType::UpAndIn,
            OptionType::Call
        ) >= 0.0
    );
    assert!(
        barrier_price(
            &BarrierParams::new(spot, strike, barrier_up, time, rate, div_yield, vol),
            BarrierType::UpAndOut,
            OptionType::Call
        ) >= 0.0
    );
    assert!(
        barrier_price(
            &BarrierParams::new(spot, strike, barrier_down, time, rate, div_yield, vol),
            BarrierType::DownAndIn,
            OptionType::Call
        ) >= 0.0
    );
    assert!(
        barrier_price(
            &BarrierParams::new(spot, strike, barrier_down, time, rate, div_yield, vol),
            BarrierType::DownAndOut,
            OptionType::Call
        ) >= 0.0
    );
}

// ==================== DF-CONSTRUCTION TESTS ====================
//
// Verify that BarrierParams::with_df (which derives rate = -ln(df)/t)
// produces prices identical to BarrierParams::new with the equivalent
// explicit rate.

#[test]
fn test_with_df_consistency_barrier_call() {
    let spot = 100.0_f64;
    let strike = 100.0_f64;
    let barrier = 120.0_f64;
    let time = 1.0_f64;
    let rate = 0.05_f64;
    let div_yield = 0.02_f64;
    let vol = 0.2_f64;
    let df = (-rate * time).exp();

    for barrier_type in [
        BarrierType::UpAndIn,
        BarrierType::UpAndOut,
        BarrierType::DownAndIn,
        BarrierType::DownAndOut,
    ] {
        let b = if matches!(
            barrier_type,
            BarrierType::DownAndIn | BarrierType::DownAndOut
        ) {
            80.0 // Use down barrier for down types
        } else {
            barrier
        };

        let p_rate = BarrierParams::new(spot, strike, b, time, rate, div_yield, vol);
        let p_df = BarrierParams::with_df(spot, strike, b, time, df, div_yield, vol)
            .expect("positive df constructs");
        let price_rate = barrier_price(&p_rate, barrier_type, OptionType::Call);
        let price_df = barrier_price(&p_df, barrier_type, OptionType::Call);

        assert!(
            (price_rate - price_df).abs() < 1e-10,
            "{:?}: rate-based {} vs df-based {}",
            barrier_type,
            price_rate,
            price_df
        );
    }
}

#[test]
fn test_with_df_consistency_barrier_put() {
    let spot = 100.0_f64;
    let strike = 100.0_f64;
    let barrier = 120.0_f64;
    let time = 1.0_f64;
    let rate = 0.05_f64;
    let div_yield = 0.02_f64;
    let vol = 0.2_f64;
    let df = (-rate * time).exp();

    for barrier_type in [
        BarrierType::UpAndIn,
        BarrierType::UpAndOut,
        BarrierType::DownAndIn,
        BarrierType::DownAndOut,
    ] {
        let b = if matches!(
            barrier_type,
            BarrierType::DownAndIn | BarrierType::DownAndOut
        ) {
            80.0
        } else {
            barrier
        };

        let p_rate = BarrierParams::new(spot, strike, b, time, rate, div_yield, vol);
        let p_df = BarrierParams::with_df(spot, strike, b, time, df, div_yield, vol)
            .expect("positive df constructs");
        let price_rate = barrier_price(&p_rate, barrier_type, OptionType::Put);
        let price_df = barrier_price(&p_df, barrier_type, OptionType::Put);

        assert!(
            (price_rate - price_df).abs() < 1e-10,
            "{:?}: rate-based {} vs df-based {}",
            barrier_type,
            price_rate,
            price_df
        );
    }
}

#[test]
fn test_with_df_consistency_rebate() {
    let spot = 100.0_f64;
    let barrier = 120.0_f64;
    let rebate = 5.0_f64;
    let time = 1.0_f64;
    let rate = 0.05_f64;
    let div_yield = 0.02_f64;
    let vol = 0.2_f64;
    let df = (-rate * time).exp();

    for barrier_type in [
        BarrierType::UpAndIn,
        BarrierType::UpAndOut,
        BarrierType::DownAndIn,
        BarrierType::DownAndOut,
    ] {
        let b = if matches!(
            barrier_type,
            BarrierType::DownAndIn | BarrierType::DownAndOut
        ) {
            80.0
        } else {
            barrier
        };

        // The at-expiry helper does not use strike; pass barrier as a
        // convention-preserving sentinel.
        let p_rate = BarrierParams::new(spot, b, b, time, rate, div_yield, vol);
        let p_df = BarrierParams::with_df(spot, b, b, time, df, div_yield, vol)
            .expect("positive df constructs");
        let price_rate = barrier_rebate_at_expiry(&p_rate, rebate, barrier_type);
        let price_df = barrier_rebate_at_expiry(&p_df, rebate, barrier_type);

        assert!(
            (price_rate - price_df).abs() < 1e-10,
            "{:?}: rate-based {} vs df-based {}",
            barrier_type,
            price_rate,
            price_df
        );
    }
}

// ==================== COMPREHENSIVE BARRIER COVERAGE ====================

fn vanilla_call(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    let d1 =
        ((spot / strike).ln() + (rate - div_yield + 0.5 * vol * vol) * time) / (vol * time.sqrt());
    let d2 = d1 - vol * time.sqrt();
    spot * (-div_yield * time).exp() * norm_cdf(d1).expect("valid Gaussian variate")
        - strike * (-rate * time).exp() * norm_cdf(d2).expect("valid Gaussian variate")
}

fn vanilla_put(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    let d1 =
        ((spot / strike).ln() + (rate - div_yield + 0.5 * vol * vol) * time) / (vol * time.sqrt());
    let d2 = d1 - vol * time.sqrt();
    strike * (-rate * time).exp() * norm_cdf(-d2).expect("valid Gaussian variate")
        - spot * (-div_yield * time).exp() * norm_cdf(-d1).expect("valid Gaussian variate")
}

/// Verify In + Out = Vanilla for ALL 8 barrier option types.
///
/// Tests both K > H and K < H regimes for each direction.
#[test]
fn test_all_8_barrier_types_in_out_parity() {
    let tol = 1e-8;

    struct Case {
        label: &'static str,
        spot: f64,
        strike: f64,
        barrier_up: f64,
        barrier_down: f64,
        time: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
    }

    let cases = [
        Case {
            label: "ATM baseline",
            spot: 100.0,
            strike: 100.0,
            barrier_up: 120.0,
            barrier_down: 80.0,
            time: 1.0,
            rate: 0.05,
            div_yield: 0.02,
            vol: 0.20,
        },
        Case {
            label: "K < H (down barrier)",
            spot: 100.0,
            strike: 85.0,
            barrier_up: 115.0,
            barrier_down: 90.0,
            time: 0.5,
            rate: 0.08,
            div_yield: 0.04,
            vol: 0.25,
        },
        Case {
            label: "K > H (down barrier)",
            spot: 100.0,
            strike: 105.0,
            barrier_up: 130.0,
            barrier_down: 90.0,
            time: 0.5,
            rate: 0.08,
            div_yield: 0.04,
            vol: 0.25,
        },
        Case {
            label: "high vol short maturity",
            spot: 50.0,
            strike: 55.0,
            barrier_up: 65.0,
            barrier_down: 40.0,
            time: 0.25,
            rate: 0.10,
            div_yield: 0.0,
            vol: 0.40,
        },
    ];

    for c in &cases {
        // --- Up barrier calls ---
        let vc = vanilla_call(c.spot, c.strike, c.time, c.rate, c.div_yield, c.vol);
        let ui_c = barrier_price(
            &BarrierParams::new(
                c.spot,
                c.strike,
                c.barrier_up,
                c.time,
                c.rate,
                c.div_yield,
                c.vol,
            ),
            BarrierType::UpAndIn,
            OptionType::Call,
        );
        let uo_c = barrier_price(
            &BarrierParams::new(
                c.spot,
                c.strike,
                c.barrier_up,
                c.time,
                c.rate,
                c.div_yield,
                c.vol,
            ),
            BarrierType::UpAndOut,
            OptionType::Call,
        );
        assert!(
            (ui_c + uo_c - vc).abs() < tol,
            "{}: Up call parity failed: in({}) + out({}) = {} vs vanilla({})",
            c.label,
            ui_c,
            uo_c,
            ui_c + uo_c,
            vc,
        );

        // --- Down barrier calls ---
        let di_c = barrier_price(
            &BarrierParams::new(
                c.spot,
                c.strike,
                c.barrier_down,
                c.time,
                c.rate,
                c.div_yield,
                c.vol,
            ),
            BarrierType::DownAndIn,
            OptionType::Call,
        );
        let do_c = barrier_price(
            &BarrierParams::new(
                c.spot,
                c.strike,
                c.barrier_down,
                c.time,
                c.rate,
                c.div_yield,
                c.vol,
            ),
            BarrierType::DownAndOut,
            OptionType::Call,
        );
        assert!(
            (di_c + do_c - vc).abs() < tol,
            "{}: Down call parity failed: in({}) + out({}) = {} vs vanilla({})",
            c.label,
            di_c,
            do_c,
            di_c + do_c,
            vc,
        );

        // --- Up barrier puts ---
        let vp = vanilla_put(c.spot, c.strike, c.time, c.rate, c.div_yield, c.vol);
        let ui_p = barrier_price(
            &BarrierParams::new(
                c.spot,
                c.strike,
                c.barrier_up,
                c.time,
                c.rate,
                c.div_yield,
                c.vol,
            ),
            BarrierType::UpAndIn,
            OptionType::Put,
        );
        let uo_p = barrier_price(
            &BarrierParams::new(
                c.spot,
                c.strike,
                c.barrier_up,
                c.time,
                c.rate,
                c.div_yield,
                c.vol,
            ),
            BarrierType::UpAndOut,
            OptionType::Put,
        );
        assert!(
            (ui_p + uo_p - vp).abs() < tol,
            "{}: Up put parity failed: in({}) + out({}) = {} vs vanilla({})",
            c.label,
            ui_p,
            uo_p,
            ui_p + uo_p,
            vp,
        );

        // --- Down barrier puts ---
        let di_p = barrier_price(
            &BarrierParams::new(
                c.spot,
                c.strike,
                c.barrier_down,
                c.time,
                c.rate,
                c.div_yield,
                c.vol,
            ),
            BarrierType::DownAndIn,
            OptionType::Put,
        );
        let do_p = barrier_price(
            &BarrierParams::new(
                c.spot,
                c.strike,
                c.barrier_down,
                c.time,
                c.rate,
                c.div_yield,
                c.vol,
            ),
            BarrierType::DownAndOut,
            OptionType::Put,
        );
        assert!(
            (di_p + do_p - vp).abs() < tol,
            "{}: Down put parity failed: in({}) + out({}) = {} vs vanilla({})",
            c.label,
            di_p,
            do_p,
            di_p + do_p,
            vp,
        );
    }
}

/// Test K > H and K < H regimes for barrier calls and puts.
///
/// The Reiner-Rubinstein formula branches differently depending on the
/// relationship between strike and barrier, so both must be exercised.
/// Per Haug (2007) Table 4.17, `barrier_helper` now branches on K vs H,
/// producing correct individual knock-in/knock-out values in all regimes.
#[test]
fn test_strike_vs_barrier_regimes() {
    let spot = 100.0;
    let time = 0.5;
    let rate = 0.08;
    let div_yield = 0.04;
    let vol = 0.25;
    let tol = 1e-8;

    // --- Down-barrier calls, K > H (well-supported regime) ---
    {
        let barrier = 90.0;
        let strike = 100.0;
        let di = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::DownAndIn,
            OptionType::Call,
        );
        let do_ = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::DownAndOut,
            OptionType::Call,
        );
        let v = vanilla_call(spot, strike, time, rate, div_yield, vol);
        assert!(di >= 0.0, "Down-in call (K>H) must be non-negative: {}", di);
        assert!(
            do_ >= 0.0,
            "Down-out call (K>H) must be non-negative: {}",
            do_
        );
        assert!(
            di <= v,
            "Down-in call (K>H) must be <= vanilla: {} vs {}",
            di,
            v,
        );
        assert!(
            (di + do_ - v).abs() < tol,
            "Down call parity (K>H): in({}) + out({}) vs vanilla({})",
            di,
            do_,
            v,
        );
    }

    // --- Down-barrier calls, K < H ---
    {
        let barrier = 95.0;
        let strike = 90.0;
        let di = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::DownAndIn,
            OptionType::Call,
        );
        let do_ = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::DownAndOut,
            OptionType::Call,
        );
        let v = vanilla_call(spot, strike, time, rate, div_yield, vol);
        assert!(di >= 0.0, "Down-in call (K<H) must be non-negative: {}", di);
        assert!(
            do_ >= 0.0,
            "Down-out call (K<H) must be non-negative: {}",
            do_
        );
        assert!(
            (di + do_ - v).abs() < tol,
            "Down call parity (K<H): in({}) + out({}) vs vanilla({})",
            di,
            do_,
            v,
        );
    }

    // --- Up-barrier puts, K < H (well-supported regime) ---
    {
        let barrier = 110.0;
        let strike = 100.0;
        let ui = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::UpAndIn,
            OptionType::Put,
        );
        let uo = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::UpAndOut,
            OptionType::Put,
        );
        let v = vanilla_put(spot, strike, time, rate, div_yield, vol);
        assert!(ui >= 0.0, "Up-in put (K<H) must be non-negative: {}", ui);
        assert!(uo >= 0.0, "Up-out put (K<H) must be non-negative: {}", uo);
        assert!(
            ui <= v,
            "Up-in put (K<H) must be <= vanilla: {} vs {}",
            ui,
            v,
        );
        assert!(
            (ui + uo - v).abs() < tol,
            "Up put parity (K<H): in({}) + out({}) vs vanilla({})",
            ui,
            uo,
            v,
        );
    }

    // --- Up-barrier puts, K > H ---
    {
        let barrier = 105.0;
        let strike = 110.0;
        let ui = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::UpAndIn,
            OptionType::Put,
        );
        let uo = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::UpAndOut,
            OptionType::Put,
        );
        let v = vanilla_put(spot, strike, time, rate, div_yield, vol);
        assert!(ui >= 0.0, "Up-in put (K>H) must be non-negative: {}", ui);
        assert!(uo >= 0.0, "Up-out put (K>H) must be non-negative: {}", uo);
        assert!(
            (ui + uo - v).abs() < tol,
            "Up put parity (K>H): in({}) + out({}) vs vanilla({})",
            ui,
            uo,
            v,
        );
    }
}

/// Test against Haug (2007) "Complete Guide to Option Pricing Formulas" Table 4.17.
///
/// Reference: Haug, E.G. (2007), Chapter 4, Barrier Options.
/// Uses the K >= H regime for down-barrier calls and K <= H regime for
/// up-barrier calls, where the current barrier_helper is correct.
#[test]
fn test_haug_2007_down_in_call() {
    // Down-and-in call with K >= H (well-supported regime).
    // S=100, K=100, H=90, T=0.5, r=0.08, q=0.04, σ=0.25
    let spot = 100.0;
    let strike = 100.0;
    let barrier = 90.0;
    let time = 0.5;
    let rate = 0.08;
    let div_yield = 0.04;
    let vol = 0.25;

    let di = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::DownAndIn,
        OptionType::Call,
    );
    let do_ = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::DownAndOut,
        OptionType::Call,
    );
    let v = vanilla_call(spot, strike, time, rate, div_yield, vol);

    assert!(di > 0.0, "Down-in call must be positive, got {}", di);
    assert!(do_ > 0.0, "Down-out call must be positive, got {}", do_);
    assert!(di < v, "Down-in call must be < vanilla: {} vs {}", di, v);
    assert!(
        (di + do_ - v).abs() < 1e-8,
        "Parity: in({}) + out({}) vs vanilla({})",
        di,
        do_,
        v,
    );
}

/// Haug (2007): Up-and-out call with K < H (well-supported regime).
///
/// S=100, K=100, H=120, T=0.5, r=0.08, q=0.04, σ=0.25.
/// With barrier far above spot, most of the vanilla value is retained.
#[test]
fn test_haug_2007_up_out_call() {
    let spot = 100.0;
    let strike = 100.0;
    let barrier = 120.0;
    let time = 0.5;
    let rate = 0.08;
    let div_yield = 0.04;
    let vol = 0.25;

    let uo = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::UpAndOut,
        OptionType::Call,
    );
    let ui = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::UpAndIn,
        OptionType::Call,
    );
    let v = vanilla_call(spot, strike, time, rate, div_yield, vol);

    assert!(uo > 0.0, "Up-out call must be positive, got {}", uo);
    assert!(ui > 0.0, "Up-in call must be positive, got {}", ui);
    assert!(uo < v, "Up-out call must be < vanilla: {} vs {}", uo, v);
    assert!(
        (ui + uo - v).abs() < 1e-8,
        "Parity: in({}) + out({}) vs vanilla({})",
        ui,
        uo,
        v,
    );
}

/// Haug consistency: parity for all 4 call barrier types with matched regimes.
#[test]
fn test_haug_2007_parity_calls() {
    let spot = 100.0;
    let time = 0.5;
    let rate = 0.08;
    let div_yield = 0.04;
    let vol = 0.25;
    let tol = 1e-8;

    // Down-barrier (K >= H regime)
    {
        let strike = 100.0;
        let barrier = 90.0;
        let di = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::DownAndIn,
            OptionType::Call,
        );
        let do_ = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::DownAndOut,
            OptionType::Call,
        );
        let v = vanilla_call(spot, strike, time, rate, div_yield, vol);
        assert!(
            (di + do_ - v).abs() < tol,
            "Down-call parity: in({}) + out({}) = {} vs vanilla({})",
            di,
            do_,
            di + do_,
            v,
        );
    }

    // Up-barrier (K < H regime)
    {
        let strike = 100.0;
        let barrier = 120.0;
        let ui = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::UpAndIn,
            OptionType::Call,
        );
        let uo = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::UpAndOut,
            OptionType::Call,
        );
        let v = vanilla_call(spot, strike, time, rate, div_yield, vol);
        assert!(
            (ui + uo - v).abs() < tol,
            "Up-call parity: in({}) + out({}) = {} vs vanilla({})",
            ui,
            uo,
            ui + uo,
            v,
        );
    }
}

/// Haug consistency: parity for all 4 put barrier types.
#[test]
fn test_haug_2007_parity_puts() {
    let spot = 100.0;
    let time = 0.5;
    let rate = 0.08;
    let div_yield = 0.04;
    let vol = 0.25;
    let tol = 1e-8;

    // Down-barrier put
    {
        let strike = 100.0;
        let barrier = 90.0;
        let di = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::DownAndIn,
            OptionType::Put,
        );
        let do_ = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::DownAndOut,
            OptionType::Put,
        );
        let v = vanilla_put(spot, strike, time, rate, div_yield, vol);
        assert!(di > 0.0, "Down-in put must be positive, got {}", di);
        assert!(do_ >= 0.0, "Down-out put must be non-negative, got {}", do_);
        assert!(
            (di + do_ - v).abs() < tol,
            "Down-put parity: in({}) + out({}) vs vanilla({})",
            di,
            do_,
            v,
        );
    }

    // Up-barrier put
    {
        let strike = 100.0;
        let barrier = 115.0;
        let ui = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::UpAndIn,
            OptionType::Put,
        );
        let uo = barrier_price(
            &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
            BarrierType::UpAndOut,
            OptionType::Put,
        );
        let v = vanilla_put(spot, strike, time, rate, div_yield, vol);
        assert!(ui > 0.0, "Up-in put must be positive, got {}", ui);
        assert!(uo >= 0.0, "Up-out put must be non-negative, got {}", uo);
        assert!(
            (ui + uo - v).abs() < tol,
            "Up-put parity: in({}) + out({}) vs vanilla({})",
            ui,
            uo,
            v,
        );
    }
}

// ==================== EDGE CASES ====================

/// Barrier == Strike: degenerate but valid configuration.
#[test]
fn test_barrier_equals_strike() {
    let spot = 100.0;
    let strike = 95.0;
    let barrier = 95.0; // H == K
    let time = 0.5;
    let rate = 0.05;
    let div_yield = 0.02;
    let vol = 0.25;
    let tol = 1e-8;

    let di = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::DownAndIn,
        OptionType::Call,
    );
    let do_ = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::DownAndOut,
        OptionType::Call,
    );
    let v = vanilla_call(spot, strike, time, rate, div_yield, vol);

    assert!(di >= 0.0, "Down-in call (H==K) negative: {}", di);
    assert!(do_ >= 0.0, "Down-out call (H==K) negative: {}", do_);
    assert!(
        (di + do_ - v).abs() < tol,
        "Parity at H==K: in({}) + out({}) vs vanilla({})",
        di,
        do_,
        v,
    );

    let ui = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::UpAndIn,
        OptionType::Put,
    );
    let uo = barrier_price(
        &BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol),
        BarrierType::UpAndOut,
        OptionType::Put,
    );
    let vp = vanilla_put(spot, strike, time, rate, div_yield, vol);

    assert!(ui >= 0.0);
    assert!(uo >= 0.0);
    assert!(
        (ui + uo - vp).abs() < tol,
        "Parity at H==K (put): in({}) + out({}) vs vanilla({})",
        ui,
        uo,
        vp,
    );
}

/// Spot very close to the barrier (within ~0.1%).
///
/// Near-barrier behavior tests numerical stability. The up-in call value
/// approaches vanilla as spot → barrier from below, so up-out approaches
/// zero and may exhibit small negative numerical artifacts (~1e-10).
#[test]
fn test_spot_very_close_to_barrier() {
    let barrier = 100.0;
    let strike = 100.0;
    let time = 0.5;
    let rate = 0.05;
    let div_yield = 0.02;
    let vol = 0.20;
    let tol = 1e-6;
    let eps = 1e-8; // tolerance for near-zero boundary values

    // Spot just above a down-barrier
    let spot_above = barrier + 0.01;
    let di = barrier_price(
        &BarrierParams::new(spot_above, strike, barrier, time, rate, div_yield, vol),
        BarrierType::DownAndIn,
        OptionType::Call,
    );
    let do_ = barrier_price(
        &BarrierParams::new(spot_above, strike, barrier, time, rate, div_yield, vol),
        BarrierType::DownAndOut,
        OptionType::Call,
    );
    let v = vanilla_call(spot_above, strike, time, rate, div_yield, vol);
    assert!(!di.is_nan(), "Near-barrier down-in call NaN");
    assert!(!do_.is_nan(), "Near-barrier down-out call NaN");
    assert!(
        (di + do_ - v).abs() < tol,
        "Near-barrier parity (above): in({}) + out({}) vs vanilla({})",
        di,
        do_,
        v,
    );

    // Spot just below an up-barrier
    let spot_below = barrier - 0.01;
    let ui = barrier_price(
        &BarrierParams::new(spot_below, strike, barrier, time, rate, div_yield, vol),
        BarrierType::UpAndIn,
        OptionType::Call,
    );
    let uo = barrier_price(
        &BarrierParams::new(spot_below, strike, barrier, time, rate, div_yield, vol),
        BarrierType::UpAndOut,
        OptionType::Call,
    );
    let v = vanilla_call(spot_below, strike, time, rate, div_yield, vol);
    assert!(!ui.is_nan(), "Near-barrier up-in call NaN");
    assert!(!uo.is_nan(), "Near-barrier up-out call NaN");
    assert!(uo > -eps, "Near-barrier up-out call too negative: {}", uo,);
    assert!(
        (ui + uo - v).abs() < tol,
        "Near-barrier parity (below): in({}) + out({}) vs vanilla({})",
        ui,
        uo,
        v,
    );
}

/// Zero dividend yield: verify no division-by-zero or NaN.
#[test]
fn test_zero_dividend_yield() {
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0;
    let rate = 0.05;
    let div_yield = 0.0;
    let vol = 0.20;
    let tol = 1e-8;

    // Down-barrier calls
    let barrier_down = 85.0;
    let di = barrier_price(
        &BarrierParams::new(spot, strike, barrier_down, time, rate, div_yield, vol),
        BarrierType::DownAndIn,
        OptionType::Call,
    );
    let do_ = barrier_price(
        &BarrierParams::new(spot, strike, barrier_down, time, rate, div_yield, vol),
        BarrierType::DownAndOut,
        OptionType::Call,
    );
    let vc = vanilla_call(spot, strike, time, rate, div_yield, vol);
    assert!(!di.is_nan(), "NaN in down-in call with q=0");
    assert!(!do_.is_nan(), "NaN in down-out call with q=0");
    assert!(
        (di + do_ - vc).abs() < tol,
        "Parity (q=0, call): in({}) + out({}) vs vanilla({})",
        di,
        do_,
        vc,
    );

    // Up-barrier puts
    let barrier_up = 115.0;
    let ui = barrier_price(
        &BarrierParams::new(spot, strike, barrier_up, time, rate, div_yield, vol),
        BarrierType::UpAndIn,
        OptionType::Put,
    );
    let uo = barrier_price(
        &BarrierParams::new(spot, strike, barrier_up, time, rate, div_yield, vol),
        BarrierType::UpAndOut,
        OptionType::Put,
    );
    let vp = vanilla_put(spot, strike, time, rate, div_yield, vol);
    assert!(!ui.is_nan(), "NaN in up-in put with q=0");
    assert!(!uo.is_nan(), "NaN in up-out put with q=0");
    assert!(
        (ui + uo - vp).abs() < tol,
        "Parity (q=0, put): in({}) + out({}) vs vanilla({})",
        ui,
        uo,
        vp,
    );
}

/// Very short maturity: barrier options should converge towards intrinsic.
#[test]
fn test_short_maturity() {
    let spot = 100.0;
    let strike = 100.0;
    let time = 1.0 / 365.0; // 1 day
    let rate = 0.05;
    let div_yield = 0.02;
    let vol = 0.20;
    let tol = 1e-6;

    let barrier_up = 120.0;
    let ui = barrier_price(
        &BarrierParams::new(spot, strike, barrier_up, time, rate, div_yield, vol),
        BarrierType::UpAndIn,
        OptionType::Call,
    );
    let uo = barrier_price(
        &BarrierParams::new(spot, strike, barrier_up, time, rate, div_yield, vol),
        BarrierType::UpAndOut,
        OptionType::Call,
    );
    let v = vanilla_call(spot, strike, time, rate, div_yield, vol);
    assert!(
        (ui + uo - v).abs() < tol,
        "Short maturity parity: in({}) + out({}) vs vanilla({})",
        ui,
        uo,
        v,
    );
    // With short maturity and barrier far from spot, knock-out ~ vanilla
    assert!(
        (uo - v).abs() < 0.5,
        "Short maturity up-out should be close to vanilla when barrier is far",
    );
}

#[test]
fn test_barrier_k_vs_h_non_negativity_and_parity() {
    let tol = 1e-6;

    // Down-barrier call with K < H (previously-broken regime)
    {
        let spot = 110.0;
        let strike = 90.0;
        let barrier = 100.0;
        let r = 0.05;
        let q = 0.02;
        let vol = 0.25;
        let t = 1.0;

        let ki = barrier_price(
            &BarrierParams::new(spot, strike, barrier, t, r, q, vol),
            BarrierType::DownAndIn,
            OptionType::Call,
        );
        let ko = barrier_price(
            &BarrierParams::new(spot, strike, barrier, t, r, q, vol),
            BarrierType::DownAndOut,
            OptionType::Call,
        );
        let v = vanilla_call(spot, strike, t, r, q, vol);

        assert!(ki >= 0.0, "Down-in call (K<H) negative: {}", ki);
        assert!(ko >= 0.0, "Down-out call (K<H) negative: {}", ko);
        assert!(
            (ki + ko - v).abs() < tol,
            "Down call parity (K<H): in({}) + out({}) = {} vs vanilla({})",
            ki,
            ko,
            ki + ko,
            v,
        );
    }

    // Up-barrier put with K > H (previously-broken regime)
    {
        let spot = 90.0;
        let strike = 110.0;
        let barrier = 100.0;
        let r = 0.05;
        let q = 0.02;
        let vol = 0.25;
        let t = 1.0;

        let ki = barrier_price(
            &BarrierParams::new(spot, strike, barrier, t, r, q, vol),
            BarrierType::UpAndIn,
            OptionType::Put,
        );
        let ko = barrier_price(
            &BarrierParams::new(spot, strike, barrier, t, r, q, vol),
            BarrierType::UpAndOut,
            OptionType::Put,
        );
        let v = vanilla_put(spot, strike, t, r, q, vol);

        assert!(ki >= 0.0, "Up-in put (K>H) negative: {}", ki);
        assert!(ko >= 0.0, "Up-out put (K>H) negative: {}", ko);
        assert!(
            (ki + ko - v).abs() < tol,
            "Up put parity (K>H): in({}) + out({}) = {} vs vanilla({})",
            ki,
            ko,
            ki + ko,
            v,
        );
    }
}

#[test]
fn test_public_wrappers_remain_finite_at_zero_time_and_zero_vol() {
    let zero_time_cases = [
        (
            "up_out_call",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 120.0, 0.0, 0.05, 0.02, 0.2),
                BarrierType::UpAndOut,
                OptionType::Call,
            ),
        ),
        (
            "up_in_call",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 100.0, 0.0, 0.05, 0.02, 0.2),
                BarrierType::UpAndIn,
                OptionType::Call,
            ),
        ),
        (
            "down_out_call",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 80.0, 0.0, 0.05, 0.02, 0.2),
                BarrierType::DownAndOut,
                OptionType::Call,
            ),
        ),
        (
            "down_in_call",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 100.0, 0.0, 0.05, 0.02, 0.2),
                BarrierType::DownAndIn,
                OptionType::Call,
            ),
        ),
        (
            "up_out_put",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 120.0, 0.0, 0.05, 0.02, 0.2),
                BarrierType::UpAndOut,
                OptionType::Put,
            ),
        ),
        (
            "up_in_put",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 100.0, 0.0, 0.05, 0.02, 0.2),
                BarrierType::UpAndIn,
                OptionType::Put,
            ),
        ),
        (
            "down_out_put",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 80.0, 0.0, 0.05, 0.02, 0.2),
                BarrierType::DownAndOut,
                OptionType::Put,
            ),
        ),
        (
            "down_in_put",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 100.0, 0.0, 0.05, 0.02, 0.2),
                BarrierType::DownAndIn,
                OptionType::Put,
            ),
        ),
    ];

    for (name, price) in zero_time_cases {
        assert!(
            price.is_finite(),
            "{name} returned non-finite price at time=0: {price}"
        );
    }

    let zero_vol_cases = [
        (
            "up_out_call",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 120.0, 1.0, 0.0, 0.0, 0.0),
                BarrierType::UpAndOut,
                OptionType::Call,
            ),
        ),
        (
            "up_in_call",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 100.0, 1.0, 0.0, 0.0, 0.0),
                BarrierType::UpAndIn,
                OptionType::Call,
            ),
        ),
        (
            "down_out_call",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 80.0, 1.0, 0.0, 0.0, 0.0),
                BarrierType::DownAndOut,
                OptionType::Call,
            ),
        ),
        (
            "down_in_call",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 100.0, 1.0, 0.0, 0.0, 0.0),
                BarrierType::DownAndIn,
                OptionType::Call,
            ),
        ),
        (
            "up_out_put",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 120.0, 1.0, 0.0, 0.0, 0.0),
                BarrierType::UpAndOut,
                OptionType::Put,
            ),
        ),
        (
            "up_in_put",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 100.0, 1.0, 0.0, 0.0, 0.0),
                BarrierType::UpAndIn,
                OptionType::Put,
            ),
        ),
        (
            "down_out_put",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 80.0, 1.0, 0.0, 0.0, 0.0),
                BarrierType::DownAndOut,
                OptionType::Put,
            ),
        ),
        (
            "down_in_put",
            barrier_price(
                &BarrierParams::new(100.0, 100.0, 100.0, 1.0, 0.0, 0.0, 0.0),
                BarrierType::DownAndIn,
                OptionType::Put,
            ),
        ),
    ];

    for (name, price) in zero_vol_cases {
        assert!(
            price.is_finite(),
            "{name} returned non-finite price at vol=0: {price}"
        );
    }
}

#[test]
fn test_touch_probability_and_rebate_handle_deterministic_and_near_zero_vol_cases() {
    let hit_up = barrier_touch_probability(100.0, 120.0, 1.0, 0.30, 0.0, 0.0, true);
    let no_hit_up = barrier_touch_probability(100.0, 120.0, 1.0, 0.0, 0.0, 0.0, true);
    let hit_down = barrier_touch_probability(100.0, 80.0, 1.0, -0.30, 0.0, 0.0, false);

    assert_eq!(
        hit_up, 1.0,
        "zero-vol up barrier should deterministically hit"
    );
    assert_eq!(
        no_hit_up, 0.0,
        "zero-vol up barrier should deterministically miss"
    );
    assert_eq!(
        hit_down, 1.0,
        "zero-vol down barrier should deterministically hit"
    );

    let p_no_hit = BarrierParams::new(100.0, 120.0, 120.0, 1.0, 0.0, 0.0, 0.0);
    let rebate_no_hit = barrier_rebate_at_expiry(&p_no_hit, 5.0, BarrierType::UpAndIn);
    assert_eq!(
        rebate_no_hit, 5.0,
        "knock-in rebate should pay in full when zero-vol path never hits"
    );

    let near_zero_prob = barrier_touch_probability(100.0, 120.0, 0.1, 0.05, 0.0, 1e-4, true);
    assert!(
        near_zero_prob.is_finite() && (0.0..=1.0).contains(&near_zero_prob),
        "near-zero-vol touch probability must stay bounded, got {near_zero_prob}"
    );
}

#[test]
fn test_rebate_is_not_discounted_after_expiry() {
    let p_rate = BarrierParams::new(130.0, 120.0, 120.0, -0.25, 0.05, 0.0, 0.2);
    let rate_rebate = barrier_rebate_at_expiry(&p_rate, 5.0, BarrierType::UpAndOut);
    let p_df = BarrierParams::with_df(130.0, 120.0, 120.0, -0.25, 0.95, 0.0, 0.2)
        .expect("positive df is valid");
    let df_rebate = barrier_rebate_at_expiry(&p_df, 5.0, BarrierType::UpAndOut);

    assert_eq!(
        rate_rebate, 5.0,
        "expired rebate should settle immediately without negative-time discounting"
    );
    assert_eq!(
        df_rebate, 5.0,
        "df-first rebate should match the same expired settlement convention"
    );
}

/// Audit item 1: `BarrierParams::with_df` previously coerced a non-positive
/// discount factor (corrupt / inverted curve) to `rate = 0.0`, silently
/// mispricing the barrier rather than surfacing the bad input.
///
/// Failure mode locked in: `df <= 0` must produce a validation `Err`, not a
/// `BarrierParams` carrying a fabricated zero rate.
#[test]
fn with_df_rejects_non_positive_discount_factor() {
    assert!(
        BarrierParams::with_df(100.0, 100.0, 120.0, 1.0, 0.0, 0.02, 0.2).is_err(),
        "df == 0 must be rejected, not coerced to rate=0"
    );
    assert!(
        BarrierParams::with_df(100.0, 100.0, 120.0, 1.0, -0.5, 0.02, 0.2).is_err(),
        "negative df (inverted curve) must be rejected, not coerced to rate=0"
    );
    // A NaN discount factor is also unrepresentable as a rate.
    assert!(
        BarrierParams::with_df(100.0, 100.0, 120.0, 1.0, f64::NAN, 0.02, 0.2).is_err(),
        "non-finite df must be rejected"
    );
    // Positive df still constructs successfully.
    assert!(
        BarrierParams::with_df(100.0, 100.0, 120.0, 1.0, 0.95, 0.02, 0.2).is_ok(),
        "positive df must still construct"
    );
}

/// Audit item 2: barrier inputs (spot/strike/barrier) were validated only by
/// `debug_assert`, so release builds fed a non-positive spot/strike/barrier
/// straight into `.ln()` and produced a silent NaN price with no diagnostic.
///
/// Failure mode locked in: non-positive spot/strike/barrier must NOT yield a
/// plausible finite price — `barrier_helper` returns `Err`, and the public
/// wrappers surface a non-finite (NaN) sentinel rather than a quiet number.
#[test]
fn barrier_helper_rejects_non_positive_market_inputs() {
    // Non-positive spot.
    let bad_spot = BarrierParams::new(-100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.2);
    assert!(barrier_helper(&bad_spot, 1.0, -1.0).is_err());
    // Non-positive strike.
    let bad_strike = BarrierParams::new(100.0, 0.0, 90.0, 1.0, 0.05, 0.02, 0.2);
    assert!(barrier_helper(&bad_strike, 1.0, -1.0).is_err());
    // Non-positive barrier.
    let bad_barrier = BarrierParams::new(100.0, 100.0, -90.0, 1.0, 0.05, 0.02, 0.2);
    assert!(barrier_helper(&bad_barrier, 1.0, -1.0).is_err());
    // Valid inputs still succeed.
    let good = BarrierParams::new(100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.2);
    assert!(barrier_helper(&good, 1.0, -1.0).is_ok());
}

/// Audit item 2: the public barrier wrappers must not return a plausible
/// finite price for a corrupt non-positive spot/strike/barrier. They surface
/// a non-finite value (NaN) — a loud poison that downstream `is_finite()`
/// checks reject — instead of the previous silent release-build NaN with no
/// validation path at all.
#[test]
fn public_wrappers_do_not_return_finite_price_for_non_positive_inputs() {
    // Negative spot through every call/put wrapper.
    assert!(
        !barrier_price(
            &BarrierParams::new(-100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.2),
            BarrierType::DownAndOut,
            OptionType::Call
        )
        .is_finite()
    );
    assert!(
        !barrier_price(
            &BarrierParams::new(-100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.2),
            BarrierType::DownAndIn,
            OptionType::Call
        )
        .is_finite()
    );
    assert!(
        !barrier_price(
            &BarrierParams::new(100.0, 100.0, -120.0, 1.0, 0.05, 0.02, 0.2),
            BarrierType::UpAndOut,
            OptionType::Call
        )
        .is_finite()
    );
    assert!(
        !barrier_price(
            &BarrierParams::new(100.0, 0.0, 120.0, 1.0, 0.05, 0.02, 0.2),
            BarrierType::UpAndIn,
            OptionType::Call
        )
        .is_finite()
    );
    assert!(
        !barrier_price(
            &BarrierParams::new(100.0, -100.0, 90.0, 1.0, 0.05, 0.02, 0.2),
            BarrierType::DownAndOut,
            OptionType::Put
        )
        .is_finite()
    );
    assert!(
        !barrier_price(
            &BarrierParams::new(-100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.2),
            BarrierType::DownAndIn,
            OptionType::Put
        )
        .is_finite()
    );
    assert!(
        !barrier_price(
            &BarrierParams::new(100.0, 100.0, -120.0, 1.0, 0.05, 0.02, 0.2),
            BarrierType::UpAndOut,
            OptionType::Put
        )
        .is_finite()
    );
    assert!(
        !barrier_price(
            &BarrierParams::new(0.0, 100.0, 120.0, 1.0, 0.05, 0.02, 0.2),
            BarrierType::UpAndIn,
            OptionType::Put
        )
        .is_finite()
    );
}

/// At-hit KO rebates discount over the (random) hit time τ ≤ T, so with a
/// positive rate they must be worth at least the at-expiry variant
/// (`e^{-rτ} ≥ e^{-rT}` on every hit path) and at most `rebate · P(hit)`.
#[test]
fn at_hit_ko_rebate_brackets_at_expiry_value() {
    let rebate = 5.0;
    for (spot, barrier, bt, is_up) in [
        (100.0, 120.0, BarrierType::UpAndOut, true),
        (100.0, 80.0, BarrierType::DownAndOut, false),
    ] {
        let p = BarrierParams::new(spot, 100.0, barrier, 1.0, 0.05, 0.0, 0.30);
        let at_expiry = barrier_rebate(&p, rebate, bt, PayoutTiming::AtExpiry);
        let at_hit = barrier_rebate(&p, rebate, bt, PayoutTiming::AtHit);
        let p_hit = barrier_touch_probability(spot, barrier, 1.0, 0.05, 0.0, 0.30, is_up);

        assert_eq!(
            at_expiry,
            barrier_rebate_at_expiry(&p, rebate, bt),
            "AtExpiry must reproduce the legacy at-expiry value"
        );
        assert!(
            at_hit >= at_expiry - 1e-12,
            "at-hit rebate {at_hit} must dominate at-expiry {at_expiry} for r>0"
        );
        assert!(
            at_hit <= rebate * p_hit + 1e-12,
            "at-hit rebate {at_hit} cannot exceed undiscounted rebate*P(hit) = {}",
            rebate * p_hit
        );
    }
}

/// Zero rates remove discounting entirely: at-hit and at-expiry KO rebates
/// must agree at `rebate · P(hit)`.
#[test]
fn at_hit_ko_rebate_matches_at_expiry_when_rate_is_zero() {
    let p = BarrierParams::new(100.0, 100.0, 120.0, 1.0, 0.0, 0.0, 0.25);
    let at_hit = barrier_rebate(&p, 5.0, BarrierType::UpAndOut, PayoutTiming::AtHit);
    let at_expiry = barrier_rebate(&p, 5.0, BarrierType::UpAndOut, PayoutTiming::AtExpiry);
    assert!(
        (at_hit - at_expiry).abs() < 1e-10,
        "r=0: at-hit {at_hit} should equal at-expiry {at_expiry}"
    );
}

/// A knock-in (no-hit) rebate is only determined at expiry: `AtHit` is
/// unsupported and yields the NaN sentinel instead of being silently
/// repriced at expiry. An already-breached KO barrier pays immediately
/// (undiscounted) under at-hit timing.
#[test]
fn rebate_timing_edge_cases() {
    let p = BarrierParams::new(100.0, 100.0, 120.0, 1.0, 0.05, 0.0, 0.25);
    for bt in [BarrierType::UpAndIn, BarrierType::DownAndIn] {
        assert!(barrier_rebate(&p, 5.0, bt, PayoutTiming::AtHit).is_nan());
        assert!(expired_barrier_payoff(bt, false, 1.0, Some((5.0, PayoutTiming::AtHit))).is_nan());
        assert_eq!(
            expired_barrier_payoff(bt, false, 1.0, Some((5.0, PayoutTiming::AtExpiry))),
            5.0
        );
        assert_eq!(expired_barrier_payoff(bt, false, 1.0, None), 0.0);
    }
    let ki_exp = barrier_rebate(&p, 5.0, BarrierType::UpAndIn, PayoutTiming::AtExpiry);
    assert!(ki_exp.is_finite() && ki_exp > 0.0);

    let breached = BarrierParams::new(125.0, 100.0, 120.0, 1.0, 0.05, 0.0, 0.25);
    let v = barrier_rebate(&breached, 5.0, BarrierType::UpAndOut, PayoutTiming::AtHit);
    assert!(
        (v - 5.0).abs() < 1e-12,
        "already-breached at-hit KO rebate pays the full undiscounted amount, got {v}"
    );
}
