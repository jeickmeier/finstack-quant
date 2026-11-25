//! Typed options on listed interest rate futures.

use finstack_quant_valuations::instruments::rates::ir_future_option::{
    InterestRateFutureOption, InterestRateFutureOptionBuilder,
};
use wasm_bindgen::prelude::*;

wasm_future_option!(
    JsInterestRateFutureOption,
    "InterestRateFutureOption",
    JsInterestRateFutureOptionBuilder,
    "InterestRateFutureOptionBuilder",
    InterestRateFutureOption,
    InterestRateFutureOptionBuilder,
    "generated.valuations.InterestRateFutureOption"
);
