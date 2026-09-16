//! Typed `RevolvingCredit` instrument surface: the facility class, its
//! fluent builder and the per-path Monte Carlo result.
//!
//! Mirrors the `PyTermLoan` pattern in `instruments.rs`: every public Rust
//! field is a getter, `price` / `metric` share the `price_instrument`
//! pipeline, and the nested specs (`base_rate_spec`, `fees`,
//! `draw_repay_spec`) stay in their serde dict shape per the nested-spec rule.
//! `price_with_paths` and `expected_cashflows` bind the Rust
//! `RevolvingCreditPricer` entry points that have no `price_instrument` twin.

mod results;
mod revolving_credit;

use pyo3::prelude::*;

pub(crate) use results::PyEnhancedMonteCarloResult;
pub(crate) use revolving_credit::{PyRevolvingCredit, PyRevolvingCreditBuilder};

/// Register the typed revolving-credit classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRevolvingCredit>()?;
    m.add_class::<PyRevolvingCreditBuilder>()?;
    m.add_class::<PyEnhancedMonteCarloResult>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &[
    "EnhancedMonteCarloResult",
    "RevolvingCredit",
    "RevolvingCreditBuilder",
];
