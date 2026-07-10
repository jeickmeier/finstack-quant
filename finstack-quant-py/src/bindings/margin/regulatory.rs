//! Python bindings for FRTB SBA and SA-CCR regulatory capital frameworks.
//!
//! Exposes a deliberately simplified surface: callers build sensitivity or
//! trade containers with ergonomic `add_*` methods, then invoke a free function
//! that returns the headline capital number plus a per-component breakdown.
//! Full typed access to every enum variant is intentionally omitted; where
//! complex configuration is needed, JSON round-tripping is used.

use crate::errors::{core_to_py, display_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_margin::regulatory::{
    frtb::{CorrelationScenario, FrtbRiskClass, FrtbSbaEngine, FrtbSensitivities, RraoPosition},
    sa_ccr::{SaCcrAssetClass, SaCcrEngine, SaCcrNettingSetConfig, SaCcrTrade},
};
use finstack_quant_margin::NettingSetId;
use pyo3::prelude::*;
use pyo3::types::PyDict;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_currency(code: &str) -> PyResult<Currency> {
    code.parse::<Currency>().map_err(display_to_py)
}

fn parse_correlation_scenario(s: &str) -> PyResult<CorrelationScenario> {
    match s.to_ascii_lowercase().as_str() {
        "low" => Ok(CorrelationScenario::Low),
        "medium" | "med" | "base" => Ok(CorrelationScenario::Medium),
        "high" => Ok(CorrelationScenario::High),
        other => Err(crate::errors::value_error(format!(
            "unknown FRTB correlation scenario '{other}' (expected 'low', 'medium', or 'high')"
        ))),
    }
}

fn risk_class_label(rc: FrtbRiskClass) -> &'static str {
    match rc {
        FrtbRiskClass::Girr => "GIRR",
        FrtbRiskClass::CsrNonSec => "CSR_NON_SEC",
        FrtbRiskClass::CsrSecCtp => "CSR_SEC_CTP",
        FrtbRiskClass::CsrSecNonCtp => "CSR_SEC_NON_CTP",
        FrtbRiskClass::Equity => "EQUITY",
        FrtbRiskClass::Commodity => "COMMODITY",
        FrtbRiskClass::Fx => "FX",
        _ => "UNKNOWN",
    }
}

fn asset_class_label(ac: SaCcrAssetClass) -> &'static str {
    match ac {
        SaCcrAssetClass::InterestRate => "INTEREST_RATE",
        SaCcrAssetClass::ForeignExchange => "FOREIGN_EXCHANGE",
        SaCcrAssetClass::Credit => "CREDIT",
        SaCcrAssetClass::Equity => "EQUITY",
        SaCcrAssetClass::Commodity => "COMMODITY",
        _ => "UNKNOWN",
    }
}

fn parse_asset_class(s: &str) -> PyResult<SaCcrAssetClass> {
    match s.to_ascii_lowercase().as_str() {
        "ir" | "interest_rate" | "interestrate" | "rates" => Ok(SaCcrAssetClass::InterestRate),
        "fx" | "foreign_exchange" | "foreignexchange" => Ok(SaCcrAssetClass::ForeignExchange),
        "credit" | "cr" => Ok(SaCcrAssetClass::Credit),
        "equity" | "eq" => Ok(SaCcrAssetClass::Equity),
        "commodity" | "comm" | "co" => Ok(SaCcrAssetClass::Commodity),
        other => Err(crate::errors::value_error(format!(
            "unknown SA-CCR asset class '{other}' (expected ir/fx/credit/equity/commodity)"
        ))),
    }
}

fn parse_date(year: i32, month: u8, day: u8) -> PyResult<finstack_quant_core::dates::Date> {
    let m = time::Month::try_from(month)
        .map_err(|e| crate::errors::value_error(format!("invalid month: {e}")))?;
    finstack_quant_core::dates::Date::from_calendar_date(year, m, day)
        .map_err(|e| crate::errors::value_error(format!("invalid date: {e}")))
}

// ---------------------------------------------------------------------------
// FrtbSensitivities wrapper
// ---------------------------------------------------------------------------

/// FRTB sensitivity portfolio for the Sensitivity-Based Approach.
///
/// Build up delta/vega/curvature inputs with the ``add_*`` methods, then pass
/// to :func:`frtb_sba_charge`. JSON round-tripping is available for advanced
/// use cases (e.g. loading a full portfolio produced by an upstream tool).
#[pyclass(
    name = "FrtbSensitivities",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFrtbSensitivities {
    pub(super) inner: FrtbSensitivities,
}

#[pymethods]
impl PyFrtbSensitivities {
    /// Create an empty sensitivity container.
    ///
    /// ``base_currency`` is the reporting currency (e.g. ``"USD"``).
    #[new]
    #[pyo3(signature = (base_currency = "USD"))]
    fn new(base_currency: &str) -> PyResult<Self> {
        let ccy = parse_currency(base_currency)?;
        Ok(Self {
            inner: FrtbSensitivities::new(ccy),
        })
    }

    /// Construct from a JSON serialization of `FrtbSensitivities`.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: FrtbSensitivities = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    /// Add a GIRR delta sensitivity (currency P&L per 1 percentage-point move).
    #[pyo3(signature = (tenor, amount, currency = None))]
    fn add_girr_delta(&mut self, tenor: &str, amount: f64, currency: Option<&str>) -> PyResult<()> {
        let ccy = self.currency_or_base(currency)?;
        self.inner.add_girr_delta(ccy, tenor, amount);
        Ok(())
    }

    /// Add a CSR (non-securitization) delta sensitivity.
    #[pyo3(signature = (issuer, bucket, tenor, amount))]
    fn add_csr_delta(&mut self, issuer: &str, bucket: u8, tenor: &str, amount: f64) {
        self.inner
            .add_csr_nonsec_delta(issuer, bucket, tenor, amount);
    }

    /// Add an equity delta sensitivity.
    #[pyo3(signature = (underlier, bucket, amount))]
    fn add_equity_delta(&mut self, underlier: &str, bucket: u8, amount: f64) {
        self.inner.add_equity_delta(underlier, bucket, amount);
    }

    /// Add an FX delta sensitivity for the pair ``(ccy1, ccy2)``.
    #[pyo3(signature = (ccy1, ccy2, amount))]
    fn add_fx_delta(&mut self, ccy1: &str, ccy2: &str, amount: f64) -> PyResult<()> {
        let c1 = parse_currency(ccy1)?;
        let c2 = parse_currency(ccy2)?;
        self.inner.add_fx_delta(c1, c2, amount);
        Ok(())
    }

    /// Add a commodity delta sensitivity.
    #[pyo3(signature = (name, bucket, tenor, amount))]
    fn add_commodity_delta(&mut self, name: &str, bucket: u8, tenor: &str, amount: f64) {
        self.inner.add_commodity_delta(name, bucket, tenor, amount);
    }

    /// Add a GIRR vega sensitivity.
    #[pyo3(signature = (option_maturity, underlying_tenor, amount, currency = None))]
    fn add_girr_vega(
        &mut self,
        option_maturity: &str,
        underlying_tenor: &str,
        amount: f64,
        currency: Option<&str>,
    ) -> PyResult<()> {
        let ccy = self.currency_or_base(currency)?;
        self.inner
            .add_girr_vega(ccy, option_maturity, underlying_tenor, amount);
        Ok(())
    }

    /// Add an equity vega sensitivity.
    #[pyo3(signature = (underlier, bucket, maturity, amount))]
    fn add_equity_vega(&mut self, underlier: &str, bucket: u8, maturity: &str, amount: f64) {
        self.inner
            .add_equity_vega(underlier, bucket, maturity, amount);
    }

    /// Add an FX vega sensitivity.
    #[pyo3(signature = (ccy1, ccy2, maturity, amount))]
    fn add_fx_vega(&mut self, ccy1: &str, ccy2: &str, maturity: &str, amount: f64) -> PyResult<()> {
        let c1 = parse_currency(ccy1)?;
        let c2 = parse_currency(ccy2)?;
        self.inner.add_fx_vega(c1, c2, maturity, amount);
        Ok(())
    }

    /// Add a GIRR curvature sensitivity (CVR up and CVR down).
    #[pyo3(signature = (cvr_up, cvr_down, currency = None))]
    fn add_girr_curvature(
        &mut self,
        cvr_up: f64,
        cvr_down: f64,
        currency: Option<&str>,
    ) -> PyResult<()> {
        let ccy = self.currency_or_base(currency)?;
        self.inner.add_girr_curvature(ccy, cvr_up, cvr_down);
        Ok(())
    }

    /// Add an equity curvature sensitivity (CVR up and CVR down).
    #[pyo3(signature = (underlier, bucket, cvr_up, cvr_down))]
    fn add_equity_curvature(&mut self, underlier: &str, bucket: u8, cvr_up: f64, cvr_down: f64) {
        self.inner
            .add_equity_curvature(underlier, bucket, cvr_up, cvr_down);
    }

    /// Add an FX curvature sensitivity.
    #[pyo3(signature = (ccy1, ccy2, cvr_up, cvr_down))]
    fn add_fx_curvature(
        &mut self,
        ccy1: &str,
        ccy2: &str,
        cvr_up: f64,
        cvr_down: f64,
    ) -> PyResult<()> {
        let c1 = parse_currency(ccy1)?;
        let c2 = parse_currency(ccy2)?;
        self.inner.add_fx_curvature(c1, c2, cvr_up, cvr_down);
        Ok(())
    }

    /// Add an RRAO (residual risk add-on) position.
    ///
    /// Set ``is_exotic=True`` for the 1% weight (exotic underlying), leave as
    /// ``False`` for the 0.1% weight (other residual risk: gap, correlation,
    /// behavioural).
    #[pyo3(signature = (instrument_id, notional, is_exotic = false))]
    fn add_rrao_position(&mut self, instrument_id: &str, notional: f64, is_exotic: bool) {
        self.inner.rrao_exotic_notionals.push(RraoPosition {
            instrument_id: instrument_id.to_string(),
            notional,
            is_exotic,
        });
    }

    /// Base/reporting currency code.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "FrtbSensitivities(base={}, girr_delta={}, equity_delta={}, fx_delta={})",
            self.inner.base_currency,
            self.inner.girr_delta.len(),
            self.inner.equity_delta.len(),
            self.inner.fx_delta.len(),
        )
    }
}

impl PyFrtbSensitivities {
    fn currency_or_base(&self, currency: Option<&str>) -> PyResult<Currency> {
        match currency {
            Some(c) => parse_currency(c),
            None => Ok(self.inner.base_currency),
        }
    }
}

/// FRTB SBA engine.
#[pyclass(
    name = "FrtbSbaEngine",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
pub struct PyFrtbSbaEngine {
    inner: FrtbSbaEngine,
}

#[pymethods]
impl PyFrtbSbaEngine {
    /// Create an FRTB SBA engine.
    #[new]
    #[pyo3(signature = (correlation_scenario = None))]
    fn new(correlation_scenario: Option<&str>) -> PyResult<Self> {
        let mut builder = FrtbSbaEngine::builder();
        if let Some(s) = correlation_scenario {
            let scenario = parse_correlation_scenario(s)?;
            builder = builder.scenarios(vec![scenario]);
        }
        Ok(Self {
            inner: builder.build().map_err(core_to_py)?,
        })
    }

    /// Calculate the FRTB SBA charge for a sensitivity portfolio.
    fn calculate(
        &self,
        py: Python<'_>,
        sensitivities: &PyFrtbSensitivities,
    ) -> PyResult<(f64, Py<PyDict>)> {
        let result = py
            .detach(|| self.inner.calculate(&sensitivities.inner))
            .map_err(core_to_py)?;
        frtb_result_to_py(py, &result)
    }
}

// ---------------------------------------------------------------------------
// SaCcrTrade wrapper
// ---------------------------------------------------------------------------

/// A single derivative trade for SA-CCR EAD computation.
///
/// This wrapper provides a simple linear-trade constructor. For options or
/// bespoke trades, construct via :meth:`from_json`.
#[pyclass(
    name = "SaCcrTrade",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySaCcrTrade {
    pub(super) inner: SaCcrTrade,
}

#[pymethods]
impl PySaCcrTrade {
    /// Create a linear (non-option) SA-CCR trade.
    ///
    /// ``asset_class`` accepts ``"ir"``, ``"fx"``, ``"credit"``, ``"equity"``,
    /// or ``"commodity"``. ``direction`` is ``+1.0`` for long, ``-1.0`` for
    /// short; the supervisory delta defaults to the same value.
    #[new]
    #[pyo3(signature = (
        trade_id,
        asset_class,
        notional,
        start_year,
        start_month,
        start_day,
        end_year,
        end_month,
        end_day,
        underlier,
        hedging_set,
        direction = 1.0,
        mtm = 0.0,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        trade_id: &str,
        asset_class: &str,
        notional: f64,
        start_year: i32,
        start_month: u8,
        start_day: u8,
        end_year: i32,
        end_month: u8,
        end_day: u8,
        underlier: &str,
        hedging_set: &str,
        direction: f64,
        mtm: f64,
    ) -> PyResult<Self> {
        let ac = parse_asset_class(asset_class)?;
        let start_date = parse_date(start_year, start_month, start_day)?;
        let end_date = parse_date(end_year, end_month, end_day)?;
        Ok(Self {
            inner: SaCcrTrade {
                trade_id: trade_id.to_string(),
                asset_class: ac,
                notional,
                start_date,
                end_date,
                underlier: underlier.to_string(),
                hedging_set: hedging_set.to_string(),
                direction,
                supervisory_delta: direction,
                mtm,
                is_option: false,
                option_type: None,
            },
        })
    }

    /// Construct from a JSON serialization of `SaCcrTrade`.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: SaCcrTrade = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn trade_id(&self) -> &str {
        &self.inner.trade_id
    }

    #[getter]
    fn asset_class(&self) -> String {
        asset_class_label(self.inner.asset_class).to_string()
    }

    #[getter]
    fn notional(&self) -> f64 {
        self.inner.notional
    }

    #[getter]
    fn mtm(&self) -> f64 {
        self.inner.mtm
    }

    fn __repr__(&self) -> String {
        format!(
            "SaCcrTrade(id={}, class={}, notional={:.0}, mtm={:.0})",
            self.inner.trade_id,
            asset_class_label(self.inner.asset_class),
            self.inner.notional,
            self.inner.mtm,
        )
    }
}

/// SA-CCR netting-set configuration.
#[pyclass(
    name = "SaCcrNettingSetConfig",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySaCcrNettingSetConfig {
    inner: SaCcrNettingSetConfig,
}

#[pymethods]
impl PySaCcrNettingSetConfig {
    /// Create an unmargined netting set configuration.
    #[staticmethod]
    #[pyo3(signature = (
        counterparty_id,
        csa_id,
        collateral,
        as_of_year,
        as_of_month,
        as_of_day,
    ))]
    fn unmargined(
        counterparty_id: &str,
        csa_id: &str,
        collateral: f64,
        as_of_year: i32,
        as_of_month: u8,
        as_of_day: u8,
    ) -> PyResult<Self> {
        let as_of = parse_date(as_of_year, as_of_month, as_of_day)?;
        Ok(Self {
            inner: SaCcrNettingSetConfig::unmargined(
                NettingSetId::bilateral(counterparty_id, csa_id),
                collateral,
                as_of,
            ),
        })
    }

    /// Create a margined netting set configuration.
    #[staticmethod]
    #[pyo3(signature = (
        counterparty_id,
        csa_id,
        collateral,
        threshold,
        mta,
        nica,
        mpor_days,
        as_of_year,
        as_of_month,
        as_of_day,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn margined(
        counterparty_id: &str,
        csa_id: &str,
        collateral: f64,
        threshold: f64,
        mta: f64,
        nica: f64,
        mpor_days: u32,
        as_of_year: i32,
        as_of_month: u8,
        as_of_day: u8,
    ) -> PyResult<Self> {
        let as_of = parse_date(as_of_year, as_of_month, as_of_day)?;
        Ok(Self {
            inner: SaCcrNettingSetConfig::margined(
                NettingSetId::bilateral(counterparty_id, csa_id),
                collateral,
                threshold,
                mta,
                nica,
                mpor_days,
                as_of,
            ),
        })
    }

    /// Construct from a JSON serialization of `SaCcrNettingSetConfig`.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: SaCcrNettingSetConfig = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn is_margined(&self) -> bool {
        self.inner.is_margined
    }

    #[getter]
    fn collateral(&self) -> f64 {
        self.inner.collateral
    }
}

/// SA-CCR engine.
#[pyclass(
    name = "SaCcrEngine",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
pub struct PySaCcrEngine {
    inner: SaCcrEngine,
}

#[pymethods]
impl PySaCcrEngine {
    /// Create an SA-CCR engine.
    #[new]
    #[pyo3(signature = (alpha = None, reporting_currency = "USD"))]
    fn new(alpha: Option<f64>, reporting_currency: &str) -> PyResult<Self> {
        let mut builder =
            SaCcrEngine::builder().reporting_currency(parse_currency(reporting_currency)?);
        if let Some(a) = alpha {
            builder = builder.alpha(a);
        }
        Ok(Self {
            inner: builder.build().map_err(core_to_py)?,
        })
    }

    /// Calculate SA-CCR EAD for a netting set and trade list.
    fn calculate_ead(
        &self,
        py: Python<'_>,
        config: &PySaCcrNettingSetConfig,
        trades: Vec<PyRef<'_, PySaCcrTrade>>,
    ) -> PyResult<Py<PyDict>> {
        let trade_vec: Vec<SaCcrTrade> = trades.iter().map(|t| t.inner.clone()).collect();
        let result = py
            .detach(|| self.inner.calculate_ead(&config.inner, &trade_vec))
            .map_err(core_to_py)?;
        ead_result_to_py(py, &result)
    }
}

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

fn frtb_result_to_py(
    py: Python<'_>,
    result: &finstack_quant_margin::regulatory::frtb::FrtbSbaResult,
) -> PyResult<(f64, Py<PyDict>)> {
    let dict = PyDict::new(py);

    let delta = PyDict::new(py);
    for (rc, v) in &result.delta_by_risk_class {
        delta.set_item(risk_class_label(*rc), *v)?;
    }
    dict.set_item("delta", delta)?;

    let vega = PyDict::new(py);
    for (rc, v) in &result.vega_by_risk_class {
        vega.set_item(risk_class_label(*rc), *v)?;
    }
    dict.set_item("vega", vega)?;

    let curvature = PyDict::new(py);
    for (rc, v) in &result.curvature_by_risk_class {
        curvature.set_item(risk_class_label(*rc), *v)?;
    }
    dict.set_item("curvature", curvature)?;

    dict.set_item("drc", result.drc)?;
    dict.set_item("rrao", result.rrao)?;

    let binding_scenario_name = match result.binding_scenario {
        CorrelationScenario::Low => "low",
        CorrelationScenario::Medium => "medium",
        CorrelationScenario::High => "high",
    };
    dict.set_item("binding_scenario", binding_scenario_name)?;

    let scenarios = PyDict::new(py);
    for (s, v) in &result.scenario_charges {
        let name = match s {
            CorrelationScenario::Low => "low",
            CorrelationScenario::Medium => "medium",
            CorrelationScenario::High => "high",
        };
        scenarios.set_item(name, *v)?;
    }
    dict.set_item("scenario_charges", scenarios)?;

    Ok((result.total, dict.unbind()))
}

fn ead_result_to_py(
    py: Python<'_>,
    result: &finstack_quant_margin::regulatory::sa_ccr::EadResult,
) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("ead", result.ead)?;
    dict.set_item("rc", result.rc)?;
    dict.set_item("pfe", result.pfe)?;
    dict.set_item("multiplier", result.multiplier)?;
    dict.set_item("add_on_aggregate", result.add_on_aggregate)?;
    dict.set_item("alpha", result.alpha)?;
    dict.set_item("maturity_factor", result.maturity_factor)?;

    let by_class = PyDict::new(py);
    for (asset_class, value) in &result.add_on_by_asset_class {
        by_class.set_item(asset_class_label(*asset_class), *value)?;
    }
    dict.set_item("add_on_by_asset_class", by_class)?;
    Ok(dict.unbind())
}

/// Compute the FRTB SBA capital charge.
///
/// Returns ``(total_charge, breakdown)`` where ``breakdown`` is a dict with:
/// - ``delta``: ``{risk_class: charge}``
/// - ``vega``: ``{risk_class: charge}``
/// - ``curvature``: ``{risk_class: charge}``
/// - ``drc``: default risk charge (float)
/// - ``rrao``: residual risk add-on (float)
/// - ``binding_scenario``: scenario name selected as binding
/// - ``scenario_charges``: ``{scenario_name: sba_charge}``
///
/// If ``correlation_scenario`` is provided (``"low"``, ``"medium"``, ``"high"``),
/// only that scenario is evaluated. Otherwise all three are run and the
/// max is taken per BCBS d457.
#[pyfunction]
#[pyo3(signature = (sensitivities, correlation_scenario = None))]
pub fn frtb_sba_charge(
    py: Python<'_>,
    sensitivities: &PyFrtbSensitivities,
    correlation_scenario: Option<&str>,
) -> PyResult<(f64, Py<PyDict>)> {
    let mut builder = FrtbSbaEngine::builder();
    if let Some(s) = correlation_scenario {
        let scenario = parse_correlation_scenario(s)?;
        builder = builder.scenarios(vec![scenario]);
    }
    let engine = builder.build().map_err(core_to_py)?;
    let result = py
        .detach(|| engine.calculate(&sensitivities.inner))
        .map_err(core_to_py)?;

    frtb_result_to_py(py, &result)
}

/// Compute SA-CCR Exposure at Default for a set of trades.
///
/// Returns ``(rc, pfe, ead)`` per BCBS 279:
/// - ``rc``: replacement cost
/// - ``pfe``: potential future exposure (multiplier × aggregate add-on)
/// - ``ead``: exposure at default = α × (RC + PFE), with α = 1.4
///
/// The netting set uses default terms: zero collateral / threshold / MTA /
/// NICA, and 10-day MPoR when margined. For bespoke collateral terms,
/// build via :meth:`SaCcrEngine.calculate_from_json` (not yet exposed).
#[pyfunction]
#[pyo3(signature = (trades, margined = false, collateral = 0.0))]
pub fn saccr_ead(
    py: Python<'_>,
    trades: Vec<PyRef<'_, PySaCcrTrade>>,
    margined: bool,
    collateral: f64,
) -> PyResult<(f64, f64, f64)> {
    let engine = SaCcrEngine::builder().build().map_err(core_to_py)?;
    let netting_id = NettingSetId::bilateral("CPTY", "CSA");
    let trade_vec: Vec<SaCcrTrade> = trades.iter().map(|t| t.inner.clone()).collect();
    let as_of = if let Some(date) = trade_vec.iter().map(|t| t.start_date).min() {
        date
    } else {
        parse_date(1970, 1, 1)?
    };
    let config = if margined {
        SaCcrNettingSetConfig::margined(netting_id, collateral, 0.0, 0.0, 0.0, 10, as_of)
    } else {
        SaCcrNettingSetConfig::unmargined(netting_id, collateral, as_of)
    };
    let result = py
        .detach(|| engine.calculate_ead(&config, &trade_vec))
        .map_err(core_to_py)?;
    Ok((result.rc, result.pfe, result.ead))
}

/// Register FRTB / SA-CCR classes and functions on the margin module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFrtbSensitivities>()?;
    m.add_class::<PyFrtbSbaEngine>()?;
    m.add_class::<PySaCcrTrade>()?;
    m.add_class::<PySaCcrNettingSetConfig>()?;
    m.add_class::<PySaCcrEngine>()?;
    m.add_function(pyo3::wrap_pyfunction!(frtb_sba_charge, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(saccr_ead, m)?)?;
    Ok(())
}
