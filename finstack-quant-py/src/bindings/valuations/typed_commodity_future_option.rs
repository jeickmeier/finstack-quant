//! Typed options on listed commodity futures.

use finstack_quant_valuations::instruments::commodity::commodity_future_option::{
    CommodityFutureOption, CommodityFutureOptionBuilder,
};

super::typed_macros::py_future_option!(
    PyCommodityFutureOption,
    "CommodityFutureOption",
    PyCommodityFutureOptionBuilder,
    "CommodityFutureOptionBuilder",
    CommodityFutureOption,
    CommodityFutureOptionBuilder,
    "finstack_quant.valuations.instruments._typed_commodity_future_option"
);
