//! 1D root bracketing and solver-diagnostics utilities for calibration.

mod bracket;
#[cfg(test)]
mod tests;

pub(crate) use bracket::{BracketDiagnostics, ScanStrategy, bracket_solve_1d, diagnostics_bump_h};
