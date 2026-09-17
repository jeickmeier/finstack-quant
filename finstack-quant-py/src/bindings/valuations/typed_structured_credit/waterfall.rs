//! Typed priority-of-payments introspection (`Waterfall`).

use pyo3::prelude::*;

use crate::bindings::pandas_utils::serde_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::Waterfall;

use super::PyCoverageRules;

/// The deal's priority of payments: ordered tiers (fees, interest, coverage
/// tests, principal, residual) with their recipients, allocation mode,
/// funding source and the coverage rules the tests use.
///
/// Obtain one from :meth:`StructuredCredit.create_waterfall` (the effective
/// template or custom waterfall) or :meth:`Waterfall.from_json`; pass a
/// ``Waterfall`` (or its dict) to :meth:`StructuredCreditBuilder.waterfall`
/// to run a custom priority of payments.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.dates import DayCount
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.valuations.instruments import (
/// ...     AssetPool, PoolAsset, StructuredCredit, Tranche, TrancheStructure,
/// ... )
/// >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
/// >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_assets([
/// ...     PoolAsset.fixed_rate_bond("LOAN-1", Money(80_000_000.0, Currency("USD")), 0.07, maturity, DayCount.ACT_360)
/// ... ])
/// >>> note = (
/// ...     Tranche.builder().id("A").attachment_point(0.0).detachment_point(100.0)
/// ...     .seniority("senior").original_balance(Money(80_000_000.0, Currency("USD")))
/// ...     .coupon_fixed(0.05).maturity(maturity).build()
/// ... )
/// >>> deal = StructuredCredit.new_clo(
/// ...     "CLO-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC", payment_calendar_id="nyse"
/// ... )
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
/// >>> waterfall = deal.create_waterfall()
/// >>> waterfall.base_currency, len(waterfall.tiers) > 0
/// ('USD', True)
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "Waterfall",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyWaterfall {
    /// Inner canonical Rust waterfall.
    pub(crate) inner: Waterfall,
}

sc_wire_methods!(PyWaterfall, Waterfall, "Waterfall");

#[pymethods]
impl PyWaterfall {
    /// Base ISO-4217 currency code of the waterfall.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Ordered tiers as ``WaterfallTier`` serde dicts (``id``, ``priority``,
    /// ``payment_type``, ``allocation_mode``, ``recipients``, ``tests``,
    /// ``funding``).
    #[getter]
    fn tiers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.tiers)
    }

    /// Collateral valuation rules attached to the coverage tests, or ``None``.
    #[getter]
    fn coverage_rules(&self) -> Option<PyCoverageRules> {
        self.inner
            .coverage_rules
            .clone()
            .map(|inner| PyCoverageRules { inner })
    }

    /// Every coverage test placed in the waterfall as ``CoverageTestSpec``
    /// serde dicts, in tier order.
    ///
    /// Returns
    /// -------
    /// list[dict]
    ///     One dict per test (``id``, ``tranche_id``, ``kind``,
    ///     ``trigger_level``, ``action`` ...).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the specs cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn coverage_tests<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let specs: Vec<_> = self.inner.coverage_tests().cloned().collect();
        serde_to_py(py, &specs)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Waterfall(base_currency={}, tiers={})",
            self.inner.base_currency,
            self.inner.tiers.len()
        )
    }
}
