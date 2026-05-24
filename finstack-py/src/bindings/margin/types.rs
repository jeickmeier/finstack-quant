//! Python wrappers for margin domain types and enums.

use crate::errors::{core_to_py, display_to_py};
use finstack_margin as fm;
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// ImMethodology
// ---------------------------------------------------------------------------

/// Initial margin calculation methodology.
#[pyclass(
    name = "ImMethodology",
    module = "finstack.margin",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
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

    /// Parse from a string (e.g. ``"simm"``, ``"schedule"``).
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

// ---------------------------------------------------------------------------
// MarginTenor
// ---------------------------------------------------------------------------

/// Margin call frequency.
#[pyclass(
    name = "MarginTenor",
    module = "finstack.margin",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
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

    /// Parse from string.
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

// ---------------------------------------------------------------------------
// MarginCallType
// ---------------------------------------------------------------------------

/// Type of margin call.
#[pyclass(
    name = "MarginCallType",
    module = "finstack.margin",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyMarginCallType {
    #[allow(dead_code)]
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
    fn variation_margin_delivery() -> Self {
        Self {
            inner: fm::MarginCallType::VariationMarginDelivery,
        }
    }

    /// Variation margin return (margin to be received back).
    #[staticmethod]
    fn variation_margin_return() -> Self {
        Self {
            inner: fm::MarginCallType::VariationMarginReturn,
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

    fn __repr__(&self) -> String {
        format!("MarginCallType({:?})", self.inner)
    }
}

// ---------------------------------------------------------------------------
// ClearingStatus
// ---------------------------------------------------------------------------

/// Clearing status for OTC derivatives.
#[pyclass(
    name = "ClearingStatus",
    module = "finstack.margin",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
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

// ---------------------------------------------------------------------------
// CollateralAssetClass
// ---------------------------------------------------------------------------

/// Collateral asset class per BCBS-IOSCO standards.
#[pyclass(
    name = "CollateralAssetClass",
    module = "finstack.margin",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCollateralAssetClass {
    pub(super) inner: fm::CollateralAssetClass,
}

#[pymethods]
impl PyCollateralAssetClass {
    #[staticmethod]
    fn cash() -> Self {
        Self {
            inner: fm::CollateralAssetClass::Cash,
        }
    }

    #[staticmethod]
    fn government_bonds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::GovernmentBonds,
        }
    }

    #[staticmethod]
    fn agency_bonds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::AgencyBonds,
        }
    }

    #[staticmethod]
    fn covered_bonds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::CoveredBonds,
        }
    }

    #[staticmethod]
    fn corporate_bonds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::CorporateBonds,
        }
    }

    #[staticmethod]
    fn equity() -> Self {
        Self {
            inner: fm::CollateralAssetClass::Equity,
        }
    }

    #[staticmethod]
    fn gold() -> Self {
        Self {
            inner: fm::CollateralAssetClass::Gold,
        }
    }

    #[staticmethod]
    fn mutual_funds() -> Self {
        Self {
            inner: fm::CollateralAssetClass::MutualFunds,
        }
    }

    /// Parse from string.
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: fm::CollateralAssetClass = s.parse().map_err(crate::errors::value_error)?;
        Ok(Self { inner })
    }

    /// BCBS-IOSCO standard haircut for this asset class.
    fn standard_haircut(&self) -> PyResult<f64> {
        self.inner.standard_haircut().map_err(core_to_py)
    }

    /// FX haircut add-on for currency mismatch.
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

// ---------------------------------------------------------------------------
// NettingSetId
// ---------------------------------------------------------------------------

/// Identifies a margin netting set.
#[pyclass(name = "NettingSetId", module = "finstack.margin", skip_from_py_object)]
#[derive(Clone)]
pub struct PyNettingSetId {
    pub(super) inner: fm::NettingSetId,
}

#[pymethods]
impl PyNettingSetId {
    /// Create a bilateral netting set.
    #[staticmethod]
    fn bilateral(counterparty_id: &str, csa_id: &str) -> Self {
        Self {
            inner: fm::NettingSetId::bilateral(counterparty_id, csa_id),
        }
    }

    /// Create a cleared netting set.
    #[staticmethod]
    fn cleared(ccp_id: &str) -> Self {
        Self {
            inner: fm::NettingSetId::cleared(ccp_id),
        }
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

// ---------------------------------------------------------------------------
// CsaSpec — JSON round-trip wrapper
// ---------------------------------------------------------------------------

/// Credit Support Annex specification (ISDA standard).
#[pyclass(name = "CsaSpec", module = "finstack.margin", skip_from_py_object)]
#[derive(Clone)]
pub struct PyCsaSpec {
    pub(super) inner: fm::CsaSpec,
}

#[pymethods]
impl PyCsaSpec {
    /// Standard regulatory CSA for USD derivatives.
    #[staticmethod]
    fn usd_regulatory() -> PyResult<Self> {
        let inner = fm::CsaSpec::usd_regulatory().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard regulatory CSA for EUR derivatives.
    #[staticmethod]
    fn eur_regulatory() -> PyResult<Self> {
        let inner = fm::CsaSpec::eur_regulatory().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: fm::CsaSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    /// CSA identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Base currency code.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Whether this CSA requires initial margin.
    #[getter]
    fn requires_im(&self) -> bool {
        self.inner.requires_im()
    }

    fn __repr__(&self) -> String {
        format!(
            "CsaSpec(id={:?}, currency={}, requires_im={})",
            self.inner.id,
            self.inner.base_currency,
            self.inner.requires_im()
        )
    }
}

// ---------------------------------------------------------------------------
// EligibleCollateralSchedule
// ---------------------------------------------------------------------------

/// Eligible collateral schedule with haircuts.
#[pyclass(
    name = "EligibleCollateralSchedule",
    module = "finstack.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEligibleCollateralSchedule {
    #[allow(dead_code)]
    pub(super) inner: fm::EligibleCollateralSchedule,
}

#[pymethods]
impl PyEligibleCollateralSchedule {
    /// Cash-only schedule.
    #[staticmethod]
    fn cash_only() -> PyResult<Self> {
        let inner = fm::EligibleCollateralSchedule::cash_only().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard BCBS-IOSCO compliant schedule.
    #[staticmethod]
    fn bcbs_standard() -> PyResult<Self> {
        let inner = fm::EligibleCollateralSchedule::bcbs_standard().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// US Treasuries repo schedule.
    #[staticmethod]
    fn us_treasuries() -> PyResult<Self> {
        let inner = fm::EligibleCollateralSchedule::us_treasuries().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: fm::EligibleCollateralSchedule =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
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

    /// Check if an asset class is eligible.
    fn is_eligible(&self, asset_class: &PyCollateralAssetClass) -> bool {
        self.inner.is_eligible(&asset_class.inner)
    }

    /// Get the haircut for an asset class.
    fn haircut_for(&self, asset_class: &PyCollateralAssetClass) -> Option<f64> {
        self.inner.haircut_for(&asset_class.inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "EligibleCollateralSchedule(eligible={}, rehyp={})",
            self.inner.eligible.len(),
            self.inner.rehypothecation_allowed
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

    let constants = pyo3::types::PyDict::new(py);
    constants.set_item(
        "BCBS_IOSCO_SCHEDULE_ID",
        fm::calculators::im::schedule::BCBS_IOSCO_SCHEDULE_ID,
    )?;
    m.add("CONSTANTS", constants)?;

    Ok(())
}
