//! Shared listed-contract terms and generic option-on-future mechanics.

pub mod future_option;
mod terms;

pub use future_option::{
    FutureOptionExercise, FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement,
    FutureOptionTerms,
};
pub use terms::{ListedFutureSettlement, ListedFutureTerms};
