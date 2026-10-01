//! Market convention registry.
//!
//! [`JsConventionRegistry`] reads the process-global Rust
//! `ConventionRegistry` (embedded convention data). Each lookup returns the
//! convention record as a plain object typed by the schema-generated
//! TypeScript types.

use crate::utils::input::js_string;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::market::conventions::ids::{
    CdsConventionKey, InflationSwapConventionId, IrFutureContractId, SwaptionConventionId,
    XccyConventionId,
};
use finstack_quant_valuations::market::conventions::ConventionRegistry;
use wasm_bindgen::prelude::*;

fn registry() -> Result<&'static ConventionRegistry, JsValue> {
    ConventionRegistry::try_global().map_err(to_js_err)
}

/// Handle on the process-global market convention registry.
///
/// The registry holds the embedded rate-index, CDS, swaption, inflation-swap,
/// interest-rate future and cross-currency conventions that the instrument
/// constructors resolve by identifier.
#[wasm_bindgen(js_name = ConventionRegistry)]
pub struct JsConventionRegistry;

#[wasm_bindgen(js_class = ConventionRegistry)]
impl JsConventionRegistry {
    /// Open the process-global convention registry.
    /// @returns A handle on the registry; every handle reads the same embedded data.
    /// @throws Error - Throws with kind `validation` if the embedded convention data fails to load (does not occur for a released build).
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<JsConventionRegistry, JsValue> {
        registry()?;
        Ok(Self)
    }

    /// Conventions of a rate index (mirrors Rust `ConventionRegistry::require_rate_index`).
    /// @param id - Rate index identifier, e.g. `"USD-SOFR-OIS"`.
    /// @returns `RateIndexConventions` plain object: currency, kind, tenor, day count, payment and reset lags, calendar and fixed-leg defaults.
    /// @throws Error - Throws with kind `not_found` if `id` has no registered conventions, and kind `invalid_type` if it is not a string.
    #[wasm_bindgen(js_name = requireRateIndex)]
    pub fn require_rate_index(&self, id: JsValue) -> Result<JsValue, JsValue> {
        let id = finstack_quant_core::types::IndexId::new(js_string(&id, "id")?);
        to_js_value(registry()?.require_rate_index(&id).map_err(to_js_err)?)
    }

    /// CDS conventions for a currency and documentation clause (mirrors Rust `ConventionRegistry::resolve_cds`).
    /// @param currency - ISO-4217 currency code of the contract, e.g. `"USD"`.
    /// @param doc_clause - Documentation clause: `cr14`, `mr14`, `mm14`, `xr14`, `isda_na`, `isda_eu`, `isda_as`, `isda_au`, or `isda_nz`.
    /// @returns `CdsConventionSpec` plain object: convention family, calendar, day count, business-day convention, stub rule, settlement lag and frequency.
    /// @throws Error - Throws with kind `validation` if `currency` or `docClause` is not recognized, kind `not_found` if the pair has no registered conventions, and kind `invalid_type` for a non-string argument.
    #[wasm_bindgen(js_name = resolveCds)]
    pub fn resolve_cds(&self, currency: JsValue, doc_clause: JsValue) -> Result<JsValue, JsValue> {
        let key = CdsConventionKey {
            currency: super::typed::arg::en(&currency, "currency")?,
            doc_clause: super::typed::arg::en(&doc_clause, "docClause")?,
        };
        to_js_value(registry()?.resolve_cds(&key).map_err(to_js_err)?)
    }

    /// Primary CDS convention family of a currency (mirrors Rust `ConventionRegistry::primary_cds_family`).
    /// @param currency - ISO-4217 currency code, e.g. `"EUR"`.
    /// @returns The convention family serde name (for example `"isda_eu"`), or `null` when the currency has none.
    /// @throws Error - Throws with kind `validation` if `currency` is not an ISO-4217 code, and kind `invalid_type` if it is not a string.
    #[wasm_bindgen(js_name = primaryCdsFamily)]
    pub fn primary_cds_family(&self, currency: JsValue) -> Result<JsValue, JsValue> {
        let currency = super::typed::arg::en(&currency, "currency")?;
        to_js_value(&registry()?.primary_cds_family(currency))
    }

    /// Conventions of a swaption market (mirrors Rust `ConventionRegistry::require_swaption`).
    /// @param id - Swaption convention identifier, e.g. `"USD"`.
    /// @returns `SwaptionConventions` plain object: calendar, settlement lag, business-day convention and the underlying swap's fixed-leg and floating-index conventions.
    /// @throws Error - Throws with kind `not_found` if `id` has no registered conventions, and kind `invalid_type` if it is not a string.
    #[wasm_bindgen(js_name = requireSwaption)]
    pub fn require_swaption(&self, id: JsValue) -> Result<JsValue, JsValue> {
        let id = SwaptionConventionId::new(js_string(&id, "id")?);
        to_js_value(registry()?.require_swaption(&id).map_err(to_js_err)?)
    }

    /// Conventions of an inflation-swap market (mirrors Rust `ConventionRegistry::require_inflation_swap`).
    /// @param id - Inflation-swap convention identifier, e.g. `"USD-CPI"`.
    /// @returns `InflationSwapConventions` plain object: calendar, settlement lag, business-day convention, day count, inflation lag and interpolation.
    /// @throws Error - Throws with kind `not_found` if `id` has no registered conventions, and kind `invalid_type` if it is not a string.
    #[wasm_bindgen(js_name = requireInflationSwap)]
    pub fn require_inflation_swap(&self, id: JsValue) -> Result<JsValue, JsValue> {
        let id = InflationSwapConventionId::new(js_string(&id, "id")?);
        to_js_value(registry()?.require_inflation_swap(&id).map_err(to_js_err)?)
    }

    /// Conventions of an interest-rate future contract (mirrors Rust `ConventionRegistry::require_ir_future`).
    /// @param id - Contract identifier, e.g. `"CME:SR3"`.
    /// @returns `IrFutureConventions` plain object: index, compounding, reference period, calendar, face value, tick size and tick value.
    /// @throws Error - Throws with kind `not_found` if `id` has no registered conventions, and kind `invalid_type` if it is not a string.
    #[wasm_bindgen(js_name = requireIrFuture)]
    pub fn require_ir_future(&self, id: JsValue) -> Result<JsValue, JsValue> {
        let id = IrFutureContractId::new(js_string(&id, "id")?);
        to_js_value(registry()?.require_ir_future(&id).map_err(to_js_err)?)
    }

    /// Conventions of a cross-currency swap pair (mirrors Rust `ConventionRegistry::require_xccy`).
    /// @param id - Cross-currency convention identifier, e.g. `"EUR/USD-XCCY"`.
    /// @returns `XccyConventions` plain object: currencies, index identifiers, settlement lag, payment frequency, day count, calendars and notional exchange.
    /// @throws Error - Throws with kind `not_found` if `id` has no registered conventions, and kind `invalid_type` if it is not a string.
    #[wasm_bindgen(js_name = requireXccy)]
    pub fn require_xccy(&self, id: JsValue) -> Result<JsValue, JsValue> {
        let id = XccyConventionId::new(js_string(&id, "id")?);
        to_js_value(registry()?.require_xccy(&id).map_err(to_js_err)?)
    }
}
