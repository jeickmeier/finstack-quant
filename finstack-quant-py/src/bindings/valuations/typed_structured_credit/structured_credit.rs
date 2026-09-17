use pyo3::prelude::*;

use crate::bindings::cashflows::builder::specs::{
    PyDefaultModelSpec, PyPrepaymentModelSpec, PyRecoveryModelSpec,
};
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{
    attributes_from_py, attributes_to_py, bool_repr, enum_to_py_string,
};
use crate::errors::{core_to_py, value_error};
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_equity_metrics, run_simulation, run_simulation_with_diagnostics, CreditFactors,
    CreditModelConfig, DealFees, DealType, LossAllocationPolicy, MarketConditions, Metadata,
    Overrides, PricingMode, StructuredCredit, WaterfallRules,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};

use super::super::instruments::{
    enum_from_str, parse_typed_instrument_json, serialize_typed_instrument_json,
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

#[pymethods]
impl PyStructuredCredit {
    /// Create a fluent builder (mirrors Rust ``StructuredCredit::builder()``).
    ///
    /// The builder pre-seeds ``market_conditions``, ``credit_factors``,
    /// ``deal_metadata``, ``behavior_overrides``,
    /// and ``hedge_swaps`` with their Rust ``Default`` values (the Rust
    /// builder fields have no default), which the corresponding setters
    /// (``market_conditions``, ``credit_factors``, ``waterfall_rules``,
    /// ``fees``, ``credit_model``, ``behavior_overrides``, ``hedge_swaps``
    /// ...) can override with typed objects, dicts or JSON strings. Builders are
    /// consumed by ``build()``; create a new builder per instrument. Prefer :meth:`new_abs` / :meth:`new_clo` /
    /// :meth:`new_cmbs` / :meth:`new_rmbs` for registry-calibrated deal-type
    /// defaults; use this builder for full manual control.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import StructuredCredit
    /// >>> builder = StructuredCredit.builder()
    /// >>> builder.id("EXAMPLE") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyStructuredCreditBuilder {
        PyStructuredCreditBuilder {
            inner: Some(
                StructuredCredit::builder()
                    .market_conditions(MarketConditions::default())
                    .credit_factors(CreditFactors::default())
                    .deal_metadata(Metadata::default())
                    .behavior_overrides(Overrides::default())
                    .hedge_swaps(Vec::new()),
            ),
            credit_model: None,
        }
    }

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
    /// payment_calendar_id : str, optional
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
    ///     If the deal fails pricing validation.
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
    /// ...         datetime.date(2031, 1, 15), 12, DayCount.ACT_360, asset_type={"type": "first_lien_loan", "industry": None},
    /// ...     )
    /// ... ])
    /// >>> senior = (
    /// ...     Tranche.builder().id("A").attachment_point(10.0).detachment_point(100.0)
    /// ...     .seniority("senior").original_balance(Money(72_000_000.0, Currency("USD")))
    /// ...     .coupon_fixed(0.05).maturity(datetime.date(2031, 1, 15)).build()
    /// ... )
    /// >>> equity = (
    /// ...     Tranche.builder().id("E").attachment_point(0.0).detachment_point(10.0)
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
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, payment_calendar_id=None))]
    #[pyo3(
        text_signature = "(id, pool, tranches, closing_date, maturity, discount_curve_id, payment_calendar_id=None)"
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
        payment_calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = StructuredCredit::new_abs(
            id,
            pool.inner.clone(),
            tranches.inner.clone(),
            extract_date(closing_date)?,
            extract_date(maturity)?,
            discount_curve_id,
        );
        if let Some(calendar_id) = payment_calendar_id {
            inner = inner.with_payment_calendar(calendar_id);
        }
        inner.validate_for_pricing().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a new CLO deal with registry-calibrated defaults.
    ///
    /// See :meth:`new_abs` for parameter and return documentation; the
    /// signature is identical, only the deal-type calibration differs.
    #[staticmethod]
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, payment_calendar_id=None))]
    #[pyo3(
        text_signature = "(id, pool, tranches, closing_date, maturity, discount_curve_id, payment_calendar_id=None)"
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
        payment_calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = StructuredCredit::new_clo(
            id,
            pool.inner.clone(),
            tranches.inner.clone(),
            extract_date(closing_date)?,
            extract_date(maturity)?,
            discount_curve_id,
        );
        if let Some(calendar_id) = payment_calendar_id {
            inner = inner.with_payment_calendar(calendar_id);
        }
        inner.validate_for_pricing().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a new CMBS deal with registry-calibrated defaults.
    ///
    /// See :meth:`new_abs` for parameter and return documentation; the
    /// signature is identical, only the deal-type calibration differs.
    #[staticmethod]
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, payment_calendar_id=None))]
    #[pyo3(
        text_signature = "(id, pool, tranches, closing_date, maturity, discount_curve_id, payment_calendar_id=None)"
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
        payment_calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = StructuredCredit::new_cmbs(
            id,
            pool.inner.clone(),
            tranches.inner.clone(),
            extract_date(closing_date)?,
            extract_date(maturity)?,
            discount_curve_id,
        );
        if let Some(calendar_id) = payment_calendar_id {
            inner = inner.with_payment_calendar(calendar_id);
        }
        inner.validate_for_pricing().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a new RMBS deal with registry-calibrated defaults.
    ///
    /// See :meth:`new_abs` for parameter and return documentation; the
    /// signature is identical, only the deal-type calibration differs.
    #[staticmethod]
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, payment_calendar_id=None))]
    #[pyo3(
        text_signature = "(id, pool, tranches, closing_date, maturity, discount_curve_id, payment_calendar_id=None)"
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
        payment_calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = StructuredCredit::new_rmbs(
            id,
            pool.inner.clone(),
            tranches.inner.clone(),
            extract_date(closing_date)?,
            extract_date(maturity)?,
            discount_curve_id,
        );
        if let Some(calendar_id) = payment_calendar_id {
            inner = inner.with_payment_calendar(calendar_id);
        }
        inner.validate_for_pricing().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a validated deal from its canonical v1 envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     A ``finstack_quant.instrument/1`` envelope containing an exact
    ///     ``"structured_credit"`` payload. The UTF-8 input must not exceed
    ///     16 MiB. Bare payloads and cross-type coercion are rejected.
    ///
    /// Returns
    /// -------
    /// StructuredCredit
    ///     The validated deal represented by the exact ``"structured_credit"`` payload.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the input exceeds 16 MiB, is malformed, has an unsupported
    ///     envelope schema, carries another type, or fails structured-credit
    ///     validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import StructuredCredit
    /// >>> try:
    /// ...     StructuredCredit.from_json("{}")
    /// ... except ValueError as exc:
    /// ...     print("schema" in str(exc))
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        match parse_typed_instrument_json(json)? {
            InstrumentJson::StructuredCredit(inner) => {
                let inner = *inner;
                Ok(Self { inner })
            }
            _ => Err(value_error(
                "expected instrument type \"structured_credit\", got a different instrument type",
            )),
        }
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
    ///     Number of Monte Carlo paths; defaults to the deal's configured
    ///     ``mc_paths`` override or 10,000.
    /// antithetic : bool, default True
    ///     Use antithetic variates (pairs share random numbers).
    ///
    /// Returns
    /// -------
    /// StochasticPricingResult
    ///     Deal and tranche present values, loss statistics, Monte Carlo
    ///     error, draw diagnostics and the draw option cost.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the deal fails validation or ``num_paths`` is zero.
    /// KeyError
    ///     If a required curve is missing from ``market``.
    /// RuntimeError
    ///     If the simulation fails.
    #[pyo3(signature = (market, as_of, num_paths=None, antithetic=true))]
    #[pyo3(text_signature = "($self, market, as_of, num_paths=None, antithetic=True)")]
    fn price_stochastic(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        num_paths: Option<usize>,
        antithetic: bool,
    ) -> PyResult<PyStochasticPricingResult> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        let deal = self.inner.clone();
        let num_paths = num_paths.unwrap_or_else(|| {
            deal.instrument_pricing_overrides
                .model_config
                .mc_paths
                .unwrap_or(10_000)
        });
        let mode = PricingMode::MonteCarlo {
            num_paths,
            antithetic,
        };
        let inner = py
            .detach(move || deal.price_stochastic_with_mode(&market, as_of, mode))
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
        let as_of = extract_date(as_of)?;
        let deal = self.inner.clone();
        let run = py
            .detach(move || run_simulation_with_diagnostics(&deal, &market, as_of))
            .map_err(core_to_py)?;
        Ok(PySimulationDiagnostics {
            inner: run.diagnostics,
        })
    }

    /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
    ///
    /// Returns
    /// -------
    /// str
    ///     Canonical instrument envelope accepted by ``price_instrument`` and
    ///     ``StructuredCredit.from_json``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        self.envelope_json()
    }

    /// Instrument identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
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
    fn enable_stochastic_defaults(&self) -> PyResult<Self> {
        let mut deal = self.inner.clone();
        deal.enable_stochastic_defaults().map_err(core_to_py)?;
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
        let as_of = extract_date(as_of)?;
        let deal = self.inner.clone();
        let mut flows = py
            .detach(move || run_simulation(&deal, &market, as_of))
            .map_err(core_to_py)?;
        let inner = flows.remove(tranche_id).ok_or_else(|| {
            pyo3::exceptions::PyKeyError::new_err(format!(
                "tranche {tranche_id:?} is not a class of deal {:?}",
                self.inner.id.as_str()
            ))
        })?;
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
        let as_of = extract_date(as_of)?;
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
    fn payment_calendar_id(&self) -> Option<String> {
        self.inner.payment_calendar_id.clone()
    }

    /// Payment business-day convention string, or ``None`` for the default.
    #[getter]
    fn payment_business_day_convention(&self) -> PyResult<Option<String>> {
        self.inner
            .payment_business_day_convention
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

    /// Credit factors as their serde ``dict``.
    #[getter]
    fn credit_factors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_factors)
    }

    /// Deal metadata as its serde ``dict``.
    #[getter]
    fn deal_metadata<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.deal_metadata)
    }

    /// Behavioural overrides as their serde ``dict``.
    #[getter]
    fn behavior_overrides<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.behavior_overrides)
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
    fn cleanup_call_pct(&self) -> Option<f64> {
        self.inner.cleanup_call_pct
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

    /// Free-form attributes (tags and metadata).
    #[getter]
    fn attributes(&self) -> crate::bindings::core::types::PyAttributes {
        attributes_to_py(&self.inner.attributes)
    }

    /// Return the full deal as a plain ``dict`` (canonical serde shape).
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
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
    /// Credit model assembled by the per-field setters; applied on ``build``.
    credit_model: Option<CreditModelConfig>,
}

/// Take the wrapped Rust builder or fail if `build()` already consumed it.
fn take_sc(b: &mut PyStructuredCreditBuilder) -> PyResult<StructuredCreditBuilderInner> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.id(InstrumentId::new(value.to_string())));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.deal_type(deal_type));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.pool(value.inner.clone()));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.tranches(value.inner.clone()));
        Ok(slf)
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
        let date = extract_date(value)?;
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.closing_date(date));
        Ok(slf)
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
        let date = extract_date(value)?;
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.first_payment_date(date));
        Ok(slf)
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
        let date = extract_date(value)?;
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.quote_settlement_date(date));
        Ok(slf)
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
        let date = extract_date(value)?;
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.maturity(date));
        Ok(slf)
    }

    /// Set the payment frequency for the structure.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor
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
    ///     :meth:`StructuredCreditBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyTenor>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.frequency(value.inner));
        Ok(slf)
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
    fn payment_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.payment_calendar_id(value.to_string()));
        Ok(slf)
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
    fn payment_business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let business_day_convention: BusinessDayConvention =
            enum_from_str(value, "payment_business_day_convention")?;
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.payment_business_day_convention(business_day_convention));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.discount_curve_id(CurveId::new(value.to_string())));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.market_conditions(market_conditions));
        Ok(slf)
    }

    /// Set credit factors from a JSON object.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``CreditFactors`` object with optional ``annual_noi`` and
    ///     ``annual_debt_service`` Money values for CMBS coverage metrics.
    ///     Unknown macro-factor fields fail; missing values remain absent.
    ///
    /// Returns
    /// -------
    /// StructuredCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``CreditFactors`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn credit_factors<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let credit_factors: CreditFactors =
            crate::bindings::module_utils::py_to_serde(py, value, "credit_factors")?;
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.credit_factors(credit_factors));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.waterfall_rules(waterfall_rules));
        Ok(slf)
    }

    /// Set senior transaction fees from a JSON object.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``DealFees`` object as a dict or JSON string (trustee, senior management,
    ///     servicing, and optional master/special servicer fees), paid
    ///     ahead of every note. Skipped (``None``) by default.
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.fees(fees));
        Ok(slf)
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model = Some(converted);
        Ok(slf)
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .prepayment_spec = converted;
        Ok(slf)
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .default_spec = converted;
        Ok(slf)
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .recovery_spec = converted;
        Ok(slf)
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .stochastic_prepay_spec = Some(converted);
        Ok(slf)
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .stochastic_default_spec = Some(converted);
        Ok(slf)
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .correlation_structure = Some(converted);
        Ok(slf)
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
    ///     "recoverability_cap_pct": ...}``) and optional ``modification``.
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .delinquency = Some(converted);
        Ok(slf)
    }

    /// Set the card master-trust portfolio model.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``CardPortfolioSpec`` serde object: ``monthly_payment_rate``,
    ///     ``portfolio_yield`` and ``charge_off_rate`` (annual decimals).
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
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .card = Some(converted);
        Ok(slf)
    }

    /// Set behavioural assumption overrides.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``Overrides`` serde object (``cpr_annual``, ``psa_speed_multiplier``,
    ///     ``cdr_annual``, ``sda_speed_multiplier``, ``recovery_rate``,
    ///     ``recovery_lag_months``, ``reinvestment_price`` ...).
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
    fn behavior_overrides<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: Overrides =
            crate::bindings::module_utils::py_to_serde(py, value, "behavior_overrides")?;
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.behavior_overrides(converted));
        Ok(slf)
    }

    /// Set the deal-level OC / IC coverage tests.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict] | str
    ///     ``CoverageTestSpec`` objects (``id``, ``tranche_id``, ``kind`` (``"oc"`` / ``"ic"``),
    ///     ``trigger_level`` ratio, ``action``, optional ``after_tranche`` and
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.coverage_triggers(converted));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.coverage_rules(converted));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.call_assumption(converted));
        Ok(slf)
    }

    /// Set a custom priority of payments in place of the deal-type template.
    ///
    /// Parameters
    /// ----------
    /// value : Waterfall | dict | str
    ///     Typed :class:`Waterfall` or its serde form (``tiers``,
    ///     ``base_currency``, optional ``coverage_rules``).
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.waterfall(converted));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.hedge_swaps(hedges));
        Ok(slf)
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
    fn cleanup_call_pct<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.cleanup_call_pct(value));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.liquidation_price_pct(value));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.loss_allocation(policy));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.principal_covers_senior_interest(value));
        Ok(slf)
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
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.deal_metadata(metadata));
        Ok(slf)
    }

    /// Set free-form attributes (tags and metadata) on the deal.
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str]
    ///     Attribute bag; a dict populates ``meta`` (an optional ``"tags"``
    ///     list populates ``tags``).
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
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        let b = take_sc(&mut slf)?;
        slf.inner = Some(b.attributes(attributes));
        Ok(slf)
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
        let mut b = take_sc(&mut slf)?;
        if let Some(credit_model) = slf.credit_model.take() {
            b = b.credit_model(credit_model);
        }
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
