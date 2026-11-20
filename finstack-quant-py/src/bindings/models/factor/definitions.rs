//! Typed authoring for factor definitions and their native market mappings.

use crate::bindings::core::currency::extract_currency;
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use finstack_quant_core::types::CurveId;
use finstack_quant_factor::{FactorDefinition, FactorId, MarketMapping};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.factor._definitions", "MarketMapping")]
#[pyclass(
    name = "MarketMapping",
    module = "finstack_quant.models.factor",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(super) struct PyMarketMapping {
    inner: MarketMapping,
}

#[pymethods]
impl PyMarketMapping {
    #[doc = pydoc!("finstack_quant.models.factor._definitions", "MarketMapping.curve_parallel")]
    #[staticmethod]
    fn curve_parallel(curve_ids: Vec<String>, units: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: MarketMapping::CurveParallel {
                curve_ids: curve_ids.into_iter().map(CurveId::new).collect(),
                units: py_to_serde(units.py(), units, "units")?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.models.factor._definitions", "MarketMapping.curve_bucketed")]
    #[staticmethod]
    fn curve_bucketed(curve_id: &str, tenor_weights: Vec<(f64, f64)>) -> Self {
        Self {
            inner: MarketMapping::CurveBucketed {
                curve_id: CurveId::new(curve_id),
                tenor_weights,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.models.factor._definitions", "MarketMapping.equity_spot")]
    #[staticmethod]
    fn equity_spot(tickers: Vec<String>) -> Self {
        Self {
            inner: MarketMapping::EquitySpot { tickers },
        }
    }

    #[doc = pydoc!("finstack_quant.models.factor._definitions", "MarketMapping.fx_rate")]
    #[staticmethod]
    fn fx_rate(base: &Bound<'_, PyAny>, quote: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: MarketMapping::FxRate {
                pair: (extract_currency(base)?, extract_currency(quote)?),
            },
        })
    }

    #[doc = pydoc!("finstack_quant.models.factor._definitions", "MarketMapping.vol_shift")]
    #[staticmethod]
    fn vol_shift(vol_surface_ids: Vec<String>, units: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: MarketMapping::VolShift {
                vol_surface_ids,
                units: py_to_serde(units.py(), units, "units")?,
            },
        })
    }
}

wire_methods!(
    PyMarketMapping,
    MarketMapping,
    "MarketMapping",
    module = "finstack_quant.models.factor._definitions"
);

#[doc = pydoc!("finstack_quant.models.factor._definitions", "FactorDefinition")]
#[pyclass(
    name = "FactorDefinition",
    module = "finstack_quant.models.factor",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyFactorDefinition {
    pub(crate) inner: FactorDefinition,
}

#[pymethods]
impl PyFactorDefinition {
    #[doc = pydoc!("finstack_quant.models.factor._definitions", "FactorDefinition.__init__")]
    #[new]
    #[pyo3(signature = (id, factor_type, market_mapping, description=None))]
    fn new(
        id: &str,
        factor_type: &Bound<'_, PyAny>,
        market_mapping: &PyMarketMapping,
        description: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FactorDefinition {
                id: FactorId::new(id),
                factor_type: py_to_serde(factor_type.py(), factor_type, "factor_type")?,
                market_mapping: market_mapping.inner.clone(),
                description,
            },
        })
    }
}

wire_methods!(
    PyFactorDefinition,
    FactorDefinition,
    "FactorDefinition",
    module = "finstack_quant.models.factor._definitions"
);
