//! Typed options on listed volatility-index futures.

use finstack_quant_valuations::instruments::equity::vol_index_future_option::{
    VolatilityIndexFutureOption, VolatilityIndexFutureOptionBuilder,
};

super::typed_macros::py_future_option!(
    PyVolatilityIndexFutureOption,
    "VolatilityIndexFutureOption",
    PyVolatilityIndexFutureOptionBuilder,
    "VolatilityIndexFutureOptionBuilder",
    VolatilityIndexFutureOption,
    VolatilityIndexFutureOptionBuilder,
    "finstack_quant.valuations.instruments._typed_volatility_index_future_option"
);
