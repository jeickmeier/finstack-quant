//! WASM bindings for the dated scalar series of
//! `finstack_quant_core::market_data::scalars`: `ScalarTimeSeries` and
//! `InflationIndex`.
//!
//! Dates are ISO-8601 strings (`"YYYY-MM-DD"`), as for the curves.

use crate::api::core::currency::JsCurrency;
use crate::utils::input::{from_js_json, js_currency, js_int, js_opt_currency, js_string, js_uint};
use crate::utils::wire::js_date;
use crate::utils::{date_to_iso, parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::scalars::{
    InflationExpectedRelease, InflationIndex as RustInflationIndex, InflationObservation,
    ScalarTimeSeries as RustScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
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
///   "step",
///   "flat",
/// );
/// series.getValue("2025-01-03"); // 0.0433
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
    ///   text); dates must be strictly ascending and unique (empty accepted),
    ///   and values must be finite.
    /// * `interpolation` - Required in-range fill policy: `"step"` or `"linear"`.
    /// * `extrapolation` - Required right-tail policy after the last observation:
    ///   `"flat"` or `"error"`.
    /// * `currency` - ISO-4217 code of the series' unit; omitted for a
    ///   unitless series such as a rate or an index level.
    ///
    /// @returns The validated `ScalarTimeSeries`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for a malformed date or pair, a
    /// non-monotonic or duplicate observation set, an unknown currency, or an
    /// unknown interpolation/extrapolation name.
    /// @param id - Series identifier, the `MarketContext` lookup key.
    /// @param observations - Array of `[isoDate, value]` pairs (or its JSON text); dates must be strictly ascending and unique (empty accepted), and values must be finite.
    /// @param interpolation - Required in-range fill policy between observations: `"step"` or `"linear"`.
    /// @param extrapolation - Required right-tail policy after the last observation: `"flat"` or `"error"`.
    /// @param currency - ISO-4217 code of the series' unit; omitted for a unitless series such as a rate or an index level.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "readonly (readonly [string, number])[] | string")]
        observations: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] interpolation: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] extrapolation: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "string | null")] currency: Option<JsValue>,
    ) -> Result<JsScalarTimeSeries, JsValue> {
        let id = js_string(&id, "id")?;
        let observations = observations_arg(&observations)?;
        let interpolation = js_string(&interpolation, "interpolation")?
            .parse::<SeriesInterpolation>()
            .map_err(to_js_err)?;
        let extrapolation = js_string(&extrapolation, "extrapolation")?
            .parse::<SeriesExtrapolation>()
            .map_err(to_js_err)?;
        let currency = js_opt_currency(currency.as_ref(), "currency")?;
        RustScalarTimeSeries::new(id, observations, currency, interpolation, extrapolation)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Value on a date under the series interpolation and right-tail
    /// extrapolation policies (Rust `ScalarTimeSeries::get_value`).
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 lookup date.
    ///
    /// @returns The observed or interpolated value.
    /// @throws `TypeError` if `date` is not a string; `FinstackError` for a
    /// malformed date or a date the series cannot serve (for example empty
    /// series, before the first observation, or after the last observation
    /// under `"error"` extrapolation).
    /// @param date - ISO-8601 lookup date.
    #[wasm_bindgen(js_name = getValue)]
    pub fn get_value(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] date: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .get_value(js_date(&date, "date")?)
            .map_err(to_js_err)
    }

    /// Value observed exactly on a date, with no interpolation (Rust
    /// `ScalarTimeSeries::get_value_exact`).
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 observation date.
    ///
    /// @returns The value recorded on `date`.
    /// @throws `TypeError` if `date` is not a string; `FinstackError` for a
    /// malformed date or a date with no observation.
    /// @param date - ISO-8601 observation date.
    #[wasm_bindgen(js_name = getValueExact)]
    pub fn get_value_exact(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] date: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .get_value_exact(js_date(&date, "date")?)
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

    /// Right-tail extrapolation policy after the latest observation: `"flat"` or `"error"`.
    #[wasm_bindgen(getter, js_name = extrapolation)]
    pub fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    /// Observations in date order as `[isoDate, value]` pairs.
    #[wasm_bindgen(getter, js_name = observations, unchecked_return_type = "[string, number][]")]
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

json_round_trip!(JsScalarTimeSeries, ScalarTimeSeries);

/// Immutable monthly CPI fixing snapshot at an explicit knowledge cutoff.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const index = new core.InflationIndex(
///   "US-CPI-U",
///   [
///     { reference_month: "2024-10-01", value: 315.664, available_on: "2024-11-13" },
///     { reference_month: "2024-11-01", value: 315.493, available_on: "2024-12-11" },
///   ],
///   "USD",
///   "2025-01-02",
/// );
/// index.getObservation("2024-10-01").value; // 315.664
/// index.dateRange(); // ["2024-10-01", "2024-11-01"]
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
    /// Construct an index from monthly CPI fixing records.
    ///
    /// # Arguments
    ///
    /// * `id` - Index identifier, the `MarketContext` lookup key.
    /// * `observations` - Canonical first-of-month records, strictly ascending and
    ///   unique, with finite positive levels and availability on or before the cutoff.
    /// * `currency` - ISO-4217 code of the index's currency.
    /// * `knowledge_date` - Inclusive ISO-8601 snapshot cutoff date.
    ///
    /// @returns The validated `InflationIndex`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for malformed dates, invalid values,
    /// unsorted or duplicate months, availability outside the reference-month/cutoff
    /// interval, or an unknown currency. Empty histories are valid.
    /// @param id - Index identifier, the `MarketContext` lookup key.
    /// @param observations - Ordered monthly records (or JSON text), with snake_case fields and ISO-8601 dates; no lag or interpolation is applied.
    /// @param currency - ISO-4217 code of the index's currency.
    /// @param knowledgeDate - Inclusive ISO-8601 snapshot cutoff date.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "readonly InflationObservation[] | string")]
        observations: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
        #[wasm_bindgen(js_name = "knowledgeDate", unchecked_param_type = "string")] knowledge_date: JsValue,
    ) -> Result<JsInflationIndex, JsValue> {
        let id = js_string(&id, "id")?;
        let observations =
            from_js_json::<Vec<InflationObservation>>(&observations, "observations")?;
        let currency = js_currency(&currency, "currency")?;
        let inner = RustInflationIndex::new(
            id,
            observations,
            currency,
            js_date(&knowledge_date, "knowledgeDate")?,
        )
        .map_err(to_js_err)?;
        Ok(Self::from_inner(Arc::new(inner)))
    }

    /// Look up the exact stored observation for a canonical reference month.
    ///
    /// # Arguments
    ///
    /// * `reference_month` - ISO-8601 first calendar day of the reference month.
    ///
    /// @returns The stored record, with snake_case fields and ISO dates.
    /// @throws `FinstackError` for an invalid month key or a missing observation;
    /// `TypeError` for a non-string argument.
    /// @param referenceMonth - ISO-8601 first calendar day; no carry or interpolation.
    #[wasm_bindgen(js_name = getObservation, unchecked_return_type = "InflationObservation")]
    pub fn get_observation(
        &self,
        #[wasm_bindgen(js_name = "referenceMonth", unchecked_param_type = "string")]
        reference_month: JsValue,
    ) -> Result<JsValue, JsValue> {
        to_js_value(
            &self
                .inner
                .get_observation(js_date(&reference_month, "referenceMonth")?)
                .map_err(to_js_err)?,
        )
    }

    /// Resolve a monthly fixing eligible at an explicit query knowledge date.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar year of the reference month.
    /// * `month` - Calendar month number, 1 through 12.
    /// * `knowledge_date` - Inclusive ISO cutoff, no later than the snapshot cutoff.
    ///
    /// @returns The published positive CPI level.
    /// @throws `TypeError` for mistyped arguments; `FinstackError` for an invalid
    /// date, absent month, unavailable observation or cutoff after the snapshot.
    /// @param year - Calendar year of the reference month.
    /// @param month - Calendar month number, 1 through 12.
    /// @param knowledgeDate - Inclusive ISO cutoff, no later than the snapshot cutoff.
    #[wasm_bindgen(js_name = qualifiedMonthlyObservation)]
    pub fn qualified_monthly_observation(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] month: JsValue,
        #[wasm_bindgen(js_name = "knowledgeDate", unchecked_param_type = "string")] knowledge_date: JsValue,
    ) -> Result<f64, JsValue> {
        let month = time::Month::try_from(js_uint::<u8>(&month, "month")?)
            .map_err(|err| to_js_err(finstack_quant_core::Error::Validation(err.to_string())))?;
        self.inner
            .qualified_monthly_observation(
                js_int(&year, "year")?,
                month,
                js_date(&knowledge_date, "knowledgeDate")?,
            )
            .map_err(to_js_err)
    }

    /// First and last observation dates (Rust `InflationIndex::date_range`).
    ///
    /// @returns `[firstIsoDate, lastIsoDate]`, or `null` for an empty snapshot.
    /// @throws `FinstackError` if host serialization fails.
    #[wasm_bindgen(js_name = dateRange, unchecked_return_type = "[string, string] | null")]
    pub fn date_range(&self) -> Result<JsValue, JsValue> {
        to_js_value(
            &self
                .inner
                .date_range()
                .map(|(first, last)| (date_to_iso(first), date_to_iso(last))),
        )
    }

    /// Index identifier (the `MarketContext` lookup key).
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().to_owned()
    }

    /// Currency of the index.
    #[wasm_bindgen(getter, js_name = currency)]
    pub fn currency(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.currency(),
        }
    }

    /// Inclusive snapshot knowledge cutoff as an ISO-8601 date.
    #[wasm_bindgen(getter, js_name = knowledgeDate)]
    pub fn knowledge_date(&self) -> String {
        date_to_iso(self.inner.knowledge_date())
    }

    /// Stored monthly records in ascending reference-month order.
    /// @returns Records with snake_case fields and ISO dates; empty when no history exists.
    /// @throws `FinstackError` if host serialization fails.
    #[wasm_bindgen(getter, js_name = observations, unchecked_return_type = "InflationObservation[]")]
    pub fn observations(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.observations())
    }

    /// Copy the snapshot with a replacement expected-release schedule.
    ///
    /// # Arguments
    ///
    /// * `expected_releases` - Strictly ascending first-of-month records with
    ///   `expected_release >= reference_month` and `known_on <= knowledge_date`.
    ///
    /// @returns A new snapshot retaining observations and knowledge cutoff.
    /// @throws `FinstackError` for invalid records, dates or ordering; `TypeError`
    /// for unsupported argument types.
    /// @param expectedReleases - Ordered monthly schedule facts (or JSON text), snake_case fields and ISO dates. These do not assert observation availability.
    #[wasm_bindgen(js_name = withExpectedReleases)]
    pub fn with_expected_releases(
        &self,
        #[wasm_bindgen(
            js_name = "expectedReleases",
            unchecked_param_type = "readonly InflationExpectedRelease[] | string"
        )]
        expected_releases: JsValue,
    ) -> Result<JsInflationIndex, JsValue> {
        let expected_releases =
            from_js_json::<Vec<InflationExpectedRelease>>(&expected_releases, "expectedReleases")?;
        self.inner
            .as_ref()
            .clone()
            .with_expected_releases(expected_releases)
            .map(|index| Self::from_inner(Arc::new(index)))
            .map_err(to_js_err)
    }

    /// Look up the expected-release assumption for an exact reference month.
    ///
    /// # Arguments
    ///
    /// * `reference_month` - ISO first calendar day of the reference month.
    ///
    /// @returns The stored schedule fact, or `null` when unspecified.
    /// @throws `TypeError` for non-string inputs; `FinstackError` for invalid month keys.
    /// @param referenceMonth - ISO first calendar day; other days are rejected.
    #[wasm_bindgen(js_name = getExpectedRelease, unchecked_return_type = "InflationExpectedRelease | null")]
    pub fn get_expected_release(
        &self,
        #[wasm_bindgen(js_name = "referenceMonth", unchecked_param_type = "string")]
        reference_month: JsValue,
    ) -> Result<JsValue, JsValue> {
        to_js_value(
            &self
                .inner
                .get_expected_release(js_date(&reference_month, "referenceMonth")?)
                .map_err(to_js_err)?,
        )
    }

    /// Configured expected-release assumptions in reference-month order.
    ///
    /// @returns Ordered records with ISO dates, empty when no schedule is set.
    /// @throws `FinstackError` if host serialization fails.
    #[wasm_bindgen(js_name = getExpectedReleases, unchecked_return_type = "InflationExpectedRelease[]")]
    pub fn get_expected_releases(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.get_expected_releases())
    }
}

json_round_trip!(JsInflationIndex, InflationIndex, arc);
