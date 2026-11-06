//! Free-function twins of the Python quote and rate-bound constructors.
//!
//! Python exposes `RateQuote.deposit(...)`, `CdsQuote.par_spread(...)`,
//! `VolQuote.option_vol(...)` and `RateBounds.for_currency(...)` as static
//! methods on typed wrappers. WASM calibration inputs are plain objects, so
//! each constructor is a free function returning the serde wire object of the
//! Rust type. Both hosts convert their arguments and call the same typed Rust
//! constructors (`RateQuote::deposit`, `CdsQuote::par_spread`,
//! `VolQuote::option_vol`, ...), which own the defaults and quote validation;
//! quotes without a constructor (`cdsQuoteUpfront`, `cdsTrancheQuote`) are
//! deserialized from their serde fields and checked with their `validate()`.

use crate::utils::input::{
    from_js_json, js_f64, js_opt_bool, js_opt_f64, js_opt_string, js_string,
};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_calibration::quotes::InflationQuote;
use finstack_quant_calibration::quotes::Pillar;
use finstack_quant_calibration::quotes::RateQuote;
use finstack_quant_calibration::quotes::VolQuote;
use finstack_quant_calibration::quotes::{CdsQuote, CdsTrancheQuote, XccyQuote};
use finstack_quant_calibration::validation::RateBounds;
use finstack_quant_core::currency::Currency;
use serde_json::{Map, Value};
use wasm_bindgen::prelude::*;

fn js_pillar(value: &JsValue, label: &str) -> Result<Pillar, JsValue> {
    match value.as_string() {
        Some(text) => text.parse::<Pillar>().map_err(to_js_err),
        None => from_js_json(value, label),
    }
}

fn js_date(value: &JsValue, label: &str) -> Result<finstack_quant_core::dates::Date, JsValue> {
    parse_iso_date(&js_string(value, label)?)
}

/// Build a money-market deposit rate quote.
///
/// Free-function twin of Python `RateQuote.deposit` (Rust `RateQuote::deposit`).
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
#[wasm_bindgen(js_name = rateQuoteDeposit, unchecked_return_type = "generated.calibration.RateQuote")]
pub fn rate_quote_deposit(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] index: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string | generated.calibration.Pillar")] pillar: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
) -> Result<JsValue, JsValue> {
    let quote = RateQuote::deposit(
        js_string(&id, "id")?,
        js_string(&index, "index")?,
        js_pillar(&pillar, "pillar")?,
        js_f64(&rate, "rate")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a forward-rate-agreement quote.
///
/// Free-function twin of Python `RateQuote.fra` (Rust `RateQuote::fra`).
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
#[wasm_bindgen(js_name = rateQuoteFra, unchecked_return_type = "generated.calibration.RateQuote")]
pub fn rate_quote_fra(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] index: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string | generated.calibration.Pillar")] start: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string | generated.calibration.Pillar")] end: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
) -> Result<JsValue, JsValue> {
    let quote = RateQuote::fra(
        js_string(&id, "id")?,
        js_string(&index, "index")?,
        js_pillar(&start, "start")?,
        js_pillar(&end, "end")?,
        js_f64(&rate, "rate")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build an interest-rate futures price quote.
///
/// Free-function twin of Python `RateQuote.futures` (Rust `RateQuote::futures`).
/// @param id - Unique quote identifier (the residual key in calibration reports).
/// @param contract - Futures contract identifier (for example `"CME:SR3"`).
/// @param expiry - ISO-8601 last trading date of the contract.
/// @param price - Futures price (for example `98.50`); the implied rate is `(100 - price) / 100`.
/// @param convexityAdjustment - Convexity adjustment as a decimal rate subtracted from the futures-implied forward (Hull convention); defaults to `0.0`.
/// @returns A validated `RateQuote` object with `type: "futures"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the expiry is not an ISO-8601 date or the price is not finite.
#[wasm_bindgen(js_name = rateQuoteFutures, unchecked_return_type = "generated.calibration.RateQuote")]
pub fn rate_quote_futures(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] contract: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] price: JsValue,
    #[wasm_bindgen(
        js_name = "convexityAdjustment",
        unchecked_optional_param_type = "number | null"
    )]
    convexity_adjustment: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let quote = RateQuote::futures(
        js_string(&id, "id")?,
        js_string(&contract, "contract")?.as_str(),
        js_date(&expiry, "expiry")?,
        js_f64(&price, "price")?,
        js_opt_f64(convexity_adjustment.as_ref(), "convexityAdjustment")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a par swap rate quote.
///
/// Free-function twin of Python `RateQuote.swap` (Rust `RateQuote::swap`).
/// @param id - Unique quote identifier (the residual key in calibration reports).
/// @param index - Floating-leg index identifier (for example `"USD-SOFR-OIS"`).
/// @param pillar - Swap maturity pillar: tenor string (`"5Y"`), ISO-8601 date string, or pillar object.
/// @param rate - Fixed par rate as a decimal.
/// @param spreadDecimal - Optional floating-leg spread as a decimal (`0.0010` is 10 bp); omitted means no spread.
/// @returns A validated `RateQuote` object with `type: "swap"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the pillar cannot be parsed or the rate is not finite.
#[wasm_bindgen(js_name = rateQuoteSwap, unchecked_return_type = "generated.calibration.RateQuote")]
pub fn rate_quote_swap(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] index: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string | generated.calibration.Pillar")] pillar: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(
        js_name = "spreadDecimal",
        unchecked_optional_param_type = "number | null"
    )]
    spread_decimal: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let quote = RateQuote::swap(
        js_string(&id, "id")?,
        js_string(&index, "index")?,
        js_pillar(&pillar, "pillar")?,
        js_f64(&rate, "rate")?,
        js_opt_f64(spread_decimal.as_ref(), "spreadDecimal")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a running par-spread CDS quote.
///
/// Free-function twin of Python `CdsQuote.par_spread` (Rust `CdsQuote::par_spread`).
/// @param id - Unique quote identifier.
/// @param entity - Reference entity name.
/// @param currency - ISO-4217 contract currency of the CDS convention.
/// @param docClause - ISDA documentation clause: `"isda_na"`, `"isda_eu"`, `"cr14"`, `"mr14"`, `"mm14"` or `"xr14"`.
/// @param pillar - Maturity pillar: tenor string (`"5Y"`), ISO-8601 date string, or pillar object.
/// @param spreadBp - Par spread in basis points (`80.0` is 80 bp).
/// @param recoveryRate - Assumed recovery rate as a decimal in `[0, 1)`.
/// @returns A validated `CdsQuote` object with `type: "cds_par_spread"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the convention, pillar or a numeric input is invalid.
#[wasm_bindgen(js_name = cdsQuoteParSpread, unchecked_return_type = "generated.calibration.CdsQuote")]
pub fn cds_quote_par_spread(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] entity: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    #[wasm_bindgen(js_name = "docClause", unchecked_param_type = "string")] doc_clause: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string | generated.calibration.Pillar")] pillar: JsValue,
    #[wasm_bindgen(js_name = "spreadBp", unchecked_param_type = "number")] spread_bp: JsValue,
    #[wasm_bindgen(js_name = "recoveryRate", unchecked_param_type = "number")]
    recovery_rate: JsValue,
) -> Result<JsValue, JsValue> {
    let quote = CdsQuote::par_spread(
        js_string(&id, "id")?,
        js_string(&entity, "entity")?,
        &js_string(&currency, "currency")?,
        &js_string(&doc_clause, "docClause")?,
        js_pillar(&pillar, "pillar")?,
        js_f64(&spread_bp, "spreadBp")?,
        js_f64(&recovery_rate, "recoveryRate")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build an upfront-plus-running-coupon CDS quote.
///
/// Free-function twin of Python `CdsQuote.upfront`. The object is the Rust
/// `CdsQuote::CdsUpfront` variant without its `type` tag, which this function
/// sets to `"cds_upfront"`; the quote is then checked with `CdsQuote::validate`.
/// @param params - Object (or JSON text) holding every field of the variant, keyed by its snake_case serde name; all are required:
///   - `id` (string): Unique quote identifier, the residual label.
///   - `entity` (string): Reference entity name.
///   - `convention` (object): CDS convention key `{ currency, doc_clause }`; `currency` is an upper-case ISO-4217 code and `doc_clause` one of `"cr14"`, `"mr14"`, `"mm14"`, `"xr14"`, `"isda_na"`, `"isda_eu"`, `"isda_as"`, `"isda_au"`, `"isda_nz"` or `"custom"`.
///   - `pillar` (object): Maturity pillar in its serde form, a tenor `{ tenor: { count: 5, unit: "years" } }` (units `"days"`, `"weeks"`, `"months"`, `"years"`) or a date `{ date: "2029-06-20" }`.
///   - `coupon_bp` (number): Standard running coupon in basis points (for example `100.0`).
///   - `upfront_pct` (number): Upfront payment as a decimal fraction of notional (`0.01` is 1%), positive when the protection buyer pays.
///   - `recovery_rate` (number): Assumed recovery rate as a decimal in `[0, 1)`.
/// @returns A validated `CdsQuote` object with `type: "cds_upfront"`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `params` is neither an
/// object nor JSON text or holds a non-finite number, and a `FinstackError`
/// (`kind: "validation"`) if a field is missing, unknown or malformed (an
/// unknown currency or doc clause, a bad pillar) or the quote fails validation.
#[wasm_bindgen(js_name = cdsQuoteUpfront, unchecked_return_type = "generated.calibration.CdsQuote")]
pub fn cds_quote_upfront(
    #[wasm_bindgen(
        unchecked_param_type = "Omit<Extract<generated.calibration.CdsQuote, { type: \"cds_upfront\" }>, \"type\"> | string"
    )]
    params: JsValue,
) -> Result<JsValue, JsValue> {
    let mut fields: Map<String, Value> = from_js_json(&params, "params")?;
    fields.insert("type".to_string(), Value::from("cds_upfront"));
    let quote: CdsQuote = serde_json::from_value(Value::Object(fields)).map_err(|error| {
        to_js_err(finstack_quant_core::Error::Validation(format!(
            "params: {error}"
        )))
    })?;
    quote.validate().map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a listed equity or FX option implied-volatility quote.
///
/// Free-function twin of Python `VolQuote.option_vol` (Rust `VolQuote::option_vol`).
/// @param id - Unique quote identifier.
/// @param underlying - Underlying identifier (ticker) the surface is keyed by.
/// @param expiry - ISO-8601 option expiry date.
/// @param strike - Absolute strike in underlying price units.
/// @param vol - Black implied volatility as an annualized decimal (`0.28` is 28%).
/// @param optionType - `"call"` or `"put"`; defaults to `"call"`.
/// @returns A validated `VolQuote` object tagged `option_vol`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the expiry is not an ISO-8601 date, the option type is unknown or a numeric input is invalid.
#[wasm_bindgen(js_name = volQuoteOptionVol, unchecked_return_type = "generated.calibration.VolQuote")]
pub fn vol_quote_option_vol(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] underlying: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(
        js_name = "optionType",
        unchecked_optional_param_type = "string | null"
    )]
    option_type: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let option_type = js_opt_string(option_type.as_ref(), "optionType")?;
    let quote = VolQuote::option_vol(
        js_string(&id, "id")?,
        js_string(&underlying, "underlying")?,
        js_date(&expiry, "expiry")?,
        js_f64(&strike, "strike")?,
        js_f64(&vol, "vol")?,
        option_type.as_deref(),
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a swaption volatility quote.
///
/// Free-function twin of Python `VolQuote.swaption_vol` (Rust `VolQuote::swaption_vol`).
/// @param id - Unique quote identifier.
/// @param expiry - ISO-8601 option expiry date.
/// @param maturity - ISO-8601 maturity date of the underlying swap.
/// @param strike - Fixed strike rate as a decimal.
/// @param vol - Volatility: absolute rate volatility for normal quotes (for example `0.0072`), annualized decimal for lognormal quotes.
/// @param quoteType - `"normal"` (`vol` is an absolute rate volatility) or `"black_lognormal"` (`vol` is a relative volatility); required, because it decides the unit of `vol`.
/// @param convention - Swaption market convention identifier; omitted uses the Rust default `VolQuote::DEFAULT_SWAPTION_CONVENTION` (`"USD"`).
/// @returns A validated `VolQuote` object tagged `swaption_vol`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if a date is not ISO-8601, the quote type is unknown or a numeric input is invalid.
#[wasm_bindgen(js_name = volQuoteSwaptionVol, unchecked_return_type = "generated.calibration.VolQuote")]
pub fn vol_quote_swaption_vol(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(js_name = "quoteType", unchecked_param_type = "string")] quote_type: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] convention: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let convention = js_opt_string(convention.as_ref(), "convention")?;
    let quote = VolQuote::swaption_vol(
        js_string(&id, "id")?,
        js_date(&expiry, "expiry")?,
        js_date(&maturity, "maturity")?,
        js_f64(&strike, "strike")?,
        js_f64(&vol, "vol")?,
        &js_string(&quote_type, "quoteType")?,
        convention.as_deref(),
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a cap or floor volatility quote.
///
/// Free-function twin of Python `VolQuote.cap_floor_vol` (Rust `VolQuote::cap_floor_vol`).
/// @param id - Unique quote identifier.
/// @param expiry - ISO-8601 cap/floor maturity date.
/// @param strike - Strike rate as a decimal.
/// @param vol - Volatility: absolute rate volatility for normal quotes, annualized decimal for lognormal quotes.
/// @param quoteType - `"normal"` (`vol` is an absolute rate volatility) or `"black_lognormal"` (`vol` is a relative volatility); required, because it decides the unit of `vol`.
/// @param isCap - `true` for a cap, `false` for a floor; defaults to `true`.
/// @returns A validated `VolQuote` object tagged `cap_floor_vol`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the expiry is not an ISO-8601 date, the quote type is unknown or a numeric input is invalid.
#[wasm_bindgen(js_name = volQuoteCapFloorVol, unchecked_return_type = "generated.calibration.VolQuote")]
pub fn vol_quote_cap_floor_vol(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(js_name = "quoteType", unchecked_param_type = "string")] quote_type: JsValue,
    #[wasm_bindgen(js_name = "isCap", unchecked_optional_param_type = "boolean | null")]
    is_cap: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let quote = VolQuote::cap_floor_vol(
        js_string(&id, "id")?,
        js_date(&expiry, "expiry")?,
        js_f64(&strike, "strike")?,
        js_f64(&vol, "vol")?,
        &js_string(&quote_type, "quoteType")?,
        js_opt_bool(is_cap.as_ref(), "isCap")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
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
#[wasm_bindgen(js_name = rateBoundsForCurrency, unchecked_return_type = "generated.calibration.RateBounds")]
pub fn rate_bounds_for_currency(
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
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
#[wasm_bindgen(js_name = rateBoundsEmergingMarkets, unchecked_return_type = "generated.calibration.RateBounds")]
pub fn rate_bounds_emerging_markets() -> Result<JsValue, JsValue> {
    to_js_value(&RateBounds::emerging_markets())
}

/// Build a zero-coupon inflation swap quote.
///
/// Free-function twin of Python `InflationQuote.inflation_swap` (Rust `InflationQuote::inflation_swap`).
/// @param id - Unique quote identifier.
/// @param maturity - ISO-8601 swap maturity date.
/// @param rate - Fixed zero-coupon swap rate as a decimal (`0.025` is 2.5%).
/// @param index - Inflation index identifier (for example `"USA-CPI-U"`).
/// @param convention - Inflation-swap convention identifier (for example `"USD"`).
/// @returns A validated `InflationQuote` object tagged `inflation_swap`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the maturity is not an ISO-8601 date or the rate is not finite.
#[wasm_bindgen(js_name = inflationQuoteInflationSwap, unchecked_return_type = "generated.calibration.InflationQuote")]
pub fn inflation_quote_inflation_swap(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] index: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] convention: JsValue,
) -> Result<JsValue, JsValue> {
    let quote = InflationQuote::inflation_swap(
        js_string(&id, "id")?,
        js_date(&maturity, "maturity")?,
        js_f64(&rate, "rate")?,
        js_string(&index, "index")?,
        js_string(&convention, "convention")?.as_str(),
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a year-on-year inflation swap quote.
///
/// Free-function twin of Python `InflationQuote.yoy_inflation_swap` (Rust `InflationQuote::yoy_inflation_swap`).
/// @param id - Unique quote identifier.
/// @param maturity - ISO-8601 swap maturity date.
/// @param rate - Fixed year-on-year swap rate as a decimal.
/// @param index - Inflation index identifier (for example `"USA-CPI-U"`).
/// @param frequency - Payment frequency tenor (for example `"1Y"`).
/// @param convention - Inflation-swap convention identifier (for example `"USD"`).
/// @returns A validated `InflationQuote` object tagged `yoy_inflation_swap`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and
/// a `FinstackError` (`kind: "validation"`) if the maturity is not an ISO-8601 date, the frequency is not a tenor or the rate is not finite.
#[wasm_bindgen(js_name = inflationQuoteYoyInflationSwap, unchecked_return_type = "generated.calibration.InflationQuote")]
pub fn inflation_quote_yoy_inflation_swap(
    #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] index: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] frequency: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] convention: JsValue,
) -> Result<JsValue, JsValue> {
    let quote = InflationQuote::yoy_inflation_swap(
        js_string(&id, "id")?,
        js_date(&maturity, "maturity")?,
        js_f64(&rate, "rate")?,
        js_string(&index, "index")?,
        &js_string(&frequency, "frequency")?,
        js_string(&convention, "convention")?.as_str(),
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a validated cross-currency basis-swap quote using the canonical Rust constructor.
///
/// # Arguments
///
/// * `fields` - Quote fields: identifier, convention identifier, maturity pillar, basis spread in basis points, and optional positive spot FX in quote currency per base currency. Omitted spot FX uses market FX during instrument construction.
///
/// @returns The validated canonical cross-currency quote object.
///
/// # Errors
///
/// Throws `TypeError` for non-JSON values including nonfinite numbers, or
/// `FinstackError` with kind `validation` for malformed fields, an invalid pillar,
/// or nonpositive spot FX. Convention identifiers are resolved during calibration.
/// @param fields - Quote fields: identifier, convention identifier, maturity pillar, basis spread in basis points, and optional positive spot FX in quote currency per base currency. Omitted spot FX uses market FX during instrument construction.
#[wasm_bindgen(js_name = xccyQuote, unchecked_return_type = "generated.calibration.XccyQuote")]
pub fn xccy_quote(
    #[wasm_bindgen(unchecked_param_type = "generated.calibration.XccyQuote")] fields: JsValue,
) -> Result<JsValue, JsValue> {
    let fields: XccyQuote = from_js_json(&fields, "fields")?;
    let quote = XccyQuote::new(
        fields.id,
        fields.convention,
        fields.far_pillar,
        fields.basis_spread_bp,
        fields.spot_fx,
    )
    .map_err(to_js_err)?;
    to_js_value(&quote)
}

/// Build a validated CDS index tranche quote from its serde fields (checked with `CdsTrancheQuote::validate`).
///
/// # Arguments
///
/// * `fields` - Quote fields including the index and series, ISO maturity date, decimal attachment and detachment in [0, 1] with attachment below detachment, decimal upfront in [-1, 1], positive running coupon in basis points, and currency/documentation convention.
///
/// @returns The validated canonical CDS index tranche quote object.
///
/// # Errors
///
/// Throws `TypeError` for non-JSON values including nonfinite numbers, or
/// `FinstackError` with kind `validation` for malformed fields, invalid dates or
/// conventions, or violated tranche, upfront, or coupon bounds.
/// @param fields - Quote fields including the index and series, ISO maturity date, decimal attachment and detachment in [0, 1] with attachment below detachment, decimal upfront in [-1, 1], positive running coupon in basis points, and currency/documentation convention.
#[wasm_bindgen(js_name = cdsTrancheQuote, unchecked_return_type = "generated.calibration.CdsTrancheQuote")]
pub fn cds_tranche_quote(
    #[wasm_bindgen(unchecked_param_type = "generated.calibration.CdsTrancheQuote")] fields: JsValue,
) -> Result<JsValue, JsValue> {
    let quote: CdsTrancheQuote = from_js_json(&fields, "fields")?;
    quote.validate().map_err(to_js_err)?;
    to_js_value(&quote)
}
