//! Browser-safe WASM wrappers for strict portfolio materialization.

use crate::utils::input::js_opt_uint;
use std::sync::Arc;

use finstack_quant_core::contract::LoadLimits;
use finstack_quant_portfolio::{InstrumentArtifactCache, Portfolio as RustPortfolio};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::JsPortfolio;
use crate::utils::{materialization_to_js_error, to_js_value};

/// Reusable bounded cache for decoded content-addressed instrument artifacts.
///
/// @example
/// ```typescript
/// const cache = new portfolio.InstrumentArtifactCache(5000);
/// const first = portfolio.Portfolio.fromMaterialization(bundle, cache);
/// const second = portfolio.Portfolio.fromMaterialization(bundle, cache);
/// cache.free();
/// ```
#[wasm_bindgen(js_name = InstrumentArtifactCache)]
pub struct JsInstrumentArtifactCache {
    inner: Arc<InstrumentArtifactCache>,
}

#[wasm_bindgen(js_class = InstrumentArtifactCache)]
impl JsInstrumentArtifactCache {
    /// Create an empty cache with an explicit entry capacity.
    ///
    /// @param capacity - Maximum retained artifacts. Omit, `null`, or
    /// `undefined` to use the native default of 4,096.
    /// @returns A reusable cache with a 64 MiB encoded-source byte bound.
    #[wasm_bindgen(constructor)]
    pub fn new(capacity: Option<JsValue>) -> Result<JsInstrumentArtifactCache, JsValue> {
        let capacity: Option<usize> = js_opt_uint(capacity.as_ref(), "capacity")?;
        Ok(Self {
            inner: Arc::new(
                capacity
                    .map(InstrumentArtifactCache::with_capacity)
                    .unwrap_or_default(),
            ),
        })
    }

    /// Number of decoded artifacts currently retained.
    ///
    /// @returns A non-negative entry count.
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        self.inner.len()
    }

    /// Cumulative number of successful cache-miss decodes.
    ///
    /// @returns A non-negative decode count for this cache instance.
    #[wasm_bindgen(getter, js_name = decodeCount)]
    pub fn decode_count(&self) -> usize {
        self.inner.decode_count()
    }
}

#[wasm_bindgen(js_class = Portfolio)]
impl JsPortfolio {
    /// Build a reusable portfolio from one strict materialization bundle.
    ///
    /// Accepts browser-native strings and `Uint8Array` values. The returned
    /// object contains `{ portfolio, report }`, where `portfolio` is a reusable
    /// WASM handle and `report` is plain structured JavaScript data.
    ///
    /// @param bundle - Complete UTF-8 materialization JSON string or `Uint8Array`.
    /// @param cache - Optional reusable decoded-artifact cache created outside
    /// any timed validation region. Omit, `null`, or `undefined` for a per-call
    /// cache with the native default bounds (the Python `cache=None` twin).
    /// @returns An object containing the reusable portfolio and load report.
    /// @throws Error - Throws `TypeError` for unsupported input types. Contract
    /// failures throw `ContractValidationError` (`kind` `validation`) with a
    /// `code` of `report` plus a structured `report` property if the persisted
    /// contract is malformed, invalid, or unsupported, or a `code` of
    /// `limit_exceeded` if it exceeds a resource limit.
    #[wasm_bindgen(js_name = fromMaterialization)]
    pub fn from_materialization(
        bundle: JsValue,
        cache: &JsInstrumentArtifactCache,
    ) -> Result<JsValue, JsValue> {
        let bytes = extract_bundle_bytes(bundle)?;
        let (portfolio, report) =
            RustPortfolio::from_materialization(&bytes, &cache.inner, &LoadLimits::default())
                .map_err(materialization_to_js_error)?;

        let result = js_sys::Object::new();
        let portfolio = JsPortfolio {
            inner: Arc::new(portfolio),
        };
        js_sys::Reflect::set(
            &result,
            &JsValue::from("portfolio"),
            &JsValue::from(portfolio),
        )?;
        js_sys::Reflect::set(&result, &JsValue::from("report"), &to_js_value(&report)?)?;
        Ok(result.into())
    }

    /// Validate a strict materialization bundle for ingestion-form feedback.
    ///
    /// Contract diagnostics are returned as a plain `ValidationReport` rather
    /// than thrown. Unsupported input types, resource-limit failures, and
    /// non-contract native failures still throw.
    ///
    /// @param bundle - Complete UTF-8 materialization JSON string or `Uint8Array`.
    /// @param cache - Optional reusable decoded-artifact cache used while
    /// validating. Omit, `null`, or `undefined` for a per-call cache with the
    /// native default bounds (the Python `cache=None` twin).
    /// @returns A materialization report whose build/index phase counters are zero,
    /// or a `ValidationReport` when the contract is invalid but still reportable.
    /// @throws Error - Throws `TypeError` for unsupported input types or a
    /// structured `ContractValidationError` when validation cannot produce a report.
    #[wasm_bindgen(js_name = validateMaterialization)]
    pub fn validate_materialization(
        bundle: JsValue,
        cache: &JsInstrumentArtifactCache,
    ) -> Result<JsValue, JsValue> {
        let bytes = extract_bundle_bytes(bundle)?;
        match RustPortfolio::validate_materialization(&bytes, &cache.inner, &LoadLimits::default())
        {
            Ok(report) => to_js_value(&report),
            Err(finstack_quant_portfolio::Error::MaterializationFailed(report)) => {
                to_js_value(&report)
            }
            Err(error) => Err(materialization_to_js_error(error)),
        }
    }
}

fn extract_bundle_bytes(bundle: JsValue) -> Result<Vec<u8>, JsValue> {
    if let Some(text) = bundle.as_string() {
        return Ok(text.into_bytes());
    }
    if bundle.is_instance_of::<js_sys::Uint8Array>() {
        return Ok(js_sys::Uint8Array::new(&bundle).to_vec());
    }
    Err(crate::utils::input::invalid_type(
        "bundle",
        "expected a string or Uint8Array",
    ))
}
