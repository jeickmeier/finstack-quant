//! Quadrature-grid regression on the `cdx_ig_46_payer_atm_jun26` golden.
//!
//! Lives in the umbrella test suite: the market bootstrap goes through
//! `finstack-quant-calibration`, which the credit crate cannot dev-depend on,
//! and the golden fixture is checked in here. The quadrature helpers it
//! reaches are `#[doc(hidden)] pub` for this reason.

use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::bloomberg_quadrature::{
    calibrate_lognormal_mean, normal_integral, z_limit, ForwardCdsContext,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOption;
use finstack_quant_valuations::instruments::credit_derivatives::constants::bloomberg_cdso;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use super::common::cdx_ig_46_fixture;
use time::macros::date;

fn context_for(
    option: &CdsOption,
    market: &MarketContext,
    as_of: Date,
    sigma: f64,
) -> ForwardCdsContext {
    let cds = synthetic_underlying_cds(option, as_of).expect("synthetic cds");
    let discount = market
        .get_discount(&option.discount_curve_id)
        .expect("discount");
    let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
    ForwardCdsContext::build(
        option,
        discount.as_ref(),
        hazard.as_ref(),
        &cds,
        as_of,
        sigma,
    )
    .expect("forward cds context")
}

#[test]
fn cdx_ig_46_production_integrand_converges_at_quadrature_step() {
    let as_of = date!(2026 - 05 - 07);
    let (option, market) = cdx_ig_46_fixture();
    let sigma = finstack_quant_valuations::instruments::credit_derivatives::cds_option::pricer::resolve_sigma(&option, &market, as_of).expect("fixture volatility");
    let ctx = context_for(&option, &market, as_of, sigma);
    let m = calibrate_lognormal_mean(&ctx).expect("calibrate lognormal mean");
    let t_expiry = ctx.t_expiry.max(0.0);
    let s0 = (-0.5_f64 * ctx.sigma * ctx.sigma * t_expiry).exp();
    let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();
    let integrand = |z: f64| {
        let s = m * s0 * (sigma_sqrt_t * z).exp();
        ctx.swap_value_per_n(s)
    };

    let production = normal_integral(
        bloomberg_cdso::Z_STEP,
        z_limit(ctx.sigma, t_expiry),
        integrand,
    )
    .expect("valid integration grid");
    let fine = normal_integral(
        bloomberg_cdso::Z_STEP * 0.5,
        z_limit(ctx.sigma, t_expiry),
        integrand,
    )
    .expect("valid integration grid");
    let dollar_diff = (production - fine).abs() * option.notional.amount();

    assert!(
        dollar_diff < 0.01,
        "production quadrature grid should be sub-cent stable on V_te(s): diff=${dollar_diff:.8}",
    );
}

#[test]
fn cdx_ig_46_option_payoff_matches_threshold_split_integration() {
    let as_of = date!(2026 - 05 - 07);
    let (option, market) = cdx_ig_46_fixture();
    let sigma = finstack_quant_valuations::instruments::credit_derivatives::cds_option::pricer::resolve_sigma(&option, &market, as_of).expect("fixture volatility");
    let ctx = context_for(&option, &market, as_of, sigma);
    let m = calibrate_lognormal_mean(&ctx).expect("calibrate mean");
    let vol_time = sigma * ctx.t_expiry.sqrt();
    let strike = ctx.spread_strike().expect("spread strike");
    // With no realized losses, the payer exercises at S = K. Splitting
    // there gives a smooth reference integrand, independent of production's
    // adaptive treatment of the positive-part kink.
    let boundary = ((strike / m).ln() + 0.5 * vol_time * vol_time) / vol_time;
    let reference = finstack_quant_core::math::integration::adaptive_simpson(
        |z| {
            let spread = m * (-0.5 * vol_time * vol_time + vol_time * z).exp();
            Ok(
                (ctx.swap_value_per_n(spread) + ctx.signed_strike_adjustment_per_n())
                    * (-0.5 * z * z).exp()
                    / (2.0 * std::f64::consts::PI).sqrt(),
            )
        },
        boundary,
        z_limit(sigma, ctx.t_expiry),
        1e-12,
        20,
        100_000,
    )
    .expect("smooth reference integral")
        * ctx.df_to_settlement
        * option.notional.amount();
    let production = finstack_quant_valuations::instruments::credit_derivatives::cds_option::bloomberg_quadrature::price_with_calibrated_mean(&ctx, m, ctx.t_expiry)
        .expect("payoff integration")
        * option.notional.amount();
    assert!(
        (production - reference).abs() < 0.01,
        "payoff quadrature must be sub-cent stable: production={production}, reference={reference}"
    );
}

/// Bloomberg CDSO screen values for the fixture, with the fixture tolerances.
const SCREEN: [(&str, f64, f64); 6] = [
    ("npv", 118_781.76, 6.0),
    ("fwd_bp", 55.2848, 0.07),
    ("delta", 0.507, 0.001),
    ("gamma", 0.547, 0.005),
    ("vega", 3_411.78, 34.12),
    ("theta", -1_499.93, 55.0),
];

/// Diagnostic only: prices the fixture under each candidate CDSO convention
/// and scores the result against six screen values. Run with
/// `mise run rust-test-filter -- finstack-quant-valuations cdx_ig_46_convention_matrix --integration valuations --slow`.
#[test]
#[ignore = "diagnostic convention matrix; prints a table and asserts nothing"]
fn cdx_ig_46_convention_matrix() {
    use finstack_quant_valuations::instruments::credit_derivatives::cds_option::bloomberg_quadrature::quadrature_payoff;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::special_functions::norm_cdf;

    let as_of = date!(2026 - 05 - 07);
    let (option, market) = cdx_ig_46_fixture();
    let sigma = finstack_quant_valuations::instruments::credit_derivatives::cds_option::pricer::resolve_sigma(&option, &market, as_of).expect("fixture volatility");
    let notional = option.notional.amount();
    let discount = market
        .get_discount(&option.discount_curve_id)
        .expect("discount");
    let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
    let df_expiry = DiscountCurve::df_between_dates(discount.as_ref(), as_of, option.expiry)
        .expect("df to expiry");

    let price = |ctx: &ForwardCdsContext, extra_fep: bool, t: f64, m: f64| -> f64 {
        let fep = if extra_fep {
            ctx.sign() * ctx.front_end_protection
        } else {
            0.0
        };
        quadrature_payoff(
            ctx,
            m,
            ctx.signed_strike_adjustment_per_n(),
            ctx.signed_loss_settlement_per_n() + fep,
            t,
        )
        .expect("payoff")
            * notional
    };

    println!(
        "{:<34} {:>11} {:>8} {:>7} {:>7} {:>9} {:>10} {:>3}",
        "clock/discount/annuity/fep", "npv", "fwd_bp", "delta", "gamma", "vega", "theta", "ok"
    );
    println!(
        "{:<34} {:>11.2} {:>8.4} {:>7.4} {:>7.4} {:>9.2} {:>10.2}",
        "BLOOMBERG", SCREEN[0].1, SCREEN[1].1, SCREEN[2].1, SCREEN[3].1, SCREEN[4].1, SCREEN[5].1
    );
    for clock_days in [41.0_f64, 42.0] {
        for discount_to_expiry in [false, true] {
            for drop_first_coupon in [false, true] {
                for extra_fep in [false, true] {
                    let mut ctx = context_for(&option, &market, as_of, sigma);
                    ctx.t_expiry = clock_days / bloomberg_cdso::G_DAYS_IN_YEAR;
                    if discount_to_expiry {
                        ctx.df_to_settlement = df_expiry;
                    }
                    if drop_first_coupon {
                        // HELP CDSO: "subtract the PV01 of the first cashflow".
                        let pay_days =
                            (ctx.times_from_expiry[0] * bloomberg_cdso::G_DAYS_IN_YEAR).round();
                        let pay_date = option.expiry + time::Duration::days(pay_days as i64);
                        let sp_pay = hazard.sp_on_date(pay_date).expect("sp pay")
                            / hazard.sp_on_date(as_of).expect("sp as_of");
                        let annuity = ctx.bootstrapped_l_at_expiry
                            + ctx.accrual_pcd_to_expiry * ctx.survival_to_expiry
                            - ctx.accrual_factors[0] * ctx.fwd_discount_factors[0] * sp_pay;
                        ctx.forward_par_spread *= ctx.bootstrapped_l_at_expiry / annuity;
                        ctx.bootstrapped_l_at_expiry = annuity;
                        ctx.accrual_pcd_to_expiry = 0.0;
                        ctx.times_from_expiry.remove(0);
                        ctx.accrual_factors.remove(0);
                        ctx.fwd_discount_factors.remove(0);
                    }
                    let t = ctx.t_expiry;
                    let m = calibrate_lognormal_mean(&ctx).expect("calibrate");
                    let npv = price(&ctx, extra_fep, t, m);
                    let theta = price(
                        &ctx,
                        extra_fep,
                        t - 1.0 / bloomberg_cdso::THETA_DAYS_IN_YEAR,
                        m,
                    ) - npv;
                    ctx.sigma = sigma + 0.01;
                    let m_bumped = calibrate_lognormal_mean(&ctx).expect("calibrate bumped");
                    let vega = price(&ctx, extra_fep, t, m_bumped) - npv;
                    ctx.sigma = sigma;

                    let strike = ctx.spread_strike().expect("spread strike");
                    let vol_time = sigma * t.sqrt();
                    let black_delta = |forward: f64| {
                        norm_cdf(((forward / strike).ln() + 0.5 * vol_time * vol_time) / vol_time)
                            .expect("valid normal variate")
                    };
                    let forward = ctx.forward_par_spread;
                    let delta = black_delta(forward);
                    let gamma = black_delta(forward + 0.0005) - black_delta(forward - 0.0005);
                    let actual = [npv, forward * 1e4, delta, gamma, vega, theta];
                    let ok = SCREEN
                        .iter()
                        .zip(actual)
                        .filter(|((_, expected, tol), value)| (value - expected).abs() <= *tol)
                        .count();
                    println!(
                        "{:<34} {:>11.2} {:>8.4} {:>7.4} {:>7.4} {:>9.2} {:>10.2} {:>3}",
                        format!(
                            "{}d/{}/{}/{}",
                            clock_days,
                            if discount_to_expiry {
                                "expiry"
                            } else {
                                "settle"
                            },
                            if drop_first_coupon { "no-cpn1" } else { "stub" },
                            if extra_fep { "fep+" } else { "fep0" }
                        ),
                        npv,
                        actual[1],
                        delta,
                        gamma,
                        vega,
                        theta,
                        ok
                    );
                }
            }
        }
    }

    // Published forward definitions on the flat curve.
    let ctx = context_for(&option, &market, as_of, sigma);
    let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
    let spot_annuity = finstack_quant_valuations_core::instruments::Instrument::price_with_metrics(
        &cds,
        &market,
        as_of,
        &[finstack_quant_valuations_core::metrics::MetricId::RiskyPv01],
        finstack_quant_valuations_core::instruments::PricingOptions::default(),
    )
    .expect("spot annuity")
    .measures
    .get("risky_pv01")
    .copied()
    .expect("risky_pv01 metric")
        / cds.notional.amount()
        * 1e4;
    let forward_annuity = ctx.bootstrapped_l_at_expiry * df_expiry;
    println!(
        "spot clean annuity {spot_annuity:.6}, forward annuity (today) {forward_annuity:.6}, \
         Martin s0*A0/Af = {:.4} bp, default-adjusted forward (OpenGamma eq. 79) = {:.4} bp",
        53.6264 * spot_annuity / forward_annuity,
        ctx.forward_par_spread * 1e4
    );

    // At the displayed breakeven, compare clean exercise cash with the premium.
    // This diagnostic uses a screen output and cannot be used to price the option.
    let breakeven = 57.8824 / 1e4;
    let clean_payoff = ctx.swap_value_per_n(breakeven) + ctx.signed_strike_adjustment_per_n();
    let payoff_pv = notional * ctx.df_to_settlement * clean_payoff;
    let implied_vol = option
        .implied_vol(&market, as_of, SCREEN[0].1, None)
        .expect("invert screen premium");
    println!(
        "discounted payoff at screen breakeven {payoff_pv:.2}, model vol implied by screen premium {:.5}%",
        implied_vol * 100.0
    );

    // Sensitivity only: force the displayed spread into the existing cash
    // calibration anchor, holding every other model input fixed.
    let mut displayed = ctx;
    displayed.forward_par_spread = SCREEN[1].1 / 1e4;
    let mean = calibrate_lognormal_mean(&displayed).expect("calibrate displayed forward scenario");
    println!(
        "payer NPV with only the displayed forward substituted {:.2}",
        price(&displayed, false, displayed.t_expiry, mean)
    );
}
