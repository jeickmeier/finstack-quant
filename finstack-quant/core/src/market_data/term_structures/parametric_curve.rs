//! Nelson-Siegel and Nelson-Siegel-Svensson parametric yield curves.
//!
//! Provides parametric curve fitting as an alternative to knot-based bootstrap.
//! The family of Nelson-Siegel models represents the zero-rate term structure
//! using a small number of interpretable parameters.
//!
//! # Financial Concept
//!
//! The Nelson-Siegel (NS) model parameterizes the zero-rate curve using four
//! parameters:
//! ```text
//! z(t) = β₀ + β₁ × ((1 - e^(-t/τ)) / (t/τ))
//!            + β₂ × ((1 - e^(-t/τ)) / (t/τ) - e^(-t/τ))
//!
//! where:
//!   β₀ = long-term rate level
//!   β₁ = short-term component
//!   β₂ = medium-term hump component
//!   τ  = decay factor (> 0)
//! ```
//!
//! The Nelson-Siegel-Svensson (NSS) extension adds a second hump:
//! ```text
//! z(t) = NS(t) + β₃ × ((1 - e^(-t/τ₂)) / (t/τ₂) - e^(-t/τ₂))
//! ```
//!
//! # Use Cases
//!
//! 1. Direct calibration from instruments (global optimization over 4 or 6 parameters)
//! 2. Fit-to-curve post-processing (fit NS/NSS to an already-bootstrapped curve)
//! 3. Central bank yield curve reporting (ECB, Bundesbank publish NS/NSS params)
//!
//! # References
//!
//! - Nelson, C. R., & Siegel, A. F. (1987). "Parsimonious Modeling of Yield Curves."
//!   *Journal of Business*, 60(4), 473-489.
//! - Svensson, L. E. O. (1994). "Estimating and Interpreting Forward Interest Rates:
//!   Sweden 1992-1994." *NBER Working Paper No. 4871*.
//! - Diebold, F. X., & Li, C. (2006). "Forecasting the Term Structure of Government
//!   Bond Yields." *Journal of Econometrics*, 130(2), 337-364.

use crate::{
    dates::{Date, DayCount},
    market_data::traits::{Discounting, TermStructure},
    types::CurveId,
};
use serde::{Deserialize, Serialize};

/// Nelson-Siegel model variant selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NsVariant {
    /// Four-parameter Nelson-Siegel model.
    Ns,
    /// Six-parameter Nelson-Siegel-Svensson model.
    Nss,
}

/// Nelson-Siegel model parameters.
///
/// Stores either the 4-parameter NS or 6-parameter NSS specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum NelsonSiegelModel {
    /// Four-parameter Nelson-Siegel.
    Ns {
        /// Long-term rate level.
        beta0: f64,
        /// Short-term component.
        beta1: f64,
        /// Medium-term hump.
        beta2: f64,
        /// Decay factor (must be > 0).
        tau: f64,
    },
    /// Six-parameter Nelson-Siegel-Svensson.
    Nss {
        /// Long-term rate level.
        beta0: f64,
        /// Short-term component.
        beta1: f64,
        /// Medium-term hump.
        beta2: f64,
        /// Second hump.
        beta3: f64,
        /// First decay factor (must be > 0).
        tau1: f64,
        /// Second decay factor (must be > 0, ≠ τ₁).
        tau2: f64,
    },
}

impl NelsonSiegelModel {
    /// Compute the zero rate at time `t` using the parametric formula.
    ///
    /// Returns the continuously compounded zero rate.
    #[must_use]
    pub fn zero_rate(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return match self {
                Self::Ns { beta0, beta1, .. } | Self::Nss { beta0, beta1, .. } => beta0 + beta1,
            };
        }

        match self {
            Self::Ns {
                beta0,
                beta1,
                beta2,
                tau,
            } => {
                let x = t / tau;
                let decay = (1.0 - (-x).exp()) / x;
                beta0 + beta1 * decay + beta2 * (decay - (-x).exp())
            }
            Self::Nss {
                beta0,
                beta1,
                beta2,
                beta3,
                tau1,
                tau2,
            } => {
                let x1 = t / tau1;
                let x2 = t / tau2;
                let decay1 = (1.0 - (-x1).exp()) / x1;
                let decay2 = (1.0 - (-x2).exp()) / x2;
                beta0
                    + beta1 * decay1
                    + beta2 * (decay1 - (-x1).exp())
                    + beta3 * (decay2 - (-x2).exp())
            }
        }
    }

    /// Compute the instantaneous forward rate at time `t`.
    ///
    /// This is the analytical derivative of `z(t) * t` with respect to `t`.
    #[must_use]
    pub fn forward_rate(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return self.zero_rate(0.0);
        }

        match self {
            Self::Ns {
                beta0,
                beta1,
                beta2,
                tau,
            } => {
                let x = t / tau;
                let e = (-x).exp();
                beta0 + beta1 * e + beta2 * x * e
            }
            Self::Nss {
                beta0,
                beta1,
                beta2,
                beta3,
                tau1,
                tau2,
            } => {
                let x1 = t / tau1;
                let x2 = t / tau2;
                let e1 = (-x1).exp();
                let e2 = (-x2).exp();
                beta0 + beta1 * e1 + beta2 * x1 * e1 + beta3 * x2 * e2
            }
        }
    }

    /// Number of parameters in this model.
    #[must_use]
    pub fn num_params(&self) -> usize {
        match self {
            Self::Ns { .. } => 4,
            Self::Nss { .. } => 6,
        }
    }

    /// Convert parameters to a flat vector for optimizer consumption.
    #[must_use]
    pub fn to_params_vec(&self) -> Vec<f64> {
        match self {
            Self::Ns {
                beta0,
                beta1,
                beta2,
                tau,
            } => vec![*beta0, *beta1, *beta2, *tau],
            Self::Nss {
                beta0,
                beta1,
                beta2,
                beta3,
                tau1,
                tau2,
            } => vec![*beta0, *beta1, *beta2, *beta3, *tau1, *tau2],
        }
    }

    /// Construct from a flat parameter vector.
    ///
    /// # Errors
    /// Returns an error if the vector length doesn't match the variant.
    pub fn from_params_vec(variant: NsVariant, params: &[f64]) -> crate::Result<Self> {
        match variant {
            NsVariant::Ns => {
                if params.len() != 4 {
                    return Err(crate::Error::Validation(format!(
                        "NS requires 4 parameters, got {}",
                        params.len()
                    )));
                }
                Ok(Self::Ns {
                    beta0: params[0],
                    beta1: params[1],
                    beta2: params[2],
                    tau: params[3],
                })
            }
            NsVariant::Nss => {
                if params.len() != 6 {
                    return Err(crate::Error::Validation(format!(
                        "NSS requires 6 parameters, got {}",
                        params.len()
                    )));
                }
                Ok(Self::Nss {
                    beta0: params[0],
                    beta1: params[1],
                    beta2: params[2],
                    beta3: params[3],
                    tau1: params[4],
                    tau2: params[5],
                })
            }
        }
    }

    /// Validate parameter constraints.
    pub fn validate(&self) -> crate::Result<()> {
        match self {
            Self::Ns { tau, .. } => {
                if *tau <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "NS tau must be positive, got {tau}"
                    )));
                }
            }
            Self::Nss { tau1, tau2, .. } => {
                if *tau1 <= 0.0 || *tau2 <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "NSS tau values must be positive, got tau1={tau1}, tau2={tau2}"
                    )));
                }
                if (*tau1 - *tau2).abs() < 1e-10 {
                    return Err(crate::Error::Validation(
                        "NSS tau1 and tau2 must be distinct".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Variant selector for this model.
    #[must_use]
    pub fn variant(&self) -> NsVariant {
        match self {
            Self::Ns { .. } => NsVariant::Ns,
            Self::Nss { .. } => NsVariant::Nss,
        }
    }
}

/// Parametric yield curve based on Nelson-Siegel or Nelson-Siegel-Svensson models.
///
/// Unlike knot-based curves, a parametric curve is defined entirely by its
/// model parameters. No interpolation engine is needed.
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<ParametricCurve>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawParametricCurve", into = "RawParametricCurve")]
pub struct ParametricCurve {
    id: CurveId,
    base_date: Date,
    day_count: DayCount,
    model: NelsonSiegelModel,
}

/// Raw serializable state of ParametricCurve.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawParametricCurve {
    /// Curve identifier.
    id: String,
    /// Base date.
    base_date: Date,
    /// Day count convention.
    day_count: DayCount,
    /// Nelson-Siegel model parameters.
    model: NelsonSiegelModel,
}

impl From<ParametricCurve> for RawParametricCurve {
    fn from(curve: ParametricCurve) -> Self {
        RawParametricCurve {
            id: curve.id.to_string(),
            base_date: curve.base_date,
            day_count: curve.day_count,
            model: curve.model,
        }
    }
}

impl TryFrom<RawParametricCurve> for ParametricCurve {
    type Error = crate::Error;

    fn try_from(raw: RawParametricCurve) -> crate::Result<Self> {
        ParametricCurve::builder(raw.id)
            .base_date(raw.base_date)
            .day_count(raw.day_count)
            .model(raw.model)
            .build()
    }
}

impl ParametricCurve {
    /// Start building a parametric curve.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> ParametricCurveBuilder {
        ParametricCurveBuilder {
            id: id.into(),
            base_date: None,
            day_count: DayCount::Act365F,
            model: None,
        }
    }

    /// Access the fitted model parameters.
    #[must_use]
    pub fn params(&self) -> &NelsonSiegelModel {
        &self.model
    }

    /// Continuously compounded zero rate at time `t`.
    #[must_use]
    #[inline]
    pub fn zero_rate(&self, t: f64) -> f64 {
        self.model.zero_rate(t)
    }

    /// Instantaneous forward rate at time `t`.
    #[must_use]
    #[inline]
    pub fn forward_rate(&self, t: f64) -> f64 {
        self.model.forward_rate(t)
    }

    /// Curve identifier.
    #[must_use]
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Base date of the curve.
    #[must_use]
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base_date
    }

    /// Day count convention.
    #[must_use]
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Create a builder from this curve with a new ID (for rebuildable-with-id pattern).
    pub fn to_builder_with_id(&self, id: CurveId) -> ParametricCurveBuilder {
        ParametricCurveBuilder {
            id,
            base_date: Some(self.base_date),
            day_count: self.day_count,
            model: Some(self.model.clone()),
        }
    }
}

impl TermStructure for ParametricCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }
}

impl Discounting for ParametricCurve {
    fn base_date(&self) -> Date {
        self.base_date
    }

    fn df(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 1.0;
        }
        let z = self.model.zero_rate(t);
        (-z * t).exp()
    }
}

/// Builder for [`ParametricCurve`].
pub struct ParametricCurveBuilder {
    id: CurveId,
    base_date: Option<Date>,
    day_count: DayCount,
    model: Option<NelsonSiegelModel>,
}

impl ParametricCurveBuilder {
    /// Set the base date.
    pub fn base_date(mut self, date: Date) -> Self {
        self.base_date = Some(date);
        self
    }

    /// Set the day count convention.
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
        self
    }

    /// Set the Nelson-Siegel model parameters.
    pub fn model(mut self, model: NelsonSiegelModel) -> Self {
        self.model = Some(model);
        self
    }

    /// Build the parametric curve.
    pub fn build(self) -> crate::Result<ParametricCurve> {
        let base_date = self
            .base_date
            .ok_or(crate::Error::Validation("base_date is required".into()))?;
        let model = self
            .model
            .ok_or(crate::Error::Validation("model is required".into()))?;
        model.validate()?;

        Ok(ParametricCurve {
            id: self.id,
            base_date,
            day_count: self.day_count,
            model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn base_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).unwrap()
    }

    #[test]
    fn ns_zero_rate_at_zero() {
        let model = NelsonSiegelModel::Ns {
            beta0: 0.03,
            beta1: -0.02,
            beta2: 0.01,
            tau: 1.5,
        };
        // At t=0, z(0) = beta0 + beta1 = 0.01
        assert!((model.zero_rate(0.0) - 0.01).abs() < 1e-10);
    }

    #[test]
    fn ns_long_term_rate() {
        let model = NelsonSiegelModel::Ns {
            beta0: 0.05,
            beta1: -0.02,
            beta2: 0.01,
            tau: 1.5,
        };
        // At t→∞, z(∞) → beta0 = 0.05
        assert!((model.zero_rate(100.0) - 0.05).abs() < 1e-3);
    }

    #[test]
    fn parametric_curve_discount_factor() {
        let curve = ParametricCurve::builder("USD-NS")
            .base_date(base_date())
            .model(NelsonSiegelModel::Ns {
                beta0: 0.03,
                beta1: -0.01,
                beta2: 0.005,
                tau: 2.0,
            })
            .build()
            .unwrap();

        assert!((curve.df(0.0) - 1.0).abs() < 1e-12);
        assert!(curve.df(1.0) < 1.0);
        assert!(curve.df(5.0) < curve.df(1.0));
    }

    #[test]
    fn nss_model_basic() {
        let model = NelsonSiegelModel::Nss {
            beta0: 0.04,
            beta1: -0.02,
            beta2: 0.01,
            beta3: 0.005,
            tau1: 1.5,
            tau2: 5.0,
        };
        model.validate().unwrap();
        assert!((model.zero_rate(0.0) - 0.02).abs() < 1e-10);
        assert!((model.zero_rate(100.0) - 0.04).abs() < 1e-3);
    }

    #[test]
    fn round_trip_serde() {
        let curve = ParametricCurve::builder("USD-NS")
            .base_date(base_date())
            .model(NelsonSiegelModel::Ns {
                beta0: 0.03,
                beta1: -0.01,
                beta2: 0.005,
                tau: 2.0,
            })
            .build()
            .unwrap();

        let json = serde_json::to_string(&curve).unwrap();
        let restored: ParametricCurve = serde_json::from_str(&json).unwrap();
        assert_eq!(curve.id(), restored.id());
        assert!((curve.df(5.0) - restored.df(5.0)).abs() < 1e-12);
    }

    #[test]
    fn forward_rate_ns() {
        let model = NelsonSiegelModel::Ns {
            beta0: 0.03,
            beta1: -0.02,
            beta2: 0.01,
            tau: 1.5,
        };
        // Forward rate at t→∞ should converge to beta0
        assert!((model.forward_rate(100.0) - 0.03).abs() < 1e-4);
        // Forward rate at t=0 should equal beta0 + beta1 = 0.01
        assert!((model.forward_rate(0.0) - 0.01).abs() < 1e-10);
    }

    #[test]
    fn params_vec_round_trip() {
        let model = NelsonSiegelModel::Ns {
            beta0: 0.03,
            beta1: -0.02,
            beta2: 0.01,
            tau: 1.5,
        };
        let vec = model.to_params_vec();
        let restored = NelsonSiegelModel::from_params_vec(NsVariant::Ns, &vec).unwrap();
        assert_eq!(model, restored);
    }

    #[test]
    fn invalid_tau_rejected() {
        let model = NelsonSiegelModel::Ns {
            beta0: 0.03,
            beta1: -0.02,
            beta2: 0.01,
            tau: -1.0,
        };
        assert!(model.validate().is_err());
    }

    #[test]
    fn nss_equal_taus_rejected() {
        let model = NelsonSiegelModel::Nss {
            beta0: 0.04,
            beta1: -0.02,
            beta2: 0.01,
            beta3: 0.005,
            tau1: 2.0,
            tau2: 2.0,
        };
        assert!(model.validate().is_err());
    }
}
