//! Expiry-resolved SIMM curvature input conversion.

use crate::errors::core_to_py;
use finstack_quant_margin::SimmCurvatureSensitivity;
use pyo3::prelude::*;

/// One expiry-resolved volatility-weighted vega before historical SIMM curvature scaling.
#[pyclass(
    name = "SimmCurvatureSensitivity",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySimmCurvatureSensitivity {
    pub(super) inner: SimmCurvatureSensitivity,
}

#[pymethods]
impl PySimmCurvatureSensitivity {
    /// Construct and validate a factor's `sigma * dPV/dsigma` input before SF or HVR.
    /// Unknown class/bucket/tenor, empty factor and non-finite vega raise `ValueError`.
    #[new]
    #[pyo3(signature = (risk_class, bucket, factor, expiry_tenor, volatility_weighted_vega, risk_tenor=None))]
    fn new(
        risk_class: &str,
        bucket: String,
        factor: String,
        expiry_tenor: String,
        volatility_weighted_vega: f64,
        risk_tenor: Option<String>,
    ) -> PyResult<Self> {
        let inner = SimmCurvatureSensitivity {
            risk_class: risk_class.parse().map_err(crate::errors::value_error)?,
            bucket,
            factor,
            risk_tenor,
            expiry_tenor,
            volatility_weighted_vega,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// SIMM class selecting bucket and correlation rules.
    #[getter]
    fn risk_class(&self) -> String {
        self.inner.risk_class.to_string()
    }

    /// Currency, sector, commodity bucket, `fx`, or `residual` as appropriate.
    #[getter]
    fn bucket(&self) -> &str {
        &self.inner.bucket
    }

    /// Subcurve, issuer, underlier, commodity name or FX currency pair.
    #[getter]
    fn factor(&self) -> &str {
        &self.inner.factor
    }

    /// Required IR/credit risk-factor tenor; absent for the other classes.
    #[getter]
    fn risk_tenor(&self) -> Option<&str> {
        self.inner.risk_tenor.as_deref()
    }

    /// Option expiry selecting the prescribed SF scale.
    #[getter]
    fn expiry_tenor(&self) -> &str {
        &self.inner.expiry_tenor
    }

    /// Signed base-currency `sigma * dPV/dsigma` before SF, HVR, VRW or concentration.
    #[getter]
    fn volatility_weighted_vega(&self) -> f64 {
        self.inner.volatility_weighted_vega
    }
}
