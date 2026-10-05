//! Fourier pricing engines.

pub mod characteristic_function;
pub mod cos;

pub use cos::{
    bs_cos_price, merton_jump_cos_price, vg_cos_price, CosConfig, CosMarketParams, CosPricer,
};

/// Inputs rejected before pricing (volatility, term count, truncation
/// multiplier): a validation error.
pub(crate) fn invalid_input(message: impl Into<String>) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(format!("Fourier model failure: {}", message.into()))
}

/// Pricing that failed numerically (degenerate truncation range, non-finite
/// characteristic function or price): `Calibration { category: "fourier" }`,
/// a computation error.
pub(crate) fn numerical(message: impl Into<String>) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Calibration {
        message: message.into(),
        category: "fourier".to_string(),
    }
}
