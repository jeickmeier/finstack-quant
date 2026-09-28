//! Underlying identity, spot ids and size share one wire vocabulary: the
//! underlying spot is the typed `spot_id: PriceId` (never an `attributes.meta`
//! key or an id derived from a ticker), instrument-local quotes are
//! `quoted_spot` / `quoted_forward`, quanto inputs use the shared `QuantoSpec`
//! (`fx_spot_id`, `fx_vol_surface_id`), and a count of underlying units is
//! `quantity`. Each retired spelling is rejected by `deny_unknown_fields`.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_valuations::instruments::{
    AsianOption, BarrierOption, CommodityAsianOption, CommodityForward, CommodityFuture,
    CommodityOption, CommoditySpreadOption, CommoditySwap, CommoditySwaption, ConvertibleBond,
    Equity, EquityFuture, EquityOption, FxForward, FxFuture, FxSpot, LookbackOption, Ndf,
    QuantoOption, VarianceSwap, VolatilityDependency,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Serialize `instrument`, insert `retired` with `value` into the object at
/// `pointer` (JSON pointer, `""` for the root), and require a rejection that
/// names the retired key.
fn assert_rejects_at<T: Serialize + DeserializeOwned>(
    instrument: &T,
    pointer: &str,
    retired: &str,
    value: Value,
) {
    let mut json = serde_json::to_value(instrument).expect("serialize");
    json.pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("{pointer} must be an object"))
        .insert(retired.to_string(), value);
    let err = serde_json::from_value::<T>(json)
        .err()
        .unwrap_or_else(|| panic!("retired key {retired} must be rejected"));
    assert!(err.to_string().contains(retired), "{retired}: {err}");
}

#[test]
// schema-rejection-test: ConvertibleBond `underlying_equity_id`
fn convertible_underlying_equity_id_is_rejected() {
    let bond = ConvertibleBond::example().expect("example");
    assert_rejects_at(&bond, "", "underlying_equity_id", "TECH".into());
    let json = serde_json::to_value(&bond).expect("serialize");
    assert_eq!(json["spot_id"], "TECH");
    assert_eq!(json["vol_surface_id"], "TECH-VOL");
}

#[test]
// schema-rejection-test: Equity `price_id`, `price_quote`, `shares`
fn equity_retired_spot_and_size_keys_are_rejected() {
    let equity = Equity::example().expect("example");
    assert_rejects_at(&equity, "", "price_id", "AAPL-SPOT".into());
    assert_rejects_at(&equity, "", "price_quote", 150.0.into());
    assert_rejects_at(&equity, "", "shares", 100.0.into());
    let json = serde_json::to_value(&equity).expect("serialize");
    assert_eq!(json["spot_id"], "AAPL-SPOT");
    assert_eq!(json["quantity"], 100.0);
}

#[test]
// schema-rejection-test: FX `spot_rate_override`, FxSpot `spot_rate`, Ndf `forward_rate_override`
fn fx_retired_quote_override_keys_are_rejected() {
    let forward = FxForward::example().expect("example");
    assert_rejects_at(&forward, "", "spot_rate_override", 1.1.into());
    let future = FxFuture::example().expect("example");
    assert_rejects_at(&future, "", "spot_rate_override", 1.1.into());
    let ndf = Ndf::example().expect("example");
    assert_rejects_at(&ndf, "", "spot_rate_override", 7.1.into());
    assert_rejects_at(&ndf, "", "forward_rate_override", 7.25.into());
    let spot = FxSpot::example().expect("example");
    assert_rejects_at(&spot, "", "spot_rate", 1.1.into());
}

#[test]
// schema-rejection-test: QuantoOption `fx_rate_id`, `fx_vol_id`, `underlying_quantity`
fn quanto_option_retired_fx_and_size_keys_are_rejected() {
    let option = QuantoOption::example().expect("example");
    assert_rejects_at(&option, "", "fx_rate_id", "JPYUSD-SPOT".into());
    assert_rejects_at(&option, "", "fx_vol_id", "JPYUSD-VOL".into());
    assert_rejects_at(&option, "", "underlying_quantity", 4_000.0.into());
    let json = serde_json::to_value(&option).expect("serialize");
    assert_eq!(json["fx_spot_id"], "JPYUSD-SPOT");
    assert_eq!(json["fx_vol_surface_id"], "JPYUSD-VOL");
    assert_eq!(json["quantity"], 4_000.0);
}

#[test]
// schema-rejection-test: EquityFuture quanto `settlement_discount_curve_id`, `equity_vol_surface_id`
fn equity_future_retired_quanto_keys_are_rejected() {
    let future = EquityFuture::example().expect("example");
    assert_rejects_at(
        &future,
        "/quanto",
        "settlement_discount_curve_id",
        "USD-OIS".into(),
    );
    assert_rejects_at(
        &future,
        "/quanto",
        "equity_vol_surface_id",
        "SX5E-VOL".into(),
    );
    let json = serde_json::to_value(&future).expect("serialize");
    // `discount_curve_id` is the settlement (payoff) curve on every quanto
    // instrument; the underlying carry curve lives in the shared QuantoSpec.
    assert_eq!(json["discount_curve_id"], "USD-OIS");
    assert_eq!(json["quanto"]["asset_discount_curve_id"], "EUR-OIS");
    assert_eq!(json["quanto"]["asset_currency"], "EUR");
    assert_eq!(json["vol_surface_id"], "SX5E-VOL");
}

#[test]
fn equity_future_quanto_asset_currency_must_match_underlying() {
    let mut future = EquityFuture::example().expect("example");
    if let Some(quanto) = future.quanto.as_mut() {
        quanto.asset_currency = finstack_quant_core::currency::Currency::GBP;
    }
    let err = future.validate().expect_err("mismatched asset currency");
    assert!(err.to_string().contains("asset_currency"), "{err}");
}

#[test]
// schema-rejection-test: commodity flattened `ticker`, CommodityFuture `underlying`
fn commodity_retired_ticker_keys_are_rejected() {
    assert_rejects_at(
        &CommodityForward::example().expect("example"),
        "",
        "ticker",
        "CL".into(),
    );
    assert_rejects_at(
        &CommodityOption::example().expect("example"),
        "",
        "ticker",
        "CL".into(),
    );
    assert_rejects_at(
        &CommodityAsianOption::example().expect("example"),
        "",
        "ticker",
        "CL".into(),
    );
    assert_rejects_at(
        &CommoditySwap::example().expect("example"),
        "",
        "ticker",
        "NG".into(),
    );
    assert_rejects_at(
        &CommoditySwaption::example().expect("example"),
        "",
        "ticker",
        "NG".into(),
    );
    let future = CommodityFuture::example().expect("example");
    assert_rejects_at(&future, "", "underlying", "TSI-62-FE".into());
    let json =
        serde_json::to_value(CommodityForward::example().expect("example")).expect("serialize");
    assert_eq!(json["underlying_ticker"], "CL");
}

#[test]
// schema-rejection-test: CommoditySpreadOption / CommoditySwaption `notional`
fn commodity_unit_count_notional_is_rejected() {
    let spread = CommoditySpreadOption::example().expect("example");
    assert_rejects_at(&spread, "", "notional", 10_000.0.into());
    assert_rejects_at(
        &CommoditySwaption::example().expect("example"),
        "",
        "notional",
        10_000.0.into(),
    );
    let json = serde_json::to_value(&spread).expect("serialize");
    assert_eq!(json["quantity"], 10_000.0);
}

#[test]
// schema-rejection-test: VolatilityDependency `underlying_id`
fn volatility_dependency_underlying_id_is_rejected() {
    let dependency = VolatilityDependency::new("SPX-VOL", Some("SPX".into()), None);
    assert_rejects_at(&dependency, "", "underlying_id", "SPX".into());
    let json = serde_json::to_value(&dependency).expect("serialize");
    assert_eq!(json["spot_id"], "SPX");
}

#[test]
fn variance_swap_requires_typed_market_ids() {
    let swap = VarianceSwap::example().expect("example");
    let mut json = serde_json::to_value(&swap).expect("serialize");
    json.as_object_mut().expect("object").remove("spot_id");
    let err = serde_json::from_value::<VarianceSwap>(json).expect_err("spot_id is required");
    assert!(err.to_string().contains("spot_id"), "{err}");
}

#[test]
fn equity_never_prices_off_a_derived_or_global_spot_id() {
    // Scalars under every id the retired candidate list tried: the ticker,
    // `{ticker}-SPOT`, `{id}-SPOT` and the global `EQUITY-SPOT`.
    let market = MarketContext::new()
        .insert_price("AAPL", MarketScalar::Unitless(1.0))
        .insert_price("AAPL-SPOT", MarketScalar::Unitless(2.0))
        .insert_price("EQUITY-SPOT", MarketScalar::Unitless(3.0));
    let as_of = time::macros::date!(2025 - 01 - 02);
    let equity = Equity::new("AAPL", "AAPL", finstack_quant_core::currency::Currency::USD);
    let err = equity
        .price_per_share(&market, as_of)
        .expect_err("no spot_id and no quoted_spot must not resolve a price");
    assert!(err.to_string().contains("spot_id"), "{err}");

    let priced = equity.with_spot_id("AAPL-SPOT");
    let price = priced
        .price_per_share(&market, as_of)
        .expect("typed spot_id");
    assert_eq!(price.amount(), 2.0);
    // No dividend-yield id means a zero yield, never `{ticker}-DIVYIELD`.
    let market = market.insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.5));
    assert_eq!(priced.dividend_yield(&market).expect("zero"), 0.0);
}

#[test]
// schema-rejection-test: EquityOption / AsianOption / BarrierOption / LookbackOption `notional`
fn equity_style_option_unit_count_notional_is_rejected() {
    let notional = serde_json::json!({"amount": "100", "currency": "USD"});
    let equity = EquityOption::example().expect("example");
    assert_rejects_at(&equity, "", "notional", notional.clone());
    let asian = AsianOption::example().expect("example");
    assert_rejects_at(&asian, "", "notional", notional.clone());
    let barrier = BarrierOption::example().expect("example");
    assert_rejects_at(&barrier, "", "notional", notional.clone());
    let lookback = LookbackOption::example().expect("example");
    assert_rejects_at(&lookback, "", "notional", notional);
    let json = serde_json::to_value(&equity).expect("serialize");
    assert_eq!(json["quantity"], 100.0);
    assert_eq!(json["currency"], "USD");
}
