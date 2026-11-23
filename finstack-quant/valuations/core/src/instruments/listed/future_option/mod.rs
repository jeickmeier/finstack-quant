//! Shared economics and implementation support for asset-owned futures options.

mod instrument_impl;
mod pricing;
pub(crate) mod types;

#[cfg(test)]
mod tests;

#[cfg(test)]
use types::{
    FutureOptionExercise, FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement,
    FutureOptionTerms,
};
