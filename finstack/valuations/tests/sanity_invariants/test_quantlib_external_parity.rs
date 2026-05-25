//! External QuantLib parity for vanilla bond, IRS, and FX-forward pricing.
//!
//! Loads `data/quantlib_parity/bond_5pct_10y_usd.json` (generated offline by
//! `scripts/generate_quantlib_fixture.py`) and asserts:
//!
//! - finstack `Bond::value()` matches QL `bond.NPV()` at T₀ and T₁ within $0.05
//!   on a $100 face (≈ 5 bp).
//! - finstack central-difference DV01 (±1bp shifts of the discount curve) is
//!   within 2% of QL's DV01.
//!
//! Convention alignment: the fixture is generated with 30/360 BondBasis,
//! semi-annual, 2 settlement days — the defaults of finstack's `Bond::fixed`
//! factory. No special builder calls are required on the Rust side.
//!
//! Companion attribution parity (carry + rates + residual via
//! `attribute_pnl_metrics_based`) lives in
//! `finstack/attribution/tests/attribution/quantlib_parity.rs`; both tests
//! consume the same shared fixture so the underlying scenario stays in sync.

use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

use crate::finstack_test_utils as test_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_core::money::Money;
use finstack_core::Result as CoreResult;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::fx::FxForward;
use finstack_valuations::instruments::rates::irs::PayReceive;
use finstack_valuations::instruments::Instrument;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/quantlib_parity")
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

/// Build a flat continuously-compounded discount curve at `rate`. Knot grid
/// spans 0..40y so the 10y bond is well within bounds.
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

#[derive(Debug, Deserialize)]
struct BondFixture {
    instrument: String,
    currency: String,
    spec: BondSpec,
    scenario: BondScenario,
    quantlib: BondQuantlib,
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
struct BondQuantlib {
    t0: BondQlSnapshot,
    t1: BondQlSnapshot,
}

#[derive(Debug, Deserialize)]
struct BondQlSnapshot {
    npv: f64,
    #[serde(default)]
    dv01: Option<f64>,
}

/// Tolerance on bond NPV (USD per $100 face) — generous enough to absorb
/// minor flat-curve interpolation differences between QL and finstack, tight
/// enough to catch real pricing bugs.
const BOND_NPV_TOLERANCE: f64 = 0.05;
/// Tolerance on bond DV01 (relative to QL's value).
const BOND_DV01_REL_TOLERANCE: f64 = 0.02;

#[test]
fn external_quantlib_parity_vanilla_bond_npv_and_dv01() {
    let fixture: BondFixture = load_fixture("bond_5pct_10y_usd.json");
    assert_eq!(fixture.instrument, "FixedRateBond");
    assert_eq!(fixture.currency, "USD");

    let t0 = parse_iso_date(&fixture.scenario.t0);
    let t1 = parse_iso_date(&fixture.scenario.t1);
    let issue = parse_iso_date(&fixture.spec.issue_date);
    let maturity = parse_iso_date(&fixture.spec.maturity_date);

    // `Bond::fixed` defaults: 30/360 BondBasis, semi-annual, 2 settlement
    // days — match the fixture's QuantLib conventions.
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

    // NPV parity at both dates.
    let pv_t0 = instrument.value(&market_t0, t0).expect("value t0");
    let pv_t1 = instrument.value(&market_t1, t1).expect("value t1");

    let dt0 = (pv_t0.amount() - fixture.quantlib.t0.npv).abs();
    let dt1 = (pv_t1.amount() - fixture.quantlib.t1.npv).abs();
    assert!(
        dt0 < BOND_NPV_TOLERANCE,
        "T0 NPV parity: finstack={}, quantlib={}, diff={} > tol {}",
        pv_t0.amount(),
        fixture.quantlib.t0.npv,
        dt0,
        BOND_NPV_TOLERANCE
    );
    assert!(
        dt1 < BOND_NPV_TOLERANCE,
        "T1 NPV parity: finstack={}, quantlib={}, diff={} > tol {}",
        pv_t1.amount(),
        fixture.quantlib.t1.npv,
        dt1,
        BOND_NPV_TOLERANCE
    );

    // DV01 parity via central-difference (±1bp shifts of the discount curve).
    // finstack's full DV01 metric pipeline is exercised elsewhere; here we
    // compare the raw sensitivity QL also computes.
    if let Some(ql_dv01) = fixture.quantlib.t0.dv01 {
        let bump_bp = 1.0_f64;
        let market_up = MarketContext::new().insert(flat_discount_curve(
            "USD-OIS",
            t0,
            fixture.scenario.rate_t0 + bump_bp * 1e-4,
        ));
        let market_dn = MarketContext::new().insert(flat_discount_curve(
            "USD-OIS",
            t0,
            fixture.scenario.rate_t0 - bump_bp * 1e-4,
        ));
        let pv_up = instrument.value(&market_up, t0).expect("value up");
        let pv_dn = instrument.value(&market_dn, t0).expect("value dn");
        let finstack_dv01 = (pv_up.amount() - pv_dn.amount()) / (2.0 * bump_bp);

        let rel = ((finstack_dv01 - ql_dv01) / ql_dv01).abs();
        assert!(
            rel < BOND_DV01_REL_TOLERANCE,
            "DV01 parity: finstack={}, quantlib={}, rel_diff={} > tol {}",
            finstack_dv01,
            ql_dv01,
            rel,
            BOND_DV01_REL_TOLERANCE
        );
    }
}

// ---------------------------------------------------------------------------
// IRS parity
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct IrsFixture {
    instrument: String,
    currency: String,
    spec: IrsSpec,
    scenario: IrsScenario,
    quantlib: IrsQuantlib,
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
struct IrsQuantlib {
    t0: IrsQlSnapshot,
    t1: IrsQlSnapshot,
}

#[derive(Debug, Deserialize)]
struct IrsQlSnapshot {
    npv: f64,
    #[serde(default)]
    dv01: Option<f64>,
}

fn flat_forward_curve(id: &str, base: Date, rate: f64) -> ForwardCurve {
    ForwardCurve::builder(id, 0.25)
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

/// IRS NPV tolerance (USD on a $10M notional, ≈0.15% of notional).
///
/// QL `MakeVanillaSwap` uses `Backward` date generation with a short front
/// stub by default and `USDLibor3M` fixings (Act/360, 2-day fixing lag);
/// finstack's `usd_irs_swap` helper uses `StubKind::None` and projects the
/// float leg purely from the `USD-SOFR-3M` forward curve. Even at matching
/// effective/maturity dates, the schedule's first coupon length differs
/// slightly, and the projected float rate at each coupon shows a small offset
/// from QL's index-fixing projection. For a flat curve at 5% the cumulative
/// drift is ~$10K on $10M notional (≈2% of NPV) — small enough to confirm
/// finstack's IRS pricer agrees with QL in approach but too large to assert
/// tight parity without bespoke schedule matching.
const IRS_NPV_TOLERANCE_USD: f64 = 15_000.0;
/// Relative tolerance on IRS DV01 (less stringent than bond DV01 — schedule
/// differences propagate through both the level and the sensitivity). The
/// same root cause as `IRS_NPV_TOLERANCE_USD` documented above.
const IRS_DV01_REL_TOLERANCE: f64 = 0.05;

#[test]
fn external_quantlib_parity_vanilla_irs_npv_and_dv01() {
    let fixture: IrsFixture = load_fixture("irs_5y_usd.json");
    assert_eq!(fixture.instrument, "VanillaSwap");
    assert_eq!(fixture.currency, "USD");

    let t0 = parse_iso_date(&fixture.scenario.t0);
    let t1 = parse_iso_date(&fixture.scenario.t1);
    // Use QL's spot-start convention: effective = T+2, maturity = effective+5y.
    // Pricing dates remain T0/T1; the schedule starts 2 business days later.
    let settlement = parse_iso_date(&fixture.spec.settlement_date);
    let end = parse_iso_date(&fixture.spec.maturity_date);

    let swap = test_utils::usd_irs_swap(
        "QL-PARITY-IRS",
        Money::new(fixture.spec.notional, Currency::USD),
        fixture.spec.fixed_rate,
        settlement,
        end,
        PayReceive::Pay,
    )
    .expect("usd_irs_swap");
    let instrument: Arc<dyn Instrument> = Arc::new(swap);

    let market_t0 = irs_market(t0, fixture.scenario.rate_t0);
    let market_t1 = irs_market(t1, fixture.scenario.rate_t1);

    let pv_t0 = instrument.value(&market_t0, t0).expect("value t0");
    let pv_t1 = instrument.value(&market_t1, t1).expect("value t1");

    let dt0 = (pv_t0.amount() - fixture.quantlib.t0.npv).abs();
    let dt1 = (pv_t1.amount() - fixture.quantlib.t1.npv).abs();
    assert!(
        dt0 < IRS_NPV_TOLERANCE_USD,
        "IRS T0 NPV: finstack={}, quantlib={}, diff={} > tol {}",
        pv_t0.amount(),
        fixture.quantlib.t0.npv,
        dt0,
        IRS_NPV_TOLERANCE_USD
    );
    assert!(
        dt1 < IRS_NPV_TOLERANCE_USD,
        "IRS T1 NPV: finstack={}, quantlib={}, diff={} > tol {}",
        pv_t1.amount(),
        fixture.quantlib.t1.npv,
        dt1,
        IRS_NPV_TOLERANCE_USD
    );

    if let Some(ql_dv01) = fixture.quantlib.t0.dv01 {
        let bump_bp = 1.0_f64;
        let market_up = irs_market(t0, fixture.scenario.rate_t0 + bump_bp * 1e-4);
        let market_dn = irs_market(t0, fixture.scenario.rate_t0 - bump_bp * 1e-4);
        let pv_up = instrument.value(&market_up, t0).expect("value up");
        let pv_dn = instrument.value(&market_dn, t0).expect("value dn");
        let finstack_dv01 = (pv_up.amount() - pv_dn.amount()) / (2.0 * bump_bp);
        let rel = ((finstack_dv01 - ql_dv01) / ql_dv01).abs();
        assert!(
            rel < IRS_DV01_REL_TOLERANCE,
            "IRS DV01: finstack={}, quantlib={}, rel_diff={} > tol {}",
            finstack_dv01,
            ql_dv01,
            rel,
            IRS_DV01_REL_TOLERANCE
        );
    }
}

// ---------------------------------------------------------------------------
// FX-forward parity
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct FxForwardFixture {
    instrument: String,
    currency: String,
    spec: FxForwardSpec,
    scenario: FxForwardScenario,
    quantlib: FxForwardQuantlib,
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
struct FxForwardQuantlib {
    t0: FxForwardQlSnapshot,
    t1: FxForwardQlSnapshot,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FxForwardQlSnapshot {
    npv_usd: f64,
}

/// Fixed-rate FX provider for parity tests.
struct FixedEurUsd(f64);
impl FxProvider for FixedEurUsd {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> CoreResult<f64> {
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
    MarketContext::new()
        .insert(flat_discount_curve("USD-OIS", as_of, usd_rate))
        .insert(flat_discount_curve("EUR-OIS", as_of, eur_rate))
        .insert_fx(FxMatrix::new(Arc::new(FixedEurUsd(spot_eur_usd))))
}

/// FX-forward NPV tolerance (USD on a $1M EUR notional).
const FX_FORWARD_NPV_TOLERANCE_USD: f64 = 5.0;

#[test]
fn external_quantlib_parity_fx_forward_npv() {
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
        .attributes(finstack_valuations::instruments::Attributes::new())
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

    let pv_t0 = instrument.value(&market_t0, t0).expect("value t0");
    let pv_t1 = instrument.value(&market_t1, t1).expect("value t1");
    assert_eq!(pv_t0.currency(), Currency::USD);

    let dt0 = (pv_t0.amount() - fixture.quantlib.t0.npv_usd).abs();
    let dt1 = (pv_t1.amount() - fixture.quantlib.t1.npv_usd).abs();
    assert!(
        dt0 < FX_FORWARD_NPV_TOLERANCE_USD,
        "FX-forward T0 NPV: finstack={}, quantlib={}, diff={} > tol {}",
        pv_t0.amount(),
        fixture.quantlib.t0.npv_usd,
        dt0,
        FX_FORWARD_NPV_TOLERANCE_USD
    );
    assert!(
        dt1 < FX_FORWARD_NPV_TOLERANCE_USD,
        "FX-forward T1 NPV: finstack={}, quantlib={}, diff={} > tol {}",
        pv_t1.amount(),
        fixture.quantlib.t1.npv_usd,
        dt1,
        FX_FORWARD_NPV_TOLERANCE_USD
    );
}
