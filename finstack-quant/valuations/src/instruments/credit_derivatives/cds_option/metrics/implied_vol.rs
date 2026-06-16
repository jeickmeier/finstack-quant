//! Implied volatility metric for `CDSOption`.
//!
//! Computes the Black-on-spreads implied volatility that matches the
//! instrument's current PV (`context.base_value`) using the CDS option
//! pricer and core math solvers (HybridSolver).

use crate::define_metric_calculator;
use crate::instruments::credit_derivatives::cds_option::CDSOption;

define_metric_calculator!(
    /// Implied volatility metric for credit options on CDS spreads.
    ImpliedVolCalculator,
    instrument = CDSOption,
    calc = |option, ctx| {
        let target = ctx.base_value.amount();
        option.implied_vol(&ctx.curves, ctx.as_of, target, None)
    }
);
