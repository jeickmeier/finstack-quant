//! WASM bindings for the dated scalar series of
//! `finstack_quant_core::market_data::scalars`: `ScalarTimeSeries` and
//! `InflationIndex`.
//!
//! Dates are ISO-8601 strings (`"YYYY-MM-DD"`), as for the curves.

use crate::api::core::currency::JsCurrency;
use crate::utils::input::{from_js_json, js_opt_f64_seq, js_opt_string, js_string, js_uint};
use crate::utils::{date_to_iso, parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::scalars::{
    InflationIndex as RustInflationIndex, InflationInterpolation, InflationLag,
    ScalarTimeSeries as RustScalarTimeSeries, SeriesInterpolation,
};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Read `[isoDate, value]` observation pairs.
fn observations_arg(value: &JsValue) -> Result<Vec<(Date, f64)>, JsValue> {
    from_js_json::<Vec<(String, f64)>>(value, "observations")?
        .into_iter()
        .map(|(date, value)| Ok((parse_iso_date(&date)?, value)))
        .collect()
}

/// Observations as `[isoDate, value]` pairs.
fn observations_value(observations: Vec<(Date, f64)>) -> Result<JsValue, JsValue> {
    let pairs: Vec<(String, f64)> = observations
        .into_iter()
        .map(|(date, value)| (date_to_iso(date), value))
        .collect();
    to_js_value(&pairs)
}

/// Read an ISO-8601 date argument.
fn date_arg(value: &JsValue, label: &str) -> Result<Date, JsValue> {
    parse_iso_date(&js_string(value, label)?)
}

/// Dated scalar market series, such as an equity index level or a fixing history.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const series = new core.ScalarTimeSeries(
///   "SOFR-FIXINGS",
///   [
///     ["2025-01-02", 0.0431],
///     ["2025-01-03", 0.0433],
///   ],
/// );
/// series.valueOn("2025-01-03"); // 0.0433
/// series.lastDate; // "2025-01-03"
/// ```
#[wasm_bindgen(js_name = ScalarTimeSeries)]
#[derive(Clone)]
pub struct JsScalarTimeSeries {
    pub(crate) inner: RustScalarTimeSeries,
}

#[wasm_bindgen(js_class = ScalarTimeSeries)]
impl JsScalarTimeSeries {
    /// Construct a series from dated observations (Rust `ScalarTimeSeries::new`).
    ///
    /// # Arguments
    ///
    /// * `id` - Series identifier, the `MarketContext` lookup key.
    /// * `observations` - Array of `[isoDate, value]` pairs (or its JSON
    ///   text); dates must be unique and values finite.
    /// * `currency` - ISO-4217 code of the series' unit; omitted for a
    ///   unitless series such as a rate or an index level.
    /// * `interpolation` - How `valueOn` fills dates between observations:
    ///   `"step"` or `"linear"`; omitted uses the Rust default (`"step"`).
    ///
    /// @returns The validated `ScalarTimeSeries`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for a malformed date or pair, an
    /// empty or duplicated observation set, an unknown currency, or an unknown
    /// interpolation name.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: JsValue,
        observations: JsValue,
        currency: Option<JsValue>,
        interpolation: Option<JsValue>,
    ) -> Result<JsScalarTimeSeries, JsValue> {
        let id = js_string(&id, "id")?;
        let observations = observations_arg(&observations)?;
        let currency = js_opt_string(currency.as_ref(), "currency")?
            .map(|code| code.parse::<Currency>())
            .transpose()
            .map_err(to_js_err)?;
        let interpolation = js_opt_string(interpolation.as_ref(), "interpolation")?
            .map(|name| name.parse::<SeriesInterpolation>())
            .transpose()
            .map_err(to_js_err)?
            .unwrap_or_default();
        RustScalarTimeSeries::new(id, observations, currency)
            .map(|series| Self {
                inner: series.with_interpolation(interpolation),
            })
            .map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form shared with Python
    /// `ScalarTimeSeries.to_json`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical ScalarTimeSeries JSON text or plain object;
    ///   unknown fields are rejected and the series is re-validated.
    ///
    /// @returns The validated `ScalarTimeSeries`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema or
    /// fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsScalarTimeSeries, JsValue> {
        from_js_json::<RustScalarTimeSeries>(&json, "json").map(|inner| Self { inner })
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid series).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Value on a date under the series interpolation (Rust `ScalarTimeSeries::value_on`).
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 lookup date.
    ///
    /// @returns The observed or interpolated value.
    /// @throws `TypeError` if `date` is not a string; `FinstackError` for a
    /// malformed date or a date the series cannot serve (for example before
    /// the first observation).
    #[wasm_bindgen(js_name = valueOn)]
    pub fn value_on(&self, date: JsValue) -> Result<f64, JsValue> {
        self.inner
            .value_on(date_arg(&date, "date")?)
            .map_err(to_js_err)
    }

    /// Value observed exactly on a date, with no interpolation (Rust
    /// `ScalarTimeSeries::value_on_exact`).
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 observation date.
    ///
    /// @returns The value recorded on `date`.
    /// @throws `TypeError` if `date` is not a string; `FinstackError` for a
    /// malformed date or a date with no observation.
    #[wasm_bindgen(js_name = valueOnExact)]
    pub fn value_on_exact(&self, date: JsValue) -> Result<f64, JsValue> {
        self.inner
            .value_on_exact(date_arg(&date, "date")?)
            .map_err(to_js_err)
    }

    /// Series identifier (the `MarketContext` lookup key).
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Currency of the series values, or `undefined` for a unitless series.
    #[wasm_bindgen(getter, js_name = currency)]
    pub fn currency(&self) -> Option<JsCurrency> {
        self.inner.currency().map(|inner| JsCurrency { inner })
    }

    /// Interpolation between observations: `"step"` or `"linear"`.
    #[wasm_bindgen(getter, js_name = interpolation)]
    pub fn interpolation(&self) -> String {
        self.inner.interpolation().to_string()
    }

    /// Observations in date order as `[isoDate, value]` pairs.
    #[wasm_bindgen(getter, js_name = observations)]
    pub fn observations(&self) -> Result<JsValue, JsValue> {
        observations_value(self.inner.observations())
    }

    /// Date of the first observation as an ISO-8601 string, or `undefined` when empty.
    #[wasm_bindgen(getter, js_name = firstDate)]
    pub fn first_date(&self) -> Option<String> {
        self.inner.first_date().map(date_to_iso)
    }

    /// Date of the last observation as an ISO-8601 string, or `undefined` when empty.
    #[wasm_bindgen(getter, js_name = lastDate)]
    pub fn last_date(&self) -> Option<String> {
        self.inner.last_date().map(date_to_iso)
    }
}

/// Historical consumer-price index with the publication lag, interpolation
/// and seasonality used to index inflation-linked cashflows.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const index = new core.InflationIndex(
///   "US-CPI-U",
///   [
///     ["2024-10-01", 315.664],
///     ["2024-11-01", 315.493],
///     ["2024-12-01", 315.605],
///   ],
///   "USD",
///   "linear",
///   "3M",
/// );
/// index.lag; // "3M"
/// index.dateRange(); // ["2024-10-01", "2024-12-01"]
/// ```
#[wasm_bindgen(js_name = InflationIndex)]
#[derive(Clone)]
pub struct JsInflationIndex {
    pub(crate) inner: Arc<RustInflationIndex>,
}

impl JsInflationIndex {
    pub(crate) fn from_inner(inner: Arc<RustInflationIndex>) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = InflationIndex)]
impl JsInflationIndex {
    /// Construct an index from dated CPI prints (Rust `InflationIndex::new`).
    ///
    /// # Arguments
    ///
    /// * `id` - Index identifier, the `MarketContext` lookup key.
    /// * `observations` - Array of `[isoDate, level]` pairs (or its JSON
    ///   text): unique dates and strictly positive index levels.
    /// * `currency` - ISO-4217 code of the index's currency.
    /// * `interpolation` - How the level is read between prints: `"step"` or
    ///   `"linear"`; omitted uses the Rust default (`"step"`).
    /// * `lag` - Observation lag as text: `"3M"`, `"90D"` or `"none"`;
    ///   omitted keeps the Rust default (`"none"`).
    /// * `seasonality` - Twelve multiplicative monthly factors, January
    ///   first; omitted applies no seasonality.
    ///
    /// @returns The validated `InflationIndex`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for a malformed date or pair, an
    /// empty or duplicated observation set, an unknown currency,
    /// interpolation or lag, or a `seasonality` array that does not hold
    /// twelve valid factors.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: JsValue,
        observations: JsValue,
        currency: JsValue,
        interpolation: Option<JsValue>,
        lag: Option<JsValue>,
        seasonality: Option<JsValue>,
    ) -> Result<JsInflationIndex, JsValue> {
        let id = js_string(&id, "id")?;
        let observations = observations_arg(&observations)?;
        let currency: Currency = js_string(&currency, "currency")?
            .parse()
            .map_err(to_js_err)?;
        let interpolation = js_opt_string(interpolation.as_ref(), "interpolation")?
            .map(|name| name.parse::<InflationInterpolation>())
            .transpose()
            .map_err(to_js_err)?
            .unwrap_or_default();
        let lag = js_opt_string(lag.as_ref(), "lag")?
            .map(|label| label.parse::<InflationLag>())
            .transpose()
            .map_err(to_js_err)?;
        let seasonality = js_opt_f64_seq(seasonality.as_ref(), "seasonality")?
            .map(|factors| {
                <[f64; 12]>::try_from(factors).map_err(|got| {
                    to_js_err(format!(
                        "seasonality must have exactly 12 monthly factors, got {}",
                        got.len()
                    ))
                })
            })
            .transpose()?;
        let mut inner = RustInflationIndex::new(id, observations, currency)
            .map_err(to_js_err)?
            .with_interpolation(interpolation);
        if let Some(lag) = lag {
            inner = inner.with_lag(lag);
        }
        if let Some(factors) = seasonality {
            inner = inner.with_seasonality(factors).map_err(to_js_err)?;
        }
        Ok(Self::from_inner(Arc::new(inner)))
    }

    /// Deserialize from the canonical JSON wire form shared with Python
    /// `InflationIndex.to_json`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical InflationIndex JSON text or plain object; unknown
    ///   fields are rejected and the index is re-validated.
    ///
    /// @returns The validated `InflationIndex`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema or
    /// fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsInflationIndex, JsValue> {
        from_js_json::<RustInflationIndex>(&json, "json")
            .map(|index| Self::from_inner(Arc::new(index)))
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid index).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Index level on a date, applying the index lag, interpolation and
    /// seasonality (Rust `InflationIndex::value_on`).
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 lookup date.
    ///
    /// @returns The index level applicable on `date`.
    /// @throws `TypeError` if `date` is not a string; `FinstackError` for a
    /// malformed date or a date the index history cannot serve.
    #[wasm_bindgen(js_name = valueOn)]
    pub fn value_on(&self, date: JsValue) -> Result<f64, JsValue> {
        self.inner
            .value_on(date_arg(&date, "date")?)
            .map_err(to_js_err)
    }

    /// Index ratio `I(settleDate) / I(baseDate)` (Rust `InflationIndex::ratio`).
    ///
    /// # Arguments
    ///
    /// * `base_date` - ISO-8601 date of the base (issue) index level.
    /// * `settle_date` - ISO-8601 date of the settlement index level.
    ///
    /// @returns The uplift factor applied to an inflation-linked notional.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` for a
    /// malformed date or a date the index history cannot serve.
    #[wasm_bindgen(js_name = ratio)]
    pub fn ratio(&self, base_date: JsValue, settle_date: JsValue) -> Result<f64, JsValue> {
        self.inner
            .ratio(
                date_arg(&base_date, "baseDate")?,
                date_arg(&settle_date, "settleDate")?,
            )
            .map_err(to_js_err)
    }

    /// Reference CPI on a date under a months-lag convention, interpolated
    /// linearly by day of month between the two lagged monthly prints (Rust
    /// `InflationIndex::ref_cpi_months_lag`).
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 settlement date.
    /// * `lag_months` - Indexation lag in whole months (for example `3` for
    ///   US TIPS and UK index-linked gilts issued since 2005).
    ///
    /// @returns The reference index level for `date`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` for a
    /// malformed date or when a required monthly print is missing.
    #[wasm_bindgen(js_name = refCpiMonthsLag)]
    pub fn ref_cpi_months_lag(&self, date: JsValue, lag_months: JsValue) -> Result<f64, JsValue> {
        self.inner
            .ref_cpi_months_lag(date_arg(&date, "date")?, js_uint(&lag_months, "lagMonths")?)
            .map_err(to_js_err)
    }

    /// First and last observation dates (Rust `InflationIndex::date_range`).
    ///
    /// @returns A two-element array `[firstIsoDate, lastIsoDate]`.
    /// @throws `FinstackError` if the index holds no observations.
    #[wasm_bindgen(js_name = dateRange)]
    pub fn date_range(&self) -> Result<Vec<String>, JsValue> {
        let (first, last) = self.inner.date_range().map_err(to_js_err)?;
        Ok(vec![date_to_iso(first), date_to_iso(last)])
    }

    /// Index identifier (the `MarketContext` lookup key).
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Currency of the index.
    #[wasm_bindgen(getter, js_name = currency)]
    pub fn currency(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.currency,
        }
    }

    /// Interpolation between prints: `"step"` or `"linear"`.
    #[wasm_bindgen(getter, js_name = interpolation)]
    pub fn interpolation(&self) -> String {
        self.inner.interpolation().to_string()
    }

    /// Observation lag as text, such as `"3M"`, `"90D"` or `"none"`.
    #[wasm_bindgen(getter, js_name = lag)]
    pub fn lag(&self) -> String {
        self.inner.lag().to_string()
    }

    /// Twelve monthly seasonality factors (January first), or `undefined` when none are set.
    #[wasm_bindgen(getter, js_name = seasonality)]
    pub fn seasonality(&self) -> Option<Box<[f64]>> {
        self.inner
            .seasonality()
            .map(|factors| factors.to_vec().into_boxed_slice())
    }

    /// Observations in date order as `[isoDate, level]` pairs.
    #[wasm_bindgen(getter, js_name = observations)]
    pub fn observations(&self) -> Result<JsValue, JsValue> {
        observations_value(self.inner.observations())
    }
}
