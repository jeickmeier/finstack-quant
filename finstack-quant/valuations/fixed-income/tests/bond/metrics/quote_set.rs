//! The whole quote set: each field against its metric, each quote kind back to the
//! price, and what a quote kind that does not apply to the bond does.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{BondQuoteInput, Instrument, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_fixed_income::bond::pricing::quote_conversions::{
    BondQuoteSet, compute_quotes,
};
use finstack_quant_valuations_fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use time::Date;
use time::macros::date;

const AS_OF: Date = date!(2025 - 01 - 15);

/// Largest closing error of a yield quote, in percent of par. `YTM_TOLERANCE` stops
/// the yield solve within 1e-12, so the price is off by about modified duration (at
/// most 10 here) times 1e-12 of notional: 1e-9 percent.
const YIELD_CLOSE_PCT: f64 = 1e-9;
/// Largest closing error of a spread quote, in percent of par. `Z_SPREAD_TOLERANCE`
/// and the tree's OAS tolerance of 1e-6 bp both stop the solve within 1e-10, so the
/// price is off by about duration (at most 10 here) times 1e-10 of notional.
const SPREAD_CLOSE_PCT: f64 = 1e-7;

fn market() -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(AS_OF)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97),
            (2.0, 0.94),
            (3.0, 0.91),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .build()
        .expect("discount curve");
    let forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(date!(2024 - 12 - 30))
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (10.0, 0.03)])
        .build()
        .expect("forward curve");
    MarketContext::new().insert(discount).insert(forward)
}

/// Seasoned, so accrued interest separates the clean and dirty paths.
fn settled() -> Bond {
    Bond::fixed(
        "QS-SETTLED",
        Money::new(1_000_000.0, Currency::USD).expect("money"),
        Rate::from_decimal(0.06).expect("rate"),
        date!(2024 - 03 - 15),
        date!(2032 - 03 - 15),
        StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond")
}

fn bullet() -> Bond {
    let mut bond = settled();
    bond.settlement_convention = None;
    bond
}

fn call(on: Date, price_pct_of_par: f64) -> CallPut {
    CallPut {
        start: on,
        end: on,
        price_pct_of_par,
        make_whole: None,
    }
}

fn with_calls(calls: Vec<CallPut>) -> Bond {
    let mut bond = bullet();
    bond.call_put = Some(CallPutSchedule {
        calls,
        puts: vec![],
    });
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_hw1f_sigma(0.01)
        .with_hw1f_mean_reversion(0.03);
    bond
}

fn callable() -> Bond {
    with_calls(vec![
        call(date!(2027 - 03 - 15), 103.0),
        call(date!(2029 - 03 - 15), 101.0),
    ])
}

/// Issued on the valuation date, so no fixing is needed.
fn frn() -> Bond {
    let mut bond = Bond::floating(
        "QS-FRN",
        Money::new(1_000_000.0, Currency::USD).expect("money"),
        "USD-SOFR-3M",
        150,
        AS_OF,
        date!(2030 - 03 - 15),
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .expect("frn");
    bond.settlement_convention = None;
    bond
}

fn quotes(bond: &Bond, input: BondQuoteInput) -> finstack_quant_core::Result<BondQuoteSet> {
    compute_quotes(bond, &market(), AS_OF, input, PricingOptions::default())
}

/// The metric of `id` on `bond` quoted at `clean_price_pct`, in isolation.
fn metric(bond: &Bond, clean_price_pct: f64, id: &MetricId) -> f64 {
    let mut quoted = bond.clone();
    quoted
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(clean_price_pct);
    let result = quoted
        .price_with_metrics(
            &market(),
            AS_OF,
            std::slice::from_ref(id),
            PricingOptions::default(),
        )
        .expect("metric");
    *result.measures.get(id.as_str()).expect("measure")
}

fn assert_field_is_metric(bond: &Bond, set: &BondQuoteSet, field: Option<f64>, id: MetricId) {
    let value = field.unwrap_or_else(|| panic!("{} is set on {}", id.as_str(), bond.id.as_str()));
    let measured = metric(bond, set.clean_price_pct, &id);
    assert_eq!(
        value.to_bits(),
        measured.to_bits(),
        "{} on {}: set {value}, metric {measured}",
        id.as_str(),
        bond.id.as_str()
    );
}

/// Every field a fixed bond has equals its metric, bit for bit: they run one path.
#[test]
fn each_field_of_a_fixed_bond_is_its_metric() {
    for (bond, clean) in [
        (bullet(), 99.25),
        (settled(), 99.25),
        // A premium makes an early call the worst exit; a discount makes it maturity.
        (callable(), 108.0),
        (callable(), 92.0),
    ] {
        let set = quotes(&bond, BondQuoteInput::CleanPricePct(clean)).expect("set");
        assert_eq!(set.clean_price_pct, clean);
        assert_eq!(set.discount_margin, None);
        for (field, id) in [
            (set.ytm, MetricId::Ytm),
            (set.ytw, MetricId::Ytw),
            (set.z_spread, MetricId::ZSpread),
            (set.oas, MetricId::Oas),
            (set.asw_par, MetricId::ASWPar),
            (set.asw_market, MetricId::ASWMarket),
            (set.i_spread, MetricId::ISpread),
            (set.japanese_simple_yield, MetricId::JapaneseSimpleYield),
            (set.moosmuller_ytm, MetricId::MoosmullerYtm),
            (Some(set.clean_price_currency), MetricId::CleanPrice),
            (Some(set.dirty_price_currency), MetricId::DirtyPrice),
        ] {
            assert_field_is_metric(&bond, &set, field, id);
        }
    }
}

#[test]
fn a_premium_callable_yields_to_its_call_and_a_discount_one_to_maturity() {
    let premium = quotes(&callable(), BondQuoteInput::CleanPricePct(108.0)).expect("set");
    assert!(premium.ytw.expect("ytw") < premium.ytm.expect("ytm"));
    let discount = quotes(&callable(), BondQuoteInput::CleanPricePct(92.0)).expect("set");
    assert_eq!(discount.ytw, discount.ytm);
}

#[test]
fn each_field_of_an_frn_is_its_metric_and_it_has_no_fixed_rate_quote() {
    let bond = frn();
    let set = quotes(&bond, BondQuoteInput::CleanPricePct(99.25)).expect("set");
    for (field, id) in [
        (set.ytm, MetricId::Ytm),
        (set.ytw, MetricId::Ytw),
        (set.z_spread, MetricId::ZSpread),
        (set.discount_margin, MetricId::DiscountMargin),
        (set.oas, MetricId::Oas),
        (set.i_spread, MetricId::ISpread),
        (set.moosmuller_ytm, MetricId::MoosmullerYtm),
    ] {
        assert_field_is_metric(&bond, &set, field, id);
    }
    assert_eq!(set.asw_par, None);
    assert_eq!(set.japanese_simple_yield, None);
    // The set's gate is stricter than the metric: asw_market alone values a plain
    // FRN, while the set follows asw_par, which rejects it.
    assert_eq!(set.asw_market, None);
    assert!(metric(&bond, set.clean_price_pct, &MetricId::ASWMarket).is_finite());
}

fn assert_closes(bond: &Bond, input: BondQuoteInput, clean: f64, tolerance_pct: f64) {
    let back = quotes(bond, input)
        .unwrap_or_else(|err| panic!("{input:?} on {}: {err}", bond.id.as_str()))
        .clean_price_pct;
    assert!(
        (back - clean).abs() <= tolerance_pct,
        "{input:?} on {}: {back} against {clean}",
        bond.id.as_str()
    );
}

/// Price to set, then each quote kind back in as the input, returns that price.
#[test]
fn each_quote_kind_of_a_fixed_bond_returns_the_price() {
    for bond in [bullet(), settled()] {
        let clean = 99.25;
        let set = quotes(&bond, BondQuoteInput::CleanPricePct(clean)).expect("set");
        for (input, tolerance) in [
            (
                BondQuoteInput::DirtyPriceCurrency(set.dirty_price_currency),
                1e-12,
            ),
            (BondQuoteInput::Ytm(set.ytm.expect("ytm")), YIELD_CLOSE_PCT),
            (BondQuoteInput::Ytw(set.ytw.expect("ytw")), YIELD_CLOSE_PCT),
            (
                BondQuoteInput::ISpread(set.i_spread.expect("i_spread")),
                YIELD_CLOSE_PCT,
            ),
            (
                BondQuoteInput::ZSpread(set.z_spread.expect("z_spread")),
                SPREAD_CLOSE_PCT,
            ),
            (BondQuoteInput::Oas(set.oas.expect("oas")), SPREAD_CLOSE_PCT),
            // Closed forms in the price: only rounding separates them.
            (
                BondQuoteInput::AswMarket(set.asw_market.expect("asw_market")),
                1e-12,
            ),
            (
                BondQuoteInput::JapaneseSimpleYield(
                    set.japanese_simple_yield.expect("japanese_simple_yield"),
                ),
                1e-12,
            ),
        ] {
            assert_closes(&bond, input, clean, tolerance);
        }
    }
}

#[test]
fn each_quote_kind_of_a_callable_returns_the_price() {
    for clean in [108.0, 92.0] {
        let bond = callable();
        let set = quotes(&bond, BondQuoteInput::CleanPricePct(clean)).expect("set");
        // Z-spread and asw_market are rejected on a bond with exercise rights, in
        // their own tests.
        for (input, tolerance) in [
            (
                BondQuoteInput::DirtyPriceCurrency(set.dirty_price_currency),
                1e-12,
            ),
            (BondQuoteInput::Ytm(set.ytm.expect("ytm")), YIELD_CLOSE_PCT),
            (BondQuoteInput::Ytw(set.ytw.expect("ytw")), YIELD_CLOSE_PCT),
            (
                BondQuoteInput::ISpread(set.i_spread.expect("i_spread")),
                YIELD_CLOSE_PCT,
            ),
            (BondQuoteInput::Oas(set.oas.expect("oas")), SPREAD_CLOSE_PCT),
            (
                BondQuoteInput::JapaneseSimpleYield(
                    set.japanese_simple_yield.expect("japanese_simple_yield"),
                ),
                1e-12,
            ),
        ] {
            assert_closes(&bond, input, clean, tolerance);
        }
    }
}

#[test]
fn each_quote_kind_of_an_frn_returns_the_price() {
    let bond = frn();
    let clean = 99.25;
    let set = quotes(&bond, BondQuoteInput::CleanPricePct(clean)).expect("set");
    for (input, tolerance) in [
        (
            BondQuoteInput::DirtyPriceCurrency(set.dirty_price_currency),
            1e-12,
        ),
        (BondQuoteInput::Ytm(set.ytm.expect("ytm")), YIELD_CLOSE_PCT),
        (BondQuoteInput::Ytw(set.ytw.expect("ytw")), YIELD_CLOSE_PCT),
        (
            BondQuoteInput::ISpread(set.i_spread.expect("i_spread")),
            YIELD_CLOSE_PCT,
        ),
        (
            BondQuoteInput::ZSpread(set.z_spread.expect("z_spread")),
            SPREAD_CLOSE_PCT,
        ),
        (
            BondQuoteInput::DiscountMargin(set.discount_margin.expect("discount_margin")),
            SPREAD_CLOSE_PCT,
        ),
        (BondQuoteInput::Oas(set.oas.expect("oas")), SPREAD_CLOSE_PCT),
    ] {
        assert_closes(&bond, input, clean, tolerance);
    }
}

/// The `asw_market` metric runs the swap to the worst exit, which depends on the
/// price this quote is meant to solve, so the quote is refused at every price: here
/// at the spreads of a premium and a discount, where the worst exit differs. The
/// output field is unchanged.
#[test]
fn asw_market_input_is_rejected_on_a_bond_with_exercise_rights() {
    let bond = callable();
    for clean in [108.0, 92.0] {
        let set = quotes(&bond, BondQuoteInput::CleanPricePct(clean)).expect("set");
        assert_field_is_metric(&bond, &set, set.asw_market, MetricId::ASWMarket);
        let message = quotes(
            &bond,
            BondQuoteInput::AswMarket(set.asw_market.expect("asw_market")),
        )
        .expect_err("rejected")
        .to_string();
        assert!(
            message.contains("ASW market pricing for option-bearing bond"),
            "{message}"
        );
    }
}

/// Every route through the shared quote path refuses it, not only the quote set.
#[test]
fn quoted_asw_market_on_a_bond_with_exercise_rights_does_not_price() {
    let mut bond = callable();
    bond.instrument_pricing_overrides
        .market_quotes
        .quoted_asw_market = Some(0.01);
    let message = bond
        .value(&market(), AS_OF)
        .expect_err("rejected")
        .to_string();
    assert!(
        message.contains("ASW market pricing for option-bearing bond"),
        "{message}"
    );
    let mut plain = bullet();
    plain
        .instrument_pricing_overrides
        .market_quotes
        .quoted_asw_market = Some(0.01);
    assert!(plain.value(&market(), AS_OF).is_ok());
}

/// A quote kind that does not apply to the bond is the native error, not a `None`.
#[test]
fn a_quote_kind_the_bond_does_not_have_is_an_error() {
    let cases = [
        (
            bullet(),
            BondQuoteInput::DiscountMargin(0.01),
            "requires a floating-rate bond",
        ),
        (
            frn(),
            BondQuoteInput::AswMarket(0.01),
            "require a plain fixed-coupon bond",
        ),
        (
            frn(),
            BondQuoteInput::JapaneseSimpleYield(0.05),
            "requires a fixed-coupon bond",
        ),
        (
            callable(),
            BondQuoteInput::ZSpread(0.01),
            "requires an explicit quoted clean price",
        ),
        (
            callable(),
            BondQuoteInput::AswMarket(0.01),
            "ASW market pricing for option-bearing bond",
        ),
    ];
    for (bond, input, expected) in cases {
        let message = quotes(&bond, input).expect_err("rejected").to_string();
        assert!(message.contains(expected), "{input:?}: {message}");
    }
}

#[test]
fn a_non_finite_or_non_positive_input_is_an_error() {
    for input in [
        BondQuoteInput::Ytm(f64::NAN),
        BondQuoteInput::CleanPricePct(f64::INFINITY),
        BondQuoteInput::ZSpread(f64::NEG_INFINITY),
    ] {
        let message = quotes(&bullet(), input).expect_err("rejected").to_string();
        assert!(message.contains("must be finite"), "{input:?}: {message}");
    }
    let message = quotes(&bullet(), BondQuoteInput::DirtyPriceCurrency(-1.0))
        .expect_err("rejected")
        .to_string();
    assert!(message.contains("must be positive"), "{message}");
}

/// `ytw` alone is negative infinity here and the `i_spread` metric fails with
/// "Invalid date range". The set runs `z_spread` first, which has no spread that
/// matches same-day cash, so its error is the one the whole set returns.
#[test]
fn a_call_on_the_quote_date_below_the_price_fails_the_whole_set_at_z_spread() {
    let bond = with_calls(vec![call(AS_OF, 99.0)]);
    assert_eq!(metric(&bond, 100.0, &MetricId::Ytw), f64::NEG_INFINITY);
    let message = quotes(&bond, BondQuoteInput::CleanPricePct(100.0))
        .expect_err("rejected")
        .to_string();
    assert!(
        message.contains("bond quote metric 'z_spread'") && message.contains(bond.id.as_str()),
        "{message}"
    );
}

/// At or below the call price the quote-date exit is not the worst one.
#[test]
fn a_call_on_the_quote_date_at_or_above_the_price_gives_a_whole_set() {
    let bond = with_calls(vec![call(AS_OF, 100.0)]);
    for clean in [100.0, 90.0] {
        let set = quotes(&bond, BondQuoteInput::CleanPricePct(clean)).expect("set");
        assert_field_is_metric(&bond, &set, set.ytw, MetricId::Ytw);
        assert_field_is_metric(&bond, &set, set.i_spread, MetricId::ISpread);
    }
}

#[test]
fn a_bond_with_exercise_rights_and_no_model_parameters_fails_the_whole_set_at_oas() {
    let mut bond = callable();
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default();
    let message = quotes(&bond, BondQuoteInput::CleanPricePct(100.0))
        .expect_err("rejected")
        .to_string();
    assert!(message.contains("bond quote metric 'oas'"), "{message}");
}

#[test]
fn the_input_is_strict_on_stable_snake_case_names() {
    for (input, wire) in [
        (
            BondQuoteInput::CleanPricePct(99.5),
            r#"{"clean_price_pct":99.5}"#,
        ),
        (
            BondQuoteInput::DirtyPriceCurrency(1.5),
            r#"{"dirty_price_currency":1.5}"#,
        ),
        (BondQuoteInput::Ytm(0.5), r#"{"ytm":0.5}"#),
        (BondQuoteInput::Ytw(0.5), r#"{"ytw":0.5}"#),
        (BondQuoteInput::ZSpread(0.5), r#"{"z_spread":0.5}"#),
        (
            BondQuoteInput::DiscountMargin(0.5),
            r#"{"discount_margin":0.5}"#,
        ),
        (BondQuoteInput::Oas(0.5), r#"{"oas":0.5}"#),
        (BondQuoteInput::AswMarket(0.5), r#"{"asw_market":0.5}"#),
        (BondQuoteInput::ISpread(0.5), r#"{"i_spread":0.5}"#),
        (
            BondQuoteInput::JapaneseSimpleYield(0.5),
            r#"{"japanese_simple_yield":0.5}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&input).expect("to json"), wire);
        assert_eq!(
            serde_json::from_str::<BondQuoteInput>(wire).expect("from json"),
            input
        );
    }
    for wire in [
        r#"{"yield":0.05}"#,
        r#"{"ytm":0.05,"ytw":0.05}"#,
        r#"{"ytm":"0.05"}"#,
        r#"{}"#,
        r#""ytm""#,
    ] {
        assert!(
            serde_json::from_str::<BondQuoteInput>(wire).is_err(),
            "text {wire}"
        );
        // Python and WASM reach serde through a JSON value, not text.
        let value: serde_json::Value = serde_json::from_str(wire).expect("json");
        assert!(
            serde_json::from_value::<BondQuoteInput>(value).is_err(),
            "value {wire}"
        );
    }
}

#[test]
fn the_set_round_trips_with_null_for_what_does_not_apply() {
    let set = quotes(&bullet(), BondQuoteInput::CleanPricePct(99.25)).expect("set");
    let wire: serde_json::Value = serde_json::to_value(&set).expect("to json");
    assert_eq!(wire["discount_margin"], serde_json::Value::Null);
    let text = serde_json::to_string(&set).expect("to json");
    let back: BondQuoteSet = serde_json::from_str(&text).expect("from json");
    assert_eq!(back, set);
    let mut extra = wire;
    extra["unknown"] = serde_json::json!(1.0);
    assert!(serde_json::from_value::<BondQuoteSet>(extra).is_err());
}
