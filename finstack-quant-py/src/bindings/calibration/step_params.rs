//! Native parameter objects used by calibration step authoring.

use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::module_utils::fields_from_py;
use crate::bindings::pydoc::pydoc;
use finstack_quant_calibration::api::schema::{self, StepParams};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

macro_rules! define_step_params {
    ($($name:ident => $native:ident, $variant:ident, $py_name:literal, $constructor_doc:literal, $dict_doc:literal;)*) => {
        $(
            #[doc = pydoc!("finstack_quant.calibration._step_params", $py_name)]
            #[pyclass(name = $py_name, module = "finstack_quant.calibration", frozen, skip_from_py_object)]
            pub(super) struct $name {
                inner: schema::$native,
            }

            #[pymethods]
            impl $name {
                #[doc = pydoc!("finstack_quant.calibration._step_params", $constructor_doc)]
                #[new]
                #[pyo3(signature = (**fields))]
                fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
                    fields_from_py(py, fields, $py_name).map(|inner| Self { inner })
                }

                #[doc = pydoc!("finstack_quant.calibration._step_params", $dict_doc)]
                fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                    serde_to_py(py, &self.inner)
                }
            }
        )*

        pub(super) fn extract(value: &Bound<'_, PyAny>) -> PyResult<StepParams> {
            $(
                if let Ok(params) = value.cast::<$name>() {
                    return Ok(StepParams::$variant(params.borrow().inner.clone()));
                }
            )*
            Err(PyTypeError::new_err("expected a native calibration step parameter object"))
        }

        pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
            $(module.add_class::<$name>()?;)*
            Ok(())
        }

    }
}

define_step_params! {
    PyDiscountCurveParams => DiscountCurveParams, Discount, "DiscountCurveParams", "DiscountCurveParams.__init__", "DiscountCurveParams.to_dict";
    PyForwardCurveParams => ForwardCurveParams, Forward, "ForwardCurveParams", "ForwardCurveParams.__init__", "ForwardCurveParams.to_dict";
    PyHazardCurveParams => HazardCurveParams, Hazard, "HazardCurveParams", "HazardCurveParams.__init__", "HazardCurveParams.to_dict";
    PyInflationCurveParams => InflationCurveParams, Inflation, "InflationCurveParams", "InflationCurveParams.__init__", "InflationCurveParams.to_dict";
    PyVolSurfaceParams => VolSurfaceParams, VolSurface, "VolSurfaceParams", "VolSurfaceParams.__init__", "VolSurfaceParams.to_dict";
    PySwaptionVolParams => SwaptionVolParams, SwaptionVol, "SwaptionVolParams", "SwaptionVolParams.__init__", "SwaptionVolParams.to_dict";
    PyBaseCorrelationParams => BaseCorrelationParams, BaseCorrelation, "BaseCorrelationParams", "BaseCorrelationParams.__init__", "BaseCorrelationParams.to_dict";
    PyStudentTParams => StudentTParams, StudentT, "StudentTParams", "StudentTParams.__init__", "StudentTParams.to_dict";
    PyHullWhiteStepParams => HullWhiteStepParams, HullWhite, "HullWhiteStepParams", "HullWhiteStepParams.__init__", "HullWhiteStepParams.to_dict";
    PyCapFloorHullWhiteStepParams => CapFloorHullWhiteStepParams, CapFloorHullWhite, "CapFloorHullWhiteStepParams", "CapFloorHullWhiteStepParams.__init__", "CapFloorHullWhiteStepParams.to_dict";
    PySviSurfaceParams => SviSurfaceParams, SviSurface, "SviSurfaceParams", "SviSurfaceParams.__init__", "SviSurfaceParams.to_dict";
    PyXccyBasisParams => XccyBasisParams, XccyBasis, "XccyBasisParams", "XccyBasisParams.__init__", "XccyBasisParams.to_dict";
    PyParametricCurveParams => ParametricCurveParams, Parametric, "ParametricCurveParams", "ParametricCurveParams.__init__", "ParametricCurveParams.to_dict";
}
