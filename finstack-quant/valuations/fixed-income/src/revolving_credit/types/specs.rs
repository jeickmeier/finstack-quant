//! Draw/repayment schedules and stochastic process specifications for revolving credit.

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::validation;

/// Draw and repayment specification.
///
/// Determines whether the facility uses a known (deterministic) schedule
/// or stochastic utilization for Monte Carlo pricing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DrawRepaySpec {
    /// Deterministic schedule of draws and repayments.
    Deterministic(Vec<DrawRepayEvent>),

    /// Stochastic utilization for Monte Carlo simulation.
    Stochastic(Box<StochasticUtilizationSpec>),
}

/// A single draw or repayment event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DrawRepayEvent {
    /// Date of the draw or repayment.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,

    /// Amount being drawn or repaid (absolute value).
    pub amount: Money,

    /// True if this is a draw, false if it's a repayment.
    pub is_draw: bool,
}

/// Specification for stochastic utilization modeling.
///
/// Defines the stochastic process for Monte Carlo pricing with uncertain
/// draw/repayment patterns. Credit risk is incorporated via hazard-rate
/// survival weighting (no explicit default events). The estimator count,
/// antithetic flag and seed label are pricing settings, read from
/// `instrument_pricing_overrides.model_config` (`mc_paths`, `mc_antithetic`,
/// `mc_seed_scenario`); see [`RevolvingCreditMcRun::resolve`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StochasticUtilizationSpec {
    /// Utilization process specification.
    pub utilization_process: UtilizationProcess,

    /// Advanced Monte Carlo configuration (optional).
    ///
    /// When present, enables multi-factor modeling with credit spread
    /// and interest rate dynamics, correlation, and default modeling.
    pub mc_config: Option<McConfig>,
}

/// Resolved Monte Carlo sampling settings for a stochastic revolver.
///
/// Simulation is always deterministic: the seed is derived from the facility
/// id and the seed label, so bump-and-reprice sensitivities (DV01/CS01/Theta)
/// reuse the same random variates for base and bumped valuations (common
/// random numbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevolvingCreditMcRun {
    /// Number of independent estimators; antithetic sampling simulates two
    /// paths per estimator.
    pub num_paths: usize,
    /// Root RNG seed; every estimator draws from its own stream of it.
    pub seed: u64,
    /// Whether each estimator pairs its path with the sign-flipped mirror.
    pub antithetic: bool,
}

impl RevolvingCreditMcRun {
    const DEFAULT_NUM_PATHS: usize = 10_000;
    const DEFAULT_ANTITHETIC: bool = false;

    /// Resolve sampling settings from a facility's model configuration.
    ///
    /// `mc_paths` defaults to 10,000 and `mc_antithetic` defaults to `false`.
    /// The seed is `derive_seed(facility_id, mc_seed_scenario)`, with `"base"`
    /// when no label is set.
    ///
    /// # Arguments
    ///
    /// * `facility_id` - Facility identifier hashed into the seed.
    /// * `model_config` - The facility's
    ///   `instrument_pricing_overrides.model_config` (`mc_paths`,
    ///   `mc_antithetic`, `mc_seed_scenario`).
    ///
    /// # Errors
    ///
    /// Returns a validation error when `mc_paths` is zero or exceeds the
    /// workspace path cap.
    pub fn resolve(
        facility_id: &finstack_quant_core::types::InstrumentId,
        model_config: &finstack_quant_valuations_core::instruments::pricing_overrides::ModelConfig,
    ) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            num_paths:
                finstack_quant_valuations_core::instruments::pricing::monte_carlo::resolve_mc_paths(
                    model_config.mc_paths,
                    Self::DEFAULT_NUM_PATHS,
                )?,
            seed: model_config.mc_seed(facility_id),
            antithetic: model_config
                .mc_antithetic
                .unwrap_or(Self::DEFAULT_ANTITHETIC),
        })
    }
}

/// Advanced Monte Carlo configuration for revolving credit facilities.
///
/// Enables multi-factor modeling with credit risk, interest rate dynamics,
/// correlation between factors, and default modeling.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct McConfig {
    /// Correlation matrix (3x3) between [utilization, rate, credit].
    ///
    /// Must be symmetric, positive definite, with ones on diagonal.
    /// If None, factors are assumed independent.
    pub correlation_matrix: Option<[[f64; 3]; 3]>,

    /// Credit spread process specification. Recovery on default is the
    /// facility's `recovery_rate`; the hazard-to-spread mapping reads it there.
    pub credit_spread_process: CreditSpreadProcessSpec,

    /// Interest rate process specification (for floating rates).
    ///
    /// If None, assumes fixed rate (no stochastic dynamics).
    pub interest_rate_process: Option<InterestRateProcessSpec>,

    /// Optional utilization–credit correlation used when `correlation_matrix` is None.
    ///
    /// If provided, builds a 3×3 matrix with:
    ///   [ [1, 0, rho], [0, 1, 0], [rho, 0, 1] ]
    /// representing correlation between utilization and credit, with the rate
    /// factor uncorrelated (kept fixed in 2‑factor mode).
    #[serde(default)]
    pub util_credit_corr: Option<f64>,
}

impl McConfig {
    /// Validate the configuration parameters.
    ///
    /// Checks that:
    /// - Correlation matrix (if provided) is positive semi-definite
    /// - CIR parameters satisfy Feller condition if applicable
    /// - Credit spread parameters are valid
    ///
    /// # Returns
    ///
    /// `Ok(())` if all parameters are valid, otherwise returns an error
    /// describing the validation failure.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Validate correlation matrix if provided
        if let Some(corr) = self.correlation_matrix {
            // Check positive semi-definiteness
            finstack_quant_core::math::linalg::validate_correlation_matrix(
                &corr.iter().flatten().copied().collect::<Vec<_>>(),
                3,
            )?;
        }

        // Validate util_credit_corr if provided
        if let Some(rho) = self.util_credit_corr {
            validation::require_or(
                rho.abs() <= 1.0,
                finstack_quant_core::Error::Validation(
                    "util_credit_corr must lie in [-1, 1]".to_string(),
                ),
            )?;
        }

        // Validate credit spread process parameters
        match &self.credit_spread_process {
            CreditSpreadProcessSpec::Cir {
                kappa,
                theta,
                sigma,
                initial,
            } => {
                // All parameters must be non-negative
                validation::require_or(
                    *kappa > 0.0 && *theta >= 0.0 && *sigma >= 0.0 && *initial >= 0.0,
                    finstack_quant_core::Error::Validation("CIR credit spread process needs kappa > 0 and non-negative theta, sigma and initial".to_string()),
                )?;
            }
            CreditSpreadProcessSpec::Constant(spread) => {
                validation::require_or(
                    *spread >= 0.0,
                    finstack_quant_core::Error::Validation(
                        "constant credit spread must be non-negative".to_string(),
                    ),
                )?;
            }
            CreditSpreadProcessSpec::MarketAnchored {
                kappa,
                implied_vol,
                tenor_years,
                ..
            } => {
                validation::require_or(*kappa > 0.0 && *implied_vol >= 0.0, finstack_quant_core::Error::Validation("market-anchored credit spread process needs kappa > 0 and non-negative implied_vol".to_string()))?;
                if let Some(tenor) = tenor_years {
                    validation::require_or(
                        *tenor > 0.0,
                        finstack_quant_core::Error::Validation(
                            "market-anchored tenor_years must be positive".to_string(),
                        ),
                    )?;
                }
            }
        }

        // Validate interest rate process if provided
        if let Some(InterestRateProcessSpec::HullWhite1F {
            hw1f_mean_reversion: kappa,
            hw1f_sigma: sigma,
            ..
        }) = &self.interest_rate_process
        {
            validation::require_or(
                *kappa > 0.0 && *sigma >= 0.0,
                finstack_quant_core::Error::Validation(
                    "Hull-White process needs positive mean reversion and non-negative sigma"
                        .to_string(),
                ),
            )?;
        }
        if let Some(InterestRateProcessSpec::HullWhite1F { initial, theta, .. }) =
            &self.interest_rate_process
        {
            validation::validate_f64_finite(*initial, "Hull-White initial short rate")?;
            validation::validate_f64_finite(*theta, "Hull-White mean-reversion level")?;
        }

        Ok(())
    }
}

/// Credit spread process specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CreditSpreadProcessSpec {
    /// CIR process for stochastic credit spread/hazard rate.
    ///
    /// Models credit spread as: dλ_t = κ(θ - λ_t)dt + σ√λ_t dW_t
    Cir {
        /// Mean reversion speed (κ)
        kappa: f64,
        /// Long-term mean (θ)
        theta: f64,
        /// Volatility (σ)
        sigma: f64,
        /// Initial credit spread
        initial: f64,
    },
    /// Constant credit spread (no dynamics).
    Constant(f64),

    /// Market-anchored credit spread process calibrated to a hazard curve and CDS option vol.
    ///
    /// At the simulation anchor (the later of valuation and commitment date),
    /// the initial spread is set from the contemporaneous hazard and the mean
    /// level is anchored to conditional survival over the remaining tenor.
    /// Volatility is scaled from the CDS index option implied volatility.
    MarketAnchored {
        /// Credit curve identifier in `MarketContext` used to anchor spreads.
        credit_curve_id: CurveId,
        /// Mean reversion speed (κ) of the CIR process.
        kappa: f64,
        /// Annualized CDS (index) option implied volatility for spreads.
        implied_vol: f64,
        /// Optional tenor in years; if None, uses facility maturity horizon.
        #[serde(default)]
        tenor_years: Option<f64>,
    },
}

impl CreditSpreadProcessSpec {
    /// Short wire-style name of the process variant, for diagnostics.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Cir { .. } => "cir",
            Self::Constant(_) => "constant",
            Self::MarketAnchored { .. } => "market_anchored",
        }
    }
}

/// Interest rate process specification (for floating rates).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum InterestRateProcessSpec {
    /// Hull-White 1-factor model for short rate.
    ///
    /// Models short rate as: dr_t = κ[θ(t) - r_t]dt + σ dW_t
    ///
    /// Two calibration modes, selected by `hw1f_sigma`:
    /// - `hw1f_sigma > 0` (stochastic): the pricer fits a time-dependent θ(t)
    ///   to the facility's discount curve and reads the initial short rate
    ///   from that curve; the supplied `initial`/`theta` are ignored.
    /// - `hw1f_sigma == 0` (deterministic parity mode): the supplied constant
    ///   `initial` and `theta` are used verbatim, with no curve fitting.
    ///
    /// Consequently the σ → 0 limit of the stochastic mode does **not**
    /// converge to the σ = 0 branch unless the supplied `initial`/`theta`
    /// are themselves curve-consistent. When running a volatility ladder
    /// down to zero, keep σ strictly positive for curve-fitted dynamics.
    #[serde(rename = "hull_white_1f")]
    HullWhite1F {
        /// Hull-White 1F mean reversion speed κ, per year.
        hw1f_mean_reversion: f64,
        /// Hull-White 1F short-rate volatility σ, absolute annual decimal
        /// (`0.01` = 100 bp).
        hw1f_sigma: f64,
        /// Initial short rate used only when `hw1f_sigma == 0`. For a stochastic
        /// process, the pricer derives the initial rate from the facility's
        /// discount curve at the valuation anchor.
        initial: f64,
        /// Constant mean reversion level used only when `hw1f_sigma == 0`. For a
        /// stochastic process, the pricer fits a time-dependent θ(t) to the
        /// facility's discount curve.
        theta: f64,
    },
}

/// Utilization process for stochastic draws/repayments.
///
/// For the 80/20 implementation, we support a single mean-reverting process.
/// This can be extended in the future to support other processes (jump-diffusion,
/// regime-switching, etc.).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum UtilizationProcess {
    /// Mean-reverting utilization rate process.
    ///
    /// Models utilization as reverting to a long-run level `theta` at speed
    /// `kappa` with volatility `sigma`. Uses Ornstein-Uhlenbeck dynamics:
    /// dU(t) = kappa * (theta - U(t)) * dt + sigma * dW(t)
    ///
    /// The simulated process is a **clamped** OU: each step uses the exact OU
    /// transition and is then clamped to `[0, 1]`. When `theta` is well
    /// inside the interval and `sigma` is moderate the clamp fires only
    /// on rare tail excursions, but with `theta` near 0 or 1 and/or
    /// high `sigma` the clamp binds frequently and biases the simulated
    /// mean toward the interior. Keep the stationary standard deviation
    /// `sigma / sqrt(2 * kappa)` small relative to the distance from
    /// `theta` to the nearest boundary.
    MeanReverting {
        /// Long-run utilization level θ, as a fraction in `[0, 1]`.
        theta: f64,
        /// Mean-reversion speed κ, per year.
        kappa: f64,
        /// Utilization volatility σ, annualized (absolute utilization units).
        sigma: f64,
        /// Sensitivity of the utilization target to the simulated credit
        /// spread (adverse selection), as a decimal per unit of relative
        /// spread change.
        ///
        /// The target used by the OU step becomes
        /// `θ(t) = clamp(theta + spread_sensitivity · (s(t) / s(0) − 1), 0, 1)`
        /// where `s(t)` is the simulated spread and `s(0)` its initial level,
        /// so a spread that doubles raises the target by `spread_sensitivity`.
        /// Defaults to `0.0` (no link); the utilization/credit shock
        /// correlation in `McConfig` applies on top of it. Ignored when the
        /// facility has no credit-spread process.
        #[serde(default)]
        spread_sensitivity: f64,
    },
}
