//! Typed options on listed FX futures.

use finstack_quant_valuations::instruments::fx::fx_future_option::{
    FxFutureOption, FxFutureOptionBuilder,
};
use wasm_bindgen::prelude::*;

wasm_future_option!(
    JsFxFutureOption,
    "FxFutureOption",
    JsFxFutureOptionBuilder,
    "FxFutureOptionBuilder",
    FxFutureOption,
    FxFutureOptionBuilder,
    "generated.valuations.FxFutureOption"
);
