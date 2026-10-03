//! Stateful `FactorModel` handle over `finstack_quant_portfolio::factor_model::FactorModel`.
//!
//! The handle is built once from a `FactorModelConfig` and then reused for
//! assignment, sensitivities, risk decomposition, position what-if and factor
//! stress. Every method converts its arguments and calls one Rust method.

use pyo3::prelude::*;

use finstack_quant_models::factor::{FactorId, FactorModelConfig};
use finstack_quant_portfolio::factor_model::FactorModel;

use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::bindings::models::factor::credit::PyFactorModelConfig;
use crate::bindings::portfolio::sensitivity::PySensitivityMatrix;
use crate::errors::{portfolio_to_py, serde_json_to_py};

use super::assignment::PyFactorAssignmentReport;
use super::budget_whatif::{parse_position_changes, PyWhatIfResult};
use super::contributions::PyRiskDecomposition;
use super::stress::{PyStressPnl, PyStressResult};

/// Convert ``(factor_id, shift)`` pairs into Rust factor stresses.
fn to_stresses(stresses: Vec<(String, f64)>) -> Vec<(FactorId, f64)> {
    stresses
        .into_iter()
        .map(|(factor_id, shift)| (FactorId::new(factor_id), shift))
        .collect()
}

/// Portfolio factor-risk model built once from a ``FactorModelConfig``.
///
/// Holds the factor definitions, covariance matrix, dependency matcher and
/// sensitivity engine, so repeated analyses reuse one validated model instead
/// of rebuilding it per call.
///
/// Construct with :meth:`FactorModel.from_config`.
#[pyclass(
    name = "FactorModel",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFactorModel {
    pub(crate) inner: FactorModel,
}

#[pymethods]
impl PyFactorModel {
    /// Build a factor model from a declarative configuration.
    ///
    /// Parameters
    /// ----------
    /// config : FactorModelConfig | str
    ///     Typed ``finstack_quant.models.factor.credit.FactorModelConfig`` or
    ///     its canonical JSON: factor definitions, covariance matrix (axes in
    ///     factor order), matching rules, pricing mode, bump sizes and risk
    ///     measure.
    ///
    /// Returns
    /// -------
    /// FactorModel
    ///     Validated model handle.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or does not match the ``FactorModelConfig``
    ///     schema, matching rules reference undeclared factors, the covariance
    ///     axes do not align with the factors, or the risk measure is invalid.
    #[staticmethod]
    #[pyo3(text_signature = "(config)")]
    fn from_config(py: Python<'_>, config: &Bound<'_, PyAny>) -> PyResult<Self> {
        let config: FactorModelConfig = if let Ok(typed) = config.cast::<PyFactorModelConfig>() {
            typed.borrow().inner.clone()
        } else {
            let json: String = config.extract()?;
            py.detach(move || serde_json::from_str(&json))
                .map_err(|e| serde_json_to_py(e, "invalid FactorModelConfig JSON"))?
        };
        let inner = py
            .detach(move || FactorModel::from_config(config))
            .map_err(portfolio_to_py)?;
        Ok(Self { inner })
    }

    /// Match every position's market dependencies to the configured factors.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     Built ``Portfolio`` or ``PortfolioSpec`` JSON.
    /// market : MarketContext | str
    ///     Market used to resolve credit-index aggregates into their curve
    ///     dependencies.
    ///
    /// Returns
    /// -------
    /// FactorAssignmentReport
    ///     Per-position factor mappings plus unmatched dependencies.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a referenced credit index is absent from ``market``.
    /// PortfolioError
    ///     If the unmatched policy is strict and a dependency cannot be mapped.
    #[pyo3(text_signature = "(self, portfolio, market)")]
    fn assign_factors(
        &self,
        py: Python<'_>,
        portfolio: &Bound<'_, PyAny>,
        market: &Bound<'_, PyAny>,
    ) -> PyResult<PyFactorAssignmentReport> {
        let portfolio = extract_portfolio_ref(py, portfolio)?;
        let market = extract_market_ref(py, market)?;
        let (portfolio, market) = (&*portfolio, &*market);
        let inner = &self.inner;
        py.detach(move || inner.assign_factors(portfolio, market))
            .map(PyFactorAssignmentReport::from_inner)
            .map_err(portfolio_to_py)
    }

    /// Compute the weighted position-by-factor sensitivity matrix.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     Portfolio whose ``base_currency`` is the reporting currency of every
    ///     sensitivity.
    /// market : MarketContext | str
    ///     Market bumped by the sensitivity engine; must hold the FX needed for
    ///     cross-currency positions.
    /// as_of : datetime.date | str
    ///     Valuation date for sensitivities and spot FX.
    ///
    /// Returns
    /// -------
    /// SensitivityMatrix
    ///     One row per position, one column per factor, in factor bump units.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If market data or FX required by a position is missing.
    /// PortfolioError
    ///     If factor assignment or the sensitivity engine rejects the inputs.
    #[pyo3(text_signature = "(self, portfolio, market, as_of)")]
    fn compute_sensitivities(
        &self,
        py: Python<'_>,
        portfolio: &Bound<'_, PyAny>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PySensitivityMatrix> {
        let portfolio = extract_portfolio_ref(py, portfolio)?;
        let market = extract_market_ref(py, market)?;
        let as_of = crate::bindings::date_utils::extract_date(as_of)?;
        let (portfolio, market) = (&*portfolio, &*market);
        let base_currency = portfolio.base_currency;
        let inner = &self.inner;
        py.detach(move || inner.compute_sensitivities(portfolio, market, as_of))
            .map(|matrix| PySensitivityMatrix::from_inner(matrix, base_currency))
            .map_err(portfolio_to_py)
    }

    /// Decompose portfolio risk into factor and residual contributions.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     Portfolio to analyze.
    /// market : MarketContext | str
    ///     Market used for sensitivity generation.
    /// as_of : datetime.date | str
    ///     Valuation date of the analysis.
    ///
    /// Returns
    /// -------
    /// RiskDecomposition
    ///     Total, factor and position contributions in the configured risk
    ///     measure's units.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If market data required by a position is missing.
    /// PortfolioError
    ///     If assignment, sensitivity or decomposition inputs are invalid.
    #[pyo3(text_signature = "(self, portfolio, market, as_of)")]
    fn analyze(
        &self,
        py: Python<'_>,
        portfolio: &Bound<'_, PyAny>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyRiskDecomposition> {
        let portfolio = extract_portfolio_ref(py, portfolio)?;
        let market = extract_market_ref(py, market)?;
        let as_of = crate::bindings::date_utils::extract_date(as_of)?;
        let (portfolio, market) = (&*portfolio, &*market);
        let inner = &self.inner;
        py.detach(move || inner.analyze(portfolio, market, as_of))
            .map(PyRiskDecomposition::from_inner)
            .map_err(portfolio_to_py)
    }

    /// Run a position remove/resize what-if against the model's baseline.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     Portfolio the baseline is computed for and the changes apply to.
    /// market : MarketContext | str
    ///     Market used to value positions and compute sensitivities.
    /// as_of : datetime.date | str
    ///     Valuation date of the analysis.
    /// changes : list[dict] | str
    ///     ``PositionChange`` objects applied in order:
    ///     ``{"kind": "remove", "position_id": ...}`` or
    ///     ``{"kind": "resize", "position_id": ..., "new_quantity": ...}``.
    ///
    /// Returns
    /// -------
    /// WhatIfResult
    ///     Baseline and post-change decompositions with per-factor deltas.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``changes`` does not match the ``PositionChange`` schema.
    /// PortfolioError
    ///     If a change names an unknown position or the analysis fails.
    #[pyo3(text_signature = "(self, portfolio, market, as_of, changes)")]
    fn position_what_if(
        &self,
        py: Python<'_>,
        portfolio: &Bound<'_, PyAny>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        changes: &Bound<'_, PyAny>,
    ) -> PyResult<PyWhatIfResult> {
        let portfolio = extract_portfolio_ref(py, portfolio)?;
        let market = extract_market_ref(py, market)?;
        let as_of = crate::bindings::date_utils::extract_date(as_of)?;
        let changes = parse_position_changes(py, changes)?;
        let (portfolio, market) = (&*portfolio, &*market);
        let inner = &self.inner;
        py.detach(move || inner.position_what_if(portfolio, market, as_of, &changes))
            .map(PyWhatIfResult::from_inner)
            .map_err(portfolio_to_py)
    }

    /// Shock factors, reprice, and decompose risk under the stressed market.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     Portfolio whose P&L and stressed risk are evaluated.
    /// market : MarketContext | str
    ///     Baseline market to shock.
    /// as_of : datetime.date | str
    ///     Valuation date for both endpoints.
    /// stresses : list[tuple[str, float]]
    ///     ``(factor_id, shift)`` pairs; each shift is in the factor's
    ///     configured market-mapping units (e.g. bp for ``rate_bp``).
    ///
    /// Returns
    /// -------
    /// StressResult
    ///     Total and per-position P&L (base currency, loss negative) plus the
    ///     stressed risk decomposition.
    ///
    /// Raises
    /// ------
    /// PortfolioError
    ///     If a stress names an unknown factor or the bump is invalid.
    /// KeyError
    ///     If market data required for repricing is missing.
    #[pyo3(text_signature = "(self, portfolio, market, as_of, stresses)")]
    fn factor_stress(
        &self,
        py: Python<'_>,
        portfolio: &Bound<'_, PyAny>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        stresses: Vec<(String, f64)>,
    ) -> PyResult<PyStressResult> {
        let portfolio = extract_portfolio_ref(py, portfolio)?;
        let market = extract_market_ref(py, market)?;
        let as_of = crate::bindings::date_utils::extract_date(as_of)?;
        let stresses = to_stresses(stresses);
        let (portfolio, market) = (&*portfolio, &*market);
        let inner = &self.inner;
        py.detach(move || inner.factor_stress(portfolio, market, as_of, &stresses))
            .map(PyStressResult::from_inner)
            .map_err(portfolio_to_py)
    }

    /// Shock factors and reprice without decomposing stressed risk.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     Portfolio whose position P&L is evaluated.
    /// market : MarketContext | str
    ///     Baseline market to shock.
    /// as_of : datetime.date | str
    ///     Valuation date for both endpoints.
    /// stresses : list[tuple[str, float]]
    ///     ``(factor_id, shift)`` pairs in each factor's configured
    ///     market-mapping units.
    ///
    /// Returns
    /// -------
    /// StressPnl
    ///     Total and per-position stressed-minus-base P&L in the portfolio base
    ///     currency (loss negative).
    ///
    /// Raises
    /// ------
    /// PortfolioError
    ///     If a stress names an unknown factor or the bump is invalid.
    /// KeyError
    ///     If market data required for repricing is missing.
    #[pyo3(text_signature = "(self, portfolio, market, as_of, stresses)")]
    fn factor_stress_pnl(
        &self,
        py: Python<'_>,
        portfolio: &Bound<'_, PyAny>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        stresses: Vec<(String, f64)>,
    ) -> PyResult<PyStressPnl> {
        let portfolio = extract_portfolio_ref(py, portfolio)?;
        let market = extract_market_ref(py, market)?;
        let as_of = crate::bindings::date_utils::extract_date(as_of)?;
        let stresses = to_stresses(stresses);
        let (portfolio, market) = (&*portfolio, &*market);
        let inner = &self.inner;
        py.detach(move || inner.factor_stress_pnl(portfolio, market, as_of, &stresses))
            .map(PyStressPnl::from_inner)
            .map_err(portfolio_to_py)
    }

    fn __repr__(&self) -> String {
        format!("FactorModel(factors={})", self.inner.factors().len())
    }
}
