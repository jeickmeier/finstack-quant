//! Typed options on listed FX futures.

use finstack_quant_valuations::instruments::fx::fx_future_option::{
    FxFutureOption, FxFutureOptionBuilder,
};

super::typed_macros::py_future_option!(
    PyFxFutureOption,
    "FxFutureOption",
    PyFxFutureOptionBuilder,
    "FxFutureOptionBuilder",
    FxFutureOption,
    FxFutureOptionBuilder,
    "finstack_quant.valuations.instruments._typed_fx_future_option"
);
