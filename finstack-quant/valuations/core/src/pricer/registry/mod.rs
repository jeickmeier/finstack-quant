//! Pricer registry, trait, and dispatch infrastructure.
//!
//! Defines the [`Pricer`] trait, [`PricerRegistry`], and the [`expect_inst`]
//! downcast helper used by all pricer implementations.

pub(crate) mod dispatch;
#[cfg(test)]
mod tests;

pub(super) use dispatch::attach_metric_measures;
#[cfg(test)]
pub(super) use dispatch::{Pricer, PricerRegistry, PricingDispatch};
