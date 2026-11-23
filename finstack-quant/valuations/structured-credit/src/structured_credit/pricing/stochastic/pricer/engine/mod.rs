//! Stochastic structured-credit scenario waterfall pricing engine.

mod collector;
mod factors;
mod path;
mod pricer;
#[cfg(test)]
mod tests;

use pricer::{PreparedDefault, PreparedPrepayment};
pub(crate) use pricer::{PreparedRun, StochasticPricer};
