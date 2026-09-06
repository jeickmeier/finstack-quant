//! Python bindings for direct initial-margin calculators.
//!
//! These bindings expose the explicit calculator helper methods that do not
//! require a Python representation of the Rust `Marginable` trait.

use super::calculators::{money_from_amount, PyImResult};
use super::frame::{opt_str, records, req_f64, req_str, split_pair};
use super::types::{extract_asset_class, PyCollateralAssetClass, PyEligibleCollateralSchedule};
use crate::bindings::date_utils::extract_date;
use crate::bindings::module_utils::parse_currency;
use crate::errors::core_to_py;
use finstack_quant_margin as fm;
use pyo3::prelude::*;

fn parse_simm_version(version: &str) -> PyResult<fm::SimmVersion> {
    version
        .parse::<fm::SimmVersion>()
        .map_err(crate::errors::value_error)
}

fn parse_credit_sector(sector: &str) -> PyResult<fm::SimmCreditSector> {
    sector
        .parse::<fm::SimmCreditSector>()
        .map_err(crate::errors::value_error)
}

fn parse_risk_class(risk_class: &str) -> PyResult<fm::SimmRiskClass> {
    risk_class
        .parse::<fm::SimmRiskClass>()
        .map_err(crate::errors::value_error)
}

fn parse_schedule_asset_class(asset_class: &str) -> PyResult<fm::ScheduleAssetClass> {
    asset_class
        .parse::<fm::ScheduleAssetClass>()
        .map_err(crate::errors::value_error)
}

/// ISDA SIMM sensitivity portfolio.
///
/// Signed sensitivity amounts keyed by SIMM risk class and bucket, all in
/// ``base_currency``. Rate and credit deltas are DV01/CS01-style amounts per
/// 1bp move; vegas are currency amounts compatible with the SIMM vega
/// weights; curvature is a single signed contribution per risk class. Tenor
/// labels must be SIMM buckets (``CONSTANTS["SIMM_TENORS"]``) and commodity
/// buckets one of the 17 ISDA buckets — ``validate()`` (run automatically by
/// ``SimmCalculator.calculate_from_sensitivities``) rejects anything else so
/// a typo cannot price to zero margin.
#[pyclass(
    name = "SimmSensitivities",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySimmSensitivities {
    pub(super) inner: fm::SimmSensitivities,
}

#[pymethods]
impl PySimmSensitivities {
    /// Create an empty SIMM sensitivity container in ``base_currency``.
    /// Raises ``ValueError`` for an unknown currency code.
    #[new]
    #[pyo3(signature = (base_currency = "USD"))]
    fn new(base_currency: &str) -> PyResult<Self> {
        Ok(Self {
            inner: fm::SimmSensitivities::new(parse_currency(base_currency)?),
        })
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

    /// Construct from the canonical JSON shape; raises ``ValueError`` on
    /// malformed input.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = fm::SimmSensitivities::from_json(json).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to the canonical JSON shape.
    fn to_json(&self) -> PyResult<String> {
        self.inner.to_json().map_err(core_to_py)
    }

    /// Bulk-load sensitivities from the long-format frame ``to_dataframe``
    /// emits (CRIF-style).
    ///
    /// Parameters
    /// ----------
    /// frame : pandas.DataFrame
    ///     Columns ``risk_class``, ``kind``, ``issuer``, ``bucket``,
    ///     ``tenor``, ``amount`` with the same encoding as ``to_dataframe``:
    ///     ``issuer`` is the currency for ``interest_rate``/``fx`` delta, the
    ///     ``"CCY1/CCY2"`` pair for FX vega, the issuer for credit and the
    ///     underlier for equity; ``bucket`` is the credit sector or the
    ///     commodity bucket; ``tenor`` is the SIMM tenor where the risk class
    ///     has one. ``kind`` is ``delta``, ``vega`` or ``curvature``.
    /// base_currency : str, default "USD"
    ///     Currency in which every ``amount`` is expressed.
    ///
    /// Rows with the same key accumulate. Raises ``ValueError`` for an
    /// unknown risk class, kind, sector, currency or a missing column, and
    /// ``TypeError`` when ``frame`` has no ``to_dict`` method.
    #[staticmethod]
    #[pyo3(signature = (frame, base_currency = "USD"))]
    fn from_dataframe(frame: &Bound<'_, PyAny>, base_currency: &str) -> PyResult<Self> {
        let mut sens = fm::SimmSensitivities::new(parse_currency(base_currency)?);
        for row in records(frame)? {
            let risk_class = req_str(&row, "risk_class")?;
            let kind = req_str(&row, "kind")?;
            let amount = req_f64(&row, "amount")?;
            let issuer = opt_str(&row, "issuer")?;
            let bucket = opt_str(&row, "bucket")?;
            let tenor = opt_str(&row, "tenor")?;
            let need = |value: &Option<String>, name: &str| -> PyResult<String> {
                value.clone().ok_or_else(|| {
                    crate::errors::value_error(format!(
                        "from_dataframe: {risk_class} {kind} row needs a '{name}' value"
                    ))
                })
            };
            if kind == "curvature" {
                sens.add_curvature(parse_risk_class(&risk_class)?, amount);
                continue;
            }
            match (risk_class.as_str(), kind.as_str()) {
                ("interest_rate", "delta") => sens.add_ir_delta(
                    parse_currency(&need(&issuer, "issuer")?)?,
                    need(&tenor, "tenor")?,
                    amount,
                ),
                ("interest_rate", "vega") => sens.add_ir_vega(
                    parse_currency(&need(&issuer, "issuer")?)?,
                    need(&tenor, "tenor")?,
                    amount,
                ),
                ("credit_qualifying", "delta") => sens.add_credit_qualifying_delta(
                    parse_credit_sector(&need(&bucket, "bucket")?)?,
                    need(&issuer, "issuer")?,
                    need(&tenor, "tenor")?,
                    amount,
                ),
                ("credit_qualifying", "vega") => sens.add_credit_qualifying_vega(
                    parse_credit_sector(&need(&bucket, "bucket")?)?,
                    need(&issuer, "issuer")?,
                    need(&tenor, "tenor")?,
                    amount,
                ),
                ("credit_non_qualifying", "delta") => sens.add_credit_non_qualifying_delta(
                    need(&issuer, "issuer")?,
                    need(&tenor, "tenor")?,
                    amount,
                ),
                ("credit_non_qualifying", "vega") => sens.add_credit_non_qualifying_vega(
                    need(&issuer, "issuer")?,
                    need(&tenor, "tenor")?,
                    amount,
                ),
                ("equity", "delta") => sens.add_equity_delta(need(&issuer, "issuer")?, amount),
                ("equity", "vega") => sens.add_equity_vega(need(&issuer, "issuer")?, amount),
                ("fx", "delta") => {
                    sens.add_fx_delta(parse_currency(&need(&issuer, "issuer")?)?, amount)
                }
                ("fx", "vega") => {
                    let (c1, c2) = split_pair(&need(&issuer, "issuer")?, "fx vega issuer")?;
                    sens.add_fx_vega(parse_currency(&c1)?, parse_currency(&c2)?, amount);
                }
                ("commodity", "delta") => {
                    sens.add_commodity_delta(need(&bucket, "bucket")?, amount)
                }
                ("commodity", "vega") => sens.add_commodity_vega(need(&bucket, "bucket")?, amount),
                _ => {
                    return Err(crate::errors::value_error(format!(
                    "from_dataframe: unsupported SIMM row risk_class={risk_class:?} kind={kind:?}"
                )))
                }
            }
        }
        Ok(Self { inner: sens })
    }

    /// Add an interest-rate delta: ``amount`` is a signed DV01-style
    /// currency amount per 1bp move for ``tenor`` (a SIMM tenor bucket).
    /// Raises ``ValueError`` for an unknown currency.
    #[pyo3(signature = (currency, tenor, amount))]
    fn add_ir_delta(&mut self, currency: &str, tenor: &str, amount: f64) -> PyResult<()> {
        self.inner
            .add_ir_delta(parse_currency(currency)?, tenor, amount);
        Ok(())
    }

    /// Add an interest-rate vega: ``amount`` is a signed currency vega
    /// compatible with the SIMM IR vega weights for ``tenor``. Raises
    /// ``ValueError`` for an unknown currency.
    #[pyo3(signature = (currency, tenor, amount))]
    fn add_ir_vega(&mut self, currency: &str, tenor: &str, amount: f64) -> PyResult<()> {
        self.inner
            .add_ir_vega(parse_currency(currency)?, tenor, amount);
        Ok(())
    }

    /// Add a sector-bucketed credit-qualifying delta: ``amount`` is a
    /// signed CS01-style currency amount per 1bp move. ``sector`` is a
    /// lower-case SIMM sector label such as ``"financial"``. Raises
    /// ``ValueError`` for an unknown sector.
    #[pyo3(signature = (sector, name, tenor, amount))]
    fn add_credit_qualifying_delta(
        &mut self,
        sector: &str,
        name: &str,
        tenor: &str,
        amount: f64,
    ) -> PyResult<()> {
        self.inner
            .add_credit_qualifying_delta(parse_credit_sector(sector)?, name, tenor, amount);
        Ok(())
    }

    /// Add a sector-bucketed credit-qualifying vega: ``amount`` is a signed
    /// currency vega compatible with the SIMM credit-qualifying vega weight.
    /// Raises ``ValueError`` for an unknown sector.
    #[pyo3(signature = (sector, name, tenor, amount))]
    fn add_credit_qualifying_vega(
        &mut self,
        sector: &str,
        name: &str,
        tenor: &str,
        amount: f64,
    ) -> PyResult<()> {
        self.inner
            .add_credit_qualifying_vega(parse_credit_sector(sector)?, name, tenor, amount);
        Ok(())
    }

    /// Add a credit non-qualifying delta: ``amount`` is a signed CS01-style
    /// currency amount per 1bp move for the named securitisation.
    #[pyo3(signature = (name, tenor, amount))]
    fn add_credit_non_qualifying_delta(&mut self, name: &str, tenor: &str, amount: f64) {
        self.inner
            .add_credit_non_qualifying_delta(name, tenor, amount);
    }

    /// Add a credit non-qualifying vega: ``amount`` is a signed currency
    /// vega compatible with the SIMM credit-non-qualifying vega weight.
    #[pyo3(signature = (name, tenor, amount))]
    fn add_credit_non_qualifying_vega(&mut self, name: &str, tenor: &str, amount: f64) {
        self.inner
            .add_credit_non_qualifying_vega(name, tenor, amount);
    }

    /// Add an equity delta: ``amount`` is a signed currency sensitivity to
    /// the underlier (not a percentage delta).
    #[pyo3(signature = (underlier, amount))]
    fn add_equity_delta(&mut self, underlier: &str, amount: f64) {
        self.inner.add_equity_delta(underlier, amount);
    }

    /// Add an equity vega: ``amount`` is a signed currency vega.
    #[pyo3(signature = (underlier, amount))]
    fn add_equity_vega(&mut self, underlier: &str, amount: f64) {
        self.inner.add_equity_vega(underlier, amount);
    }

    /// Add an FX delta: ``amount`` is a signed currency sensitivity to the
    /// FX risk factor ``currency``. Raises ``ValueError`` for an unknown
    /// currency.
    #[pyo3(signature = (currency, amount))]
    fn add_fx_delta(&mut self, currency: &str, amount: f64) -> PyResult<()> {
        self.inner.add_fx_delta(parse_currency(currency)?, amount);
        Ok(())
    }

    /// Add an FX vega for the pair ``(ccy1, ccy2)``: ``amount`` is a signed
    /// currency vega. Raises ``ValueError`` for an unknown currency.
    #[pyo3(signature = (ccy1, ccy2, amount))]
    fn add_fx_vega(&mut self, ccy1: &str, ccy2: &str, amount: f64) -> PyResult<()> {
        self.inner
            .add_fx_vega(parse_currency(ccy1)?, parse_currency(ccy2)?, amount);
        Ok(())
    }

    /// Add a commodity delta: ``bucket`` is a SIMM commodity bucket id
    /// (``"1"``..``"17"``) or name (``"Crude"``, ``"Precious Metals"``);
    /// ``amount`` is a signed currency sensitivity.
    #[pyo3(signature = (bucket, amount))]
    fn add_commodity_delta(&mut self, bucket: &str, amount: f64) {
        self.inner.add_commodity_delta(bucket, amount);
    }

    /// Add a commodity vega for a SIMM commodity bucket: ``amount`` is a
    /// signed currency vega.
    #[pyo3(signature = (bucket, amount))]
    fn add_commodity_vega(&mut self, bucket: &str, amount: f64) {
        self.inner.add_commodity_vega(bucket, amount);
    }

    /// Add a curvature contribution (signed, in currency units, before the
    /// SIMM curvature scale factor) for a SIMM risk class label such as
    /// ``"interest_rate"`` or ``"equity"``. Raises ``ValueError`` for an
    /// unknown label.
    #[pyo3(signature = (risk_class, amount))]
    fn add_curvature(&mut self, risk_class: &str, amount: f64) -> PyResult<()> {
        self.inner
            .add_curvature(parse_risk_class(risk_class)?, amount);
        Ok(())
    }

    /// Add every bucket of ``other`` into this container (amounts sum), so
    /// offsetting risk nets within a netting set. Both containers must be in
    /// the same base currency; use ``scaled_to_currency`` first otherwise.
    /// Raises ``ValueError`` on a base-currency mismatch.
    fn merge(&mut self, other: &PySimmSensitivities) -> PyResult<()> {
        if other.inner.base_currency != self.inner.base_currency {
            return Err(crate::errors::value_error(format!(
                "cannot merge SIMM sensitivities in {} into a {} container; call \
                 scaled_to_currency first",
                other.inner.base_currency, self.inner.base_currency
            )));
        }
        self.inner.merge(&other.inner);
        Ok(())
    }

    /// Return a copy with every amount multiplied by the signed ``factor``
    /// (e.g. position quantity for unit-notional trade sensitivities); the
    /// base currency is unchanged.
    fn scaled(&self, factor: f64) -> Self {
        Self {
            inner: self.inner.scaled(factor),
        }
    }

    /// Return a copy re-expressed in ``target_currency``: every amount is
    /// multiplied by ``fx_rate`` (value of one unit of the current base
    /// currency in ``target_currency``); risk-factor keys are unchanged.
    /// Raises ``ValueError`` for an unknown currency.
    #[pyo3(signature = (target_currency, fx_rate))]
    fn scaled_to_currency(&self, target_currency: &str, fx_rate: f64) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .scaled_to_currency(parse_currency(target_currency)?, fx_rate),
        })
    }

    /// Net IR delta summed across all currencies and tenors.
    fn total_ir_delta(&self) -> f64 {
        self.inner.total_ir_delta()
    }

    /// Net equity delta summed across all underliers.
    fn total_equity_delta(&self) -> f64 {
        self.inner.total_equity_delta()
    }

    /// Validate tenor labels, commodity buckets, identifiers and amounts.
    ///
    /// Raises ``ValueError`` naming the offending map when a tenor is not a
    /// SIMM bucket, a commodity bucket is unknown, an identifier is empty or
    /// an amount is non-finite. ``SimmCalculator.calculate_from_sensitivities``
    /// runs this automatically.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Whether the sensitivity container has no populated buckets.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Base currency of the sensitivity set.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Export every populated sensitivity bucket as one long-format pandas
    /// ``DataFrame``.
    ///
    /// Columns: ``risk_class``, ``bucket``, ``tenor``, ``issuer``, ``kind``,
    /// ``amount``. One row per populated bucket; an empty container still
    /// carries all six columns. Long format is used deliberately — a column
    /// per bucket would give a different schema for every portfolio, and it
    /// matches ``FrtbSensitivities.to_dataframe``.
    ///
    /// ``risk_class`` uses the SIMM labels ``interest_rate``,
    /// ``credit_qualifying``, ``credit_non_qualifying``, ``equity``,
    /// ``commodity`` and ``fx``. ``kind`` is ``delta``, ``vega`` or
    /// ``curvature``; SIMM curvature is a single signed contribution per risk
    /// class, not an up/down pair.
    ///
    /// ``issuer`` carries the name axis: a currency code for IR and FX delta,
    /// a ``"CCY1/CCY2"`` pair for FX vega, an issuer or index for credit, an
    /// underlier for equity. It is ``None`` for commodity (keyed by bucket
    /// alone) and for curvature. ``bucket`` holds the SIMM credit sector for
    /// bucketed credit deltas (e.g. ``"sovereign"``) and the commodity bucket
    /// label; it is ``None`` elsewhere. ``tenor`` is the SIMM tenor bucket
    /// (``"2W"``, ``"1M"``, ..., ``"30Y"``) where the risk class has one.
    ///
    /// ``amount`` is a signed currency sensitivity in the container's base
    /// currency, in whatever convention the caller supplied — SIMM does not
    /// re-scale these on ingest. ``from_dataframe`` accepts this frame back.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::table_to_dataframe(
            py,
            &self.inner.to_table().map_err(core_to_py)?,
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Summarise the container by populated bucket counts per risk class.
    fn __repr__(&self) -> String {
        let s = &self.inner;
        format!(
            "SimmSensitivities(base_currency={}, ir_delta={}, ir_vega={}, credit_qualifying={}, \
             credit_non_qualifying={}, equity={}, fx={}, commodity={}, curvature={})",
            s.base_currency,
            s.ir_delta.len(),
            s.ir_vega.len(),
            s.credit_qualifying_delta.len() + s.credit_qualifying_vega.len(),
            s.credit_non_qualifying_delta.len() + s.credit_non_qualifying_vega.len(),
            s.equity_delta.len() + s.equity_vega.len(),
            s.fx_delta.len() + s.fx_vega.len(),
            s.commodity_delta.len() + s.commodity_vega.len(),
            s.curvature.len(),
        )
    }
}

/// Indicative historical SIMM v2.6 calculator. USD-normalized inputs are required.
/// Product-class, subcurve and some non-IR dimensions are incomplete; results
/// always carry approximation=true and do not represent current regulatory SIMM.
///
/// Loads the registry-backed SIMM parameters for one rule version and
/// aggregates explicit ``SimmSensitivities`` into an ``ImResult``.
#[pyclass(
    name = "SimmCalculator",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySimmCalculator {
    pub(super) inner: fm::SimmCalculator,
}

#[pymethods]
impl PySimmCalculator {
    /// Create a SIMM calculator from the embedded margin registry.
    ///
    /// ``version`` defaults to the Rust ``SimmVersion::default()`` (currently
    /// ``"v2_6"``); ``mpor_days`` overrides the margin period of risk in
    /// business days stamped on results (registry default 10). Raises
    /// ``ValueError`` for an unknown version.
    #[new]
    #[pyo3(signature = (version = None, mpor_days = None))]
    fn new(version: Option<&str>, mpor_days: Option<u32>) -> PyResult<Self> {
        let version = match version {
            Some(version) => parse_simm_version(version)?,
            None => fm::SimmVersion::default(),
        };
        let mut inner = fm::SimmCalculator::new(version).map_err(core_to_py)?;
        if let Some(days) = mpor_days {
            inner = inner.with_mpor(days);
        }
        Ok(Self { inner })
    }

    /// Supported SIMM version label (`"v2_6"`).
    #[getter]
    fn version(&self) -> &'static str {
        self.inner.version().as_str()
    }

    /// Margin period of risk in business days.
    #[getter]
    fn mpor_days(&self) -> u32 {
        self.inner.mpor_days()
    }

    /// Identify this calculator in notebooks and logs.
    fn __repr__(&self) -> String {
        format!(
            "SimmCalculator(version={:?}, mpor_days={})",
            self.inner.version().as_str(),
            self.inner.mpor_days()
        )
    }

    /// Calculate SIMM initial margin from explicit sensitivities.
    ///
    /// Parameters
    /// ----------
    /// sensitivities : SimmSensitivities
    ///     Sensitivity set to aggregate; validated first (unknown tenors or
    ///     commodity buckets raise instead of pricing to zero).
    /// currency : str
    ///     Must be USD and equal sensitivities.base_currency. Concentration
    ///     thresholds are USD-denominated; convert other-currency inputs first.
    /// as_of : datetime.date | str
    ///     Calculation date stamped on the result.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the sensitivities fail validation, the currency is unknown, or
    ///     the date string is not ISO 8601.
    /// TypeError
    ///     If ``as_of`` is neither a string nor date-like.
    #[pyo3(signature = (sensitivities, currency, as_of))]
    fn calculate_from_sensitivities(
        &self,
        py: Python<'_>,
        sensitivities: &PySimmSensitivities,
        currency: &str,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyImResult> {
        let ccy = parse_currency(currency)?;
        let as_of = extract_date(as_of)?;
        let inner = py
            .detach(|| {
                self.inner
                    .calculate_from_sensitivities(&sensitivities.inner, ccy, as_of)
            })
            .map_err(core_to_py)?;
        Ok(PyImResult::from_inner(inner))
    }
}

/// BCBS-IOSCO regulatory schedule initial-margin calculator.
///
/// Applies registry-backed schedule rates (percent of notional by asset
/// class and maturity bucket) to explicit notionals or to a
/// single-asset-class netting set with the net-to-gross reduction.
#[pyclass(
    name = "ScheduleImCalculator",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScheduleImCalculator {
    inner: fm::ScheduleImCalculator,
}

#[pymethods]
impl PyScheduleImCalculator {
    /// Create a schedule calculator from the embedded BCBS-IOSCO grid.
    /// Raises ``ValueError`` if the registry cannot load.
    #[staticmethod]
    fn bcbs_standard() -> PyResult<Self> {
        Ok(Self {
            inner: fm::ScheduleImCalculator::bcbs_standard().map_err(core_to_py)?,
        })
    }

    /// Create a schedule calculator from a registry id such as
    /// ``CONSTANTS["BCBS_IOSCO_SCHEDULE_ID"]``. Raises ``ValueError`` for an
    /// unknown id.
    #[staticmethod]
    fn from_registry_id(schedule_id: &str) -> PyResult<Self> {
        Ok(Self {
            inner: fm::ScheduleImCalculator::from_registry_id(schedule_id).map_err(core_to_py)?,
        })
    }

    /// Return a copy whose default asset class is ``asset_class`` (a
    /// lower-case label such as ``"interest_rate"``, ``"credit"``,
    /// ``"equity"``, ``"commodity"``, ``"fx"``, ``"other"``, or
    /// ``"custom_<name>"`` for a registry-defined class). Raises
    /// ``ValueError`` for an unknown label.
    fn with_asset_class(&self, asset_class: &str) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_asset_class(parse_schedule_asset_class(asset_class)?),
        })
    }

    /// Return a copy whose default maturity is finite nonnegative ``years``.
    /// Raises ValueError for invalid maturity.
    fn with_maturity(&self, years: f64) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_maturity(years)
                .map_err(core_to_py)?,
        })
    }

    /// Default asset class label used by trait-based calculations.
    #[getter]
    fn default_asset_class(&self) -> String {
        self.inner.default_asset_class.as_str().into_owned()
    }

    /// Default remaining maturity in years used by trait-based calculations.
    #[getter]
    fn default_maturity_years(&self) -> f64 {
        self.inner.default_maturity_years
    }

    /// Margin period of risk in business days stamped on results.
    #[getter]
    fn mpor_days(&self) -> u32 {
        self.inner.mpor_days
    }

    /// Schedule IM rate (decimal fraction of notional, ``0.04`` = 4%) for an
    /// asset class label and remaining maturity in years. Raises
    /// ``ValueError`` for an unknown label.
    fn rate(&self, asset_class: &str, maturity_years: f64) -> PyResult<f64> {
        self.inner
            .rate(parse_schedule_asset_class(asset_class)?, maturity_years)
            .map_err(core_to_py)
    }

    /// Calculate gross schedule IM from an explicit notional.
    ///
    /// Parameters
    /// ----------
    /// notional : float
    ///     Regulatory notional in ``currency``; the formula uses its absolute
    ///     value.
    /// currency : str
    ///     ISO-4217 code for the notional and result.
    /// asset_class : str
    ///     Schedule asset class label (see ``with_asset_class``).
    /// maturity_years : float
    ///     Remaining maturity used for the rate lookup.
    /// as_of : datetime.date | str
    ///     Calculation date stamped on the result.
    ///
    /// Raises ``ValueError`` if the currency, asset class, amount or date is
    /// invalid.
    #[pyo3(signature = (notional, currency, asset_class, maturity_years, as_of))]
    fn calculate_for_notional(
        &self,
        notional: f64,
        currency: &str,
        asset_class: &str,
        maturity_years: f64,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyImResult> {
        let ccy = parse_currency(currency)?;
        let as_of = extract_date(as_of)?;
        let asset_class = parse_schedule_asset_class(asset_class)?;
        Ok(PyImResult::from_inner(
            self.inner
                .calculate_for_notional(
                    money_from_amount(notional, ccy)?,
                    asset_class,
                    maturity_years,
                    as_of,
                )
                .map_err(core_to_py)?,
        ))
    }

    /// Calculate trade-specific gross IM and apply one netting-set NGR.
    ///
    /// Parameters
    /// ----------
    /// positions : list[tuple[float, float, str, float]]
    ///     ``(signed_mtm, gross_notional, asset_class, maturity_years)`` in
    ///     ``currency``. Each trade uses its own schedule rate.
    /// currency : str
    ///     ISO-4217 code for every amount and the result.
    /// as_of : datetime.date | str
    ///     Calculation date stamped on the result.
    ///
    /// Returns ``None`` for an empty position list or zero gross notional.
    /// Raises ``ValueError`` if the currency, asset class, an amount, a
    /// maturity or the date is invalid.
    #[pyo3(signature = (positions, currency, as_of))]
    fn calculate_netting_set_with_ngr(
        &self,
        positions: Vec<(f64, f64, String, f64)>,
        currency: &str,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<Option<PyImResult>> {
        let ccy = parse_currency(currency)?;
        let as_of = extract_date(as_of)?;
        let money_positions: Vec<_> = positions
            .into_iter()
            .map(|(mtm, notional, asset_class, maturity)| {
                Ok((
                    money_from_amount(mtm, ccy)?,
                    money_from_amount(notional, ccy)?,
                    parse_schedule_asset_class(&asset_class)?,
                    maturity,
                ))
            })
            .collect::<PyResult<_>>()?;
        Ok(self
            .inner
            .calculate_netting_set_with_ngr(&money_positions, as_of)
            .map_err(core_to_py)?
            .map(PyImResult::from_inner))
    }

    fn __repr__(&self) -> String {
        format!(
            "ScheduleImCalculator(default_asset_class={:?}, default_maturity_years={}, mpor_days={})",
            self.inner.default_asset_class.as_str(),
            self.inner.default_maturity_years,
            self.inner.mpor_days
        )
    }
}

/// Haircut-based initial-margin calculator.
///
/// ``IM = collateral_value x haircut`` with the asset-class FX add-on when
/// the posted collateral currency differs from the exposure currency; the
/// standard methodology for repos and securities financing. Haircuts are
/// decimal fractions.
#[pyclass(
    name = "HaircutImCalculator",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHaircutImCalculator {
    inner: fm::HaircutImCalculator,
}

#[pymethods]
impl PyHaircutImCalculator {
    /// Create a haircut calculator with the BCBS-IOSCO schedule. Raises
    /// ``ValueError`` if the registry cannot load.
    #[staticmethod]
    fn bcbs_standard() -> PyResult<Self> {
        Ok(Self {
            inner: fm::HaircutImCalculator::bcbs_standard().map_err(core_to_py)?,
        })
    }

    /// Create a haircut calculator with the US Treasuries schedule. Raises
    /// ``ValueError`` if the registry cannot load.
    #[staticmethod]
    fn us_treasuries() -> PyResult<Self> {
        Ok(Self {
            inner: fm::HaircutImCalculator::us_treasuries().map_err(core_to_py)?,
        })
    }

    /// Create a haircut calculator from an eligible-collateral schedule.
    #[staticmethod]
    fn from_schedule(schedule: &PyEligibleCollateralSchedule) -> Self {
        Self {
            inner: fm::HaircutImCalculator::new(schedule.inner.clone()),
        }
    }

    /// Return a copy configured with a default asset class (a
    /// ``CollateralAssetClass`` or its wire label). Raises ``ValueError``
    /// for an unknown label.
    fn with_default_asset_class(&self, asset_class: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_default_asset_class(extract_asset_class(asset_class)?),
        })
    }

    /// Return a copy declaring the posted-collateral currency; the FX add-on
    /// applies when it differs from the exposure currency. Raises
    /// ``ValueError`` for an unknown currency.
    fn with_posted_collateral_currency(&self, currency: &str) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_posted_collateral_currency(parse_currency(currency)?),
        })
    }

    /// Eligible-collateral schedule the haircuts are read from.
    #[getter]
    fn eligible_collateral(&self) -> PyEligibleCollateralSchedule {
        PyEligibleCollateralSchedule {
            inner: self.inner.eligible_collateral().clone(),
        }
    }

    /// Default collateral asset class assumed by trait-based calculations.
    #[getter]
    fn default_asset_class(&self) -> PyCollateralAssetClass {
        PyCollateralAssetClass {
            inner: self.inner.default_asset_class().clone(),
        }
    }

    /// Declared posted-collateral currency code, or ``None``.
    #[getter]
    fn posted_collateral_currency(&self) -> Option<String> {
        self.inner
            .posted_collateral_currency()
            .map(|c| c.to_string())
    }

    /// Margin period of risk in business days stamped on every result
    /// (``CONSTANTS["HAIRCUT_MPOR_DAYS"]``).
    #[getter]
    fn mpor_days(&self) -> u32 {
        self.inner.mpor_days()
    }

    /// Return a calculator with collateral residual maturity in years and an
    /// optional rating (for example AAA or A-). Raises ValueError for invalid terms.
    #[pyo3(signature = (remaining_years, rating=None))]
    fn with_collateral_terms(&self, remaining_years: f64, rating: Option<&str>) -> PyResult<Self> {
        let rating = rating.map(str::parse).transpose().map_err(core_to_py)?;
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_collateral_terms(remaining_years, rating)
                .map_err(core_to_py)?,
        })
    }

    /// Base haircut (decimal, excluding the FX add-on) for a collateral
    /// asset class or its wire label. Raises ``ValueError`` if the schedule
    /// has no haircut for it.
    fn haircut_for(&self, asset_class: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .haircut_for(&extract_asset_class(asset_class)?)
            .map_err(core_to_py)
    }

    /// Calculate haircut IM from an explicit collateral value.
    ///
    /// Parameters
    /// ----------
    /// collateral_value : float
    ///     Collateral market value in ``currency``.
    /// currency : str
    ///     ISO-4217 code for the collateral value and result.
    /// asset_class : CollateralAssetClass | str
    ///     Collateral asset class used for the haircut lookup.
    /// currency_mismatch : bool
    ///     Whether to add the asset-class FX mismatch add-on.
    /// as_of : datetime.date | str
    ///     Calculation date stamped on the result.
    ///
    /// Raises ``ValueError`` if the currency, amount, date, haircut or FX
    /// add-on cannot be resolved.
    #[pyo3(signature = (collateral_value, currency, asset_class, currency_mismatch, as_of))]
    fn calculate_for_collateral(
        &self,
        collateral_value: f64,
        currency: &str,
        asset_class: &Bound<'_, PyAny>,
        currency_mismatch: bool,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyImResult> {
        let ccy = parse_currency(currency)?;
        let as_of = extract_date(as_of)?;
        Ok(PyImResult::from_inner(
            self.inner
                .calculate_for_collateral(
                    money_from_amount(collateral_value, ccy)?,
                    &extract_asset_class(asset_class)?,
                    currency_mismatch,
                    as_of,
                )
                .map_err(core_to_py)?,
        ))
    }

    fn __repr__(&self) -> String {
        format!(
            "HaircutImCalculator(default_asset_class={}, posted_collateral_currency={}, eligible={}, mpor_days={})",
            self.inner.default_asset_class(),
            self.inner
                .posted_collateral_currency()
                .map_or("None".to_string(), |c| format!("{c:?}")),
            self.inner.eligible_collateral().eligible.len(),
            self.inner.mpor_days()
        )
    }
}

/// Register direct IM calculator bindings.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySimmSensitivities>()?;
    m.add_class::<PySimmCalculator>()?;
    m.add_class::<PyScheduleImCalculator>()?;
    m.add_class::<PyHaircutImCalculator>()?;
    Ok(())
}
