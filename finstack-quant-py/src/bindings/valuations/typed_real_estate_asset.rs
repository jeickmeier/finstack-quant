//! A real-estate asset with explicit leases and cash-flow valuation assumptions.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::real_estate::{
    RealEstateAsset, RealEstateAssetBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

type PyDatedAmounts<'py> = Vec<(Bound<'py, PyAny>, f64)>;

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset")]
#[pyclass(
    name = "RealEstateAsset",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRealEstateAsset {
    pub(crate) inner: RealEstateAsset,
}

instrument_envelope_methods!(
    PyRealEstateAsset(
        "RealEstateAsset",
        module = "finstack_quant.valuations.instruments._typed_real_estate_asset"
    ),
    RealEstateAsset,
    builder = PyRealEstateAssetBuilder("RealEstateAssetBuilder", no_fields),
    seed = RealEstateAsset::builder(),
);
instrument_pricing_methods!(
    PyRealEstateAsset,
    "RealEstateAsset",
    module = "finstack_quant.valuations.instruments._typed_real_estate_asset"
);

#[pymethods]
impl PyRealEstateAsset {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.to_builder")]
    fn to_builder(&self) -> PyRealEstateAssetBuilder {
        PyRealEstateAssetBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        RealEstateAsset::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.currency")]
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.valuation_method")]
    #[getter]
    fn valuation_method<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.valuation_method)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.property_type")]
    #[getter]
    fn property_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.property_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.noi_schedule")]
    #[getter]
    fn noi_schedule<'py>(&self, py: Python<'py>) -> PyResult<PyDatedAmounts<'py>> {
        self.inner
            .noi_schedule
            .iter()
            .map(|(d, x)| crate::bindings::date_utils::date_to_py(py, *d).map(|d| (d, *x)))
            .collect()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.capex_schedule")]
    #[getter]
    fn capex_schedule<'py>(&self, py: Python<'py>) -> PyResult<Option<PyDatedAmounts<'py>>> {
        self.inner
            .capex_schedule
            .as_ref()
            .map(|schedule| {
                schedule
                    .iter()
                    .map(|(date, amount)| {
                        crate::bindings::date_utils::date_to_py(py, *date)
                            .map(|date| (date, *amount))
                    })
                    .collect()
            })
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.discount_rate")]
    #[getter]
    fn discount_rate(&self) -> Option<f64> {
        self.inner.discount_rate
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.cap_rate")]
    #[getter]
    fn cap_rate(&self) -> Option<f64> {
        self.inner.cap_rate
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.stabilized_noi")]
    #[getter]
    fn stabilized_noi(&self) -> Option<f64> {
        self.inner.stabilized_noi
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.terminal_cap_rate")]
    #[getter]
    fn terminal_cap_rate(&self) -> Option<f64> {
        self.inner.terminal_cap_rate
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.terminal_growth_rate")]
    #[getter]
    fn terminal_growth_rate(&self) -> Option<f64> {
        self.inner.terminal_growth_rate
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.sale_date")]
    #[getter]
    fn sale_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .sale_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.sale_price")]
    #[getter]
    fn sale_price(&self) -> Option<PyMoney> {
        self.inner.sale_price.map(PyMoney::from_inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.purchase_price")]
    #[getter]
    fn purchase_price(&self) -> Option<PyMoney> {
        self.inner.purchase_price.map(PyMoney::from_inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.acquisition_costs")]
    #[getter]
    fn acquisition_costs(&self) -> Vec<PyMoney> {
        self.inner
            .acquisition_costs
            .iter()
            .copied()
            .map(PyMoney::from_inner)
            .collect()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.disposition_cost_decimal")]
    #[getter]
    fn disposition_cost_decimal(&self) -> Option<f64> {
        self.inner.disposition_cost_decimal
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.disposition_costs")]
    #[getter]
    fn disposition_costs(&self) -> Vec<PyMoney> {
        self.inner
            .disposition_costs
            .iter()
            .copied()
            .map(PyMoney::from_inner)
            .collect()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.appraisal_value")]
    #[getter]
    fn appraisal_value(&self) -> Option<PyMoney> {
        self.inner.appraisal_value.map(PyMoney::from_inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAsset.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder")]
#[pyclass(
    name = "RealEstateAssetBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyRealEstateAssetBuilder {
    inner: Option<RealEstateAssetBuilder>,
}

#[pymethods]
impl PyRealEstateAssetBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: RealEstateAssetBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.currency")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: RealEstateAssetBuilder| b.currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.valuation_method")]
    fn valuation_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "valuation_method")?;
        builder_set!(slf, |b: RealEstateAssetBuilder| b.valuation_method(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.property_type")]
    fn property_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "property_type")?;
        builder_set!(slf, |b: RealEstateAssetBuilder| b.property_type_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.noi_schedule")]
    fn noi_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyDatedAmounts<'_>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(d, x)| crate::bindings::date_utils::py_to_date(&d).map(|date| (date, x)))
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: RealEstateAssetBuilder| b.noi_schedule(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.capex_schedule")]
    fn capex_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<PyDatedAmounts<'_>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(|schedule| {
                schedule
                    .into_iter()
                    .map(|(date, amount)| {
                        crate::bindings::date_utils::py_to_date(&date).map(|date| (date, amount))
                    })
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        builder_set!(slf, |b: RealEstateAssetBuilder| b.capex_schedule_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.discount_rate")]
    fn discount_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: RealEstateAssetBuilder| b.discount_rate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.cap_rate")]
    fn cap_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: RealEstateAssetBuilder| b.cap_rate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.stabilized_noi")]
    fn stabilized_noi<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: RealEstateAssetBuilder| b.stabilized_noi_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.terminal_cap_rate")]
    fn terminal_cap_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: RealEstateAssetBuilder| b
            .terminal_cap_rate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.terminal_growth_rate")]
    fn terminal_growth_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: RealEstateAssetBuilder| b
            .terminal_growth_rate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.sale_date")]
    fn sale_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: RealEstateAssetBuilder| b.sale_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.sale_price")]
    fn sale_price<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(|money| money.inner);
        builder_set!(slf, |b: RealEstateAssetBuilder| b.sale_price_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.purchase_price")]
    fn purchase_price<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(|money| money.inner);
        builder_set!(slf, |b: RealEstateAssetBuilder| b.purchase_price_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.acquisition_costs")]
    fn acquisition_costs<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<Py<PyMoney>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into_iter().map(|amount| amount.get().inner).collect();
        builder_set!(slf, |b: RealEstateAssetBuilder| b.acquisition_costs(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.disposition_cost_decimal")]
    fn disposition_cost_decimal<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: RealEstateAssetBuilder| b
            .disposition_cost_decimal_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.disposition_costs")]
    fn disposition_costs<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<Py<PyMoney>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into_iter().map(|amount| amount.get().inner).collect();
        builder_set!(slf, |b: RealEstateAssetBuilder| b.disposition_costs(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.appraisal_value")]
    fn appraisal_value<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(|money| money.inner);
        builder_set!(slf, |b: RealEstateAssetBuilder| b
            .appraisal_value_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: RealEstateAssetBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_real_estate_asset", "RealEstateAssetBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: RealEstateAssetBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRealEstateAsset>()?;
    module.add_class::<PyRealEstateAssetBuilder>()?;
    Ok(())
}
