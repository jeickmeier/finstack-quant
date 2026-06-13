//! QuantLib parity for **attribution decomposition** (not base valuation).
//!
//! Base-valuation parity (`Bond::value()` and central-difference DV01 vs
//! QuantLib) lives in
//! `finstack/valuations/tests/sanity_invariants/test_bond_quantlib_external_parity.rs`.
//! This file consumes the same shared fixtures and asserts that
//! `attribute_pnl_metrics_based`'s factor decomposition agrees with the
//! QuantLib-derived expected attribution:
//!
//! - `attribution.carry`            ≈  fixture `expected_attribution.carry_pnl`
//! - `attribution.rates_curves_pnl` ≈  fixture `expected_attribution.rates_pnl_first_order`
//! - sum of all factor P&Ls + residual ≡ `actual_pnl`
//!
//! ## Convention alignment
//!
//! The bond fixture is generated with 30/360 BondBasis, semi-annual, 2
//! settlement days — matching `Bond::fixed`'s defaults. The shared discount
//! curve is flat continuously-compounded; same on both sides.
//!
//! IRS and FX-forward attribution parity tests exercise the same metric-driven
//! attribution path against instrument-specific fixtures.

use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

use finstack_attribution::attribute_pnl_metrics_based;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::{Bond, Instrument, PricingOptions};
use finstack_valuations::metrics::MetricId;

/// Shared fixtures live with the base valuation parity tests in valuations;
/// reach across the workspace via `CARGO_MANIFEST_DIR/../valuations/...`.
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("valuations")
        .join("tests")
        .join("data")
        .join("quantlib_parity")
}

fn load_fixture<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = fixture_dir().join(name);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse fixture {path:?}: {e}"))
}

fn parse_iso_date(s: &str) -> Date {
    let fmt = time::format_description::well_known::Iso8601::DEFAULT;
    Date::parse(s, &fmt).expect("ISO date")
}

fn flat_discount_curve(id: &str, base: Date, rate: f64) -> DiscountCurve {
    let mut knots: Vec<(f64, f64)> = vec![(0.0_f64, 1.0_f64)];
    for &t in &[
        0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0, 40.0,
    ] {
        knots.push((t, (-rate * t).exp()));
    }
    DiscountCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Thirty360)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .expect("flat discount curve construction")
}

// ---------------------------------------------------------------------------
// Bond fixture shape (subset — base valuation fields are exercised by the
// valuations-side test; here we only need spec + scenario + expected_attribution)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct BondFixture {
    instrument: String,
    currency: String,
    spec: BondSpec,
    scenario: BondScenario,
    expected_attribution: BondExpectedAttribution,
}

#[derive(Debug, Deserialize)]
struct BondSpec {
    issue_date: String,
    maturity_date: String,
    face_amount: f64,
    coupon_rate: f64,
}

#[derive(Debug, Deserialize)]
struct BondScenario {
    t0: String,
    t1: String,
    rate_t0: f64,
    rate_t1: f64,
}

#[derive(Debug, Deserialize)]
struct BondExpectedAttribution {
    actual_pnl: f64,
    carry_pnl: f64,
    rates_pnl_first_order: f64,
    /// QL's first-order residual is mostly second-order convexity. The bond
    /// test bounds finstack's residual by it: the metrics path includes the
    /// convexity term, so its residual must not exceed QL's FIRST-order
    /// residual (plus tolerance) — a convexity-handling regression that
    /// inflates the residual while carry/rates stay in tolerance fails here.
    residual_first_order: f64,
}

/// Absolute tolerance on per-factor attribution components (USD per $100 face).
/// Wide enough to absorb minor differences in finstack's metric calculator
/// vs QL's analytical sensitivities; tight enough to catch real attribution
/// regressions.
const ATTR_FACTOR_TOLERANCE: f64 = 0.005;

#[test]
fn quantlib_parity_metrics_based_bond_attribution() {
    let fixture: BondFixture = load_fixture("bond_5pct_10y_usd.json");
    assert_eq!(fixture.instrument, "FixedRateBond");
    assert_eq!(fixture.currency, "USD");

    let t0 = parse_iso_date(&fixture.scenario.t0);
    let t1 = parse_iso_date(&fixture.scenario.t1);
    let issue = parse_iso_date(&fixture.spec.issue_date);
    let maturity = parse_iso_date(&fixture.spec.maturity_date);

    // Build the bond + flat-curve markets matching the fixture. `Bond::fixed`
    // defaults match the QL fixture's conventions; see the valuations-side
    // base-valuation parity test for confirmation that NPV agrees.
    let bond = Bond::fixed(
        "QL-PARITY-BOND",
        Money::new(fixture.spec.face_amount, Currency::USD),
        fixture.spec.coupon_rate,
        issue,
        maturity,
        "USD-OIS",
    )
    .expect("Bond::fixed");
    let instrument: Arc<dyn Instrument> = Arc::new(bond);

    let market_t0 =
        MarketContext::new().insert(flat_discount_curve("USD-OIS", t0, fixture.scenario.rate_t0));
    let market_t1 =
        MarketContext::new().insert(flat_discount_curve("USD-OIS", t1, fixture.scenario.rate_t1));

    // Compute T0 and T1 valuations with the metrics `attribute_pnl_metrics_based`
    // consumes (Theta, DV01, BucketedDv01, Convexity). The bond's pricer
    // populates these and the attribution function decomposes against them.
    let metrics = [
        MetricId::Theta,
        MetricId::Dv01,
        MetricId::BucketedDv01,
        MetricId::Convexity,
    ];
    let val_t0 = instrument
        .price_with_metrics(&market_t0, t0, &metrics, PricingOptions::default())
        .expect("price_with_metrics t0");
    let val_t1 = instrument
        .price_with_metrics(&market_t1, t1, &metrics, PricingOptions::default())
        .expect("price_with_metrics t1");

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        t0,
        t1,
    )
    .expect("metrics-based attribution");

    // ─── Parity assertions (the point of this test) ───────────────────────
    let exp = &fixture.expected_attribution;

    // Carry parity: finstack's `Theta × days` vs QL's 1-day theta.
    let carry_diff = (attribution.carry.amount() - exp.carry_pnl).abs();
    assert!(
        carry_diff < ATTR_FACTOR_TOLERANCE,
        "carry parity: finstack={}, ql_expected={}, diff={} > tol {}",
        attribution.carry.amount(),
        exp.carry_pnl,
        carry_diff,
        ATTR_FACTOR_TOLERANCE
    );

    // Rates parity: finstack's `DV01 × Δr_bp` (or key-rate sum) vs QL's
    // `dv01 × Δrate_bp`.
    let rates_diff = (attribution.rates_curves_pnl.amount() - exp.rates_pnl_first_order).abs();
    assert!(
        rates_diff < ATTR_FACTOR_TOLERANCE,
        "rates parity: finstack={}, ql_expected={}, diff={} > tol {}",
        attribution.rates_curves_pnl.amount(),
        exp.rates_pnl_first_order,
        rates_diff,
        ATTR_FACTOR_TOLERANCE
    );

    // Reconciliation: sum of attributed factors + residual ≡ total_pnl.
    // `metrics-based` total_pnl is the raw `val_t1 − val_t0` (plus any coupon
    // income, which is zero for this 1-day brand-new-bond scenario).
    let attributed_sum = attribution.carry.amount()
        + attribution.rates_curves_pnl.amount()
        + attribution.credit_curves_pnl.amount()
        + attribution.inflation_curves_pnl.amount()
        + attribution.correlations_pnl.amount()
        + attribution.fx_pnl.amount()
        + attribution.vol_pnl.amount()
        + attribution.cross_factor_pnl.amount()
        + attribution.model_params_pnl.amount()
        + attribution.market_scalars_pnl.amount();
    let recon = attributed_sum + attribution.residual.amount();
    let recon_err = (recon - attribution.total_pnl.amount()).abs();
    assert!(
        recon_err < 1e-9,
        "reconciliation: Σ factors + residual = {}, total_pnl = {}, err = {}",
        recon,
        attribution.total_pnl.amount(),
        recon_err
    );

    // Residual magnitude bound vs the QL fixture (quant review tests-12):
    // finstack's metrics path includes the convexity term, so its residual
    // must be no larger than QL's first-order residual plus the factor
    // tolerance (scaled to face value).
    assert!(
        attribution.residual.amount().abs()
            <= exp.residual_first_order.abs() + ATTR_FACTOR_TOLERANCE * 10_000.0,
        "bond residual ({}) must not exceed QL's first-order residual ({})",
        attribution.residual.amount(),
        exp.residual_first_order
    );

    // Sanity: finstack's total_pnl matches QL's actual_pnl within the
    // base-valuation tolerance pinned by the valuations test (which compares
    // npv to ~1e-2). Reassert here so attribution regressions that change
    // total_pnl computation are caught at this layer too.
    let total_diff = (attribution.total_pnl.amount() - exp.actual_pnl).abs();
    assert!(
        total_diff < 0.05,
        "total_pnl parity: finstack={}, ql_actual_pnl={}, diff={}",
        attribution.total_pnl.amount(),
        exp.actual_pnl,
        total_diff
    );
}

// ---------------------------------------------------------------------------
// IRS attribution parity
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct IrsFixture {
    instrument: String,
    currency: String,
    spec: IrsSpec,
    scenario: IrsScenario,
    expected_attribution: IrsExpectedAttribution,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct IrsSpec {
    trade_date: String,
    settlement_date: String,
    maturity_date: String,
    notional: f64,
    fixed_rate: f64,
}

#[derive(Debug, Deserialize)]
struct IrsScenario {
    t0: String,
    t1: String,
    rate_t0: f64,
    rate_t1: f64,
}

#[derive(Debug, Deserialize)]
struct IrsExpectedAttribution {
    actual_pnl: f64,
    carry_pnl: f64,
    rates_pnl_first_order: f64,
}

fn flat_forward_curve(
    id: &str,
    base: Date,
    rate: f64,
) -> finstack_core::market_data::term_structures::ForwardCurve {
    finstack_core::market_data::term_structures::ForwardCurve::builder(id, 0.25)
        .base_date(base)
        .knots([(0.0_f64, rate), (40.0_f64, rate)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("flat forward curve construction")
}

fn irs_market(as_of: Date, rate: f64) -> MarketContext {
    MarketContext::new()
        .insert(flat_discount_curve("USD-OIS", as_of, rate))
        .insert(flat_forward_curve("USD-SOFR-3M", as_of, rate))
}

/// Build a vanilla USD payer IRS matching the QL fixture spec. Inlined here
/// (rather than re-using `valuations::tests::support::test_utils::usd_irs_swap`)
/// because `finstack-attribution`'s dev-dependencies do not include the
/// valuations test crate.
fn build_irs(
    notional: f64,
    fixed_rate: f64,
    start: Date,
    end: Date,
) -> finstack_valuations::instruments::InterestRateSwap {
    use finstack_core::dates::{BusinessDayConvention, DayCount as DC, StubKind, Tenor};
    use finstack_core::decimal::f64_to_decimal;
    use finstack_core::types::{CurveId, InstrumentId};
    use finstack_valuations::instruments::rates::irs::{
        FixedLegSpec, FloatLegSpec, FloatingLegCompounding, PayReceive,
    };
    use finstack_valuations::instruments::InterestRateSwap;
    use rust_decimal::Decimal;

    let fixed = FixedLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        rate: f64_to_decimal(fixed_rate).expect("fixed rate decimal"),
        frequency: Tenor::semi_annual(),
        day_count: DC::Thirty360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".to_string()),
        stub: StubKind::None,
        start,
        end,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,
    };
    let float = FloatLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: Decimal::ZERO,
        frequency: Tenor::quarterly(),
        day_count: DC::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".to_string()),
        stub: StubKind::None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        start,
        end,
        compounding: FloatingLegCompounding::Simple,
        payment_lag_days: 0,
        end_of_month: false,
    };
    let swap = InterestRateSwap::builder()
        .id(InstrumentId::new("QL-PARITY-IRS"))
        .notional(Money::new(notional, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(fixed)
        .float(float)
        .build()
        .expect("InterestRateSwap::builder");
    swap.validate().expect("IRS validate");
    swap
}

/// Per-component IRS tolerances (quant review M14): a single $5k absolute
/// tolerance was vacuous — the fixture's carry is $566 and rates P&L $4,198,
/// so a zeroed or sign-flipped carry, and a fully dropped rates factor, all
/// passed. Factor P&Ls are first differences, largely immune to the NPV-level
/// schedule drift, so each component is bounded relative to its own expected
/// magnitude: carry within max($100, 25%); rates within max($250, 5%) —
/// consistent with the 5% relative DV01 drift documented in the valuations
/// parity test.
fn irs_carry_tolerance(expected: f64) -> f64 {
    (0.25 * expected.abs()).max(100.0)
}
fn irs_rates_tolerance(expected: f64) -> f64 {
    (0.05 * expected.abs()).max(250.0)
}

#[test]
fn quantlib_parity_metrics_based_irs_attribution() {
    let fixture: IrsFixture = load_fixture("irs_5y_usd.json");
    assert_eq!(fixture.instrument, "VanillaSwap");
    assert_eq!(fixture.currency, "USD");

    let t0 = parse_iso_date(&fixture.scenario.t0);
    let t1 = parse_iso_date(&fixture.scenario.t1);
    let settlement = parse_iso_date(&fixture.spec.settlement_date);
    let end = parse_iso_date(&fixture.spec.maturity_date);

    let swap = build_irs(
        fixture.spec.notional,
        fixture.spec.fixed_rate,
        settlement,
        end,
    );
    let instrument: Arc<dyn Instrument> = Arc::new(swap);

    let market_t0 = irs_market(t0, fixture.scenario.rate_t0);
    let market_t1 = irs_market(t1, fixture.scenario.rate_t1);

    let metrics = [
        MetricId::Theta,
        MetricId::Dv01,
        MetricId::BucketedDv01,
        MetricId::Convexity,
    ];
    let val_t0 = instrument
        .price_with_metrics(&market_t0, t0, &metrics, PricingOptions::default())
        .expect("price_with_metrics t0");
    let val_t1 = instrument
        .price_with_metrics(&market_t1, t1, &metrics, PricingOptions::default())
        .expect("price_with_metrics t1");

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        t0,
        t1,
    )
    .expect("metrics-based attribution");

    let exp = &fixture.expected_attribution;

    let carry_tol = irs_carry_tolerance(exp.carry_pnl);
    let carry_diff = (attribution.carry.amount() - exp.carry_pnl).abs();
    assert!(
        carry_diff < carry_tol,
        "IRS carry parity: finstack={}, ql_expected={}, diff={} > tol {}",
        attribution.carry.amount(),
        exp.carry_pnl,
        carry_diff,
        carry_tol
    );

    let rates_tol = irs_rates_tolerance(exp.rates_pnl_first_order);
    let rates_diff = (attribution.rates_curves_pnl.amount() - exp.rates_pnl_first_order).abs();
    assert!(
        rates_diff < rates_tol,
        "IRS rates parity: finstack={}, ql_expected={}, diff={} > tol {}",
        attribution.rates_curves_pnl.amount(),
        exp.rates_pnl_first_order,
        rates_diff,
        rates_tol
    );

    let total_diff = (attribution.total_pnl.amount() - exp.actual_pnl).abs();
    assert!(
        // Total carries the NPV-level schedule drift: max($1k, 0.05% notional).
        total_diff < (0.0005 * fixture.spec.notional).max(1_000.0),
        "IRS total_pnl parity: finstack={}, ql_actual_pnl={}, diff={}",
        attribution.total_pnl.amount(),
        exp.actual_pnl,
        total_diff
    );

    // Reconciliation invariant (previously missing on the IRS leg): the
    // attribution must internally close, Σ factors + residual ≡ total.
    let attributed_sum = attribution.carry.amount()
        + attribution.rates_curves_pnl.amount()
        + attribution.credit_curves_pnl.amount()
        + attribution.inflation_curves_pnl.amount()
        + attribution.correlations_pnl.amount()
        + attribution.fx_pnl.amount()
        + attribution.vol_pnl.amount()
        + attribution.cross_factor_pnl.amount()
        + attribution.model_params_pnl.amount()
        + attribution.market_scalars_pnl.amount();
    let recon_err =
        (attributed_sum + attribution.residual.amount() - attribution.total_pnl.amount()).abs();
    assert!(
        recon_err < 1e-6,
        "IRS reconciliation: Σ factors + residual must equal total, err={recon_err}"
    );
}

// ---------------------------------------------------------------------------
// FX-forward attribution parity
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct FxForwardFixture {
    instrument: String,
    currency: String,
    spec: FxForwardSpec,
    scenario: FxForwardScenario,
    expected_attribution: FxForwardExpectedAttribution,
}

#[derive(Debug, Deserialize)]
struct FxForwardSpec {
    base_ccy: String,
    quote_ccy: String,
    notional_base_ccy: f64,
    maturity_date: String,
    strike: f64,
}

#[derive(Debug, Deserialize)]
struct FxForwardScenario {
    t0: String,
    t1: String,
    spot_t0: f64,
    spot_t1: f64,
    r_usd_t0: f64,
    r_usd_t1: f64,
    r_eur_t0: f64,
    r_eur_t1: f64,
}

#[derive(Debug, Deserialize)]
struct FxForwardExpectedAttribution {
    actual_pnl: f64,
    carry_pnl: f64,
    usd_rate_pnl_first_order: f64,
    eur_rate_pnl_first_order: f64,
    fx_pnl_first_order: f64,
}

/// Fixed-rate FX provider for the EUR/USD market.
struct FixedEurUsd(f64);
impl finstack_core::money::fx::FxProvider for FixedEurUsd {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: finstack_core::money::fx::FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        if from == to {
            Ok(1.0)
        } else if from == Currency::EUR && to == Currency::USD {
            Ok(self.0)
        } else if from == Currency::USD && to == Currency::EUR {
            Ok(1.0 / self.0)
        } else {
            Err(finstack_core::Error::Validation(format!(
                "no rate {from}/{to}"
            )))
        }
    }
}

fn fx_market(as_of: Date, usd_rate: f64, eur_rate: f64, spot_eur_usd: f64) -> MarketContext {
    use finstack_core::money::fx::FxMatrix;
    MarketContext::new()
        .insert(flat_discount_curve("USD-OIS", as_of, usd_rate))
        .insert(flat_discount_curve("EUR-OIS", as_of, eur_rate))
        .insert_fx(FxMatrix::new(Arc::new(FixedEurUsd(spot_eur_usd))))
}

/// Total-P&L tolerance for the FX-forward parity (USD on $1.1M EUR notional).
/// FxForward uses the same CIRP formula as the QL fixture so the base PV is
/// essentially exact.
const FX_FWD_TOTAL_TOLERANCE_USD: f64 = 5.0;
/// Per-factor tolerance on linear FX-attribution components. The fixture's
/// `residual_first_order` is ~−$0.24 on a $533 total move (a 1-day FX
/// forward is nearly linear), so per-factor agreement is asserted tightly:
/// carry within $1, rate factors within $2, and the spot-dominated FX bucket
/// within 1% + $5 (routing between fx_pnl and market_scalars_pnl varies with
/// the metric the pricer reported).
const FX_FWD_FACTOR_TOLERANCE_USD: f64 = 5.0;

#[test]
fn quantlib_parity_metrics_based_fx_forward_attribution() {
    use finstack_valuations::instruments::Attributes;
    use finstack_valuations::instruments::FxForward;

    let fixture: FxForwardFixture = load_fixture("fx_forward_1y_eurusd.json");
    assert_eq!(fixture.instrument, "FxForward");
    assert_eq!(fixture.currency, "USD");
    assert_eq!(fixture.spec.base_ccy, "EUR");
    assert_eq!(fixture.spec.quote_ccy, "USD");

    let t0 = parse_iso_date(&fixture.scenario.t0);
    let t1 = parse_iso_date(&fixture.scenario.t1);
    let maturity = parse_iso_date(&fixture.spec.maturity_date);

    let forward = FxForward::builder()
        .id(finstack_core::types::InstrumentId::new("QL-PARITY-EURUSD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(fixture.spec.notional_base_ccy, Currency::EUR))
        .domestic_discount_curve_id(finstack_core::types::CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(finstack_core::types::CurveId::new("EUR-OIS"))
        .contract_rate_opt(Some(fixture.spec.strike))
        .attributes(Attributes::new())
        .build()
        .expect("FxForward::builder");
    let instrument: Arc<dyn Instrument> = Arc::new(forward);

    let market_t0 = fx_market(
        t0,
        fixture.scenario.r_usd_t0,
        fixture.scenario.r_eur_t0,
        fixture.scenario.spot_t0,
    );
    let market_t1 = fx_market(
        t1,
        fixture.scenario.r_usd_t1,
        fixture.scenario.r_eur_t1,
        fixture.scenario.spot_t1,
    );

    // BucketedDv01 is required for correct multi-curve attribution: the
    // aggregate Dv01 for an FX forward is a JOINT both-curves bump (≈ 0 by
    // construction, USD and EUR DV01s cancel), so the aggregate fallback
    // cannot attribute a single-curve move — the per-curve key-rate series
    // pairs each curve's DV01 with its own realized shift.
    let metrics = [
        MetricId::Theta,
        MetricId::Dv01,
        MetricId::BucketedDv01,
        MetricId::Delta,
        MetricId::Fx01,
    ];
    let val_t0 = instrument
        .price_with_metrics(&market_t0, t0, &metrics, PricingOptions::default())
        .expect("price_with_metrics t0");
    let val_t1 = instrument
        .price_with_metrics(&market_t1, t1, &metrics, PricingOptions::default())
        .expect("price_with_metrics t1");

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        t0,
        t1,
    )
    .expect("metrics-based attribution");

    let exp = &fixture.expected_attribution;

    // Total_pnl parity: the FX forward's PV is closed-form and should agree
    // tightly with QL.
    let total_diff = (attribution.total_pnl.amount() - exp.actual_pnl).abs();
    assert!(
        total_diff < FX_FWD_TOTAL_TOLERANCE_USD,
        "FxForward total_pnl: finstack={}, ql={}, diff={}",
        attribution.total_pnl.amount(),
        exp.actual_pnl,
        total_diff
    );

    // Reconciliation invariant: Σ factors + residual = total_pnl.
    let attributed_sum = attribution.carry.amount()
        + attribution.rates_curves_pnl.amount()
        + attribution.credit_curves_pnl.amount()
        + attribution.inflation_curves_pnl.amount()
        + attribution.correlations_pnl.amount()
        + attribution.fx_pnl.amount()
        + attribution.vol_pnl.amount()
        + attribution.cross_factor_pnl.amount()
        + attribution.model_params_pnl.amount()
        + attribution.market_scalars_pnl.amount();
    let recon = attributed_sum + attribution.residual.amount();
    let recon_err = (recon - attribution.total_pnl.amount()).abs();
    assert!(
        recon_err < 1e-6,
        "FxForward reconciliation: Σ factors + residual = {}, total = {}",
        recon,
        attribution.total_pnl.amount()
    );

    // Per-factor parity (quant review M13: these assertions were previously
    // discarded with `let _ = (...)`, and the fixture itself carried a
    // sign-flipped rate P&L rationalized as a "structural second-order
    // residual" — false for a 1-day FX forward, whose true first-order
    // residual is −$0.24).
    //
    // Carry: theta × 1 day, closed-form on both sides.
    let carry_diff = (attribution.carry.amount() - exp.carry_pnl).abs();
    assert!(
        carry_diff < 1.0,
        "FxForward carry parity: finstack={}, ql={}, diff={}",
        attribution.carry.amount(),
        exp.carry_pnl,
        carry_diff
    );

    // Rates: both curves feed the rates bucket; the fixture moves only USD
    // (+1bp) so the rates factor must match usd + eur (= usd + 0) tightly.
    let rates_expected = exp.usd_rate_pnl_first_order + exp.eur_rate_pnl_first_order;
    let rates_diff = (attribution.rates_curves_pnl.amount() - rates_expected).abs();
    assert!(
        rates_diff < 2.0,
        "FxForward rates parity: finstack={}, ql(usd+eur)={}, diff={}",
        attribution.rates_curves_pnl.amount(),
        rates_expected,
        rates_diff
    );

    // FX-component parity: the linear path captures Delta × Δspot + DV01 × Δrate
    // on the USD curve. finstack may route the spot-driven FX P&L through
    // either `fx_pnl` (Fx01 × Δfx) or `market_scalars_pnl` (Delta × Δspot)
    // depending on which metric the pricer reported, so the two are summed.
    let fx_total = attribution.fx_pnl.amount() + attribution.market_scalars_pnl.amount();
    let fx_total_diff = (fx_total - exp.fx_pnl_first_order).abs();
    let fx_tol = exp.fx_pnl_first_order.abs() * 0.01 + FX_FWD_FACTOR_TOLERANCE_USD;
    assert!(
        fx_total_diff < fx_tol,
        "FxForward FX-component parity: finstack(fx+scalars)={}, ql_fx_first_order={}, diff={} > tol {}",
        fx_total,
        exp.fx_pnl_first_order,
        fx_total_diff,
        fx_tol
    );

    // The fixture's own first-order residual must stay tiny — this pins the
    // generator's sign convention (a flipped rate sign inflates it to ~$213).
    let fixture_residual = exp.actual_pnl
        - exp.carry_pnl
        - exp.usd_rate_pnl_first_order
        - exp.eur_rate_pnl_first_order
        - exp.fx_pnl_first_order;
    assert!(
        fixture_residual.abs() < 1.0,
        "fixture first-order residual should be ~0 for a 1-day FX forward, got {fixture_residual}"
    );
}
