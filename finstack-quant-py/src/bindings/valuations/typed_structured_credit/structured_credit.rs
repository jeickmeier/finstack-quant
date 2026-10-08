use pyo3::prelude::*;

use crate::bindings::cashflows::builder::specs::{
    PyDefaultModelSpec, PyPrepaymentModelSpec, PyRecoveryModelSpec,
};
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{attributes_from_py, bool_repr, enum_to_py_string};
use crate::errors::core_to_py;
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_equity_metrics, run_simulation_with_diagnostics, CreditModelConfig, DealFees,
    DealType, LossAllocationPolicy, LossRecognition, MarketConditions, Metadata, StructuredCredit,
    TrancheDraw, TrancheReadvance, WaterfallRules,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};

use super::super::instruments::{
    enum_from_str, instrument_expiry, serialize_typed_instrument_json,
};
use super::super::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};
use super::hedge_swap::hedge_swaps_from_py;
use super::{
    PyAssetPool, PyCallAssumption, PyCoverageRules, PyEquityMetrics, PyHedgeSwap,
    PySimulationDiagnostics, PyStochasticPricingResult, PyTrancheCashflows, PyTrancheStructure,
    PyWaterfall,
};

type StructuredCreditBuilderInner =
    finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCreditBuilder;

/// Typed wrapper for the Rust `StructuredCredit` instrument (ABS/CLO/CMBS/RMBS).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "StructuredCredit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyStructuredCredit {
    /// Inner canonical Rust structured-credit deal.
    pub(crate) inner: StructuredCredit,
}

impl PyStructuredCredit {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::StructuredCredit(Box::new(self.inner.clone())),
            "StructuredCredit",
        )
    }
}

instrument_envelope_methods!(
    PyStructuredCredit,
    StructuredCredit,
    "structured_credit",
    PyStructuredCreditBuilder,
    StructuredCredit::builder(),
    no_fields,
    builder_doc = [
        " ",
        " Notes",
        " -----",
        " The builder pre-seeds ``market_conditions``, ``deal_metadata`` and",
        " ``hedge_swaps`` with their Rust ``Default`` values (the Rust builder",
        " fields have no default), which the corresponding setters",
        " (``market_conditions``, ``waterfall_rules``, ``fees``, ``credit_model``,",
        " ``hedge_swaps`` ...) can override with typed objects, dicts or JSON",
        " strings. Prefer :meth:`new_abs` / :meth:`new_clo` / :meth:`new_cmbs` /",
        " :meth:`new_rmbs` for registry-calibrated deal-type defaults; use this",
        " builder for full manual control.",
    ]
);
instrument_pricing_methods!(
    PyStructuredCredit,
    model_doc = [
        "     ``\"default\"`` is the deal's ``default_model``: the deterministic",
        "     ``\"discounting\"`` waterfall.",
    ]
);

#[pymethods]
impl PyStructuredCredit {
    /// Create a new ABS deal with registry-calibrated defaults.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// pool : AssetPool
    ///     Asset pool definition.
    /// tranches : TrancheStructure
    ///     Tranche capital structure.
    /// closing_date : datetime.date
    ///     Deal closing date (issuance).
    /// maturity : datetime.date
    ///     Legal final maturity date.
    /// discount_curve_id : str
    ///     Discount curve identifier for valuation.
    /// calendar_id : str, optional
    ///     Holiday calendar for the payment schedule (e.g. ``"nyse"``);
    ///     required before pricing, so pass it here or set it on the JSON.
    ///
    /// Returns
    /// -------
    /// StructuredCredit
    ///     The validated ABS deal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the deal fails pricing validation or its first registry payment period exceeds the supported calendar range.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.dates import DayCount
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import (
    /// ...     AssetPool, RepLine, StructuredCredit, Tranche, TrancheStructure,
    /// ... )
    /// >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
    /// ...     RepLine(
    /// ...         "LINE-1", Money(80_000_000.0, Currency("USD")), 0.07,
    /// ...         datetime.date(2031, 1, 15), 12, DayCount.ACT_360, asset_type={"type": "first_lien_loan"},
    /// ...     )
    /// ... ])
    /// >>> senior = (
    /// ...     Tranche.builder().id("A").attach_pct(10.0).detach_pct(100.0)
    /// ...     .seniority("senior").original_balance(Money(72_000_000.0, Currency("USD")))
    /// ...     .coupon_fixed(0.05).maturity(datetime.date(2031, 1, 15)).build()
    /// ... )
    /// >>> equity = (
    /// ...     Tranche.builder().id("E").attach_pct(0.0).detach_pct(10.0)
    /// ...     .seniority("equity").original_balance(Money(8_000_000.0, Currency("USD")))
    /// ...     .coupon_fixed(0.0).maturity(datetime.date(2031, 1, 15)).build()
    /// ... )
    /// >>> deal = StructuredCredit.new_abs(
    /// ...     "ABS-1", pool, TrancheStructure([senior, equity]),
    /// ...     datetime.date(2024, 1, 15), datetime.date(2031, 1, 15), "USD-SOFR-DISC",
    /// ... )
    /// >>> "ABS-1" in repr(deal)
    /// True
    #[staticmethod]
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None))]
    #[pyo3(
        text_signature = "(id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn new_abs(
        id: &str,
        pool: PyRef<'_, PyAssetPool>,
        tranches: PyRef<'_, PyTrancheStructure>,
        closing_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = StructuredCredit::new_abs(
            id,
            pool.inner.clone(),
            tranches.inner.clone(),
            py_to_date(closing_date)?,
            py_to_date(maturity)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        if let Some(calendar_id) = calendar_id {
            inner = inner.with_calendar_id(calendar_id);
        }
        inner.validate_for_pricing().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a new CLO deal with registry-calibrated defaults.
    ///
    /// See :meth:`new_abs` for parameter and return documentation; the
    /// signature is identical, only the deal-type calibration differs.
    #[staticmethod]
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None))]
    #[pyo3(
        text_signature = "(id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn new_clo(
        id: &str,
        pool: PyRef<'_, PyAssetPool>,
        tranches: PyRef<'_, PyTrancheStructure>,
        closing_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = StructuredCredit::new_clo(
            id,
            pool.inner.clone(),
            tranches.inner.clone(),
            py_to_date(closing_date)?,
            py_to_date(maturity)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        if let Some(calendar_id) = calendar_id {
            inner = inner.with_calendar_id(calendar_id);
        }
        inner.validate_for_pricing().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a new CMBS deal with registry-calibrated defaults.
    ///
    /// See :meth:`new_abs` for parameter and return documentation; the
    /// signature is identical, only the deal-type calibration differs.
    #[staticmethod]
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None))]
    #[pyo3(
        text_signature = "(id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn new_cmbs(
        id: &str,
        pool: PyRef<'_, PyAssetPool>,
        tranches: PyRef<'_, PyTrancheStructure>,
        closing_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = StructuredCredit::new_cmbs(
            id,
            pool.inner.clone(),
            tranches.inner.clone(),
            py_to_date(closing_date)?,
            py_to_date(maturity)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        if let Some(calendar_id) = calendar_id {
            inner = inner.with_calendar_id(calendar_id);
        }
        inner.validate_for_pricing().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a new RMBS deal with registry-calibrated defaults.
    ///
    /// See :meth:`new_abs` for parameter and return documentation; the
    /// signature is identical, only the deal-type calibration differs.
    #[staticmethod]
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None))]
    #[pyo3(
        text_signature = "(id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn new_rmbs(
        id: &str,
        pool: PyRef<'_, PyAssetPool>,
        tranches: PyRef<'_, PyTrancheStructure>,
        closing_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = StructuredCredit::new_rmbs(
            id,
            pool.inner.clone(),
            tranches.inner.clone(),
            py_to_date(closing_date)?,
            py_to_date(maturity)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        if let Some(calendar_id) = calendar_id {
            inner = inner.with_calendar_id(calendar_id);
        }
        inner.validate_for_pricing().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Price the deal with the scenario-waterfall Monte Carlo engine.
    ///
    /// Every path runs the full period loop and waterfall on simulated
    /// prepayment, default and recovery paths (and, for pools of real
    /// instruments, per-name defaults and simulated revolver draws).
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context with the deal's discount curve and every curve
    ///     the collateral references.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// num_paths : int, optional
    ///     Number of independent Monte Carlo estimators; must be at least two.
    ///     Defaults to the deal's configured ``mc_paths`` override or 5,000.
    ///     With ``antithetic`` each estimator averages a mirrored pair, so
    ///     ``2 * num_paths`` physical paths run while the statistical sample
    ///     size remains ``num_paths``.
    /// antithetic : bool, optional
    ///     Pair each estimator's path with its sign-flipped mirror; defaults
    ///     to the deal's configured ``mc_antithetic`` override or ``True``.
    ///
    /// Returns
    /// -------
    /// StochasticPricingResult
    ///     Deal and tranche present values, loss statistics, Monte Carlo
    ///     error, draw diagnostics and the draw option cost. Sampling error
    ///     uses the sample standard deviation and a 95% Student-t confidence
    ///     interval with ``num_paths - 1`` degrees of freedom.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the deal fails validation or the resolved ``num_paths`` is less
    ///     than two independent estimators.
    /// KeyError
    ///     If a required curve is missing from ``market``.
    /// RuntimeError
    ///     If the simulation fails.
    #[pyo3(signature = (market, as_of, num_paths=None, antithetic=None))]
    #[pyo3(text_signature = "($self, market, as_of, num_paths=None, antithetic=None)")]
    fn price_stochastic(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        num_paths: Option<usize>,
        antithetic: Option<bool>,
    ) -> PyResult<PyStochasticPricingResult> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let deal = self.inner.clone();
        let inner = py
            .detach(move || deal.price_stochastic(&market, as_of, num_paths, antithetic))
            .map_err(core_to_py)?;
        Ok(PyStochasticPricingResult { inner })
    }

    /// Run the deterministic simulation and return the deal-level accounting.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context with the deal's discount curve and every curve
    ///     the collateral references.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// SimulationDiagnostics
    ///     Reserve balance and interest per period, draw funding by source
    ///     and unfunded draws.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the deal fails validation or (for instrument collateral) the
    ///     contractual draw calendar cannot be funded.
    /// KeyError
    ///     If a required curve is missing from ``market``.
    /// RuntimeError
    ///     If the simulation fails.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn run_simulation_with_diagnostics(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PySimulationDiagnostics> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let deal = self.inner.clone();
        let run = py
            .detach(move || run_simulation_with_diagnostics(&deal, &market, as_of))
            .map_err(core_to_py)?;
        Ok(PySimulationDiagnostics {
            inner: run.diagnostics,
        })
    }

    /// Deal classification (serde name: ``"abs"``, ``"clo"``, ``"cmbs"``, ``"rmbs"`` ...).
    #[getter]
    fn deal_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.deal_type)
    }

    /// Collateral pool.
    #[getter]
    fn pool(&self) -> PyAssetPool {
        PyAssetPool {
            inner: self.inner.pool.clone(),
        }
    }

    /// Capital structure.
    #[getter]
    fn tranches(&self) -> PyTrancheStructure {
        PyTrancheStructure {
            inner: self.inner.tranches.clone(),
        }
    }

    /// Deal closing date as ``datetime.date``.
    #[getter]
    fn closing_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.closing_date)
    }

    /// First tranche payment date as ``datetime.date``.
    #[getter]
    fn first_payment_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.first_payment_date)
    }

    /// Buyer quote settlement date as ``datetime.date``, or ``None`` for valuation date.
    #[getter]
    fn quote_settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .quote_settlement_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Legal final maturity as ``datetime.date``.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Return a copy carrying the deal-type standard fee schedule (mirrors
    /// Rust ``StructuredCredit::with_standard_fees``).
    ///
    /// Returns
    /// -------
    /// StructuredCredit
    ///     A new deal with ``fees`` set to the CLO / CMBS / RMBS / ABS
    ///     standard.
    #[pyo3(text_signature = "($self)")]
    fn with_standard_fees(&self) -> Self {
        Self {
            inner: self.inner.clone().with_standard_fees(),
        }
    }

    /// Return a copy with the deal-type stochastic prepayment, default and
    /// correlation specifications enabled for ``price_stochastic``.
    ///
    /// Returns
    /// -------
    /// StructuredCredit
    ///     A new deal with the stochastic specs attached.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the deal-type defaults cannot be built (for example an empty
    ///     pool for the RMBS coupon-driven incentive).
    #[pyo3(text_signature = "($self)")]
    fn enable_stochastic(&self) -> PyResult<Self> {
        let mut deal = self.inner.clone();
        deal.enable_stochastic().map_err(core_to_py)?;
        Ok(Self { inner: deal })
    }

    /// Effective priority of payments: the custom waterfall when one is
    /// set, otherwise the deal-type template with fees, coverage tests and
    /// hedges placed.
    ///
    /// Returns
    /// -------
    /// Waterfall
    ///     The waterfall the deterministic engine executes.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the template cannot be synthesized (invalid tranches, tests or
    ///     fees).
    #[pyo3(text_signature = "($self)")]
    fn create_waterfall(&self) -> PyResult<PyWaterfall> {
        let inner = self.inner.create_waterfall().map_err(core_to_py)?;
        Ok(PyWaterfall { inner })
    }

    /// Project one tranche's cashflows through the deterministic engine.
    ///
    /// Parameters
    /// ----------
    /// tranche_id : str
    ///     Identifier of the class.
    /// market : MarketContext
    ///     Curves and fixings for floating coupons and collateral.
    /// as_of : datetime.date
    ///     Valuation date the projection starts from.
    ///
    /// Returns
    /// -------
    /// TrancheCashflows
    ///     The class's projected flows and components.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``tranche_id`` is not a class of the deal.
    /// ValueError
    ///     If the deal fails pricing validation, ``as_of`` is invalid or
    ///     required market data is missing.
    #[pyo3(text_signature = "($self, tranche_id, market, as_of)")]
    fn tranche_cashflows(
        &self,
        py: Python<'_>,
        tranche_id: &str,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyTrancheCashflows> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let deal = self.inner.clone();
        let tranche_id = tranche_id.to_string();
        let inner = py
            .detach(move || deal.tranche_cashflows(&tranche_id, &market, as_of))
            .map_err(core_to_py)?;
        Ok(PyTrancheCashflows { inner })
    }

    /// Residual-class return analytics of the deterministic projection.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext
    ///     Curves and fixings for the projection and the NAV discounting.
    /// as_of : datetime.date
    ///     Valuation date; the invested amount is dated here.
    /// purchase_price_pct : float, optional
    ///     Entry price as a percent of the equity balance; par when omitted.
    ///
    /// Returns
    /// -------
    /// EquityMetrics
    ///     IRR, MOIC, NAV and cash-on-cash series of the residual class.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the deal has no residual class, fails pricing validation, or
    ///     ``as_of`` is invalid.
    #[pyo3(signature = (market, as_of, purchase_price_pct=None))]
    #[pyo3(text_signature = "($self, market, as_of, purchase_price_pct=None)")]
    fn equity_metrics(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        purchase_price_pct: Option<f64>,
    ) -> PyResult<PyEquityMetrics> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let deal = self.inner.clone();
        let inner = py
            .detach(move || calculate_equity_metrics(&deal, &market, as_of, purchase_price_pct))
            .map_err(core_to_py)?;
        Ok(PyEquityMetrics { inner })
    }

    /// Payment frequency.
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor {
            inner: self.inner.frequency,
        }
    }

    /// Payment calendar identifier, or ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.as_ref().map(ToString::to_string)
    }

    /// Payment business-day convention string, or ``None`` for the default.
    #[getter]
    fn business_day_convention(&self) -> PyResult<Option<String>> {
        self.inner
            .business_day_convention
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Credit model as its ``CreditModelConfig`` serde ``dict``.
    #[getter]
    fn credit_model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_model)
    }

    /// Deterministic prepayment model as a typed ``PrepaymentModelSpec``.
    #[getter]
    fn prepayment_spec(&self) -> PyPrepaymentModelSpec {
        PyPrepaymentModelSpec {
            inner: self.inner.credit_model.prepayment_spec.clone(),
        }
    }

    /// Deterministic default model as a typed ``DefaultModelSpec``.
    #[getter]
    fn default_spec(&self) -> PyDefaultModelSpec {
        PyDefaultModelSpec {
            inner: self.inner.credit_model.default_spec.clone(),
        }
    }

    /// Recovery model as a typed ``RecoveryModelSpec``.
    #[getter]
    fn recovery_spec(&self) -> PyRecoveryModelSpec {
        PyRecoveryModelSpec {
            inner: self.inner.credit_model.recovery_spec.clone(),
        }
    }

    /// Stochastic prepayment specification as its serde ``dict``, or ``None``.
    #[getter]
    fn stochastic_prepay_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .stochastic_prepay_spec
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Stochastic default specification as its serde ``dict``, or ``None``.
    #[getter]
    fn stochastic_default_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .stochastic_default_spec
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Stochastic recovery specification (``RecoverySpec`` serde ``dict``:
    /// ``{"type": "constant", "rate": ...}`` or ``{"type":
    /// "market_correlated", "mean_recovery": ..., "recovery_volatility": ...,
    /// "factor_correlation": ...}``), or ``None`` for constant recoveries at
    /// the deterministic rate.
    #[getter]
    fn stochastic_recovery_spec<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .stochastic_recovery_spec
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Default correlation structure as its serde ``dict``, or ``None``.
    #[getter]
    fn correlation_structure<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .correlation_structure
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Delinquency model as its ``DelinquencyModel`` serde ``dict``, or ``None``.
    #[getter]
    fn delinquency<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .delinquency
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Card portfolio model as its ``CardPortfolioSpec`` serde ``dict``, or ``None``.
    #[getter]
    fn card<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .card
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Market conditions as their serde ``dict``.
    #[getter]
    fn market_conditions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.market_conditions)
    }

    /// Deal metadata as its serde ``dict``.
    #[getter]
    fn deal_metadata<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.deal_metadata)
    }

    /// Hedges settled through the waterfall as typed ``HedgeSwap`` objects.
    #[getter]
    fn hedge_swaps(&self) -> Vec<PyHedgeSwap> {
        self.inner
            .hedge_swaps
            .iter()
            .map(|hedge| PyHedgeSwap {
                inner: hedge.clone(),
            })
            .collect()
    }

    /// Senior transaction fees as their ``DealFees`` serde ``dict``, or
    /// ``None`` when no fee tier is attached.
    #[getter]
    fn fees<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .fees
            .as_ref()
            .map(|fees| serde_to_py(py, fees))
            .transpose()
    }

    /// Deal-level coverage tests as ``CoverageTestSpec`` serde dicts.
    #[getter]
    fn coverage_triggers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.coverage_triggers)
    }

    /// Clean-up call pool-factor threshold (decimal), or ``None``.
    #[getter]
    fn cleanup_call_decimal(&self) -> Option<f64> {
        self.inner.cleanup_call_decimal
    }

    /// Assumed optional redemption, or ``None``.
    #[getter]
    fn call_assumption(&self) -> Option<PyCallAssumption> {
        self.inner
            .call_assumption
            .clone()
            .map(|inner| PyCallAssumption { inner })
    }

    /// Collateral liquidation price in percent of par, or ``None`` for par.
    #[getter]
    fn liquidation_price_pct(&self) -> Option<f64> {
        self.inner.liquidation_price_pct
    }

    /// Explicit loss-allocation policy (``"write_down"`` /
    /// ``"par_preserving"``), or ``None`` for the deal-type default.
    #[getter]
    fn loss_allocation(&self) -> PyResult<Option<String>> {
        self.inner
            .loss_allocation
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Scheduled lender draws on notes as a list of ``TrancheDraw`` serde
    /// dicts (``tranche_id``, ``date``, ``amount``).
    #[getter]
    fn tranche_draws<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.tranche_draws)
    }

    /// Per-period re-advance rule as its ``TrancheReadvance`` serde dict
    /// (``tranche_id``, ``commitment``), or ``None``.
    #[getter]
    fn tranche_readvance<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .tranche_readvance
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Explicit loss-recognition timing (``"at_default"`` /
    /// ``"at_liquidation"``), or ``None`` for the deal-type default.
    #[getter]
    fn loss_recognition(&self) -> PyResult<Option<String>> {
        self.inner
            .loss_recognition
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Explicit principal-covers-senior-interest flag, or ``None`` for the
    /// deal-type default.
    #[getter]
    fn principal_covers_senior_interest(&self) -> Option<bool> {
        self.inner.principal_covers_senior_interest
    }

    /// Collateral valuation rules for the coverage tests, or ``None``.
    #[getter]
    fn coverage_rules(&self) -> Option<PyCoverageRules> {
        self.inner
            .coverage_rules
            .clone()
            .map(|inner| PyCoverageRules { inner })
    }

    /// Declarative waterfall rules as their serde ``dict``, or ``None``.
    #[getter]
    fn waterfall_rules<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .waterfall_rules
            .as_ref()
            .map(|rules| serde_to_py(py, rules))
            .transpose()
    }

    /// Custom priority of payments, or ``None`` when the template applies.
    #[getter]
    fn waterfall(&self) -> Option<PyWaterfall> {
        self.inner
            .waterfall
            .clone()
            .map(|inner| PyWaterfall { inner })
    }

    /// Canonical example deal: a USD 100M CLO with one 7% fixed-rate
    /// collateral bond and one 6% senior note, closing 2024-01-01, legal
    /// final 2034-01-01, discounted on ``USD-OIS`` with the ``nyse`` calendar
    /// (mirrors Rust ``StructuredCredit::example``).
    ///
    /// Returns
    /// -------
    /// StructuredCredit
    ///     The example deal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (does not occur for a released build).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import StructuredCredit
    /// >>> StructuredCredit.example().id
    /// 'CLO-EXAMPLE'
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        StructuredCredit::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Expiry date exposed by the ``Instrument`` trait, or ``None``.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    /// ``True`` when a stochastic prepayment, default or correlation spec is set
    /// (mirrors Rust ``StructuredCredit::is_stochastic``).
    #[getter]
    fn is_stochastic(&self) -> bool {
        self.inner.is_stochastic()
    }

    /// Return a copy with every stochastic specification cleared (mirrors
    /// Rust ``StructuredCredit::disable_stochastic``); the inverse of
    /// ``enable_stochastic``.
    ///
    /// Returns
    /// -------
    /// StructuredCredit
    ///     A new deal without the stochastic prepayment, default and
    ///     correlation specs (``stochastic_recovery_spec`` is kept).
    #[pyo3(text_signature = "($self)")]
    fn disable_stochastic(&self) -> Self {
        let mut deal = self.inner.clone();
        deal.disable_stochastic();
        Self { inner: deal }
    }

    /// Loss-allocation policy in force for pricing: ``loss_allocation`` when
    /// set, otherwise the deal-type convention (``"write_down"`` /
    /// ``"par_preserving"``).
    #[getter]
    fn effective_loss_allocation(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.effective_loss_allocation())
    }

    /// Loss-recognition timing in force: ``loss_recognition`` when set,
    /// otherwise the deal-type convention (``"at_liquidation"`` for RMBS and
    /// CMBS, ``"at_default"`` otherwise).
    #[getter]
    fn effective_loss_recognition(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.effective_loss_recognition())
    }

    /// Whether the template waterfall pays senior fees and senior note
    /// interest from principal when interest proceeds fall short:
    /// ``principal_covers_senior_interest`` when set, otherwise ``True`` for
    /// CLO/CBO and ``False`` for every other deal type.
    #[getter]
    fn effective_principal_covers_senior_interest(&self) -> bool {
        self.inner.effective_principal_covers_senior_interest()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "StructuredCredit(id={:?}, deal_type={:?})",
            self.inner.id.as_str(),
            self.inner.deal_type
        )
    }
}

/// Fluent builder for [`PyStructuredCredit`]; wraps the Rust
/// `FinancialBuilder`-generated builder (consuming setters).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "StructuredCreditBuilder",
    skip_from_py_object
)]
pub struct PyStructuredCreditBuilder {
    inner: Option<StructuredCreditBuilderInner>,
}

crate::bindings::valuations::pricing::pricing_override_methods!(
    PyStructuredCredit,
    PyStructuredCreditBuilder,
    "StructuredCreditBuilder",
    no_fields
);

#[pymethods]
impl PyStructuredCreditBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the deal.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the deal-type classification.
    ///
    /// Parameters
    /// ----------
    /// value : {"clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"}
    ///     Deal classification.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized deal type.
    #[pyo3(text_signature = "($self, value)")]
    fn deal_type<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let deal_type: DealType = enum_from_str(value, "deal_type")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .deal_type(deal_type))
    }

    /// Set the asset pool.
    ///
    /// Parameters
    /// ----------
    /// value : AssetPool
    ///     Asset pool definition.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn pool<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyAssetPool>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .pool(value.inner.clone()))
    }

    /// Set the tranche capital structure.
    ///
    /// Parameters
    /// ----------
    /// value : TrancheStructure
    ///     Tranche capital structure.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn tranches<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyTrancheStructure>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .tranches(value.inner.clone()))
    }

    /// Set the deal closing (issuance) date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date
    ///     Deal closing date.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn closing_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b.closing_date(date))
    }

    /// Set the first payment date to tranches.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date
    ///     First payment date.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn first_payment_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .first_payment_date(date))
    }

    /// Set buyer settlement for clean/dirty price, yield and spread metrics.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date
    ///     Buyer settlement date, on or after valuation and closing. Payments
    ///     on or before this date belong to the seller. When omitted, metrics
    ///     settle on the valuation date; model PV retains its valuation date.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     This builder for further configuration.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date is invalid or the builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn quote_settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .quote_settlement_date(date))
    }

    /// Set the legal final maturity date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date
    ///     Legal final maturity date.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b.maturity(date))
    }

    /// Set the payment frequency for the structure.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor | str
    ///     Payment frequency.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`, or a string ``value`` is not a recognized tenor string.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let frequency = crate::bindings::valuations::convert::tenor_from_py(value, "frequency")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .frequency(frequency))
    }

    /// Set the payment calendar identifier for schedule adjustments.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Holiday calendar identifier (e.g. ``"nyse"``). Required for
    ///     accurate schedule generation.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .calendar_id(value.into()))
    }

    /// Set the business day convention for tranche payments.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Business day convention (e.g. ``"following"``,
    ///     ``"modified_following"``). Defaults to ``"following"`` when
    ///     never set.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized business day convention.
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let business_day_convention: BusinessDayConvention =
            crate::bindings::valuations::convert::bdc_from_str(value, "business_day_convention")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .business_day_convention(business_day_convention))
    }

    /// Set the discount curve identifier for valuation.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Discount curve identifier.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .discount_curve_id(CurveId::new(value.to_string())))
    }

    /// Set market conditions from a JSON object.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``MarketConditions`` object containing finite annual decimal ``refi_rate``
    ///     for Richard-Roll refinancing incentives; negative rates are accepted.
    ///     This replaces the registry default. Unknown macro-factor fields fail.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``MarketConditions`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn market_conditions<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let market_conditions: MarketConditions =
            crate::bindings::module_utils::py_to_serde(py, value, "market_conditions")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .market_conditions(market_conditions))
    }

    /// Set declarative waterfall rules from a JSON object.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``WaterfallRules`` object as a dict or JSON string (available-funds caps,
    ///     step-down, shifting interest, controlled accumulation), layered
    ///     onto the base waterfall.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``WaterfallRules`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn waterfall_rules<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let waterfall_rules: WaterfallRules =
            crate::bindings::module_utils::py_to_serde(py, value, "waterfall_rules")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .waterfall_rules(waterfall_rules))
    }

    /// Set senior transaction fees from a JSON object.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``DealFees`` object as a dict or JSON string (trustee, senior management,
    ///     servicing, and optional master/special servicer fees), paid
    ///     ahead of every note. Optional ``workout_fee_pct`` (percent of the
    ///     P&I collected on specially serviced loans) and
    ///     ``liquidation_fee_pct`` (percent of liquidation proceeds) are taken
    ///     inside the collateral flows. Skipped (``None``) by default.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``DealFees`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn fees<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees: DealFees = crate::bindings::module_utils::py_to_serde(py, value, "fees")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b.fees(fees))
    }

    /// Replace the whole credit model (prepayment, default, recovery,
    /// stochastic and correlation specs, delinquency and card models).
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``CreditModelConfig`` serde object. Later per-field setters
    ///     (:meth:`prepayment_spec` ...) modify this model.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn credit_model<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: CreditModelConfig =
            crate::bindings::module_utils::py_to_serde(py, value, "credit_model")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .credit_model(converted))
    }

    /// Set the deterministic prepayment model.
    ///
    /// Parameters
    /// ----------
    /// value : PrepaymentModelSpec | dict | str
    ///     Typed spec (``PrepaymentModelSpec.constant_cpr`` / ``psa`` /
    ///     ``abs`` / ``vector`` / ``cmbs_with_lockout``) or its serde form.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn prepayment_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: PrepaymentModelSpec =
            if let Ok(typed) = value.cast::<PyPrepaymentModelSpec>() {
                typed.borrow().inner.clone()
            } else {
                crate::bindings::module_utils::py_to_serde(py, value, "prepayment_spec")?
            };
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .prepayment_spec(converted))
    }

    /// Set the deterministic default model.
    ///
    /// Parameters
    /// ----------
    /// value : DefaultModelSpec | dict | str
    ///     Typed spec (``DefaultModelSpec.constant_cdr`` / ``sda`` / ``vector``
    ///     / ``cumulative_loss`` / ``timing``) or its serde form.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn default_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: DefaultModelSpec = if let Ok(typed) = value.cast::<PyDefaultModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "default_spec")?
        };
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .default_spec(converted))
    }

    /// Set the recovery model.
    ///
    /// Parameters
    /// ----------
    /// value : RecoveryModelSpec | dict | str
    ///     Typed spec (rate, lag, optional severity vector) or its serde form.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: RecoveryModelSpec = if let Ok(typed) = value.cast::<PyRecoveryModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "recovery_spec")?
        };
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .recovery_spec(converted))
    }

    /// Set the stochastic prepayment specification used by
    /// ``price_stochastic``.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``StochasticPrepaySpec`` serde object (Richard-Roll or factor
    ///     parameters).
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn stochastic_prepay_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::module_utils::py_to_serde(py, value, "stochastic_prepay_spec")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .stochastic_prepay_spec(converted))
    }

    /// Set the stochastic default specification used by
    /// ``price_stochastic``.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``StochasticDefaultSpec`` serde object.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn stochastic_default_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::module_utils::py_to_serde(py, value, "stochastic_default_spec")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .stochastic_default_spec(converted))
    }

    /// Set the stochastic recovery specification used by
    /// ``price_stochastic``.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``RecoverySpec`` serde object: ``{"type": "constant", "rate":
    ///     0.4}`` or ``{"type": "market_correlated", "mean_recovery": 0.4,
    ///     "recovery_volatility": 0.25, "factor_correlation": 0.4}`` (recovery
    ///     falls with the systematic factor, so heavy-default paths recover
    ///     less).
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``RecoverySpec`` shape or this
    ///     builder was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn stochastic_recovery_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::module_utils::py_to_serde(py, value, "stochastic_recovery_spec")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .stochastic_recovery_spec(converted))
    }

    /// Set the default correlation structure used by ``price_stochastic``.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``CorrelationStructure`` serde object (factor loadings).
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn correlation_structure<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::module_utils::py_to_serde(py, value, "correlation_structure")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .correlation_structure(converted))
    }

    /// Set the delinquency roll-rate, advancing and modification model
    /// (ABS/RMBS asset and rep-line pools).
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``DelinquencyModel`` serde object: ``roll_rates`` (per bucket,
    ///     the last rolls to charge-off), ``cure_rates``, ``advancing``
    ///     (``{"policy": "none"}`` or ``{"policy": "principal_and_interest",
    ///     "recoverability_cap_pct": ..., "reimburse_from_collections":
    ///     false}``) and optional ``modification``.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn delinquency<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::DelinquencyModel =
            crate::bindings::module_utils::py_to_serde(py, value, "delinquency")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .delinquency(converted))
    }

    /// Set the card master-trust portfolio model.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``CardPortfolioSpec`` serde object: ``monthly_payment_rate``,
    ///     ``portfolio_yield`` and ``charge_off_rate`` (annual decimals),
    ///     plus the optional ``seller_interest`` (``Money`` serde object in
    ///     the pool currency) and ``fixed_allocation_decimal`` (decimal in
    ///     ``(0, 1]``) that fix the investor allocation of trust collections
    ///     once the revolving period ends.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn card<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::CardPortfolioSpec =
            crate::bindings::module_utils::py_to_serde(py, value, "card")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b.card(converted))
    }

    /// Set the deal-level OC / IC coverage tests.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict] | str
    ///     ``CoverageTestSpec`` objects (``id``, ``tranche_id``, ``kind`` (``"oc"`` / ``"ic"``),
    ///     ``trigger_level`` ratio, ``action``, optional ``placement``
    ///     (``{"kind": "after_tranche", "tranche_id": ...}`` or
    ///     ``{"kind": "after_junior_fees"}``), optional ``divert_pct`` and
    ///     ``include_cash``).
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn coverage_triggers<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: Vec<finstack_quant_valuations::instruments::fixed_income::structured_credit::CoverageTestSpec> =
            crate::bindings::module_utils::py_to_serde(py, value, "coverage_triggers")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .coverage_triggers(converted))
    }

    /// Set the collateral valuation rules for the coverage tests.
    ///
    /// Parameters
    /// ----------
    /// value : CoverageRules | dict | str
    ///     Typed :class:`CoverageRules` or its serde form.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn coverage_rules<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::CoverageRules = if let Ok(typed) = value.cast::<PyCoverageRules>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "coverage_rules")?
        };
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .coverage_rules(converted))
    }

    /// Set the assumed optional redemption for price-to-call analytics.
    ///
    /// Parameters
    /// ----------
    /// value : CallAssumption | dict | str
    ///     Typed :class:`CallAssumption` or its serde form (``date``,
    ///     ``price_pct``, ``scope``).
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn call_assumption<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::CallAssumption = if let Ok(typed) = value.cast::<PyCallAssumption>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "call_assumption")?
        };
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .call_assumption(converted))
    }

    /// Set a custom priority of payments in place of the deal-type template.
    ///
    /// Parameters
    /// ----------
    /// value : Waterfall | dict | str
    ///     Typed :class:`Waterfall` or its serde form (``tiers``,
    ///     ``currency``, optional ``coverage_rules``).
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn waterfall<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::Waterfall = if let Ok(typed) = value.cast::<PyWaterfall>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "waterfall")?
        };
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .waterfall(converted))
    }

    /// Set the interest-rate hedges settled through the waterfall.
    ///
    /// Parameters
    /// ----------
    /// value : list[HedgeSwap | dict] | str
    ///     Typed :class:`HedgeSwap` objects, their serde dicts, or a JSON
    ///     array string.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn hedge_swaps<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let hedges = hedge_swaps_from_py(py, value)?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b.hedge_swaps(hedges))
    }

    /// Set the clean-up call pool-factor threshold.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Pool factor (decimal in ``(0, 1)``, typically ``0.10``) below
    ///     which the deal is redeemed when the liquidation proceeds cover
    ///     the notes.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn cleanup_call_decimal<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .cleanup_call_decimal(value))
    }

    /// Set the collateral liquidation price used by deal calls and clean-up
    /// calls.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Percent of par the collateral realizes (``100.0`` = par).
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn liquidation_price_pct<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .liquidation_price_pct(value))
    }

    /// Set how collateral losses reach the note balances.
    ///
    /// Parameters
    /// ----------
    /// value : {"write_down", "par_preserving"}
    ///     ``"write_down"`` allocates realized losses junior-first at
    ///     default (RMBS/CMBS convention); ``"par_preserving"`` keeps note
    ///     balances at par and realizes shortfalls at legal final
    ///     (CLO/ABS convention). The deal type's default applies when never
    ///     set.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn loss_allocation<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let policy: LossAllocationPolicy = enum_from_str(value, "loss_allocation")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .loss_allocation(policy))
    }

    /// Set the scheduled lender draws on notes after closing.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict] | str
    ///     ``TrancheDraw`` serde objects ``{"tranche_id": "A", "date":
    ///     "2025-01-01", "amount": {"amount": 5000000.0, "currency":
    ///     "USD"}}``, ascending by date; each is applied on the first payment
    ///     date at or after its date, lifting the note's balance and adding
    ///     the cash to principal proceeds.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``TrancheDraw`` shape or this
    ///     builder was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn tranche_draws<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let draws: Vec<TrancheDraw> =
            crate::bindings::module_utils::py_to_serde(py, value, "tranche_draws")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .tranche_draws(draws))
    }

    /// Set the per-period re-advance of one note up to its commitment and
    /// the borrowing base while the deal revolves.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``TrancheReadvance`` serde object ``{"tranche_id": "A",
    ///     "commitment": {"amount": 70000000.0, "currency": "USD"}}``;
    ///     requires ``coverage_rules.borrowing_base``.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``TrancheReadvance`` shape or this
    ///     builder was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn tranche_readvance<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: TrancheReadvance =
            crate::bindings::module_utils::py_to_serde(py, value, "tranche_readvance")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .tranche_readvance(spec))
    }

    /// Set when collateral losses are booked.
    ///
    /// Parameters
    /// ----------
    /// value : {"at_default", "at_liquidation"}
    ///     ``"at_default"`` books the expected net loss on the default date
    ///     (CLO/ABS convention); ``"at_liquidation"`` books the realized
    ///     loss when the claim settles after the recovery lag (RMBS/CMBS
    ///     convention), which delays write-downs and every cumulative-loss
    ///     trigger. The deal type's default applies when never set.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn loss_recognition<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let timing: LossRecognition = enum_from_str(value, "loss_recognition")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .loss_recognition(timing))
    }

    /// Set whether principal proceeds cover senior fees and senior interest
    /// shortfalls before any note is redeemed.
    ///
    /// Parameters
    /// ----------
    /// value : bool
    ///     ``True`` for the CLO principal-waterfall convention, ``False``
    ///     for strictly separate accounts. The deal type's default applies
    ///     when never set.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn principal_covers_senior_interest<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .principal_covers_senior_interest(value))
    }

    /// Set deal metadata (counterparties, identifiers).
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``Metadata`` serde object.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the expected shape or this builder
    ///     was already consumed by :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn deal_metadata<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let metadata: Metadata =
            crate::bindings::module_utils::py_to_serde(py, value, "deal_metadata")?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .deal_metadata(metadata))
    }

    /// Set free-form attributes (tags and metadata) on the deal.
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict | None
    ///     Attribute bag: an ``Attributes`` or its serde ``dict`` form
    ///     (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
    ///     ``None`` clears it.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
    /// ValueError
    ///     If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or
    ///     value, or this builder was already consumed by
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        builder_set!(slf, |b: StructuredCreditBuilderInner| b
            .attributes(attributes))
    }

    /// Build the validated structured-credit deal.
    ///
    /// Returns
    /// -------
    /// StructuredCredit
    ///     The validated deal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed deal fails pricing validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyStructuredCredit> {
        let b = crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyStructuredCredit { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "StructuredCreditBuilder(consumed={})",
            bool_repr(self.inner.is_none())
        )
    }
}
