//! Listed futures share one position and lifecycle vocabulary on the wire:
//! `terms.contracts` × `terms.multiplier`, `terms.entry_price` for the trade
//! price, `terms.last_trading_date` / `terms.settlement_date` for the
//! lifecycle and `terms.settlement_price` for the official final settlement.
//! A commodity forward's market forward override is `quoted_forward` and an FX
//! future's quote currency is `terms.currency`. Each retired spelling is
//! rejected by `deny_unknown_fields`, and the migrated shapes reproduce the
//! old contract arithmetic.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{PriceCurve, PriceCurveKind};
use finstack_quant_valuations::instruments::equity::vol_index_future::VolatilityIndexFuture;
use finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture;
use finstack_quant_valuations::instruments::{
    BondFuture, CommodityForward, FxFuture, Instrument, ListedFutureTerms, Position,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use time::macros::date;

/// Serialize `instrument`, insert the retired top-level spelling with `value`,
/// and require rejection naming that key.
fn assert_rejects<T: Serialize + DeserializeOwned>(instrument: &T, retired: &str, value: Value) {
    let mut json = serde_json::to_value(instrument).expect("serialize");
    json.as_object_mut()
        .expect("instrument serializes as an object")
        .insert(retired.to_string(), value);
    let err = serde_json::from_value::<T>(json)
        .err()
        .unwrap_or_else(|| panic!("retired key {retired} must be rejected"));
    assert!(err.to_string().contains(retired), "{retired}: {err}");
}

#[test]
// schema-rejection-test: InterestRateFuture root `notional`, `expiry`, `quoted_price`, `settlement_price`, `position`
fn interest_rate_future_retired_position_keys_are_rejected() {
    let future = InterestRateFuture::example().expect("example");
    assert_rejects(
        &future,
        "notional",
        serde_json::json!({"amount": "1000000", "currency": "USD"}),
    );
    assert_rejects(&future, "expiry", serde_json::json!("2025-03-17"));
    assert_rejects(&future, "quoted_price", serde_json::json!(95.5));
    assert_rejects(&future, "settlement_price", serde_json::json!(96.0));
    assert_rejects(&future, "position", serde_json::json!("long"));
}

#[test]
// schema-rejection-test: InterestRateFuture `contract_specs.tick_value`
fn interest_rate_future_retired_tick_value_is_rejected() {
    let future = InterestRateFuture::example().expect("example");
    let mut json = serde_json::to_value(&future).expect("serialize");
    json["contract_specs"]["tick_value"] = serde_json::json!(6.25);
    let err = serde_json::from_value::<InterestRateFuture>(json).expect_err("tick_value retired");
    assert!(err.to_string().contains("tick_value"), "{err}");
}

#[test]
// schema-rejection-test: BondFuture root `notional`, `expiry`, `delivery_end`, `quoted_price`, `position`
fn bond_future_retired_position_keys_are_rejected() {
    let future = BondFuture::example().expect("example");
    assert_rejects(
        &future,
        "notional",
        serde_json::json!({"amount": "1000000", "currency": "USD"}),
    );
    assert_rejects(&future, "expiry", serde_json::json!("2025-09-19"));
    assert_rejects(&future, "delivery_end", serde_json::json!("2025-09-30"));
    assert_rejects(&future, "quoted_price", serde_json::json!(112.25));
    assert_rejects(&future, "position", serde_json::json!("long"));
}

#[test]
// schema-rejection-test: VolatilityIndexFuture root `notional`, `expiry`, `settlement_date`, `settlement_fixing`, `quoted_price`, `position`, `contract_specs`
fn vol_index_future_retired_keys_are_rejected() {
    let future = VolatilityIndexFuture::example().expect("example");
    assert_rejects(
        &future,
        "notional",
        serde_json::json!({"amount": "100000", "currency": "USD"}),
    );
    assert_rejects(&future, "expiry", serde_json::json!("2025-03-19"));
    assert_rejects(&future, "settlement_date", serde_json::json!("2025-03-19"));
    assert_rejects(&future, "settlement_fixing", serde_json::json!(19.0));
    assert_rejects(&future, "quoted_price", serde_json::json!(21.5));
    assert_rejects(&future, "position", serde_json::json!("long"));
    assert_rejects(
        &future,
        "contract_specs",
        serde_json::json!({"multiplier": 1000.0, "tick_size": 0.05, "tick_value": 50.0, "index_id": "VIX"}),
    );
}

#[test]
// schema-rejection-test: FxFuture root `quote_currency`
fn fx_future_retired_quote_currency_is_rejected() {
    let future = FxFuture::example().expect("example");
    assert_eq!(future.quote_currency(), Currency::USD);
    assert_rejects(&future, "quote_currency", serde_json::json!("USD"));
}

#[test]
// schema-rejection-test: CommodityForward `quoted_price`
fn commodity_forward_retired_quoted_price_is_rejected() {
    let mut forward = CommodityForward::example().expect("example");
    forward.quoted_forward = Some(75.0);
    let mut json = serde_json::to_value(&forward).expect("serialize");
    let map = json.as_object_mut().expect("object");
    let value = map
        .remove("quoted_forward")
        .expect("quoted_forward serializes");
    map.insert("quoted_price".to_string(), value);
    let err = serde_json::from_value::<CommodityForward>(json).expect_err("quoted_price retired");
    assert!(err.to_string().contains("quoted_price"), "{err}");
}

/// The IR future P&L is `sign × contracts × multiplier × (mark − entry)`. With
/// `contracts = notional / face_value` and `multiplier = tick_value /
/// tick_size` it reproduces the retired exchange-tick formula
/// `(mark − entry) / tick_size × tick_value × notional / face_value`
/// (CME SR3: $6.25 per 0.0025 tick). Tolerance: two float re-associations.
#[test]
fn interest_rate_future_terms_reproduce_tick_formula() {
    let mut future = InterestRateFuture::example().expect("example");
    future.terms.contracts = 10.0;
    future.terms.settlement_price = Some(96.0);
    let as_of = date!(2025 - 03 - 18);
    future.terms.settlement_date = as_of;
    let pv = future.npv_raw(&MarketContext::new(), as_of).expect("pv");
    let tick_formula = (96.0 - 95.50) / 0.0025 * 6.25 * 10_000_000.0 / 1_000_000.0;
    assert!(
        (pv - tick_formula).abs() <= 1e-9 * tick_formula.abs(),
        "{pv} vs {tick_formula}"
    );
}

/// VIX: the SOQ `terms.settlement_price` is required on the settlement date,
/// and the migrated `contracts = notional / (multiplier × entry)` reproduces
/// the retired notional scaling.
#[test]
fn vol_index_future_uses_settlement_price_on_settlement_date() {
    let settlement = date!(2025 - 04 - 01);
    let vix = PriceCurve::builder("VIX")
        .kind(PriceCurveKind::VolIndex)
        .base_date(date!(2025 - 01 - 01))
        .spot_price(18.0)
        .knots([(0.0, 18.0), (1.0, 22.0)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(vix);
    let notional = 100_000.0;
    let entry = 20.0;
    let mut future = VolatilityIndexFuture::builder()
        .id("VIX-SOQ".into())
        .terms(
            ListedFutureTerms::new(
                notional / (1_000.0 * entry),
                1_000.0,
                Currency::USD,
                entry,
                settlement,
                settlement,
                Position::Long,
            )
            .expect("terms"),
        )
        .discount_curve_id("USD-OIS".into())
        .vol_index_curve_id("VIX".into())
        .build()
        .expect("future");

    let err = future
        .value(&market, settlement)
        .expect_err("settlement day needs the SOQ");
    assert!(err.to_string().contains("terms.settlement_price"), "{err}");

    future.terms.settlement_price = Some(23.0);
    let pv = future.value(&market, settlement).expect("pv").amount();
    let retired_formula = (23.0 - entry) * 1_000.0 * notional / (1_000.0 * entry);
    assert!((pv - retired_formula).abs() <= 1e-9 * retired_formula.abs());
}
