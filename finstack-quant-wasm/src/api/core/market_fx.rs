//! WASM bindings for `finstack_quant_core::money::fx`: conversion policy,
//! rate results, explicit pair quote metadata and the `FxMatrix` handle.

use std::sync::Arc;

use crate::api::core::currency::JsCurrency;
use crate::utils::input::{from_js_json, js_currency, js_f64, js_string};
use crate::utils::to_js_err;
use crate::utils::wire::js_date;
use finstack_quant_core::currency::Currency as RustCurrency;
use finstack_quant_core::money::fx::{
    CurrencyPair, FxConversionPolicy as RustFxConversionPolicy, FxMatrix as RustFxMatrix,
    FxPairConvention as RustFxPairConvention, FxQuery, FxRateResult as RustFxRateResult,
    invert_fx_rate as rust_invert_fx_rate,
};
use wasm_bindgen::prelude::*;

/// Typed FX conversion policy wrapper for WASM callers.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const policy = core.FxConversionPolicy.cashflowDate();
/// console.log(policy.toString());
/// ```
#[wasm_bindgen(js_name = FxConversionPolicy)]
#[derive(Clone, Copy, Debug)]
pub struct JsFxConversionPolicy {
    inner: RustFxConversionPolicy,
}

#[wasm_bindgen(js_class = FxConversionPolicy)]
impl JsFxConversionPolicy {
    /// Use spot/forward on the cashflow date.
    /// @returns An `FxConversionPolicy` handle.
    #[wasm_bindgen(js_name = cashflowDate)]
    pub fn cashflow_date() -> Self {
        Self {
            inner: RustFxConversionPolicy::CashflowDate,
        }
    }

    /// Use period end date.
    /// @returns An `FxConversionPolicy` handle.
    #[wasm_bindgen(js_name = periodEnd)]
    pub fn period_end() -> Self {
        Self {
            inner: RustFxConversionPolicy::PeriodEnd,
        }
    }

    /// Parse from a string label such as ``\"cashflow_date\"``.
    /// @param name - Policy label: `cashflow_date` or `period_end`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception unless `name` is `cashflow_date` or
    /// `period_end`.
    /// @returns An `FxConversionPolicy` handle.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(
        #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    ) -> Result<Self, JsValue> {
        let name: &str = &js_string(&name, "name")?;
        Ok(Self {
            inner: name.parse().map_err(to_js_err)?,
        })
    }

    /// String form of the conversion policy.
    /// @returns Human-readable string form of this value.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Structured FX lookup result for WASM callers.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const matrix = new core.FxMatrix(null);
/// matrix.setQuotes([["EUR", "USD", 1.1]]);
/// const result = matrix.rate(
///   "EUR",
///   "USD",
///   "2026-01-02",
///   core.FxConversionPolicy.cashflowDate()
/// );
/// console.log(result.rate, result.triangulated);
/// ```
#[wasm_bindgen(js_name = FxRateResult)]
pub struct JsFxRateResult {
    pub(crate) inner: RustFxRateResult,
}

#[wasm_bindgen(js_class = FxRateResult)]
impl JsFxRateResult {
    /// The FX conversion rate.
    #[wasm_bindgen(getter, js_name = rate)]
    pub fn rate(&self) -> f64 {
        self.inner.rate
    }

    /// Whether the rate was obtained via triangulation.
    #[wasm_bindgen(getter, js_name = triangulated)]
    pub fn triangulated(&self) -> bool {
        self.inner.triangulated
    }

    /// Serialize to the canonical JSON wire form shared with Python `FxRateResult.to_json`.
    ///
    /// @returns Compact JSON text with `rate` and `triangulated`.
    /// @throws If serialization fails (not expected for a valid result).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical FxRateResult JSON text or plain object; unknown fields are rejected.
    ///
    /// @returns The parsed `FxRateResult`.
    /// @throws If `json` is malformed or has unknown or missing fields.
    /// @param json - Canonical FxRateResult JSON text or plain object; unknown fields are rejected.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsFxRateResult, JsValue> {
        from_js_json::<RustFxRateResult>(&json, "json").map(|inner| JsFxRateResult { inner })
    }
}

/// Validated quote metadata for one ordered FX pair.
///
/// The supplied order is kept (an outright rate is quote units per one base
/// unit) and the pip size is explicit; nothing is inferred from the
/// currencies (record CORE-CONVENTIONS-011 #2).
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const conv = new core.FxPairConvention("EUR", "USD", 0.0001);
/// conv.base.toString(); // "EUR"
/// conv.pipSize;         // 0.0001
/// conv.ratePoints(50);  // 0.005
/// ```
#[wasm_bindgen(js_name = FxPairConvention)]
#[derive(Clone, Copy, Debug)]
pub struct JsFxPairConvention {
    pub(crate) inner: RustFxPairConvention,
}

#[wasm_bindgen(js_class = FxPairConvention)]
impl JsFxPairConvention {
    /// Quote metadata for the ordered pair `base`/`quote`.
    ///
    /// # Arguments
    ///
    /// * `base` - Base currency ISO-4217 code.
    /// * `quote` - Quote currency ISO-4217 code; must differ from `base`.
    /// * `pip_size` - Outright-rate increment per pip, finite and positive.
    ///
    /// @param base - Base currency ISO-4217 code: one unit of it is quoted.
    /// @param quote - Quote currency ISO-4217 code; must differ from `base`.
    /// @param pipSize - Outright-rate increment per pip in this pair's quote units; finite and positive.
    /// @returns A validated `FxPairConvention`.
    /// @throws `TypeError` (kind `invalid_type`) for a non-string code or non-number size; `FinstackError` (kind `validation`) for equal currencies or a non-positive or non-finite pip size.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string")] base: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] quote: JsValue,
        #[wasm_bindgen(js_name = "pipSize", unchecked_param_type = "number")] pip_size: JsValue,
    ) -> Result<JsFxPairConvention, JsValue> {
        let base = js_currency(&base, "base")?;
        let quote = js_currency(&quote, "quote")?;
        let pip_size = js_f64(&pip_size, "pipSize")?;
        RustFxPairConvention::new(base, quote, pip_size)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Base currency of the ordered pair.
    /// @returns The base `Currency`.
    #[wasm_bindgen(getter, js_name = base)]
    pub fn base(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.get_base(),
        }
    }

    /// Quote currency of the ordered pair.
    /// @returns The quote `Currency`.
    #[wasm_bindgen(getter, js_name = quote)]
    pub fn quote(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.get_quote(),
        }
    }

    /// Outright-rate increment per pip.
    /// @returns The pip size in quote units per base unit.
    #[wasm_bindgen(getter, js_name = pipSize)]
    pub fn pip_size(&self) -> f64 {
        self.inner.get_pip_size()
    }

    /// Rate points for a signed number of forward pips (`pips * pipSize`).
    ///
    /// # Arguments
    ///
    /// * `pips` - Signed forward points in pips; must be finite.
    ///
    /// @param pips - Signed forward points in pips; must be finite.
    /// @returns The rate points.
    /// @throws `TypeError` (kind `invalid_type`) if `pips` is not a number; `FinstackError` (kind `validation`) if it or the product is not finite.
    #[wasm_bindgen(js_name = ratePoints)]
    pub fn rate_points(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] pips: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .rate_points(js_f64(&pips, "pips")?)
            .map_err(to_js_err)
    }
}

/// Reciprocal of a strictly positive finite FX rate.
/// @param rate - Outright FX rate to invert, in quote-per-base units. Must be
/// finite and strictly positive; the reciprocal must also be a valid FX rate.
/// @returns `1 / rate` when that reciprocal is a valid FX rate.
///
/// # Errors
///
/// Throws a `validation` error if `rate` is non-finite, zero or negative, or
/// its reciprocal overflows.
#[wasm_bindgen(js_name = invertFxRate)]
pub fn invert_fx_rate(
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
) -> Result<f64, JsValue> {
    let rate = js_f64(&rate, "rate")?;
    rust_invert_fx_rate(rate).map_err(to_js_err)
}

/// Foreign-exchange rate matrix for currency conversion.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const matrix = new core.FxMatrix(null);
/// matrix.setQuotes([["EUR", "USD", 1.1]]);
/// const policy = core.FxConversionPolicy.cashflowDate();
/// console.log(matrix.rate("EUR", "USD", "2026-01-02", policy).rate);
/// ```
#[wasm_bindgen(js_name = FxMatrix)]
pub struct JsFxMatrix {
    pub(crate) inner: Arc<RustFxMatrix>,
}

/// The required routing choice: `null` disables routing, a string names the
/// pivot, and `undefined` (an omitted choice) is rejected.
fn required_pivot(
    value: &JsValue,
    label: &str,
) -> Result<Option<finstack_quant_core::currency::Currency>, JsValue> {
    if value.is_undefined() {
        return Err(crate::utils::input::invalid_type(
            label,
            "a currency code or null (no default pivot)",
        ));
    }
    if value.is_null() {
        return Ok(None);
    }
    crate::utils::input::js_currency(value, label).map(Some)
}

#[wasm_bindgen(js_class = FxMatrix)]
impl JsFxMatrix {
    /// Create an empty FX matrix with an explicit routing choice (Rust
    /// `FxMatrix::new`): rates come only from quotes set on it.
    ///
    /// # Arguments
    ///
    /// * `pivot_currency` - The single cross-routing pivot, or `null` for
    ///   direct quotes only; required (no default pivot).
    ///
    /// @param pivotCurrency - The single cross-routing pivot such as `"USD"`, or `null` for direct quotes only; required (no default pivot).
    /// @returns A new empty `FxMatrix`.
    /// @throws `TypeError` (kind `invalid_type`) if `pivotCurrency` is omitted or not a string or `null`; `FinstackError` (kind `validation`) for an unknown currency code.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "pivotCurrency", unchecked_param_type = "string | null")]
        pivot_currency: JsValue,
    ) -> Result<JsFxMatrix, JsValue> {
        Ok(Self {
            inner: Arc::new(RustFxMatrix::new(
                Arc::new(finstack_quant_core::money::fx::SimpleFxProvider::new()),
                required_pivot(&pivot_currency, "pivotCurrency")?,
            )),
        })
    }

    /// Set several pair-global quotes atomically (Rust `FxMatrix::set_quotes`).
    ///
    /// # Arguments
    ///
    /// * `quotes` - Array of `[base, quote, rate]` triples: two ISO-4217
    ///   currency codes and the finite, strictly positive number of quote
    ///   units per one base unit.
    ///
    /// @throws `TypeError` (kind `invalid_type`) if `quotes` is not an array
    /// (or its JSON text); `FinstackError` (kind `validation`) for a triple of
    /// the wrong shape, an unknown currency code, or a non-positive or
    /// non-finite rate. On an error none of the batch is applied.
    /// @param quotes - Array of `[base, quote, rate]` triples: two ISO-4217 currency codes and the finite, strictly positive number of quote units per one base unit.
    #[wasm_bindgen(js_name = setQuotes)]
    pub fn set_quotes(
        &self,
        #[wasm_bindgen(
            unchecked_param_type = "readonly (readonly [string, string, number])[] | string"
        )]
        quotes: JsValue,
    ) -> Result<(), JsValue> {
        let quotes = from_js_json::<Vec<(String, String, f64)>>(&quotes, "quotes")?
            .into_iter()
            .map(|(base, quote, rate)| {
                Ok((
                    base.parse::<RustCurrency>().map_err(to_js_err)?,
                    quote.parse::<RustCurrency>().map_err(to_js_err)?,
                    rate,
                ))
            })
            .collect::<Result<Vec<_>, JsValue>>()?;
        self.inner.set_quotes(&quotes).map_err(to_js_err)
    }

    /// Build a matrix from quotes keyed by currency pair (Rust `CurrencyPair`
    /// parsing plus `FxMatrix::from_quotes`).
    ///
    /// # Arguments
    ///
    /// * `quotes` - Plain object (or its JSON text) mapping a pair to its
    ///   rate, such as `{ "EUR/USD": 1.1, "GBPUSD": 1.27 }`. A key is
    ///   `"BASE/QUOTE"` or the six-letter compact form; the rate is the
    ///   finite, strictly positive number of quote units per one base unit.
    /// * `pivot_currency` - The single cross-routing pivot, or `null` for
    ///   direct quotes only; required (no default pivot).
    ///
    /// @returns A new `FxMatrix` holding every quote.
    /// @throws `TypeError` (kind `invalid_type`) if `quotes` is not a plain
    /// object or JSON text; `FinstackError` (kind `validation`) for a malformed
    /// pair key, an unknown currency code, or a non-positive or non-finite rate.
    /// @param quotes - Plain object (or its JSON text) mapping a pair to its rate, such as `{ "EUR/USD": 1.1, "GBPUSD": 1.27 }`. A key is `"BASE/QUOTE"` or the six-letter compact form; the rate is the finite, strictly positive number of quote units per one base unit.
    /// @param pivotCurrency - The single cross-routing pivot such as `"USD"`, or `null` for direct quotes only; required (no default pivot).
    #[wasm_bindgen(js_name = fromDict)]
    pub fn from_dict(
        #[wasm_bindgen(unchecked_param_type = "Record<string, number> | string")] quotes: JsValue,
        #[wasm_bindgen(js_name = "pivotCurrency", unchecked_param_type = "string | null")]
        pivot_currency: JsValue,
    ) -> Result<JsFxMatrix, JsValue> {
        let quotes = from_js_json::<std::collections::BTreeMap<String, f64>>(&quotes, "quotes")?
            .into_iter()
            .map(|(pair, rate)| {
                let pair: CurrencyPair = pair.parse().map_err(to_js_err)?;
                Ok((pair.base, pair.quote, rate))
            })
            .collect::<Result<Vec<_>, JsValue>>()?;
        RustFxMatrix::from_quotes(&quotes, required_pivot(&pivot_currency, "pivotCurrency")?)
            .map(|matrix| Self {
                inner: Arc::new(matrix),
            })
            .map_err(to_js_err)
    }

    /// Set an authoritative quote scoped to one date and conversion policy.
    /// For its date and policy the pinned quote takes priority over a
    /// pair-global quote in either orientation and over provider observations.
    /// @param base - Base currency code of the FX quote, where the rate is quote per base.
    /// @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
    /// @param date - ISO-8601 date used by the calculation or market-data lookup.
    /// @param policy - FX quote-selection policy for resolving direct, inverse, or triangulated rates.
    /// @param rate - Finite positive quote-currency units per one base-currency unit.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code is invalid, `date`
    /// is not a valid ISO date, or `rate` is non-finite or not strictly positive.
    #[wasm_bindgen(js_name = setQuoteOn)]
    pub fn set_quote_on(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] base: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] quote: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] date: JsValue,
        policy: &JsFxConversionPolicy,
        #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    ) -> Result<(), JsValue> {
        let rate = js_f64(&rate, "rate")?;
        self.inner
            .set_quote_on(
                js_currency(&base, "base")?,
                js_currency(&quote, "quote")?,
                js_date(&date, "date")?,
                policy.inner,
                rate,
            )
            .map_err(to_js_err)
    }

    /// Look up an FX rate.
    /// Pinned fixings for the date and policy precede global quotes, then
    /// provider observations, resolving source priority before taking a
    /// reciprocal. Only a pair the provider reports missing is routed through
    /// the pivot; any other provider error is thrown unchanged.
    ///
    /// The policy is required: there is no default conversion policy.
    ///
    /// # Arguments
    /// * `base` - Base (from) currency ISO code.
    /// * `quote` - Quote (to) currency ISO code.
    /// * `date` - ISO date string.
    /// * `policy` - Required reusable conversion policy handle.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code or `date` is invalid,
    /// no direct, inverse, or triangulated quote is available, or a resolved quote
    /// is non-finite or non-positive.
    /// @param base - Base (from) currency ISO code.
    /// @param quote - Quote (to) currency ISO code.
    /// @param date - ISO date string.
    /// @param policy - Required reusable conversion policy handle.
    /// @returns Resolved FX rate, including whether it was triangulated.
    pub fn rate(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] base: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] quote: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] date: JsValue,
        policy: &JsFxConversionPolicy,
    ) -> Result<JsFxRateResult, JsValue> {
        let query = FxQuery::new(
            js_currency(&base, "base")?,
            js_currency(&quote, "quote")?,
            js_date(&date, "date")?,
            policy.inner,
        );
        self.inner
            .rate(query)
            .map(|inner| JsFxRateResult { inner })
            .map_err(to_js_err)
    }
}
