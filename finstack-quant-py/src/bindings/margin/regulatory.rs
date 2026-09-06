//! Python bindings for FRTB SBA and SA-CCR regulatory capital frameworks.
//!
//! Callers build sensitivity or trade containers with the ``add_*`` methods
//! (or bulk-load a long-format frame), then run an engine or the matching
//! free function to get a typed result with a per-component breakdown.

use super::frame::{
    opt_str, records, req_bool, req_bucket, req_date, req_f64, req_str, split_pair,
};
use super::types::{extract_netting_set_id, PyNettingSetId};
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::module_utils::parse_currency;
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    serde_to_py, table_to_dataframe, ColumnSchema,
};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_margin::regulatory::{
    frtb::{
        CorrelationScenario, DrcAssetType, DrcPosition, DrcSector, DrcSeniority, FrtbRiskClass,
        FrtbSbaEngine, FrtbSbaResult, FrtbSensitivities,
    },
    sa_ccr::{
        EadResult, SaCcrAssetClass, SaCcrEngine, SaCcrNettingSetConfig, SaCcrOptionType,
        SaCcrSupervisoryCategory, SaCcrTrade,
    },
};
use pyo3::prelude::*;
use std::collections::BTreeMap;

/// Parse the serde name of a [`CorrelationScenario`] (`"low"`, `"medium"`, `"high"`).
fn parse_correlation_scenario(s: &str) -> PyResult<CorrelationScenario> {
    finstack_quant_core::wire::serde_parse(s).map_err(core_to_py)
}

/// Parse a lower-case serde wire label into an enum.
fn parse_label<T: serde::de::DeserializeOwned>(s: &str) -> PyResult<T> {
    finstack_quant_core::wire::serde_parse(s).map_err(core_to_py)
}

/// Render a value as its own canonical serde wire label.
///
/// Reading the serde representation rather than re-listing variants here means
/// a new variant on a `#[non_exhaustive]` enum cannot silently collapse into a
/// shared `"unknown"` key — which, in a breakdown map, would drop a capital
/// charge as soon as two new variants collided.
fn serde_label<T: serde::Serialize + std::fmt::Debug>(value: T) -> String {
    finstack_quant_core::wire::serde_label(&value).unwrap_or_else(|_| format!("{value:?}"))
}

fn risk_class_label(rc: FrtbRiskClass) -> String {
    serde_label(rc)
}

fn scenario_label(scenario: CorrelationScenario) -> String {
    serde_label(scenario)
}

fn asset_class_label(ac: SaCcrAssetClass) -> String {
    serde_label(ac)
}

fn py_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// FRTB sensitivity portfolio for the Sensitivity-Based Approach.
///
/// Build up delta/vega/curvature/DRC/RRAO inputs with the ``add_*`` methods
/// (or ``from_dataframe``), then pass to ``frtb_sba_charge`` or
/// ``FrtbSbaEngine.calculate``. Units: GIRR deltas are base-currency P&L per
/// **1 percentage point** of curve shift (``100 x DV01``); CSR deltas are
/// base-currency P&L per 1 basis point of spread; equity, commodity and FX
/// deltas are base-currency P&L per 1 percentage point of the underlying;
/// vegas are base-currency P&L as volatility-scaled vega (sigma times dV/dsigma); curvature
/// pairs are the up/down shocked P&L positions; DRC amounts are signed JTD
/// notionals before LGD; RRAO amounts are gross notionals.
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
    /// ``base_currency`` is the reporting currency (e.g. ``"USD"``); raises
    /// ``ValueError`` for an unknown code.
    #[new]
    #[pyo3(signature = (base_currency = "USD"))]
    fn new(base_currency: &str) -> PyResult<Self> {
        let ccy = parse_currency(base_currency)?;
        Ok(Self {
            inner: FrtbSensitivities::new(ccy),
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

    /// Construct from a JSON serialization of `FrtbSensitivities`; raises
    /// ``ValueError`` on malformed input.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: FrtbSensitivities = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Bulk-load sensitivities from the long-format frame ``to_dataframe``
    /// emits.
    ///
    /// Parameters
    /// ----------
    /// frame : pandas.DataFrame
    ///     Columns ``risk_class``, ``kind``, ``issuer``, ``bucket``,
    ///     ``tenor``, ``amount`` encoded as ``to_dataframe`` documents.
    ///     ``curvature_up`` / ``curvature_down`` rows are recombined into
    ///     pairs; ``rrao`` rows carry ``exotic_notional`` / ``other_notional``.
    /// base_currency : str, default "USD"
    ///     Reporting currency of every ``amount``.
    ///
    /// Rows with the same key accumulate. ``drc`` rows are rejected because
    /// the frame does not carry sector, seniority or asset type — add DRC
    /// positions with ``add_drc_position``. Raises ``ValueError`` for an
    /// unknown risk class or kind, a missing column, or a bad currency.
    #[staticmethod]
    #[pyo3(signature = (frame, base_currency = "USD"))]
    #[allow(clippy::too_many_lines)]
    fn from_dataframe(frame: &Bound<'_, PyAny>, base_currency: &str) -> PyResult<Self> {
        let mut sens = FrtbSensitivities::new(parse_currency(base_currency)?);
        for row in records(frame)? {
            let risk_class = req_str(&row, "risk_class")?;
            let kind = req_str(&row, "kind")?;
            let amount = req_f64(&row, "amount")?;
            let issuer = opt_str(&row, "issuer")?;
            let tenor = opt_str(&row, "tenor")?;
            let need = |value: &Option<String>, name: &str| -> PyResult<String> {
                value.clone().ok_or_else(|| {
                    crate::errors::value_error(format!(
                        "from_dataframe: {risk_class} {kind} row needs a '{name}' value"
                    ))
                })
            };
            let unsupported = || {
                crate::errors::value_error(format!(
                    "from_dataframe: unsupported FRTB row risk_class={risk_class:?} kind={kind:?}"
                ))
            };
            let (up, down) = match kind.as_str() {
                "curvature_up" => (amount, 0.0),
                "curvature_down" => (0.0, amount),
                _ => (0.0, 0.0),
            };
            let is_curvature = kind == "curvature_up" || kind == "curvature_down";
            match risk_class.as_str() {
                "girr" => {
                    let ccy = parse_currency(&need(&issuer, "issuer")?)?;
                    match kind.as_str() {
                        "delta" => sens.add_girr_delta(ccy, &need(&tenor, "tenor")?, amount),
                        "inflation_delta" => sens.add_girr_inflation_delta(ccy, amount),
                        "xccy_basis_delta" => sens.add_girr_xccy_basis_delta(ccy, amount),
                        "vega" => {
                            let (option_maturity, underlying_tenor) =
                                split_pair(&need(&tenor, "tenor")?, "girr vega tenor")?;
                            sens.add_girr_vega(ccy, &option_maturity, &underlying_tenor, amount);
                        }
                        _ if is_curvature => sens.add_girr_curvature(ccy, up, down),
                        _ => return Err(unsupported()),
                    }
                }
                "csr_non_sec" | "csr_sec_ctp" | "csr_sec_non_ctp" | "commodity" => {
                    let name = need(&issuer, "issuer")?;
                    let bucket = req_bucket(&row, "bucket")?;
                    let label = || need(&tenor, "tenor");
                    match (risk_class.as_str(), kind.as_str()) {
                        ("csr_non_sec", "delta") => {
                            let (tenor, basis) = split_pair(&label()?, "delta tenor/basis")?;
                            sens.add_csr_nonsec_delta(&name, bucket, &tenor, &basis, amount)
                        }
                        ("csr_non_sec", "vega") => {
                            sens.add_csr_nonsec_vega(&name, bucket, &label()?, amount)
                        }
                        ("csr_non_sec", _) if is_curvature => {
                            sens.add_csr_nonsec_curvature(&name, bucket, up, down)
                        }
                        ("csr_sec_ctp", "delta") => {
                            let (tenor, basis) = split_pair(&label()?, "delta tenor/basis")?;
                            sens.add_csr_sec_ctp_delta(&name, bucket, &tenor, &basis, amount)
                        }
                        ("csr_sec_ctp", "vega") => {
                            sens.add_csr_sec_ctp_vega(&name, bucket, &label()?, amount)
                        }
                        ("csr_sec_ctp", _) if is_curvature => {
                            sens.add_csr_sec_ctp_curvature(&name, bucket, up, down)
                        }
                        ("csr_sec_non_ctp", "delta") => {
                            let (tenor, basis) = split_pair(&label()?, "delta tenor/basis")?;
                            sens.add_csr_sec_nonctp_delta(&name, bucket, &tenor, &basis, amount)
                        }
                        ("csr_sec_non_ctp", "vega") => {
                            sens.add_csr_sec_nonctp_vega(&name, bucket, &label()?, amount)
                        }
                        ("csr_sec_non_ctp", _) if is_curvature => {
                            sens.add_csr_sec_nonctp_curvature(&name, bucket, up, down)
                        }
                        ("commodity", "delta") => {
                            let (tenor, basis) = split_pair(&label()?, "delta tenor/basis")?;
                            sens.add_commodity_delta(&name, bucket, &tenor, &basis, amount)
                        }
                        ("commodity", "vega") => {
                            sens.add_commodity_vega(&name, bucket, &label()?, amount)
                        }
                        ("commodity", _) if is_curvature => {
                            sens.add_commodity_curvature(&name, bucket, up, down)
                        }
                        _ => return Err(unsupported()),
                    }
                }
                "equity" => {
                    let underlier = need(&issuer, "issuer")?;
                    let bucket = req_bucket(&row, "bucket")?;
                    match kind.as_str() {
                        "delta" => sens.add_equity_delta(&underlier, bucket, amount),
                        "repo_delta" => sens.add_equity_repo_delta(&underlier, bucket, amount),
                        "vega" => sens.add_equity_vega(
                            &underlier,
                            bucket,
                            &need(&tenor, "tenor")?,
                            amount,
                        ),
                        _ if is_curvature => {
                            sens.add_equity_curvature(&underlier, bucket, up, down)
                        }
                        _ => return Err(unsupported()),
                    }
                }
                "fx" => {
                    let (c1, c2) = split_pair(&need(&issuer, "issuer")?, "fx issuer")?;
                    let (c1, c2) = (parse_currency(&c1)?, parse_currency(&c2)?);
                    match kind.as_str() {
                        "delta" => sens.add_fx_delta(c1, c2, amount),
                        "vega" => sens.add_fx_vega(c1, c2, &need(&tenor, "tenor")?, amount),
                        _ if is_curvature => sens.add_fx_curvature(c1, c2, up, down),
                        _ => return Err(unsupported()),
                    }
                }
                "rrao" => {
                    let is_exotic = match kind.as_str() {
                        "exotic_notional" => true,
                        "other_notional" => false,
                        _ => return Err(unsupported()),
                    };
                    sens.add_rrao_position(&need(&issuer, "issuer")?, amount, is_exotic);
                }
                "drc" => {
                    return Err(crate::errors::value_error(
                        "from_dataframe: drc rows carry no sector/seniority/asset_type; add \
                         default-risk positions with add_drc_position",
                    ))
                }
                _ => return Err(unsupported()),
            }
        }
        Ok(Self { inner: sens })
    }

    /// Validate labels, buckets, identifiers and amounts without pricing.
    ///
    /// Raises ``ValueError`` naming the first invalid field when a tenor or
    /// bucket is unsupported, an identifier is empty, or a value is
    /// non-finite. The engines run this automatically.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Add a GIRR delta: ``amount`` is base-currency P&L per **1 percentage
    /// point** of curve shift (``100 x DV01``) at ``tenor``; ``currency``
    /// defaults to the base currency. Raises ``ValueError`` for an unknown
    /// currency.
    #[pyo3(signature = (tenor, amount, currency = None))]
    fn add_girr_delta(&mut self, tenor: &str, amount: f64, currency: Option<&str>) -> PyResult<()> {
        let ccy = self.currency_or_base(currency)?;
        self.inner.add_girr_delta(ccy, tenor, amount);
        Ok(())
    }

    /// Add a GIRR inflation delta: ``amount`` is base-currency P&L per 1
    /// percentage point of inflation shift. Raises ``ValueError`` for an
    /// unknown currency.
    #[pyo3(signature = (amount, currency = None))]
    fn add_girr_inflation_delta(&mut self, amount: f64, currency: Option<&str>) -> PyResult<()> {
        let ccy = self.currency_or_base(currency)?;
        self.inner.add_girr_inflation_delta(ccy, amount);
        Ok(())
    }

    /// Add a GIRR cross-currency basis delta: ``amount`` is base-currency
    /// P&L per 1 percentage point of basis shift for ``currency``. Raises
    /// ``ValueError`` for an unknown currency.
    #[pyo3(signature = (amount, currency = None))]
    fn add_girr_xccy_basis_delta(&mut self, amount: f64, currency: Option<&str>) -> PyResult<()> {
        let ccy = self.currency_or_base(currency)?;
        self.inner.add_girr_xccy_basis_delta(ccy, amount);
        Ok(())
    }

    /// Add a CSR non-securitisation delta: ``amount`` is base-currency P&L
    /// per percentage-point spread move (100 times CS01); ``bucket`` is the 1-based CSR
    /// non-sec bucket (MAR21.51).
    #[pyo3(signature = (issuer, bucket, tenor, basis, amount))]
    fn add_csr_nonsec_delta(
        &mut self,
        issuer: &str,
        bucket: u8,
        tenor: &str,
        basis: &str,
        amount: f64,
    ) {
        self.inner
            .add_csr_nonsec_delta(issuer, bucket, tenor, basis, amount);
    }

    /// Add a CSR non-securitisation vega: ``amount`` is base-currency P&L
    /// as volatility-scaled vega (sigma times dV/dsigma) at option ``maturity``.
    #[pyo3(signature = (issuer, bucket, maturity, amount))]
    fn add_csr_nonsec_vega(&mut self, issuer: &str, bucket: u8, maturity: &str, amount: f64) {
        self.inner
            .add_csr_nonsec_vega(issuer, bucket, maturity, amount);
    }

    /// Add a CSR non-securitisation curvature pair (up / down shocked P&L
    /// positions in base currency).
    #[pyo3(signature = (issuer, bucket, cvr_up, cvr_down))]
    fn add_csr_nonsec_curvature(&mut self, issuer: &str, bucket: u8, cvr_up: f64, cvr_down: f64) {
        self.inner
            .add_csr_nonsec_curvature(issuer, bucket, cvr_up, cvr_down);
    }

    /// Add a CSR securitisation (CTP) delta: ``amount`` is base-currency
    /// P&L per percentage-point spread move (100 times CS01); ``bucket`` is the 1-based
    /// sec-CTP bucket (MAR21.59).
    #[pyo3(signature = (tranche, bucket, tenor, basis, amount))]
    fn add_csr_sec_ctp_delta(
        &mut self,
        tranche: &str,
        bucket: u8,
        tenor: &str,
        basis: &str,
        amount: f64,
    ) {
        self.inner
            .add_csr_sec_ctp_delta(tranche, bucket, tenor, basis, amount);
    }

    /// Add a CSR securitisation (CTP) vega: ``amount`` is base-currency P&L
    /// as volatility-scaled vega (sigma times dV/dsigma) at option ``maturity``.
    #[pyo3(signature = (tranche, bucket, maturity, amount))]
    fn add_csr_sec_ctp_vega(&mut self, tranche: &str, bucket: u8, maturity: &str, amount: f64) {
        self.inner
            .add_csr_sec_ctp_vega(tranche, bucket, maturity, amount);
    }

    /// Add a CSR securitisation (CTP) curvature pair (up / down shocked P&L
    /// positions in base currency).
    #[pyo3(signature = (tranche, bucket, cvr_up, cvr_down))]
    fn add_csr_sec_ctp_curvature(&mut self, tranche: &str, bucket: u8, cvr_up: f64, cvr_down: f64) {
        self.inner
            .add_csr_sec_ctp_curvature(tranche, bucket, cvr_up, cvr_down);
    }

    /// Add a CSR securitisation (non-CTP) delta: ``amount`` is base-currency
    /// P&L per percentage-point spread move (100 times CS01); ``bucket`` is the 1-based
    /// sec non-CTP bucket (MAR21.64).
    #[pyo3(signature = (tranche, bucket, tenor, basis, amount))]
    fn add_csr_sec_nonctp_delta(
        &mut self,
        tranche: &str,
        bucket: u8,
        tenor: &str,
        basis: &str,
        amount: f64,
    ) {
        self.inner
            .add_csr_sec_nonctp_delta(tranche, bucket, tenor, basis, amount);
    }

    /// Add a CSR securitisation (non-CTP) vega: ``amount`` is base-currency
    /// P&L as volatility-scaled vega (sigma times dV/dsigma) at option ``maturity``.
    #[pyo3(signature = (tranche, bucket, maturity, amount))]
    fn add_csr_sec_nonctp_vega(&mut self, tranche: &str, bucket: u8, maturity: &str, amount: f64) {
        self.inner
            .add_csr_sec_nonctp_vega(tranche, bucket, maturity, amount);
    }

    /// Add a CSR securitisation (non-CTP) curvature pair (up / down shocked
    /// P&L positions in base currency).
    #[pyo3(signature = (tranche, bucket, cvr_up, cvr_down))]
    fn add_csr_sec_nonctp_curvature(
        &mut self,
        tranche: &str,
        bucket: u8,
        cvr_up: f64,
        cvr_down: f64,
    ) {
        self.inner
            .add_csr_sec_nonctp_curvature(tranche, bucket, cvr_up, cvr_down);
    }

    /// Add an equity delta: ``amount`` is base-currency P&L per 1 percentage
    /// point move in the underlier; ``bucket`` is the 1-based equity bucket
    /// (MAR21.72).
    #[pyo3(signature = (underlier, bucket, amount))]
    fn add_equity_delta(&mut self, underlier: &str, bucket: u8, amount: f64) {
        self.inner.add_equity_delta(underlier, bucket, amount);
    }

    /// Add equity repo-rate P&L per one percentage-point rate shift.
    #[pyo3(signature = (underlier, bucket, amount))]
    fn add_equity_repo_delta(&mut self, underlier: &str, bucket: u8, amount: f64) {
        self.inner.add_equity_repo_delta(underlier, bucket, amount);
    }

    /// Add an FX delta for the pair ``(ccy1, ccy2)``: ``amount`` is
    /// base-currency P&L per 1 percentage point move in the exchange rate.
    /// Raises ``ValueError`` for an unknown currency.
    #[pyo3(signature = (ccy1, ccy2, amount))]
    fn add_fx_delta(&mut self, ccy1: &str, ccy2: &str, amount: f64) -> PyResult<()> {
        let c1 = parse_currency(ccy1)?;
        let c2 = parse_currency(ccy2)?;
        self.inner.add_fx_delta(c1, c2, amount);
        Ok(())
    }

    /// Add a commodity delta: ``amount`` is base-currency P&L per 1
    /// percentage point move in the commodity price at ``tenor``; ``bucket``
    /// is the 1-based commodity bucket (MAR21.82).
    #[pyo3(signature = (name, bucket, tenor, basis, amount))]
    fn add_commodity_delta(
        &mut self,
        name: &str,
        bucket: u8,
        tenor: &str,
        basis: &str,
        amount: f64,
    ) {
        self.inner
            .add_commodity_delta(name, bucket, tenor, basis, amount);
    }

    /// Add a commodity vega: ``amount`` is base-currency P&L per unit
    /// implied-volatility move at option ``maturity``.
    #[pyo3(signature = (name, bucket, maturity, amount))]
    fn add_commodity_vega(&mut self, name: &str, bucket: u8, maturity: &str, amount: f64) {
        self.inner
            .add_commodity_vega(name, bucket, maturity, amount);
    }

    /// Add a commodity curvature pair (up / down shocked P&L positions in
    /// base currency).
    #[pyo3(signature = (name, bucket, cvr_up, cvr_down))]
    fn add_commodity_curvature(&mut self, name: &str, bucket: u8, cvr_up: f64, cvr_down: f64) {
        self.inner
            .add_commodity_curvature(name, bucket, cvr_up, cvr_down);
    }

    /// Add a GIRR vega: ``amount`` is base-currency P&L per unit
    /// implied-volatility move for the ``option_maturity`` x
    /// ``underlying_tenor`` point. Raises ``ValueError`` for an unknown
    /// currency.
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

    /// Add an equity vega: ``amount`` is base-currency P&L per unit
    /// implied-volatility move at option ``maturity``.
    #[pyo3(signature = (underlier, bucket, maturity, amount))]
    fn add_equity_vega(&mut self, underlier: &str, bucket: u8, maturity: &str, amount: f64) {
        self.inner
            .add_equity_vega(underlier, bucket, maturity, amount);
    }

    /// Add an FX vega: ``amount`` is base-currency P&L per unit
    /// implied-volatility move at option ``maturity``. Raises ``ValueError``
    /// for an unknown currency.
    #[pyo3(signature = (ccy1, ccy2, maturity, amount))]
    fn add_fx_vega(&mut self, ccy1: &str, ccy2: &str, maturity: &str, amount: f64) -> PyResult<()> {
        let c1 = parse_currency(ccy1)?;
        let c2 = parse_currency(ccy2)?;
        self.inner.add_fx_vega(c1, c2, maturity, amount);
        Ok(())
    }

    /// Add a GIRR curvature pair (up / down shocked P&L positions in base
    /// currency). Raises ``ValueError`` for an unknown currency.
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

    /// Add an equity curvature pair (up / down shocked P&L positions in base
    /// currency).
    #[pyo3(signature = (underlier, bucket, cvr_up, cvr_down))]
    fn add_equity_curvature(&mut self, underlier: &str, bucket: u8, cvr_up: f64, cvr_down: f64) {
        self.inner
            .add_equity_curvature(underlier, bucket, cvr_up, cvr_down);
    }

    /// Add an FX curvature pair (up / down shocked P&L positions in base
    /// currency). Raises ``ValueError`` for an unknown currency.
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

    /// Add a Default Risk Charge position.
    ///
    /// Parameters
    /// ----------
    /// issuer : str
    ///     Issuer identifier; long and short JTD net per issuer at charge time.
    /// jtd_amount : float
    ///     Signed jump-to-default **notional** in base currency (positive =
    ///     long, negative = short), before the seniority LGD.
    /// rating_bucket : int
    ///     Credit-rating bucket, 1 (AAA) to 9 (defaulted) per MAR22.24.
    /// sector : str
    ///     ``"corporate"``, ``"sovereign"`` or ``"local_government"``.
    /// seniority : str
    ///     ``"senior_unsecured"``, ``"subordinated"``, ``"equity"`` or
    ///     ``"covered_bond"`` (selects the LGD).
    /// asset_type : str
    ///     ``"corporate"``, ``"sovereign"``, ``"local_government"`` or ``"equity"``;
    ///     securitization DRC is rejected by the engine.
    /// maturity_years : float
    ///     Finite nonnegative residual maturity; JTD scales by years clipped to [0.25, 1.0].
    /// pnl_adjustment : float, default 0.0
    ///     Mark-to-market adjustment per MAR22.9 (negative for a long
    ///     position carrying an unrealised loss).
    ///
    /// Raises ``ValueError`` for an unknown sector, seniority or asset-type
    /// label.
    #[pyo3(signature = (issuer, jtd_amount, rating_bucket, sector, seniority, asset_type, maturity_years, pnl_adjustment = 0.0))]
    #[allow(clippy::too_many_arguments)]
    fn add_drc_position(
        &mut self,
        issuer: &str,
        jtd_amount: f64,
        rating_bucket: u8,
        sector: &str,
        seniority: &str,
        asset_type: &str,
        maturity_years: f64,
        pnl_adjustment: f64,
    ) -> PyResult<()> {
        self.inner.add_drc_position(DrcPosition {
            maturity_years,
            issuer: issuer.to_string(),
            jtd_amount,
            rating_bucket,
            sector: parse_label::<DrcSector>(sector)?,
            seniority: parse_label::<DrcSeniority>(seniority)?,
            asset_type: parse_label::<DrcAssetType>(asset_type)?,
            pnl_adjustment,
        });
        Ok(())
    }

    /// Add an RRAO (residual risk add-on) position: ``notional`` is the
    /// gross notional in base currency. Set ``is_exotic=True`` for the 1%
    /// weight (exotic underlying), leave as ``False`` for the 0.1% weight
    /// (other residual risk: gap, correlation, behavioural).
    #[pyo3(signature = (instrument_id, notional, is_exotic = false))]
    fn add_rrao_position(&mut self, instrument_id: &str, notional: f64, is_exotic: bool) {
        self.inner
            .add_rrao_position(instrument_id, notional, is_exotic);
    }

    /// Base/reporting currency code.
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
    /// per bucket would give a different schema for every portfolio.
    ///
    /// ``risk_class`` uses the same labels as the ``frtb_sba_charge``
    /// breakdown (``girr``, ``csr_non_sec``, ``csr_sec_ctp``,
    /// ``csr_sec_non_ctp``, ``equity``, ``commodity``, ``fx``), plus ``drc``
    /// and ``rrao`` for the two position lists.
    ///
    /// ``kind`` is ``delta``, ``vega``, ``curvature_up``, ``curvature_down``,
    /// ``inflation_delta``, ``xccy_basis_delta``, ``jtd`` (DRC notional), or
    /// ``exotic_notional`` / ``other_notional`` (RRAO). A curvature pair is
    /// split across two rows so ``amount`` stays scalar.
    ///
    /// ``issuer`` carries the name axis: a currency code for GIRR, a
    /// ``"CCY1/CCY2"`` pair for FX, an issuer, tranche, underlier, commodity
    /// name, or instrument id elsewhere. ``bucket`` is the FRTB bucket index
    /// as a **string** (``pd.to_numeric`` if you need it numeric);
    /// ``tenor`` is the tenor or option maturity, and for GIRR vega the
    /// ``"{option_maturity}/{underlying_tenor}"`` pair. Both are ``None``
    /// where the risk class has no such axis.
    ///
    /// ``amount`` keeps each bucket's own convention: GIRR deltas are
    /// base-currency P&L per **1 percentage point** of curve shift (that is,
    /// ``100 x DV01``), DRC rows are signed JTD notionals before LGD, and
    /// RRAO rows are gross notionals. ``from_dataframe`` accepts this frame
    /// back (except ``drc`` rows).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        table_to_dataframe(py, &self.inner.to_table().map_err(core_to_py)?)
    }

    /// Summarise the container by populated bucket counts.
    fn __repr__(&self) -> String {
        let s = &self.inner;
        let delta = s.girr_delta.len()
            + s.girr_inflation_delta.len()
            + s.girr_xccy_basis_delta.len()
            + s.csr_nonsec_delta.len()
            + s.csr_sec_ctp_delta.len()
            + s.csr_sec_nonctp_delta.len()
            + s.equity_delta.len()
            + s.commodity_delta.len()
            + s.fx_delta.len();
        let vega = s.girr_vega.len()
            + s.csr_nonsec_vega.len()
            + s.csr_sec_ctp_vega.len()
            + s.csr_sec_nonctp_vega.len()
            + s.equity_vega.len()
            + s.commodity_vega.len()
            + s.fx_vega.len();
        let curvature = s.girr_curvature.len()
            + s.csr_nonsec_curvature.len()
            + s.csr_sec_ctp_curvature.len()
            + s.csr_sec_nonctp_curvature.len()
            + s.equity_curvature.len()
            + s.commodity_curvature.len()
            + s.fx_curvature.len();
        format!(
            "FrtbSensitivities(base_currency={}, delta={delta}, vega={vega}, curvature={curvature}, drc={}, rrao={})",
            s.base_currency,
            s.drc_positions.len(),
            s.rrao_exotic_notionals.len(),
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
///
/// Evaluates delta, vega and curvature under each configured correlation
/// scenario, takes the maximum, then adds DRC and RRAO (BCBS d457).
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
    ///
    /// Parameters
    /// ----------
    /// scenarios : list[str] | None
    ///     Correlation scenarios to evaluate (``"low"``, ``"medium"``,
    ///     ``"high"``); the charge is the maximum across them. ``None``
    ///     evaluates all three (the regulatory default).
    /// risk_classes : list[str] | None
    ///     Risk classes whose delta/vega/curvature are included (``"girr"``,
    ///     ``"csr_non_sec"``, ``"csr_sec_ctp"``, ``"csr_sec_non_ctp"``,
    ///     ``"equity"``, ``"commodity"``, ``"fx"``); ``None`` includes all.
    ///
    /// Raises ``ValueError`` for an unknown label or an empty list.
    #[new]
    #[pyo3(signature = (scenarios = None, risk_classes = None))]
    fn new(scenarios: Option<Vec<String>>, risk_classes: Option<Vec<String>>) -> PyResult<Self> {
        let scenarios = match scenarios {
            Some(labels) => labels
                .iter()
                .map(|s| parse_correlation_scenario(s))
                .collect::<PyResult<Vec<_>>>()?,
            None => CorrelationScenario::ALL.to_vec(),
        };
        let risk_classes = match risk_classes {
            Some(labels) => labels
                .iter()
                .map(|s| parse_label::<FrtbRiskClass>(s))
                .collect::<PyResult<Vec<_>>>()?,
            None => FrtbRiskClass::ALL.to_vec(),
        };
        let inner = FrtbSbaEngine::new(scenarios, risk_classes).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Correlation scenario labels evaluated, in configured order.
    #[getter]
    fn scenarios(&self) -> Vec<String> {
        self.inner
            .scenarios()
            .iter()
            .map(|s| scenario_label(*s))
            .collect()
    }

    /// Risk-class labels included, in configured order.
    #[getter]
    fn risk_classes(&self) -> Vec<String> {
        self.inner
            .risk_classes()
            .iter()
            .map(|rc| risk_class_label(*rc))
            .collect()
    }

    /// Calculate the FRTB SBA charge for a sensitivity portfolio.
    ///
    /// Returns a ``FrtbSbaResult`` carrying the total charge, the
    /// per-risk-class delta/vega/curvature breakdown, DRC, RRAO, and the
    /// per-scenario charges with the binding scenario named. Raises
    /// ``ValueError`` if the sensitivities fail validation.
    fn calculate(
        &self,
        py: Python<'_>,
        sensitivities: &PyFrtbSensitivities,
    ) -> PyResult<PyFrtbSbaResult> {
        let result = py
            .detach(|| self.inner.calculate(&sensitivities.inner))
            .map_err(core_to_py)?;
        Ok(PyFrtbSbaResult::from_inner(result))
    }

    fn __repr__(&self) -> String {
        format!(
            "FrtbSbaEngine(scenarios={:?}, risk_classes={:?})",
            self.scenarios(),
            self.risk_classes()
        )
    }
}

/// A single derivative trade for SA-CCR EAD computation (BCBS 279).
///
/// Build with keyword arguments or ``from_json``; both validate the
/// direction / supervisory-delta / option-type coherence up front.
/// ``notional`` and ``mtm`` are in the netting set's reporting currency;
/// ``direction`` is ``+1.0`` (long) or ``-1.0`` (short);
/// ``supervisory_delta`` is in ``[-1, 1]`` (exactly ``±1`` for linear trades,
/// the signed option delta otherwise).
#[pyclass(
    name = "SaCcrTrade",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySaCcrTrade {
    pub(super) inner: SaCcrTrade,
}

const TRADE_COLUMNS: &[ColumnSchema<'_>] = &[
    ("trade_id", "str"),
    ("asset_class", "str"),
    ("notional", "float64"),
    ("start_date", "str"),
    ("end_date", "str"),
    ("underlier", "str"),
    ("hedging_set", "str"),
    ("direction", "float64"),
    ("supervisory_delta", "float64"),
    ("mtm", "float64"),
    ("is_option", "bool"),
    ("option_type", "str"),
    ("supervisory_category", "str"),
    ("option_maturity_date", "str"),
];

impl PySaCcrTrade {
    fn row(&self) -> serde_json::Value {
        let t = &self.inner;
        serde_json::json!({
            "trade_id": t.trade_id,
            "asset_class": asset_class_label(t.asset_class),
            "notional": t.notional,
            "start_date": t.start_date.to_string(),
            "end_date": t.end_date.to_string(),
            "underlier": t.underlier,
            "hedging_set": t.hedging_set,
            "direction": t.direction,
            "supervisory_delta": t.supervisory_delta,
            "mtm": t.mtm,
            "is_option": t.is_option,
            "option_type": t.option_type.map(serde_label),
            "supervisory_category": t.supervisory_category.map(serde_label),
            "option_maturity_date": t.option_maturity_date.map(|d| d.to_string()),
        })
    }
}

#[pymethods]
impl PySaCcrTrade {
    /// Create and validate a trade.
    ///
    /// Parameters
    /// ----------
    /// trade_id : str
    ///     Unique trade identifier.
    /// asset_class : str
    ///     ``"interest_rate"``, ``"foreign_exchange"``, ``"credit"``,
    ///     ``"equity"`` or ``"commodity"``.
    /// notional : float
    ///     Adjusted notional in the reporting currency.
    /// start_date : datetime.date | str
    ///     Trade start date (forward-start trades start after ``as_of``).
    /// end_date : datetime.date | str
    ///     Trade end date / maturity.
    /// underlier : str
    ///     Underlier reference (currency pair, issuer, equity, commodity).
    /// hedging_set : str
    ///     Hedging-set identifier within the asset class.
    /// direction : float
    ///     ``+1.0`` long or ``-1.0`` short.
    /// supervisory_delta : float
    ///     ``±1`` for linear trades; the signed option delta in ``[-1, 1]``
    ///     otherwise, sign-consistent with ``option_type`` (BCBS 279 ¶112).
    /// mtm : float
    ///     Current mark-to-market in the reporting currency.
    /// is_option : bool, default False
    ///     Whether the trade is an option.
    /// option_type : str | None
    ///     ``"call_long"``, ``"call_short"``, ``"put_long"`` or
    ///     ``"put_short"``; required when ``is_option`` is ``True``.
    ///
    /// Raises ``ValueError`` when a label is unknown or the trade fails the
    /// BCBS 279 coherence checks, ``TypeError`` for a non-date-like date.
    #[new]
    #[pyo3(signature = (trade_id, asset_class, notional, start_date, end_date, underlier, hedging_set, direction, supervisory_delta, mtm, is_option = false, option_type = None, supervisory_category = None, option_maturity_date = None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        trade_id: &str,
        asset_class: &str,
        notional: f64,
        start_date: &Bound<'_, PyAny>,
        end_date: &Bound<'_, PyAny>,
        underlier: &str,
        hedging_set: &str,
        direction: f64,
        supervisory_delta: f64,
        mtm: f64,
        is_option: bool,
        option_type: Option<&str>,
        supervisory_category: Option<&str>,
        option_maturity_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let inner = SaCcrTrade {
            supervisory_category: supervisory_category
                .map(parse_label::<SaCcrSupervisoryCategory>)
                .transpose()?,
            option_maturity_date: option_maturity_date.map(extract_date).transpose()?,
            trade_id: trade_id.to_string(),
            asset_class: parse_label::<SaCcrAssetClass>(asset_class)?,
            notional,
            start_date: extract_date(start_date)?,
            end_date: extract_date(end_date)?,
            underlier: underlier.to_string(),
            hedging_set: hedging_set.to_string(),
            direction,
            supervisory_delta,
            mtm,
            is_option,
            option_type: option_type
                .map(parse_label::<SaCcrOptionType>)
                .transpose()?,
        };
        inner.validate().map_err(core_to_py)?;
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

    /// Construct and validate from the canonical `SaCcrTrade` JSON; raises
    /// ``ValueError`` on malformed input or an incoherent trade.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: SaCcrTrade = serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Build one validated trade per row of a trade tape.
    ///
    /// ``frame`` must carry the columns ``to_dataframe`` emits (``trade_id``,
    /// ``asset_class``, ``notional``, ``start_date``, ``end_date``,
    /// ``underlier``, ``hedging_set``, ``direction``, ``supervisory_delta``,
    /// ``mtm``; ``is_option`` and ``option_type`` are optional and default to
    /// a linear trade). Dates may be ISO strings or date-like values.
    /// Returns a list of ``SaCcrTrade``; raises ``ValueError`` naming the
    /// first invalid row.
    #[staticmethod]
    fn from_dataframe(frame: &Bound<'_, PyAny>) -> PyResult<Vec<Self>> {
        records(frame)?
            .iter()
            .map(|row| {
                let is_option = match row.get_item("is_option")? {
                    Some(v) if !v.is_none() => req_bool(row, "is_option")?,
                    _ => false,
                };
                let inner = SaCcrTrade {
                    supervisory_category: opt_str(row, "supervisory_category")?
                        .map(|s| parse_label::<SaCcrSupervisoryCategory>(&s))
                        .transpose()?,
                    option_maturity_date: row
                        .get_item("option_maturity_date")?
                        .filter(|v| !v.is_none())
                        .as_ref()
                        .map(extract_date)
                        .transpose()?,
                    trade_id: req_str(row, "trade_id")?,
                    asset_class: parse_label::<SaCcrAssetClass>(&req_str(row, "asset_class")?)?,
                    notional: req_f64(row, "notional")?,
                    start_date: req_date(row, "start_date")?,
                    end_date: req_date(row, "end_date")?,
                    underlier: req_str(row, "underlier")?,
                    hedging_set: req_str(row, "hedging_set")?,
                    direction: req_f64(row, "direction")?,
                    supervisory_delta: req_f64(row, "supervisory_delta")?,
                    mtm: req_f64(row, "mtm")?,
                    is_option,
                    option_type: opt_str(row, "option_type")?
                        .map(|label| parse_label::<SaCcrOptionType>(&label))
                        .transpose()?,
                };
                inner.validate().map_err(core_to_py)?;
                Ok(Self { inner })
            })
            .collect()
    }

    /// Export the trade as a single-row pandas ``DataFrame`` with the
    /// columns ``from_dataframe`` reads; dates are ISO 8601 strings and
    /// ``option_type`` is null for linear trades.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &[self.row()], TRADE_COLUMNS)
    }

    /// Trade identifier.
    #[getter]
    fn trade_id(&self) -> &str {
        &self.inner.trade_id
    }

    /// SA-CCR asset class label (``interest_rate``, ``foreign_exchange``,
    /// ``credit``, ``equity``, or ``commodity``).
    #[getter]
    fn asset_class(&self) -> String {
        asset_class_label(self.inner.asset_class)
    }

    /// Supervisory category wire label, or None for IR and FX.
    #[getter]
    fn supervisory_category(&self) -> Option<String> {
        self.inner.supervisory_category.map(serde_label)
    }

    /// Option expiry as a date, distinct from the underlying end date.
    #[getter]
    fn option_maturity_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .option_maturity_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Trade notional, in the netting set's reporting currency.
    #[getter]
    fn notional(&self) -> f64 {
        self.inner.notional
    }

    /// Trade start date as ``datetime.date``.
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }

    /// Trade end date / maturity as ``datetime.date``.
    #[getter]
    fn end_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end_date)
    }

    /// Underlier reference.
    #[getter]
    fn underlier(&self) -> &str {
        &self.inner.underlier
    }

    /// Hedging-set identifier within the asset class.
    #[getter]
    fn hedging_set(&self) -> &str {
        &self.inner.hedging_set
    }

    /// ``+1.0`` for long, ``-1.0`` for short.
    #[getter]
    fn direction(&self) -> f64 {
        self.inner.direction
    }

    /// Supervisory delta in ``[-1, 1]``.
    #[getter]
    fn supervisory_delta(&self) -> f64 {
        self.inner.supervisory_delta
    }

    /// Current mark-to-market value, in the netting set's reporting currency.
    #[getter]
    fn mtm(&self) -> f64 {
        self.inner.mtm
    }

    /// Whether the trade is an option.
    #[getter]
    fn is_option(&self) -> bool {
        self.inner.is_option
    }

    /// Option type label (``call_long``, ``call_short``, ``put_long``,
    /// ``put_short``) or ``None`` for a linear trade.
    #[getter]
    fn option_type(&self) -> Option<String> {
        self.inner.option_type.map(serde_label)
    }

    fn __repr__(&self) -> String {
        format!(
            "SaCcrTrade(trade_id={:?}, asset_class={:?}, notional={:.2}, start_date={}, end_date={}, direction={}, supervisory_delta={}, mtm={:.2}, is_option={})",
            self.inner.trade_id,
            asset_class_label(self.inner.asset_class),
            self.inner.notional,
            self.inner.start_date,
            self.inner.end_date,
            self.inner.direction,
            self.inner.supervisory_delta,
            self.inner.mtm,
            py_bool(self.inner.is_option),
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
}

/// SA-CCR netting-set configuration.
///
/// Collateral terms that select the margined or unmargined RC/PFE formulas.
/// All amounts are in the netting set's reporting currency; ``mpor_days`` is
/// the margin period of risk in business days.
#[pyclass(
    name = "SaCcrNettingSetConfig",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySaCcrNettingSetConfig {
    pub(super) inner: SaCcrNettingSetConfig,
}

#[pymethods]
impl PySaCcrNettingSetConfig {
    /// Create an unmargined netting set configuration.
    ///
    /// Parameters
    /// ----------
    /// netting_set_id : NettingSetId
    ///     Bilateral (``NettingSetId.bilateral``) or cleared
    ///     (``NettingSetId.cleared``) netting-set key.
    /// collateral : float
    ///     Net collateral held (positive = bank holds collateral).
    /// as_of : datetime.date | str
    ///     Valuation date for forward-start and remaining-maturity
    ///     calculations.
    ///
    /// Raises ``ValueError`` for a non-finite amount or a non-ISO date
    /// string; ``TypeError`` for a non-date-like ``as_of``.
    #[staticmethod]
    #[pyo3(signature = (netting_set_id, collateral, as_of))]
    fn unmargined(
        netting_set_id: &Bound<'_, PyAny>,
        collateral: f64,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let inner = SaCcrNettingSetConfig::unmargined(
            extract_netting_set_id(netting_set_id)?,
            collateral,
            extract_date(as_of)?,
        );
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a margined netting set configuration.
    ///
    /// Parameters
    /// ----------
    /// netting_set_id : NettingSetId
    ///     Bilateral or cleared netting-set key.
    /// collateral : float
    ///     Net collateral held (positive = bank holds collateral).
    /// threshold : float
    ///     CSA threshold (TH), non-negative.
    /// mta : float
    ///     Minimum transfer amount, non-negative.
    /// nica : float
    ///     Net independent collateral amount, signed.
    /// mpor_days : int
    ///     Margin period of risk in business days; must be positive
    ///     (10 bilateral, 5 cleared under BCBS 279).
    /// as_of : datetime.date | str
    ///     Valuation date for forward-start and remaining-maturity
    ///     calculations.
    ///
    /// Raises ``ValueError`` if an amount is non-finite, threshold or MTA is
    /// negative, ``mpor_days`` is below ten bilateral or five cleared business days, or the date string is not ISO 8601.
    #[staticmethod]
    #[pyo3(signature = (netting_set_id, collateral, threshold, mta, nica, mpor_days, as_of))]
    fn margined(
        netting_set_id: &Bound<'_, PyAny>,
        collateral: f64,
        threshold: f64,
        mta: f64,
        nica: f64,
        mpor_days: u32,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let inner = SaCcrNettingSetConfig::margined(
            extract_netting_set_id(netting_set_id)?,
            collateral,
            threshold,
            mta,
            nica,
            mpor_days,
            extract_date(as_of)?,
        );
        inner.validate().map_err(core_to_py)?;
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

    /// Construct from a JSON serialization of `SaCcrNettingSetConfig`;
    /// raises ``ValueError`` on malformed input.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: SaCcrNettingSetConfig = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Validate the collateral and margin-agreement terms; raises
    /// ``ValueError`` on a non-finite amount, negative threshold/MTA, or a
    /// margined set with zero MPOR.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Valuation date as ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    /// Netting-set key.
    #[getter]
    fn netting_set_id(&self) -> PyNettingSetId {
        PyNettingSetId {
            inner: self.inner.netting_set_id.clone(),
        }
    }

    /// Whether the netting set is margined (a CSA with an MPoR applies).
    #[getter]
    fn is_margined(&self) -> bool {
        self.inner.is_margined
    }

    /// Net collateral held against the netting set, in its reporting currency.
    #[getter]
    fn collateral(&self) -> f64 {
        self.inner.collateral
    }

    /// CSA threshold (TH) in the reporting currency.
    #[getter]
    fn threshold(&self) -> f64 {
        self.inner.threshold
    }

    /// Minimum transfer amount in the reporting currency.
    #[getter]
    fn mta(&self) -> f64 {
        self.inner.mta
    }

    /// Net independent collateral amount in the reporting currency.
    #[getter]
    fn nica(&self) -> f64 {
        self.inner.nica
    }

    /// Margin period of risk in business days.
    #[getter]
    fn mpor_days(&self) -> u32 {
        self.inner.mpor_days
    }

    fn __repr__(&self) -> String {
        format!(
            "SaCcrNettingSetConfig(netting_set_id={}, as_of={}, is_margined={}, collateral={:.2}, threshold={:.2}, mta={:.2}, nica={:.2}, mpor_days={})",
            self.inner.netting_set_id,
            self.inner.as_of,
            py_bool(self.inner.is_margined),
            self.inner.collateral,
            self.inner.threshold,
            self.inner.mta,
            self.inner.nica,
            self.inner.mpor_days,
        )
    }
}

/// SA-CCR engine (BCBS 279): ``EAD = alpha * (RC + PFE)``.
///
/// All monetary inputs must share one currency; the engine performs no
/// conversion.
#[pyclass(
    name = "SaCcrEngine",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
pub struct PySaCcrEngine {
    inner: SaCcrEngine,
}

fn build_engine(alpha: Option<f64>) -> PyResult<SaCcrEngine> {
    match alpha {
        Some(a) => SaCcrEngine::with_alpha(a).map_err(core_to_py),
        None => Ok(SaCcrEngine::default()),
    }
}

#[pymethods]
impl PySaCcrEngine {
    /// Create an SA-CCR engine; ``alpha`` overrides the regulatory 1.4
    /// (must be finite and at least 1.0, else ``ValueError``).
    #[new]
    #[pyo3(signature = (alpha = None))]
    fn new(alpha: Option<f64>) -> PyResult<Self> {
        Ok(Self {
            inner: build_engine(alpha)?,
        })
    }

    /// Alpha multiplier applied to ``RC + PFE``.
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha()
    }

    /// Calculate SA-CCR EAD for a netting set and trade list.
    ///
    /// Returns an ``EadResult`` carrying EAD, RC, PFE, the multiplier, the
    /// aggregate and per-asset-class add-ons, alpha, and the maturity factor.
    /// Raises ``ValueError`` if the configuration or a trade fails validation.
    fn calculate_ead(
        &self,
        py: Python<'_>,
        config: &PySaCcrNettingSetConfig,
        trades: Vec<PyRef<'_, PySaCcrTrade>>,
    ) -> PyResult<PyEadResult> {
        let trade_vec: Vec<SaCcrTrade> = trades.iter().map(|t| t.inner.clone()).collect();
        let result = py
            .detach(|| self.inner.calculate_ead(&config.inner, &trade_vec))
            .map_err(core_to_py)?;
        Ok(PyEadResult::from_inner(result))
    }

    fn __repr__(&self) -> String {
        format!("SaCcrEngine(alpha={})", self.inner.alpha())
    }
}

/// FRTB SBA capital-charge result (BCBS d457).
///
/// Returned by ``frtb_sba_charge`` and ``FrtbSbaEngine.calculate``.
/// Carries the headline charge, the per-risk-class delta/vega/curvature
/// breakdown, the default risk charge, the residual risk add-on, and the
/// charge under each correlation scenario together with the binding one.
#[pyclass(
    name = "FrtbSbaResult",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFrtbSbaResult {
    inner: FrtbSbaResult,
}

impl PyFrtbSbaResult {
    fn from_inner(inner: FrtbSbaResult) -> Self {
        Self { inner }
    }

    fn labeled(map: &BTreeMap<FrtbRiskClass, f64>) -> BTreeMap<String, f64> {
        map.iter()
            .map(|(rc, v)| (risk_class_label(*rc), *v))
            .collect()
    }
}

#[pymethods]
impl PyFrtbSbaResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from the JSON produced by ``to_json``; raises
    /// ``ValueError`` on malformed input.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: FrtbSbaResult = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize back to the same JSON shape ``from_json`` accepts.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Total capital charge, in the sensitivity portfolio's base currency.
    #[getter]
    fn total(&self) -> f64 {
        self.inner.total
    }

    /// Default Risk Charge (credit + equity jump-to-default).
    #[getter]
    fn drc(&self) -> f64 {
        self.inner.drc
    }

    /// Residual Risk Add-On.
    #[getter]
    fn rrao(&self) -> f64 {
        self.inner.rrao
    }

    /// Correlation scenario that produced the binding charge
    /// (``"low"``, ``"medium"``, or ``"high"``).
    #[getter]
    fn binding_scenario(&self) -> String {
        scenario_label(self.inner.binding_scenario)
    }

    /// Delta risk charge keyed by risk-class wire label (e.g. ``"girr"``).
    #[getter]
    fn delta_by_risk_class(&self) -> BTreeMap<String, f64> {
        Self::labeled(&self.inner.delta_by_risk_class)
    }

    /// Vega risk charge keyed by risk-class wire label.
    #[getter]
    fn vega_by_risk_class(&self) -> BTreeMap<String, f64> {
        Self::labeled(&self.inner.vega_by_risk_class)
    }

    /// Curvature risk charge keyed by risk-class wire label.
    #[getter]
    fn curvature_by_risk_class(&self) -> BTreeMap<String, f64> {
        Self::labeled(&self.inner.curvature_by_risk_class)
    }

    /// Delta+vega+curvature charge under each evaluated correlation scenario.
    #[getter]
    fn scenario_charges(&self) -> BTreeMap<String, f64> {
        self.inner
            .scenario_charges
            .iter()
            .map(|(s, v)| (scenario_label(*s), *v))
            .collect()
    }

    /// Policy metadata stamped by the computing layer as a dict: numeric
    /// mode, active rounding context, any applied FX policy and the
    /// parallel-execution flag.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Export the headline charge as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``total``, ``drc``, ``rrao`` (floats in the portfolio's base
    /// currency) and ``binding_scenario`` (string). Per-risk-class detail lives
    /// in ``to_breakdown_dataframe``.
    ///
    /// One row, so a book of desks stacks with
    /// ``pd.concat([r.to_dataframe() for r in results])``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "total": self.inner.total,
            "drc": self.inner.drc,
            "rrao": self.inner.rrao,
            "binding_scenario": self.binding_scenario(),
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &["total", "drc", "rrao", "binding_scenario"],
        )
    }

    /// Export the per-risk-class breakdown as a long-format pandas ``DataFrame``.
    ///
    /// Columns: ``component`` (``"delta"``, ``"vega"``, or ``"curvature"``),
    /// ``risk_class`` (wire label, e.g. ``"girr"``), and ``charge`` (float in
    /// the portfolio's base currency). Rows are emitted component-major and
    /// risk-class sorted so repeated runs are byte-identical.
    ///
    /// The components do not sum to ``total``: SBA aggregates them with
    /// prescribed correlations, and ``drc``/``rrao`` sit outside this frame.
    #[pyo3(text_signature = "($self)")]
    fn to_breakdown_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        const COLUMNS: &[ColumnSchema<'_>] = &[
            ("component", "str"),
            ("risk_class", "str"),
            ("charge", "float64"),
        ];
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for (component, map) in [
            ("delta", &self.inner.delta_by_risk_class),
            ("vega", &self.inner.vega_by_risk_class),
            ("curvature", &self.inner.curvature_by_risk_class),
        ] {
            for (rc, charge) in map {
                rows.push(serde_json::json!({
                    "component": component,
                    "risk_class": risk_class_label(*rc),
                    "charge": charge,
                }));
            }
        }
        serde_rows_to_dataframe_with_schema(py, &rows, COLUMNS)
    }

    /// Export the per-scenario SBA charges as a pandas ``DataFrame``.
    ///
    /// Columns: ``scenario`` (``"low"``, ``"medium"``, ``"high"``),
    /// ``charge`` (delta+vega+curvature under that scenario, float in the
    /// portfolio's base currency) and ``binding`` (``True`` on the scenario
    /// that produced ``total``). One row per evaluated scenario in
    /// low/medium/high order.
    #[pyo3(text_signature = "($self)")]
    fn to_scenario_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        const COLUMNS: &[ColumnSchema<'_>] = &[
            ("scenario", "str"),
            ("charge", "float64"),
            ("binding", "bool"),
        ];
        let rows: Vec<serde_json::Value> = self
            .inner
            .scenario_charges
            .iter()
            .map(|(scenario, charge)| {
                serde_json::json!({
                    "scenario": scenario_label(*scenario),
                    "charge": charge,
                    "binding": *scenario == self.inner.binding_scenario,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, COLUMNS)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "FrtbSbaResult(total={:.2}, drc={:.2}, rrao={:.2}, binding_scenario={:?})",
            self.inner.total,
            self.inner.drc,
            self.inner.rrao,
            self.binding_scenario()
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
}

/// SA-CCR Exposure at Default result (BCBS 279).
///
/// Returned by ``saccr_ead`` and ``SaCcrEngine.calculate_ead``.
/// ``ead == alpha * (rc + pfe)``; ``pfe == multiplier * add_on_aggregate``.
#[pyclass(
    name = "EadResult",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEadResult {
    inner: EadResult,
}

impl PyEadResult {
    fn from_inner(inner: EadResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyEadResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from the JSON produced by ``to_json``; raises
    /// ``ValueError`` on malformed input.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: EadResult = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize back to the same JSON shape ``from_json`` accepts.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Exposure at Default: ``alpha * (rc + pfe)``, in the netting set's
    /// reporting currency.
    #[getter]
    fn ead(&self) -> f64 {
        self.inner.ead
    }

    /// Replacement cost component.
    #[getter]
    fn rc(&self) -> f64 {
        self.inner.rc
    }

    /// Potential future exposure: ``multiplier * add_on_aggregate``.
    #[getter]
    fn pfe(&self) -> f64 {
        self.inner.pfe
    }

    /// PFE multiplier, which recognises over-collateralization and negative
    /// mark-to-market (1.0 when neither applies, floored at 0.05).
    #[getter]
    fn multiplier(&self) -> f64 {
        self.inner.multiplier
    }

    /// Aggregate add-on across asset classes, before the multiplier.
    #[getter]
    fn add_on_aggregate(&self) -> f64 {
        self.inner.add_on_aggregate
    }

    /// Alpha multiplier (1.4 per BCBS 279 unless overridden on the engine).
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }

    /// Maturity factor applied to the netting set.
    #[getter]
    fn maturity_factor(&self) -> f64 {
        self.inner.maturity_factor
    }

    /// Add-on keyed by asset-class wire label (e.g. ``"interest_rate"``).
    #[getter]
    fn add_on_by_asset_class(&self) -> BTreeMap<String, f64> {
        self.inner
            .add_on_by_asset_class
            .iter()
            .map(|(ac, v)| (asset_class_label(*ac), *v))
            .collect()
    }

    /// Policy metadata stamped by the computing layer as a dict: numeric
    /// mode, active rounding context, any applied FX policy and the
    /// parallel-execution flag.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Export the headline exposure as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``ead``, ``rc``, ``pfe``, ``multiplier``, ``add_on_aggregate``
    /// (floats in the netting set's reporting currency), ``alpha`` and
    /// ``maturity_factor`` (dimensionless floats). Per-asset-class add-on
    /// detail lives in ``to_add_on_dataframe``.
    ///
    /// One row, so a book of netting sets stacks with
    /// ``pd.concat([r.to_dataframe() for r in results])``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "ead": self.inner.ead,
            "rc": self.inner.rc,
            "pfe": self.inner.pfe,
            "multiplier": self.inner.multiplier,
            "add_on_aggregate": self.inner.add_on_aggregate,
            "alpha": self.inner.alpha,
            "maturity_factor": self.inner.maturity_factor,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "ead",
                "rc",
                "pfe",
                "multiplier",
                "add_on_aggregate",
                "alpha",
                "maturity_factor",
            ],
        )
    }

    /// Export the per-asset-class add-on as a pandas ``DataFrame``.
    ///
    /// Columns: ``asset_class`` (wire label) and ``add_on`` (float in the
    /// netting set's reporting currency). One row per asset class present,
    /// sorted by ``asset_class`` so repeated runs are byte-identical. A netting
    /// set with no trades yields a zero-row frame that still carries both
    /// columns with their real dtypes.
    #[pyo3(text_signature = "($self)")]
    fn to_add_on_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        const COLUMNS: &[ColumnSchema<'_>] = &[("asset_class", "str"), ("add_on", "float64")];
        let rows: Vec<serde_json::Value> = self
            .inner
            .add_on_by_asset_class
            .iter()
            .map(|(ac, add_on)| {
                serde_json::json!({
                    "asset_class": asset_class_label(*ac),
                    "add_on": add_on,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, COLUMNS)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EadResult(ead={:.2}, rc={:.2}, pfe={:.2}, alpha={:.2})",
            self.inner.ead, self.inner.rc, self.inner.pfe, self.inner.alpha
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
}

/// Compute the FRTB SBA capital charge.
///
/// Returns a ``FrtbSbaResult`` exposing ``total``, ``drc``, ``rrao``,
/// ``binding_scenario``, ``scenario_charges``, and the per-risk-class
/// ``delta_by_risk_class`` / ``vega_by_risk_class`` /
/// ``curvature_by_risk_class`` breakdowns, plus ``to_dataframe`` /
/// ``to_breakdown_dataframe`` / ``to_scenario_dataframe`` exits.
///
/// If ``correlation_scenario`` is provided (``"low"``, ``"medium"``, ``"high"``),
/// only that scenario is evaluated. Otherwise all three are run and the
/// max is taken per BCBS d457. Raises ``ValueError`` for an unknown scenario
/// label or sensitivities that fail validation.
#[pyfunction]
#[pyo3(signature = (sensitivities, correlation_scenario = None))]
pub fn frtb_sba_charge(
    py: Python<'_>,
    sensitivities: &PyFrtbSensitivities,
    correlation_scenario: Option<&str>,
) -> PyResult<PyFrtbSbaResult> {
    let engine = match correlation_scenario {
        Some(s) => {
            let scenario = parse_correlation_scenario(s)?;
            FrtbSbaEngine::new(vec![scenario], FrtbRiskClass::ALL.to_vec()).map_err(core_to_py)?
        }
        None => FrtbSbaEngine::default(),
    };
    let result = py
        .detach(|| engine.calculate(&sensitivities.inner))
        .map_err(core_to_py)?;

    Ok(PyFrtbSbaResult::from_inner(result))
}

/// Compute SA-CCR Exposure at Default for a netting set of trades.
///
/// Thin wrapper over ``SaCcrEngine.calculate_ead``: builds the engine with
/// the regulatory alpha of 1.4 (or ``alpha`` when supplied) and prices
/// ``trades`` under ``config``.
///
/// Parameters
/// ----------
/// trades : list[SaCcrTrade]
///     Derivative trades in the netting set (may be empty, giving zero EAD).
/// config : SaCcrNettingSetConfig
///     Netting-set collateral, threshold, MTA, NICA, MPoR and valuation date
///     (``SaCcrNettingSetConfig.unmargined`` / ``margined``).
/// alpha : float | None
///     Supervisory alpha override; must be finite and at least 1.0.
///
/// Returns an ``EadResult`` per BCBS 279. Raises ``ValueError`` if ``alpha``
/// is invalid or the configuration or a trade fails validation.
#[pyfunction]
#[pyo3(signature = (trades, config, alpha = None))]
pub fn saccr_ead(
    py: Python<'_>,
    trades: Vec<PyRef<'_, PySaCcrTrade>>,
    config: &PySaCcrNettingSetConfig,
    alpha: Option<f64>,
) -> PyResult<PyEadResult> {
    let engine = build_engine(alpha)?;
    let trade_vec: Vec<SaCcrTrade> = trades.iter().map(|t| t.inner.clone()).collect();
    let result = py
        .detach(|| engine.calculate_ead(&config.inner, &trade_vec))
        .map_err(core_to_py)?;
    Ok(PyEadResult::from_inner(result))
}

/// Register FRTB / SA-CCR classes and functions on the margin module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFrtbSensitivities>()?;
    m.add_class::<PyFrtbSbaEngine>()?;
    m.add_class::<PyFrtbSbaResult>()?;
    m.add_class::<PyEadResult>()?;
    m.add_class::<PySaCcrTrade>()?;
    m.add_class::<PySaCcrNettingSetConfig>()?;
    m.add_class::<PySaCcrEngine>()?;
    m.add_function(pyo3::wrap_pyfunction!(frtb_sba_charge, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(saccr_ead, m)?)?;
    Ok(())
}
