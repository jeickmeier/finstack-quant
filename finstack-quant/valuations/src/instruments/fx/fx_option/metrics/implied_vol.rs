//! Implied volatility metric for FX options.
//!
//! Solves for σ such that model PV(σ) equals the instrument's base PV
//! already computed in the `MetricContext`. Uses the configured pricer
//! (Hybrid solver under the hood) with log-σ parameterization.

use crate::define_metric_calculator;
use crate::instruments::fx::fx_option::FxOption;

define_metric_calculator!(
    /// Implied volatility metric for FX options.
    ImpliedVolCalculator,
    instrument = FxOption,
    calc = |option, ctx| {
        let target = ctx.base_value.amount();
        option.implied_vol(&ctx.curves, ctx.as_of, target, None)
    }
);
