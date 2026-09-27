use finstack_quant_core::{Error, InputError, Result};

use super::{AssetDynamics, MertonBarrierType};

/// Merton structural credit model.
///
/// Models a firm's equity as a call option on its assets, where default
/// occurs when asset value falls below the debt barrier.
///
/// # Fields
///
/// - `asset_value` (V_0): Current market value of the firm's assets.
/// - `asset_vol` (sigma_V): Annualized volatility of asset returns.
/// - `debt_barrier` (B): Face value of debt / default point.
/// - `risk_free_rate` (r): Continuous risk-free rate.
/// - `payout_rate` (q): Continuous dividend / payout yield on assets.
/// - `barrier_type`: Terminal or first-passage barrier monitoring.
/// - `dynamics`: Asset return dynamics specification.
///
/// # Wire format
///
/// Deserialization is routed through [`MertonModel::new_with_dynamics`] via
/// [`RawMertonModel`], so a model loaded from JSON satisfies exactly the same
/// invariants as one built in Rust. The serialized field set is unchanged.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawMertonModel")]
pub struct MertonModel {
    /// Current firm asset value `V_0`, in the issuer's reporting currency.
    /// Strictly positive.
    pub(super) asset_value: f64,
    /// Asset volatility `sigma_V`, annualized and expressed as a decimal
    /// fraction (`0.25` is 25%). Strictly positive.
    pub(super) asset_vol: f64,
    /// Default barrier `B`, the debt face value the asset value is compared
    /// against. Strictly positive and in the same currency as `asset_value`.
    pub(super) debt_barrier: f64,
    /// Continuously compounded risk-free rate `r`, as a decimal fraction.
    pub(super) risk_free_rate: f64,
    /// Continuous payout (dividend) rate on assets, as a decimal fraction.
    /// Reduces the drift of the asset process under the risk-neutral measure.
    pub(super) payout_rate: f64,
    /// Whether default is tested only at maturity or continuously over the
    /// life of the debt.
    pub(super) barrier_type: MertonBarrierType,
    /// Stochastic process governing the asset value.
    pub(super) dynamics: AssetDynamics,
}

/// Unvalidated wire representation of a [`MertonModel`].
///
/// Exists solely so `#[serde(try_from = ...)]` can funnel deserialization
/// through [`MertonModel::new_with_dynamics`]. Field names and types mirror
/// [`MertonModel`] exactly, so the JSON representation is identical.
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(rename = "MertonModel"))]
pub struct RawMertonModel {
    /// Current firm asset value `V_0`, in the issuer's reporting currency.
    /// Strictly positive.
    pub asset_value: f64,
    /// Asset volatility `sigma_V`, annualized and expressed as a decimal
    /// fraction (`0.25` is 25%). Strictly positive.
    pub asset_vol: f64,
    /// Default barrier `B`, the debt face value the asset value is compared
    /// against. Strictly positive and in the same currency as `asset_value`.
    pub debt_barrier: f64,
    /// Continuously compounded risk-free rate `r`, as a decimal fraction.
    pub risk_free_rate: f64,
    /// Continuous payout (dividend) rate on assets, as a decimal fraction.
    /// Reduces the drift of the asset process under the risk-neutral measure.
    pub payout_rate: f64,
    /// Whether default is tested only at maturity or continuously over the
    /// life of the debt.
    pub barrier_type: MertonBarrierType,
    /// Stochastic process governing the asset value.
    pub dynamics: AssetDynamics,
}

impl TryFrom<RawMertonModel> for MertonModel {
    type Error = Error;

    fn try_from(raw: RawMertonModel) -> Result<Self> {
        Self::new_with_dynamics(
            raw.asset_value,
            raw.asset_vol,
            raw.debt_barrier,
            raw.risk_free_rate,
            raw.payout_rate,
            raw.barrier_type,
            raw.dynamics,
        )
    }
}

impl MertonModel {
    /// Create a new Merton model with GBM dynamics and terminal barrier.
    ///
    /// # Arguments
    ///
    /// * `asset_value` - Current asset value V_0 (must be > 0)
    /// * `asset_vol` - Asset volatility sigma_V (must be > 0)
    /// * `debt_barrier` - Debt face value B (must be > 0)
    /// * `risk_free_rate` - Risk-free rate r
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NonPositiveValue`] if `asset_value`, `asset_vol`,
    /// or `debt_barrier` are non-positive.
    pub fn new(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
    ) -> Result<Self> {
        Self::new_with_dynamics(
            asset_value,
            asset_vol,
            debt_barrier,
            risk_free_rate,
            0.0,
            MertonBarrierType::Terminal,
            AssetDynamics::GeometricBrownian,
        )
    }

    /// Create a new Merton model with full parameterisation.
    ///
    /// # Arguments
    ///
    /// * `asset_value` - Current asset value V_0 (must be > 0)
    /// * `asset_vol` - Asset volatility sigma_V (must be > 0)
    /// * `debt_barrier` - Debt face value B (must be > 0)
    /// * `risk_free_rate` - Risk-free rate r
    /// * `payout_rate` - Dividend / payout yield q
    /// * `barrier_type` - Terminal or first-passage
    /// * `dynamics` - Asset return dynamics
    ///
    /// # `V <= B` (in-default state)
    ///
    /// `asset_value <= debt_barrier` is intentionally accepted: it represents
    /// a firm at or through its default point (distressed names). Pricing
    /// then degenerates consistently — first-passage paths default
    /// immediately, terminal-barrier default probabilities approach 1, and
    /// the CreditGrades survival formula returns PD = 1 in the
    /// zero-variance limit. Callers wanting a strictly solvent firm should
    /// validate `asset_value > debt_barrier` themselves.
    ///
    /// # Dynamics and barrier compatibility
    ///
    /// Not every pairing has a closed-form default probability, so the
    /// unsupported combinations are rejected here rather than silently
    /// falling back to a different process:
    ///
    /// - `JumpDiffusion` requires `MertonBarrierType::Terminal`. First passage of a
    ///   jump-diffusion to a barrier has no elementary closed form.
    /// - `CreditGrades` requires `MertonBarrierType::FirstPassage` with a zero
    ///   growth rate. The CreditGrades survival function *is* a first-passage
    ///   law with a stochastic flat barrier, so any other pairing would
    ///   describe a process the model does not evaluate.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NonPositiveValue`] if `asset_value`, `asset_vol`,
    /// or `debt_barrier` are non-positive, and [`Error::Validation`] if
    /// `dynamics` carries an out-of-domain parameter (a negative jump
    /// intensity or barrier uncertainty, or a `mean_recovery` outside
    /// `[0, 1]`) or is paired with an incompatible `barrier_type`.
    pub fn new_with_dynamics(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        barrier_type: MertonBarrierType,
        dynamics: AssetDynamics,
    ) -> Result<Self> {
        if !(asset_value.is_finite() && asset_value > 0.0) {
            return Err(InputError::NonPositiveValue.into());
        }
        if !(asset_vol.is_finite() && asset_vol > 0.0) {
            return Err(InputError::NonPositiveValue.into());
        }
        if !(debt_barrier.is_finite() && debt_barrier > 0.0) {
            return Err(InputError::NonPositiveValue.into());
        }
        if !risk_free_rate.is_finite() {
            return Err(Error::Validation(format!(
                "MertonModel: risk_free_rate must be finite, got {risk_free_rate}"
            )));
        }
        if !payout_rate.is_finite() {
            return Err(Error::Validation(format!(
                "MertonModel: payout_rate must be finite, got {payout_rate}"
            )));
        }
        if let MertonBarrierType::FirstPassage {
            barrier_growth_rate,
        } = barrier_type
        {
            if !barrier_growth_rate.is_finite() {
                return Err(Error::Validation(format!(
                    "MertonModel: barrier_growth_rate must be finite, got {barrier_growth_rate}"
                )));
            }
        }
        dynamics.validate()?;
        match (&dynamics, &barrier_type) {
            (AssetDynamics::JumpDiffusion { .. }, MertonBarrierType::FirstPassage { .. }) => {
                return Err(Error::Validation(
                    "MertonModel: JumpDiffusion dynamics require MertonBarrierType::Terminal; \
                     first passage of a jump-diffusion has no closed-form default \
                     probability. Use Monte Carlo for pathwise first-passage default."
                        .to_string(),
                ));
            }
            (AssetDynamics::CreditGrades { .. }, MertonBarrierType::Terminal) => {
                return Err(Error::Validation(
                    "MertonModel: CreditGrades dynamics require \
                     MertonBarrierType::FirstPassage { barrier_growth_rate: 0.0 }; the \
                     CreditGrades survival function is a first-passage law with a \
                     stochastic flat barrier."
                        .to_string(),
                ));
            }
            (
                AssetDynamics::CreditGrades { .. },
                MertonBarrierType::FirstPassage {
                    barrier_growth_rate,
                },
            ) if *barrier_growth_rate != 0.0 => {
                return Err(Error::Validation(format!(
                    "MertonModel: CreditGrades dynamics require a zero barrier_growth_rate \
                     (the CreditGrades barrier is flat in expectation), got \
                     {barrier_growth_rate}"
                )));
            }
            _ => {}
        }
        Ok(Self {
            asset_value,
            asset_vol,
            debt_barrier,
            risk_free_rate,
            payout_rate,
            barrier_type,
            dynamics,
        })
    }

    /// Current asset value V_0.
    #[inline]
    pub fn asset_value(&self) -> f64 {
        self.asset_value
    }

    /// Asset volatility sigma_V.
    #[inline]
    pub fn asset_vol(&self) -> f64 {
        self.asset_vol
    }

    /// Debt barrier B.
    #[inline]
    pub fn debt_barrier(&self) -> f64 {
        self.debt_barrier
    }

    /// Risk-free rate r.
    #[inline]
    pub fn risk_free_rate(&self) -> f64 {
        self.risk_free_rate
    }

    /// Payout rate q (dividend yield).
    #[inline]
    pub fn payout_rate(&self) -> f64 {
        self.payout_rate
    }

    /// Barrier monitoring type.
    #[inline]
    pub fn barrier_type(&self) -> &MertonBarrierType {
        &self.barrier_type
    }

    /// Asset dynamics specification.
    #[inline]
    pub fn dynamics(&self) -> &AssetDynamics {
        &self.dynamics
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AssetDynamics, MertonBarrierType, MertonModel};

    #[test]
    fn credit_grades_produces_valid_model() {
        let m = MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.30, 0.40).expect("cg");
        assert!(m.asset_value() > 0.0);
        assert!(m.asset_vol() > 0.0);
        assert!(matches!(m.dynamics(), AssetDynamics::CreditGrades { .. }));
        assert!(matches!(
            m.barrier_type(),
            MertonBarrierType::FirstPassage { .. }
        ));
        let pd = m.default_probability(5.0);
        assert!(pd > 0.0 && pd < 1.0, "PD should be in (0,1), got {pd}");
    }
}
