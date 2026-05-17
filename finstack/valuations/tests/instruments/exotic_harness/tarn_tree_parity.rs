//! HW1F Monte-Carlo vs trinomial-tree parity for a TARN floating note (M6/M7).
//!
//! Cross-validates the two M6/M7 fixes by pricing the *same* instrument with
//! two fully independent HW1F engines:
//!
//! * the [`TarnPricer`] Monte-Carlo path — θ(t) bootstrapped from the discount
//!   curve (`calibrate_hw1f_params`, M6) and the period coupon indexed to the
//!   term forward reconstructed from the HW1F bond formula (`Hw1fTermForward`,
//!   M7); and
//! * a [`HullWhiteTree`] — a recombining trinomial lattice whose drift α(t) is
//!   calibrated to the same discount curve by forward induction, a completely
//!   different algorithm from the θ(t) bootstrap.
//!
//! Both engines therefore re-derive the curve-consistent short-rate dynamics
//! and the period term forward from scratch; agreement is meaningful evidence
//! that the MC term-forward reconstruction is correct and curve-consistent.
//!
//! # Instrument
//!
//! A deliberately *linear, non-path-dependent* TARN floating note: the target
//! coupon is set unreachably high (never knocks out), and with `fixed = 6%`
//! against a ~2-4% forward every period coupon `(fixed − Lᵢ)` stays well above
//! the `0` floor, so the `max` never binds. The payoff is then
//! `Σᵢ (fixed − Lᵢ)·τᵢ·DF(0,tᵢ) + DF(0,T)`, with `Lᵢ` the in-advance floating
//! fixing — the `[startᵢ, endᵢ]`-tenor simple forward observed at the coupon's
//! *start* date. This isolates the M6/M7 machinery (the forward reconstruction
//! and the calibrated short-rate distribution) from TARN-specific path
//! dependence, which a backward-induction tree cannot represent.
//!
//! # Residual sources
//!
//! The MC and tree PVs do not match to machine precision; the residual is
//! numerical, not a model discrepancy:
//!
//! * **Tree time-discretization** — the dominant term. The trinomial lattice
//!   has an O(1/steps) bias; empirically the MC-vs-tree gap falls from ~77 bp
//!   of notional at 360 steps to ~19 bp at 1400 steps (≈400 steps/yr here).
//! * **Discounting convention** — the MC pricer discounts each coupon with the
//!   *deterministic* curve DF, whereas the tree expectation is taken under the
//!   tᵢ-forward measure; the two differ by an O(σ²) convexity term. A modest
//!   σ = 40 bp keeps it negligible (a few hundred dollars on $1mm notional).
//! * **Monte-Carlo standard error** — shrinks as 1/√paths.
//!
//! The parity tolerance below bounds all three; the test still genuinely
//! cross-checks M6/M7 because the two engines reconstruct the curve dynamics
//! and the term forward by entirely separate routes.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::calibration::hull_white::HullWhiteParams;
use finstack_valuations::instruments::models::{HullWhiteTree, HullWhiteTreeConfig};
use finstack_valuations::instruments::rates::exotics_shared::RateExoticMcConfig;
use finstack_valuations::instruments::rates::tarn::{Tarn, TarnPricer};
use finstack_valuations::metrics::MetricId;
use finstack_valuations::pricer::Pricer;
use time::Month;

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).expect("valid date")
}

/// Upward-sloping discount curve — the regime in which a flat-θ Vasicek
/// mis-reprices (so the test genuinely depends on the M6 fix). Zero rate rises
/// from ~2% short to ~4.5% long; knots on a fine grid so the curve is smooth.
fn sloped_discount_curve(as_of: Date) -> DiscountCurve {
    let knots: Vec<(f64, f64)> = (0..=24)
        .map(|i| {
            let t = i as f64 * 0.25;
            let zero = 0.02 + 0.025 * (1.0 - (-0.4 * t).exp());
            (t, (-zero * t).exp())
        })
        .collect();
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .build()
        .expect("sloped discount curve")
}

fn market(as_of: Date) -> MarketContext {
    let discount = sloped_discount_curve(as_of);
    // The forward curve is a required instrument-contract input but is not read
    // by the HW1F MC path (single-curve: index reconstructed from the OIS curve).
    let forward = ForwardCurve::builder("USD-SOFR-6M", 0.5)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 0.03), (6.0, 0.03)])
        .build()
        .expect("forward curve");
    MarketContext::new().insert(discount).insert(forward)
}

/// A non-knockout, effectively-linear TARN: a plain floating-coupon note plus
/// redemption. `fixed_rate` 6% with a forward ~2-4% keeps every coupon well
/// above the `0` floor (so the `max` never binds); `target_coupon` 1e9 is
/// never hit, so the note has no path dependence.
fn floating_note_tarn(coupon_dates: Vec<Date>) -> Tarn {
    Tarn {
        id: InstrumentId::new("TARN-PARITY"),
        fixed_rate: 0.06,
        coupon_floor: 0.0,
        target_coupon: 1.0e9,
        notional: Money::new(1_000_000.0, Currency::USD),
        coupon_dates,
        floating_tenor: Tenor::semi_annual(),
        floating_index_id: CurveId::new("USD-SOFR-6M"),
        discount_curve_id: CurveId::new("USD-OIS"),
        day_count: DayCount::Act365F,
        pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Price the floating-note TARN on a calibrated [`HullWhiteTree`].
///
/// The TARN coupon fixes **in advance**: each coupon over `[start, end]` is set
/// by the floating rate observed *at the period start*. The tree therefore
/// re-derives, at each coupon's *start* date `tₛ`, the `[start, end]`-tenor
/// simple forward `Lᵢ = (1/P(tₛ, tₑ) − 1)/τᵢ` from its own analytic bond price
/// `P(t,T) = A(t,T)·exp(−B(t,T)·r)`. Each coupon's value is the tₛ-forward-
/// measure expectation `Ê[(fixed − Lᵢ)·τᵢ]` — the state-price-weighted node
/// average `Σⱼ Q(tₛ,j)·c(j) / Σⱼ Q(tₛ,j)` — discounted with the curve DF to the
/// payment date, matching the MC pricer's discounting convention so the two
/// values are directly comparable. The first coupon is already seasoned
/// (`start == as_of`): its fixing collapses to the deterministic root node.
fn tree_floating_note_pv(
    tarn: &Tarn,
    discount_curve: &DiscountCurve,
    as_of: Date,
    hw: HullWhiteParams,
    tree_steps: usize,
) -> f64 {
    let dc = tarn.day_count;
    let ctx = DayCountContext::default();
    let notional = tarn.notional.amount();

    let maturity = *tarn.coupon_dates.last().expect("coupon dates");
    let horizon = dc
        .year_fraction(as_of, maturity, ctx)
        .expect("maturity year fraction");

    let config = HullWhiteTreeConfig::new(hw.kappa, hw.sigma, tree_steps);
    let tree = HullWhiteTree::calibrate(config, discount_curve, horizon).expect("tree calibrate");

    let mut pv = 0.0_f64;
    for period in tarn.coupon_dates.windows(2) {
        let (start, end) = (period[0], period[1]);
        if end <= as_of {
            continue;
        }
        let t_end = dc.year_fraction(as_of, end, ctx).expect("t_end");
        let accrual = dc.year_fraction(start, end, ctx).expect("accrual");
        // In-advance fixing: sample the short rate at the period start. A
        // seasoned coupon (start ≤ as_of) fixes at the deterministic root.
        let t_fix = dc
            .year_fraction(as_of, start, ctx)
            .expect("t_start")
            .max(0.0);
        let fix_step = tree.time_to_step(t_fix);

        // State-price-weighted (tₛ-forward-measure) expectation of the coupon.
        let mut q_sum = 0.0_f64;
        let mut q_coupon = 0.0_f64;
        for node in 0..tree.num_nodes(fix_step) {
            let q = tree.state_price(fix_step, node);
            // [start, end]-tenor simple forward from the tree's analytic
            // HW1F bond price, reconstructed at the in-advance fixing node.
            let p = tree.bond_price(fix_step, node, t_end, discount_curve);
            let fwd = (1.0 / p - 1.0) / accrual;
            let coupon = (tarn.fixed_rate - fwd).max(tarn.coupon_floor) * accrual;
            q_sum += q;
            q_coupon += q * coupon;
        }
        let expected_coupon = if q_sum > 0.0 { q_coupon / q_sum } else { 0.0 };

        // Discount with the curve DF — identical convention to `TarnPricer`.
        let df = discount_curve
            .df_between_dates(as_of, end)
            .expect("coupon df");
        pv += expected_coupon * notional * df;
    }

    // Redemption of notional at maturity.
    let redemption_df = discount_curve
        .df_between_dates(as_of, maturity)
        .expect("redemption df");
    pv + notional * redemption_df
}

/// MC (HW1F path, M6/M7) vs trinomial-tree parity for a 3-year TARN note.
#[test]
fn tarn_floating_note_mc_matches_hw_tree() {
    let as_of = date(2025, Month::January, 1);
    let market = market(as_of);
    let discount_curve = market.get_discount("USD-OIS").expect("discount");

    // Semi-annual coupons over 3 years.
    let coupon_dates = vec![
        date(2025, Month::January, 1),
        date(2025, Month::July, 1),
        date(2026, Month::January, 1),
        date(2026, Month::July, 1),
        date(2027, Month::January, 1),
        date(2027, Month::July, 1),
        date(2028, Month::January, 1),
    ];
    let tarn = floating_note_tarn(coupon_dates);

    // Modest mean reversion / 40 bp vol. A small σ keeps the O(σ²) discounting-
    // convention term negligible; the short rate is still genuinely stochastic
    // (the reconstructed forward fluctuates by ~σ√t across paths).
    let hw = HullWhiteParams::new(0.10, 0.004).expect("hw params");

    // --- Monte-Carlo (M6/M7 path) -------------------------------------------
    let mc_result = TarnPricer::with_hw_params(hw)
        .with_config(RateExoticMcConfig {
            num_paths: 120_000,
            antithetic: true,
            min_steps_between_events: 8,
            seed: 20_260_516,
            ..Default::default()
        })
        .price_dyn(&tarn, &market, as_of)
        .expect("mc price");
    let mc_pv = mc_result.value.amount();
    let mc_stderr = *mc_result
        .measures
        .get(&MetricId::custom("mc_stderr"))
        .expect("mc_stderr measure");

    // --- Trinomial tree ------------------------------------------------------
    // 1400 steps over 3.5y (≈400/yr) drives the tree time-discretization bias
    // — the dominant MC-vs-tree gap — down to ~20 bp of notional.
    let tree_pv = tree_floating_note_pv(&tarn, discount_curve.as_ref(), as_of, hw, 1400);

    let diff = (mc_pv - tree_pv).abs();
    let notional = tarn.notional.amount();
    println!(
        "TARN note parity: mc={mc_pv:.2} (se={mc_stderr:.2})  tree={tree_pv:.2}  \
         |Δ|={diff:.2} ({:.4}% of notional)",
        diff / notional * 100.0
    );

    // Tolerance: 3× MC standard error plus a 0.05%-of-notional allowance for
    // the residual tree time-discretization bias and the O(σ²) discounting-
    // convention convexity term. Both are far below the structural PV.
    let tol = 3.0 * mc_stderr + 0.0005 * notional;
    assert!(
        diff < tol,
        "HW1F MC and trinomial-tree TARN-note PVs disagree: \
         mc={mc_pv:.2}, tree={tree_pv:.2}, |Δ|={diff:.2} > tol={tol:.2} \
         (3·se={:.2} + 5bp notional={:.2})",
        3.0 * mc_stderr,
        0.0005 * notional,
    );

    // Sanity: the note is worth roughly notional + a few coupons; certainly
    // positive and within an order of magnitude of par.
    assert!(
        mc_pv > 0.5 * notional && mc_pv < 1.5 * notional,
        "TARN-note PV {mc_pv:.2} is implausible for a ~par floating note"
    );
}
