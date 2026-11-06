//! WASM bindings for `finstack_quant_core::money::fx`: conversion policy,
//! rate results, market pair conventions and the `FxMatrix` handle.

use std::sync::Arc;

use crate::api::core::currency::JsCurrency;
use crate::utils::input::{from_js_json, js_currency, js_f64, js_string};
use crate::utils::to_js_err;
use crate::utils::wire::js_date;
use finstack_quant_core::currency::Currency as RustCurrency;
use finstack_quant_core::money::fx::{
    CurrencyPair, FxConversionPolicy as RustFxConversionPolicy, FxMatrix as RustFxMatrix,
    FxPairConvention as RustFxPairConvention, FxQuery, FxQuoteConvention as RustFxQuoteConvention,
    FxRateResult as RustFxRateResult, fx_pair_convention as rust_fx_pair_convention,
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
#[derive(Clone, Copy, Debug, Default)]
pub struct JsFxConversionPolicy {
    inner: RustFxConversionPolicy,
}

#[wasm_bindgen(js_class = FxConversionPolicy)]
impl JsFxConversionPolicy {
    /// The Rust default policy (`FxConversionPolicy::default()`, cashflow date).
    ///
    /// @returns The default conversion policy, the one `FxMatrix.rate` uses
    /// when `policy` is omitted.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Use an average over the period.
    /// @returns An `FxConversionPolicy` handle.
    #[wasm_bindgen(js_name = periodAverage)]
    pub fn period_average() -> Self {
        Self {
            inner: RustFxConversionPolicy::PeriodAverage,
        }
    }

    /// Parse from a string label such as ``\"cashflow_date\"``.
    /// @param name - Policy label: `cashflow_date`, `period_end`, or `period_average`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception unless `name` is `cashflow_date`,
    /// `period_end`, or `period_average`.
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
/// const matrix = new core.FxMatrix();
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

/// USD quotation style for a market FX pair (Direct or Indirect versus USD).
///
/// **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
/// means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
/// quotation of market CCY1 versus USD.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const direct = core.FxQuoteConvention.direct();
/// direct.toString(); // "direct"
/// ```
#[wasm_bindgen(js_name = FxQuoteConvention)]
#[derive(Clone, Copy, Debug)]
pub struct JsFxQuoteConvention {
    inner: RustFxQuoteConvention,
}

#[wasm_bindgen(js_class = FxQuoteConvention)]
impl JsFxQuoteConvention {
    /// USD is the quote currency (units of USD per one unit of CCY1).
    /// @returns An `FxQuoteConvention` handle.
    pub fn direct() -> Self {
        Self {
            inner: RustFxQuoteConvention::Direct,
        }
    }

    /// USD is the base currency (units of CCY2 per one USD).
    /// @returns An `FxQuoteConvention` handle.
    pub fn indirect() -> Self {
        Self {
            inner: RustFxQuoteConvention::Indirect,
        }
    }

    /// Parse from a string label such as `"direct"` or `"indirect"`.
    /// @param name - Convention label: `direct` or `indirect`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception unless `name` is `direct` or `indirect`.
    /// @returns An `FxQuoteConvention` handle.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(
        #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    ) -> Result<Self, JsValue> {
        let name: &str = &js_string(&name, "name")?;
        Ok(Self {
            inner: name.parse().map_err(to_js_err)?,
        })
    }

    /// String form of the USD quotation style (`"direct"` or `"indirect"`).
    /// @returns Human-readable string form of this value.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.
///
/// Instances come from `fxPairConvention`. `base` / `quote` are always market
/// CCY1/CCY2, even when the lookup arguments were inverted.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const conv = core.fxPairConvention("USD", "EUR");
/// conv.base.toString();          // "EUR"
/// conv.usdQuotation.toString(); // "direct"
/// conv.pipSize;            // 0.0001
/// conv.settlementDays;     // 2
/// ```
#[wasm_bindgen(js_name = FxPairConvention)]
#[derive(Clone, Copy, Debug)]
pub struct JsFxPairConvention {
    inner: RustFxPairConvention,
}

#[wasm_bindgen(js_class = FxPairConvention)]
impl JsFxPairConvention {
    /// Market CCY1 (one unit of this currency in the screen pair).
    #[wasm_bindgen(getter, js_name = base)]
    pub fn base(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.base,
        }
    }

    /// Market CCY2 (units of this currency per one unit of CCY1).
    #[wasm_bindgen(getter, js_name = quote)]
    pub fn quote(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.quote,
        }
    }

    /// Direct if the USD leg quotes USD as CCY2; Indirect if USD is CCY1.
    #[wasm_bindgen(getter, js_name = usdQuotation)]
    pub fn usd_quotation(&self) -> JsFxQuoteConvention {
        JsFxQuoteConvention {
            inner: self.inner.usd_quotation,
        }
    }

    /// Pip size in outright-rate units (`0.01` or `0.0001`).
    #[wasm_bindgen(getter, js_name = pipSize)]
    pub fn pip_size(&self) -> f64 {
        self.inner.pip_size
    }

    /// Standard spot lag in business days (T+1 or T+2).
    #[wasm_bindgen(getter, js_name = settlementDays)]
    pub fn settlement_days(&self) -> u32 {
        self.inner.settlement_days
    }
}

/// Market convention for an unordered currency pair.
///
/// Returned `base` / `quote` are always the market CCY1/CCY2, even when the
/// arguments are inverted.
/// @param base - One currency ISO code of the pair. Orientation is ignored.
/// @param quote - The other currency ISO code of the pair. Orientation is ignored.
/// @returns Market CCY1/CCY2, USD quotation, pip size, and standard spot lag.
///
/// # Errors
///
/// Throws a JavaScript exception if either code is not a recognized ISO-4217
/// alphabetic currency.
#[wasm_bindgen(js_name = fxPairConvention)]
pub fn fx_pair_convention(
    #[wasm_bindgen(unchecked_param_type = "string")] base: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] quote: JsValue,
) -> Result<JsFxPairConvention, JsValue> {
    let base = js_currency(&base, "base")?;
    let quote = js_currency(&quote, "quote")?;
    Ok(JsFxPairConvention {
        inner: rust_fx_pair_convention(base, quote),
    })
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
/// const matrix = new core.FxMatrix();
/// matrix.setQuotes([["EUR", "USD", 1.1]]);
/// console.log(matrix.rate("EUR", "USD", "2026-01-02").rate);
/// ```
#[wasm_bindgen(js_name = FxMatrix)]
pub struct JsFxMatrix {
    pub(crate) inner: Arc<RustFxMatrix>,
}

impl Default for JsFxMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_class = FxMatrix)]
impl JsFxMatrix {
    /// Create an empty FX matrix (Rust `FxMatrix::default`): rates come only
    /// from quotes set on it.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RustFxMatrix::default()),
        }
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
    ///
    /// @returns A new `FxMatrix` holding every quote.
    /// @throws `TypeError` (kind `invalid_type`) if `quotes` is not a plain
    /// object or JSON text; `FinstackError` (kind `validation`) for a malformed
    /// pair key, an unknown currency code, or a non-positive or non-finite rate.
    /// @param quotes - Plain object (or its JSON text) mapping a pair to its rate, such as `{ "EUR/USD": 1.1, "GBPUSD": 1.27 }`. A key is `"BASE/QUOTE"` or the six-letter compact form; the rate is the finite, strictly positive number of quote units per one base unit.
    #[wasm_bindgen(js_name = fromDict)]
    pub fn from_dict(
        #[wasm_bindgen(unchecked_param_type = "Record<string, number> | string")] quotes: JsValue,
    ) -> Result<JsFxMatrix, JsValue> {
        let quotes = from_js_json::<std::collections::BTreeMap<String, f64>>(&quotes, "quotes")?
            .into_iter()
            .map(|(pair, rate)| {
                let pair: CurrencyPair = pair.parse().map_err(to_js_err)?;
                Ok((pair.base, pair.quote, rate))
            })
            .collect::<Result<Vec<_>, JsValue>>()?;
        RustFxMatrix::from_quotes(&quotes)
            .map(|matrix| Self {
                inner: Arc::new(matrix),
            })
            .map_err(to_js_err)
    }

    /// Set an authoritative quote scoped to one date and conversion policy.
    /// Pair-global quotes in either orientation take priority; pinned quotes
    /// precede provider observations.
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
    /// Global quotes precede pinned fixings, then provider observations,
    /// resolving source priority before taking a reciprocal.
    ///
    /// The published facade makes `policy` optional: when omitted it passes
    /// `new FxConversionPolicy()`, the Rust default (cashflow date).
    ///
    /// # Arguments
    /// * `base` - Base (from) currency ISO code.
    /// * `quote` - Quote (to) currency ISO code.
    /// * `date` - ISO date string.
    /// * `policy` - Reusable conversion policy handle; omitted means the Rust
    ///   default (cashflow date).
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code or `date` is invalid,
    /// no direct, inverse, or triangulated quote is available, or a resolved quote
    /// is non-finite or non-positive.
    /// @param base - Base (from) currency ISO code.
    /// @param quote - Quote (to) currency ISO code.
    /// @param date - ISO date string.
    /// @param policy - Reusable conversion policy handle; omitted means the Rust default (cashflow date).
    /// @returns Resolved FX rate, including whether it was triangulated.
    pub fn rate(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] base: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] quote: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] date: JsValue,
        // The facade (exports/core.js) defaults an omitted policy.
        #[wasm_bindgen(unchecked_optional_param_type = "FxConversionPolicy")]
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
