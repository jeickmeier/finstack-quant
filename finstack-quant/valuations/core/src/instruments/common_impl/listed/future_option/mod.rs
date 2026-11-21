//! Shared economics and implementation support for asset-owned futures options.

mod instrument_impl;
mod pricing;
mod types;

#[cfg(test)]
mod tests;

pub use types::{
    FutureOptionExercise, FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement,
    FutureOptionTerms,
};
