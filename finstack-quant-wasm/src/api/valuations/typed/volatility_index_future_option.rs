//! Typed options on listed volatility-index futures.

use finstack_quant_valuations::instruments::equity::vol_index_future_option::{
    VolatilityIndexFutureOption, VolatilityIndexFutureOptionBuilder,
};
use wasm_bindgen::prelude::*;

wasm_future_option!(
    JsVolatilityIndexFutureOption,
    "VolatilityIndexFutureOption",
    JsVolatilityIndexFutureOptionBuilder,
    "VolatilityIndexFutureOptionBuilder",
    VolatilityIndexFutureOption,
    VolatilityIndexFutureOptionBuilder,
    "generated.valuations.VolatilityIndexFutureOption"
);
