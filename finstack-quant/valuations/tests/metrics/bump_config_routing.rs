//! Bump-size routing: every finite-difference greek honours `bump_config`.
//!
//! Each calculator resolves its spot, volatility and rate bumps from
//! `metric_pricing_overrides.bump_config` layered over the
//! `valuations.sensitivities.v1` extension. The tests below check three things:
//!
//! - With no override, the reported value is bit-identical to the value pinned
//!   before the bump sizes were routed (the defaults equal the old constants).
//! - With an override, the value equals a test-side re-run of the same
//!   finite-difference stencil at the overridden bump (relative 1e-12: the
//!   stencil performs the same floating-point operations, so only
//!   re-association noise remains).
//! - `bump_config` and the `valuations.sensitivities.v1` extension produce the
//!   same number for the same bump.
//!
//! Closed-form anchors: EquityOption vanna/volga against Black-Scholes and
//! FxOption volga against Garman-Kohlhagen, both at 5e-3 relative. A central
//! difference of width `h` carries an `O(h²)` truncation error; the equity
//! stencil stays inside 5e-3 at `h = 2%` spot and 2 vol points, and the FX
//! volga is Richardson-extrapolated from the `h` and `h/2` overrides.

use crate::metrics::date_support::date;
use crate::metrics::discount_forward_curve_support::flat_discount_with_tenor;
use crate::metrics::option_support::{equity_option_european_call, fx_option_european_call};
use crate::metrics::volatility_support::flat_vol_surface;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::BarrierType;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_valuations::instruments::exotics::lookback_option::{
    LookbackOption, LookbackType,
};
use finstack_quant_valuations::instruments::fx::quanto_option::QuantoOption;
use finstack_quant_valuations::instruments::{
    Attributes, EquityOption, FxBarrierOption, Instrument, InstrumentPricingOverrides,
    MetricPricingOverrides, Monitoring, OptionType, PricingOptions,
};
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;

// ---------------------------------------------------------------- helpers

fn measure<I: Instrument>(
    inst: &I,
    market: &MarketContext,
    as_of: Date,
    id: MetricId,
    opts: PricingOptions,
) -> f64 {
    let result = inst
        .price_with_metrics(market, as_of, std::slice::from_ref(&id), opts)
        .unwrap_or_else(|e| panic!("{id} should compute: {e}"));
    result
        .measures
        .get(&id)
        .copied()
        .unwrap_or_else(|| panic!("{id} missing from measures"))
}

fn pv<I: Instrument>(inst: &I, market: &MarketContext, as_of: Date) -> f64 {
    inst.value(market, as_of).expect("pv").amount()
}

fn assert_rel(label: &str, actual: f64, expected: f64, tol: f64) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tol * expected.abs().max(1.0),
        "{label}: expected {expected}, got {actual} (diff {diff}, rel tol {tol})"
    );
}

/// Compare every value with its pinned bit pattern and report all mismatches.
fn assert_pins(pins: &[(&str, f64, u64)]) {
    let mismatches: Vec<String> = pins
        .iter()
        .filter(|(_, value, bits)| value.to_bits() != *bits)
        .map(|(name, value, bits)| {
            format!(
                "{name}: got {value} (0x{:016x}), pinned {} (0x{bits:016x})",
                value.to_bits(),
                f64::from_bits(*bits)
            )
        })
        .collect();
    assert!(
        mismatches.is_empty(),
        "default outputs moved:\n{}",
        mismatches.join("\n")
    );
}

fn bump_price(market: &MarketContext, id: &str, pct: f64) -> MarketContext {
    let bumped = match market.get_price(id).expect("price scalar") {
        MarketScalar::Unitless(v) => MarketScalar::Unitless(v * (1.0 + pct)),
        MarketScalar::Price(m) => {
            MarketScalar::Price(Money::new(m.amount() * (1.0 + pct), m.currency()).expect("money"))
        }
    };
    market.clone().insert_price(id, bumped)
}

fn bump_vol(market: &MarketContext, id: &str, abs: f64) -> MarketContext {
    market
        .bump([MarketBump::Curve {
            id: CurveId::from(id),
            spec: BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: abs,
                bump_type: BumpType::Parallel,
            },
        }])
        .expect("vol bump")
}

fn bump_rate(market: &MarketContext, id: &str, bp: f64) -> MarketContext {
    market
        .bump([MarketBump::Curve {
            id: CurveId::from(id),
            spec: BumpSpec::parallel_bp(bp),
        }])
        .expect("rate bump")
}

fn spot_value(market: &MarketContext, id: &str) -> f64 {
    match market.get_price(id).expect("price scalar") {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    }
}

fn overrides(spot: Option<f64>, vol: Option<f64>, rate: Option<f64>) -> MetricPricingOverrides {
    let mut po = MetricPricingOverrides::default();
    if let Some(v) = spot {
        po = po.with_spot_bump_decimal(v);
    }
    if let Some(v) = vol {
        po = po.with_vol_bump_decimal(v);
    }
    if let Some(v) = rate {
        po = po.with_rate_bump_bp(v);
    }
    po
}

// ---------------------------------------------------------- EquityOption

const EQ_SPOT: f64 = 100.0;
const EQ_VOL: f64 = 0.20;
const EQ_RATE: f64 = 0.03;
const EQ_DIV: f64 = 0.01;

fn equity_market(as_of: Date) -> MarketContext {
    let expiries = [0.25, 0.5, 1.0, 2.0];
    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    MarketContext::new()
        .insert(flat_discount_with_tenor("USD-OIS", as_of, EQ_RATE, 5.0))
        .insert_surface(flat_vol_surface("EQUITY-VOL", &expiries, &strikes, EQ_VOL))
        .insert_price("EQUITY-SPOT", MarketScalar::Unitless(EQ_SPOT))
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(EQ_DIV))
}

fn equity_fixture_with_strike(strike: f64) -> (EquityOption, MarketContext, Date) {
    let as_of = date(2025, 1, 2);
    let opt = equity_option_european_call("EQ-ROUTING", "SPX", strike, date(2025, 7, 2), 100.0)
        .expect("equity option");
    (opt, equity_market(as_of), as_of)
}

fn equity_fixture() -> (EquityOption, MarketContext, Date) {
    equity_fixture_with_strike(100.0)
}

fn equity_delta_stencil(
    opt: &EquityOption,
    market: &MarketContext,
    as_of: Date,
    spot_bump: f64,
) -> f64 {
    let h = spot_value(market, "EQUITY-SPOT") * spot_bump;
    let up = pv(opt, &bump_price(market, "EQUITY-SPOT", spot_bump), as_of);
    let dn = pv(opt, &bump_price(market, "EQUITY-SPOT", -spot_bump), as_of);
    (up - dn) / (2.0 * h)
}

#[test]
fn equity_option_defaults_are_bit_identical() {
    let (opt, market, as_of) = equity_fixture();
    let d = PricingOptions::default;
    assert_pins(&[
        (
            "vanna",
            measure(&opt, &market, as_of, MetricId::Vanna, d()),
            0x3f3720cead800000,
        ),
        (
            "volga",
            measure(&opt, &market, as_of, MetricId::Volga, d()),
            0x3ef208f984000000,
        ),
        (
            "charm",
            measure(&opt, &market, as_of, MetricId::Charm, d()),
            0xc0142f1e783b1d00,
        ),
        (
            "speed",
            measure(&opt, &market, as_of, MetricId::Speed, d()),
            0xbfac6cb09b97e000,
        ),
        (
            "color",
            measure(&opt, &market, as_of, MetricId::Color, d()),
            0x40073fbf6d9ec400,
        ),
    ]);
}

#[test]
fn equity_option_vanna_volga_honour_bump_config() {
    // Out of the money: at the forward-ATM strike d2 ≈ 0, so vanna and volga
    // vanish and a relative comparison would test only finite-difference noise.
    let (mut opt, market, as_of) = equity_fixture_with_strike(110.0);
    let (h_s, h_v) = (0.02, 0.02);
    opt.metric_pricing_overrides = overrides(Some(h_s), Some(h_v), None);

    let vanna = measure(
        &opt,
        &market,
        as_of,
        MetricId::Vanna,
        PricingOptions::default(),
    );
    let volga = measure(
        &opt,
        &market,
        as_of,
        MetricId::Volga,
        PricingOptions::default(),
    );

    // Test-side stencil at the overridden bumps.
    let vol_up = bump_vol(&market, "EQUITY-VOL", h_v);
    let vol_dn = bump_vol(&market, "EQUITY-VOL", -h_v);
    let vanna_ref = (equity_delta_stencil(&opt, &vol_up, as_of, h_s)
        - equity_delta_stencil(&opt, &vol_dn, as_of, h_s))
        / (2.0 * h_v * 100.0);
    let width = h_v * 100.0;
    let volga_ref = (pv(&opt, &vol_up, as_of) - 2.0 * pv(&opt, &market, as_of)
        + pv(&opt, &vol_dn, as_of))
        / (width * width);
    assert_rel("vanna stencil", vanna, vanna_ref, 1e-12);
    assert_rel("volga stencil", volga, volga_ref, 1e-12);

    // Black-Scholes closed form, scaled to the option's quantity by PV ratio.
    let t = DayCount::Act365F
        .year_fraction(as_of, opt.expiry, DayCountContext::default())
        .expect("t");
    let df = market
        .get_discount("USD-OIS")
        .expect("curve")
        .df_between_dates(as_of, opt.expiry)
        .expect("df");
    let r = -df.ln() / t;
    let (s, k, q, sig) = (EQ_SPOT, opt.strike, EQ_DIV, EQ_VOL);
    let d1 = ((s / k).ln() + (r - q + 0.5 * sig * sig) * t) / (sig * t.sqrt());
    let d2 = d1 - sig * t.sqrt();
    let call = s * (-q * t).exp() * norm_cdf(d1) - k * df * norm_cdf(d2);
    let scale = pv(&opt, &market, as_of) / call;
    let bs_vanna = -(-q * t).exp() * norm_pdf(d1) * d2 / sig * 0.01 * scale;
    let bs_volga = s * (-q * t).exp() * norm_pdf(d1) * t.sqrt() * d1 * d2 / sig * 1e-4 * scale;
    assert_rel("vanna vs Black-Scholes", vanna, bs_vanna, 5e-3);
    assert_rel("volga vs Black-Scholes", volga, bs_volga, 5e-3);
}

#[test]
fn equity_option_charm_honours_explicit_spot_bump() {
    let (mut opt, market, as_of) = equity_fixture();
    let h_s = 0.02;
    opt.metric_pricing_overrides = overrides(Some(h_s), None, None);
    let charm = measure(
        &opt,
        &market,
        as_of,
        MetricId::Charm,
        PricingOptions::default(),
    );

    let rolled = as_of + time::Duration::days(1);
    let delta_now = equity_delta_stencil(&opt, &market, as_of, h_s);
    let delta_next = equity_delta_stencil(&opt, &market, rolled, h_s);
    let charm_ref = (delta_next - delta_now) / (1.0 / 365.0);
    assert_rel("charm stencil", charm, charm_ref, 1e-12);
}

#[test]
fn bump_config_and_sensitivities_extension_agree() {
    let (opt, market, as_of) = equity_fixture();
    let mut cfg = FinstackConfig::default();
    cfg.extensions
        .insert(
            "valuations.sensitivities.v1",
            serde_json::json!({"spot_bump_decimal": 0.02, "vol_bump_decimal": 0.02, "rate_bump_bp": 5.0}),
        )
        .expect("extension key");
    let via_ext = PricingOptions::default().with_config(&cfg);
    let mut via_po = opt.clone();
    via_po.metric_pricing_overrides = overrides(Some(0.02), Some(0.02), Some(5.0));

    for id in [MetricId::Vanna, MetricId::Volga, MetricId::Charm] {
        let ext = measure(&opt, &market, as_of, id.clone(), via_ext.clone());
        let po = measure(
            &via_po,
            &market,
            as_of,
            id.clone(),
            PricingOptions::default(),
        );
        assert_eq!(
            ext.to_bits(),
            po.to_bits(),
            "{id}: extension {ext} vs bump_config {po}"
        );
    }

    let (quanto, qmarket) = (quanto_option(), quanto_market());
    let mut quanto_po = quanto.clone();
    quanto_po.metric_pricing_overrides = overrides(None, None, Some(5.0));
    let ext = measure(&quanto, &qmarket, QUANTO_AS_OF, MetricId::Rho, via_ext);
    let po = measure(
        &quanto_po,
        &qmarket,
        QUANTO_AS_OF,
        MetricId::Rho,
        PricingOptions::default(),
    );
    assert_eq!(
        ext.to_bits(),
        po.to_bits(),
        "quanto rho: extension {ext} vs bump_config {po}"
    );
}

// -------------------------------------------------------------- FxOption

struct FlatFx(f64);
impl FxProvider for FlatFx {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        match (from, to) {
            (Currency::EUR, Currency::USD) => Ok(self.0),
            (Currency::USD, Currency::EUR) => Ok(1.0 / self.0),
            (a, b) if a == b => Ok(1.0),
            _ => Err(finstack_quant_core::Error::Validation("no fx".into())),
        }
    }
}

const FX_SPOT: f64 = 1.10;
const FX_VOL: f64 = 0.12;

fn fx_fixture_with_strike(
    strike: f64,
) -> (
    finstack_quant_valuations::instruments::FxOption,
    MarketContext,
    Date,
) {
    let as_of = date(2025, 1, 2);
    let expiries = [0.25, 0.5, 1.0, 2.0];
    let strikes = [0.9, 1.0, 1.1, 1.2, 1.3];
    let market = MarketContext::new()
        .insert_fx(FxMatrix::new(Arc::new(FlatFx(FX_SPOT))))
        .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.04, 5.0))
        .insert(flat_discount_with_tenor("EUR-OIS", as_of, 0.02, 5.0))
        .insert_surface(flat_vol_surface("EURUSD-VOL", &expiries, &strikes, FX_VOL));
    let opt = fx_option_european_call(
        "FX-ROUTING",
        Currency::EUR,
        Currency::USD,
        strike,
        date(2025, 7, 2),
        Money::new(1_000_000.0, Currency::EUR).expect("money"),
        "EURUSD-VOL",
    )
    .expect("fx option");
    (opt, market, as_of)
}

fn fx_fixture() -> (
    finstack_quant_valuations::instruments::FxOption,
    MarketContext,
    Date,
) {
    fx_fixture_with_strike(FX_SPOT)
}

#[test]
fn fx_option_defaults_are_bit_identical() {
    let (opt, market, as_of) = fx_fixture();
    let d = PricingOptions::default;
    assert_pins(&[
        (
            "vanna",
            measure(&opt, &market, as_of, MetricId::Vanna, d()),
            0xc0a34726bab18300,
        ),
        (
            "volga",
            measure(&opt, &market, as_of, MetricId::Volga, d()),
            0x4008886939fae200,
        ),
    ]);
}

#[test]
fn fx_option_vanna_volga_honour_bump_config() {
    // Out of the money: near the forward-ATM strike d1·d2 ≈ 0, so volga
    // vanishes and a relative comparison would test only finite-difference noise.
    let (mut opt, market, as_of) = fx_fixture_with_strike(1.25);
    let h_v = 0.02;
    opt.metric_pricing_overrides = overrides(None, Some(h_v), None);
    let vanna = measure(
        &opt,
        &market,
        as_of,
        MetricId::Vanna,
        PricingOptions::default(),
    );
    let volga = measure(
        &opt,
        &market,
        as_of,
        MetricId::Volga,
        PricingOptions::default(),
    );

    // Stencil: analytic delta/vega re-priced on the ±h vol markets.
    let up = bump_vol(&market, "EURUSD-VOL", h_v);
    let dn = bump_vol(&market, "EURUSD-VOL", -h_v);
    let d = PricingOptions::default;
    let vanna_ref = (measure(&opt, &up, as_of, MetricId::Delta, d())
        - measure(&opt, &dn, as_of, MetricId::Delta, d()))
        / (2.0 * h_v * 100.0);
    let volga_ref = (measure(&opt, &up, as_of, MetricId::Vega, d())
        - measure(&opt, &dn, as_of, MetricId::Vega, d()))
        / (2.0 * h_v)
        * 0.01;
    assert_rel("fx vanna stencil", vanna, vanna_ref, 1e-12);
    assert_rel("fx volga stencil", volga, volga_ref, 1e-12);

    // Garman-Kohlhagen volga per vol point², on the EUR notional.
    let t = DayCount::Act365F
        .year_fraction(as_of, opt.expiry, DayCountContext::default())
        .expect("t");
    let df_d = market
        .get_discount("USD-OIS")
        .expect("usd")
        .df_between_dates(as_of, opt.expiry)
        .expect("df");
    let df_f = market
        .get_discount("EUR-OIS")
        .expect("eur")
        .df_between_dates(as_of, opt.expiry)
        .expect("df");
    let fwd = FX_SPOT * df_f / df_d;
    let d1 = ((fwd / opt.strike).ln() + 0.5 * FX_VOL * FX_VOL * t) / (FX_VOL * t.sqrt());
    let d2 = d1 - FX_VOL * t.sqrt();
    let vega_unit = FX_SPOT * df_f * norm_pdf(d1) * t.sqrt();
    let gk_volga = vega_unit * d1 * d2 / FX_VOL * 1e-4 * 1_000_000.0;
    // The central difference carries an O(h²) truncation error (≈0.9% at
    // h = 2 vol points for this strike), so compare the Richardson
    // combination of the h and 2h overrides, which cancels the h² term.
    opt.metric_pricing_overrides = overrides(None, Some(h_v / 2.0), None);
    let volga_half = measure(
        &opt,
        &market,
        as_of,
        MetricId::Volga,
        PricingOptions::default(),
    );
    let richardson = (4.0 * volga_half - volga) / 3.0;
    assert_rel("fx volga vs Garman-Kohlhagen", richardson, gk_volga, 5e-3);
}

// ----------------------------------------------------------- QuantoOption

const QUANTO_AS_OF: Date = time::macros::date!(2026 - 01 - 02);

fn flat_surface(id: &str, strikes: &[f64], level: f64) -> VolSurface {
    let tenors = [0.25, 0.5, 1.0, 2.0, 5.0];
    let mut b = VolSurface::builder(CurveId::new(id))
        .expiries(&tenors)
        .strikes(strikes);
    for _ in 0..tenors.len() {
        b = b.row(&vec![level; strikes.len()]);
    }
    b.build().expect("flat surface")
}

fn quanto_market() -> MarketContext {
    let usd = DiscountCurve::builder("USD-OIS")
        .base_date(QUANTO_AS_OF)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("usd");
    let jpy = DiscountCurve::builder("JPY-OIS")
        .base_date(QUANTO_AS_OF)
        .knots([(0.0, 1.0), (1.0, 0.999), (5.0, 0.995)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("jpy");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::JPY, Currency::USD, 1.0 / 150.0)
        .expect("rate");
    MarketContext::new()
        .insert(usd)
        .insert(jpy)
        .insert_surface(flat_surface(
            "NKY-VOL",
            &[10_000.0, 25_000.0, 35_000.0, 50_000.0, 75_000.0],
            0.20,
        ))
        .insert_surface(flat_surface("JPYUSD-VOL", &[0.5, 0.8, 1.0, 1.2, 1.5], 0.10))
        .insert_fx(FxMatrix::new(provider))
        .insert_price("NKY-DIV", MarketScalar::Unitless(0.01))
        .insert_price("NKY-SPOT", MarketScalar::Unitless(35_000.0))
        .insert_price("JPYUSD-SPOT", MarketScalar::Unitless(1.0 / 150.0))
}

fn quanto_option() -> QuantoOption {
    QuantoOption::builder()
        .id(InstrumentId::new("QUANTO-ROUTING"))
        .underlying_ticker("NKY".to_string())
        .strike(35_000.0)
        .option_type(OptionType::Call)
        .expiry(time::macros::date!(2027 - 01 - 04))
        .notional(Money::new(1_000_000.0, Currency::USD).expect("money"))
        .quantity_opt(Some(4_000.0))
        .payoff_fx_rate_opt(Some(1.0 / 140.0))
        .base_currency(Currency::JPY)
        .quote_currency(Currency::USD)
        .correlation(-0.2)
        .day_count(DayCount::Act365F)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("JPY-OIS"))
        .spot_id("NKY-SPOT".into())
        .vol_surface_id(CurveId::new("NKY-VOL"))
        .div_yield_id_opt(Some(PriceId::new("NKY-DIV")))
        .fx_spot_id_opt(Some("JPYUSD-SPOT".into()))
        .fx_vol_surface_id_opt(Some(CurveId::new("JPYUSD-VOL")))
        .attributes(Attributes::new())
        .build()
        .expect("quanto option")
}

#[test]
fn quanto_defaults_are_bit_identical() {
    let (opt, market) = (quanto_option(), quanto_market());
    let m = |id| measure(&opt, &market, QUANTO_AS_OF, id, PricingOptions::default());
    assert_pins(&[
        ("delta", m(MetricId::Delta), 0x402d387eb9b2e2f9),
        ("gamma", m(MetricId::Gamma), 0x3f5998b5a0d6ce52),
        ("vega", m(MetricId::Vega), 0x41181e8d0fc216ea),
        ("rho", m(MetricId::Rho), 0xc01e1b66ed300000),
        ("foreign_rho", m(MetricId::ForeignRho), 0x4049b76ea7a3c800),
        ("fx_delta", m(MetricId::FxDelta), 0x0000000000000000),
        ("fx_vega", m(MetricId::FxVega), 0x4069b5dd2fd70a00),
    ]);
}

#[test]
fn quanto_rho_and_fx_greeks_honour_bump_config() {
    let market = quanto_market();
    let mut opt = quanto_option();
    let (h_s, h_v, h_r) = (0.02, 0.02, 5.0);
    opt.metric_pricing_overrides = overrides(Some(h_s), Some(h_v), Some(h_r));
    let m = |id| measure(&opt, &market, QUANTO_AS_OF, id, PricingOptions::default());
    let base = pv(&opt, &market, QUANTO_AS_OF);

    let rho_ref = (pv(&opt, &bump_rate(&market, "USD-OIS", h_r), QUANTO_AS_OF) - base) / h_r;
    let foreign_ref = (pv(&opt, &bump_rate(&market, "JPY-OIS", h_r), QUANTO_AS_OF) - base) / h_r;
    assert_rel("quanto rho", m(MetricId::Rho), rho_ref, 1e-12);
    assert_rel(
        "quanto foreign rho",
        m(MetricId::ForeignRho),
        foreign_ref,
        1e-12,
    );

    // FxDelta / FxVega report cash P&L per 1% FX move / 1 FX vol point.
    let fx_delta_ref = (pv(&opt, &bump_price(&market, "JPYUSD-SPOT", h_s), QUANTO_AS_OF)
        - pv(
            &opt,
            &bump_price(&market, "JPYUSD-SPOT", -h_s),
            QUANTO_AS_OF,
        ))
        / (2.0 * h_s * 100.0);
    let fx_vega_ref = (pv(&opt, &bump_vol(&market, "JPYUSD-VOL", h_v), QUANTO_AS_OF)
        - pv(&opt, &bump_vol(&market, "JPYUSD-VOL", -h_v), QUANTO_AS_OF))
        / (2.0 * h_v * 100.0);
    assert_rel("quanto fx delta", m(MetricId::FxDelta), fx_delta_ref, 1e-12);
    assert_rel("quanto fx vega", m(MetricId::FxVega), fx_vega_ref, 1e-12);

    let h = 35_000.0 * h_s;
    let delta_ref = (pv(&opt, &bump_price(&market, "NKY-SPOT", h_s), QUANTO_AS_OF)
        - pv(&opt, &bump_price(&market, "NKY-SPOT", -h_s), QUANTO_AS_OF))
        / (2.0 * h);
    assert_rel("quanto delta", m(MetricId::Delta), delta_ref, 1e-12);
}

// -------------------------------------------------------- FxBarrierOption

fn barrier_fixture() -> (FxBarrierOption, MarketContext, Date) {
    let as_of = date(2024, 1, 1);
    let expiries = [0.25, 0.5, 1.0, 2.0, 5.0];
    let strikes = [0.9, 1.0, 1.1, 1.2, 1.3];
    let market = MarketContext::new()
        .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 5.0))
        .insert(flat_discount_with_tenor("EUR-OIS", as_of, 0.01, 5.0))
        .insert_surface(flat_vol_surface("EURUSD-VOL", &expiries, &strikes, 0.15))
        .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));
    let option = FxBarrierOption::builder()
        .id(InstrumentId::new("FXBAR-ROUTING"))
        .strike(1.10)
        .barrier(1.60)
        .option_type(OptionType::Call)
        .barrier_type(BarrierType::UpAndOut)
        .expiry(date(2025, 1, 1))
        .monitoring_start_date_opt(Some(as_of))
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("money"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .monitoring(Monitoring::Continuous)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .fx_spot_id("EURUSD-SPOT".into())
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .attributes(Attributes::new())
        .build()
        .expect("fx barrier option");
    (option, market, as_of)
}

#[test]
fn fx_barrier_defaults_are_bit_identical() {
    let (opt, market, as_of) = barrier_fixture();
    let m = |id| measure(&opt, &market, as_of, id, PricingOptions::default());
    assert_pins(&[
        ("delta", m(MetricId::Delta), 0x411bdd91514e6e26),
        ("gamma", m(MetricId::Gamma), 0x4124b16b0eb0aceb),
        ("vega", m(MetricId::Vega), 0x4092ef1227cc1860),
        ("rho", m(MetricId::Rho), 0x40464c009e4a2800),
        ("vanna", m(MetricId::Vanna), 0xc0e2b4c78f91a6d0),
        ("volga", m(MetricId::Volga), 0xc086ad0d63429780),
    ]);
}

#[test]
fn fx_barrier_greeks_honour_bump_config() {
    let (mut opt, market, as_of) = barrier_fixture();
    let (h_s, h_v, h_r) = (0.02, 0.02, 5.0);
    opt.metric_pricing_overrides = overrides(Some(h_s), Some(h_v), Some(h_r));
    let m = |id| measure(&opt, &market, as_of, id, PricingOptions::default());
    let base = pv(&opt, &market, as_of);

    let h = 1.10 * h_s;
    let up = pv(&opt, &bump_price(&market, "EURUSD-SPOT", h_s), as_of);
    let dn = pv(&opt, &bump_price(&market, "EURUSD-SPOT", -h_s), as_of);
    assert_rel(
        "barrier delta",
        m(MetricId::Delta),
        (up - dn) / (2.0 * h),
        1e-12,
    );
    assert_rel(
        "barrier gamma",
        m(MetricId::Gamma),
        (up - 2.0 * base + dn) / (h * h),
        1e-12,
    );
    let v_up = pv(&opt, &bump_vol(&market, "EURUSD-VOL", h_v), as_of);
    let v_dn = pv(&opt, &bump_vol(&market, "EURUSD-VOL", -h_v), as_of);
    assert_rel(
        "barrier vega",
        m(MetricId::Vega),
        (v_up - v_dn) / (2.0 * h_v * 100.0),
        1e-12,
    );
    let rho_ref = (pv(&opt, &bump_rate(&market, "USD-OIS", h_r), as_of) - base) / h_r;
    assert_rel("barrier rho", m(MetricId::Rho), rho_ref, 1e-12);
}

// --------------------------------------------------------- LookbackOption

fn lookback_fixture() -> (LookbackOption, MarketContext, Date) {
    let as_of = date(2025, 1, 2);
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, (-0.05_f64 * 10.0).exp())])
        .build()
        .expect("curve");
    let surface = VolSurface::from_grid(
        "SPX-VOL",
        &[0.0, 10.0],
        &[0.0, 10_000.0],
        &[0.20, 0.20, 0.20, 0.20],
    )
    .expect("surface");
    let market = MarketContext::new()
        .insert(curve)
        .insert_surface(surface)
        .insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD).expect("money")),
        )
        .insert_price("SPX-DIV", MarketScalar::Unitless(0.0));
    let option = LookbackOption::builder()
        .id(InstrumentId::new("LOOKBACK-ROUTING"))
        .underlying_ticker("SPX".to_string())
        .strike_opt(Some(100.0))
        .option_type(OptionType::Call)
        .lookback_type(LookbackType::FixedStrike)
        .expiry(date(2026, 1, 2))
        .quantity(1.0)
        .currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .vol_surface_id(CurveId::new("SPX-VOL"))
        .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("lookback option");
    (option, market, as_of)
}

#[test]
fn lookback_rho_honours_rate_bump() {
    let (mut opt, market, as_of) = lookback_fixture();
    let default_rho = measure(
        &opt,
        &market,
        as_of,
        MetricId::Rho,
        PricingOptions::default(),
    );
    assert_pins(&[("lookback rho", default_rho, 0x3f72bd0500ee5000)]);

    let h_r = 5.0;
    opt.metric_pricing_overrides = overrides(None, None, Some(h_r));
    let rho = measure(
        &opt,
        &market,
        as_of,
        MetricId::Rho,
        PricingOptions::default(),
    );
    let rho_ref =
        (pv(&opt, &bump_rate(&market, "USD-OIS", h_r), as_of) - pv(&opt, &market, as_of)) / h_r;
    assert_rel("lookback rho per bp", rho, rho_ref, 1e-12);
}
