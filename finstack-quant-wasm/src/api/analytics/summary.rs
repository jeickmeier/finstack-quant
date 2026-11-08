//! Analytics semantics for restored generic table envelopes.

use crate::utils::input::from_js_json;
use crate::utils::to_js_err;
use finstack_quant_core::table::TableEnvelope;
use wasm_bindgen::prelude::*;

/// Validate that a table satisfies the canonical fixed analytics summary contract.
///
/// Structural table validity does not establish the summary's financial metadata.
/// This checks identities, units, required policy inputs, field-specific result
/// availability and sample provenance; it does not recalculate the underlying data.
/// @param table - Native TableEnvelope object preserving numeric NaN/infinity, or canonical JSON text using the established nan/inf/-inf numeric-result sentinels. Unknown fields, inappropriate nulls and inconsistent metadata are rejected.
/// @throws Error - Rejects malformed generic envelopes or an invalid analytics summary shape, column order, required inputs, sample provenance or result domain.
/// @example
/// ```typescript
/// import init, { analytics } from "finstack-quant-wasm";
/// await init();
/// const p = new analytics.Performance({ dates: ["2025-01-01", "2025-01-02"], series: [[0]], kind: "returns", ticker_names: ["A"], benchmark_ticker: "A", frequency: "daily" });
/// analytics.validateSummary(p.summary(0, 0, 0, 0.95));
/// p.free();
/// ```
#[wasm_bindgen(js_name = validateSummary)]
pub fn validate_summary(
    #[wasm_bindgen(unchecked_param_type = "TableEnvelope | string")] table: JsValue,
) -> Result<(), JsValue> {
    let table: TableEnvelope = if table.is_string() {
        from_js_json(&table, "table")?
    } else {
        // Native table results can contain nonfinite JavaScript numbers. The
        // strict canonical JSON walker intentionally rejects those values,
        // so deserialize the native typed carrier directly, retaining its
        // serde unknown-field/domain checks and validating analytics below.
        serde_wasm_bindgen::from_value(table).map_err(to_js_err)?
    };
    finstack_quant_analytics::validate_summary(&table).map_err(to_js_err)
}
