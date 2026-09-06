//! Python wrappers for margin domain types and enums.

use crate::bindings::module_utils::parse_currency;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, ColumnSchema};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_core::money::Money;
use finstack_quant_margin as fm;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Initial margin calculation methodology.
///
/// Immutable, hashable enum-style wrapper. Build one with a class factory
/// (``ImMethodology.simm()``) or parse the lower-case wire label with
/// ``from_str``; ``str()`` renders that same label.
#[pyclass(
    name = "ImMethodology",
    module = "finstack_quant.margin",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyImMethodology {
    pub(super) inner: fm::ImMethodology,
}

#[pymethods]
impl PyImMethodology {
    /// Haircut-based IM (repos and securities financing).
    #[staticmethod]
    fn haircut() -> Self {
        Self {
            inner: fm::ImMethodology::Haircut,
        }
    }

    /// ISDA SIMM (sensitivities-based, OTC derivatives).
    #[staticmethod]
    fn simm() -> Self {
        Self {
            inner: fm::ImMethodology::Simm,
        }
    }

    /// BCBS-IOSCO regulatory schedule approach.
    #[staticmethod]
    fn schedule() -> Self {
        Self {
            inner: fm::ImMethodology::Schedule,
        }
    }

    /// Internal model approved by regulator.
    #[staticmethod]
    fn internal_model() -> Self {
        Self {
            inner: fm::ImMethodology::InternalModel,
        }
    }

    /// Clearing house (CCP-specific) methodology.
    #[staticmethod]
    fn clearing_house() -> Self {
        Self {
            inner: fm::ImMethodology::ClearingHouse,
        }
    }

    /// Parse the lower-case wire label (``"simm"``, ``"schedule"``,
    /// ``"haircut"``, ``"internal_model"``, ``"clearing_house"``).
    ///
    /// Raises ``ValueError`` for any other spelling, including ``"SIMM"``.
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: fm::ImMethodology = s
            .parse()
            .map_err(|e: String| crate::errors::value_error(e))?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!("ImMethodology({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Margin call frequency.
///
/// Immutable, hashable enum-style wrapper; ``from_str`` parses the lower-case
/// wire label (``"daily"``, ``"weekly"``, ``"monthly"``, ``"on_demand"``).
#[pyclass(
    name = "MarginTenor",
    module = "finstack_quant.margin",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyMarginTenor {
    pub(super) inner: fm::MarginTenor,
}

#[pymethods]
impl PyMarginTenor {
    /// Daily margin calls (standard for OTC derivatives post-2016).
    #[staticmethod]
    fn daily() -> Self {
        Self {
            inner: fm::MarginTenor::Daily,
        }
    }

    /// Weekly margin calls.
    #[staticmethod]
    fn weekly() -> Self {
        Self {
            inner: fm::MarginTenor::Weekly,
        }
    }

    /// Monthly margin calls.
    #[staticmethod]
    fn monthly() -> Self {
        Self {
            inner: fm::MarginTenor::Monthly,
        }
    }

    /// On-demand margin calls.
    #[staticmethod]
    fn on_demand() -> Self {
        Self {
            inner: fm::MarginTenor::OnDemand,
        }
    }

    /// Parse the lower-case wire label; raises ``ValueError`` otherwise.
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: fm::MarginTenor = s
            .parse()
            .map_err(|e: String| crate::errors::value_error(e))?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!("MarginTenor({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Type of margin call.
///
/// Immutable, hashable enum-style wrapper; ``from_str`` parses the lower-case
/// wire label (``"initial_margin"``, ``"variation_margin_post"``,
/// ``"variation_margin_collect"``, ``"top_up"``, ``"substitution"``) and
/// ``str()`` renders it. This is the ``call_type`` column of
/// ``VmCalculator.generate_margin_calls``.
#[pyclass(
    name = "MarginCallType",
    module = "finstack_quant.margin",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyMarginCallType {
    pub(super) inner: fm::MarginCallType,
}

#[pymethods]
impl PyMarginCallType {
    /// Initial margin posting requirement.
    #[staticmethod]
    fn initial_margin() -> Self {
        Self {
            inner: fm::MarginCallType::InitialMargin,
        }
    }

    /// Variation margin delivery (margin to be posted).
    #[staticmethod]
    fn variation_margin_post() -> Self {
        Self {
            inner: fm::MarginCallType::VariationMarginPost,
        }
    }

    /// Variation margin return (margin to be received back).
    #[staticmethod]
    fn variation_margin_collect() -> Self {
        Self {
            inner: fm::MarginCallType::VariationMarginCollect,
        }
    }

    /// Top-up margin call.
    #[staticmethod]
    fn top_up() -> Self {
        Self {
            inner: fm::MarginCallType::TopUp,
        }
    }

    /// Collateral substitution request.
    #[staticmethod]
    fn substitution() -> Self {
        Self {
            inner: fm::MarginCallType::Substitution,
        }
    }

    /// Parse the lower-case wire label; raises ``ValueError`` otherwise.
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: fm::MarginCallType = s
            .parse()
            .map_err(|e: String| crate::errors::value_error(e))?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!("MarginCallType({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Clearing status for OTC derivatives.
#[pyclass(
    name = "ClearingStatus",
    module = "finstack_quant.margin",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyClearingStatus {
    pub(super) inner: fm::ClearingStatus,
}

#[pymethods]
impl PyClearingStatus {
    /// Bilateral (uncleared) trade governed by CSA.
    #[staticmethod]
    fn bilateral() -> Self {
        Self {
            inner: fm::ClearingStatus::Bilateral,
        }
    }

    /// Trade cleared through a CCP.
    #[staticmethod]
    fn cleared(ccp: &str) -> Self {
        Self {
            inner: fm::ClearingStatus::Cleared {
                ccp: ccp.to_string(),
            },
        }
    }

    /// Whether this is a bilateral trade.
    #[getter]
    fn is_bilateral(&self) -> bool {
        matches!(self.inner, fm::ClearingStatus::Bilateral)
    }

    /// Whether this is a cleared trade.
    #[getter]
    fn is_cleared(&self) -> bool {
        matches!(self.inner, fm::ClearingStatus::Cleared { .. })
    }

    fn __repr__(&self) -> String {
        format!("ClearingStatus({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Collateral asset class per BCBS-IOSCO standards.
///
/// ``from_str`` parses the lower-case wire label (``"cash"``,
/// ``"government_bonds"``, ``"agency_bonds"``, ``"covered_bonds"``,
/// ``"corporate_bonds"``, ``"equity"``, ``"gold"``, ``"mutual_funds"``).
#[pyclass(
    name = "CollateralAssetClass",
    module = "finstack_quant.margin",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyCollateralAssetClass {
    pub(super) inner: fm::CollateralAssetClass,
}

#[pymethods]
impl PyCollateralAssetClass {
    /// Cash in an eligible currency.
    #[staticmethod]
    fn cash() -> Self {
        Self {
            inner: fm::CollateralAssetClass::Cash,
        }
    }

    /// Sovereign government bonds.
    #[staticmethod]
    fn government_bonds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::GovernmentBonds,
        }
    }

    /// Debt issued or guaranteed by eligible agencies.
    #[staticmethod]
    fn agency_bonds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::AgencyBonds,
        }
    }

    /// Covered bonds meeting the applicable eligibility criteria.
    #[staticmethod]
    fn covered_bonds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::CoveredBonds,
        }
    }

    /// Investment-grade corporate bonds.
    #[staticmethod]
    fn corporate_bonds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::CorporateBonds,
        }
    }

    /// Eligible listed equity.
    #[staticmethod]
    fn equity() -> Self {
        Self {
            inner: fm::CollateralAssetClass::Equity,
        }
    }

    /// Eligible gold collateral.
    #[staticmethod]
    fn gold() -> Self {
        Self {
            inner: fm::CollateralAssetClass::Gold,
        }
    }

    /// Eligible mutual funds or exchange-traded funds.
    #[staticmethod]
    fn mutual_funds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::MutualFunds,
        }
    }

    /// Parse the lower-case wire label; raises ``ValueError`` otherwise.
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: fm::CollateralAssetClass = s.parse().map_err(crate::errors::value_error)?;
        Ok(Self { inner })
    }

    /// BCBS-IOSCO standard haircut for this asset class, as a decimal
    /// fraction (``0.02`` = 2%). Raises ``ValueError`` if the registry has no
    /// entry for the class.
    fn standard_haircut(&self) -> PyResult<f64> {
        self.inner.standard_haircut().map_err(core_to_py)
    }

    /// FX haircut add-on for currency mismatch, as a decimal fraction.
    /// Raises ``ValueError`` if the registry has no entry for the class.
    fn fx_addon(&self) -> PyResult<f64> {
        self.inner.fx_addon().map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!("CollateralAssetClass({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Parse a ``CollateralAssetClass | str`` argument.
pub(super) fn extract_asset_class(obj: &Bound<'_, PyAny>) -> PyResult<fm::CollateralAssetClass> {
    if let Ok(wrapped) = obj.cast::<PyCollateralAssetClass>() {
        return Ok(wrapped.borrow().inner.clone());
    }
    let label: String = obj.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected a CollateralAssetClass or its lower-case wire label",
        )
    })?;
    label.parse().map_err(crate::errors::value_error)
}

/// Identifies a margin netting set.
///
/// Immutable, hashable and comparable, so a netting-set id can key a dict or
/// group a DataFrame. Build bilateral ids from a counterparty and CSA, or
/// cleared ids from a CCP; ``to_json`` / ``from_json`` and pickle round-trip
/// the canonical wire form.
#[pyclass(
    name = "NettingSetId",
    module = "finstack_quant.margin",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyNettingSetId {
    pub(super) inner: fm::NettingSetId,
}

#[pymethods]
impl PyNettingSetId {
    /// Create a bilateral netting set keyed by counterparty and CSA ids.
    #[staticmethod]
    fn bilateral(counterparty_id: &str, csa_id: &str) -> Self {
        Self {
            inner: fm::NettingSetId::bilateral(counterparty_id, csa_id),
        }
    }

    /// Create a cleared netting set keyed by the CCP id.
    #[staticmethod]
    fn cleared(ccp_id: &str) -> Self {
        Self {
            inner: fm::NettingSetId::cleared(ccp_id),
        }
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

    /// Deserialize from the JSON produced by ``to_json``; raises
    /// ``ValueError`` on malformed input.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: fm::NettingSetId = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to the canonical JSON wire form.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Whether this is a cleared netting set.
    #[getter]
    fn is_cleared(&self) -> bool {
        self.inner.is_cleared()
    }

    /// Counterparty identifier. For cleared netting sets this is the
    /// CCP id; for bilateral, the explicit counterparty id.
    #[getter]
    fn counterparty_id(&self) -> &str {
        self.inner.counterparty_id()
    }

    /// CSA identifier when bilateral; `None` for cleared netting sets.
    #[getter]
    fn csa_id(&self) -> Option<&str> {
        self.inner.csa_id()
    }

    /// CCP identifier when cleared; `None` for bilateral netting sets.
    #[getter]
    fn ccp_id(&self) -> Option<&str> {
        self.inner.ccp_id()
    }

    fn __repr__(&self) -> String {
        format!("NettingSetId({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Parse a ``NettingSetId`` argument.
pub(super) fn extract_netting_set_id(obj: &Bound<'_, PyAny>) -> PyResult<fm::NettingSetId> {
    obj.cast::<PyNettingSetId>()
        .map(|wrapped| wrapped.borrow().inner.clone())
        .map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "expected a NettingSetId (NettingSetId.bilateral(...) or \
                 NettingSetId.cleared(...)), got {}",
                obj.get_type()
                    .name()
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            ))
        })
}

/// Parse an ``ImMethodology | str`` argument.
fn extract_im_methodology(obj: &Bound<'_, PyAny>) -> PyResult<fm::ImMethodology> {
    if let Ok(wrapped) = obj.cast::<PyImMethodology>() {
        return Ok(wrapped.borrow().inner);
    }
    let label: String = obj.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected an ImMethodology or its lower-case wire label (e.g. \"simm\")",
        )
    })?;
    label
        .parse()
        .map_err(|e: String| crate::errors::value_error(e))
}

/// Credit Support Annex specification (ISDA standard).
///
/// Build one from the registry presets (``usd_regulatory``,
/// ``eur_regulatory``, ``regulatory(currency, id, collateral_curve)``), then
/// adjust legacy bilateral terms with ``with_vm_threshold`` / ``with_im``.
/// Every commercial term is readable through typed getters; amounts are
/// floats in ``base_currency``.
#[pyclass(
    name = "CsaSpec",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCsaSpec {
    pub(super) inner: fm::CsaSpec,
}

impl PyCsaSpec {
    fn base_money(&self, amount: f64, field: &str) -> PyResult<Money> {
        if !amount.is_finite() {
            return Err(crate::errors::value_error(format!(
                "CSA {field} must be finite, got {amount}"
            )));
        }
        Money::new(amount, self.inner.base_currency).map_err(core_to_py)
    }
}

#[pymethods]
impl PyCsaSpec {
    /// Standard regulatory CSA for USD derivatives (zero VM threshold, daily
    /// exchange, SIMM IM, BCBS-IOSCO collateral). Raises ``ValueError`` if
    /// the embedded registry cannot be loaded.
    #[staticmethod]
    fn usd_regulatory() -> PyResult<Self> {
        let inner = fm::CsaSpec::usd_regulatory().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard regulatory CSA for EUR derivatives. Raises ``ValueError`` if
    /// the embedded registry cannot be loaded.
    #[staticmethod]
    fn eur_regulatory() -> PyResult<Self> {
        let inner = fm::CsaSpec::eur_regulatory().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard regulatory CSA for any currency.
    ///
    /// Parameters
    /// ----------
    /// currency : str
    ///     ISO-4217 base currency for thresholds, MTA and collateral values.
    /// id : str
    ///     CSA identifier used in margin lookups; must be non-empty.
    /// collateral_curve : str
    ///     Discount-curve id for collateral valuation (e.g. ``"GBP-SONIA"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``currency`` is unknown or the embedded registry cannot be loaded.
    #[staticmethod]
    #[pyo3(signature = (currency, id, collateral_curve))]
    fn regulatory(currency: &str, id: &str, collateral_curve: &str) -> PyResult<Self> {
        let inner =
            fm::CsaSpec::regulatory_for_currency(parse_currency(currency)?, id, collateral_curve)
                .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Return a copy with bilateral (legacy, non-zero) VM threshold terms.
    ///
    /// Parameters
    /// ----------
    /// threshold : float
    ///     VM threshold in ``base_currency`` below which no margin is exchanged.
    /// mta : float
    ///     Minimum transfer amount in ``base_currency``.
    /// rounding : float | None
    ///     Transfer rounding increment in ``base_currency``; ``None`` keeps
    ///     the Rust default of 10,000.
    /// independent_amount : float | None
    ///     Independent amount in ``base_currency``; ``None`` keeps zero.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an amount is non-finite or outside the representable range.
    #[pyo3(signature = (threshold, mta, rounding = None, independent_amount = None))]
    fn with_vm_threshold(
        &self,
        threshold: f64,
        mta: f64,
        rounding: Option<f64>,
        independent_amount: Option<f64>,
    ) -> PyResult<Self> {
        let threshold = self.base_money(threshold, "threshold")?;
        let mta = self.base_money(mta, "mta")?;
        let rounding = rounding
            .map(|r| self.base_money(r, "rounding"))
            .transpose()?;
        let independent_amount = independent_amount
            .map(|ia| self.base_money(ia, "independent_amount"))
            .transpose()?;
        let inner = self
            .inner
            .clone()
            .with_vm_threshold(threshold, mta, rounding, independent_amount)
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Return a copy with explicit initial-margin terms.
    ///
    /// Parameters
    /// ----------
    /// methodology : ImMethodology | str
    ///     IM regime (``ImMethodology.simm()`` or the wire label ``"simm"``).
    /// mpor_days : int
    ///     Margin period of risk in business days; must be positive.
    /// threshold : float
    ///     IM threshold in ``base_currency``.
    /// mta : float
    ///     IM minimum transfer amount in ``base_currency``.
    /// segregated : bool, default True
    ///     Whether IM must be held with a third-party custodian.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``mpor_days`` is zero, an amount is non-finite, or the
    ///     methodology label is unknown.
    #[pyo3(signature = (methodology, mpor_days, threshold, mta, segregated = true))]
    fn with_im(
        &self,
        methodology: &Bound<'_, PyAny>,
        mpor_days: u32,
        threshold: f64,
        mta: f64,
        segregated: bool,
    ) -> PyResult<Self> {
        let methodology = extract_im_methodology(methodology)?;
        let threshold = self.base_money(threshold, "im threshold")?;
        let mta = self.base_money(mta, "im mta")?;
        let inner = self
            .inner
            .clone()
            .with_im(methodology, mpor_days, threshold, mta, segregated)
            .map_err(core_to_py)?;
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

    /// Deserialize from a JSON string; raises ``ValueError`` on malformed
    /// input or a spec that fails Rust-side validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: fm::CsaSpec = serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Stable CSA identifier used in margin lookups.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Base currency code.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Contractual business-day calendar identifier.
    #[getter]
    fn calendar_id(&self) -> &str {
        &self.inner.calendar_id
    }

    /// Discount-curve id used to value collateral.
    #[getter]
    fn collateral_curve_id(&self) -> &str {
        self.inner.collateral_curve_id.as_str()
    }

    /// Whether this CSA requires initial margin.
    #[getter]
    fn requires_im(&self) -> bool {
        self.inner.requires_im()
    }

    /// VM threshold in ``base_currency``.
    #[getter]
    fn vm_threshold(&self) -> f64 {
        self.inner.vm_threshold().amount()
    }

    /// VM minimum transfer amount in ``base_currency``.
    #[getter]
    fn vm_mta(&self) -> f64 {
        self.inner.vm_params.mta.amount()
    }

    /// VM transfer rounding increment in ``base_currency``.
    #[getter]
    fn vm_rounding(&self) -> f64 {
        self.inner.vm_params.rounding.amount()
    }

    /// VM independent amount in ``base_currency``.
    #[getter]
    fn vm_independent_amount(&self) -> f64 {
        self.inner.vm_params.independent_amount.amount()
    }

    /// VM call frequency.
    #[getter]
    fn vm_frequency(&self) -> PyMarginTenor {
        PyMarginTenor {
            inner: self.inner.vm_params.frequency,
        }
    }

    /// VM settlement lag in business days (T+n).
    #[getter]
    fn vm_settlement_lag(&self) -> u32 {
        self.inner.vm_params.settlement_lag
    }

    /// IM methodology, or ``None`` when no IM is exchanged.
    #[getter]
    fn im_methodology(&self) -> Option<PyImMethodology> {
        self.inner.im_params.as_ref().map(|p| PyImMethodology {
            inner: p.methodology,
        })
    }

    /// IM margin period of risk in business days, or ``None`` without IM.
    #[getter]
    fn im_mpor_days(&self) -> Option<u32> {
        self.inner.im_params.as_ref().map(|p| p.mpor_days)
    }

    /// IM threshold in ``base_currency``, or ``None`` without IM.
    #[getter]
    fn im_threshold(&self) -> Option<f64> {
        self.inner.im_threshold().map(Money::amount)
    }

    /// IM minimum transfer amount in ``base_currency``, or ``None`` without IM.
    #[getter]
    fn im_mta(&self) -> Option<f64> {
        self.inner.im_params.as_ref().map(|p| p.mta.amount())
    }

    /// Whether IM must be segregated, or ``None`` without IM.
    #[getter]
    fn im_segregated(&self) -> Option<bool> {
        self.inner.im_params.as_ref().map(|p| p.segregated)
    }

    /// Eligible-collateral schedule governing what can be posted.
    #[getter]
    fn eligible_collateral(&self) -> PyEligibleCollateralSchedule {
        PyEligibleCollateralSchedule {
            inner: self.inner.eligible_collateral.clone(),
        }
    }

    /// Margin-call timing terms as a dict (``notification_deadline_hours``,
    /// ``response_deadline_hours``, ``dispute_resolution_days``,
    /// ``delivery_grace_days``).
    #[getter]
    fn call_timing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.call_timing)
    }

    /// Validate CSA identifiers and the contractual holiday calendar.
    ///
    /// Raises ``ValueError`` if the spec fails the Rust-side validation that
    /// ``from_json`` also applies on ingest.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Export the commercial terms as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``id``, ``base_currency``, ``calendar_id``,
    /// ``collateral_curve_id``, ``vm_threshold``, ``vm_mta``, ``vm_rounding``,
    /// ``vm_independent_amount``, ``vm_frequency``, ``vm_settlement_lag``,
    /// ``requires_im``, ``im_methodology``, ``im_mpor_days``, ``im_threshold``,
    /// ``im_mta``, ``im_segregated``. Amount columns are floats in
    /// ``base_currency``; the ``im_*`` columns are null when no IM applies.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let im = self.inner.im_params.as_ref();
        let row = serde_json::json!({
            "id": self.inner.id,
            "base_currency": self.inner.base_currency.to_string(),
            "calendar_id": self.inner.calendar_id,
            "collateral_curve_id": self.inner.collateral_curve_id.as_str(),
            "vm_threshold": self.vm_threshold(),
            "vm_mta": self.vm_mta(),
            "vm_rounding": self.vm_rounding(),
            "vm_independent_amount": self.vm_independent_amount(),
            "vm_frequency": self.inner.vm_params.frequency.to_string(),
            "vm_settlement_lag": self.inner.vm_params.settlement_lag,
            "requires_im": im.is_some(),
            "im_methodology": im.map(|p| p.methodology.to_string()),
            "im_mpor_days": im.map(|p| p.mpor_days),
            "im_threshold": im.map(|p| p.threshold.amount()),
            "im_mta": im.map(|p| p.mta.amount()),
            "im_segregated": im.map(|p| p.segregated),
        });
        crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "id",
                "base_currency",
                "calendar_id",
                "collateral_curve_id",
                "vm_threshold",
                "vm_mta",
                "vm_rounding",
                "vm_independent_amount",
                "vm_frequency",
                "vm_settlement_lag",
                "requires_im",
                "im_methodology",
                "im_mpor_days",
                "im_threshold",
                "im_mta",
                "im_segregated",
            ],
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

    fn __repr__(&self) -> String {
        let im = match self.inner.im_params.as_ref() {
            Some(p) => format!(
                "im={}(mpor={}, threshold={:.2}, mta={:.2})",
                p.methodology,
                p.mpor_days,
                p.threshold.amount(),
                p.mta.amount()
            ),
            None => "im=None".to_string(),
        };
        format!(
            "CsaSpec(id={:?}, currency={}, vm_threshold={:.2}, vm_mta={:.2}, vm_frequency={}, {im})",
            self.inner.id,
            self.inner.base_currency,
            self.vm_threshold(),
            self.vm_mta(),
            self.inner.vm_params.frequency,
        )
    }
}

/// Eligible collateral schedule with haircuts.
///
/// Answers "what can I post and at what haircut": ``to_dataframe`` lists
/// every eligible asset class with its haircut, rating and maturity
/// constraints; ``haircut_for_maturity`` resolves the maturity-bucketed
/// haircut for a bond; ``check_concentration_limits`` flags a proposed
/// collateral mix that breaches a concentration limit. Haircuts are decimal
/// fractions (``0.02`` = 2%).
#[pyclass(
    name = "EligibleCollateralSchedule",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEligibleCollateralSchedule {
    pub(super) inner: fm::EligibleCollateralSchedule,
}

#[pymethods]
impl PyEligibleCollateralSchedule {
    /// Cash-only schedule. Raises ``ValueError`` if the registry cannot load.
    #[staticmethod]
    fn cash_only() -> PyResult<Self> {
        let inner = fm::EligibleCollateralSchedule::cash_only().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard BCBS-IOSCO compliant schedule. Raises ``ValueError`` if the
    /// registry cannot load.
    #[staticmethod]
    fn bcbs_standard() -> PyResult<Self> {
        let inner = fm::EligibleCollateralSchedule::bcbs_standard().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// US Treasuries repo schedule. Raises ``ValueError`` if the registry
    /// cannot load.
    #[staticmethod]
    fn us_treasuries() -> PyResult<Self> {
        let inner = fm::EligibleCollateralSchedule::us_treasuries().map_err(core_to_py)?;
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

    /// Deserialize from JSON; raises ``ValueError`` on malformed input.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: fm::EligibleCollateralSchedule =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Whether rehypothecation is allowed.
    #[getter]
    fn rehypothecation_allowed(&self) -> bool {
        self.inner.rehypothecation_allowed
    }

    /// Number of eligible collateral types.
    #[getter]
    fn eligible_count(&self) -> usize {
        self.inner.eligible.len()
    }

    /// Haircut for collateral types not listed explicitly (decimal), or
    /// ``None`` when only listed types are accepted.
    #[getter]
    fn default_haircut(&self) -> Option<f64> {
        self.inner.default_haircut
    }

    /// Whether an asset class (or its wire label) is eligible.
    fn is_eligible(&self, asset_class: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_eligible(&extract_asset_class(asset_class)?))
    }

    /// Haircut (decimal) for an asset class, ignoring maturity constraints:
    /// the first matching entry, else ``default_haircut``, else ``None``.
    fn haircut_for(&self, asset_class: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
        Ok(self.inner.haircut_for(&extract_asset_class(asset_class)?))
    }

    /// Haircut (decimal) for an asset class at a remaining maturity.
    ///
    /// Parameters
    /// ----------
    /// asset_class : CollateralAssetClass | str
    ///     Collateral asset class or its wire label.
    /// remaining_years : float
    ///     Remaining maturity in years, matched against each entry's maturity
    ///     constraints.
    ///
    /// Returns ``None`` when no entry matches and the schedule has no default
    /// haircut.
    #[pyo3(signature = (asset_class, remaining_years))]
    fn haircut_for_maturity(
        &self,
        asset_class: &Bound<'_, PyAny>,
        remaining_years: f64,
    ) -> PyResult<Option<f64>> {
        Ok(self
            .inner
            .haircut_for_maturity(&extract_asset_class(asset_class)?, remaining_years))
    }

    /// Check a proposed collateral mix against the concentration limits.
    ///
    /// Parameters
    /// ----------
    /// allocations : list[tuple[CollateralAssetClass | str, float]]
    ///     ``(asset_class, amount)`` pairs; amounts share one currency and
    ///     are converted to fractions of their total.
    ///
    /// Returns a pandas ``DataFrame`` with columns ``asset_class``,
    /// ``fraction``, ``limit``, ``excess`` (all decimal fractions), one row
    /// per breached limit; an empty frame means the mix is within limits.
    fn check_concentration_limits<'py>(
        &self,
        py: Python<'py>,
        allocations: Vec<(Bound<'py, PyAny>, f64)>,
    ) -> PyResult<Bound<'py, PyAny>> {
        const COLUMNS: &[ColumnSchema<'_>] = &[
            ("asset_class", "str"),
            ("fraction", "float64"),
            ("limit", "float64"),
            ("excess", "float64"),
        ];
        let allocations: Vec<(fm::CollateralAssetClass, f64)> = allocations
            .iter()
            .map(|(asset_class, amount)| Ok((extract_asset_class(asset_class)?, *amount)))
            .collect::<PyResult<_>>()?;
        let rows: Vec<serde_json::Value> = self
            .inner
            .check_concentration_limits(&allocations)
            .into_iter()
            .map(|breach| {
                serde_json::json!({
                    "asset_class": breach.asset_class.to_string(),
                    "fraction": breach.fraction,
                    "limit": breach.limit,
                    "excess": breach.excess,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, COLUMNS)
    }

    /// Export the eligibility rows as a pandas ``DataFrame``.
    ///
    /// Columns: ``asset_class``, ``min_rating``, ``min_remaining_years``,
    /// ``max_remaining_years``, ``haircut``, ``fx_haircut_addon``,
    /// ``concentration_limit``. Haircuts and limits are decimal fractions;
    /// optional constraints are null when absent. One row per eligible entry,
    /// in schedule order (the order ``haircut_for`` searches).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        const COLUMNS: &[ColumnSchema<'_>] = &[
            ("asset_class", "str"),
            ("min_rating", "str"),
            ("min_remaining_years", "float64"),
            ("max_remaining_years", "float64"),
            ("haircut", "float64"),
            ("fx_haircut_addon", "float64"),
            ("concentration_limit", "float64"),
        ];
        let rows: Vec<serde_json::Value> = self
            .inner
            .eligible
            .iter()
            .map(|entry| {
                let maturity = entry.maturity_constraints.as_ref();
                serde_json::json!({
                    "asset_class": entry.asset_class.to_string(),
                    "min_rating": entry.min_rating,
                    "min_remaining_years": maturity.and_then(|m| m.min_remaining_years),
                    "max_remaining_years": maturity.and_then(|m| m.max_remaining_years),
                    "haircut": entry.haircut,
                    "fx_haircut_addon": entry.fx_haircut_addon,
                    "concentration_limit": entry.concentration_limit,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, COLUMNS)
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

    fn __repr__(&self) -> String {
        let classes: Vec<String> = self
            .inner
            .eligible
            .iter()
            .map(|e| e.asset_class.to_string())
            .collect();
        format!(
            "EligibleCollateralSchedule(eligible=[{}], default_haircut={}, rehypothecation_allowed={})",
            classes.join(", "),
            self.inner
                .default_haircut
                .map_or("None".to_string(), |h| h.to_string()),
            if self.inner.rehypothecation_allowed {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// Register all types in this module.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyImMethodology>()?;
    m.add_class::<PyMarginTenor>()?;
    m.add_class::<PyMarginCallType>()?;
    m.add_class::<PyClearingStatus>()?;
    m.add_class::<PyCollateralAssetClass>()?;
    m.add_class::<PyNettingSetId>()?;
    m.add_class::<PyCsaSpec>()?;
    m.add_class::<PyEligibleCollateralSchedule>()?;

    // `CONSTANTS` mirrors `finstack_quant_margin::constants` plus the
    // registry/calculator constants hosts need to interpret results.
    let constants = PyDict::new(py);
    constants.set_item(
        "CALENDAR_DAYS_PER_YEAR",
        fm::constants::CALENDAR_DAYS_PER_YEAR,
    )?;
    constants.set_item(
        "DURATION_APPROXIMATION_FACTOR",
        fm::constants::DURATION_APPROXIMATION_FACTOR,
    )?;
    constants.set_item("ONE_BP", fm::constants::ONE_BP)?;
    constants.set_item(
        "STANDARD_CDS_MATURITY_YEARS",
        fm::constants::STANDARD_CDS_MATURITY_YEARS,
    )?;
    constants.set_item(
        "DEFAULT_BOND_INDEX_DURATION",
        fm::constants::DEFAULT_BOND_INDEX_DURATION,
    )?;
    let tenor_buckets = PyDict::new(py);
    use fm::constants::tenor_buckets as tb;
    for (name, years) in [
        ("BUCKET_3M", tb::BUCKET_3M),
        ("BUCKET_6M", tb::BUCKET_6M),
        ("BUCKET_1Y", tb::BUCKET_1Y),
        ("BUCKET_2Y", tb::BUCKET_2Y),
        ("BUCKET_3Y", tb::BUCKET_3Y),
        ("BUCKET_5Y", tb::BUCKET_5Y),
        ("BUCKET_10Y", tb::BUCKET_10Y),
        ("BUCKET_15Y", tb::BUCKET_15Y),
        ("BUCKET_20Y", tb::BUCKET_20Y),
    ] {
        tenor_buckets.set_item(name, years)?;
    }
    constants.set_item("tenor_buckets", tenor_buckets)?;
    constants.set_item("BCBS_IOSCO_SCHEDULE_ID", fm::BCBS_IOSCO_SCHEDULE_ID)?;
    constants.set_item("HAIRCUT_MPOR_DAYS", fm::calculators::im::HAIRCUT_MPOR_DAYS)?;
    constants.set_item("SIMM_TENORS", fm::SIMM_TENORS.to_vec())?;
    constants.set_item(
        "SIMM_COMMODITY_BUCKET_COUNT",
        fm::types::SIMM_COMMODITY_BUCKET_COUNT,
    )?;
    m.add("CONSTANTS", constants)?;

    Ok(())
}
