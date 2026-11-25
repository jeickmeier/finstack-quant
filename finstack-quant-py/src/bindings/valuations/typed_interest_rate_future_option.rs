//! Typed options on listed interest rate futures.

use finstack_quant_valuations::instruments::rates::ir_future_option::{
    InterestRateFutureOption, InterestRateFutureOptionBuilder,
};

super::typed_macros::py_future_option!(
    PyInterestRateFutureOption,
    "InterestRateFutureOption",
    PyInterestRateFutureOptionBuilder,
    "InterestRateFutureOptionBuilder",
    InterestRateFutureOption,
    InterestRateFutureOptionBuilder,
    "finstack_quant.valuations.instruments._typed_interest_rate_future_option"
);
