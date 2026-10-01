use std::ops::Deref;

use crate::volatility::heston::HestonParams;

/// Default Heston parameters used when no market scalar is supplied.
///
/// These are conservative, broadly representative SPX-style values. They are
/// the single source of truth for Heston defaults across all equity option
/// pricers (Fourier, PDE, Monte Carlo).
pub mod heston_defaults {
    /// Default mean reversion speed of variance (κ).
    pub const KAPPA: f64 = 2.0;
    /// Default long-run variance level (θ).
    pub const THETA: f64 = 0.04;
    /// Default vol-of-vol (σᵥ).
    pub const SIGMA_V: f64 = 0.3;
    /// Default spot/variance correlation (ρ); negative for equity (leverage effect).
    pub const RHO: f64 = -0.7;
    /// Default initial variance (v₀).
    pub const V0: f64 = 0.04;
}

/// Relative price tolerance for residual integration mass and no-arbitrage bounds.
pub(super) const HESTON_TAIL_DIAGNOSTIC_THRESHOLD: f64 = 1e-9;

/// Maximum quadrature nodes in one attempt; fail explicitly beyond this work budget.
pub(super) const HESTON_MAX_NODES: usize = 1_048_576;
/// Target characteristic-function magnitude used to initialize the integration limit.
pub(super) const HESTON_TAIL_LOG_TARGET: f64 = 27.631_021_115_928_547;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
/// Market inputs for closed-form Heston pricing.
///
/// The stochastic parameters are stored once in the canonical models-layer
/// [`HestonParams`]; this wrapper adds only the continuous carry rates required
/// by risk-neutral pricing.
///
/// # References
///
/// - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility
///   with Applications to Bond and Currency Options." *Review of Financial Studies*, 6(2), 327-343. `docs/REFERENCES.md#heston-1993`
pub struct HestonPricingParams {
    /// Continuously compounded risk-free rate as an annual decimal.
    pub r: f64,
    /// Continuously compounded dividend or foreign yield as an annual decimal.
    pub q: f64,
    /// Canonical Heston stochastic parameters.
    #[serde(flatten)]
    pub model: HestonParams,
}

impl Deref for HestonPricingParams {
    type Target = HestonParams;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl HestonPricingParams {
    /// Create new Heston model parameters
    ///
    /// # Arguments
    ///
    /// * `r` - Continuously compounded risk-free rate in decimal annual units
    /// * `q` - Continuous dividend yield in decimal annual units
    /// * `kappa` - Mean-reversion speed of the stochastic volatility or short-rate factor
    /// * `theta` - Long-run mean level of the mean-reverting stochastic factor
    /// * `sigma_v` - Volatility-of-variance parameter for the Heston-style variance process
    /// * `rho` - Instantaneous correlation between Brownian drivers, in `(-1, 1)`
    /// * `v0` - Initial variance level for the stochastic volatility process at time zero
    pub fn new(
        r: f64,
        q: f64,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
        v0: f64,
    ) -> finstack_quant_core::Result<Self> {
        if !r.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Heston parameter r (risk-free rate) must be finite, got {r}"
            )));
        }
        if !q.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Heston parameter q (dividend yield) must be finite, got {q}"
            )));
        }
        Ok(Self {
            r,
            q,
            model: HestonParams::new(v0, kappa, theta, sigma_v, rho)?,
        })
    }

    /// Return whether the variance process satisfies the inclusive Feller condition.
    #[must_use]
    pub fn satisfies_feller(&self) -> bool {
        self.model.satisfies_feller_condition()
    }
}

/// Configuration for Heston Fourier integration.
///
/// Provides tuning knobs for the numerical integration.
#[derive(Debug, Clone, Copy)]
pub struct HestonFourierSettings {
    /// Upper limit for Fourier integral (default: 100)
    pub u_max: f64,
    /// Number of panels for composite Gauss-Legendre (default: 100)
    pub panels: usize,
    /// Gauss-Legendre order per panel (default: 16)
    pub gl_order: usize,
    /// Non-negative origin tolerance below the first quadrature node (default: 1e-8)
    pub phi_eps: f64,
}

impl Default for HestonFourierSettings {
    fn default() -> Self {
        Self {
            u_max: 100.0,
            panels: 100,
            gl_order: 16,
            phi_eps: 1e-8,
        }
    }
}

/// Gauss-Legendre orders supported by `composite_gauss_legendre_grid`.
///
/// A `gl_order` outside this set has no node/weight table, which would make
/// `HestonStripPricer::new` return `None` and silently degrade to the slower
/// per-strike path. Callers must pick one of these values.
pub(super) const SUPPORTED_GL_ORDERS: [usize; 4] = [2, 4, 8, 16];

impl HestonFourierSettings {
    /// Construct validated Fourier integration settings.
    ///
    /// # Arguments
    ///
    /// * `u_max` - Positive finite upper integration frequency.
    /// * `panels` - Positive composite panel count within the shared node budget.
    /// * `gl_order` - Gauss-Legendre nodes per panel: 2, 4, 8, or 16.
    /// * `phi_eps` - Finite non-negative origin tolerance below every grid node.
    ///
    /// # Errors
    ///
    /// Returns a [`finstack_quant_core::Error::Validation`] if `gl_order` is not one
    /// of the supported composite Gauss-Legendre orders ({2, 4, 8, 16}), if
    /// `panels == 0`, or if `u_max` is not a positive finite number. An
    /// invalid origin tolerance or a grid beyond the quadrature node budget
    /// is also rejected.
    pub fn new(
        u_max: f64,
        panels: usize,
        gl_order: usize,
        phi_eps: f64,
    ) -> finstack_quant_core::Result<Self> {
        let settings = Self {
            u_max,
            panels,
            gl_order,
            phi_eps,
        };
        settings.validate()?;
        Ok(settings)
    }

    /// Validate that these settings can drive the composite Gauss-Legendre grid.
    ///
    /// # Errors
    ///
    /// Returns a [`finstack_quant_core::Error::Validation`] if `gl_order` is not in
    /// {2, 4, 8, 16}, if the grid exceeds its node budget, if `panels == 0`,
    /// if `u_max` is not positive finite, or if `phi_eps` could omit a grid node.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !SUPPORTED_GL_ORDERS.contains(&self.gl_order) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "HestonFourierSettings.gl_order must be one of {SUPPORTED_GL_ORDERS:?}, got {}",
                self.gl_order
            )));
        }
        if self.panels == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "HestonFourierSettings.panels must be positive, got 0".to_string(),
            ));
        }
        if !self.u_max.is_finite() || self.u_max <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "HestonFourierSettings.u_max must be a positive finite number, got {}",
                self.u_max
            )));
        }
        if self
            .panels
            .checked_mul(self.gl_order)
            .is_none_or(|nodes| nodes > HESTON_MAX_NODES)
        {
            return Err(finstack_quant_core::Error::Validation(
                "HestonFourierSettings exceeds the quadrature node budget".to_string(),
            ));
        }
        // The smallest supported normalized Gauss-Legendre node is >0.005
        // panel widths. No validated grid may silently discard origin nodes.
        let first_node_lower_bound = 0.005 * self.u_max / self.panels as f64;
        if !self.phi_eps.is_finite() || self.phi_eps < 0.0 || self.phi_eps >= first_node_lower_bound
        {
            return Err(finstack_quant_core::Error::Validation(
                "HestonFourierSettings.phi_eps must be finite, non-negative, and below the first integration node".to_string(),
            ));
        }
        Ok(())
    }

    /// Create settings adapted continuously to time to maturity at 20% initial volatility.
    ///
    /// The integration limit grows as `1/sqrt(time)` for short-dated options.
    /// Pricing also accounts for the model's stochastic-variance tail and checks
    /// numerical convergence before returning a value.
    ///
    /// # Arguments
    ///
    /// * `time` - Positive remaining time to expiry in years.
    #[must_use]
    pub fn for_maturity(time: f64) -> Self {
        Self::for_maturity_with_variance(time, 0.04)
    }

    /// Create settings adapted continuously to maturity and initial variance.
    ///
    /// Uses the Gaussian short-time decay `exp(-v0*time*u²/2)` with a target
    /// magnitude of `1e-12`. The integration limit has a minimum of 80, with
    /// at most one frequency unit per 16-node panel. The pricing driver adds
    /// a stochastic-variance tail bound and refines this initial grid.
    /// Extreme inputs that exceed the finite work budget are rejected by
    /// [`Self::validate`] and the checked pricing functions.
    ///
    /// # Arguments
    ///
    /// * `time` - Positive remaining time to expiry in years.
    /// * `v0` - Positive initial instantaneous variance, in annual decimal-volatility squared.
    #[must_use]
    pub fn for_maturity_with_variance(time: f64, v0: f64) -> Self {
        let u_max = if time.is_finite() && time > 0.0 && v0.is_finite() && v0 > 0.0 {
            (2.0 * HESTON_TAIL_LOG_TARGET / (v0 * time))
                .sqrt()
                .max(80.0)
        } else {
            f64::NAN
        };
        Self {
            u_max,
            panels: u_max.ceil() as usize,
            gl_order: 16,
            phi_eps: 0.0,
        }
    }
}
