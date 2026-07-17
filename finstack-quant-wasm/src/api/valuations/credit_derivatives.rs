//! WASM bindings for CDS-family instrument example payloads.
//!
//! Mirrors `finstack-quant-py/src/bindings/valuations/credit_derivatives.rs`. This
//! file exists so the Rust source tree matches the Python wrapper layout.
//!
//! Pricing / validation / serialization for CDS instruments is provided by the
//! generic `priceInstrument`, `priceInstrumentWithMetrics`, and
//! `validateInstrumentJson` entry points exposed from the JS facade at
//! `valuations.instruments`; this module only owns the example-payload
//! factories.

use crate::utils::to_js_err;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::CDSIndex;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranche;
use finstack_quant_valuations::instruments::InstrumentJson;
use wasm_bindgen::prelude::*;

/// Example tagged `CreditDefaultSwap` instrument JSON.
#[wasm_bindgen(js_name = creditDefaultSwapExampleJson)]
pub fn credit_default_swap_example_json() -> Result<String, JsValue> {
    serde_json::to_string(&InstrumentJson::CreditDefaultSwap(
        CreditDefaultSwap::example(),
    ))
    .map_err(to_js_err)
}

/// Example tagged `CDSIndex` instrument JSON.
#[wasm_bindgen(js_name = cdsIndexExampleJson)]
pub fn cds_index_example_json() -> Result<String, JsValue> {
    serde_json::to_string(&InstrumentJson::CDSIndex(CDSIndex::example())).map_err(to_js_err)
}

/// Example tagged `CDSTranche` instrument JSON.
#[wasm_bindgen(js_name = cdsTrancheExampleJson)]
pub fn cds_tranche_example_json() -> Result<String, JsValue> {
    serde_json::to_string(&InstrumentJson::CDSTranche(CDSTranche::example())).map_err(to_js_err)
}

/// Example tagged `CDSOption` instrument JSON.
#[wasm_bindgen(js_name = cdsOptionExampleJson)]
pub fn cds_option_example_json() -> Result<String, JsValue> {
    let option = CDSOption::example().map_err(to_js_err)?;
    serde_json::to_string(&InstrumentJson::CDSOption(option)).map_err(to_js_err)
}
