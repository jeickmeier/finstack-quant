//! Free-function twins of the Python quote and rate-bound constructors.
//!
//! Python exposes `RateQuote.deposit(...)`, `CdsQuote.par_spread(...)`,
//! `VolQuote.option_vol(...)` and `RateBounds.for_currency(...)` as static
//! methods on typed wrappers. WASM calibration inputs are plain objects, so
//! each constructor is a free function returning the serde wire object of the
//! Rust type. Both hosts marshal their arguments into wire fields and call the
//! same Rust constructors (`RateQuote::from_wire_fields`,
//! `CdsQuote::from_wire_fields`, `VolQuote::from_wire_fields`), which own the
//! defaults, the strict deserialization and the quote validation.

use super::fields::Fields;
use crate::utils::input::js_string;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_calibration::quotes::cds::CdsQuote;
use finstack_quant_calibration::quotes::rates::RateQuote;
use finstack_quant_calibration::quotes::vol::VolQuote;
use finstack_quant_calibration::RateBounds;
use finstack_quant_core::currency::Currency;
use wasm_bindgen::prelude::*;

fn rate_quote(kind: &str, fields: Fields) -> Result<JsValue, JsValue> {
    to_js_value(&RateQuote::from_wire_fields(kind, fields.0).map_err(to_js_err)?)
}

fn cds_quote(kind: &str, fields: Fields) -> Result<JsValue, JsValue> {
    to_js_value(&CdsQuote::from_wire_fields(kind, fields.0).map_err(to_js_err)?)
}

fn vol_quote(kind: &str, fields: Fields) -> Result<JsValue, JsValue> {
    to_js_value(&VolQuote::from_wire_fields(kind, fields.0).map_err(to_js_err)?)
}

/// Build a money-market deposit rate quote.
///
/// Free-function twin of Python `RateQuote.deposit` (Rust `RateQuote::from_wire_fields`).
/// @param id - Unique quote identifier (the residual key in calibration reports).
/// @param index - Rate index identifier (for example `"USD-SOFR-OIS"`).
/// @param pillar - Maturity pillar: tenor string (`"3M"`, `"5Y"`), ISO-8601 date string, or a `{"tenor": {...}}` / `{"date": "..."}` object.
/// @param rate - Simple deposit rate as a decimal (`0.0525` is 5.25%).
/// @returns A validated `RateQuote` object with `type: "deposit"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the pillar cannot be parsed or the rate is not finite.
#[wasm_bindgen(js_name = rateQuoteDeposit)]
pub fn rate_quote_deposit(
    id: JsValue,
    index: JsValue,
    pillar: JsValue,
    rate: JsValue,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .string("index", &index, "index")?
        .pillar("pillar", &pillar, "pillar")?
        .number("rate", &rate, "rate")?;
    rate_quote("deposit", fields)
}

/// Build a forward-rate-agreement quote.
///
/// Free-function twin of Python `RateQuote.fra` (Rust `RateQuote::from_wire_fields`).
/// @param id - Unique quote identifier (the residual key in calibration reports).
/// @param index - Rate index identifier of the underlying floating rate.
/// @param start - Accrual start pillar: tenor string, ISO-8601 date string, or pillar object.
/// @param end - Accrual end pillar: tenor string, ISO-8601 date string, or pillar object.
/// @param rate - FRA rate as a decimal.
/// @returns A validated `RateQuote` object with `type: "fra"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a pillar cannot be parsed or the rate is not finite.
#[wasm_bindgen(js_name = rateQuoteFra)]
pub fn rate_quote_fra(
    id: JsValue,
    index: JsValue,
    start: JsValue,
    end: JsValue,
    rate: JsValue,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .string("index", &index, "index")?
        .pillar("start", &start, "start")?
        .pillar("end", &end, "end")?
        .number("rate", &rate, "rate")?;
    rate_quote("fra", fields)
}

/// Build an interest-rate futures price quote.
///
/// Free-function twin of Python `RateQuote.futures` (Rust `RateQuote::from_wire_fields`).
/// @param id - Unique quote identifier (the residual key in calibration reports).
/// @param contract - Futures contract identifier (for example `"CME:SR3"`).
/// @param expiry - ISO-8601 last trading date of the contract.
/// @param price - Futures price (for example `98.50`); the implied rate is `(100 - price) / 100`.
/// @param convexity_adjustment - Convexity adjustment as a decimal rate subtracted from the futures-implied forward (Hull convention); defaults to `0.0`.
/// @returns A validated `RateQuote` object with `type: "futures"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the expiry is not an ISO-8601 date or the price is not finite.
#[wasm_bindgen(js_name = rateQuoteFutures)]
pub fn rate_quote_futures(
    id: JsValue,
    contract: JsValue,
    expiry: JsValue,
    price: JsValue,
    convexity_adjustment: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .string("contract", &contract, "contract")?
        .date("expiry", &expiry, "expiry")?
        .number("price", &price, "price")?
        .opt_number(
            "convexity_adjustment",
            convexity_adjustment.as_ref(),
            "convexityAdjustment",
        )?;
    rate_quote("futures", fields)
}

/// Build a par swap rate quote.
///
/// Free-function twin of Python `RateQuote.swap` (Rust `RateQuote::from_wire_fields`).
/// @param id - Unique quote identifier (the residual key in calibration reports).
/// @param index - Floating-leg index identifier (for example `"USD-SOFR-OIS"`).
/// @param pillar - Swap maturity pillar: tenor string (`"5Y"`), ISO-8601 date string, or pillar object.
/// @param rate - Fixed par rate as a decimal.
/// @param spread_decimal - Optional floating-leg spread as a decimal (`0.0010` is 10 bp); omitted means no spread.
/// @returns A validated `RateQuote` object with `type: "swap"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the pillar cannot be parsed or the rate is not finite.
#[wasm_bindgen(js_name = rateQuoteSwap)]
pub fn rate_quote_swap(
    id: JsValue,
    index: JsValue,
    pillar: JsValue,
    rate: JsValue,
    spread_decimal: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .string("index", &index, "index")?
        .pillar("pillar", &pillar, "pillar")?
        .number("rate", &rate, "rate")?
        .opt_number("spread_decimal", spread_decimal.as_ref(), "spreadDecimal")?;
    rate_quote("swap", fields)
}

/// Build a running par-spread CDS quote.
///
/// Free-function twin of Python `CdsQuote.par_spread` (Rust `CdsQuote::from_wire_fields`).
/// @param id - Unique quote identifier.
/// @param entity - Reference entity name.
/// @param currency - ISO-4217 contract currency of the CDS convention.
/// @param doc_clause - ISDA documentation clause: `"isda_na"`, `"isda_eu"`, `"cr14"`, `"mr14"`, `"mm14"` or `"xr14"`.
/// @param pillar - Maturity pillar: tenor string (`"5Y"`), ISO-8601 date string, or pillar object.
/// @param spread_bp - Par spread in basis points (`80.0` is 80 bp).
/// @param recovery_rate - Assumed recovery rate as a decimal in `[0, 1)`.
/// @returns A validated `CdsQuote` object with `type: "cds_par_spread"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the convention, pillar or a numeric input is invalid.
#[wasm_bindgen(js_name = cdsQuoteParSpread)]
pub fn cds_quote_par_spread(
    id: JsValue,
    entity: JsValue,
    currency: JsValue,
    doc_clause: JsValue,
    pillar: JsValue,
    spread_bp: JsValue,
    recovery_rate: JsValue,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .string("entity", &entity, "entity")?
        .cds_convention(&currency, &doc_clause)?
        .pillar("pillar", &pillar, "pillar")?
        .number("spread_bp", &spread_bp, "spreadBp")?
        .number("recovery_rate", &recovery_rate, "recoveryRate")?;
    cds_quote("cds_par_spread", fields)
}

/// Build an upfront-plus-running-coupon CDS quote.
///
/// Free-function twin of Python `CdsQuote.upfront` (Rust `CdsQuote::from_wire_fields`).
/// @param id - Unique quote identifier.
/// @param entity - Reference entity name.
/// @param currency - ISO-4217 contract currency of the CDS convention.
/// @param doc_clause - ISDA documentation clause: `"isda_na"`, `"isda_eu"`, `"cr14"`, `"mr14"`, `"mm14"` or `"xr14"`.
/// @param pillar - Maturity pillar: tenor string (`"5Y"`), ISO-8601 date string, or pillar object.
/// @param coupon_bp - Standard running coupon in basis points (for example `100.0`).
/// @param upfront_pct - Upfront payment as a fraction of notional (`0.01` is 1%).
/// @param recovery_rate - Assumed recovery rate as a decimal in `[0, 1)`.
/// @returns A validated `CdsQuote` object with `type: "cds_upfront"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the convention, pillar or a numeric input is invalid.
#[wasm_bindgen(js_name = cdsQuoteUpfront)]
#[allow(clippy::too_many_arguments)]
pub fn cds_quote_upfront(
    id: JsValue,
    entity: JsValue,
    currency: JsValue,
    doc_clause: JsValue,
    pillar: JsValue,
    coupon_bp: JsValue,
    upfront_pct: JsValue,
    recovery_rate: JsValue,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .string("entity", &entity, "entity")?
        .cds_convention(&currency, &doc_clause)?
        .pillar("pillar", &pillar, "pillar")?
        .number("coupon_bp", &coupon_bp, "couponBp")?
        .number("upfront_pct", &upfront_pct, "upfrontPct")?
        .number("recovery_rate", &recovery_rate, "recoveryRate")?;
    cds_quote("cds_upfront", fields)
}

/// Build a listed equity or FX option implied-volatility quote.
///
/// Free-function twin of Python `VolQuote.option_vol` (Rust `VolQuote::from_wire_fields`).
/// @param id - Unique quote identifier.
/// @param underlying - Underlying identifier (ticker) the surface is keyed by.
/// @param expiry - ISO-8601 option expiry date.
/// @param strike - Absolute strike in underlying price units.
/// @param vol - Black implied volatility as an annualized decimal (`0.28` is 28%).
/// @param option_type - `"call"` or `"put"`; defaults to `"call"`.
/// @returns A validated `VolQuote` object tagged `option_vol`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the expiry is not an ISO-8601 date, the option type is unknown or a numeric input is invalid.
#[wasm_bindgen(js_name = volQuoteOptionVol)]
pub fn vol_quote_option_vol(
    id: JsValue,
    underlying: JsValue,
    expiry: JsValue,
    strike: JsValue,
    vol: JsValue,
    option_type: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .string("underlying", &underlying, "underlying")?
        .date("expiry", &expiry, "expiry")?
        .number("strike", &strike, "strike")?
        .number("vol", &vol, "vol")?
        .opt_string("option_type", option_type.as_ref(), "optionType")?;
    vol_quote("option_vol", fields)
}

/// Build a swaption volatility quote.
///
/// Free-function twin of Python `VolQuote.swaption_vol` (Rust `VolQuote::from_wire_fields`).
/// @param id - Unique quote identifier.
/// @param expiry - ISO-8601 option expiry date.
/// @param maturity - ISO-8601 maturity date of the underlying swap.
/// @param strike - Fixed strike rate as a decimal.
/// @param vol - Volatility: absolute rate volatility for normal quotes (for example `0.0072`), annualized decimal for lognormal quotes.
/// @param quote_type - `"normal"` or `"black_lognormal"`; defaults to `"normal"`.
/// @param convention - Swaption market convention identifier; defaults to `"USD"`.
/// @returns A validated `VolQuote` object tagged `swaption_vol`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601, the quote type is unknown or a numeric input is invalid.
#[wasm_bindgen(js_name = volQuoteSwaptionVol)]
pub fn vol_quote_swaption_vol(
    id: JsValue,
    expiry: JsValue,
    maturity: JsValue,
    strike: JsValue,
    vol: JsValue,
    quote_type: Option<JsValue>,
    convention: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .date("expiry", &expiry, "expiry")?
        .date("maturity", &maturity, "maturity")?
        .number("strike", &strike, "strike")?
        .number("vol", &vol, "vol")?
        .opt_string("quote_type", quote_type.as_ref(), "quoteType")?
        .opt_string("convention", convention.as_ref(), "convention")?;
    vol_quote("swaption_vol", fields)
}

/// Build a cap or floor volatility quote.
///
/// Free-function twin of Python `VolQuote.cap_floor_vol` (Rust `VolQuote::from_wire_fields`).
/// @param id - Unique quote identifier.
/// @param expiry - ISO-8601 cap/floor maturity date.
/// @param strike - Strike rate as a decimal.
/// @param vol - Volatility: absolute rate volatility for normal quotes, annualized decimal for lognormal quotes.
/// @param quote_type - `"normal"` or `"black_lognormal"`; defaults to `"normal"`.
/// @param is_cap - `true` for a cap, `false` for a floor; defaults to `true`.
/// @returns A validated `VolQuote` object tagged `cap_floor_vol`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the expiry is not an ISO-8601 date, the quote type is unknown or a numeric input is invalid.
#[wasm_bindgen(js_name = volQuoteCapFloorVol)]
pub fn vol_quote_cap_floor_vol(
    id: JsValue,
    expiry: JsValue,
    strike: JsValue,
    vol: JsValue,
    quote_type: Option<JsValue>,
    is_cap: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let fields = Fields::new()
        .string("id", &id, "id")?
        .date("expiry", &expiry, "expiry")?
        .number("strike", &strike, "strike")?
        .number("vol", &vol, "vol")?
        .opt_string("quote_type", quote_type.as_ref(), "quoteType")?
        .opt_bool("is_cap", is_cap.as_ref(), "isCap")?;
    vol_quote("cap_floor_vol", fields)
}

/// Currency-appropriate default zero-rate bounds for curve calibration.
///
/// Free-function twin of Python `RateBounds.for_currency` (Rust
/// `RateBounds::for_currency`).
/// @param currency - ISO-4217 currency code (for example `"USD"`).
/// @returns A `RateBounds` object with `min_rate` and `max_rate` as decimal zero rates.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `currency` is not a string
/// and a `FinstackError` (`kind: "validation"`) if it is not a valid ISO-4217 code.
#[wasm_bindgen(js_name = rateBoundsForCurrency)]
pub fn rate_bounds_for_currency(currency: JsValue) -> Result<JsValue, JsValue> {
    let currency: Currency = js_string(&currency, "currency")?
        .parse()
        .map_err(to_js_err)?;
    to_js_value(&RateBounds::for_currency(currency))
}

/// Wide zero-rate bounds suitable for emerging-market curves.
///
/// Free-function twin of Python `RateBounds.emerging_markets` (Rust
/// `RateBounds::emerging_markets`).
/// @returns A `RateBounds` object with `min_rate` and `max_rate` as decimal zero rates.
///
/// # Errors
///
/// Throws a `FinstackError` only if the bounds cannot be converted to a
/// JavaScript value.
#[wasm_bindgen(js_name = rateBoundsEmergingMarkets)]
pub fn rate_bounds_emerging_markets() -> Result<JsValue, JsValue> {
    to_js_value(&RateBounds::emerging_markets())
}
