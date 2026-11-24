use super::engine::*;
use crate::cds_option::CdsOption;
use crate::cds_option::pricer::synthetic_underlying_cds;
use crate::constants::bloomberg_cdso;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::ZERO_TOLERANCE;
use finstack_quant_core::math::special_functions::norm_pdf;
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::instruments::SettlementType;
use rust_decimal::Decimal;
use time::macros::date;

fn bp_to_decimal(bp: f64) -> Decimal {
    Decimal::try_from(bp / BASIS_POINTS_PER_UNIT).expect("valid decimal from bp")
}

fn flat_discount(id: &str, base: Date, rate: f64) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("flat discount curve")
}

fn flat_hazard(id: &str, base: Date, recovery: f64, hazard_rate: f64) -> HazardCurve {
    HazardCurve::builder(id)
        .base_date(base)
        .recovery_rate(recovery)
        .knots([(1.0, hazard_rate), (5.0, hazard_rate), (10.0, hazard_rate)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()
        .expect("flat hazard curve")
}

fn market(as_of: Date) -> MarketContext {
    MarketContext::new()
        .try_insert(flat_discount("USD-OIS", as_of, 0.03))
        .expect("insert market curve")
        .try_insert(flat_hazard("HZ-SN", as_of, 0.4, 0.02))
        .expect("insert market curve")
}

fn option(as_of: Date, option_type: OptionType, strike_bp: f64, vol: f64) -> CdsOption {
    option_with_coupon(as_of, option_type, strike_bp, strike_bp, vol)
}

fn option_with_coupon(
    as_of: Date,
    option_type: OptionType,
    strike_bp: f64,
    coupon_bp: f64,
    vol: f64,
) -> CdsOption {
    let mut option = CdsOption::builder()
        .id(InstrumentId::new("CDSO-UNIT"))
        .strike(crate::cds_option::strike::CdsOptionStrike::Spread(
            bp_to_decimal(strike_bp),
        ))
        .option_type(option_type)
        .expiry(as_of.add_months(12).expect("valid date shift"))
        .underlying_maturity(as_of.add_months(60).expect("valid date shift"))
        .notional(Money::from((10_000_000_i64, Currency::USD)))
        .settlement(SettlementType::Cash)
        .recovery_rate(0.40)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id(CurveId::new("HZ-SN"))
        .vol_surface_id(CurveId::new("CDSO-VOL"))
        .coupon_bp(Decimal::try_from(coupon_bp).expect("valid coupon bp"))
        .build()
        .expect("valid cds option");
    option
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(vol);
    option
}

fn context_for(
    option: &CdsOption,
    market: &MarketContext,
    as_of: Date,
    sigma: f64,
) -> ForwardCdsContext {
    let cds = synthetic_underlying_cds(option, as_of).expect("synthetic cds");
    let disc = market
        .get_discount(&option.discount_curve_id)
        .expect("discount");
    let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
    ForwardCdsContext::build(option, disc.as_ref(), hazard.as_ref(), &cds, as_of, sigma)
        .expect("forward cds context")
}

fn deterministic_payoff_per_n(ctx: &ForwardCdsContext) -> f64 {
    let exercise = ctx.scale
        * ctx.exercise_survival_multiplier()
        * (ctx.sign() * ctx.forward_value()
            + ctx.signed_strike_adjustment_per_n()
            + ctx.signed_loss_settlement_per_n())
        .max(0.0);
    let default_payment = if !ctx.is_index && !ctx.knockout && ctx.option_type == OptionType::Call {
        ctx.front_end_protection
    } else {
        0.0
    };
    (exercise + default_payment) * ctx.df_to_settlement
}

fn normal_cdf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.231_641_9 * x.abs());
    let poly = t
        * (0.319_381_530
            + t * (-0.356_563_782
                + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
    let tail = norm_pdf(x).expect("valid normal variate") * poly;
    if x >= 0.0 { 1.0 - tail } else { tail }
}

fn black76_payer_per_n(ctx: &ForwardCdsContext) -> f64 {
    let f = ctx.forward_par_spread.max(ZERO_TOLERANCE);
    let k = ctx
        .spread_strike()
        .expect("Black-76 helper requires a spread strike")
        .max(ZERO_TOLERANCE);
    let vol_sqrt_t = ctx.sigma * ctx.t_expiry.sqrt();
    let d1 = ((f / k).ln() + 0.5 * vol_sqrt_t * vol_sqrt_t) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;
    ctx.df_to_settlement
        * ctx.exercise_survival_multiplier()
        * ctx.bootstrapped_l_at_expiry
        * (f * normal_cdf(d1) - k * normal_cdf(d2))
}

#[test]
fn normal_quadrature_rejects_invalid_grids_and_node_values() {
    for (step, limit) in [(0.0, 6.0), (0.05, f64::INFINITY), (f64::from_bits(1), 6.0)] {
        assert!(normal_integral(step, limit, |_| 1.0).is_err());
    }
    assert!(normal_integral(0.05, 6.0, |_| f64::NAN).is_err());
}

#[test]
fn normal_quadrature_converges_when_step_is_halved() {
    let coarse = normal_integral(0.05, bloomberg_cdso::MIN_Z_LIMIT, |z| {
        (0.30 * z).exp().max(0.0)
    })
    .expect("valid quadrature grid");
    let fine = normal_integral(0.025, bloomberg_cdso::MIN_Z_LIMIT, |z| {
        (0.30 * z).exp().max(0.0)
    })
    .expect("valid quadrature grid");
    assert!(
        (coarse - fine).abs() < 1e-8,
        "normal quadrature should be stable under step halving: coarse={coarse}, fine={fine}",
    );
}

/// Bloomberg CDSO screen reconciliation depends on the published
/// standard-normal driver grid `[-6, 6]` for ordinary market vols. Keep
/// that fixed range for realistic short-dated CDSO inputs while retaining
/// the legacy stress guard for extreme `σ√t`.
#[test]
fn z_limit_uses_bloomberg_grid_with_legacy_stress_guard() {
    let cdx_ig_46_sigma = 0.3603_f64;
    let cdx_ig_46_t = 42.0 / 365.0;
    assert_eq!(
        z_limit(cdx_ig_46_sigma, cdx_ig_46_t),
        bloomberg_cdso::MIN_Z_LIMIT
    );

    let stressed_sigma = 2.0_f64;
    let stressed_t = 4.0_f64;
    let stressed_limit = z_limit(stressed_sigma, stressed_t);
    assert!(
        (stressed_limit - 16.0).abs() < 1e-12,
        "extreme vol-time inputs should still use the legacy 4σ√t guard, got {stressed_limit}"
    );
}

#[test]
fn index_front_end_protection_is_recomputed_on_bumped_curve() {
    let as_of = date!(2025 - 01 - 01);
    let mut option = option(as_of, OptionType::Call, 100.0, 0.30);
    option.underlying_is_index = true;
    option.knockout = false;
    option.index_factor = 1.0;
    option.coupon_bp = Some(Decimal::new(100, 0));

    let market = market(as_of);
    let base_ctx = context_for(&option, &market, as_of, 0.30);
    let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
    let bumped_hazard = hazard
        .with_parallel_hazard_rate_bump_bp(1.0)
        .expect("bumped hazard");
    let bumped_market = market
        .try_insert(bumped_hazard)
        .expect("insert market curve");
    let bumped_ctx = context_for(&option, &bumped_market, as_of, 0.30);

    assert!(
        bumped_ctx.front_end_protection > base_ctx.front_end_protection,
        "FEP should be rebuilt from the bumped hazard curve: base={}, bumped={}",
        base_ctx.front_end_protection,
        bumped_ctx.front_end_protection,
    );
}

#[test]
fn zero_vol_limit_matches_bloomberg_deterministic_payoff() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let option = option(as_of, OptionType::Call, 100.0, 1e-6);
    let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
    let ctx = context_for(&option, &market, as_of, 1e-6);

    let actual_per_n = npv(&option, &cds, &market, 1e-6, as_of)
        .expect("npv")
        .amount()
        / option.notional.amount();
    let expected_per_n = deterministic_payoff_per_n(&ctx);

    assert!(
        (actual_per_n - expected_per_n).abs() < 1e-8,
        "zero-vol CDSO payoff should converge to Bloomberg deterministic payoff: actual={actual_per_n}, expected={expected_per_n}"
    );
}

#[test]
fn bloomberg_intrinsic_lower_bound_holds() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);

    for option_type in [OptionType::Call, OptionType::Put] {
        for strike_bp in [50.0, 100.0, 200.0, 400.0] {
            let option = option(as_of, option_type, strike_bp, 0.30);
            let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
            let ctx = context_for(&option, &market, as_of, 0.30);
            let actual_per_n = npv(&option, &cds, &market, 0.30, as_of)
                .expect("npv")
                .amount()
                / option.notional.amount();
            let lower_bound = deterministic_payoff_per_n(&ctx);

            assert!(
                actual_per_n + 1e-10 >= lower_bound,
                "Bloomberg intrinsic lower bound violated for {:?} strike {strike_bp}: actual={actual_per_n}, lower_bound={lower_bound}",
                option_type,
            );
        }
    }
}

#[test]
fn calibration_mean_is_option_type_invariant() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let call = option(as_of, OptionType::Call, 125.0, 0.35);
    let put = option(as_of, OptionType::Put, 125.0, 0.35);
    let call_ctx = context_for(&call, &market, as_of, 0.35);
    let put_ctx = context_for(&put, &market, as_of, 0.35);

    let call_m = calibrate_lognormal_mean(&call_ctx).expect("call calibration");
    let put_m = calibrate_lognormal_mean(&put_ctx).expect("put calibration");

    assert!(
        (call_m - put_m).abs() < 1e-12,
        "lognormal mean calibration should not depend on payer/receiver option type: call_m={call_m}, put_m={put_m}"
    );
}

#[test]
fn calibration_rejects_distressed_forward_spreads() {
    let as_of = date!(2025 - 01 - 01);
    let distressed_market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", as_of, 0.03))
        .expect("insert market curve")
        .try_insert(flat_hazard("HZ-SN", as_of, 0.4, 0.25))
        .expect("insert market curve");
    let option = option(as_of, OptionType::Call, 100.0, 0.30);
    let ctx = context_for(&option, &distressed_market, as_of, 0.30);

    let err = calibrate_lognormal_mean(&ctx).expect_err("distressed calibration should fail");

    assert!(
        err.to_string().contains("distressed"),
        "error should explain distressed-credit calibration guard: {err}"
    );
}

#[test]
fn bloomberg_put_call_parity_holds() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);

    for strike_bp in [50.0, 100.0, 200.0, 400.0] {
        let call = option(as_of, OptionType::Call, strike_bp, 0.30);
        let put = option(as_of, OptionType::Put, strike_bp, 0.30);
        let call_cds = synthetic_underlying_cds(&call, as_of).expect("call cds");
        let put_cds = synthetic_underlying_cds(&put, as_of).expect("put cds");
        let call_ctx = context_for(&call, &market, as_of, 0.30);

        let call_pv = npv(&call, &call_cds, &market, 0.30, as_of)
            .expect("call npv")
            .amount();
        let put_pv = npv(&put, &put_cds, &market, 0.30, as_of)
            .expect("put npv")
            .amount();
        let expected = call.notional.amount()
            * call_ctx.scale
            * call_ctx.exercise_survival_multiplier()
            * call_ctx.df_to_settlement
            * (call_ctx.forward_value()
                + call_ctx.signed_strike_adjustment_per_n()
                + call_ctx.signed_loss_settlement_per_n())
            + call.notional.amount() * call_ctx.df_to_settlement * call_ctx.front_end_protection;

        assert!(
            (call_pv - put_pv - expected).abs() < 1e-3,
            "Bloomberg parity OC-OP=P_te*(F0+H(K)+D) failed at strike {strike_bp}: call={call_pv}, put={put_pv}, expected_diff={expected}, diff={}",
            (call_pv - put_pv - expected).abs()
        );
    }
}

/// Companion to [`bloomberg_put_call_parity_holds`] that decouples the
/// underlying CDS coupon `c` from the option strike `K`. The default
/// parity helper sets `c = K`, which collapses `H(K) = ξN(c−K)A(K)` to
/// zero — so it does not actually exercise the strike-adjustment term.
/// This test pins parity for the CDX-style case where the underlying
/// runs at a standard 100 bp coupon and the option is struck above
/// and below that coupon, making `(c−K)L(K)` non-trivial in the
/// parity arithmetic.
#[test]
fn bloomberg_put_call_parity_holds_with_distinct_coupon_and_strike() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let coupon_bp = 100.0;
    let vol = 0.30;

    for strike_bp in [50.0, 75.0, 125.0, 200.0, 400.0] {
        let call = option_with_coupon(as_of, OptionType::Call, strike_bp, coupon_bp, vol);
        let put = option_with_coupon(as_of, OptionType::Put, strike_bp, coupon_bp, vol);
        let call_cds = synthetic_underlying_cds(&call, as_of).expect("call cds");
        let put_cds = synthetic_underlying_cds(&put, as_of).expect("put cds");
        let call_ctx = context_for(&call, &market, as_of, vol);

        // Sanity: the strike-adjustment term must be non-zero in this
        // configuration, otherwise the test does not exercise H(K).
        let strike_adj = call_ctx.signed_strike_adjustment_per_n();
        assert!(
            strike_adj.abs() > 1e-12,
            "test setup error: strike adjustment H(K)/N is zero at strike {strike_bp} with coupon {coupon_bp}"
        );

        let call_pv = npv(&call, &call_cds, &market, vol, as_of)
            .expect("call npv")
            .amount();
        let put_pv = npv(&put, &put_cds, &market, vol, as_of)
            .expect("put npv")
            .amount();
        let expected = call.notional.amount()
            * call_ctx.scale
            * call_ctx.exercise_survival_multiplier()
            * call_ctx.df_to_settlement
            * (call_ctx.forward_value()
                + call_ctx.signed_strike_adjustment_per_n()
                + call_ctx.signed_loss_settlement_per_n())
            + call.notional.amount() * call_ctx.df_to_settlement * call_ctx.front_end_protection;

        // For deep-OTM strikes with c ≠ K, |expected| can dwarf the
        // ATM scale (the (c−K)L(K) term grows linearly with the
        // strike–coupon gap), so an absolute tolerance gates on the
        // wrong scale. Use the larger of $1e-3 (ATM-scale floor) and
        // a 1e-7 relative bound — the trapezoidal quadrature targets
        // ~1e-9 relative precision so this leaves plenty of headroom.
        let abs_diff = (call_pv - put_pv - expected).abs();
        let tolerance = 1e-3_f64.max(1e-7 * expected.abs().max(call_pv.abs()).max(put_pv.abs()));
        assert!(
            abs_diff < tolerance,
            "Bloomberg parity OC-OP=P_te*(F0+H(K)+D) failed at strike {strike_bp} \
             (coupon {coupon_bp}): call={call_pv}, put={put_pv}, expected_diff={expected}, \
             diff={abs_diff}, tol={tolerance}, strike_adj_per_N={strike_adj}"
        );
    }
}

#[test]
fn stripped_low_vol_fixture_approaches_black76() {
    // Black-76 has no FEP-equivalent — it values only the spread payoff
    // at exercise — so to compare against it we must use a knockout
    // contract (where the option pays nothing on default-before-expiry).
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let sigma = 0.01;
    let mut option = option(as_of, OptionType::Call, 100.0, sigma);
    option.knockout = true;
    let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
    let ctx = context_for(&option, &market, as_of, sigma);

    let actual_per_n = npv(&option, &cds, &market, sigma, as_of)
        .expect("npv")
        .amount()
        / option.notional.amount();
    let black_per_n = black76_payer_per_n(&ctx);
    let tolerance = 0.01 * black_per_n.abs().max(1e-8);

    assert!(
        (actual_per_n - black_per_n).abs() <= tolerance,
        "stripped low-vol fixture should approach Black-76: actual={actual_per_n}, black={black_per_n}, diff={}, tol={tolerance}",
        (actual_per_n - black_per_n).abs()
    );
}

/// Single-name no-knockout behavior is represented through the exercise
/// survival multiplier, not an index front-end-protection payoff. The
/// Bloomberg index-option FEP convention is specific to indices.
#[test]
fn non_knockout_single_name_does_not_add_index_fep() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let sigma = 0.30;

    let mut knockout_option = option(as_of, OptionType::Call, 100.0, sigma);
    knockout_option.knockout = true;
    assert!(!knockout_option.underlying_is_index);
    let mut non_knockout_option = option(as_of, OptionType::Call, 100.0, sigma);
    non_knockout_option.knockout = false;
    assert!(!non_knockout_option.underlying_is_index);

    let knockout_cds = synthetic_underlying_cds(&knockout_option, as_of).expect("ko cds");
    let non_knockout_cds = synthetic_underlying_cds(&non_knockout_option, as_of).expect("nko cds");

    let knockout_ctx = context_for(&knockout_option, &market, as_of, sigma);
    let non_knockout_ctx = context_for(&non_knockout_option, &market, as_of, sigma);

    // F_0 is the clean forward swap value and does not include an index
    // front-end-protection term for single-name options.
    let f0_gap = non_knockout_ctx.forward_value() - knockout_ctx.forward_value();
    assert!(
        f0_gap.abs() < 1e-12,
        "single-name F_0 must not add index FEP: gap={f0_gap}",
    );

    // Pricing must still reflect the knockout survival multiplier.
    let ko_pv = npv(&knockout_option, &knockout_cds, &market, sigma, as_of)
        .expect("ko npv")
        .amount();
    let nko_pv = npv(
        &non_knockout_option,
        &non_knockout_cds,
        &market,
        sigma,
        as_of,
    )
    .expect("nko npv")
    .amount();
    assert!(
        nko_pv > ko_pv,
        "non-knockout single-name option must price above knockout: ko={ko_pv}, nko={nko_pv}",
    );
}

/// Pin the Bloomberg CDSO theta convention so it cannot silently drift
/// from DOCS 2055833 §2.5 ("shorten the exercise time `t_e` by
/// `1/365.25`"). On `cdx_ig_46_payer_atm_jun26` the pure-T-shift
/// formulation is empirically closer to the CDSO screen than the
/// alternative as-of-shift; see the Phase 5b remediation note in
/// `tests/golden/data/pricing/bloomberg/cds_option/cdx_ig_46_payer_atm_jun26.json`.
#[test]
fn theta_uses_pure_t_shift_with_365_25_denominator() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let option = option(as_of, OptionType::Call, 100.0, 0.30);
    let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
    let ctx = context_for(&option, &market, as_of, 0.30);

    let actual = theta(&option, &cds, &market, 0.30, as_of).expect("theta");

    // Reference recomputation: same calibrated m, only shift t_expiry.
    let m = calibrate_lognormal_mean(&ctx).expect("calibration");
    let base = price_with_calibrated_mean(&ctx, m, ctx.t_expiry).expect("payoff integration");
    let expected = {
        let shortened = (ctx.t_expiry - 1.0 / bloomberg_cdso::THETA_DAYS_IN_YEAR).max(0.0);
        (price_with_calibrated_mean(&ctx, m, shortened).expect("payoff integration") - base)
            * option.notional.amount()
    };
    assert!(
        (actual - expected).abs() < 1e-9,
        "theta must use pure-T-shift on the integrand only: actual={actual}, expected={expected}"
    );

    // The 365 vs 365.25 denominator is small but real ($ ~ 1bp/day on
    // realistic notionals); regressing it would silently move every
    // CDSO theta. Lock the difference > 0 so a typo would fail.
    let shortened_365 = (ctx.t_expiry - 1.0 / 365.0).max(0.0);
    let theta_365 =
        (price_with_calibrated_mean(&ctx, m, shortened_365).expect("payoff integration") - base)
            * option.notional.amount();
    assert!(
        (actual - theta_365).abs() > 0.0,
        "theta with 1/365.25 must differ from theta with 1/365.0; if equal, day basis was changed"
    );
}

/// Companion guard: confirm the theta path does NOT propagate the
/// as-of date through the curves. If a future refactor switches to
/// as-of-shift, df_to_expiry and survival_to_expiry would change and
/// this test would diverge from the reference reconstruction above.
#[test]
fn theta_does_not_advance_curves_with_as_of_shift() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let option = option(as_of, OptionType::Call, 100.0, 0.30);
    let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");

    let ctx_today = context_for(&option, &market, as_of, 0.30);
    let ctx_tomorrow = context_for(&option, &market, as_of + time::Duration::days(1), 0.30);

    // The two contexts have meaningfully different df_to_expiry and
    // survival_to_expiry on a flat curve (one fewer day of discount /
    // survival). If theta() were doing as-of-shift, its result would
    // approximately equal:
    //     P_tomorrow(m_tomorrow) - P_today(m_today)
    // We assert the actual theta does NOT match that as-of-shift
    // reconstruction (it would be off by ~10–20% on this fixture).
    let m_today = calibrate_lognormal_mean(&ctx_today).expect("calibration today");
    let m_tomorrow = calibrate_lognormal_mean(&ctx_tomorrow).expect("calibration tomorrow");
    let as_of_shift_theta =
        (price_with_calibrated_mean(&ctx_tomorrow, m_tomorrow, ctx_tomorrow.t_expiry)
            .expect("payoff integration")
            - price_with_calibrated_mean(&ctx_today, m_today, ctx_today.t_expiry)
                .expect("payoff integration"))
            * option.notional.amount();

    let actual = theta(&option, &cds, &market, 0.30, as_of).expect("theta");

    // We only require the two formulations to be measurably distinct
    // — the precise gap depends on the curve. Any nontrivial difference
    // (> 1% of the larger magnitude) confirms the implementation is
    // not silently doing as-of-shift.
    let denom = actual.abs().max(as_of_shift_theta.abs()).max(1e-9);
    let rel_gap = (actual - as_of_shift_theta).abs() / denom;
    assert!(
        rel_gap > 0.01,
        "theta() must use pure-T-shift, not as-of-shift: pure_t={actual}, as_of_shift={as_of_shift_theta}, rel_gap={rel_gap}"
    );
}

/// With a flat observed hazard, identical survival inputs must give
/// the same annuity in the curve anchor and the spread-state model.
#[test]
fn curve_and_state_annuities_share_the_same_cashflow_convention() {
    let as_of = date!(2025 - 01 - 01);
    let market = market(as_of);
    let mut option = option(as_of, OptionType::Call, 100.0, 0.30);
    option.knockout = true;
    let ctx = context_for(&option, &market, as_of, 0.30);
    let triangle_annuity = ctx.flat_annuity(0.02 * ctx.lgd);
    assert!(
        (triangle_annuity - ctx.bootstrapped_l_at_expiry).abs() < 1e-12,
        "same hazard must give same annuity: state={triangle_annuity}, curve={}",
        ctx.bootstrapped_l_at_expiry
    );
}

// Price-strike (CDX HY convention) payoff tests

/// S&P CDS Indices Primer default-adjusted strike, used as a TEST ORACLE
/// only. Production uses the direct `H_price = ξ(K − 1)·f0/f` term and
/// keeps realized loss in the `D` settlement; this oracle folds factor
/// and loss into an equivalent strike quoted on the current factor.
fn adjusted_strike_pct(k_pct: f64, f0: f64, f: f64, realized_loss: f64) -> f64 {
    let k = k_pct / 100.0;
    100.0 * (1.0 - ((1.0 - k) * f0 - realized_loss) / f)
}

#[allow(clippy::too_many_arguments)]
fn price_strike_option(
    as_of: Date,
    option_type: OptionType,
    strike_price_pct: f64,
    coupon_bp: f64,
    vol: f64,
    strike_factor: f64,
    current_factor: f64,
    realized_loss: Option<f64>,
) -> CdsOption {
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::{Instrument, SettlementType};

    let mut option = CdsOption::builder()
        .id(InstrumentId::new("CDSO-HY-UNIT"))
        .strike(crate::cds_option::strike::CdsOptionStrike::CleanPricePct(
            Decimal::try_from(strike_price_pct).expect("valid clean-price strike"),
        ))
        .option_type(option_type)
        .expiry(as_of.add_months(12).expect("valid date shift"))
        .underlying_maturity(as_of.add_months(60).expect("valid date shift"))
        .notional(Money::from((10_000_000_i64, Currency::USD)))
        .settlement(SettlementType::Cash)
        .recovery_rate(0.40)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id(CurveId::new("HZ-SN"))
        .vol_surface_id(CurveId::new("CDSO-VOL"))
        .underlying_is_index(true)
        .index_factor(current_factor)
        .strike_index_factor(strike_factor)
        .coupon_bp(Decimal::try_from(coupon_bp).expect("valid coupon bp"))
        .build()
        .expect("valid cds option");
    option.realized_loss = realized_loss.unwrap_or(0.0);
    option
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(vol);
    option
        .validate_invariants()
        .expect("post-construction state must satisfy full validation");
    option
}

fn npv_per_unit(option: &CdsOption, market: &MarketContext, as_of: Date, sigma: f64) -> f64 {
    let cds = synthetic_underlying_cds(option, as_of).expect("synthetic cds");
    npv(option, &cds, market, sigma, as_of)
        .expect("npv")
        .amount()
        / option.notional.amount()
}

#[test]
fn sp_primer_factor_loss_fixture_yields_107_9874() {
    // S&P Dow Jones Indices CDS Indices Primer clean-price adjustment:
    // K = 107.0, f0 = 1.00, f = 0.99, one default at 9.25% recovery of
    // a 1% weight name: L = (1 − 0.0925)·0.01 = 0.009075.
    // Published adjusted strike: 107.9874% (rounded to 4 dp).
    let k_adj = adjusted_strike_pct(107.0, 1.0, 0.99, (1.0 - 0.0925) * 0.01);
    assert!(
        (k_adj - 107.9874).abs() < 5e-4,
        "S&P primer adjusted strike mismatch: got {k_adj}, expected 107.9874"
    );
}

/// The direct `H_price` production form must equal the adjusted-strike
/// oracle when the oracle OMITS realized loss from `D` (the loss is
/// already folded into its strike). This is the plan's anti-double-count
/// identity: `ξ(K_adj − 1) = ξ(K − 1)·f0/f + ξ·L/f`.
#[test]
fn price_strike_direct_form_matches_adjusted_strike_oracle() {
    let as_of = date!(2025 - 01 - 01);
    let mkt = market(as_of);
    let (k_pct, f0, f, loss) = (107.0, 1.0, 0.99, (1.0 - 0.0925) * 0.01);
    let vol = 0.35;

    for option_type in [OptionType::Call, OptionType::Put] {
        let direct = price_strike_option(as_of, option_type, k_pct, 500.0, vol, f0, f, Some(loss));
        // Oracle: adjusted strike quoted on the CURRENT factor (f0' = f),
        // realized loss removed from the settlement term.
        let oracle = price_strike_option(
            as_of,
            option_type,
            adjusted_strike_pct(k_pct, f0, f, loss),
            500.0,
            vol,
            f,
            f,
            None,
        );
        let npv_direct = npv_per_unit(&direct, &mkt, as_of, vol);
        let npv_oracle = npv_per_unit(&oracle, &mkt, as_of, vol);
        assert!(
            (npv_direct - npv_oracle).abs() <= 1e-10 * npv_direct.abs().max(1.0),
            "{option_type:?}: direct={npv_direct}, oracle={npv_oracle}"
        );
    }
}

/// A deliberately double-counted oracle (adjusted strike AND realized
/// loss retained in `D`) must NOT match — this protects the production
/// path against a regression that counts the loss twice.
#[test]
fn double_counted_adjusted_strike_oracle_differs() {
    let as_of = date!(2025 - 01 - 01);
    let mkt = market(as_of);
    let (k_pct, f0, f, loss) = (107.0, 1.0, 0.99, (1.0 - 0.0925) * 0.01);
    let vol = 0.35;

    let direct = price_strike_option(
        as_of,
        OptionType::Call,
        k_pct,
        500.0,
        vol,
        f0,
        f,
        Some(loss),
    );
    // Same (valid) factor/loss state as `direct`, but the strike already
    // folds the loss in — retaining it in `D` then counts it twice.
    let double_counted = price_strike_option(
        as_of,
        OptionType::Call,
        adjusted_strike_pct(k_pct, f0, f, loss),
        500.0,
        vol,
        f0,
        f,
        Some(loss),
    );
    let npv_direct = npv_per_unit(&direct, &mkt, as_of, vol);
    let npv_double = npv_per_unit(&double_counted, &mkt, as_of, vol);
    // The double count adds ~ξ·L/f to every exercised node: material.
    assert!(
        (npv_double - npv_direct).abs() > 1e-4,
        "double-counted oracle must differ: direct={npv_direct}, double={npv_double}"
    );
}

/// Pre-default state (`f = f0`, `L = 0`): the deterministic price term
/// reduces to `ξ(K − 1)` exactly.
#[test]
fn pre_default_price_term_reduces_to_k_minus_one() {
    let as_of = date!(2025 - 01 - 01);
    let mkt = market(as_of);
    let payer = price_strike_option(as_of, OptionType::Call, 107.0, 500.0, 0.35, 1.0, 1.0, None);
    let ctx = context_for(&payer, &mkt, as_of, 0.35);
    assert!(
        (ctx.signed_strike_adjustment_per_n() - 0.07).abs() < 1e-14,
        "payer H_price should be K − 1 = 0.07, got {}",
        ctx.signed_strike_adjustment_per_n()
    );

    let receiver = price_strike_option(as_of, OptionType::Put, 107.0, 500.0, 0.35, 1.0, 1.0, None);
    let ctx_r = context_for(&receiver, &mkt, as_of, 0.35);
    assert!(
        (ctx_r.signed_strike_adjustment_per_n() + 0.07).abs() < 1e-14,
        "receiver H_price should be −(K − 1) = −0.07, got {}",
        ctx_r.signed_strike_adjustment_per_n()
    );
}

/// Payer/receiver parity for the price-strike payoff:
/// `payer − receiver = df · (f·F0 + (K − 1)·f0 + L)` per unit
/// notional, both node-by-node (max(x,0) − max(−x,0) = x) and after
/// integration against the calibrated lognormal density.
#[test]
fn price_strike_payer_receiver_parity_holds() {
    let as_of = date!(2025 - 01 - 01);
    let mkt = market(as_of);
    let (k_pct, f0, f, loss) = (105.0, 1.0, 0.99, 0.004);
    let vol = 0.40;

    let payer = price_strike_option(
        as_of,
        OptionType::Call,
        k_pct,
        500.0,
        vol,
        f0,
        f,
        Some(loss),
    );
    let receiver =
        price_strike_option(as_of, OptionType::Put, k_pct, 500.0, vol, f0, f, Some(loss));
    let ctx = context_for(&payer, &mkt, as_of, vol);

    let parity_rhs =
        ctx.df_to_settlement * (f * ctx.forward_value() + (k_pct / 100.0 - 1.0) * f0 + loss);
    let lhs = npv_per_unit(&payer, &mkt, as_of, vol) - npv_per_unit(&receiver, &mkt, as_of, vol);
    assert!(
        (lhs - parity_rhs).abs() < 1e-9,
        "parity violated: payer − receiver = {lhs}, expected {parity_rhs}"
    );
}

/// Payer value rises in the clean-price strike and in the credit spread;
/// receiver value falls in both.
#[test]
fn price_strike_monotonicity_in_strike_and_spread() {
    let as_of = date!(2025 - 01 - 01);
    let mkt = market(as_of);
    let wider_spread_market = MarketContext::new()
        .try_insert(flat_discount("USD-OIS", as_of, 0.03))
        .expect("insert market curve")
        .try_insert(flat_hazard("HZ-SN", as_of, 0.4, 0.03))
        .expect("insert market curve");
    let vol = 0.40;

    let payer_low = price_strike_option(as_of, OptionType::Call, 103.0, 500.0, vol, 1.0, 1.0, None);
    let payer_high =
        price_strike_option(as_of, OptionType::Call, 107.0, 500.0, vol, 1.0, 1.0, None);
    assert!(
        npv_per_unit(&payer_high, &mkt, as_of, vol) > npv_per_unit(&payer_low, &mkt, as_of, vol),
        "payer must gain value as the clean-price strike rises"
    );

    let receiver_low =
        price_strike_option(as_of, OptionType::Put, 103.0, 500.0, vol, 1.0, 1.0, None);
    let receiver_high =
        price_strike_option(as_of, OptionType::Put, 107.0, 500.0, vol, 1.0, 1.0, None);
    assert!(
        npv_per_unit(&receiver_high, &mkt, as_of, vol)
            < npv_per_unit(&receiver_low, &mkt, as_of, vol),
        "receiver must lose value as the clean-price strike rises"
    );

    // Wider spreads: protection worth more → payer up, receiver down.
    assert!(
        npv_per_unit(&payer_low, &wider_spread_market, as_of, vol)
            > npv_per_unit(&payer_low, &mkt, as_of, vol),
        "payer must gain value as spreads widen"
    );
    assert!(
        npv_per_unit(&receiver_high, &wider_spread_market, as_of, vol)
            < npv_per_unit(&receiver_high, &mkt, as_of, vol),
        "receiver must lose value as spreads widen"
    );
}

/// The small-positive-vol limit converges to the clean deterministic
/// intrinsic payoff without altering the explicit-vol validation
/// contract.
#[test]
fn price_strike_small_vol_limit_matches_deterministic_payoff() {
    let as_of = date!(2025 - 01 - 01);
    let mkt = market(as_of);
    let vol = 1e-4;
    for option_type in [OptionType::Call, OptionType::Put] {
        let option = price_strike_option(
            as_of,
            option_type,
            105.0,
            500.0,
            vol,
            1.0,
            0.99,
            Some(0.004),
        );
        let ctx = context_for(&option, &mkt, as_of, vol);
        let intrinsic = deterministic_payoff_per_n(&ctx);
        let priced = npv_per_unit(&option, &mkt, as_of, vol);
        assert!(
            (priced - intrinsic).abs() < 1e-6,
            "{option_type:?}: small-vol limit {priced} must approach \
             deterministic intrinsic {intrinsic}"
        );
    }
}

mod production_cds_option_audit {
    use super::*;

    #[test]
    fn exercise_payment_date_changes_discounting_without_changing_variance() {
        let as_of = date!(2025 - 01 - 01);
        let curves = market(as_of);
        let mut option = option(as_of, OptionType::Call, 100.0, 0.3);
        option.knockout = true;
        let cds = synthetic_underlying_cds(&option, as_of).expect("CDS");
        let original = npv(&option, &cds, &curves, 0.3, as_of)
            .expect("price")
            .amount();
        option.exercise_settlement_date = Some(option.expiry + time::Duration::days(5));
        let delayed = npv(&option, &cds, &curves, 0.3, as_of)
            .expect("price")
            .amount();
        assert!((delayed - original * (-0.03_f64 * 5.0 / 365.0).exp()).abs() < 1e-8);
    }

    #[test]
    fn non_knockout_single_name_pays_front_end_protection_only_to_payer() {
        let as_of = date!(2025 - 01 - 01);
        let curves = market(as_of);
        for side in [OptionType::Call, OptionType::Put] {
            let mut option = option(as_of, side, 100.0, 0.3);
            let cds = synthetic_underlying_cds(&option, as_of).expect("CDS");
            option.knockout = true;
            let ko = npv(&option, &cds, &curves, 0.3, as_of)
                .expect("KO")
                .amount();
            option.knockout = false;
            let nko = npv(&option, &cds, &curves, 0.3, as_of)
                .expect("NKO")
                .amount();
            let t = (option.expiry - as_of).whole_days() as f64 / 365.0;
            let expected = if side == OptionType::Call {
                option.notional.amount() * (-0.03 * t).exp() * 0.6 * (1.0 - (-0.02 * t).exp())
            } else {
                0.0
            };
            assert!(
                (nko - ko - expected).abs() < 1e-8,
                "{side:?}: NKO-KO={} vs FEP={expected}",
                nko - ko
            );
        }
    }

    #[test]
    fn valuation_clock_ignores_exercise_settlement_date() {
        let initial = date!(2025 - 01 - 01);
        let later = date!(2025 - 07 - 01);
        let mut option = option(initial, OptionType::Call, 100.0, 0.3);
        option.exercise_settlement_date = Some(option.expiry + time::Duration::days(5));
        let expected = (option.expiry - later).whole_days() as f64 / 365.0;
        assert!(
            (option.time_to_expiry(later).expect("time") - expected).abs() < 1e-14,
            "model time must run from current valuation date to legal expiry"
        );
    }

    #[test]
    fn front_end_protection_window_advances_with_valuation_date() {
        let initial = date!(2025 - 01 - 01);
        let later = date!(2025 - 07 - 01);
        let mut option = option(initial, OptionType::Call, 100.0, 0.3);
        option.underlying_is_index = true;
        option.knockout = false;
        let curves = market(initial);
        let ctx = context_for(&option, &curves, later, 0.3);
        let t = (option.expiry - later).whole_days() as f64 / 365.0;
        assert!((ctx.front_end_protection - 0.6 * (1.0 - (-0.02 * t).exp())).abs() < 1e-12);
    }

    #[test]
    fn index_parity_contains_front_end_protection_once() {
        let as_of = date!(2025 - 01 - 01);
        let curves = MarketContext::new()
            .try_insert(flat_discount("USD-OIS", as_of, 0.0))
            .expect("insert market curve")
            .try_insert(flat_hazard("HZ-SN", as_of, 0.4, 0.02))
            .expect("insert market curve");
        let mut payer = option_with_coupon(as_of, OptionType::Call, 100.0, 100.0, 0.3);
        payer.underlying_is_index = true;
        payer.knockout = false;
        payer.strike =
            crate::cds_option::strike::CdsOptionStrike::CleanPricePct(Decimal::from(100));
        payer.strike_index_factor = Some(1.0);
        let mut receiver = payer.clone();
        receiver.option_type = OptionType::Put;
        let cds = synthetic_underlying_cds(&payer, as_of).expect("CDS");
        let p = npv(&payer, &cds, &curves, 0.3, as_of)
            .expect("payer")
            .amount();
        let r = npv(&receiver, &cds, &curves, 0.3, as_of)
            .expect("receiver")
            .amount();
        // With no discounting and a par clean-price strike, add back
        // the running premium PV to isolate the complete protection leg.
        let t = (payer.underlying_maturity + time::Duration::days(1) - as_of).whole_days() as f64
            / 365.0;
        let ctx = context_for(&payer, &curves, as_of, 0.3);
        let expiry_t = (payer.expiry - as_of).whole_days() as f64 / 365.0;
        let annuity: f64 = ctx
            .accrual_factors
            .iter()
            .zip(&ctx.times_from_expiry)
            .map(|(a, t)| a * (-0.02 * (expiry_t + t)).exp())
            .sum::<f64>()
            - ctx.accrual_pcd_to_expiry * (-0.02 * expiry_t).exp();
        let expected = payer.notional.amount() * (0.6 * (1.0 - (-0.02 * t).exp()) - 0.01 * annuity);
        assert!(
            (p - r - expected).abs() < 1e-7 * payer.notional.amount(),
            "payer-receiver {} vs single protection amount {expected}",
            p - r
        );
    }

    #[test]
    fn knockout_forward_excludes_pre_expiry_defaults() {
        let as_of = date!(2025 - 01 - 01);
        let curves = MarketContext::new()
            .try_insert(flat_discount("USD-OIS", as_of, 0.0))
            .expect("insert market curve")
            .try_insert(flat_hazard("HZ-SN", as_of, 0.4, 0.02))
            .expect("insert market curve");
        let mut option = option_with_coupon(as_of, OptionType::Call, 100.0, 100.0, 0.3);
        option.knockout = true;
        let ctx = context_for(&option, &curves, as_of, 0.3);
        let expiry_t = (option.expiry - as_of).whole_days() as f64 / 365.0;
        let maturity_t = (option.underlying_maturity + time::Duration::days(1) - as_of).whole_days()
            as f64
            / 365.0;
        let expected = 0.6 * ((-0.02 * expiry_t).exp() - (-0.02 * maturity_t).exp());
        let actual = (ctx.forward_value() + ctx.coupon * ctx.bootstrapped_l_at_expiry)
            * ctx.df_to_settlement
            * ctx.survival_to_expiry;
        assert!(
            (actual - expected).abs() < 1e-9,
            "KO forward protection {actual} vs post-expiry-only {expected}"
        );
    }
}
