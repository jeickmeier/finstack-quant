//! Typed options on listed commodity futures.

use finstack_quant_valuations::instruments::commodity::commodity_future_option::{
    CommodityFutureOption, CommodityFutureOptionBuilder,
};
use wasm_bindgen::prelude::*;

wasm_future_option!(
    JsCommodityFutureOption,
    "CommodityFutureOption",
    JsCommodityFutureOptionBuilder,
    "CommodityFutureOptionBuilder",
    CommodityFutureOption,
    CommodityFutureOptionBuilder,
    "generated.valuations.CommodityFutureOption"
);
