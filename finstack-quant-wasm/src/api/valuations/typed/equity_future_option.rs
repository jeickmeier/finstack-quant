//! Typed options on listed equity futures.

use finstack_quant_valuations::instruments::equity::equity_future_option::{
    EquityFutureOption, EquityFutureOptionBuilder,
};
use wasm_bindgen::prelude::*;

wasm_future_option!(
    JsEquityFutureOption,
    "EquityFutureOption",
    JsEquityFutureOptionBuilder,
    "EquityFutureOptionBuilder",
    EquityFutureOption,
    EquityFutureOptionBuilder,
    "generated.valuations.EquityFutureOption"
);
