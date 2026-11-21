//! Python wrappers for margin metric types.
//!
//! The Rust factories are `MarginUtilization::new`, `ExcessCollateral::new`,
//! `MarginFundingCost::calculate` and `Haircut01::calculate`; Python exposes
//! each as the class constructor (the documented host idiom for
//! compute-on-construct value objects).

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::table_builders::TableBuilder;
use crate::errors::core_to_py;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use pyo3::prelude::*;

fn money(amount: f64, currency: &str) -> PyResult<Money> {
    let ccy: Currency = currency.parse().map_err(crate::errors::core_to_py)?;
    Money::new(amount, ccy).map_err(core_to_py)
}

/// Margin utilization result (ratio of posted to required margin).
#[pyclass(
    name = "MarginUtilization",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMarginUtilization {
    inner: finstack_quant_margin::metrics::MarginUtilization,
}

wire_methods!(
    PyMarginUtilization,
    finstack_quant_margin::metrics::MarginUtilization,
    "MarginUtilization"
);
py_getters!(PyMarginUtilization |s| {
    /// Posted margin amount.
    posted -> f64 = s.inner.posted.amount();
    /// Required margin amount.
    required -> f64 = s.inner.required.amount();
    /// Utilization ratio (posted / required).
    ratio -> f64 = s.inner.ratio;
});

#[pymethods]
impl PyMarginUtilization {
    /// Compare posted and required margin in one currency.
    ///
    /// ``posted_amount`` and ``required_amount`` are floats in ``currency``.
    /// Raises ``ValueError`` for an unknown currency or a non-finite amount.
    #[new]
    fn new(posted_amount: f64, required_amount: f64, currency: &str) -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_margin::metrics::MarginUtilization::new(
                money(posted_amount, currency)?,
                money(required_amount, currency)?,
            )
            .map_err(core_to_py)?,
        })
    }

    /// Whether margin is adequate (ratio >= 1.0).
    fn is_adequate(&self) -> bool {
        self.inner.is_adequate()
    }

    /// Shortfall amount (zero when adequate).
    fn shortfall(&self) -> PyResult<f64> {
        Ok(self.inner.shortfall().map_err(core_to_py)?.amount())
    }

    fn __repr__(&self) -> String {
        format!(
            "MarginUtilization(posted={:.2}, required={:.2}, ratio={:.4}, adequate={})",
            self.inner.posted.amount(),
            self.inner.required.amount(),
            self.inner.ratio,
            if self.inner.is_adequate() {
                "True"
            } else {
                "False"
            }
        )
    }
}

impl PyMarginUtilization {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("posted", vec![self.inner.posted.amount()])?
            .f64("required", vec![self.inner.required.amount()])?
            .f64("ratio", vec![self.inner.ratio])?
            .f64(
                "shortfall",
                vec![self.inner.shortfall().map_err(core_to_py)?.amount()],
            )?
            .bool("is_adequate", vec![self.inner.is_adequate()])?
            .utf8("currency", vec![self.inner.posted.currency().to_string()])?
            .build()
    }
}

tabular_methods!(PyMarginUtilization);

/// Excess collateral result.
#[pyclass(
    name = "ExcessCollateral",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyExcessCollateral {
    inner: finstack_quant_margin::metrics::ExcessCollateral,
}

wire_methods!(
    PyExcessCollateral,
    finstack_quant_margin::metrics::ExcessCollateral,
    "ExcessCollateral"
);
py_getters!(PyExcessCollateral |s| {
    /// Market value of posted collateral in the CSA currency.
    collateral_value -> f64 = s.inner.collateral_value.amount();
    /// Required collateral amount in the same currency as the mark.
    required_value -> f64 = s.inner.required_value.amount();
    /// Excess amount (positive) or shortfall (negative).
    excess -> f64 = s.inner.excess.amount();
});

#[pymethods]
impl PyExcessCollateral {
    /// Compare posted collateral value with the required amount in one
    /// currency. Raises ``ValueError`` for an unknown currency or a
    /// non-finite amount.
    #[new]
    fn new(collateral_value: f64, required_value: f64, currency: &str) -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_margin::metrics::ExcessCollateral::new(
                money(collateral_value, currency)?,
                money(required_value, currency)?,
            )
            .map_err(core_to_py)?,
        })
    }

    /// Whether there is excess collateral.
    fn has_excess(&self) -> bool {
        self.inner.has_excess()
    }

    /// Whether there is a shortfall.
    fn has_shortfall(&self) -> bool {
        self.inner.has_shortfall()
    }

    /// Excess as a decimal fraction of the required amount.
    fn excess_percentage(&self) -> f64 {
        self.inner.excess_percentage()
    }

    fn __repr__(&self) -> String {
        format!(
            "ExcessCollateral(collateral_value={:.2}, required_value={:.2}, excess={:.2}, excess_percentage={:.4})",
            self.inner.collateral_value.amount(),
            self.inner.required_value.amount(),
            self.inner.excess.amount(),
            self.inner.excess_percentage()
        )
    }
}

impl PyExcessCollateral {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64(
                "collateral_value",
                vec![self.inner.collateral_value.amount()],
            )?
            .f64("required_value", vec![self.inner.required_value.amount()])?
            .f64("excess", vec![self.inner.excess.amount()])?
            .f64("excess_percentage", vec![self.inner.excess_percentage()])?
            .bool("has_excess", vec![self.inner.has_excess()])?
            .bool("has_shortfall", vec![self.inner.has_shortfall()])?
            .utf8(
                "currency",
                vec![self.inner.collateral_value.currency().to_string()],
            )?
            .build()
    }
}

tabular_methods!(PyExcessCollateral);

/// Margin funding cost result.
#[pyclass(
    name = "MarginFundingCost",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMarginFundingCost {
    inner: finstack_quant_margin::metrics::MarginFundingCost,
}

wire_methods!(
    PyMarginFundingCost,
    finstack_quant_margin::metrics::MarginFundingCost,
    "MarginFundingCost"
);
py_getters!(PyMarginFundingCost |s| {
    /// Posted margin amount.
    margin_posted -> f64 = s.inner.margin_posted.amount();
    /// Funding rate (annualized decimal).
    funding_rate -> f64 = s.inner.funding_rate;
    /// Collateral return rate (annualized decimal).
    collateral_rate -> f64 = s.inner.collateral_rate;
    /// Annualized funding cost.
    annual_cost -> f64 = s.inner.annual_cost.amount();
});

#[pymethods]
impl PyMarginFundingCost {
    /// Compute the annual funding cost of posted margin.
    ///
    /// ``margin_posted`` is a float in ``currency``; ``funding_rate`` and
    /// ``collateral_rate`` are annualised decimal rates (``0.03`` = 3%).
    /// Raises ``ValueError`` for an unknown currency, a non-finite amount or
    /// funding spread, or an annual cost outside the Decimal range.
    #[new]
    fn new(
        margin_posted: f64,
        funding_rate: f64,
        collateral_rate: f64,
        currency: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_margin::metrics::MarginFundingCost::calculate(
                money(margin_posted, currency)?,
                funding_rate,
                collateral_rate,
            )
            .map_err(core_to_py)?,
        })
    }

    /// Funding spread (funding rate - collateral rate), decimal.
    fn spread_rate(&self) -> f64 {
        self.inner.spread_rate()
    }

    /// Annualized funding spread in basis points.
    fn spread_bp(&self) -> f64 {
        self.inner.spread_bp()
    }

    /// Cost for a period of ``year_fraction`` years; raises ``ValueError``
    /// for a non-finite fraction or a cost outside the Decimal range.
    fn cost_for_period(&self, year_fraction: f64) -> PyResult<f64> {
        self.inner
            .cost_for_period(year_fraction)
            .map(|cost| cost.amount())
            .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "MarginFundingCost(margin_posted={:.2}, funding_rate={:.4}, collateral_rate={:.4}, annual_cost={:.2})",
            self.inner.margin_posted.amount(),
            self.inner.funding_rate,
            self.inner.collateral_rate,
            self.inner.annual_cost.amount()
        )
    }
}

impl PyMarginFundingCost {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("margin_posted", vec![self.inner.margin_posted.amount()])?
            .f64("funding_rate", vec![self.inner.funding_rate])?
            .f64("collateral_rate", vec![self.inner.collateral_rate])?
            .f64("spread_rate", vec![self.inner.spread_rate()])?
            .f64("annual_cost", vec![self.inner.annual_cost.amount()])?
            .utf8(
                "currency",
                vec![self.inner.margin_posted.currency().to_string()],
            )?
            .build()
    }
}

tabular_methods!(PyMarginFundingCost);

/// Haircut sensitivity: PV change for +1bp haircut change.
#[pyclass(
    name = "Haircut01",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHaircut01 {
    inner: finstack_quant_margin::metrics::Haircut01,
}

wire_methods!(
    PyHaircut01,
    finstack_quant_margin::metrics::Haircut01,
    "Haircut01"
);
py_getters!(PyHaircut01 |s| {
    /// Collateral value.
    collateral_value -> f64 = s.inner.collateral_value.amount();
    /// Current haircut (decimal).
    current_haircut -> f64 = s.inner.current_haircut;
    /// PV change for a +1bp haircut.
    pv_change -> f64 = s.inner.pv_change.amount();
});

#[pymethods]
impl PyHaircut01 {
    /// Compute the collateral-value sensitivity to a +1bp haircut change.
    ///
    /// ``collateral_value`` is a float in ``currency``; ``current_haircut``
    /// is a decimal fraction (``0.05`` = 5%). Raises ``ValueError`` for an
    /// unknown currency or a non-finite amount.
    #[new]
    fn new(collateral_value: f64, current_haircut: f64, currency: &str) -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_margin::metrics::Haircut01::calculate(
                money(collateral_value, currency)?,
                current_haircut,
            )
            .map_err(core_to_py)?,
        })
    }

    /// Current haircut in basis points.
    fn haircut_bp(&self) -> f64 {
        self.inner.haircut_bp()
    }

    fn __repr__(&self) -> String {
        format!(
            "Haircut01(collateral_value={:.2}, current_haircut={:.4}, pv_change={:.2})",
            self.inner.collateral_value.amount(),
            self.inner.current_haircut,
            self.inner.pv_change.amount()
        )
    }
}

impl PyHaircut01 {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64(
                "collateral_value",
                vec![self.inner.collateral_value.amount()],
            )?
            .f64("current_haircut", vec![self.inner.current_haircut])?
            .f64("haircut_bp", vec![self.inner.haircut_bp()])?
            .f64("pv_change", vec![self.inner.pv_change.amount()])?
            .utf8(
                "currency",
                vec![self.inner.collateral_value.currency().to_string()],
            )?
            .build()
    }
}

tabular_methods!(PyHaircut01);

/// Register metric classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMarginUtilization>()?;
    m.add_class::<PyExcessCollateral>()?;
    m.add_class::<PyMarginFundingCost>()?;
    m.add_class::<PyHaircut01>()?;
    Ok(())
}
