//! Ignored diagnostics for the `cdx_ig_46_payer_atm_jun26` Bloomberg CDSO golden.
//!
//! Probes settlement/time-axis conventions, CS01 bump mechanics, and
//! snapshot-vs-bootstrap curve construction. Run with:
//!
//! ```text
//! cargo test -p finstack-quant-valuations --test instruments \
//!   cds_option::test_cdx_ig_46_cdso_diagnostics::diag_cdx_ig_46_cdso_internals \
//!   -- --exact --include-ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_valuations::calibration::api::engine;
use finstack_quant_valuations::calibration::api::schema::CalibrationEnvelope;
use finstack_quant_valuations::calibration::bumps::{
    bump_hazard_spreads, bump_hazard_spreads_with_doc_clause_and_valuation_convention, BumpRequest,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds::CdsValuationConvention;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::price_instrument_json_with_metrics_and_history;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use time::macros::date;

use finstack_quant_core::dates::{CalendarRegistry, DateExt};
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::Result;
use finstack_quant_valuations::calibration::bumps::bump_hazard_shift;
use finstack_quant_valuations::constants::{bloomberg_cdso, numerical};
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::bloomberg_quadrature::{
    calibrate_lognormal_mean, index_option_front_end_protection_start, normal_integral, npv,
    price_with_calibrated_mean, quadrature_payoff, theta, z_limit, ForwardCdsContext,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;

const DIAG_G_DAYS_IN_YEAR: f64 = 365.0;
const DIAG_THETA_DAYS_IN_YEAR: f64 = 365.25;

const FIXTURE: &str = "tests/golden/data/pricing/cds_option/cdx_ig_46_payer_atm_jun26.json";

const BBG_NPV: f64 = 118_781.76;
const BBG_PAR_BP: f64 = 55.2848;
const BBG_VEGA: f64 = 3411.78;
const BBG_CS01: f64 = 25_352.02;
const BBG_THETA: f64 = -1499.93;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

fn load_fixture_json() -> Value {
    let raw = fs::read_to_string(fixture_path()).expect("read fixture");
    serde_json::from_str(&raw).expect("parse fixture")
}

fn fixture_market_envelope(fixture: &Value) -> &Value {
    &fixture["market"]["envelope"]
}

fn fixture_instrument(fixture: &Value) -> &Value {
    &fixture["instrument"]
}

fn fixture_instrument_spec(fixture: &Value) -> &Value {
    &fixture["instrument"]["spec"]
}

fn snapshot_curves(as_of: Date) -> MarketContext {
    let disc_knots: Vec<(f64, f64)> = vec![
        (0.0, 1.0),
        (0.083_333_333_333_333_33, 0.996_974_336_6),
        (0.166_666_666_666_666_66, 0.993_945_403_4),
        (0.25, 0.990_919_479_2),
        (0.5, 0.981_857_591_5),
        (1.0, 0.963_444_880_8),
        (2.0, 0.928_476_693_3),
        (3.0, 0.895_395_284_1),
        (4.0, 0.862_827_924_5),
        (5.0, 0.830_398_145_4),
        (6.0, 0.798_061_194_2),
        (7.0, 0.766_170_920_7),
        (8.0, 0.734_944_715_2),
        (9.0, 0.704_364_007_7),
        (10.0, 0.674_488_940_5),
    ];
    let disc = DiscountCurve::builder("USD-S531-SWAP-20260507")
        .base_date(as_of)
        .knots(disc_knots)
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("discount curve");

    let haz_knots: Vec<(f64, f64)> = vec![
        (0.630_555_555_555_555_5, 0.002_676_511_8),
        (1.136_111_111_111_111, 0.002_677_801_4),
        (2.152_777_777_777_777_7, 0.005_556_018_9),
        (3.166_666_666_666_666_5, 0.008_468_701_1),
        (4.180_555_555_555_555, 0.013_194_693_2),
        (5.194_444_444_444_445, 0.017_154_183_4),
        (7.225, 0.021_843_883_2),
        (10.269_444_444_444_444, 0.025_886_011_0),
    ];
    let par_knots: Vec<(f64, f64)> = vec![
        (0.5, 16.14),
        (1.0, 16.14),
        (2.0, 24.14),
        (3.0, 32.36),
        (4.0, 42.9932),
        (5.0, 53.6264),
        (7.0, 72.64),
        (10.0, 92.35),
    ];
    let hazard = HazardCurve::builder("CDX-NA-IG-46-CBBT")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .recovery_rate(0.4)
        .knots(haz_knots)
        .par_spreads(par_knots)
        .build()
        .expect("hazard curve");

    MarketContext::new().insert(disc).insert(hazard)
}

fn bootstrap_market(fixture: &Value) -> MarketContext {
    let envelope: CalibrationEnvelope =
        serde_json::from_value(fixture_market_envelope(fixture).clone()).expect("parse envelope");
    let result = engine::execute_with_diagnostics(&envelope).expect("calibrate");
    MarketContext::try_from(result.result.final_market).expect("rehydrate market")
}

fn load_option(fixture: &Value) -> CDSOption {
    let spec = fixture_instrument_spec(fixture);
    serde_json::from_value(spec.clone()).expect("parse cds option spec")
}

fn metrics(option: &CDSOption, market: &MarketContext, as_of: Date) -> (f64, f64, f64, f64, f64) {
    let result = option
        .price_with_metrics(
            market,
            as_of,
            &[
                MetricId::ParSpread,
                MetricId::Vega,
                MetricId::Cs01,
                MetricId::Theta,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    (
        result.value.amount(),
        *result.measures.get("par_spread").unwrap(),
        *result.measures.get("vega").unwrap(),
        *result.measures.get("cs01").unwrap(),
        *result.measures.get("theta").unwrap(),
    )
}

fn central_cs01(pv_up: f64, pv_down: f64, bump_bp: f64) -> f64 {
    (pv_up - pv_down) / (2.0 * bump_bp)
}

#[test]
fn cdx_ig_46_production_integrand_converges_at_quadrature_step() {
    let fixture = load_fixture_json();
    let as_of = date!(2026 - 05 - 07);
    let market = bootstrap_market(&fixture);
    let option = load_option(&fixture);
    let ctx = context_for(&option, &market, as_of, 0.3603);
    let m = calibrate_lognormal_mean(&ctx).expect("calibrate lognormal mean");
    let t_expiry = ctx.t_expiry.max(0.0);
    let s0 = (-0.5 * ctx.sigma * ctx.sigma * t_expiry).exp();
    let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();
    let integrand = |z: f64| {
        let s = m * s0 * (sigma_sqrt_t * z).exp();
        ctx.swap_value_per_n(s)
    };

    let production = normal_integral(
        bloomberg_cdso::Z_STEP,
        z_limit(ctx.sigma, t_expiry),
        integrand,
    );
    let fine = normal_integral(
        bloomberg_cdso::Z_STEP * 0.5,
        z_limit(ctx.sigma, t_expiry),
        integrand,
    );
    let dollar_diff = (production - fine).abs() * option.notional.amount();

    assert!(
        dollar_diff < 0.01,
        "production quadrature grid should be sub-cent stable on V_te(s): diff=${dollar_diff:.8}",
    );
}

#[test]
fn cdx_ig_46_reported_npv_uses_supplied_curve_not_zero_rebootstrap() {
    let fixture = load_fixture_json();
    let as_of = date!(2026 - 05 - 07);
    let market = bootstrap_market(&fixture);
    let option = load_option(&fixture);
    let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
    let supplied_pv = npv(&option, &cds, &market, 0.3603, as_of)
        .expect("supplied market npv")
        .amount();

    let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
    let zero_hazard = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard.as_ref(),
        &market,
        &BumpRequest::Parallel(0.0),
        Some(&option.discount_curve_id),
        Some(CdsDocClause::IsdaNa),
        Some(CdsValuationConvention::BloombergCdswClean),
    )
    .expect("zero-bump hazard rebootstrap");
    let zero_market = market.insert(zero_hazard);
    let zero_pv = npv(&option, &cds, &zero_market, 0.3603, as_of)
        .expect("zero-bump market npv")
        .amount();

    // $6 band: matches the golden fixture tolerance. Removing the ARRC 2-day
    // lookback from cleared-OIS presets (2026-06 moderate-fix pass) shifted
    // the bootstrapped USD swap curve, leaving a documented -$5.32 residual
    // versus the Bloomberg screen value.
    assert!(
        (supplied_pv - BBG_NPV).abs() < 6.0,
        "reported NPV should remain anchored to the supplied fixture market: supplied={supplied_pv}, target={BBG_NPV}",
    );
    assert!(
        (zero_pv - supplied_pv).abs() > 100.0,
        "zero-bump rebootstrap drift is intentional sensitivity-path behavior, not reported NPV behavior: supplied={supplied_pv}, zero={zero_pv}",
    );
}

fn print_row(label: &str, npv: f64, par: f64, vega: f64, cs01: f64, theta: f64) {
    eprintln!(
        "{label:<42} npv={npv:>12.2} (d={:>+8.2})  par={par:>10.4} (d={:>+7.4})  \
         vega={vega:>8.2} (d={:>+6.2})  cs01={cs01:>10.2} (d={:>+7.2})  theta={theta:>10.2} (d={:>+7.2})",
        npv - BBG_NPV,
        par - BBG_PAR_BP,
        vega - BBG_VEGA,
        cs01 - BBG_CS01,
        theta - BBG_THETA,
    );
}

#[test]
#[ignore = "diagnostic: cdx_ig_46 CDSO convention probe"]
fn diag_cdx_ig_46_cdso_internals() {
    let fixture = load_fixture_json();
    let as_of = date!(2026 - 05 - 07);
    let bootstrap = bootstrap_market(&fixture);
    let snapshot = snapshot_curves(as_of);
    let base = load_option(&fixture);

    eprintln!("\n=== CDSO cdx_ig_46 convention probe (as_of={as_of}) ===");
    eprintln!(
        "  legal expiry={}  exercise_settlement={:?}  cash_settlement={:?}",
        base.expiry, base.exercise_settlement_date, base.cash_settlement_date
    );

    eprintln!("\n--- Curve source ---");
    let m = metrics(&base, &bootstrap, as_of);
    print_row("bootstrap (fixture envelope)", m.0, m.1, m.2, m.3, m.4);
    let s = metrics(&base, &snapshot, as_of);
    print_row("snapshot (hand-entered knots)", s.0, s.1, s.2, s.3, s.4);

    eprintln!("\n--- Date convention variants (bootstrap curves) ---");
    print_row("baseline", m.0, m.1, m.2, m.3, m.4);

    let mut no_exercise = base.clone();
    no_exercise.exercise_settlement_date = None;
    let v = metrics(&no_exercise, &bootstrap, as_of);
    print_row("exercise_settlement=None", v.0, v.1, v.2, v.3, v.4);

    let mut exercise_eq_expiry = base.clone();
    exercise_eq_expiry.exercise_settlement_date = Some(base.expiry);
    let v = metrics(&exercise_eq_expiry, &bootstrap, as_of);
    print_row("exercise_settlement=legal expiry", v.0, v.1, v.2, v.3, v.4);

    let mut expiry_eq_exercise = base.clone();
    expiry_eq_exercise.expiry = base.exercise_settlement_date.unwrap();
    let v = metrics(&expiry_eq_exercise, &bootstrap, as_of);
    print_row("legal expiry=exercise settlement", v.0, v.1, v.2, v.3, v.4);

    let mut cash_to_expiry = base.clone();
    cash_to_expiry.exercise_settlement_date = None;
    cash_to_expiry.cash_settlement_date = Some(as_of);
    let v = metrics(&cash_to_expiry, &bootstrap, as_of);
    print_row("Black t: as_of→legal expiry", v.0, v.1, v.2, v.3, v.4);

    eprintln!("\n--- CS01 rebootstrap probe (bootstrap curves) ---");
    let pv_base = base.value(&bootstrap, as_of).unwrap().amount();
    let hazard = bootstrap.get_hazard(&base.credit_curve_id).unwrap();
    for bump_bp in [0.0_f64, 0.25, 1.0] {
        let bumped = bump_hazard_spreads(
            hazard.as_ref(),
            &bootstrap,
            &BumpRequest::Parallel(bump_bp),
            Some(&base.discount_curve_id),
        )
        .unwrap();
        let pv_b = base
            .value(&bootstrap.clone().insert(bumped), as_of)
            .unwrap()
            .amount();
        let dpv = pv_b - pv_base;
        let per_bp = if bump_bp.abs() > 1e-12 {
            dpv / bump_bp
        } else {
            f64::NAN
        };
        eprintln!("  bump={bump_bp:>5.2} bp → ΔPV=${dpv:>12.2}  per-bp=${per_bp:>10.2}");
    }

    eprintln!("\n--- Golden runner path (sanity) ---");
    let instrument_json = fixture_instrument(&fixture).clone();
    let result = price_instrument_json_with_metrics_and_history(
        &serde_json::to_string(&instrument_json).unwrap(),
        &bootstrap,
        "2026-05-07",
        "bloomberg_cdso",
        &[
            "par_spread".to_string(),
            "vega".to_string(),
            "cs01".to_string(),
            "theta".to_string(),
        ],
        None,
        None,
    )
    .unwrap();
    print_row(
        "price_instrument_json_with_metrics_and_history",
        result.value.amount(),
        result.measures["par_spread"],
        result.measures["vega"],
        result.measures["cs01"],
        result.measures["theta"],
    );
}

#[test]
#[ignore = "diagnostic: CS01 rebootstrap anchor probe"]
fn diag_cdx_ig_46_cs01_rebootstrap_anchor() {
    let fixture = load_fixture_json();
    let as_of = date!(2026 - 05 - 07);
    let market = bootstrap_market(&fixture);
    let option = load_option(&fixture);
    let doc_clause = CdsDocClause::IsdaNa;
    let valuation_convention = CdsValuationConvention::BloombergCdswClean;

    let hazard = market.get_hazard(&option.credit_curve_id).unwrap();
    let discount_id = &option.discount_curve_id;
    let revalue = |ctx: &MarketContext| option.value(ctx, as_of).map(|m| m.amount());

    let pv_context_base = revalue(&market).unwrap();

    eprintln!("\n=== CS01 rebootstrap anchor probe (cdx_ig_46) ===");
    eprintln!("  Bloomberg Spread DV01 = ${BBG_CS01:.2}");

    let bump_up = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard.as_ref(),
        &market,
        &BumpRequest::Parallel(1.0),
        Some(discount_id),
        Some(doc_clause),
        Some(valuation_convention),
    )
    .unwrap();
    let bump_down = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard.as_ref(),
        &market,
        &BumpRequest::Parallel(-1.0),
        Some(discount_id),
        Some(doc_clause),
        Some(valuation_convention),
    )
    .unwrap();
    let pv_up_orig = revalue(&market.clone().insert(bump_up)).unwrap();
    let pv_down_orig = revalue(&market.clone().insert(bump_down)).unwrap();
    let cs01_orig = central_cs01(pv_up_orig, pv_down_orig, 1.0);
    eprintln!("\n--- Bump from in-context hazard ---");
    eprintln!("  PV(context base)           = ${pv_context_base:>12.2}");
    eprintln!(
        "  CS01 central (±1bp)        = ${cs01_orig:>12.2}  Δ vs BBG = {:+.2}",
        cs01_orig - BBG_CS01
    );

    let rebased_zero_from_orig = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard.as_ref(),
        &market,
        &BumpRequest::Parallel(0.0),
        Some(discount_id),
        Some(doc_clause),
        Some(valuation_convention),
    )
    .unwrap();
    let pv_rebased_zero = revalue(&market.clone().insert(rebased_zero_from_orig.clone())).unwrap();
    let offset = pv_rebased_zero - pv_context_base;
    eprintln!("  0bp rebootstrap offset     = ${offset:>12.2}  (PV@rebased0 − PV@context)");

    let market_rebased = market.clone().insert(rebased_zero_from_orig.clone());
    let hazard_rebased = market_rebased.get_hazard(&option.credit_curve_id).unwrap();
    let bump_up_rebased = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard_rebased.as_ref(),
        &market_rebased,
        &BumpRequest::Parallel(1.0),
        Some(discount_id),
        Some(doc_clause),
        Some(valuation_convention),
    )
    .unwrap();
    let bump_down_rebased = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard_rebased.as_ref(),
        &market_rebased,
        &BumpRequest::Parallel(-1.0),
        Some(discount_id),
        Some(doc_clause),
        Some(valuation_convention),
    )
    .unwrap();
    let pv_up_rebased = revalue(&market_rebased.clone().insert(bump_up_rebased)).unwrap();
    let pv_down_rebased = revalue(&market_rebased.clone().insert(bump_down_rebased)).unwrap();
    let cs01_rebased_anchor = central_cs01(pv_up_rebased, pv_down_rebased, 1.0);
    let pv_rebased_base = revalue(&market_rebased).unwrap();

    eprintln!("\n--- Rebootstrap 0bp → insert → bump ±1bp from rebased hazard ---");
    eprintln!("  PV(rebased base)           = ${pv_rebased_base:>12.2}");
    eprintln!(
        "  CS01 central (±1bp)        = ${cs01_rebased_anchor:>12.2}  Δ vs BBG = {:+.2}",
        cs01_rebased_anchor - BBG_CS01
    );

    eprintln!("\n--- One-sided +1bp from each base ---");
    eprintln!(
        "  context base   ΔPV=${:>12.2}",
        pv_up_orig - pv_context_base
    );
    eprintln!(
        "  rebased base   ΔPV=${:>12.2}",
        pv_up_rebased - pv_rebased_base
    );

    eprintln!("\n--- Hazard λ drift (context vs 0bp rebootstrap) ---");
    let rebased_knots: Vec<(f64, f64)> = rebased_zero_from_orig.knot_points().collect();
    for (t, lambda) in hazard.knot_points() {
        let rebased = rebased_knots
            .iter()
            .find(|(t_b, _)| (t_b - t).abs() < 1e-6)
            .map(|(_, l)| *l)
            .unwrap_or(f64::NAN);
        eprintln!(
            "  t={t:.4}  λ_ctx={lambda:.10}  λ_rebased0={rebased:.10}  Δλ={:+.2e}",
            rebased - lambda
        );
    }

    let orig_par: Vec<(f64, f64)> = hazard.par_spread_points().collect();
    let rebased_par: Vec<(f64, f64)> = rebased_zero_from_orig.par_spread_points().collect();
    let max_par_diff = orig_par
        .iter()
        .zip(rebased_par.iter())
        .map(|((_, a), (_, b))| (a - b).abs())
        .fold(0.0_f64, f64::max);
    eprintln!("\n  max |Δ par spread| after 0bp rebootstrap = {max_par_diff:.2e} bp");

    // Spot 5Y CDX CDS — same rebootstrap question as test_bloomberg_cdsw_parity.
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    const BBG_SPOT_CS01: f64 = 46_963.21;
    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "CDX-NA-IG-46-SPOT-CS01",
        Money::new(100_000_000.0, Currency::USD),
        100.0,
        date!(2026 - 03 - 20),
        date!(2031 - 06 - 20),
        "USD-S531-SWAP-20260507",
        "CDX-NA-IG-46-CBBT",
    )
    .expect("spot cds");
    cds.protection.recovery_rate = 0.4;
    cds.valuation_convention = CdsValuationConvention::BloombergCdswClean;
    let pv_cds = cds.value(&market, as_of).unwrap().amount();
    let revalue_cds = |ctx: &MarketContext| cds.value(ctx, as_of).map(|m| m.amount());

    let cds_up = revalue_cds(
        &market.clone().insert(
            bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                hazard.as_ref(),
                &market,
                &BumpRequest::Parallel(1.0),
                Some(discount_id),
                Some(doc_clause),
                Some(valuation_convention),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let cds_down = revalue_cds(
        &market.clone().insert(
            bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                hazard.as_ref(),
                &market,
                &BumpRequest::Parallel(-1.0),
                Some(discount_id),
                Some(doc_clause),
                Some(valuation_convention),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let cs01_cds_orig = central_cs01(cds_up, cds_down, 1.0);

    let cds_up_reb = revalue_cds(
        &market_rebased.clone().insert(
            bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                hazard_rebased.as_ref(),
                &market_rebased,
                &BumpRequest::Parallel(1.0),
                Some(discount_id),
                Some(doc_clause),
                Some(valuation_convention),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let cds_down_reb = revalue_cds(
        &market_rebased.clone().insert(
            bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                hazard_rebased.as_ref(),
                &market_rebased,
                &BumpRequest::Parallel(-1.0),
                Some(discount_id),
                Some(doc_clause),
                Some(valuation_convention),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let cs01_cds_rebased = central_cs01(cds_up_reb, cds_down_reb, 1.0);

    eprintln!("\n--- Spot 5Y CDX CDS Spread DV01 cross-check ---");
    eprintln!("  Bloomberg Spread DV01      = ${BBG_SPOT_CS01:.2}");
    eprintln!(
        "  CS01 context anchor        = ${cs01_cds_orig:.2}  Δ = {:+.2}",
        cs01_cds_orig - BBG_SPOT_CS01
    );
    eprintln!(
        "  CS01 rebased anchor        = ${cs01_cds_rebased:.2}  Δ = {:+.2}",
        cs01_cds_rebased - BBG_SPOT_CS01
    );
    eprintln!(
        "  0bp offset on spot CDS PV  = ${:.2}",
        revalue_cds(&market_rebased).unwrap() - pv_cds
    );
}

// Diagnostics moved out of bloomberg_quadrature.rs to keep production source focused.
fn calibrate_lognormal_mean_to_target_at(
    ctx: &ForwardCdsContext,
    target: f64,
    t_expiry: f64,
) -> Result<f64> {
    let t_expiry = t_expiry.max(0.0);
    let s0 = (-0.5 * ctx.sigma * ctx.sigma * t_expiry).exp();
    let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();
    let expected_v_te = |m: f64| -> f64 {
        normal_integral(bloomberg_cdso::Z_STEP, z_limit(ctx.sigma, t_expiry), |z| {
            let s = m * s0 * (sigma_sqrt_t * z).exp();
            ctx.swap_value_per_n(s)
        })
    };
    let f = |log_m: f64| -> f64 { expected_v_te(log_m.exp()) - target };
    const LOG_M_LO: f64 = -18.420_680_743_952_367;
    const LOG_M_HI: f64 = 4.605_170_185_988_092;
    const MAX_EXPANSIONS: usize = 30;
    let m_seed = ctx.forward_par_spread.clamp(1e-8, 100.0);
    let log_seed = m_seed.ln().clamp(LOG_M_LO, LOG_M_HI);
    let f_seed = f(log_seed);
    let (mut lo_x, mut lo_f) = (log_seed, f_seed);
    let (mut hi_x, mut hi_f) = (log_seed, f_seed);
    let mut bracket = None;
    let step = (2.0_f64).ln();
    for k in 1..=MAX_EXPANSIONS {
        let widen = step * (k as f64);
        let x_lo_new = (log_seed - widen).max(LOG_M_LO);
        let x_hi_new = (log_seed + widen).min(LOG_M_HI);
        if x_lo_new < lo_x {
            let f_new = f(x_lo_new);
            if f_new.is_finite() && f_new * lo_f <= 0.0 {
                bracket = Some((x_lo_new, lo_x));
                break;
            }
            lo_x = x_lo_new;
            lo_f = f_new;
        }
        if x_hi_new > hi_x {
            let f_new = f(x_hi_new);
            if f_new.is_finite() && f_new * hi_f <= 0.0 {
                bracket = Some((hi_x, x_hi_new));
                break;
            }
            hi_x = x_hi_new;
            hi_f = f_new;
        }
    }
    let Some((lo, hi)) = bracket else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "diagnostic calibration bracket violation: target={target}, t={t_expiry}, \
             seed={m_seed}, f_lo={lo_f:.6e}, f_hi={hi_f:.6e}",
        )));
    };
    let solver = BrentSolver::new().tolerance(1e-12);
    solver.solve_in_bracket(f, lo, hi).map(f64::exp)
}

fn cdx_fixture_internal() -> (CDSOption, MarketContext) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/data/pricing/cds_option/cdx_ig_46_payer_atm_jun26.json");
    let raw = fs::read_to_string(path).expect("read cdx fixture");
    let fixture: Value = serde_json::from_str(&raw).expect("parse fixture");
    let option: CDSOption = serde_json::from_value(fixture_instrument_spec(&fixture).clone())
        .expect("parse option spec");
    let envelope: CalibrationEnvelope =
        serde_json::from_value(fixture_market_envelope(&fixture).clone()).expect("parse envelope");
    let result = engine::execute_with_diagnostics(&envelope).expect("calibrate market");
    let market = MarketContext::try_from(result.result.final_market).expect("market context");
    (option, market)
}

fn context_for(
    option: &CDSOption,
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

fn price_with_fep_split(ctx: &ForwardCdsContext, f0_fep: f64, d_fep: f64) -> Result<f64> {
    let m = calibrate_lognormal_mean_to_target_at(
        ctx,
        ctx.no_knockout_forward() + f0_fep,
        ctx.t_expiry,
    )?;
    let realized_loss = if ctx.is_index {
        ctx.realized_index_loss / ctx.scale.max(numerical::ZERO_TOLERANCE)
    } else {
        0.0
    };
    Ok(quadrature_payoff(
        ctx,
        m,
        ctx.signed_strike_adjustment_per_n(),
        ctx.sign() * (realized_loss + d_fep),
        ctx.t_expiry,
    ) * 100_000_000.0)
}

fn price_with_fep_split_at_t(
    ctx: &ForwardCdsContext,
    f0_fep: f64,
    d_fep: f64,
    t_expiry: f64,
) -> Result<f64> {
    let m =
        calibrate_lognormal_mean_to_target_at(ctx, ctx.no_knockout_forward() + f0_fep, t_expiry)?;
    let realized_loss = if ctx.is_index {
        ctx.realized_index_loss / ctx.scale.max(numerical::ZERO_TOLERANCE)
    } else {
        0.0
    };
    Ok(quadrature_payoff(
        ctx,
        m,
        ctx.signed_strike_adjustment_per_n(),
        ctx.sign() * (realized_loss + d_fep),
        t_expiry,
    ) * 100_000_000.0)
}

fn solve_d_fep_for_target(ctx: &ForwardCdsContext, f0_fep: f64, target: f64) -> Result<f64> {
    solve_d_fep_for_target_at(ctx, f0_fep, target, ctx.t_expiry)
}

fn solve_d_fep_for_target_at_t(
    ctx: &ForwardCdsContext,
    f0_fep: f64,
    target: f64,
    t_expiry: f64,
) -> Result<f64> {
    solve_d_fep_for_target_at(ctx, f0_fep, target, t_expiry)
}

fn solve_d_fep_for_target_at(
    ctx: &ForwardCdsContext,
    f0_fep: f64,
    target: f64,
    t_expiry: f64,
) -> Result<f64> {
    let mut lo = -0.002;
    let mut hi = 0.002;
    let mut f_lo = price_with_fep_split_at_t(ctx, f0_fep, lo, t_expiry)? - target;
    let f_hi = price_with_fep_split_at_t(ctx, f0_fep, hi, t_expiry)? - target;
    assert!(
        f_lo * f_hi <= 0.0,
        "target not bracketed for d_fep/t solve: f0_fep={f0_fep}, t={t_expiry}, f_lo={f_lo}, f_hi={f_hi}",
    );
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        let f_mid = price_with_fep_split_at_t(ctx, f0_fep, mid, t_expiry)? - target;
        if f_mid.abs() < 1e-8 {
            return Ok(mid);
        }
        if f_lo * f_mid <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
            f_lo = f_mid;
        }
    }
    Ok(0.5 * (lo + hi))
}

#[test]
#[ignore = "diagnostic: cdx_ig_46 CDSO risk metric FEP placement"]
fn diag_cdx_ig_46_risk_fep_split() {
    const BBG_NPV: f64 = 118_781.76;
    const BBG_VEGA: f64 = 3_411.78;
    const BBG_CS01: f64 = 25_352.02;
    let as_of = date!(2026 - 05 - 07);
    let sigma = 0.3603;
    let (option, market) = cdx_fixture_internal();
    let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
    let ctx = context_for(&option, &market, as_of, sigma);
    let bumped_ctx = context_for(&option, &market, as_of, sigma + 0.01);
    let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
    let up_hazard = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard.as_ref(),
        &market,
        &BumpRequest::Parallel(1.0),
        Some(&option.discount_curve_id),
        Some(CdsDocClause::IsdaNa),
        Some(CdsValuationConvention::BloombergCdswClean),
    )
    .expect("up hazard");
    let down_hazard = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard.as_ref(),
        &market,
        &BumpRequest::Parallel(-1.0),
        Some(&option.discount_curve_id),
        Some(CdsDocClause::IsdaNa),
        Some(CdsValuationConvention::BloombergCdswClean),
    )
    .expect("down hazard");
    let zero_hazard = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
        hazard.as_ref(),
        &market,
        &BumpRequest::Parallel(0.0),
        Some(&option.discount_curve_id),
        Some(CdsDocClause::IsdaNa),
        Some(CdsValuationConvention::BloombergCdswClean),
    )
    .expect("zero hazard");
    let up_market = market.clone().insert(up_hazard);
    let down_market = market.clone().insert(down_hazard);
    let zero_market = market.clone().insert(zero_hazard);
    let up_ctx = context_for(&option, &up_market, as_of, sigma);
    let down_ctx = context_for(&option, &down_market, as_of, sigma);

    eprintln!("\n=== cdx_ig_46 CDSO risk FEP split ===");
    eprintln!(
        "base fep(T+2->expiry)         = {:.12}",
        ctx.front_end_protection
    );
    eprintln!("target NPV / Vega / CS01      = {BBG_NPV:.4} / {BBG_VEGA:.4} / {BBG_CS01:.4}");
    eprintln!("alpha = fraction of FEP in F0; beta = fraction of FEP in D, solved to match NPV");
    for alpha in [0.0, 0.25, 0.50, 0.75, 1.0] {
        let f0_fep = alpha * ctx.front_end_protection;
        let d_fep = solve_d_fep_for_target(&ctx, f0_fep, BBG_NPV).expect("d solve");
        let beta = d_fep / ctx.front_end_protection;
        let npv = price_with_fep_split(&ctx, f0_fep, d_fep).expect("base price");
        let bumped = price_with_fep_split(
            &bumped_ctx,
            alpha * bumped_ctx.front_end_protection,
            beta * bumped_ctx.front_end_protection,
        )
        .expect("vega price");
        let vega = bumped - npv;

        let up = price_with_fep_split(
            &up_ctx,
            alpha * up_ctx.front_end_protection,
            beta * up_ctx.front_end_protection,
        )
        .expect("up price");
        let down = price_with_fep_split(
            &down_ctx,
            alpha * down_ctx.front_end_protection,
            beta * down_ctx.front_end_protection,
        )
        .expect("down price");
        let cs01 = central_cs01(up, down, 1.0);

        eprintln!(
            "alpha={alpha:.2} beta={beta:.6} npv={npv:.4} vega={vega:.4} d_vega={:+.4} cs01={cs01:.4} d_cs01={:+.4}",
            vega - BBG_VEGA,
            cs01 - BBG_CS01
        );
    }

    let production = npv(&option, &cds, &market, sigma, as_of)
        .expect("production npv")
        .amount();
    let production_vega = npv(&option, &cds, &market, sigma + 0.01, as_of)
        .expect("production bumped")
        .amount()
        - production;
    eprintln!("production npv/vega           = {production:.4} / {production_vega:.4}");
    let vega_central_half = npv(&option, &cds, &market, sigma + 0.005, as_of)
        .expect("vega central up half")
        .amount()
        - npv(&option, &cds, &market, sigma - 0.005, as_of)
            .expect("vega central down half")
            .amount();
    let vega_central_full = 0.5
        * (npv(&option, &cds, &market, sigma + 0.01, as_of)
            .expect("vega central up full")
            .amount()
            - npv(&option, &cds, &market, sigma - 0.01, as_of)
                .expect("vega central down full")
                .amount());
    eprintln!("vega central +/-0.5vp / +/-1vp = {vega_central_half:.4} / {vega_central_full:.4}");
    let fep_start = index_option_front_end_protection_start(&option, as_of).expect("fep start");
    eprintln!("\n-- t-expiry variants with D solved to target NPV --");
    for t_days in [42.0, 42.25, 42.5, 42.75, 43.0] {
        let t = t_days / DIAG_G_DAYS_IN_YEAR;
        let d_fep = solve_d_fep_for_target_at_t(&ctx, 0.0, BBG_NPV, t).expect("d/t solve");
        let base_t = price_with_fep_split_at_t(&ctx, 0.0, d_fep, t).expect("base t");
        let bumped_t = price_with_fep_split_at_t(&bumped_ctx, 0.0, d_fep, t).expect("bumped t");
        let beta_t = d_fep / ctx.front_end_protection;
        let cs_up_t =
            price_with_fep_split_at_t(&up_ctx, 0.0, beta_t * up_ctx.front_end_protection, t)
                .expect("cs up t");
        let cs_down_t =
            price_with_fep_split_at_t(&down_ctx, 0.0, beta_t * down_ctx.front_end_protection, t)
                .expect("cs down t");
        let cs_t = central_cs01(cs_up_t, cs_down_t, 1.0);
        let m_t = calibrate_lognormal_mean_to_target_at(&ctx, ctx.no_knockout_forward(), t)
            .expect("theta m/t");
        let theta_t = quadrature_payoff(
            &ctx,
            m_t,
            ctx.signed_strike_adjustment_per_n(),
            ctx.sign() * d_fep,
            (t - (1.0 / DIAG_THETA_DAYS_IN_YEAR)).max(0.0),
        ) * option.notional.amount()
            - base_t;
        eprintln!(
            "t_days={t_days:.2} d_fep={d_fep:.12} vega={:.4} d_vega={:+.4} cs01={cs_t:.4} d_cs01={:+.4} theta={theta_t:.4} d_theta={:+.4}",
            bumped_t - base_t,
            bumped_t - base_t - BBG_VEGA,
            cs_t - BBG_CS01,
            theta_t + 1_499.93
        );
    }
    for start in [
        fep_start,
        option
            .effective_cash_settlement_date(as_of)
            .expect("cash date"),
        option
            .effective_cash_settlement_date(as_of)
            .expect("cash date")
            .add_business_days(
                2,
                CalendarRegistry::global()
                    .resolve_str(option.underlying_convention.default_calendar())
                    .expect("calendar"),
            )
            .expect("cash plus two"),
    ] {
        let sp_s = hazard.sp_on_date(start).expect("start sp");
        let sp_e = hazard.sp_on_date(option.expiry).expect("expiry sp");
        let d = ctx.lgd * (1.0 - (sp_e / sp_s).clamp(0.0, 1.0));
        let t = 42.5 / DIAG_G_DAYS_IN_YEAR;
        let p = price_with_fep_split_at_t(&ctx, 0.0, d, t).expect("date/t price");
        let v = price_with_fep_split_at_t(&bumped_ctx, 0.0, d, t).expect("date/t bumped") - p;
        eprintln!(
            "t_days=42.50 fep_start={start} d_fep={d:.12} npv={p:.4} d_npv={:+.4} vega={v:.4} d_vega={:+.4}",
            p - BBG_NPV,
            v - BBG_VEGA
        );
    }

    let prod_up = price_with_fep_split(&up_ctx, 0.0, up_ctx.front_end_protection)
        .expect("production up custom");
    let prod_down = price_with_fep_split(&down_ctx, 0.0, down_ctx.front_end_protection)
        .expect("production down custom");
    let zero = npv(&option, &cds, &zero_market, sigma, as_of)
        .expect("zero rebootstrap")
        .amount();
    let prod_held_fep_up =
        price_with_fep_split(&up_ctx, 0.0, ctx.front_end_protection).expect("held-fep up");
    let prod_held_fep_down =
        price_with_fep_split(&down_ctx, 0.0, ctx.front_end_protection).expect("held-fep down");
    let direct_up_market = market.clone().insert(
        bump_hazard_shift(hazard.as_ref(), &BumpRequest::Parallel(1.0)).expect("direct up hazard"),
    );
    let direct_down_market = market.clone().insert(
        bump_hazard_shift(hazard.as_ref(), &BumpRequest::Parallel(-1.0))
            .expect("direct down hazard"),
    );
    let direct_up = npv(&option, &cds, &direct_up_market, sigma, as_of)
        .expect("direct up")
        .amount();
    let direct_down = npv(&option, &cds, &direct_down_market, sigma, as_of)
        .expect("direct down")
        .amount();
    eprintln!("\n-- cs01 conventions (production FEP placement) --");
    eprintln!(
        "rebootstrap central           = {:.4}",
        central_cs01(prod_up, prod_down, 1.0)
    );
    eprintln!(
        "rebootstrap one-sided up      = {:.4}",
        prod_up - production
    );
    eprintln!(
        "rebootstrap one-sided down    = {:.4}",
        production - prod_down
    );
    eprintln!(
        "rebootstrap zero-base pv      = {zero:.4}  drift={:+.4}",
        zero - production
    );
    eprintln!("rebootstrap up from zero      = {:.4}", prod_up - zero);
    eprintln!("rebootstrap down from zero    = {:.4}", zero - prod_down);
    eprintln!(
        "rebootstrap central held FEP  = {:.4}",
        central_cs01(prod_held_fep_up, prod_held_fep_down, 1.0)
    );
    eprintln!(
        "direct hazard central         = {:.4}",
        central_cs01(direct_up, direct_down, 1.0)
    );
    for convention in [
        CdsValuationConvention::BloombergCdswCleanFullPremium,
        CdsValuationConvention::QuantLibIsdaParity,
    ] {
        let up_alt = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard.as_ref(),
            &market,
            &BumpRequest::Parallel(1.0),
            Some(&option.discount_curve_id),
            Some(CdsDocClause::IsdaNa),
            Some(convention),
        )
        .expect("alt up hazard");
        let down_alt = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard.as_ref(),
            &market,
            &BumpRequest::Parallel(-1.0),
            Some(&option.discount_curve_id),
            Some(CdsDocClause::IsdaNa),
            Some(convention),
        )
        .expect("alt down hazard");
        let up_alt_market = market.clone().insert(up_alt);
        let down_alt_market = market.clone().insert(down_alt);
        let up_alt_pv = npv(&option, &cds, &up_alt_market, sigma, as_of)
            .expect("alt up pv")
            .amount();
        let down_alt_pv = npv(&option, &cds, &down_alt_market, sigma, as_of)
            .expect("alt down pv")
            .amount();
        eprintln!(
            "rebootstrap central {:?} = {:.4}",
            convention,
            central_cs01(up_alt_pv, down_alt_pv, 1.0)
        );
    }

    let m = calibrate_lognormal_mean(&ctx).expect("theta calibration");
    let shortened_t = (ctx.t_expiry - (1.0 / DIAG_THETA_DAYS_IN_YEAR)).max(0.0);
    let shortened_expiry = option.expiry - time::Duration::days(1);
    let sp_start = hazard
        .sp_on_date(fep_start)
        .expect("sp start")
        .max(numerical::ZERO_TOLERANCE);
    let sp_short = hazard
        .sp_on_date(shortened_expiry)
        .expect("sp shortened expiry")
        .clamp(0.0, 1.0);
    let shortened_fep = ctx.lgd * (1.0 - (sp_short / sp_start).clamp(0.0, 1.0));
    let t_start = hazard
        .day_count()
        .year_fraction(
            hazard.base_date(),
            fep_start,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .expect("fep start time");
    let t_expiry_hazard = hazard
        .day_count()
        .year_fraction(
            hazard.base_date(),
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .expect("expiry hazard time");
    let sp_short_frac = hazard.sp((t_expiry_hazard - (1.0 / DIAG_THETA_DAYS_IN_YEAR)).max(t_start));
    let shortened_fep_frac = ctx.lgd * (1.0 - (sp_short_frac / sp_start).clamp(0.0, 1.0));
    let theta_current = theta(&option, &cds, &market, sigma, as_of).expect("theta");
    let cds_tomorrow =
        synthetic_underlying_cds(&option, as_of + time::Duration::days(1)).expect("tom cds");
    let theta_asof_shift = npv(
        &option,
        &cds_tomorrow,
        &market,
        sigma,
        as_of + time::Duration::days(1),
    )
    .expect("asof shift theta")
    .amount()
        - production;
    let theta_fep_horizon = (quadrature_payoff(
        &ctx,
        m,
        ctx.signed_strike_adjustment_per_n(),
        ctx.sign() * shortened_fep,
        shortened_t,
    ) * option.notional.amount())
        - production;
    eprintln!("\n-- theta conventions --");
    eprintln!("pure t shift current          = {theta_current:.4}");
    eprintln!("as-of +1 calendar revalue     = {theta_asof_shift:.4}");
    eprintln!(
        "pure t + shortened FEP end    = {theta_fep_horizon:.4}  fep_short={shortened_fep:.12}"
    );
    let theta_fep_horizon_frac = (quadrature_payoff(
        &ctx,
        m,
        ctx.signed_strike_adjustment_per_n(),
        ctx.sign() * shortened_fep_frac,
        shortened_t,
    ) * option.notional.amount())
        - production;
    eprintln!(
        "pure t + fractional FEP end   = {theta_fep_horizon_frac:.4}  fep_short={shortened_fep_frac:.12}"
    );
}

#[test]
#[ignore = "diagnostic: exact cdx_ig_46 CDSO NPV decomposition"]
fn diag_cdx_ig_46_npv_decomposition() {
    let as_of = date!(2026 - 05 - 07);
    let (option, market) = cdx_fixture_internal();
    let sigma = 0.3603;
    let ctx = context_for(&option, &market, as_of, sigma);
    let m = calibrate_lognormal_mean(&ctx).expect("base calibration");

    let h1 = if ctx.knockout {
        0.0
    } else {
        ctx.lgd * (1.0 - ctx.survival_to_expiry)
    };
    let h2 = (ctx.forward_par_spread - ctx.coupon) * ctx.bootstrapped_l_at_expiry;
    let f0 = ctx.no_knockout_forward();
    let h_k_flat = ctx.signed_strike_adjustment_per_n();
    let h_k_boot = ctx.sign() * (ctx.coupon - ctx.strike) * ctx.bootstrapped_l_at_expiry;

    let base = price_with_calibrated_mean(&ctx, m, ctx.t_expiry) * option.notional.amount();
    let no_fep = calibrate_lognormal_mean_to_target_at(&ctx, h2, ctx.t_expiry)
        .map(|m| price_with_calibrated_mean(&ctx, m, ctx.t_expiry) * option.notional.amount());
    let display_target =
        h1 + (ctx.display_forward_par_spread - ctx.coupon) * ctx.bootstrapped_l_at_expiry;
    let display_f0 = calibrate_lognormal_mean_to_target_at(&ctx, display_target, ctx.t_expiry)
        .map(|m| price_with_calibrated_mean(&ctx, m, ctx.t_expiry) * option.notional.amount());
    let boot_h = quadrature_payoff(
        &ctx,
        m,
        h_k_boot,
        ctx.signed_loss_settlement_per_n(),
        ctx.t_expiry,
    ) * option.notional.amount();
    let fep_as_payoff = quadrature_payoff(
        &ctx,
        m,
        h_k_flat,
        ctx.signed_loss_settlement_per_n() + h1,
        ctx.t_expiry,
    ) * option.notional.amount();
    let fep_as_payoff_boot_h = quadrature_payoff(
        &ctx,
        m,
        h_k_boot,
        ctx.signed_loss_settlement_per_n() + h1,
        ctx.t_expiry,
    ) * option.notional.amount();
    let fep_from = |start: Date, end: Date| -> f64 {
        let sp_start = market
            .get_hazard(&option.credit_curve_id)
            .expect("hazard")
            .sp_on_date(start)
            .unwrap_or(1.0)
            .max(numerical::ZERO_TOLERANCE);
        let sp_end = market
            .get_hazard(&option.credit_curve_id)
            .expect("hazard")
            .sp_on_date(end)
            .unwrap_or(1.0);
        ctx.lgd * (1.0 - (sp_end / sp_start).clamp(0.0, 1.0))
    };
    let fep_next_day = fep_from(as_of + time::Duration::days(1), option.expiry);
    let fep_t_plus_2 = fep_from(as_of + time::Duration::days(4), option.expiry);
    let fep_cash_settle = fep_from(
        option
            .effective_cash_settlement_date(as_of)
            .expect("cash settlement"),
        option.expiry,
    );
    let fep_next_day_price = quadrature_payoff(
        &ctx,
        m,
        h_k_flat,
        ctx.signed_loss_settlement_per_n() + fep_next_day,
        ctx.t_expiry,
    ) * option.notional.amount();
    let fep_t_plus_2_price = quadrature_payoff(
        &ctx,
        m,
        h_k_flat,
        ctx.signed_loss_settlement_per_n() + fep_t_plus_2,
        ctx.t_expiry,
    ) * option.notional.amount();
    let fep_cash_settle_price = quadrature_payoff(
        &ctx,
        m,
        h_k_flat,
        ctx.signed_loss_settlement_per_n() + fep_cash_settle,
        ctx.t_expiry,
    ) * option.notional.amount();
    let pure_forward_target = f0 - h1;
    let pure_forward_m =
        calibrate_lognormal_mean_to_target_at(&ctx, pure_forward_target, ctx.t_expiry)
            .expect("pure forward calibration");
    let pure_forward_plus_fep = quadrature_payoff(
        &ctx,
        pure_forward_m,
        h_k_flat,
        ctx.signed_loss_settlement_per_n() + h1,
        ctx.t_expiry,
    ) * option.notional.amount();
    let full_fep_m = calibrate_lognormal_mean_to_target_at(&ctx, f0 + h1, ctx.t_expiry)
        .expect("full fep calibration");
    let full_fep_flat_h =
        price_with_calibrated_mean(&ctx, full_fep_m, ctx.t_expiry) * option.notional.amount();
    let full_fep_boot_h = quadrature_payoff(
        &ctx,
        full_fep_m,
        h_k_boot,
        ctx.signed_loss_settlement_per_n(),
        ctx.t_expiry,
    ) * option.notional.amount();
    let legal_t = 41.0 / 365.0;
    let legal_t_price = price_with_calibrated_mean(&ctx, m, legal_t) * option.notional.amount();

    eprintln!("\n=== cdx_ig_46 CDSO NPV decomposition ===");
    eprintln!("target Bloomberg NPV          = 118781.76");
    eprintln!("base price                    = {base:.4}");
    eprintln!("base - target                 = {:+.4}", base - 118_781.76);
    eprintln!("\n-- context --");
    eprintln!("t_expiry                      = {:.10}", ctx.t_expiry);
    eprintln!("df_to_expiry                  = {:.10}", ctx.df_to_expiry);
    eprintln!(
        "survival_to_expiry            = {:.10}",
        ctx.survival_to_expiry
    );
    eprintln!("coupon                        = {:.8}", ctx.coupon);
    eprintln!("strike                        = {:.8}", ctx.strike);
    eprintln!(
        "economic forward par bp       = {:.8}",
        ctx.forward_par_spread * 10_000.0
    );
    eprintln!(
        "display forward par bp        = {:.8}",
        ctx.display_forward_par_spread * 10_000.0
    );
    eprintln!(
        "bootstrapped L                = {:.10}",
        ctx.bootstrapped_l_at_expiry
    );
    eprintln!(
        "flat L(strike)                = {:.10}",
        ctx.flat_annuity(ctx.strike)
    );
    eprintln!(
        "flat L(fwd)                   = {:.10}",
        ctx.flat_annuity(ctx.forward_par_spread)
    );
    eprintln!("\n-- calibration --");
    eprintln!("h1 FEP                        = {:.12}", h1);
    eprintln!("h2 (fwd-coupon)*L             = {:.12}", h2);
    eprintln!("F0 target                     = {:.12}", f0);
    eprintln!("m base bp                     = {:.8}", m * 10_000.0);
    eprintln!("H(K) flat                     = {:.12}", h_k_flat);
    eprintln!("H(K) boot L                   = {:.12}", h_k_boot);
    eprintln!("\n-- variants --");
    match no_fep {
        Ok(price) => {
            eprintln!(
                "no FEP target                 = {price:.4}  diff={:+.4}",
                price - 118_781.76
            );
        }
        Err(err) => {
            eprintln!("no FEP target                 = calibration failed: {err}");
        }
    }
    match display_f0 {
        Ok(price) => {
            eprintln!(
                "display F0 target             = {price:.4}  diff={:+.4}",
                price - 118_781.76
            );
        }
        Err(err) => {
            eprintln!("display F0 target             = calibration failed: {err}");
        }
    }
    eprintln!(
        "boot L in H(K)                = {boot_h:.4}  diff={:+.4}",
        boot_h - 118_781.76
    );
    eprintln!(
        "FEP as payoff D               = {fep_as_payoff:.4}  diff={:+.4}",
        fep_as_payoff - 118_781.76
    );
    eprintln!(
        "FEP D from as_of+1d           = {fep_next_day_price:.4}  diff={:+.4}  fep={:.12}",
        fep_next_day_price - 118_781.76,
        fep_next_day
    );
    eprintln!(
        "FEP D from T+2 calendar       = {fep_t_plus_2_price:.4}  diff={:+.4}  fep={:.12}",
        fep_t_plus_2_price - 118_781.76,
        fep_t_plus_2
    );
    eprintln!(
        "FEP D from cash settlement    = {fep_cash_settle_price:.4}  diff={:+.4}  fep={:.12}",
        fep_cash_settle_price - 118_781.76,
        fep_cash_settle
    );
    eprintln!(
        "FEP as D + boot L H(K)        = {fep_as_payoff_boot_h:.4}  diff={:+.4}",
        fep_as_payoff_boot_h - 118_781.76
    );
    eprintln!(
        "pure fwd F0 + FEP as D        = {pure_forward_plus_fep:.4}  diff={:+.4}",
        pure_forward_plus_fep - 118_781.76
    );
    eprintln!(
        "full FEP in F0 + flat H(K)    = {full_fep_flat_h:.4}  diff={:+.4}",
        full_fep_flat_h - 118_781.76
    );
    eprintln!(
        "full FEP in F0 + boot H(K)    = {full_fep_boot_h:.4}  diff={:+.4}",
        full_fep_boot_h - 118_781.76
    );
    eprintln!(
        "price with 41/365 t only      = {legal_t_price:.4}  diff={:+.4}",
        legal_t_price - 118_781.76
    );
}
