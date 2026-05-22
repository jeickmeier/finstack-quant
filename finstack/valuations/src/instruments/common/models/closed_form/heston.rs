//! Heston model semi-analytical pricing via Fourier inversion.
//!
//! Implements the Heston (1993) characteristic function approach for
//! European option pricing under stochastic volatility.
//!
//! # Algorithm
//!
//! Uses the Gil-Pelaez / P1-P2 formulation:
//! ```text
//! C = S * exp(-qT) * P1 - K * exp(-rT) * P2
//! ```
//!
//! where P1 and P2 are risk-neutral probabilities computed via Fourier inversion
//! of the probability characteristic functions ψ_j(φ).
//!
//! # Numerical Stability
//!
//! Implements the "Little Heston Trap" formulation from Albrecher et al. (2007)
//! to avoid branch-cut discontinuities in the complex logarithm.
//!
//! # Conventions
//!
//! | Parameter | Convention | Units |
//! |-----------|-----------|-------|
//! | Rates (r, q) | Continuously compounded | Decimal (0.05 = 5%) |
//! | Variance (v0, theta) | Annualized variance | Decimal (0.04 = 20% vol) |
//! | Vol-of-vol (sigma_v) | Annualized | Decimal |
//! | Time (T) | ACT/365-style | Years |
//! | Prices | Per unit of underlying | Currency units |
//!
//! # Reference
//!
//! - Heston (1993) - "A Closed-Form Solution for Options with Stochastic Volatility"
//! - Carr & Madan (1999) - "Option valuation using the fast Fourier transform"
//! - Albrecher et al. (2007) - "The Little Heston Trap"

use finstack_core::market_data::context::MarketContext;
use finstack_core::math::gauss_legendre_integrate_composite;
use num_complex::Complex;
use std::f64::consts::PI;
use tracing::warn;

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

const HESTON_G_DENOM_EPS: f64 = 1e-8;
const HESTON_EXPONENT_REAL_LIMIT: f64 = 700.0;

/// Truncated-tail mass (on the probability scale) above which the Gil-Pelaez
/// integral is considered mis-truncated and a diagnostic is surfaced.
///
/// A well-resolved Heston Fourier integral has a tail far below this; the
/// `[0, 1]` probability clamp would otherwise silently hide truncation error
/// from too small a `u_max` (audit item 4). `1e-4` ≈ 1bp on the probability,
/// which feeds into a price error worth flagging for risk use.
const HESTON_TAIL_DIAGNOSTIC_THRESHOLD: f64 = 1e-4;

#[derive(Debug, Clone, Copy)]
/// Heston stochastic volatility model parameters.
///
/// # References
///
/// - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility
///   with Applications to Bond and Currency Options." *Review of Financial Studies*, 6(2), 327-343.
pub struct HestonParams {
    /// Risk-free interest rate
    pub r: f64,
    /// Continuous dividend yield
    pub q: f64,
    /// Mean reversion speed of variance
    pub kappa: f64,
    /// Long-run variance level
    pub theta: f64,
    /// Volatility of variance (vol-of-vol)
    pub sigma_v: f64,
    /// Correlation between asset price and variance
    pub rho: f64,
    /// Initial variance level
    pub v0: f64,
}

impl HestonParams {
    /// Create new Heston model parameters
    pub fn new(
        r: f64,
        q: f64,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
        v0: f64,
    ) -> finstack_core::Result<Self> {
        if !r.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter r (risk-free rate) must be finite, got {r}"
            )));
        }
        if !q.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter q (dividend yield) must be finite, got {q}"
            )));
        }
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter kappa (mean reversion) must be positive, got {kappa}"
            )));
        }
        if theta <= 0.0 || !theta.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter theta (long-run variance) must be positive, got {theta}"
            )));
        }
        if sigma_v <= 0.0 || !sigma_v.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter sigma_v (vol-of-vol) must be positive, got {sigma_v}"
            )));
        }
        if rho <= -1.0 || rho >= 1.0 || !rho.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter rho (correlation) must be in (-1, 1), got {rho}"
            )));
        }
        if v0 <= 0.0 || !v0.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Heston parameter v0 (initial variance) must be positive, got {v0}"
            )));
        }

        let params = Self {
            r,
            q,
            kappa,
            theta,
            sigma_v,
            rho,
            v0,
        };

        if 2.0 * params.kappa * params.theta <= params.sigma_v * params.sigma_v {
            warn!(
                r = params.r,
                q = params.q,
                kappa = params.kappa,
                theta = params.theta,
                sigma_v = params.sigma_v,
                rho = params.rho,
                v0 = params.v0,
                "Heston Feller condition violated (2κθ ≤ σ²): informational only — the \
                 Fourier pricer never simulates the variance path, so this poses no \
                 zero-variance pricing risk; relevant only for Monte Carlo simulation \
                 of the variance process"
            );
        }

        Ok(params)
    }

    /// Build `HestonParams` for a given (r, q) pair, sourcing
    /// (κ, θ, σᵥ, ρ, v₀) from the market context's named scalars
    /// (`HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`, `HESTON_RHO`,
    /// `HESTON_V0`) and falling back to [`heston_defaults`] when a scalar is
    /// missing or has the wrong type.
    ///
    /// This is the single entry point used by the Fourier, PDE, and Monte
    /// Carlo equity-option pricers, ensuring all three see identical
    /// parameters when invoked on the same market.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::new`] (positive κ, θ,
    /// σᵥ, v₀; ρ ∈ (−1, 1); finite r, q).
    pub fn from_market(market: &MarketContext, r: f64, q: f64) -> finstack_core::Result<Self> {
        use crate::instruments::common_impl::helpers::get_unitless_scalar;
        let kappa = get_unitless_scalar(market, "HESTON_KAPPA", heston_defaults::KAPPA);
        let theta = get_unitless_scalar(market, "HESTON_THETA", heston_defaults::THETA);
        let sigma_v = get_unitless_scalar(market, "HESTON_SIGMA_V", heston_defaults::SIGMA_V);
        let rho = get_unitless_scalar(market, "HESTON_RHO", heston_defaults::RHO);
        let v0 = get_unitless_scalar(market, "HESTON_V0", heston_defaults::V0);
        Self::new(r, q, kappa, theta, sigma_v, rho, v0)
    }
}

/// Convert Monte Carlo Heston parameters into closed-form Fourier parameters.
///
/// This is a [`TryFrom`] (not `From`) because the conversion must re-run
/// [`HestonParams::new`] validation: the Monte Carlo `HestonParams` accepts a
/// correlation `ρ ∈ [-1, 1]` (inclusive), whereas the closed-form Fourier
/// pricer requires `ρ ∈ (-1, 1)` (exclusive). A plain `From` impl bypassed all
/// validation and could fabricate a closed-form parameter set the Fourier
/// pricer cannot handle.
impl TryFrom<finstack_monte_carlo::process::heston::HestonParams> for HestonParams {
    type Error = finstack_core::Error;

    fn try_from(
        value: finstack_monte_carlo::process::heston::HestonParams,
    ) -> finstack_core::Result<Self> {
        Self::new(
            value.r,
            value.q,
            value.kappa,
            value.theta,
            value.sigma_v,
            value.rho,
            value.v0,
        )
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
    /// Small epsilon to avoid singularity at φ=0 (default: 1e-8)
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

/// Gauss-Legendre orders supported by [`composite_gauss_legendre_grid`].
///
/// A `gl_order` outside this set has no node/weight table, which would make
/// [`HestonStripPricer::new`] return `None` and silently degrade to the slower
/// per-strike path. Callers must pick one of these values.
const SUPPORTED_GL_ORDERS: [usize; 4] = [2, 4, 8, 16];

impl HestonFourierSettings {
    /// Construct validated Fourier integration settings.
    ///
    /// # Errors
    ///
    /// Returns a [`finstack_core::Error::Validation`] if `gl_order` is not one
    /// of the supported composite Gauss-Legendre orders ({2, 4, 8, 16}), if
    /// `panels == 0`, or if `u_max` is not a positive finite number. An
    /// unsupported `gl_order` would otherwise cause silent degradation to the
    /// slower per-strike pricing path.
    pub fn new(
        u_max: f64,
        panels: usize,
        gl_order: usize,
        phi_eps: f64,
    ) -> finstack_core::Result<Self> {
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
    /// Returns a [`finstack_core::Error::Validation`] if `gl_order` is not in
    /// {2, 4, 8, 16}, if `panels == 0`, or if `u_max` is not positive finite.
    pub fn validate(&self) -> finstack_core::Result<()> {
        if !SUPPORTED_GL_ORDERS.contains(&self.gl_order) {
            return Err(finstack_core::Error::Validation(format!(
                "HestonFourierSettings.gl_order must be one of {SUPPORTED_GL_ORDERS:?}, got {}",
                self.gl_order
            )));
        }
        if self.panels == 0 {
            return Err(finstack_core::Error::Validation(
                "HestonFourierSettings.panels must be positive, got 0".to_string(),
            ));
        }
        if !self.u_max.is_finite() || self.u_max <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "HestonFourierSettings.u_max must be a positive finite number, got {}",
                self.u_max
            )));
        }
        Ok(())
    }

    /// Create settings adapted to the option's time to maturity.
    ///
    /// Short-dated options require finer integration grids because
    /// the characteristic function oscillates more rapidly.
    ///
    /// | Maturity | u_max | panels | gl_order |
    /// |----------|-------|--------|----------|
    /// | T < 0.05 | 200   | 200    | 16       |
    /// | T < 0.25 | 150   | 150    | 16       |
    /// | T < 1.0  | 100   | 100    | 16       |
    /// | T >= 1.0 | 80    | 80     | 16       |
    #[must_use]
    pub fn for_maturity(time: f64) -> Self {
        if time < 0.05 {
            Self {
                u_max: 200.0,
                panels: 200,
                gl_order: 16,
                phi_eps: 1e-8,
            }
        } else if time < 0.25 {
            Self {
                u_max: 150.0,
                panels: 150,
                gl_order: 16,
                phi_eps: 1e-8,
            }
        } else if time < 1.0 {
            Self::default()
        } else {
            Self {
                u_max: 80.0,
                panels: 80,
                gl_order: 16,
                phi_eps: 1e-8,
            }
        }
    }
}

/// Cached Heston Fourier data for pricing multiple strikes with shared parameters.
///
/// The characteristic function portion of the Gil-Pelaez integrand is independent
/// of strike, so it can be precomputed once on the composite Gauss-Legendre grid
/// and reused across a strike strip.
#[derive(Debug, Clone)]
pub struct HestonStripPricer {
    spot: f64,
    time: f64,
    params: HestonParams,
    /// Composite quadrature grid as `(phi, weight)` pairs.
    grid: Vec<(f64, f64)>,
    /// Cached `psi_1(phi) / (i * phi)` values on the grid.
    psi1_over_iphi: Vec<Complex<f64>>,
    /// Cached `psi_2(phi) / (i * phi)` values on the grid.
    psi2_over_iphi: Vec<Complex<f64>>,
    /// `true` when too many grid nodes had a non-finite / overflow-zeroed
    /// characteristic function, so the cached integral is unreliable and
    /// pricing must fall back to Black-Scholes (mirrors the scalar path).
    integrand_corrupted: bool,
}

/// Maximum fraction of integration nodes that may be non-finite / zeroed before
/// the cached strip integral is deemed unreliable and pricing falls back to BS.
///
/// A Heston characteristic function that overflows at a node makes
/// [`heston_pj_characteristic_function`] return exactly `Complex::ZERO`. A few
/// such nodes (typically in the tail, where the integrand is already tiny) are
/// harmless, but when a large fraction of nodes are corrupted the Gil-Pelaez
/// integral silently loses mass and yields a plausible-but-wrong probability.
const HESTON_STRIP_MAX_CORRUPT_FRACTION: f64 = 0.05;

impl HestonStripPricer {
    /// Build a strip pricer with characteristic-function values cached on the
    /// composite Gauss-Legendre integration grid.
    #[must_use]
    pub fn new(
        spot: f64,
        time: f64,
        params: &HestonParams,
        settings: &HestonFourierSettings,
    ) -> Option<Self> {
        let grid =
            composite_gauss_legendre_grid(0.0, settings.u_max, settings.gl_order, settings.panels)?;
        let i = Complex::new(0.0, 1.0);
        let log_spot = spot.ln();
        let mut psi1_over_iphi = Vec::with_capacity(grid.len());
        let mut psi2_over_iphi = Vec::with_capacity(grid.len());

        // Count interior nodes (φ away from the singularity) and how many of
        // them returned a non-finite / overflow-zeroed characteristic function.
        // `heston_pj_characteristic_function` signals overflow by returning
        // exactly `Complex::ZERO`, so a zero psi at a genuine φ is treated as a
        // corrupted node.
        let mut interior_nodes = 0_usize;
        let mut corrupted_nodes = 0_usize;

        for (phi, _) in &grid {
            if phi.abs() < settings.phi_eps {
                psi1_over_iphi.push(Complex::new(0.0, 0.0));
                psi2_over_iphi.push(Complex::new(0.0, 0.0));
                continue;
            }

            interior_nodes += 1;
            let denom = i * *phi;
            let psi1 = heston_pj_characteristic_function(1, *phi, time, log_spot, params);
            let psi2 = heston_pj_characteristic_function(2, *phi, time, log_spot, params);

            let psi1_ok = psi1.is_finite() && psi1.norm_sqr() > 0.0;
            let psi2_ok = psi2.is_finite() && psi2.norm_sqr() > 0.0;
            if !psi1_ok || !psi2_ok {
                corrupted_nodes += 1;
            }

            psi1_over_iphi.push(if psi1.is_finite() {
                psi1 / denom
            } else {
                Complex::new(0.0, 0.0)
            });
            psi2_over_iphi.push(if psi2.is_finite() {
                psi2 / denom
            } else {
                Complex::new(0.0, 0.0)
            });
        }

        let integrand_corrupted = interior_nodes > 0
            && (corrupted_nodes as f64) / (interior_nodes as f64)
                > HESTON_STRIP_MAX_CORRUPT_FRACTION;

        Some(Self {
            spot,
            time,
            params: *params,
            grid,
            psi1_over_iphi,
            psi2_over_iphi,
            integrand_corrupted,
        })
    }

    /// Evaluate one Gil-Pelaez probability on the cached grid.
    ///
    /// Returns `(clamped_probability, raw_probability, tail_estimate)`. The raw
    /// (pre-clamp) probability and the truncated-tail estimate let the caller
    /// detect `u_max` truncation error that the `[0, 1]` clamp would otherwise
    /// hide (audit item 4). The tail estimate is the absolute integrand mass in
    /// the last [`HESTON_TAIL_WINDOW_FRACTION`] of the integration range,
    /// divided by π.
    fn probability(&self, log_strike: f64, cached_values: &[Complex<f64>]) -> (f64, f64, f64) {
        let i = Complex::new(0.0, 1.0);
        let mut integral = 0.0;
        let mut tail_abs_mass = 0.0;

        // `u_max` is the largest grid abscissa; the tail window is its last
        // `HESTON_TAIL_WINDOW_FRACTION`.
        let u_max = self
            .grid
            .iter()
            .map(|(phi, _)| *phi)
            .fold(0.0_f64, f64::max);
        let tail_window_start = u_max * (1.0 - HESTON_TAIL_WINDOW_FRACTION);

        for ((phi, weight), cached) in self.grid.iter().zip(cached_values.iter()) {
            let exp_term = (-i * *phi * log_strike).exp();
            let value = (exp_term * *cached).re;
            if value.is_finite() {
                integral += *weight * value;
                if *phi >= tail_window_start {
                    tail_abs_mass += weight.abs() * value.abs();
                }
            }
        }

        let raw = 0.5 + integral / PI;
        (raw.clamp(0.0, 1.0), raw, tail_abs_mass / PI)
    }

    /// Price a single European call using the cached strip pricer.
    ///
    /// If too many integration nodes had a non-finite / overflow-zeroed
    /// characteristic function (see HESTON_STRIP_MAX_CORRUPT_FRACTION), the
    /// cached Gil-Pelaez integral is unreliable and this degrades to a
    /// Black-Scholes price at the integrated vol `sqrt(v0)` — mirroring the
    /// scalar [`heston_call_price_fourier_with_settings`] fallback rather than
    /// returning a plausible-but-wrong finite number.
    #[must_use]
    pub fn price_call(&self, strike: f64) -> f64 {
        if self.integrand_corrupted {
            return black_scholes_call(
                self.spot,
                strike,
                self.time,
                self.params.r,
                self.params.q,
                self.params.v0.sqrt(),
            );
        }

        let log_strike = strike.ln();
        let (p1, raw_p1, tail_p1) = self.probability(log_strike, &self.psi1_over_iphi);
        let (p2, raw_p2, tail_p2) = self.probability(log_strike, &self.psi2_over_iphi);

        // Audit item 4: surface a diagnostic when the truncated-tail estimate or
        // a pre-clamp probability excursion shows the integral was mis-truncated
        // at `u_max`, instead of silently relying on the `[0, 1]` clamp.
        let tail = tail_p1.max(tail_p2);
        let raw_excursion = (raw_p1 - raw_p1.clamp(0.0, 1.0))
            .abs()
            .max((raw_p2 - raw_p2.clamp(0.0, 1.0)).abs());
        if tail > HESTON_TAIL_DIAGNOSTIC_THRESHOLD
            || raw_excursion > HESTON_TAIL_DIAGNOSTIC_THRESHOLD
        {
            warn!(
                spot = self.spot,
                strike,
                time = self.time,
                tail_estimate = tail,
                raw_probability_excursion = raw_excursion,
                "Heston strip Gil-Pelaez integral truncated at u_max with a \
                 non-negligible residual tail; the price may be mis-truncated — \
                 consider a larger u_max"
            );
        }

        let call_price = self.spot * (-self.params.q * self.time).exp() * p1
            - strike * (-self.params.r * self.time).exp() * p2;

        if !call_price.is_finite() {
            return black_scholes_call(
                self.spot,
                strike,
                self.time,
                self.params.r,
                self.params.q,
                self.params.v0.sqrt(),
            );
        }

        call_price.max(0.0)
    }

    /// Price a strip of European calls using the cached strip pricer.
    #[must_use]
    pub fn price_calls(&self, strikes: &[f64]) -> Vec<f64> {
        strikes
            .iter()
            .map(|&strike| self.price_call(strike))
            .collect()
    }
}

fn gl_nodes_weights(order: usize) -> Option<(&'static [f64], &'static [f64])> {
    match order {
        2 => Some((
            &[-0.577_350_269_189_625_7, 0.577_350_269_189_625_7],
            &[1.0, 1.0],
        )),
        4 => Some((
            &[
                -0.861_136_311_594_052_6,
                -0.339_981_043_584_856_3,
                0.339_981_043_584_856_3,
                0.861_136_311_594_052_6,
            ],
            &[
                0.347_854_845_137_453_85,
                0.652_145_154_862_546_1,
                0.652_145_154_862_546_1,
                0.347_854_845_137_453_85,
            ],
        )),
        8 => Some((
            &[
                -0.960_289_856_497_536_3,
                -0.796_666_477_413_626_7,
                -0.525_532_409_916_329,
                -0.183_434_642_495_649_8,
                0.183_434_642_495_649_8,
                0.525_532_409_916_329,
                0.796_666_477_413_626_7,
                0.960_289_856_497_536_3,
            ],
            &[
                0.101_228_536_290_376_26,
                0.222_381_034_453_374_48,
                0.313_706_645_877_887_27,
                0.362_683_783_378_361_96,
                0.362_683_783_378_361_96,
                0.313_706_645_877_887_27,
                0.222_381_034_453_374_48,
                0.101_228_536_290_376_26,
            ],
        )),
        16 => Some((
            &[
                -0.989_400_934_991_649_9,
                -0.944_575_023_073_232_6,
                -0.865_631_202_387_831_8,
                -0.755_404_408_355_003,
                -0.617_876_244_402_643_8,
                -0.458_016_777_657_227_37,
                -0.281_603_550_779_258_9,
                -0.095_012_509_837_637_44,
                0.095_012_509_837_637_44,
                0.281_603_550_779_258_9,
                0.458_016_777_657_227_37,
                0.617_876_244_402_643_8,
                0.755_404_408_355_003,
                0.865_631_202_387_831_8,
                0.944_575_023_073_232_6,
                0.989_400_934_991_649_9,
            ],
            &[
                0.027_152_459_411_754_095,
                0.062_253_523_938_647_894,
                0.095_158_511_682_492_78,
                0.124_628_971_255_533_88,
                0.149_595_988_816_576_73,
                0.169_156_519_395_002_54,
                0.182_603_415_044_923_58,
                0.189_450_610_455_068_5,
                0.189_450_610_455_068_5,
                0.182_603_415_044_923_58,
                0.169_156_519_395_002_54,
                0.149_595_988_816_576_73,
                0.124_628_971_255_533_88,
                0.095_158_511_682_492_78,
                0.062_253_523_938_647_894,
                0.027_152_459_411_754_095,
            ],
        )),
        _ => None,
    }
}

fn composite_gauss_legendre_grid(
    a: f64,
    b: f64,
    order: usize,
    panels: usize,
) -> Option<Vec<(f64, f64)>> {
    if panels == 0 || !(a.is_finite() && b.is_finite()) || b <= a {
        return None;
    }

    let (xs, ws) = gl_nodes_weights(order)?;
    let h = (b - a) / panels as f64;
    let mut grid = Vec::with_capacity(xs.len() * panels);

    for panel_idx in 0..panels {
        let panel_start = a + panel_idx as f64 * h;
        let panel_end = panel_start + h;
        let half = 0.5 * (panel_end - panel_start);
        let mid = panel_start + half;

        for (x, w) in xs.iter().zip(ws.iter()) {
            grid.push((mid + half * x, half * w));
        }
    }

    Some(grid)
}

/// Heston probability characteristic function ψ_j(φ) for j ∈ {1, 2}.
///
/// Uses the "Little Heston Trap" formulation from Albrecher et al. (2007)
/// to avoid branch-cut discontinuities and overflow from `exp(+dT)`.
///
/// The key change vs. the original Heston (1993) is:
/// - `g⁻ = (b - ρσφi - d) / (b - ρσφi + d)` (swapped numerator/denominator)
/// - `exp(-dT)` instead of `exp(+dT)` (avoids overflow for large T or Re(d) > 0)
///
/// # Arguments
///
/// * `j` - Probability index (1 or 2)
/// * `phi` - Fourier variable
/// * `time` - Time to maturity
/// * `log_spot` - Natural log of spot price
/// * `params` - Heston model parameters
///
/// # Returns
///
/// Complex value of ψ_j(φ)
///
/// # References
///
/// - Albrecher et al. (2007) — "The Little Heston Trap"
fn heston_pj_characteristic_function(
    j: u8,
    phi: f64,
    time: f64,
    log_spot: f64,
    params: &HestonParams,
) -> Complex<f64> {
    let kappa = params.kappa;
    let theta = params.theta;
    let sigma = params.sigma_v;
    let rho = params.rho;
    let v0 = params.v0;
    let r = params.r;
    let q = params.q;

    let i = Complex::new(0.0, 1.0);
    let zero = Complex::new(0.0, 0.0);

    // For P1: u = 0.5, b = kappa - rho*sigma
    // For P2: u = -0.5, b = kappa
    let (u, b) = if j == 1 {
        (0.5, kappa - rho * sigma)
    } else {
        (-0.5, kappa)
    };

    let a = kappa * theta;
    let sigma_sq = sigma * sigma;

    // d = sqrt((rho*sigma*phi*i - b)^2 - sigma^2*(2*u*phi*i - phi^2))
    let d_sq = (rho * sigma * phi * i - b).powi(2) - sigma_sq * (2.0 * u * phi * i - phi * phi);
    let d = d_sq.sqrt();

    // Little Heston Trap formulation (Albrecher et al. 2007):
    // g⁻ = (b - rho*sigma*phi*i - d) / (b - rho*sigma*phi*i + d)
    // Uses exp(-dT) to avoid overflow
    let b_minus_rsi = b - rho * sigma * phi * i;
    let g_denom = b_minus_rsi + d;
    let g_denom_limit = HESTON_G_DENOM_EPS * (1.0 + b_minus_rsi.norm() + d.norm());
    if !g_denom.is_finite() || g_denom.norm() <= g_denom_limit {
        return zero;
    }
    let g_minus = (b_minus_rsi - d) / g_denom;
    if !g_minus.is_finite() {
        return zero;
    }

    // exp(-d*T) — bounded, avoids the overflow of exp(+dT)
    let exp_minus_dt = (-d * time).exp();
    if !exp_minus_dt.is_finite() {
        return zero;
    }

    let one = Complex::new(1.0, 0.0);

    // C = (r-q)*phi*i*T + (a/sigma^2) * [(b - rho*sigma*phi*i - d)*T
    //     - 2*ln((1 - g⁻*exp(-dT)) / (1 - g⁻))]
    let c = (r - q) * phi * i * time
        + (a / sigma_sq)
            * ((b_minus_rsi - d) * time
                - 2.0 * ((one - g_minus * exp_minus_dt) / (one - g_minus)).ln());

    // D = (b - rho*sigma*phi*i - d) / sigma^2
    //     * (1 - exp(-dT)) / (1 - g⁻*exp(-dT))
    let d_val =
        (b_minus_rsi - d) / sigma_sq * (one - exp_minus_dt) / (one - g_minus * exp_minus_dt);
    if !c.is_finite() || !d_val.is_finite() {
        return zero;
    }

    // ψ_j(φ) = exp(C + D*v0 + i*φ*ln(S))
    let exponent = c + d_val * v0 + i * phi * log_spot;
    if !exponent.is_finite() || exponent.re > HESTON_EXPONENT_REAL_LIMIT {
        return zero;
    }

    let psi = exponent.exp();
    if psi.is_finite() {
        psi
    } else {
        zero
    }
}

/// Fraction of the upper integration range whose absolute integrand mass is
/// used to estimate the truncated Gil-Pelaez tail (audit item 4).
const HESTON_TAIL_WINDOW_FRACTION: f64 = 0.1;

/// Diagnostics from a single Gil-Pelaez probability inversion.
///
/// Carries the information needed to detect the two silent failure modes the
/// audit flagged: characteristic-function corruption (item 5) and truncation
/// of the Fourier integral at a fixed `u_max` (item 4).
#[derive(Debug, Clone, Copy)]
struct HestonPjDiagnostics {
    /// Probability clamped to `[0, 1]` — the value used for pricing.
    probability: f64,
    /// Probability *before* the `[0, 1]` clamp. A value materially outside
    /// `[0, 1]` is direct evidence that the truncated integral lost or gained
    /// mass; the clamp would otherwise hide it.
    raw_probability: f64,
    /// Estimated magnitude of the truncated tail beyond `u_max`, expressed on
    /// the probability scale. Computed from the absolute integrand mass in the
    /// last [`HESTON_TAIL_WINDOW_FRACTION`] of the integration range — if the
    /// integrand has genuinely decayed this is tiny; if `u_max` is too small
    /// for the maturity it stays large.
    tail_estimate: f64,
    /// `true` when too many interior integration nodes had a non-finite /
    /// overflow-zeroed characteristic function (see
    /// [`HESTON_STRIP_MAX_CORRUPT_FRACTION`]); the integral is then unreliable
    /// and pricing must fall back to Black-Scholes — mirroring the strip pricer.
    corrupted: bool,
}

/// Compute the Pj probability for Heston call pricing via Fourier inversion,
/// returning full diagnostics alongside the value.
///
/// P_j = 0.5 + (1/π) ∫_0^∞ Re[exp(-i*φ*ln(K)) * ψ_j(φ) / (i*φ)] dφ
///
/// The integral is evaluated on the explicit composite Gauss-Legendre grid so
/// that, in a single pass, the routine can also:
/// - count overflow-zeroed characteristic-function nodes (audit item 5), and
/// - estimate the truncated tail mass beyond `u_max` (audit item 4).
///
/// # Arguments
///
/// * `j` - Probability index (1 or 2)
/// * `spot` - Current spot price
/// * `strike` - Strike price
/// * `time` - Time to maturity
/// * `params` - Heston model parameters
/// * `settings` - Integration settings
fn heston_pj_with_diagnostics(
    j: u8,
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonParams,
    settings: &HestonFourierSettings,
) -> HestonPjDiagnostics {
    let log_spot = spot.ln();
    let log_strike = strike.ln();
    let i = Complex::new(0.0, 1.0);

    // Build the same composite Gauss-Legendre grid the strip pricer uses, so we
    // can inspect per-node behaviour rather than treating the quadrature as a
    // black box.
    let grid =
        composite_gauss_legendre_grid(0.0, settings.u_max, settings.gl_order, settings.panels);
    let Some(grid) = grid else {
        // Degenerate settings: fall back to the library quadrature with no
        // node-level diagnostics available.
        let integrand = |phi: f64| {
            if phi.abs() < settings.phi_eps {
                return 0.0;
            }
            let psi = heston_pj_characteristic_function(j, phi, time, log_spot, params);
            let exp_term = (-i * phi * log_strike).exp();
            (exp_term * psi / (i * phi)).re
        };
        let (integral, integration_failed) = match gauss_legendre_integrate_composite(
            integrand,
            0.0,
            settings.u_max,
            settings.gl_order,
            settings.panels,
        ) {
            Ok(v) => (v, false),
            Err(_) => (0.0, true),
        };
        let raw = 0.5 + integral / PI;
        return HestonPjDiagnostics {
            probability: raw.clamp(0.0, 1.0),
            raw_probability: raw,
            tail_estimate: f64::INFINITY,
            // If the fallback integrator also failed, surface corruption so the
            // caller falls back to Black-Scholes rather than silently using 0.5.
            corrupted: integration_failed,
        };
    };

    // The tail window starts at this φ; absolute integrand mass beyond it
    // estimates the error from truncating the integral at `u_max`.
    let tail_window_start = settings.u_max * (1.0 - HESTON_TAIL_WINDOW_FRACTION);

    let mut integral = 0.0;
    let mut tail_abs_mass = 0.0;
    let mut interior_nodes = 0_usize;
    let mut corrupted_nodes = 0_usize;

    for (phi, weight) in &grid {
        // Handle singularity at φ=0.
        if phi.abs() < settings.phi_eps {
            continue;
        }
        interior_nodes += 1;

        let psi = heston_pj_characteristic_function(j, *phi, time, log_spot, params);
        // `heston_pj_characteristic_function` signals overflow by returning
        // exactly `Complex::ZERO`; a zero psi at a genuine φ is a corrupted node.
        if !(psi.is_finite() && psi.norm_sqr() > 0.0) {
            corrupted_nodes += 1;
        }

        let exp_term = (-i * *phi * log_strike).exp();
        let value = (exp_term * psi / (i * *phi)).re;
        if value.is_finite() {
            integral += *weight * value;
            if *phi >= tail_window_start {
                tail_abs_mass += weight.abs() * value.abs();
            }
        }
    }

    let corrupted = interior_nodes > 0
        && (corrupted_nodes as f64) / (interior_nodes as f64) > HESTON_STRIP_MAX_CORRUPT_FRACTION;

    let raw_probability = 0.5 + integral / PI;
    HestonPjDiagnostics {
        probability: raw_probability.clamp(0.0, 1.0),
        raw_probability,
        tail_estimate: tail_abs_mass / PI,
        corrupted,
    }
}

/// Compute the Pj probability for Heston call pricing via Fourier inversion.
///
/// Thin wrapper over [`heston_pj_with_diagnostics`] returning only the clamped
/// probability. Production callers need the corruption / truncation diagnostics
/// (e.g. [`heston_call_price_fourier_with_settings`]) and use the diagnostics
/// variant directly, so this convenience form is exercised only by tests.
#[cfg(test)]
fn heston_pj(
    j: u8,
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonParams,
    settings: &HestonFourierSettings,
) -> f64 {
    heston_pj_with_diagnostics(j, spot, strike, time, params, settings).probability
}

/// Price a European call option under the Heston model using Fourier inversion.
///
/// # Arguments
///
/// * `spot` - Current spot price
/// * `strike` - Strike price
/// * `time` - Time to maturity (years)
/// * `params` - Heston model parameters
///
/// # Returns
///
/// Call option price
///
/// # Formula
///
/// C = S * exp(-qT) * P1 - K * exp(-rT) * P2
///
/// where P1 and P2 are risk-neutral probabilities computed via Fourier inversion.
///
/// # Integration Settings
///
/// Uses [`HestonFourierSettings::for_maturity`] to adapt the integration grid
/// to the option's time to maturity. Short-dated options use finer grids to
/// handle the more rapidly oscillating characteristic function. For custom
/// control, use [`heston_call_price_fourier_with_settings`].
///
/// # Example
///
/// ```text
/// use finstack_valuations::instruments::models::closed_form::heston::{
///     heston_call_price_fourier, HestonParams,
/// };
///
/// let params = HestonParams::new(
///     0.05,  // risk-free rate
///     0.02,  // dividend yield
///     2.0,   // kappa (mean reversion)
///     0.04,  // theta (long-run variance)
///     0.3,   // sigma_v (vol-of-vol)
///     -0.7,  // rho (correlation)
///     0.04,  // v0 (initial variance)
/// )
/// .unwrap();
///
/// let price = heston_call_price_fourier(100.0, 100.0, 1.0, &params);
/// assert!(price > 0.0 && price < 100.0);
/// ```
#[must_use]
pub fn heston_call_price_fourier(spot: f64, strike: f64, time: f64, params: &HestonParams) -> f64 {
    heston_call_price_fourier_with_settings(
        spot,
        strike,
        time,
        params,
        &HestonFourierSettings::for_maturity(time),
    )
}

/// Price a strip of European call options under the Heston model using shared
/// characteristic-function precomputation.
#[must_use]
pub fn heston_call_prices_fourier(
    spot: f64,
    strikes: &[f64],
    time: f64,
    params: &HestonParams,
) -> Vec<f64> {
    heston_call_prices_fourier_with_settings(
        spot,
        strikes,
        time,
        params,
        &HestonFourierSettings::for_maturity(time),
    )
}

/// Price a strip of European call options with custom integration settings.
#[must_use]
pub fn heston_call_prices_fourier_with_settings(
    spot: f64,
    strikes: &[f64],
    time: f64,
    params: &HestonParams,
    settings: &HestonFourierSettings,
) -> Vec<f64> {
    if time <= 0.0 {
        return strikes
            .iter()
            .map(|&strike| (spot - strike).max(0.0))
            .collect();
    }

    if params.sigma_v < 1e-10 {
        return strikes
            .iter()
            .map(|&strike| {
                black_scholes_call(spot, strike, time, params.r, params.q, params.v0.sqrt())
            })
            .collect();
    }

    if let Some(pricer) = HestonStripPricer::new(spot, time, params, settings) {
        pricer.price_calls(strikes)
    } else {
        strikes
            .iter()
            .map(|&strike| {
                heston_call_price_fourier_with_settings(spot, strike, time, params, settings)
            })
            .collect()
    }
}

/// Price a strip of European put options under the Heston model using shared
/// characteristic-function precomputation.
#[must_use]
pub fn heston_put_prices_fourier(
    spot: f64,
    strikes: &[f64],
    time: f64,
    params: &HestonParams,
) -> Vec<f64> {
    heston_put_prices_fourier_with_settings(
        spot,
        strikes,
        time,
        params,
        &HestonFourierSettings::for_maturity(time),
    )
}

/// Price a strip of European put options with custom integration settings.
#[must_use]
pub fn heston_put_prices_fourier_with_settings(
    spot: f64,
    strikes: &[f64],
    time: f64,
    params: &HestonParams,
    settings: &HestonFourierSettings,
) -> Vec<f64> {
    if time <= 0.0 {
        return strikes
            .iter()
            .map(|&strike| (strike - spot).max(0.0))
            .collect();
    }

    let call_prices =
        heston_call_prices_fourier_with_settings(spot, strikes, time, params, settings);
    call_prices
        .into_iter()
        .zip(strikes.iter())
        .map(|(call_price, strike)| {
            let forward = spot * (-params.q * time).exp();
            let discount_k = *strike * (-params.r * time).exp();
            (call_price - forward + discount_k).max(0.0)
        })
        .collect()
}

/// Price a European call option with custom integration settings.
///
/// See [`heston_call_price_fourier`] for details.
#[must_use]
pub fn heston_call_price_fourier_with_settings(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonParams,
    settings: &HestonFourierSettings,
) -> f64 {
    // Handle expired options
    if time <= 0.0 {
        return (spot - strike).max(0.0);
    }

    // Special case: very small vol-of-vol approaches Black-Scholes
    // This avoids numerical issues when sigma_v is tiny
    if params.sigma_v < 1e-10 {
        return black_scholes_call(spot, strike, time, params.r, params.q, params.v0.sqrt());
    }

    // Compute P1 and P2 via Fourier inversion, with diagnostics.
    let d1 = heston_pj_with_diagnostics(1, spot, strike, time, params, settings);
    let d2 = heston_pj_with_diagnostics(2, spot, strike, time, params, settings);

    // Audit item 5: characteristic-function overflow corruption fallback.
    // `heston_pj_characteristic_function` returns `Complex::ZERO` on overflow;
    // when a large fraction of integration nodes are zeroed the Gil-Pelaez
    // integral silently loses mass and yields a plausible-but-wrong probability.
    // The strip pricer already detects this and falls back to Black-Scholes —
    // the scalar path must do the same rather than integrating zeros into a
    // finite-but-wrong price.
    if d1.corrupted || d2.corrupted {
        warn!(
            spot,
            strike,
            time,
            kappa = params.kappa,
            theta = params.theta,
            sigma_v = params.sigma_v,
            rho = params.rho,
            v0 = params.v0,
            "Heston scalar Fourier integrand corrupted (characteristic function \
             overflowed on too many integration nodes); falling back to a \
             Black-Scholes price at sqrt(v0)"
        );
        return black_scholes_call(spot, strike, time, params.r, params.q, params.v0.sqrt());
    }

    // Audit item 4: truncation-tail diagnostic. The Gil-Pelaez integral is
    // truncated at a fixed `u_max`; a non-negligible tail beyond `u_max`, or a
    // pre-clamp probability materially outside `[0, 1]`, means the truncated
    // integral mis-priced and the `[0, 1]` clamp is hiding it. Surface a
    // diagnostic so the mis-truncation is observable (short-dated wings are the
    // typical trigger) instead of being silently clamped away.
    let tail = d1.tail_estimate.max(d2.tail_estimate);
    let raw_p1_excursion = (d1.raw_probability - d1.raw_probability.clamp(0.0, 1.0)).abs();
    let raw_p2_excursion = (d2.raw_probability - d2.raw_probability.clamp(0.0, 1.0)).abs();
    let raw_excursion = raw_p1_excursion.max(raw_p2_excursion);
    if tail > HESTON_TAIL_DIAGNOSTIC_THRESHOLD || raw_excursion > HESTON_TAIL_DIAGNOSTIC_THRESHOLD {
        warn!(
            spot,
            strike,
            time,
            u_max = settings.u_max,
            tail_estimate = tail,
            raw_probability_excursion = raw_excursion,
            "Heston Gil-Pelaez integral truncated at u_max with a non-negligible \
             residual tail (or a pre-clamp probability outside [0,1]); the price \
             may be mis-truncated — consider a larger u_max (e.g. \
             HestonFourierSettings::for_maturity for short maturities)"
        );
    }

    // C = S * exp(-qT) * P1 - K * exp(-rT) * P2
    let call_price = spot * (-params.q * time).exp() * d1.probability
        - strike * (-params.r * time).exp() * d2.probability;

    // Defensive fallback: if the Fourier integration produced a non-finite result
    // (extreme parameters, characteristic-function overflow across the integration
    // range), degrade gracefully to a Black-Scholes price at the integrated vol
    // sqrt(v0). This avoids silent zero/NaN prices for deep-OTM/short-dated edge
    // cases where the per-phi `return zero` paths dominate the integrand.
    if !call_price.is_finite() {
        return black_scholes_call(spot, strike, time, params.r, params.q, params.v0.sqrt());
    }

    // Clamp to non-negative (numerical errors can cause tiny negatives for deep OTM)
    call_price.max(0.0)
}

/// Price a European put option under the Heston model using Fourier inversion.
///
/// # Arguments
///
/// * `spot` - Current spot price
/// * `strike` - Strike price
/// * `time` - Time to maturity (years)
/// * `params` - Heston model parameters
///
/// # Returns
///
/// Put option price
///
/// # Formula
///
/// Uses put-call parity: P = C - S*exp(-qT) + K*exp(-rT)
#[must_use]
pub fn heston_put_price_fourier(spot: f64, strike: f64, time: f64, params: &HestonParams) -> f64 {
    heston_put_price_fourier_with_settings(
        spot,
        strike,
        time,
        params,
        &HestonFourierSettings::for_maturity(time),
    )
}

/// Price a European put option with custom integration settings.
///
/// See [`heston_put_price_fourier`] for details.
pub fn heston_put_price_fourier_with_settings(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonParams,
    settings: &HestonFourierSettings,
) -> f64 {
    if time <= 0.0 {
        return (strike - spot).max(0.0);
    }

    // Use put-call parity: P = C - S*exp(-qT) + K*exp(-rT)
    let call_price = heston_call_price_fourier_with_settings(spot, strike, time, params, settings);
    let forward = spot * (-params.q * time).exp();
    let discount_k = strike * (-params.r * time).exp();

    let put_price = call_price - forward + discount_k;
    if !put_price.is_finite() {
        // Mirror the call-side fallback so put pricing degrades to BS rather than
        // returning zero on extreme parameters.
        let bs_call = black_scholes_call(spot, strike, time, params.r, params.q, params.v0.sqrt());
        return (bs_call - forward + discount_k).max(0.0);
    }
    put_price.max(0.0)
}

/// Black-Scholes call price (fallback for sigma_v ≈ 0).
fn black_scholes_call(spot: f64, strike: f64, time: f64, r: f64, q: f64, vol: f64) -> f64 {
    use crate::instruments::common_impl::models::closed_form::vanilla::bs_price;
    use crate::instruments::common_impl::parameters::OptionType;
    bs_price(spot, strike, r, q, vol, time, OptionType::Call)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;

    #[test]
    fn from_market_uses_defaults_when_market_is_empty() {
        let market = MarketContext::new();
        let params = HestonParams::from_market(&market, 0.05, 0.02).expect("valid defaults");
        assert_eq!(params.r, 0.05);
        assert_eq!(params.q, 0.02);
        assert_eq!(params.kappa, heston_defaults::KAPPA);
        assert_eq!(params.theta, heston_defaults::THETA);
        assert_eq!(params.sigma_v, heston_defaults::SIGMA_V);
        assert_eq!(params.rho, heston_defaults::RHO);
        assert_eq!(params.v0, heston_defaults::V0);
    }

    #[test]
    fn from_market_overrides_defaults_with_market_scalars() {
        let market = MarketContext::new()
            .insert_price("HESTON_KAPPA", MarketScalar::Unitless(1.5))
            .insert_price("HESTON_THETA", MarketScalar::Unitless(0.06))
            .insert_price("HESTON_SIGMA_V", MarketScalar::Unitless(0.4))
            .insert_price("HESTON_RHO", MarketScalar::Unitless(-0.5))
            .insert_price("HESTON_V0", MarketScalar::Unitless(0.05));
        let params = HestonParams::from_market(&market, 0.03, 0.01).expect("valid market");
        assert_eq!(params.kappa, 1.5);
        assert_eq!(params.theta, 0.06);
        assert_eq!(params.sigma_v, 0.4);
        assert_eq!(params.rho, -0.5);
        assert_eq!(params.v0, 0.05);
    }

    #[test]
    fn from_market_rejects_rho_at_boundary() {
        let market = MarketContext::new().insert_price("HESTON_RHO", MarketScalar::Unitless(1.0));
        let err = HestonParams::from_market(&market, 0.0, 0.0).expect_err("rho=1 invalid");
        assert!(err.to_string().contains("rho"));
    }

    #[test]
    fn from_market_rejects_negative_kappa() {
        let market =
            MarketContext::new().insert_price("HESTON_KAPPA", MarketScalar::Unitless(-0.1));
        let err = HestonParams::from_market(&market, 0.0, 0.0).expect_err("negative kappa");
        assert!(err.to_string().contains("kappa"));
    }

    /// Test that ψ_j(0) ≈ 1 for both probability characteristic functions.
    #[test]
    fn test_pj_char_function_at_zero() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let log_spot = 100.0_f64.ln();

        // At φ=0, ψ_j(0) should equal 1 (or very close)
        for j in [1u8, 2u8] {
            let psi = heston_pj_characteristic_function(j, 1e-10, 1.0, log_spot, &params);
            assert!(
                (psi.re - 1.0).abs() < 0.01,
                "ψ_{}(0) real part should be ~1, got {}",
                j,
                psi.re
            );
            assert!(
                psi.im.abs() < 0.01,
                "ψ_{}(0) imag part should be ~0, got {}",
                j,
                psi.im
            );
        }
    }

    /// Test that P1 and P2 are within valid probability range [0, 1].
    #[test]
    fn test_probabilities_in_valid_range() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let settings = HestonFourierSettings::default();

        // Test various moneyness levels
        for strike in [80.0, 100.0, 120.0] {
            let p1 = heston_pj(1, 100.0, strike, 1.0, &params, &settings);
            let p2 = heston_pj(2, 100.0, strike, 1.0, &params, &settings);

            assert!(
                (0.0..=1.0).contains(&p1),
                "P1 should be in [0,1], got {} for K={}",
                p1,
                strike
            );
            assert!(
                (0.0..=1.0).contains(&p2),
                "P2 should be in [0,1], got {} for K={}",
                p2,
                strike
            );

            // P1 >= P2 for calls (P1 is stock measure, P2 is money measure)
            assert!(
                p1 >= p2 - 1e-6,
                "P1 should be >= P2, got P1={}, P2={} for K={}",
                p1,
                p2,
                strike
            );
        }
    }

    /// Test that call price is positive and reasonable.
    #[test]
    fn test_heston_call_positive() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        let price = heston_call_price_fourier(100.0, 100.0, 1.0, &params);

        assert!(price > 0.0, "Call price should be positive, got {}", price);
        assert!(
            price < 100.0,
            "Call price should be less than spot, got {}",
            price
        );
    }

    /// Test put-call parity holds.
    #[test]
    fn test_heston_put_call_parity() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        let call = heston_call_price_fourier(100.0, 100.0, 1.0, &params);
        let put = heston_put_price_fourier(100.0, 100.0, 1.0, &params);

        // Put-call parity: C - P = S*exp(-qT) - K*exp(-rT)
        let lhs = call - put;
        let rhs = 100.0 * (-0.02_f64 * 1.0).exp() - 100.0 * (-0.05_f64 * 1.0).exp();

        assert!(
            (lhs - rhs).abs() < 0.01,
            "Put-call parity failed: C-P={} vs S*exp(-qT)-K*exp(-rT)={}",
            lhs,
            rhs
        );
    }

    /// Test convergence to Black-Scholes as vol-of-vol → 0.
    #[test]
    fn test_black_scholes_limit() {
        let vol = 0.2;
        let variance = vol * vol;

        // Heston with very small sigma_v should match Black-Scholes
        let params = HestonParams::new(
            0.05,     // r
            0.0,      // q
            2.0,      // kappa (doesn't matter when sigma_v=0)
            variance, // theta = v0 for consistency
            1e-12,    // sigma_v ≈ 0
            0.0,      // rho
            variance, // v0
        )
        .expect("valid");

        let heston_price = heston_call_price_fourier(100.0, 100.0, 1.0, &params);
        let bs_price = black_scholes_call(100.0, 100.0, 1.0, 0.05, 0.0, vol);

        assert!(
            (heston_price - bs_price).abs() < 0.01,
            "Heston should converge to BS: Heston={}, BS={}",
            heston_price,
            bs_price
        );
    }

    /// Test against the volatility/heston.rs implementation.
    ///
    /// Cross-validates our closed-form implementation against the
    /// HestonModel implementation in the volatility module.
    #[test]
    fn test_cross_validation_with_volatility_heston() {
        use crate::instruments::common_impl::models::volatility::heston::{
            HestonModel, HestonParameters,
        };

        // Test parameters
        let spot = 100.0;
        let strike = 100.0;
        let time = 0.5;
        let r = 0.05;
        let q = 0.02;
        let v0 = 0.04;
        let kappa = 2.0;
        let theta = 0.04;
        let sigma_v = 0.3;
        let rho = -0.7;

        // Our implementation
        let params = HestonParams::new(r, q, kappa, theta, sigma_v, rho, v0).expect("valid");
        let our_price = heston_call_price_fourier(spot, strike, time, &params);

        // Volatility module implementation
        let vol_params =
            HestonParameters::new(v0, kappa, theta, sigma_v, rho).expect("valid Heston params");
        let model = HestonModel::new(vol_params);
        let vol_price = model
            .price_european_call(spot, strike, time, r, q)
            .expect("Heston pricing should succeed");

        // Both implementations should produce the same price up to integration
        // noise. The two implementations use different quadrature schemes
        // (composite Gauss-Legendre here, adaptive GL in volatility/heston.rs)
        // so a small tolerance is expected, but the previous 0.1 tolerance was
        // far too loose — at this parameter set both schemes agree to ~5 bps,
        // and any drift beyond ~10 bps signals a real divergence between the
        // two implementations of the same algorithm.
        let diff_bps = (our_price - vol_price).abs() * 10_000.0 / our_price.max(1e-12);
        assert!(
            diff_bps < 10.0,
            "Heston implementations diverged by {:.2} bps at canonical params \
             (closed_form={:.6}, volatility module={:.6}). Cross-validation tolerance \
             tightened from the legacy 100bps to catch silent drift between the two \
             Fourier-inversion implementations.",
            diff_bps,
            our_price,
            vol_price
        );
    }

    /// Test a known reference case with reasonable parameters.
    ///
    /// Uses typical equity option parameters and validates the price
    /// is within an expected range based on Black-Scholes bounds.
    #[test]
    fn test_reference_typical_params() {
        let params = HestonParams::new(
            0.05, // r
            0.0,  // q
            2.0,  // kappa
            0.04, // theta
            0.3,  // sigma_v
            -0.5, // rho
            0.04, // v0
        )
        .expect("valid");

        let price = heston_call_price_fourier(100.0, 100.0, 0.5, &params);

        // With v0=0.04 (20% vol) and T=0.5, ATM call should be roughly 5-8
        // BS with 20% vol gives ~5.87 for these params
        assert!(
            price > 4.0 && price < 10.0,
            "Heston price {} should be in reasonable range for these parameters",
            price
        );
    }

    /// Test another reference case: ATM option with typical equity parameters.
    ///
    /// Parameters: S=100, K=100, T=1, r=0.05, q=0.02
    /// v0=0.04, kappa=2.0, theta=0.04, sigma=0.3, rho=-0.7
    #[test]
    fn test_reference_typical_equity() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        let call = heston_call_price_fourier(100.0, 100.0, 1.0, &params);
        let put = heston_put_price_fourier(100.0, 100.0, 1.0, &params);

        // With v0=0.04 (20% vol), ATM call should be roughly 8-10
        assert!(
            call > 5.0 && call < 15.0,
            "ATM call price {} should be reasonable",
            call
        );
        assert!(
            put > 3.0 && put < 12.0,
            "ATM put price {} should be reasonable",
            put
        );
    }

    /// Test OTM and ITM options have correct ordering.
    #[test]
    fn test_moneyness_ordering() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        let call_itm = heston_call_price_fourier(100.0, 90.0, 1.0, &params);
        let call_atm = heston_call_price_fourier(100.0, 100.0, 1.0, &params);
        let call_otm = heston_call_price_fourier(100.0, 110.0, 1.0, &params);

        // ITM > ATM > OTM for calls
        assert!(
            call_itm > call_atm,
            "ITM call {} should be > ATM call {}",
            call_itm,
            call_atm
        );
        assert!(
            call_atm > call_otm,
            "ATM call {} should be > OTM call {}",
            call_atm,
            call_otm
        );
    }

    /// Test expired option returns intrinsic value.
    #[test]
    fn test_expired_option() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        // ITM call
        let call_itm = heston_call_price_fourier(100.0, 90.0, 0.0, &params);
        assert!(
            (call_itm - 10.0).abs() < 1e-10,
            "Expired ITM call should be intrinsic: {}",
            call_itm
        );

        // OTM call
        let call_otm = heston_call_price_fourier(100.0, 110.0, 0.0, &params);
        assert!(
            call_otm.abs() < 1e-10,
            "Expired OTM call should be 0: {}",
            call_otm
        );

        // ITM put
        let put_itm = heston_put_price_fourier(100.0, 110.0, 0.0, &params);
        assert!(
            (put_itm - 10.0).abs() < 1e-10,
            "Expired ITM put should be intrinsic: {}",
            put_itm
        );
    }

    /// Test with extreme parameters to ensure stability.
    #[test]
    fn test_stability_extreme_params() {
        // High vol-of-vol
        let params_high_vov =
            HestonParams::new(0.05, 0.0, 5.0, 0.09, 1.0, -0.9, 0.09).expect("valid");
        let price = heston_call_price_fourier(100.0, 100.0, 1.0, &params_high_vov);
        assert!(
            price.is_finite() && price >= 0.0,
            "Should handle high vol-of-vol"
        );

        // Very short maturity
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let price_short = heston_call_price_fourier(100.0, 100.0, 0.01, &params);
        assert!(
            price_short.is_finite() && price_short >= 0.0,
            "Should handle short maturity"
        );

        // Deep OTM
        let price_deep_otm = heston_call_price_fourier(100.0, 200.0, 1.0, &params);
        assert!(
            price_deep_otm.is_finite() && price_deep_otm >= 0.0,
            "Should handle deep OTM"
        );

        // Deep ITM
        let price_deep_itm = heston_call_price_fourier(100.0, 50.0, 1.0, &params);
        assert!(
            price_deep_itm.is_finite() && price_deep_itm > 40.0,
            "Should handle deep ITM"
        );
    }

    /// Test improved accuracy for very short-dated options.
    #[test]
    fn test_short_maturity_adaptive() {
        let params = HestonParams::new(0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        // Very short maturity: T = 1 week
        let time = 7.0 / 365.0;
        let price = heston_call_price_fourier(100.0, 100.0, time, &params);

        // Should be close to BS with vol = sqrt(v0) = 0.2
        let bs = black_scholes_call(100.0, 100.0, time, 0.05, 0.0, 0.2);

        // With short maturity and moderate vol-of-vol, Heston ≈ BS
        assert!(
            (price - bs).abs() < 0.5,
            "Short-dated Heston={:.4} should be close to BS={:.4}",
            price,
            bs
        );
        assert!(price > 0.0, "Price must be positive");
    }

    /// Test that adaptive settings produce valid results across maturities.
    #[test]
    fn test_adaptive_settings_consistency() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        for &time in &[0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0] {
            let price = heston_call_price_fourier(100.0, 100.0, time, &params);
            assert!(
                price.is_finite() && price >= 0.0,
                "Price must be finite and non-negative for T={}: got {}",
                time,
                price
            );

            // Put-call parity must hold
            let put = heston_put_price_fourier(100.0, 100.0, time, &params);
            let parity =
                price - put - (100.0 * (-0.02 * time).exp() - 100.0 * (-0.05 * time).exp());
            assert!(
                parity.abs() < 0.1,
                "Put-call parity violated for T={}: residual={}",
                time,
                parity
            );
        }
    }

    /// Test multi-strike pricing matches the existing single-strike API.
    #[test]
    fn test_heston_call_strip_matches_single_strike_prices() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];

        let strip_prices = heston_call_prices_fourier(100.0, &strikes, 0.5, &params);

        assert_eq!(strip_prices.len(), strikes.len());
        for (idx, &strike) in strikes.iter().enumerate() {
            let single_price = heston_call_price_fourier(100.0, strike, 0.5, &params);
            assert!(
                (strip_prices[idx] - single_price).abs() < 1e-12,
                "strip price {} should match single-strike price {} for K={}",
                strip_prices[idx],
                single_price,
                strike
            );
        }
    }

    /// Test multi-strike put pricing matches the existing single-strike API.
    #[test]
    fn test_heston_put_strip_matches_single_strike_prices() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];

        let strip_prices = heston_put_prices_fourier(100.0, &strikes, 0.5, &params);

        assert_eq!(strip_prices.len(), strikes.len());
        for (idx, &strike) in strikes.iter().enumerate() {
            let single_price = heston_put_price_fourier(100.0, strike, 0.5, &params);
            assert!(
                (strip_prices[idx] - single_price).abs() < 1e-12,
                "strip put price {} should match single-strike put price {} for K={}",
                strip_prices[idx],
                single_price,
                strike
            );
        }
    }

    /// Test multi-strike pricing preserves expected call ordering across a strip.
    #[test]
    fn test_heston_call_strip_monotonic_in_strike() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let strikes: Vec<f64> = (75..=124).map(f64::from).collect();

        let strip_prices = heston_call_prices_fourier(100.0, &strikes, 1.0, &params);

        assert_eq!(strip_prices.len(), strikes.len());
        for window in strip_prices.windows(2) {
            assert!(
                window[0] >= window[1],
                "call strip should be non-increasing in strike: {:?}",
                window
            );
        }
    }

    /// Test strip pricing remains positive and respects put-call parity.
    #[test]
    fn test_heston_call_strip_consistency_across_many_strikes() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let spot: f64 = 100.0;
        let time: f64 = 1.0;
        let strikes: Vec<f64> = (75..=124).map(f64::from).collect();

        let strip_prices = heston_call_prices_fourier(spot, &strikes, time, &params);

        for (&strike, &call) in strikes.iter().zip(strip_prices.iter()) {
            assert!(
                call.is_finite() && call >= 0.0,
                "call strip price should be finite and non-negative"
            );

            let put = heston_put_price_fourier(spot, strike, time, &params);
            let parity =
                call - put - (spot * (-params.q * time).exp() - strike * (-params.r * time).exp());
            assert!(
                parity.abs() < 1e-10,
                "put-call parity should hold across strip for K={strike}: residual={parity}"
            );
        }
    }

    #[test]
    fn test_validation_rejects_invalid_params() {
        assert!(HestonParams::new(0.05, 0.02, -1.0, 0.04, 0.3, -0.7, 0.04).is_err());
        assert!(HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, 1.1, 0.04).is_err());
        assert!(HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.0).is_err());
    }

    /// W-02: unsupported `gl_order` must be rejected at the construction
    /// boundary rather than silently degrading the pricer to the per-strike path.
    #[test]
    fn fourier_settings_rejects_unsupported_gl_order() {
        let err = HestonFourierSettings::new(100.0, 100, 10, 1e-8)
            .expect_err("gl_order=10 has no Gauss-Legendre table and must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("gl_order"),
            "error should mention gl_order, got: {msg}"
        );

        // Supported orders construct successfully.
        for &order in &SUPPORTED_GL_ORDERS {
            assert!(
                HestonFourierSettings::new(100.0, 100, order, 1e-8).is_ok(),
                "gl_order={order} should be accepted"
            );
        }
    }

    /// W-02: `validate` rejects degenerate `panels` / `u_max` too.
    #[test]
    fn fourier_settings_rejects_degenerate_grid() {
        assert!(HestonFourierSettings::new(100.0, 0, 16, 1e-8).is_err());
        assert!(HestonFourierSettings::new(0.0, 100, 16, 1e-8).is_err());
        assert!(HestonFourierSettings::new(f64::NAN, 100, 16, 1e-8).is_err());
        // The default settings must always be valid.
        assert!(HestonFourierSettings::default().validate().is_ok());
    }

    /// W-03: with extreme parameters that overflow the characteristic function
    /// on a large fraction of grid nodes, the strip pricer must degrade to a
    /// Black-Scholes fallback (like the scalar Fourier path) rather than return
    /// a plausible-but-wrong finite number from a mass-losing integral.
    #[test]
    fn strip_pricer_falls_back_to_bs_on_corrupted_nodes() {
        // Extreme κ/θ/σᵥ with positive correlation and long maturity drives the
        // characteristic-function exponent past its real-part overflow limit on
        // the bulk of the integration grid, so `heston_pj_characteristic_function`
        // returns `Complex::ZERO` for those nodes.
        let params = HestonParams::new(0.05, 0.0, 10.0, 100.0, 90.0, 0.99, 90.0).expect("valid");
        let settings = HestonFourierSettings::default();
        let spot = 100.0;
        let strike = 100.0;
        let time = 30.0;

        let pricer =
            HestonStripPricer::new(spot, time, &params, &settings).expect("grid constructs");
        assert!(
            pricer.integrand_corrupted,
            "extreme params should corrupt a large fraction of integration nodes"
        );

        let strip_price = pricer.price_call(strike);
        let bs = black_scholes_call(spot, strike, time, params.r, params.q, params.v0.sqrt());

        // The strip price must equal the BS fallback exactly (same code path),
        // not a finite-but-wrong value from the corrupted Fourier integral.
        // Before the W-03 fix the strip path had no mass-loss fallback: the
        // corrupted Gil-Pelaez integral lost most of its mass and produced a
        // plausible-but-wrong call price with no diagnostic.
        assert!(
            (strip_price - bs).abs() < 1e-9,
            "corrupted strip pricer should return the BS fallback {bs}, got {strip_price}"
        );
        assert!(strip_price.is_finite(), "fallback price must be finite");
    }

    /// W-03: a well-behaved parameter set must NOT trip the corruption fallback
    /// — the strip price must still match the per-strike Fourier price.
    #[test]
    fn strip_pricer_no_false_corruption_on_normal_params() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let settings = HestonFourierSettings::default();
        let pricer = HestonStripPricer::new(100.0, 1.0, &params, &settings).expect("constructs");
        assert!(
            !pricer.integrand_corrupted,
            "benign parameters must not be flagged as corrupted"
        );
        let strip = pricer.price_call(100.0);
        let scalar = heston_call_price_fourier_with_settings(100.0, 100.0, 1.0, &params, &settings);
        assert!(
            (strip - scalar).abs() < 1e-9,
            "uncorrupted strip price {strip} should match scalar path {scalar}"
        );
    }

    #[test]
    fn test_characteristic_function_handles_extreme_inputs() {
        let params = HestonParams::new(0.05, 0.0, 0.1, 0.04, 1.0, 0.9, 0.04).expect("valid");
        let psi = heston_pj_characteristic_function(1, 0.0, 1.0, 100.0_f64.ln(), &params);
        assert!(
            psi.is_finite(),
            "characteristic function should stay finite"
        );
    }

    /// Audit item 6: `From<monte_carlo::HestonParams>` bypassed
    /// `HestonParams::new` validation entirely.
    ///
    /// Failure mode locked in: the Monte Carlo `HestonParams` accepts
    /// `ρ ∈ [-1, 1]` (inclusive), but the closed-form Fourier pricer requires
    /// `ρ ∈ (-1, 1)` (exclusive). A `ρ = ±1` Monte Carlo parameter set must NOT
    /// convert into a closed-form `HestonParams` silently — the conversion is
    /// now a `TryFrom` that re-runs the full validation.
    #[test]
    fn try_from_monte_carlo_params_revalidates_correlation_bound() {
        // ρ = 1.0 is valid for the MC process but invalid for the closed-form
        // Fourier pricer; the boundary value must be rejected on conversion.
        let mc_rho_one = finstack_monte_carlo::process::heston::HestonParams::new(
            0.05, 0.02, 2.0, 0.04, 0.3, 1.0, 0.04,
        )
        .expect("rho=1 is accepted by the Monte Carlo constructor");
        let converted: Result<HestonParams, _> = HestonParams::try_from(mc_rho_one);
        assert!(
            converted.is_err(),
            "rho=1 MC params must fail conversion to closed-form HestonParams"
        );

        let mc_rho_neg_one = finstack_monte_carlo::process::heston::HestonParams::new(
            0.05, 0.02, 2.0, 0.04, 0.3, -1.0, 0.04,
        )
        .expect("rho=-1 is accepted by the Monte Carlo constructor");
        assert!(
            HestonParams::try_from(mc_rho_neg_one).is_err(),
            "rho=-1 MC params must fail conversion to closed-form HestonParams"
        );

        // A well-formed MC parameter set still converts successfully.
        let mc_ok = finstack_monte_carlo::process::heston::HestonParams::new(
            0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04,
        )
        .expect("valid MC params");
        let cf = HestonParams::try_from(mc_ok).expect("valid MC params must convert");
        assert_eq!(cf.rho, -0.7);
        assert_eq!(cf.kappa, 2.0);
    }

    /// Audit item 5: the scalar `heston_pj` Fourier path silently integrated
    /// overflow-zeroed characteristic-function nodes (`Complex::ZERO`) into a
    /// finite-but-wrong probability — unlike the strip pricer, which counts
    /// corrupted nodes and falls back to Black-Scholes.
    ///
    /// Failure mode locked in: with parameters that overflow the characteristic
    /// function on a large fraction of integration nodes, the scalar
    /// `heston_call_price_fourier_with_settings` must degrade to the same
    /// Black-Scholes fallback the strip pricer uses, not return a plausible
    /// mass-losing Fourier price.
    #[test]
    fn scalar_fourier_falls_back_to_bs_on_corrupted_nodes() {
        // Same extreme parameter set the strip-pricer corruption test uses:
        // huge κ/θ/σᵥ + ρ≈1 + long maturity overflow the char-function
        // exponent on the bulk of the integration grid.
        let params = HestonParams::new(0.05, 0.0, 10.0, 100.0, 90.0, 0.99, 90.0).expect("valid");
        let settings = HestonFourierSettings::default();
        let spot = 100.0;
        let strike = 100.0;
        let time = 30.0;

        let scalar_price =
            heston_call_price_fourier_with_settings(spot, strike, time, &params, &settings);
        let bs = black_scholes_call(spot, strike, time, params.r, params.q, params.v0.sqrt());

        // The corrupted scalar path must return the BS fallback exactly, the
        // same way the strip pricer does — not a finite-but-wrong number from
        // a mass-losing Gil-Pelaez integral.
        assert!(
            (scalar_price - bs).abs() < 1e-9,
            "corrupted scalar Fourier pricer should return the BS fallback {bs}, \
             got {scalar_price}"
        );
        assert!(scalar_price.is_finite());
    }

    /// Audit item 5: a benign parameter set must NOT trip the scalar corruption
    /// fallback — the scalar Fourier price must still match the strip price.
    #[test]
    fn scalar_fourier_no_false_corruption_on_normal_params() {
        let params = HestonParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let settings = HestonFourierSettings::default();
        let scalar = heston_call_price_fourier_with_settings(100.0, 100.0, 1.0, &params, &settings);
        let strip = HestonStripPricer::new(100.0, 1.0, &params, &settings)
            .expect("constructs")
            .price_call(100.0);
        assert!(
            (scalar - strip).abs() < 1e-9,
            "benign params: scalar {scalar} should match strip {strip}, no false fallback"
        );
    }

    /// Audit item 4: the Gil-Pelaez probability integral was truncated at a
    /// fixed `u_max` with no residual-tail check, and the `[0, 1]` clamp hid the
    /// resulting truncation error.
    ///
    /// Failure mode locked in: `heston_pj_with_diagnostics` exposes the
    /// pre-clamp probability and an estimated truncation-tail mass. For a
    /// short-dated option (rapidly oscillating, slowly decaying integrand) the
    /// diagnostic must remain finite and the tail-mass estimate must be
    /// available so a caller can detect mis-truncation instead of silently
    /// trusting a clamped value.
    #[test]
    fn gil_pelaez_exposes_truncation_tail_diagnostic() {
        let params = HestonParams::new(0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04).expect("valid");

        // Short maturity with a deliberately too-small u_max: the integrand has
        // not decayed by u_max, so the truncation tail is non-negligible.
        let coarse = HestonFourierSettings::new(8.0, 40, 16, 1e-8).expect("valid settings");
        let diag = heston_pj_with_diagnostics(1, 100.0, 100.0, 0.02, &params, &coarse);
        assert!(
            diag.probability.is_finite() && diag.raw_probability.is_finite(),
            "diagnostic probabilities must be finite"
        );
        assert!(
            diag.tail_estimate.is_finite() && diag.tail_estimate >= 0.0,
            "tail-mass estimate must be a finite non-negative number, got {}",
            diag.tail_estimate
        );
        // The clamped probability is always a valid probability.
        assert!((0.0..=1.0).contains(&diag.probability));
        // A coarse/short-dated truncation must register a non-trivial tail so
        // the mis-truncation is observable rather than hidden by the clamp.
        assert!(
            diag.tail_estimate > 1e-6,
            "coarse u_max on a short-dated option must flag a non-negligible \
             truncation tail, got {}",
            diag.tail_estimate
        );

        // With a well-resolved grid the tail estimate must be small (the
        // integrand has genuinely decayed) — no false positive.
        let fine = HestonFourierSettings::for_maturity(1.0);
        let diag_fine = heston_pj_with_diagnostics(1, 100.0, 100.0, 1.0, &params, &fine);
        assert!(
            diag_fine.tail_estimate < 1e-3,
            "well-resolved integral must have a small truncation tail, got {}",
            diag_fine.tail_estimate
        );
    }
}
