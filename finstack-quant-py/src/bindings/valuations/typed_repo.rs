//! A fixed-term repurchase agreement with explicit collateral terms.

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::repo::{Repo, RepoBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo")]
#[pyclass(
    name = "Repo",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRepo {
    pub(crate) inner: Repo,
}

instrument_envelope_methods!(
    PyRepo(
        "Repo",
        module = "finstack_quant.valuations.instruments._typed_repo"
    ),
    Repo,
    builder = PyRepoBuilder("RepoBuilder", no_fields),
    seed = Repo::builder(),
);
instrument_trait_methods!(
    PyRepo,
    "Repo",
    module = "finstack_quant.valuations.instruments._typed_repo"
);

#[pymethods]
impl PyRepo {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.to_builder")]
    fn to_builder(&self) -> PyRepoBuilder {
        PyRepoBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        Repo::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.collateral")]
    #[getter]
    fn collateral<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.collateral)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.repo_rate")]
    #[getter]
    fn repo_rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.repo_rate)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.repo_type")]
    #[getter]
    fn repo_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.repo_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        super::convert::enum_to_py_string(&self.inner.business_day_convention)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.margin_spec")]
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.margin_spec)
    }
}

crate::bindings::macros::py_getters!(PyRepo |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.cash_amount")]
    cash_amount -> PyMoney = PyMoney::from_inner(s.inner.cash_amount);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.haircut")]
    haircut -> f64 = s.inner.haircut;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.triparty")]
    triparty -> bool = s.inner.triparty;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "Repo.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder")]
#[pyclass(
    name = "RepoBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyRepoBuilder {
    inner: Option<RepoBuilder>,
}

#[pymethods]
impl PyRepoBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: RepoBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.cash_amount")]
    fn cash_amount<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: RepoBuilder| b.cash_amount(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.collateral")]
    fn collateral<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "collateral")?;
        builder_set!(slf, |b: RepoBuilder| b.collateral(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.repo_rate")]
    fn repo_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::money::decimal_from_py(value)?;
        builder_set!(slf, |b: RepoBuilder| b.repo_rate(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: RepoBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: RepoBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.haircut")]
    fn haircut<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: RepoBuilder| b.haircut(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.repo_type")]
    fn repo_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "repo_type")?;
        builder_set!(slf, |b: RepoBuilder| b.repo_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.triparty")]
    fn triparty<'py>(mut slf: PyRefMut<'py, Self>, value: bool) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: RepoBuilder| b.triparty(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: RepoBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.business_day_convention")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "business_day_convention")?;
        builder_set!(slf, |b: RepoBuilder| b.business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.calendar_id")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: RepoBuilder| b.calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: RepoBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.margin_spec")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "margin_spec")?;
        builder_set!(slf, |b: RepoBuilder| b.margin_spec_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_repo", "RepoBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: RepoBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRepo>()?;
    module.add_class::<PyRepoBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["Repo", "RepoBuilder"];
