//! Coverage test calculations for structured credit instruments.
//!
//! This module provides OC and IC test calculations for waterfall diversion.

pub(crate) mod evaluator;
#[cfg(test)]
mod tests;

pub(crate) use evaluator::{TestContext, TestResult};
