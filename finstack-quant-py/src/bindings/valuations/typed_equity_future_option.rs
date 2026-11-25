//! Typed options on listed equity futures.

use finstack_quant_valuations::instruments::equity::equity_future_option::{
    EquityFutureOption, EquityFutureOptionBuilder,
};

super::typed_macros::py_future_option!(
    PyEquityFutureOption,
    "EquityFutureOption",
    PyEquityFutureOptionBuilder,
    "EquityFutureOptionBuilder",
    EquityFutureOption,
    EquityFutureOptionBuilder,
    "finstack_quant.valuations.instruments._typed_equity_future_option"
);
