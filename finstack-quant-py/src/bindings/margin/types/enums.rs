//! Margin methodology, tenor, call type, clearing, asset class, and netting-set bindings.

use crate::bindings::macros::{py_getters, typed_or_label, wire_methods};
use crate::errors::core_to_py;
use finstack_quant_margin as fm;
use pyo3::prelude::*;

/// Gross IM, collateral target and signed transfer for one CSA IM account.
#[pyclass(
    name = "ImCollateralResult",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyImCollateralResult {
    pub(crate) inner: fm::types::ImCollateralResult,
}

py_getters!(PyImCollateralResult |s| {
    /// Gross model IM before contractual terms. Amount is in major units of `currency`.
    gross_initial_margin -> f64 = s.inner.gross_initial_margin.amount();
    /// Target collateral balance after the allocated CSA threshold. Amount is in major units of `currency`.
    required_collateral -> f64 = s.inner.required_collateral.amount();
    /// Existing balance of this one-way IM account. Amount is in major units of `currency`.
    current_collateral -> f64 = s.inner.current_collateral.amount();
    /// Signed MTA-filtered transfer: positive posts, negative returns excess. Amount is in major units of `currency`.
    transfer -> f64 = s.inner.transfer.amount();
    /// ISO currency of all account amounts.
    currency -> String = s.inner.gross_initial_margin.currency().to_string();
    /// Whether the required IM is segregated and unavailable to meet VM.
    segregated -> bool = s.inner.segregated;
});

macro_rules! margin_label_enum {
    (
        $py_type:ident,
        $rust_type:ty,
        $py_name:literal,
        from_str: $(#[$from_doc:meta])*,
        variants: [ $( $(#[$vdoc:meta])* $fn_name:ident => $variant:ident ),+ $(,)? ]
    ) => {
        #[pymethods]
        impl $py_type {
            $(
                $(#[$vdoc])*
                #[staticmethod]
                fn $fn_name() -> Self {
                    Self {
                        inner: <$rust_type>::$variant,
                    }
                }
            )+

            $(#[$from_doc])*
            #[staticmethod]
            fn from_str(s: &str) -> PyResult<Self> {
                let inner: $rust_type = s
                    .parse()
                    .map_err(|e: String| crate::errors::value_error(e))?;
                Ok(Self { inner })
            }

            fn __repr__(&self) -> String {
                format!(concat!($py_name, "({})"), self.inner)
            }

            fn __str__(&self) -> String {
                self.inner.to_string()
            }
        }
    };
}

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
    pub(crate) inner: fm::types::ImMethodology,
}

margin_label_enum!(
    PyImMethodology,
    fm::types::ImMethodology,
    "ImMethodology",
    from_str:
        /// Parse the lower-case wire label (``"simm"``, ``"schedule"``,
        /// ``"haircut"``, ``"internal_model"``, ``"clearing_house"``).
        ///
        /// Raises ``ValueError`` for any other spelling, including ``"SIMM"``.
    ,
    variants: [
        /// Haircut-based IM (repos and securities financing).
        haircut => Haircut,
        /// ISDA SIMM (sensitivities-based, OTC derivatives).
        simm => Simm,
        /// BCBS-IOSCO regulatory schedule approach.
        schedule => Schedule,
        /// Internal model approved by regulator.
        internal_model => InternalModel,
        /// Clearing house (CCP-specific) methodology.
        clearing_house => ClearingHouse,
    ]
);

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
    pub(crate) inner: fm::types::MarginTenor,
}

margin_label_enum!(
    PyMarginTenor,
    fm::types::MarginTenor,
    "MarginTenor",
    from_str:
        /// Parse the lower-case wire label; raises ``ValueError`` otherwise.
    ,
    variants: [
        /// Daily margin calls (standard for OTC derivatives post-2016).
        daily => Daily,
        /// Weekly margin calls.
        weekly => Weekly,
        /// Monthly margin calls.
        monthly => Monthly,
        /// On-demand margin calls.
        on_demand => OnDemand,
    ]
);

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
    pub(crate) inner: fm::types::MarginCallType,
}

margin_label_enum!(
    PyMarginCallType,
    fm::types::MarginCallType,
    "MarginCallType",
    from_str:
        /// Parse the lower-case wire label; raises ``ValueError`` otherwise.
    ,
    variants: [
        /// Initial margin posting requirement.
        initial_margin => InitialMargin,
        /// Variation margin delivery (margin to be posted).
        variation_margin_post => VariationMarginPost,
        /// Variation margin return (margin to be received back).
        variation_margin_collect => VariationMarginCollect,
        /// Top-up margin call.
        top_up => TopUp,
        /// Collateral substitution request.
        substitution => Substitution,
    ]
);

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
    pub(crate) inner: fm::types::ClearingStatus,
}

py_getters!(PyClearingStatus |s| {
    /// Whether this is a bilateral trade.
    is_bilateral -> bool = matches!(s.inner, fm::types::ClearingStatus::Bilateral);
    /// Whether this is a cleared trade.
    is_cleared -> bool = matches!(s.inner, fm::types::ClearingStatus::Cleared { .. });
});

#[pymethods]
impl PyClearingStatus {
    /// Bilateral (uncleared) trade governed by CSA.
    #[staticmethod]
    fn bilateral() -> Self {
        Self {
            inner: fm::types::ClearingStatus::Bilateral,
        }
    }

    /// Trade cleared through a CCP.
    #[staticmethod]
    fn cleared(ccp: &str) -> Self {
        Self {
            inner: fm::types::ClearingStatus::Cleared {
                ccp: ccp.to_string(),
            },
        }
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
    pub(crate) inner: fm::types::CollateralAssetClass,
}

margin_label_enum!(
    PyCollateralAssetClass,
    fm::types::CollateralAssetClass,
    "CollateralAssetClass",
    from_str:
        /// Parse the lower-case wire label; raises ``ValueError`` otherwise.
    ,
    variants: [
        /// Cash in an eligible currency.
        cash => Cash,
        /// Sovereign government bonds.
        government_bonds => GovernmentBonds,
        /// Debt issued or guaranteed by eligible agencies.
        agency_bonds => AgencyBonds,
        /// Covered bonds meeting the applicable eligibility criteria.
        covered_bonds => CoveredBonds,
        /// Investment-grade corporate bonds.
        corporate_bonds => CorporateBonds,
        /// Eligible listed equity.
        equity => Equity,
        /// Eligible gold collateral.
        gold => Gold,
        /// Eligible mutual funds or exchange-traded funds.
        mutual_funds => MutualFunds,
    ]
);

#[pymethods]
impl PyCollateralAssetClass {
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
}

typed_or_label!(
    pub(crate) fn extract_asset_class -> fm::types::CollateralAssetClass,
    PyCollateralAssetClass,
    "a CollateralAssetClass or its lower-case wire label",
    |label| label.parse().map_err(crate::errors::value_error)
);

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
    pub(crate) inner: fm::types::NettingSetId,
}

wire_methods!(PyNettingSetId, fm::types::NettingSetId, "NettingSetId");
py_getters!(PyNettingSetId |s| {
    /// Whether this is a cleared netting set.
    is_cleared -> bool = s.inner.is_cleared();
});

#[pymethods]
impl PyNettingSetId {
    /// Create a bilateral netting set keyed by counterparty and CSA ids.
    #[staticmethod]
    fn bilateral(counterparty_id: &str, csa_id: &str) -> Self {
        Self {
            inner: fm::types::NettingSetId::bilateral(counterparty_id, csa_id),
        }
    }

    /// Create a cleared netting set keyed by the CCP id.
    #[staticmethod]
    fn cleared(ccp_id: &str) -> Self {
        Self {
            inner: fm::types::NettingSetId::cleared(ccp_id),
        }
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
pub(crate) fn extract_netting_set_id(obj: &Bound<'_, PyAny>) -> PyResult<fm::types::NettingSetId> {
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

typed_or_label!(
    pub(crate) fn extract_im_methodology -> fm::types::ImMethodology,
    PyImMethodology,
    "an ImMethodology or its lower-case wire label (e.g. \"simm\")",
    |label| label.parse().map_err(|e: String| crate::errors::value_error(e))
);
