//! Main evaluator implementation.

pub(crate) mod prepared;
pub(crate) mod runner;
#[cfg(test)]
mod tests;

pub(crate) use prepared::{McZCache, PeriodEval, PeriodRow};
