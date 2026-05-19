//! Ignored diagnostics for the `cdx_ig_46_payer_atm_jun26` Bloomberg CDSO golden.
//!
//! Probes settlement/time-axis conventions, CS01 bump mechanics, and
//! snapshot-vs-bootstrap curve construction. Run with:
//!
//! ```text
//! cargo test -p finstack-valuations --test instruments \
//!   cds_option::test_cdx_ig_46_cdso_diagnostics::diag_cdx_ig_46_cdso_internals \
//!   -- --exact --include-ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_core::dates::Date;
use finstack_core::dates::DayCount;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_core::math::interp::InterpStyle;
use finstack_valuations::calibration::api::engine;
use finstack_valuations::calibration::api::schema::CalibrationEnvelope;
use finstack_valuations::calibration::bumps::{
    bump_hazard_spreads, bump_hazard_spreads_with_doc_clause_and_valuation_convention, BumpRequest,
};
use finstack_valuations::instruments::credit_derivatives::cds::CdsValuationConvention;
use finstack_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::market::conventions::ids::CdsDocClause;
use finstack_valuations::metrics::MetricId;
use finstack_valuations::pricer::price_instrument_json_with_metrics;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use time::macros::date;

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
        serde_json::from_value(fixture["inputs"]["market_envelope"].clone())
            .expect("parse envelope");
    let result = engine::execute_with_diagnostics(&envelope).expect("calibrate");
    MarketContext::try_from(result.result.final_market).expect("rehydrate market")
}

fn load_option(fixture: &Value) -> CDSOption {
    let spec = &fixture["inputs"]["instrument_json"]["spec"];
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
            finstack_valuations::instruments::PricingOptions::default(),
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
    let instrument_json = fixture["inputs"]["instrument_json"].clone();
    let result = price_instrument_json_with_metrics(
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
    )
    .unwrap();
    print_row(
        "price_instrument_json_with_metrics",
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
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;
    const BBG_SPOT_CS01: f64 = 46_963.21;
    let mut cds = crate::finstack_test_utils::cds_buy_protection(
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
