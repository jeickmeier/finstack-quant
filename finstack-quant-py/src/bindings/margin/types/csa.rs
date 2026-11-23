//! CSA specification, eligible collateral schedule, SIMM classification, and OTC margin bindings.

use super::enums::{
    PyImCollateralResult, PyImMethodology, PyMarginTenor, extract_asset_class,
    extract_im_methodology,
};
use crate::bindings::core::config::PyFinstackConfig;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::module_utils::parse_currency;
use crate::bindings::table_builders::{
    serde_object_to_single_row_arrow_table, serde_rows_to_arrow_table,
};
use crate::errors::core_to_py;
use finstack_quant_core::money::Money;
use finstack_quant_margin as fm;
use pyo3::prelude::*;

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
    pub(crate) inner: fm::types::CsaSpec,
}

impl PyCsaSpec {
    fn base_money(&self, amount: f64, field: &str) -> PyResult<Money> {
        Money::new(amount, self.inner.base_currency).map_err(|error| {
            crate::errors::kind_to_py(error.kind(), format!("invalid {field}: {error}"))
        })
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder")]
#[pyclass(
    name = "CsaSpecBuilder",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
pub(crate) struct PyCsaSpecBuilder {
    inner: Option<fm::types::CsaSpecBuilder>,
}

wire_methods!(PyCsaSpec, fm::types::CsaSpec, "CsaSpec", validate);
py_getters!(PyCsaSpec |s| {
    /// Base currency code.
    base_currency -> String = s.inner.base_currency.to_string();
    /// Whether this CSA requires initial margin.
    requires_im -> bool = s.inner.requires_im();
    /// VM threshold in ``base_currency``.
    vm_threshold -> f64 = s.inner.vm_threshold().amount();
    /// VM minimum transfer amount in ``base_currency``.
    vm_mta -> f64 = s.inner.vm_params.mta.amount();
    /// VM transfer rounding increment in ``base_currency``.
    vm_rounding -> f64 = s.inner.vm_params.rounding.amount();
    /// VM independent amount in ``base_currency``.
    vm_independent_amount -> f64 = s.inner.vm_params.independent_amount.amount();
    /// VM call frequency.
    vm_frequency -> PyMarginTenor = PyMarginTenor {
        inner: s.inner.vm_params.frequency,
    };
    /// VM settlement lag in business days (T+n).
    vm_settlement_lag -> u32 = s.inner.vm_params.settlement_lag;
    /// IM methodology, or ``None`` when no IM is exchanged.
    im_methodology -> Option<PyImMethodology> = s.inner.im_params.as_ref().map(|p| PyImMethodology {
        inner: p.methodology,
    });
    /// IM margin period of risk in business days, or ``None`` without IM.
    im_mpor_days -> Option<u32> = s.inner.im_params.as_ref().map(|p| p.mpor_days);
    /// IM threshold in ``base_currency``, or ``None`` without IM.
    im_threshold -> Option<f64> = s.inner.im_threshold().map(Money::amount);
    /// IM minimum transfer amount in ``base_currency``, or ``None`` without IM.
    im_mta -> Option<f64> = s.inner.im_params.as_ref().map(|p| p.mta.amount());
    /// Whether IM must be segregated, or ``None`` without IM.
    im_segregated -> Option<bool> = s.inner.im_params.as_ref().map(|p| p.segregated);
    /// Eligible-collateral schedule governing what can be posted.
    eligible_collateral -> PyEligibleCollateralSchedule = PyEligibleCollateralSchedule {
        inner: s.inner.eligible_collateral.clone(),
    };
});

#[pymethods]
impl PyCsaSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpec.builder")]
    #[staticmethod]
    fn builder(
        id: &str,
        base_currency: &Bound<'_, PyAny>,
        collateral_curve_id: &str,
    ) -> PyResult<PyCsaSpecBuilder> {
        let currency = crate::bindings::core::currency::extract_currency(base_currency)?;
        Ok(PyCsaSpecBuilder {
            inner: Some(fm::types::CsaSpec::builder(
                id,
                currency,
                collateral_curve_id,
            )),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpec.to_builder")]
    fn to_builder(&self) -> PyCsaSpecBuilder {
        PyCsaSpecBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }
}

impl PyCsaSpecBuilder {
    fn take(&mut self) -> PyResult<fm::types::CsaSpecBuilder> {
        self.inner
            .take()
            .ok_or_else(|| crate::errors::value_error("CSA builder has already been consumed"))
    }
}

#[pymethods]
impl PyCsaSpecBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder.build")]
    fn build(&mut self) -> PyResult<PyCsaSpec> {
        Ok(PyCsaSpec {
            inner: self.take()?.build().map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder.calendar_id")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner = Some(slf.take()?.calendar_id(value));
        Ok(slf)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder.vm_threshold")]
    fn vm_threshold<'py>(
        mut slf: PyRefMut<'py, Self>,
        threshold: &crate::bindings::core::money::PyMoney,
        mta: &crate::bindings::core::money::PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner = Some(slf.take()?.vm_threshold(threshold.inner, mta.inner));
        Ok(slf)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder.im_simm")]
    fn im_simm(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        let builder = slf
            .inner
            .as_ref()
            .ok_or_else(|| crate::errors::value_error("CSA builder has already been consumed"))?;
        slf.inner = Some(builder.clone().im_simm().map_err(core_to_py)?);
        Ok(slf)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder.eligible_collateral")]
    fn eligible_collateral<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyEligibleCollateralSchedule,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner = Some(slf.take()?.eligible_collateral(value.inner.clone()));
        Ok(slf)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder.vm_params")]
    fn vm_params<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let term = crate::bindings::module_utils::py_to_serde(value.py(), value, "vm_params")?;
        slf.inner = Some(slf.take()?.vm_params(term));
        Ok(slf)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder.im_params")]
    fn im_params<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let term = crate::bindings::module_utils::py_to_serde(value.py(), value, "im_params")?;
        slf.inner = Some(slf.take()?.im_params(term));
        Ok(slf)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "CsaSpecBuilder.call_timing")]
    fn call_timing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let term = crate::bindings::module_utils::py_to_serde(value.py(), value, "call_timing")?;
        slf.inner = Some(slf.take()?.call_timing(term));
        Ok(slf)
    }
}

#[pymethods]
impl PyCsaSpec {
    /// Standard regulatory CSA for USD derivatives (zero VM threshold, daily
    /// exchange, SIMM IM, BCBS-IOSCO collateral). Raises ``ValueError`` if
    /// the embedded registry cannot be loaded.
    #[staticmethod]
    fn usd_regulatory() -> PyResult<Self> {
        let inner = fm::types::CsaSpec::usd_regulatory().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard regulatory CSA for EUR derivatives. Raises ``ValueError`` if
    /// the embedded registry cannot be loaded.
    #[staticmethod]
    fn eur_regulatory() -> PyResult<Self> {
        let inner = fm::types::CsaSpec::eur_regulatory().map_err(core_to_py)?;
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
        let inner = fm::types::CsaSpec::regulatory_for_currency(
            parse_currency(currency)?,
            id,
            collateral_curve,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard regulatory CSA built from a config's margin-registry overlay.
    ///
    /// Parameters
    /// ----------
    /// config : FinstackConfig
    ///     Config whose ``"margin.registry.v1"`` extension (if any) overlays
    ///     the embedded margin registry; without one the result equals
    ///     ``CsaSpec.regulatory(currency, id, collateral_curve)``.
    /// currency : str
    ///     ISO-4217 base currency for thresholds, MTA and collateral values.
    /// id : str
    ///     CSA identifier used in margin lookups; must be non-empty.
    /// collateral_curve : str
    ///     Discount-curve id for collateral valuation (e.g. ``"USD-OIS"``).
    ///
    /// Returns
    /// -------
    /// CsaSpec
    ///     The validated regulatory CSA using the merged registry terms.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``currency`` is unknown, ``id`` is empty, or the overlay is
    ///     malformed or fails registry validation.
    #[staticmethod]
    #[pyo3(signature = (config, currency, id, collateral_curve))]
    fn regulatory_from_config(
        config: PyRef<'_, PyFinstackConfig>,
        currency: &str,
        id: &str,
        collateral_curve: &str,
    ) -> PyResult<Self> {
        let inner = fm::types::CsaSpec::regulatory_from_config(
            &config.inner,
            parse_currency(currency)?,
            id,
            collateral_curve,
        )
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
    /// segregated : bool | None, default None
    ///     Whether IM must be held with a third-party custodian. ``None``
    ///     keeps the margin-registry default for ``methodology`` (``True``
    ///     for SIMM and schedule IM, ``False`` for clearing-house and
    ///     haircut IM).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``mpor_days`` is zero, an amount is non-finite, or the
    ///     methodology label is unknown.
    #[pyo3(signature = (methodology, mpor_days, threshold, mta, segregated = None))]
    fn with_im(
        &self,
        methodology: &Bound<'_, PyAny>,
        mpor_days: u32,
        threshold: f64,
        mta: f64,
        segregated: Option<bool>,
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

    /// Apply this CSA's IM threshold and MTA once to gross IM across its netting sets.
    /// Both inputs are nonnegative major units of `base_currency`. Gross IM must
    /// already reflect elected MPOR. Positive transfer posts; negative returns.
    /// Raises `ValueError` for invalid amounts, invalid terms or absent IM terms.
    fn apply_im_terms(
        &self,
        gross_initial_margin: f64,
        current_collateral: f64,
    ) -> PyResult<PyImCollateralResult> {
        let gross = self.base_money(gross_initial_margin, "gross_initial_margin")?;
        let current = self.base_money(current_collateral, "current_collateral")?;
        Ok(PyImCollateralResult {
            inner: self
                .inner
                .apply_im_terms(gross, current)
                .map_err(core_to_py)?,
        })
    }

    /// Stable CSA identifier used in margin lookups.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
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

    /// Margin-call timing terms as a dict (``notification_deadline_hours``,
    /// ``response_deadline_hours``, ``dispute_resolution_days``,
    /// ``delivery_grace_days``).
    #[getter]
    fn call_timing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.call_timing)
    }

    /// Validate CSA identifiers and the contractual holiday calendar.
    ///
    /// Raises ``ValueError`` if the spec fails the Rust-side validation that
    /// ``from_json`` also applies on ingest.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
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

impl PyCsaSpec {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
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
        serde_object_to_single_row_arrow_table(
            &row,
            &[
                ("id", "str"),
                ("base_currency", "str"),
                ("calendar_id", "str"),
                ("collateral_curve_id", "str"),
                ("vm_threshold", "float64"),
                ("vm_mta", "float64"),
                ("vm_rounding", "float64"),
                ("vm_independent_amount", "float64"),
                ("vm_frequency", "str"),
                ("vm_settlement_lag", "int64"),
                ("requires_im", "bool"),
                ("im_methodology", "str"),
                ("im_mpor_days", "int64"),
                ("im_threshold", "float64"),
                ("im_mta", "float64"),
                ("im_segregated", "bool"),
            ],
        )
    }
}

tabular_methods!(PyCsaSpec);

/// Eligible collateral schedule with haircuts.
///
/// Answers "what can I post and at what haircut": ``to_pandas`` / ``to_arrow`` lists
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
    pub(crate) inner: fm::types::EligibleCollateralSchedule,
}

wire_methods!(
    PyEligibleCollateralSchedule,
    fm::types::EligibleCollateralSchedule,
    "EligibleCollateralSchedule"
);
py_getters!(PyEligibleCollateralSchedule |s| {
    /// Whether rehypothecation is allowed.
    rehypothecation_allowed -> bool = s.inner.rehypothecation_allowed;
    /// Number of eligible collateral types.
    eligible_count -> usize = s.inner.eligible.len();
    /// Haircut for collateral types not listed explicitly (decimal), or
    /// ``None`` when only listed types are accepted.
    default_haircut -> Option<f64> = s.inner.default_haircut;
});

#[pymethods]
impl PyEligibleCollateralSchedule {
    /// Cash-only schedule. Raises ``ValueError`` if the registry cannot load.
    #[staticmethod]
    fn cash_only() -> PyResult<Self> {
        let inner = fm::types::EligibleCollateralSchedule::cash_only().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard BCBS-IOSCO compliant schedule. Raises ``ValueError`` if the
    /// registry cannot load.
    #[staticmethod]
    fn bcbs_standard() -> PyResult<Self> {
        let inner = fm::types::EligibleCollateralSchedule::bcbs_standard().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// US Treasuries repo schedule. Raises ``ValueError`` if the registry
    /// cannot load.
    #[staticmethod]
    fn us_treasuries() -> PyResult<Self> {
        let inner = fm::types::EligibleCollateralSchedule::us_treasuries().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Load a named schedule from a config's margin-registry overlay.
    ///
    /// Parameters
    /// ----------
    /// config : FinstackConfig
    ///     Config whose ``"margin.registry.v1"`` extension (if any) overlays
    ///     the embedded margin registry; configured schedules take precedence.
    /// schedule_id : str
    ///     Registry id of the schedule, e.g. ``"bcbs_standard"``,
    ///     ``"cash_only"``, ``"us_treasuries"`` or an id the overlay adds.
    ///
    /// Returns
    /// -------
    /// EligibleCollateralSchedule
    ///     A copy of the merged registry's schedule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the overlay is malformed or ``schedule_id`` names no schedule.
    #[staticmethod]
    #[pyo3(signature = (config, schedule_id))]
    fn from_finstack_config(
        config: PyRef<'_, PyFinstackConfig>,
        schedule_id: &str,
    ) -> PyResult<Self> {
        let inner =
            fm::types::EligibleCollateralSchedule::from_finstack_config(&config.inner, schedule_id)
                .map_err(core_to_py)?;
        Ok(Self { inner })
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
    /// Returns an :class:`~finstack_quant.core.table.ArrowTable` with columns
    /// ``asset_class``, ``fraction``, ``limit``, ``excess`` (all decimal
    /// fractions), one row per breached limit; an empty table means the mix
    /// is within limits.
    fn check_concentration_limits<'py>(
        &self,
        allocations: Vec<(Bound<'py, PyAny>, f64)>,
    ) -> PyResult<PyArrowTable> {
        const COLUMNS: &[(&str, &str)] = &[
            ("asset_class", "str"),
            ("fraction", "float64"),
            ("limit", "float64"),
            ("excess", "float64"),
        ];
        let allocations: Vec<(fm::types::CollateralAssetClass, f64)> = allocations
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
        serde_rows_to_arrow_table(&rows, COLUMNS)
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

impl PyEligibleCollateralSchedule {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: &[(&str, &str)] = &[
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
        serde_rows_to_arrow_table(&rows, COLUMNS)
    }
}

tabular_methods!(PyEligibleCollateralSchedule);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "SimmCreditClassification")]
#[pyclass(
    name = "SimmCreditClassification",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PySimmCreditClassification {
    inner: fm::types::SimmCreditClassification,
}

#[pymethods]
impl PySimmCreditClassification {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "SimmCreditClassification.qualifying")]
    #[staticmethod]
    fn qualifying(sector: &Bound<'_, PyAny>) -> PyResult<Self> {
        let sector =
            crate::bindings::module_utils::py_to_serde(sector.py(), sector, "SIMM credit sector")?;
        Ok(Self {
            inner: fm::types::SimmCreditClassification::qualifying(sector),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "SimmCreditClassification.non_qualifying")]
    #[staticmethod]
    fn non_qualifying() -> Self {
        Self {
            inner: fm::types::SimmCreditClassification::non_qualifying(),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "SimmCreditClassification.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner)
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "OtcMarginSpec")]
#[pyclass(
    name = "OtcMarginSpec",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyOtcMarginSpec {
    inner: fm::types::OtcMarginSpec,
}

pub(crate) fn extract_otc_margin_spec(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<fm::types::OtcMarginSpec> {
    if let Ok(spec) = value.cast::<PyOtcMarginSpec>() {
        return Ok(spec.borrow().inner.clone());
    }
    crate::bindings::module_utils::spec_from_py(py, value, "margin_spec")
}

#[pymethods]
impl PyOtcMarginSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "OtcMarginSpec.bilateral_simm")]
    #[staticmethod]
    fn bilateral_simm(csa: &PyCsaSpec) -> Self {
        Self {
            inner: fm::types::OtcMarginSpec::bilateral_simm(csa.inner.clone()),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "OtcMarginSpec.bilateral_schedule")]
    #[staticmethod]
    fn bilateral_schedule(csa: &PyCsaSpec) -> Self {
        Self {
            inner: fm::types::OtcMarginSpec::bilateral_schedule(csa.inner.clone()),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "OtcMarginSpec.cleared")]
    #[staticmethod]
    fn cleared(ccp: &str, currency: &str) -> PyResult<Self> {
        let inner = fm::types::OtcMarginSpec::cleared(ccp, parse_currency(currency)?)
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "OtcMarginSpec.with_simm_credit_classification")]
    fn with_simm_credit_classification(&self, classification: &PySimmCreditClassification) -> Self {
        Self {
            inner: self
                .inner
                .clone()
                .with_simm_credit_classification(classification.inner),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._types.csa", "OtcMarginSpec.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner)
    }
}
