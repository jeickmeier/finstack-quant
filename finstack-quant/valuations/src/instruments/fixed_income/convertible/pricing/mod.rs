//! Convertible Bond pricing facade and pricer re-export.
//!
//! Exposes the pricing entrypoints for `ConvertibleBond`. Core pricing
//! logic lives in `pricer`. Instruments and metrics should depend on this
//! module rather than private files to keep the public API stable.

pub(crate) mod pricer;

pub use pricer::{
    calculate_conversion_premium, calculate_convertible_greeks, calculate_parity,
    compute_conversion_value, price_convertible_bond, ConvertibleTreeType,
};
