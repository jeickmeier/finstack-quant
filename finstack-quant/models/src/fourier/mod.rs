//! Fourier pricing engines.

pub mod characteristic_function;
pub mod cos;

pub use cos::{
    bs_cos_price, merton_jump_cos_price, vg_cos_price, BlackScholesCosParams, CosConfig, CosPricer,
    MertonJumpCosParams, VarianceGammaCosParams,
};

/// Product-independent Fourier pricing failure.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("Fourier model failure: {message}")]
pub struct FourierError {
    /// Diagnostic describing the invalid input or numerical failure.
    pub message: String,
    /// [`ErrorKind::Validation`](finstack_quant_core::error::ErrorKind::Validation)
    /// when the inputs were rejected before pricing (volatility, term count,
    /// truncation multiplier);
    /// [`ErrorKind::Computation`](finstack_quant_core::error::ErrorKind::Computation)
    /// when pricing failed numerically (degenerate truncation range,
    /// non-finite characteristic function or price).
    pub kind: finstack_quant_core::error::ErrorKind,
}

impl FourierError {
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: finstack_quant_core::error::ErrorKind::Validation,
        }
    }

    pub(crate) fn numerical(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: finstack_quant_core::error::ErrorKind::Computation,
        }
    }

    /// Classify this error for host-language exception mapping (see
    /// [`FourierError::kind`](Self::kind) field docs).
    #[must_use]
    pub fn kind(&self) -> finstack_quant_core::error::ErrorKind {
        self.kind
    }
}

impl From<FourierError> for finstack_quant_core::Error {
    /// Rejected inputs become validation errors and numerical failures
    /// `Calibration { category: "fourier" }` (a computation error), so the
    /// fold keeps [`FourierError::kind`].
    fn from(error: FourierError) -> Self {
        match error.kind {
            finstack_quant_core::error::ErrorKind::Computation => Self::Calibration {
                message: error.message,
                category: "fourier".to_string(),
            },
            _ => Self::Validation(format!("Fourier model failure: {}", error.message)),
        }
    }
}
