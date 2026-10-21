//! Shared economics and implementation support for asset-owned futures options.

mod instrument_impl;
mod types;

pub use types::{
    FutureOptionExercise, FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement,
    FutureOptionTerms,
};
