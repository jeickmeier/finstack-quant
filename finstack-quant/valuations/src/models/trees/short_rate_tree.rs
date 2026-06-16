//! Short-rate tree models for bond valuation with embedded options.
//!
//! Implements curve-consistent short-rate trees for pricing callable/putable bonds
//! and calculating Option-Adjusted Spread (OAS). Uses industry-standard models
//! like Ho-Lee and Black-Derman-Toy.
//!
//! # Volatility Conventions
//!
//! **Critical**: The volatility parameter interpretation depends on the model type:
//!
//! | Model | Vol Type | Parameter | Formula | Typical Range |
//! |-------|----------|-----------|---------|---------------|
//! | Ho-Lee | Normal/Absolute | σ (bps/yr) | dr = θdt + σdW | 50-150 bps (0.005-0.015) |
//! | BDT | Lognormal/Relative | σ (%) | dr/r = θdt + σdW | 15-30% (0.15-0.30) |
//!
//! ## Converting Between Conventions
//!
//! Use `finstack_quant_core::math::volatility::convert_atm_volatility` to convert:
//!
//! ```ignore
//! use finstack_quant_core::math::volatility::{convert_atm_volatility, VolatilityConvention};
//!
//! let normal_vol = 0.01;
//! let rate_level = 0.05;
//!
//! let lognormal_vol = convert_atm_volatility(
//!     normal_vol,
//!     VolatilityConvention::Normal,
//!     VolatilityConvention::Lognormal,
//!     rate_level,
//!     1.0,
//! )?;
//! assert!(lognormal_vol > 0.15 && lognormal_vol < 0.25);
//!
//! let back_to_normal = convert_atm_volatility(
//!     lognormal_vol,
//!     VolatilityConvention::Lognormal,
//!     VolatilityConvention::Normal,
//!     rate_level,
//!     1.0,
//! )?;
//! assert!((back_to_normal - normal_vol).abs() < 1e-10);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! ## Calibration Sources
//!
//! - **Swaption market**: ATM swaption vols are typically quoted in normal (bps)
//! - **Cap/floor market**: Often quoted in lognormal (Black vol)
//! - **Historical**: Calculate from rate time series

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_core::{Error, Result};

use super::hull_white_tree::HullWhiteTree;
use super::tree_framework::{
    price_recombining_tree, state_keys, CachedValues, NodeState, RecombiningInputs, TreeBranching,
    TreeGreeks, TreeModel, TreeValuator,
};

/// Default normal (absolute) volatility for Ho-Lee model.
///
/// 100 basis points per year, typical for developed market government bonds
/// in a normal rate environment (2-5% rates).
pub const DEFAULT_NORMAL_VOL: f64 = 0.01; // 100 bps/yr

/// Default lognormal (relative) volatility for Black-Derman-Toy model.
///
/// 20% annualized, typical for developed market government bonds.
/// This corresponds to ~100 bps normal vol at a 5% rate level.
pub const DEFAULT_LOGNORMAL_VOL: f64 = 0.20; // 20%

// ============================================================================
// Short-Rate Model Types
// ============================================================================

/// Compounding convention for per-node discount factors in the short-rate tree.
///
/// | Convention | Formula | Use Case |
/// |------------|---------|----------|
/// | `Continuous` | `exp(-r * dt)` | Default; matches continuous short-rate dynamics |
/// | `Simple` | `1 / (1 + r * dt)` | Money-market / Bloomberg BDT convention |
/// | `SemiAnnual` | `(1 + r/2)^(-2 * dt)` | US bond market convention |
/// | `Quarterly` | `(1 + r/4)^(-4 * dt)` | Quarterly compounding |
/// | `Monthly` | `(1 + r/12)^(-12 * dt)` | Monthly compounding |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreeCompounding {
    /// Continuous compounding: `df = exp(-r * dt)`.
    #[default]
    Continuous,
    /// Simple (money-market) compounding: `df = 1 / (1 + r * dt)`.
    Simple,
    /// Semi-annual compounding: `df = (1 + r/2)^(-2 * dt)`.
    SemiAnnual,
    /// Quarterly compounding: `df = (1 + r/4)^(-4 * dt)`.
    Quarterly,
    /// Monthly compounding: `df = (1 + r/12)^(-12 * dt)`.
    Monthly,
}

impl TreeCompounding {
    /// Compute the per-step discount factor for a given rate and time step.
    ///
    /// Returns a positive discount factor. For pathological inputs (e.g.,
    /// deeply negative rates with simple compounding where `1 + r*dt <= 0`),
    /// the base is clamped to a small positive value to avoid negative or
    /// NaN discount factors.
    #[inline]
    pub fn df(self, rate: f64, dt: f64) -> f64 {
        const FLOOR: f64 = 1e-15;
        match self {
            Self::Continuous => (-rate * dt).exp(),
            Self::Simple => {
                let denom = 1.0 + rate * dt;
                1.0 / denom.max(FLOOR)
            }
            Self::SemiAnnual => {
                let base = (1.0 + rate / 2.0).max(FLOOR);
                base.powf(-2.0 * dt)
            }
            Self::Quarterly => {
                let base = (1.0 + rate / 4.0).max(FLOOR);
                base.powf(-4.0 * dt)
            }
            Self::Monthly => {
                let base = (1.0 + rate / 12.0).max(FLOOR);
                base.powf(-12.0 * dt)
            }
        }
    }

    /// Invert [`df`](Self::df): the per-step rate under this convention that
    /// reproduces the given discount factor over `dt`.
    ///
    /// Returns `rate` such that `self.df(rate, dt) = df`. For `dt ≈ 0` or a
    /// non-positive `df` the continuous-equivalent fallback is used.
    #[inline]
    pub fn rate_from_df(self, df: f64, dt: f64) -> f64 {
        if dt.abs() < f64::EPSILON || df <= 0.0 {
            tracing::warn!(
                "TreeCompounding::rate_from_df: degenerate input df={df:.6e}, dt={dt}, \
                 convention={self:?}; returning 0"
            );
            return 0.0;
        }
        match self {
            Self::Continuous => -df.ln() / dt,
            Self::Simple => (1.0 / df - 1.0) / dt,
            Self::SemiAnnual => 2.0 * (df.powf(-1.0 / (2.0 * dt)) - 1.0),
            Self::Quarterly => 4.0 * (df.powf(-1.0 / (4.0 * dt)) - 1.0),
            Self::Monthly => 12.0 * (df.powf(-1.0 / (12.0 * dt)) - 1.0),
        }
    }

    /// Convert a rate under this convention to the equivalent continuous rate.
    ///
    /// Returns `r_cont` such that `exp(-r_cont * dt) = self.df(rate, dt)`.
    #[inline]
    pub fn to_continuous(self, rate: f64, dt: f64) -> f64 {
        if dt.abs() < f64::EPSILON {
            return rate;
        }
        let d = self.df(rate, dt);
        if d > 0.0 {
            -d.ln() / dt
        } else {
            tracing::warn!(
                "TreeCompounding::to_continuous: non-positive DF {d:.6e} for rate={rate}, \
                 dt={dt}, convention={self:?}; falling back to raw rate"
            );
            rate
        }
    }
}

/// Short-rate tree model types.
///
/// Each model has distinct volatility conventions and mathematical properties:
///
/// | Model | Vol Type | Negative Rates | Mean Reversion | Use Case |
/// |-------|----------|----------------|----------------|----------|
/// | Ho-Lee | Normal | ✅ Yes | ❌ No | Low/negative rate environments |
/// | BDT/BK | Lognormal | ❌ No | ✅ Yes (κ ≠ 0 → trinomial BK lattice) | Traditional positive rate environments |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortRateModel {
    /// Ho-Lee model: Gaussian/normal short rates.
    ///
    /// ## Rate Dynamics
    /// ```text
    /// dr = θ(t)dt + σdW
    /// ```
    /// where:
    /// - `θ(t)` is calibrated to match the discount curve
    /// - `σ` is the **normal volatility** (absolute, in rate units like 0.01 = 100 bps)
    ///
    /// ## Properties
    /// - ✅ Handles negative rates naturally
    /// - ❌ No mean reversion (rates can drift arbitrarily)
    /// - Analytically tractable
    ///
    /// ## Typical Volatility Range
    /// - Low rates (<2%): 50-80 bps (0.005-0.008)
    /// - Normal rates (2-5%): 80-120 bps (0.008-0.012)
    /// - High rates (>5%): 100-150 bps (0.010-0.015)
    /// - Crisis: 150-300 bps (0.015-0.030)
    HoLee,

    /// Black-Derman-Toy / Black-Karasinski model: Lognormal short rates.
    ///
    /// ## Rate Dynamics
    /// ```text
    /// d(ln r) = [θ(t) - κ ln r] dt + σ dW
    /// ```
    /// where:
    /// - `θ(t)` is calibrated to match the discount curve
    /// - `σ` is the **lognormal volatility** (relative, like 0.20 = 20%)
    /// - `κ` is the mean reversion speed (0 recovers standard BDT)
    ///
    /// ## Properties
    /// - ❌ Cannot handle negative rates (rates stay positive)
    /// - When `κ = 0`: standard BDT with constant lognormal volatility on a
    ///   binomial lattice
    /// - When `κ > 0`: Black-Karasinski on a trinomial lattice in x = ln r
    ///   (Hull-White geometry with edge branch switching); terminal log-rate
    ///   dispersion tightens toward `σ√((1-e^{-2κT})/(2κ))`
    /// - Lognormal distribution matches cap/floor market conventions
    ///
    /// ## Typical Volatility Range
    /// - Low vol environment: 10-15% (0.10-0.15)
    /// - Normal market: 15-25% (0.15-0.25)
    /// - High vol/stress: 25-40% (0.25-0.40)
    ///
    /// ## Important
    /// ⚠️ The default 1% volatility in older code is **far too low** for BDT.
    /// Use [`DEFAULT_LOGNORMAL_VOL`] (20%) or calibrate to swaption market.
    BlackDermanToy,
}

/// Configuration for short-rate tree construction.
///
/// # Volatility Convention
///
/// ⚠️ **Critical**: The `volatility` field has different interpretations depending on the model:
///
/// | Model | Volatility Type | Example |
/// |-------|-----------------|---------|
/// | [`ShortRateModel::HoLee`] | Normal (absolute) | 0.01 = 100 bps/yr |
/// | [`ShortRateModel::BlackDermanToy`] | Lognormal (relative) | 0.20 = 20%/yr |
///
/// Use the helper constructors ([`ShortRateTreeConfig::ho_lee`], [`ShortRateTreeConfig::bdt`])
/// or `finstack_quant_core::math::volatility::convert_atm_volatility` to avoid convention errors.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::models::trees::short_rate_tree::{
///     ShortRateTreeConfig, ShortRateModel, DEFAULT_NORMAL_VOL, DEFAULT_LOGNORMAL_VOL,
/// };
///
/// // Ho-Lee with 100 bps normal vol (recommended for negative rate environments)
/// let ho_lee = ShortRateTreeConfig::ho_lee(100, 0.01);
/// assert_eq!(ho_lee.model, ShortRateModel::HoLee);
///
/// // BDT with 20% lognormal vol (recommended for positive rate environments)
/// let bdt = ShortRateTreeConfig::bdt(100, 0.20, 0.03);
/// assert_eq!(bdt.model, ShortRateModel::BlackDermanToy);
///
/// // Use defaults with model-appropriate volatility
/// let ho_lee_default = ShortRateTreeConfig::default_ho_lee(100);
/// assert_eq!(ho_lee_default.volatility, DEFAULT_NORMAL_VOL);
///
/// let bdt_default = ShortRateTreeConfig::default_bdt(100);
/// assert_eq!(bdt_default.volatility, DEFAULT_LOGNORMAL_VOL);
/// ```
#[derive(Debug, Clone)]
pub struct ShortRateTreeConfig {
    /// Number of time steps in the tree.
    ///
    /// More steps improve accuracy but increase computation time O(n²).
    /// Typical values: 50 (fast), 100 (standard), 200+ (high precision).
    pub steps: usize,

    /// Tree model type determining rate dynamics and volatility interpretation.
    pub model: ShortRateModel,

    /// Interest rate volatility (annualized).
    ///
    /// ⚠️ **Interpretation depends on model**:
    /// - **Ho-Lee**: Normal volatility in rate units (0.01 = 100 bps/yr)
    /// - **BDT**: Lognormal volatility as proportion (0.20 = 20%/yr)
    ///
    /// See [`ShortRateModel`] for typical ranges per model type.
    pub volatility: f64,

    /// Mean reversion parameter.
    ///
    /// Controls how quickly rates revert to the long-term mean.
    /// - Typical values: 0.01-0.10 (1-10% per year)
    /// - Higher values = faster reversion, less rate dispersion
    /// - Ho-Lee: not supported (breaks lattice recombination); use
    ///   `HullWhiteTree` for mean-reverting normal models
    /// - BDT/Black-Karasinski: κ = 0 calibrates standard binomial BDT;
    ///   κ > 0 calibrates a trinomial Black-Karasinski lattice in x = ln r
    pub mean_reversion: Option<f64>,

    /// Tree branching type (binomial or trinomial).
    ///
    /// - **Binomial**: Standard two-branch tree (up/down)
    /// - **Trinomial**: Three-branch tree (up/mid/down) for models with
    ///   trinomial calibration support
    ///
    /// Default: Binomial. Use trinomial only with a matching calibrated lattice.
    pub branching: TreeBranching,

    /// Per-node discount factor convention.
    ///
    /// Controls whether calibration and pricing use continuous `exp(-r*dt)` or
    /// simple `1/(1+r*dt)` compounding. Bloomberg's lognormal OAS model uses
    /// simple compounding; the default is continuous for backward compatibility.
    pub compounding: TreeCompounding,
}

impl Default for ShortRateTreeConfig {
    /// Default configuration using Ho-Lee model with appropriate normal volatility.
    ///
    /// For BDT model, use [`ShortRateTreeConfig::default_bdt`] instead.
    fn default() -> Self {
        Self::default_ho_lee(100)
    }
}

impl ShortRateTreeConfig {
    /// Create a Ho-Lee configuration with specified normal volatility.
    ///
    /// Uses binomial branching by default. For trinomial branching,
    /// use [`with_trinomial`](Self::with_trinomial) after construction.
    ///
    /// # Arguments
    ///
    /// * `steps` - Number of tree steps (50-200 typical)
    /// * `normal_vol` - Normal volatility in rate units (e.g., 0.01 = 100 bps/yr)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::models::trees::short_rate_tree::ShortRateTreeConfig;
    ///
    /// // 100 steps, 80 bps normal vol
    /// let config = ShortRateTreeConfig::ho_lee(100, 0.008);
    /// ```
    pub fn ho_lee(steps: usize, normal_vol: f64) -> Self {
        Self {
            steps,
            model: ShortRateModel::HoLee,
            volatility: normal_vol,
            mean_reversion: None,
            branching: TreeBranching::Binomial,
            compounding: TreeCompounding::default(),
        }
    }

    /// Create a Black-Derman-Toy / Black-Karasinski configuration.
    ///
    /// Uses binomial branching with state-price recursion calibration.
    ///
    /// # Arguments
    ///
    /// * `steps` - Number of tree steps (50-200 typical)
    /// * `lognormal_vol` - Lognormal volatility (e.g., 0.20 = 20%/yr)
    /// * `mean_reversion` - Mean reversion speed; `0.0` calibrates standard
    ///   binomial BDT, any positive value calibrates a trinomial
    ///   Black-Karasinski lattice in x = ln r
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::models::trees::short_rate_tree::ShortRateTreeConfig;
    ///
    /// // 100 steps, 20% lognormal vol
    /// let config = ShortRateTreeConfig::bdt(100, 0.20, 0.0);
    /// ```
    pub fn bdt(steps: usize, lognormal_vol: f64, mean_reversion: f64) -> Self {
        Self {
            steps,
            model: ShortRateModel::BlackDermanToy,
            volatility: lognormal_vol,
            mean_reversion: Some(mean_reversion),
            branching: TreeBranching::Binomial,
            compounding: TreeCompounding::default(),
        }
    }

    /// Set the per-node compounding convention.
    #[must_use]
    pub fn with_compounding(mut self, compounding: TreeCompounding) -> Self {
        self.compounding = compounding;
        self
    }

    /// Create Ho-Lee configuration with default normal volatility (100 bps).
    ///
    /// Suitable for developed market government bonds in normal rate environments.
    pub fn default_ho_lee(steps: usize) -> Self {
        Self::ho_lee(steps, DEFAULT_NORMAL_VOL)
    }

    /// Create BDT configuration with default lognormal volatility (20%).
    ///
    /// Suitable for developed market government bonds with positive rates.
    /// Uses the non-mean-reverting (κ = 0) binomial BDT calibration.
    pub fn default_bdt(steps: usize) -> Self {
        Self::bdt(steps, DEFAULT_LOGNORMAL_VOL, 0.0)
    }

    /// Set trinomial branching.
    ///
    /// The selected model must calibrate a matching `2 * step + 1` lattice.
    #[must_use]
    pub fn with_trinomial(mut self) -> Self {
        self.branching = TreeBranching::Trinomial;
        self
    }

    /// Set binomial branching (standard two-branch tree).
    #[must_use]
    pub fn with_binomial(mut self) -> Self {
        self.branching = TreeBranching::Binomial;
        self
    }

    /// Create configuration from normal volatility, automatically selecting
    /// the appropriate model based on rate environment.
    ///
    /// # Arguments
    ///
    /// * `steps` - Number of tree steps
    /// * `normal_vol` - Normal volatility in rate units (e.g., 0.01 = 100 bps)
    /// * `rate_level` - Current/reference rate level for model selection
    ///
    /// # Model Selection
    ///
    /// - If `rate_level < 0.01` (1%): Uses Ho-Lee (handles negative rates)
    /// - Otherwise: Uses BDT with converted lognormal vol
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::models::trees::short_rate_tree::{
    ///     ShortRateTreeConfig, ShortRateModel,
    /// };
    ///
    /// // Low rate environment → Ho-Lee
    /// let config = ShortRateTreeConfig::from_normal_vol(100, 0.008, 0.005)?;
    /// assert_eq!(config.model, ShortRateModel::HoLee);
    ///
    /// // Normal rate environment → BDT with converted vol
    /// let config = ShortRateTreeConfig::from_normal_vol(100, 0.01, 0.05)?;
    /// assert_eq!(config.model, ShortRateModel::BlackDermanToy);
    /// // Vol should be approximately 20% (price-matching conversion)
    /// assert!(config.volatility > 0.15 && config.volatility < 0.25);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_normal_vol(steps: usize, normal_vol: f64, rate_level: f64) -> Result<Self> {
        if rate_level < 0.01 {
            // Low/negative rate environment: use Ho-Lee
            Ok(Self::ho_lee(steps, normal_vol))
        } else {
            // Positive rate environment: use BDT with converted vol
            let lognormal_vol = finstack_quant_core::math::volatility::convert_atm_volatility(
                normal_vol,
                finstack_quant_core::math::volatility::VolatilityConvention::Normal,
                finstack_quant_core::math::volatility::VolatilityConvention::Lognormal,
                rate_level,
                1.0,
            )?;
            Ok(Self::bdt(steps, lognormal_vol, 0.0))
        }
    }
}

use std::sync::Arc;

/// Result of short-rate tree calibration with quality metrics.
///
/// Provides diagnostic information about calibration quality, allowing
/// users to assess whether the tree is suitable for their use case.
#[derive(Debug, Clone, Default)]
pub struct CalibrationResult {
    /// Maximum calibration error in basis points.
    pub max_error_bps: f64,
    /// Step at which maximum error occurred.
    pub max_error_step: usize,
    /// Number of steps where the solver failed and fallback was used.
    pub fallback_count: usize,
    /// Whether calibration completed successfully.
    pub converged: bool,
}

impl CalibrationResult {
    /// Returns true if calibration quality is acceptable (max error < 1bp, no fallbacks).
    #[must_use]
    pub fn is_acceptable(&self) -> bool {
        self.converged && self.max_error_bps < 1.0 && self.fallback_count == 0
    }

    /// Returns true if calibration quality is good (max error < 0.1bp).
    #[must_use]
    pub fn is_good(&self) -> bool {
        self.converged && self.max_error_bps < 0.1 && self.fallback_count == 0
    }
}

/// Calibrated Black-Karasinski trinomial lattice data (κ ≠ 0).
///
/// The lattice lives in x = ln r with Hull-White trinomial geometry: node
/// spacing `dx = σ√(3Δt)`, width capped at `j_max` with branch switching at
/// the edges, and per-node mean-reverting transition probabilities. The
/// short rate at node (i, j) is `r = exp(a_i + (j − j_max_i)·dx)` where the
/// per-step additive shift `a_i` is calibrated to the discount curve via
/// Arrow-Debreu forward induction .
#[derive(Debug, Clone)]
struct BkTrinomialLattice {
    /// Width cap on |j| (Hull-White branch-switching boundary)
    j_max: usize,
    /// Per-step per-node transition probabilities `(p_up, p_mid, p_down)`
    probs: Vec<Vec<(f64, f64, f64)>>,
}

/// Short-rate tree for valuing bonds with embedded options
#[derive(Debug, Clone)]
pub struct ShortRateTree {
    config: ShortRateTreeConfig,
    /// Calibrated short rates at each node: rates[step][node]
    rates: Arc<Vec<Vec<f64>>>,
    /// Transition probabilities: probs[step] gives (p_up, p_down) for that step
    probs: Vec<(f64, f64)>,
    /// Time steps in years
    time_steps: Vec<f64>,
    /// Discount curve used for calibration
    calibration_curve_id: CurveId,
    /// Calibration quality metrics (populated after calibration).
    calibration_quality: Option<CalibrationResult>,
    /// Trinomial Black-Karasinski lattice (set when BDT model has κ ≠ 0).
    bk_trinomial: Option<BkTrinomialLattice>,
}

impl ShortRateTree {
    /// Create a new short-rate tree with the given configuration.
    pub fn new(config: ShortRateTreeConfig) -> Self {
        Self {
            config,
            rates: Arc::new(Vec::new()),
            probs: Vec::new(),
            time_steps: Vec::new(),
            calibration_curve_id: CurveId::new(""),
            calibration_quality: None,
            bk_trinomial: None,
        }
    }

    /// Returns the calibration result if calibration has been performed.
    ///
    /// # Returns
    ///
    /// - `Some(CalibrationResult)` with quality metrics if calibrated
    /// - `None` if not yet calibrated
    #[must_use]
    pub fn calibration_result(&self) -> Option<&CalibrationResult> {
        self.calibration_quality.as_ref()
    }

    /// Create a Ho-Lee tree with specified normal (absolute) volatility.
    ///
    /// # Arguments
    ///
    /// * `steps` - Number of tree steps (50-200 typical)
    /// * `normal_vol` - Normal volatility in rate units (e.g., 0.01 = 100 bps/yr)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::models::trees::short_rate_tree::ShortRateTree;
    ///
    /// // Ho-Lee with 100 bps annual volatility
    /// let tree = ShortRateTree::ho_lee(100, 0.01);
    /// ```
    pub fn ho_lee(steps: usize, normal_vol: f64) -> Self {
        Self::new(ShortRateTreeConfig::ho_lee(steps, normal_vol))
    }

    /// Create a Black-Derman-Toy tree with specified lognormal (relative) volatility.
    ///
    /// # Arguments
    ///
    /// * `steps` - Number of tree steps (50-200 typical)
    /// * `lognormal_vol` - Lognormal volatility (e.g., 0.20 = 20%/yr)
    /// * `mean_reversion` - `0.0` for standard binomial BDT; positive values
    ///   calibrate a trinomial Black-Karasinski lattice in x = ln r
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::models::trees::short_rate_tree::ShortRateTree;
    ///
    /// // BDT with 20% lognormal volatility
    /// let tree = ShortRateTree::black_derman_toy(100, 0.20, 0.0);
    /// ```
    ///
    /// # Warning
    ///
    /// ⚠️ The volatility parameter is **lognormal** (relative), not normal (absolute).
    /// A value of 0.20 means 20% annual rate volatility, not 20 bps.
    /// Use `finstack_quant_core::math::volatility::convert_atm_volatility` to convert from normal if needed.
    pub fn black_derman_toy(steps: usize, lognormal_vol: f64, mean_reversion: f64) -> Self {
        Self::new(ShortRateTreeConfig::bdt(
            steps,
            lognormal_vol,
            mean_reversion,
        ))
    }

    /// Create a Ho-Lee tree with default normal volatility (100 bps).
    pub fn default_ho_lee(steps: usize) -> Self {
        Self::new(ShortRateTreeConfig::default_ho_lee(steps))
    }

    /// Create a BDT tree with default lognormal volatility (20%).
    pub fn default_bdt(steps: usize) -> Self {
        Self::new(ShortRateTreeConfig::default_bdt(steps))
    }

    /// Calibrate the tree to match a given discount curve.
    ///
    /// The `curve_id` is stored so that [`calculate_greeks`](TreeModel::calculate_greeks)
    /// can look up the curve from the `MarketContext` when recalibrating bumped
    /// trees for vega and theta.
    pub fn calibrate(
        &mut self,
        curve_id: &CurveId,
        discount_curve: &dyn Discounting,
        time_to_maturity: f64,
    ) -> Result<()> {
        self.calibration_curve_id = curve_id.clone();

        // Build time grid
        let dt = time_to_maturity / self.config.steps as f64;
        self.time_steps = (0..=self.config.steps).map(|i| i as f64 * dt).collect();

        // Initialize data structures
        let mut rates = vec![Vec::new(); self.config.steps + 1];
        self.probs = vec![(0.5, 0.5); self.config.steps]; // Default to equal probabilities
        self.bk_trinomial = None;

        match self.config.model {
            ShortRateModel::HoLee => self.calibrate_ho_lee(&mut rates, discount_curve, dt)?,
            ShortRateModel::BlackDermanToy => {
                let kappa = self.config.mean_reversion.unwrap_or(0.0);
                if kappa < 0.0 {
                    return Err(Error::Validation(format!(
                        "Black-Karasinski mean reversion must be non-negative, got {kappa}"
                    )));
                }
                if kappa.abs() < 1e-12 {
                    // κ = 0: standard binomial BDT calibration.
                    self.calibrate_bdt(&mut rates, discount_curve, dt)?;
                } else {
                    // κ ≠ 0: genuine trinomial Black-Karasinski lattice in
                    // x = ln r .
                    self.calibrate_bk_trinomial(&mut rates, discount_curve, dt, kappa)?;
                }
            }
        }

        self.rates = Arc::new(rates);

        Ok(())
    }

    /// Calibrate Ho-Lee model parameters.
    ///
    /// Ho-Lee does **not** support mean reversion because the rate-dependent
    /// drift `κ·r` breaks lattice recombination. Use [`HullWhiteTree`] for
    /// mean-reverting normal short-rate models.
    ///
    /// Negative short rates are a correct and expected feature of Ho-Lee and
    /// are not treated as errors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `mean_reversion` is non-zero, or if an
    /// extreme volatility drives the lattice to a pathologically extreme node
    /// discount factor (a numerically degenerate tree unfit for pricing).
    fn calibrate_ho_lee(
        &mut self,
        rates: &mut [Vec<f64>],
        discount_curve: &dyn Discounting,
        dt: f64,
    ) -> Result<()> {
        if let Some(kappa) = self.config.mean_reversion {
            if kappa.abs() > 1e-12 {
                return Err(Error::Validation(
                    "Ho-Lee model does not support mean reversion (breaks lattice recombination); \
                     use HullWhiteTree for mean-reverting normal short-rate models"
                        .into(),
                ));
            }
        }

        let sigma = self.config.volatility;
        // Calibration must use the same per-node discount convention as
        // pricing : a tree calibrated with continuous
        // `exp(-r*dt)` but priced with e.g. simple `1/(1+r*dt)` silently
        // fails to reprice the curve.
        let comp = self.config.compounding;

        // Initialize first step with current short rate: r0 satisfies
        // comp.df(r0, T1) = P(0, T1) under the configured convention.
        let r0 = if self.time_steps[1] > 0.0 {
            comp.rate_from_df(discount_curve.df(self.time_steps[1]), self.time_steps[1])
        } else {
            0.03 // Fallback rate
        };

        rates[0] = vec![r0];

        // State prices (Arrow-Debreu prices) for the current step
        let mut state_prices = vec![1.0]; // Q[0] = 1.0

        // Build tree forward
        for step in 0..self.config.steps {
            // rates[step] discounts the interval [t_step, t_{step+1}].
            // The next row rates[step + 1] discounts [t_{step+1}, t_{step+2}],
            // so it is calibrated to P(0, t_{step+2}) when that maturity
            // exists. The terminal row rates[N] is populated for lattice
            // geometry and accessor consistency; backward induction never uses
            // it for discounting because pricing stops at maturity.

            let next_next_time = if step + 2 < self.time_steps.len() {
                self.time_steps[step + 2]
            } else {
                // Terminal row: populate but do not calibrate an unused
                // post-maturity discounting interval.
                0.0
            };

            let next_nodes = step + 2;
            let mut next_rates_base = vec![0.0; next_nodes];
            let mut next_state_prices = vec![0.0; next_nodes];

            for (i, &current_rate) in rates[step].iter().enumerate() {
                let q = state_prices[i];
                let df = comp.df(current_rate, dt);

                // Up move (to i+1)
                let r_up_base = current_rate + sigma * dt.sqrt();
                if i + 1 < next_nodes {
                    next_rates_base[i + 1] = r_up_base;
                    next_state_prices[i + 1] += q * df * 0.5;
                }

                // Down move (to i)
                let r_down_base = current_rate - sigma * dt.sqrt();
                if i < next_nodes {
                    next_rates_base[i] = r_down_base;
                    next_state_prices[i] += q * df * 0.5;
                }
            }

            // 2. Solve for theta (drift adjustment to match discount curve)
            //
            // Ho-Lee calibration: r_next[j] = r_base[j] + θ. The model ZCB
            // price Σ Q_next[j] · df(r_base[j] + θ, dt) must equal P_target.
            //
            // Under continuous compounding the θ-dependence factors out:
            // df(r+θ) = exp(-θ·dt)·df(r) ⇒ θ = -ln(P_target/P_model_base)/dt,
            // which is exact. Other conventions do not factor θ out of df(r),
            // so θ is root-found with that closed form as the initial
            // guess.
            let theta = if next_next_time > 0.0 {
                let p_target = discount_curve.df(next_next_time);
                let mut p_model_base = 0.0;
                let mut p_model_base_cont = 0.0;
                for (j, &q_next) in next_state_prices.iter().enumerate() {
                    let r_base = next_rates_base[j];
                    // Discount from t_{i+2} to t_{i+1} using r_{i+1}
                    p_model_base += q_next * comp.df(r_base, dt);
                    p_model_base_cont += q_next * (-r_base * dt).exp();
                }

                if p_model_base > 0.0 && p_target > 0.0 {
                    let theta_cont = if p_model_base_cont > 0.0 {
                        -(p_target / p_model_base_cont).ln() / dt
                    } else {
                        0.0
                    };
                    if comp == TreeCompounding::Continuous {
                        theta_cont
                    } else {
                        use finstack_quant_core::math::{BrentSolver, Solver};
                        let objective = |theta: f64| -> f64 {
                            let mut p_model = 0.0;
                            for (j, &q_next) in next_state_prices.iter().enumerate() {
                                p_model += q_next * comp.df(next_rates_base[j] + theta, dt);
                            }
                            p_model - p_target
                        };
                        match BrentSolver::new().solve(objective, theta_cont) {
                            Ok(t) => t,
                            Err(e) => {
                                return Err(Error::Validation(format!(
                                    "Ho-Lee calibration: failed to solve drift theta at \
                                     step {step} under {comp:?} compounding: {e}"
                                )));
                            }
                        }
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // 3. Apply theta directly to get final rates (θ is the rate adjustment)
            let mut next_rates = vec![0.0; next_nodes];
            for j in 0..next_nodes {
                next_rates[j] = next_rates_base[j] + theta;
            }

            rates[step + 1] = next_rates;
            state_prices = next_state_prices;
        }

        // Measure actual calibration error (floating-point accumulation)
        let mut max_error_bps = 0.0_f64;
        let mut max_error_step = 0_usize;
        {
            let max_nodes = self.config.steps + 2;
            let mut q = vec![0.0_f64; max_nodes];
            let mut next_q = vec![0.0_f64; max_nodes];
            q[0] = 1.0; // Arrow-Debreu prices
            for (step, rates_step) in rates.iter().enumerate().take(self.config.steps) {
                let next_nodes = step + 2;
                next_q[..next_nodes].fill(0.0);
                for (i, &rate_i) in rates_step.iter().enumerate() {
                    let df_i = comp.df(rate_i, dt);
                    if i + 1 < next_nodes {
                        next_q[i + 1] += q[i] * df_i * 0.5;
                    }
                    if i < next_nodes {
                        next_q[i] += q[i] * df_i * 0.5;
                    }
                }
                let model_df: f64 = next_q[..next_nodes].iter().sum();
                let t_next = self.time_steps[step + 1];
                let target_df = discount_curve.df(t_next);
                if target_df > 0.0 {
                    let err = ((model_df - target_df) / target_df).abs() * 10_000.0;
                    if err > max_error_bps {
                        max_error_bps = err;
                        max_error_step = step;
                    }
                }
                std::mem::swap(&mut q, &mut next_q);
            }
        }

        // Diagnostic guard for pathologically extreme node discount factors.
        //
        // Ho-Lee legitimately admits negative short rates, so a node discount
        // factor `exp(-r*dt)` modestly above 1 is expected and is NOT flagged.
        // But an *extreme* normal volatility drives the lattice to wildly
        // dispersed node rates: the deeply-negative tail produces a per-step
        // DF that explodes far above 1, and the deeply-positive tail produces
        // one that collapses toward 0. Either is a numerical-breakdown signal
        // — the lattice is unfit for pricing. The two-sided window below spans
        // ~140 orders of magnitude, so a normal-volatility tree (whose rates
        // stay within a few percent) never trips it. We do not change the
        // model (negative rates remain valid); we only refuse to return a
        // numerically degenerate lattice silently.
        const MAX_NODE_DISCOUNT_FACTOR: f64 = 1.0e6;
        const MIN_NODE_DISCOUNT_FACTOR: f64 = 1.0e-30;
        for (step, rates_step) in rates.iter().enumerate() {
            for (node, &rate) in rates_step.iter().enumerate() {
                let node_df = comp.df(rate, dt);
                // `contains` is `false` for a `NaN` node_df, so the negation
                // correctly flags non-finite values as pathological too.
                let df_in_range =
                    (MIN_NODE_DISCOUNT_FACTOR..=MAX_NODE_DISCOUNT_FACTOR).contains(&node_df);
                if !df_in_range {
                    self.calibration_quality = Some(CalibrationResult {
                        max_error_bps,
                        max_error_step,
                        fallback_count: 0,
                        converged: false,
                    });
                    return Err(Error::Validation(format!(
                        "Ho-Lee calibration produced a pathologically extreme \
                         node discount factor {node_df:.3e} at step {step}, \
                         node {node} (short rate {rate:.4}): the lattice is \
                         numerically degenerate and unfit for pricing. Reduce \
                         the volatility, the step count, or the maturity."
                    )));
                }
            }
        }

        self.calibration_quality = Some(CalibrationResult {
            max_error_bps,
            max_error_step,
            fallback_count: 0,
            converged: true,
        });

        Ok(())
    }

    /// Calibrate the standard (κ = 0) Black-Derman-Toy model using
    /// state-price recursion on a binomial lattice with constant lognormal
    /// volatility.
    ///
    /// Mean-reverting Black-Karasinski (κ ≠ 0) is handled by
    /// [`calibrate_bk_trinomial`](Self::calibrate_bk_trinomial), which builds
    /// a genuine trinomial lattice in x = ln r — a binomial lattice cannot
    /// represent the rate-dependent drift `−κ·ln r` while staying
    /// recombining .
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a discount factor is non-positive, if
    /// the node-rate clamp `[1e-8, 5.0]` engages materially (a tree too wide
    /// to calibrate — the lattice would silently misprice the curve), or if
    /// the calibrated tree fails to reprice the curve within tolerance.
    fn calibrate_bdt(
        &mut self,
        rates: &mut [Vec<f64>],
        discount_curve: &dyn Discounting,
        dt: f64,
    ) -> Result<()> {
        use finstack_quant_core::math::{BrentSolver, Solver};

        let sigma = self.config.volatility;
        let solver = BrentSolver::new();

        // Standard BDT (κ = 0): constant lognormal volatility, per-step
        // log-spread σ√dt. κ ≠ 0 never reaches this path — calibrate()
        // routes it to the trinomial Black-Karasinski lattice.
        let step_vol = sigma * dt.sqrt();
        let u = step_vol.exp();
        let p = 0.5;

        // Bounds for alpha solver.
        // Upper bound is generous to avoid distorting the tail of the lognormal
        // distribution; individual node rates can legitimately exceed 100% in
        // wide trees (high vol, many steps, long maturity).
        let alpha_lb = 1e-8;
        let alpha_ub = 5.0;

        // Relative tolerance for deciding that the `[alpha_lb, alpha_ub]` clamp
        // has *materially* altered a node rate. A node rate that merely sits
        // near a bound is fine; one that the clamp has moved by more than this
        // fraction means the Brent objective no longer responds to `alpha` at
        // that node, so the tree can no longer reprice the curve. When that
        // happens the calibration is unsound and is failed below rather than
        // silently returning a mispriced lattice (`max_error_bps` alone only
        // *reports* the damage — it does not prevent the tree from escaping).
        let clamp_rel_tol = 1.0e-6;
        let materially_clamped = |raw: f64| -> bool {
            let clamped = raw.clamp(alpha_lb, alpha_ub);
            // Relative deviation, guarding the (here impossible) zero `raw`.
            let denom = raw.abs().max(f64::MIN_POSITIVE);
            (raw - clamped).abs() / denom > clamp_rel_tol
        };
        let mut clamp_engaged = false;
        let mut clamp_engaged_step = 0_usize;

        // Initialize first step with initial short rate
        let r0 = if self.time_steps[1] > 0.0 {
            // Use initial forward rate from discount curve
            -discount_curve.df(self.time_steps[1]).ln() / self.time_steps[1]
        } else {
            0.03 // Fallback rate
        };

        rates[0] = vec![r0.clamp(alpha_lb, alpha_ub)]; // Ensure within bounds
        let mut state_prices = vec![vec![1.0]]; // Q[0] = [1.0]

        // Set transition probabilities (constant for BDT)
        for i in 0..self.config.steps {
            self.probs[i] = (p, 1.0 - p);
        }

        // Track calibration quality for diagnostics
        let mut max_error_bps = 0.0_f64;
        let mut max_error_step = 0_usize;
        let mut fallback_count = 0_usize;

        // Build tree forward, calibrating drift at each step
        for step in 0..self.config.steps {
            let current_time = self.time_steps[step + 1];
            let target_df = discount_curve.df(current_time);

            if target_df <= 0.0 {
                return Err(Error::Validation(format!(
                    "BDT calibration: non-positive discount factor {} at time {}",
                    target_df, current_time
                )));
            }

            let num_nodes = step + 1;
            let current_state_prices = &state_prices[step];
            let current_rates = &rates[step];

            // Solve for drift parameter alpha such that model ZCB price matches market
            let comp = self.config.compounding;
            let objective = |alpha: f64| -> f64 {
                let mut model_price = 0.0;

                for (j, &state_price) in current_state_prices.iter().enumerate().take(num_nodes) {
                    let rate = alpha * u.powf(num_nodes as f64 - 1.0 - 2.0 * j as f64);
                    let rate_clamped = rate.clamp(alpha_lb, alpha_ub);
                    model_price += state_price * comp.df(rate_clamped, dt);
                }

                model_price - target_df
            };

            // Initial guess for alpha based on previous step or forward rate
            let initial_alpha = if step == 0 {
                r0.clamp(alpha_lb, alpha_ub)
            } else {
                // Use geometric mean of previous step rates as initial guess
                let mean_rate =
                    current_rates.iter().map(|&r| r.ln()).sum::<f64>() / current_rates.len() as f64;
                mean_rate.exp().clamp(alpha_lb, alpha_ub)
            };

            // Solve for alpha with convergence tracking
            let (alpha, used_fallback) = match solver.solve(objective, initial_alpha) {
                Ok(a) => (a.clamp(alpha_lb, alpha_ub), false),
                Err(_) => {
                    // Solver failed - use fallback based on market rate
                    let market_rate = if current_time > 0.0 {
                        -target_df.ln() / current_time
                    } else {
                        0.03
                    };
                    fallback_count += 1;
                    (market_rate.clamp(alpha_lb, alpha_ub), true)
                }
            };

            let current_step_rates: Vec<f64> = (0..num_nodes)
                .map(|j| {
                    let rate = alpha * u.powf(num_nodes as f64 - 1.0 - 2.0 * j as f64);
                    if materially_clamped(rate) && !clamp_engaged {
                        clamp_engaged = true;
                        clamp_engaged_step = step;
                    }
                    rate.clamp(alpha_lb, alpha_ub)
                })
                .collect();
            rates[step] = current_step_rates.clone();

            let model_df = {
                let mut model_price = 0.0;
                for (j, &state_price) in current_state_prices.iter().enumerate().take(num_nodes) {
                    model_price += state_price * comp.df(current_step_rates[j], dt);
                }
                model_price
            };
            let error_bps = ((model_df - target_df) / target_df).abs() * 10000.0;

            if error_bps > max_error_bps {
                max_error_bps = error_bps;
                max_error_step = step;
            }

            // Log warning if calibration error is significant (>1bp) or fallback was used
            if error_bps > 1.0 || used_fallback {
                tracing::warn!(
                    "BDT calibration step {}: error={:.2}bp, target_df={:.6}, model_df={:.6}{}",
                    step,
                    error_bps,
                    target_df,
                    model_df,
                    if used_fallback {
                        " (FALLBACK USED)"
                    } else {
                        ""
                    }
                );
            }

            // Build next step rates using calibrated alpha.
            //
            // Terminal row note (same convention as Ho-Lee and BK): the final
            // iteration populates rates[N] for lattice geometry and accessor
            // consistency, but that row's alpha is the one solved for the last
            // pre-maturity interval — there is no interval beyond maturity to
            // drift-calibrate, and backward induction never uses rates[N] for
            // discounting because pricing stops at maturity.
            let next_nodes = num_nodes + 1;
            let mut next_rates = vec![0.0; next_nodes];
            let mut next_state_prices = vec![0.0; next_nodes];

            for (j, &state_price) in current_state_prices.iter().enumerate().take(num_nodes) {
                let discount_factor = comp.df(current_step_rates[j], dt);
                let state_price_contribution = state_price * discount_factor;

                // Up move: j -> j+1
                if j + 1 < next_nodes {
                    let up_rate = alpha * u.powf(next_nodes as f64 - 1.0 - 2.0 * (j + 1) as f64);
                    if materially_clamped(up_rate) && !clamp_engaged {
                        clamp_engaged = true;
                        clamp_engaged_step = step + 1;
                    }
                    next_rates[j + 1] = up_rate.clamp(alpha_lb, alpha_ub);
                    next_state_prices[j + 1] += state_price_contribution * p;
                }

                // Down move: j -> j
                if j < next_nodes {
                    let down_rate = alpha * u.powf(next_nodes as f64 - 1.0 - 2.0 * j as f64);
                    if materially_clamped(down_rate) && !clamp_engaged {
                        clamp_engaged = true;
                        clamp_engaged_step = step + 1;
                    }
                    next_rates[j] = down_rate.clamp(alpha_lb, alpha_ub);
                    next_state_prices[j] += state_price_contribution * (1.0 - p);
                }
            }

            rates[step + 1] = next_rates;
            state_prices.push(next_state_prices);
        }

        // Log calibration summary
        if max_error_bps > 1.0 || fallback_count > 0 {
            tracing::warn!(
                "BDT calibration completed: max error={:.2}bp at step {}, fallbacks={} (target: <1bp, 0 fallbacks)",
                max_error_bps,
                max_error_step,
                fallback_count
            );
        } else {
            tracing::debug!(
                "BDT calibration completed: max error={:.4}bp at step {}",
                max_error_bps,
                max_error_step
            );
        }

        // Hard repricing tolerance. A well-posed BDT tree calibrates to far
        // below 1 bp (floating-point accumulation only); the codebase's own
        // `CalibrationResult::is_acceptable` bar is 1 bp. This *hard error*
        // gate is set well above that — at 25 bp — so it never rejects a
        // merely-imperfect tree, only one that has genuinely *stopped*
        // repricing the curve. Empirically the BDT clamp failure is bimodal:
        // a wide tree either reprices fine (clamp engages only on vanishing-
        // weight tail nodes) or breaks catastrophically (thousands of bp), so
        // 25 bp cleanly separates the two. Unlike the diagnostic
        // `max_error_bps` field — which only *reports* — this gate *enforces*
        // the contract so a silently-mispriced tree can never be returned as
        // `converged`. The milder 1-25 bp band is still surfaced via the
        // `tracing::warn!` above and the `is_acceptable` / `is_good` flags.
        const MAX_CALIBRATION_ERROR_BPS: f64 = 25.0;

        // Enforce that the calibrated tree actually reprices the curve.
        //
        // The node-rate clamp `[1e-8, 5.0]` is applied inside the Brent
        // objective. When it engages on a node with material Arrow-Debreu
        // weight the objective stops responding to `alpha`, the solver settles
        // on the wrong drift, and the lattice silently stops repricing the
        // curve — exactly the failure this gate catches. (Clamp engagement on
        // a deep, vanishing-weight tail node is harmless: it leaves
        // `max_error_bps` at ~0 and is intentionally *not* failed here.)
        //
        // `max_error_bps` is re-derived above by an independent forward pass
        // over the final `rates`, so it faithfully reflects any clamp-induced
        // mispricing. When the tolerance is breached, the diagnostic message
        // reports whether the clamp engaged (the usual root cause for a wide
        // tree) so the caller knows which knob to turn.
        if !max_error_bps.is_finite() || max_error_bps > MAX_CALIBRATION_ERROR_BPS {
            self.calibration_quality = Some(CalibrationResult {
                max_error_bps,
                max_error_step,
                fallback_count,
                converged: false,
            });
            let clamp_note = if clamp_engaged {
                format!(
                    " The node-rate clamp [{alpha_lb:.0e}, {alpha_ub}] engaged \
                     materially (first at step {clamp_engaged_step}) — the tree \
                     is too wide; lower the volatility, the step count, or the \
                     maturity."
                )
            } else {
                String::new()
            };
            return Err(Error::Validation(format!(
                "BDT calibration failed to reprice the discount curve: max \
                 error {max_error_bps:.2} bp at step {max_error_step} exceeds \
                 the {MAX_CALIBRATION_ERROR_BPS:.1} bp tolerance.{clamp_note}"
            )));
        }

        // Store calibration result for user inspection
        self.calibration_quality = Some(CalibrationResult {
            max_error_bps,
            max_error_step,
            fallback_count,
            converged: true,
        });

        Ok(())
    }

    /// Calibrate a mean-reverting Black-Karasinski model on a trinomial
    /// lattice in x = ln r .
    ///
    /// # Model
    ///
    /// ```text
    /// d(ln r) = [θ(t) − κ·ln r] dt + σ dW
    /// ```
    ///
    /// Writing `x = ln r − a(t)`, the residual `dx = −κx dt + σ dW` is the
    /// same mean-reverting OU process the Hull-White trinomial discretizes,
    /// so the lattice reuses that geometry: spacing `dx = σ√(3Δt)`, width cap
    /// `j_max` with Hull & White (1994) branch switching at the edges, and
    /// per-node probabilities matching the conditional mean `−jκΔt·dx` and
    /// variance `σ²Δt`. The per-step shift `a_i` is calibrated by forward
    /// induction on Arrow-Debreu prices with a Brent solve (the rate enters
    /// the discount factor as `exp(a_i + x_j)`, so no closed form exists).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a target discount factor is
    /// non-positive, a drift solve fails, or the calibrated lattice fails to
    /// reprice the curve within tolerance.
    fn calibrate_bk_trinomial(
        &mut self,
        rates: &mut [Vec<f64>],
        discount_curve: &dyn Discounting,
        dt: f64,
        kappa: f64,
    ) -> Result<()> {
        use finstack_quant_core::math::{BrentSolver, Solver};

        let sigma = self.config.volatility;
        let comp = self.config.compounding;
        let steps = self.config.steps;

        // Trinomial spacing in x = ln r: matches per-step variance σ²Δt.
        let dx = sigma * (3.0 * dt).sqrt();
        // Hull-White width cap keeping branch probabilities positive.
        let j_max = ((0.184 / (kappa * dt)).ceil() as usize).max(1);

        let mut alpha = vec![0.0; steps + 1];
        let mut probs: Vec<Vec<(f64, f64, f64)>> = Vec::with_capacity(steps);
        let mut state_prices: Vec<f64> = vec![1.0];

        let mut max_error_bps = 0.0_f64;
        let mut max_error_step = 0_usize;

        for step in 0..steps {
            let curr_j_max = step.min(j_max);
            let next_j_max = (step + 1).min(j_max);
            let num_nodes = 2 * curr_j_max + 1;

            let mut step_probs = Vec::with_capacity(num_nodes);
            for j in 0..num_nodes {
                let j_signed = j as i32 - curr_j_max as i32;
                step_probs.push(HullWhiteTree::compute_probabilities(
                    kappa, dt, dx, j_signed, j_max,
                )?);
            }

            let t_next = self.time_steps[step + 1];
            let target_df = discount_curve.df(t_next);
            if target_df <= 0.0 {
                return Err(Error::Validation(format!(
                    "Black-Karasinski calibration: non-positive discount factor \
                     {target_df} at time {t_next}"
                )));
            }

            // Solve the additive x-shift a so the lattice reprices P(0, t_next):
            //   Σ_j Q_j · df(exp(a + x_j), Δt) = target_df
            let q = &state_prices;
            let objective = |a: f64| -> f64 {
                let mut model_df = 0.0;
                for (j, &qj) in q.iter().enumerate() {
                    let x_j = (j as i32 - curr_j_max as i32) as f64 * dx;
                    model_df += qj * comp.df((a + x_j).exp(), dt);
                }
                model_df - target_df
            };
            // Initial guess: log of the period forward rate.
            let prev_df = discount_curve.df(self.time_steps[step]);
            let fwd = if prev_df > 0.0 && target_df > 0.0 {
                comp.rate_from_df(target_df / prev_df, dt)
            } else {
                0.03
            };
            let guess = fwd.max(1e-8).ln();
            let a = BrentSolver::new().solve(objective, guess).map_err(|e| {
                Error::Validation(format!(
                    "Black-Karasinski calibration: drift solve failed at step {step}: {e}"
                ))
            })?;
            alpha[step] = a;

            rates[step] = (0..num_nodes)
                .map(|j| {
                    let x_j = (j as i32 - curr_j_max as i32) as f64 * dx;
                    (a + x_j).exp()
                })
                .collect();

            // Forward-induce Arrow-Debreu prices to the next step.
            let mut next_q = vec![0.0; 2 * next_j_max + 1];
            // Branch switching only applies once the lattice has reached its
            // cap (curr and next widths equal); while still growing, all
            // nodes branch normally.
            let boundary_j_max = if curr_j_max == next_j_max {
                curr_j_max
            } else {
                usize::MAX
            };
            for (j, &qj) in q.iter().enumerate() {
                let j_signed = j as i32 - curr_j_max as i32;
                let r_j = (a + j_signed as f64 * dx).exp();
                let contribution = qj * comp.df(r_j, dt);
                for (offset, probability) in
                    HullWhiteTree::transition_offsets(j_signed, boundary_j_max, step_probs[j])
                {
                    if let Some(idx) = HullWhiteTree::transition_index(j_signed, offset, next_j_max)
                    {
                        if idx < next_q.len() {
                            next_q[idx] += contribution * probability;
                        }
                    }
                }
            }

            let model_df: f64 = next_q.iter().sum();
            let error_bps = ((model_df - target_df) / target_df).abs() * 10_000.0;
            if error_bps > max_error_bps {
                max_error_bps = error_bps;
                max_error_step = step;
            }

            probs.push(step_probs);
            state_prices = next_q;
        }

        // Terminal row: no interval beyond maturity to calibrate; extend the
        // last drift for accessor consistency (never used for discounting).
        if steps > 0 {
            alpha[steps] = alpha[steps - 1];
        }
        let term_j_max = steps.min(j_max);
        rates[steps] = (0..=(2 * term_j_max))
            .map(|j| {
                let x_j = (j as i32 - term_j_max as i32) as f64 * dx;
                (alpha[steps] + x_j).exp()
            })
            .collect();

        // Same hard repricing gate philosophy as BDT: a well-posed lattice
        // calibrates to float noise; anything materially off must not escape.
        const MAX_CALIBRATION_ERROR_BPS: f64 = 25.0;
        let converged = max_error_bps.is_finite() && max_error_bps <= MAX_CALIBRATION_ERROR_BPS;
        self.calibration_quality = Some(CalibrationResult {
            max_error_bps,
            max_error_step,
            fallback_count: 0,
            converged,
        });
        if !converged {
            return Err(Error::Validation(format!(
                "Black-Karasinski calibration failed to reprice the discount \
                 curve: max error {max_error_bps:.2} bp at step {max_error_step} \
                 exceeds the {MAX_CALIBRATION_ERROR_BPS:.1} bp tolerance"
            )));
        }

        self.bk_trinomial = Some(BkTrinomialLattice { j_max, probs });

        Ok(())
    }

    /// Get the short rate at a specific node.
    ///
    /// # Node Ordering
    ///
    /// The ordering convention differs by model:
    ///
    /// | Model | Node 0 | Node N |
    /// |-------|--------|--------|
    /// | Ho-Lee | **lowest** rate | **highest** rate |
    /// | BDT (κ = 0, binomial) | **highest** rate (`α·u^(n-1)`) | **lowest** rate (`α·u^(-(n-1))`) |
    /// | BK (κ ≠ 0, trinomial) | **lowest** rate (j = −j_max) | **highest** rate (j = +j_max) |
    pub fn rate_at_node(&self, step: usize, node: usize) -> Result<f64> {
        if step >= self.rates.len() || node >= self.rates[step].len() {
            return Err(Error::internal(format!(
                "short-rate tree node out of bounds: step={step}, node={node}"
            )));
        }
        Ok(self.rates[step][node])
    }

    /// Get transition probabilities at a step
    pub fn probabilities(&self, step: usize) -> Result<(f64, f64)> {
        if step >= self.probs.len() {
            return Err(Error::internal(format!(
                "short-rate tree probability row out of bounds: step={step}"
            )));
        }
        Ok(self.probs[step])
    }

    /// Get time at step
    pub fn time_at_step(&self, step: usize) -> Result<f64> {
        if step >= self.time_steps.len() {
            return Err(Error::internal(format!(
                "short-rate tree time step out of bounds: step={step}"
            )));
        }
        Ok(self.time_steps[step])
    }

    fn expected_nodes_at_step(branching: TreeBranching, step: usize) -> usize {
        match branching {
            TreeBranching::Binomial => step + 1,
            TreeBranching::Trinomial => 2 * step + 1,
        }
    }

    fn validate_lattice_geometry(&self) -> Result<()> {
        if self.rates.len() != self.config.steps + 1 {
            return Err(Error::internal(format!(
                "short-rate tree lattice geometry mismatch: expected {} rate rows, got {}",
                self.config.steps + 1,
                self.rates.len()
            )));
        }

        // Black-Karasinski trinomial lattice: width grows 2·step+1 until the
        // j_max cap, then stays at 2·j_max+1.
        if let Some(lattice) = &self.bk_trinomial {
            for (step, rates_at_step) in self.rates.iter().enumerate() {
                let expected = 2 * step.min(lattice.j_max) + 1;
                if rates_at_step.len() != expected {
                    return Err(Error::internal(format!(
                        "Black-Karasinski lattice geometry mismatch: step {} expected {} \
                         nodes, got {}",
                        step,
                        expected,
                        rates_at_step.len()
                    )));
                }
            }
            return Ok(());
        }

        for (step, rates_at_step) in self.rates.iter().enumerate() {
            let expected = Self::expected_nodes_at_step(self.config.branching, step);
            if rates_at_step.len() != expected {
                return Err(Error::internal(format!(
                    "short-rate tree lattice geometry mismatch for {:?}: step {} expected {} nodes, got {}",
                    self.config.branching,
                    step,
                    expected,
                    rates_at_step.len()
                )));
            }
        }

        Ok(())
    }

    /// Backward induction over the Black-Karasinski trinomial lattice.
    ///
    /// Honors the per-node transition probabilities, the Hull & White edge
    /// branch switching, and the configured per-node compounding. The OAS is
    /// applied as a parallel shift (in bp) to the node short rate before
    /// discounting, matching the recombining-engine convention.
    fn price_bk_trinomial<V: TreeValuator>(
        &self,
        lattice: &BkTrinomialLattice,
        initial_vars: &HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
        oas: f64,
    ) -> Result<f64> {
        let steps = self.config.steps;
        let dt = time_to_maturity / steps as f64;
        let comp = self.config.compounding;
        let oas_shift = oas / 10_000.0;
        let j_max = lattice.j_max;

        let cached_hazard = initial_vars.get(state_keys::HAZARD_RATE).copied();
        let cached_spot = initial_vars.get(state_keys::SPOT).copied();
        let cached_df = initial_vars.get(state_keys::DF).copied();
        let cached_for = |rate: f64| -> CachedValues {
            CachedValues {
                spot: cached_spot,
                interest_rate: Some(rate),
                hazard_rate: cached_hazard,
                df: cached_df,
            }
        };

        // Terminal payoffs.
        let mut values: Vec<f64> = Vec::with_capacity(self.rates[steps].len());
        for &r in self.rates[steps].iter() {
            let state = NodeState::with_cached(
                steps,
                time_to_maturity,
                initial_vars,
                market_context,
                cached_for(r + oas_shift),
            );
            values.push(valuator.value_at_maturity(&state)?);
        }

        // Backward induction with per-node probabilities.
        let mut scratch: Vec<f64> = Vec::new();
        for step in (0..steps).rev() {
            let curr_j_max = step.min(j_max);
            let next_j_max = (step + 1).min(j_max);
            let num_nodes = 2 * curr_j_max + 1;
            let boundary_j_max = if curr_j_max == next_j_max {
                curr_j_max
            } else {
                usize::MAX
            };
            let time_t = step as f64 * dt;

            scratch.clear();
            for j in 0..num_nodes {
                let j_signed = j as i32 - curr_j_max as i32;
                let node_probs = lattice.probs[step][j];

                let mut expected_value = 0.0;
                for (offset, probability) in
                    HullWhiteTree::transition_offsets(j_signed, boundary_j_max, node_probs)
                {
                    if let Some(idx) = HullWhiteTree::transition_index(j_signed, offset, next_j_max)
                    {
                        if idx < values.len() {
                            expected_value += probability * values[idx];
                        }
                    }
                }

                let r = self.rates[step][j] + oas_shift;
                let continuation = expected_value * comp.df(r, dt);
                let state = NodeState::with_cached(
                    step,
                    time_t,
                    initial_vars,
                    market_context,
                    cached_for(r),
                );
                scratch.push(valuator.value_at_node(&state, continuation, dt)?);
            }
            std::mem::swap(&mut values, &mut scratch);
        }

        values.first().copied().ok_or_else(|| {
            Error::internal("Black-Karasinski backward induction produced no root value")
        })
    }
}

impl TreeModel for ShortRateTree {
    fn price<V: TreeValuator>(
        &self,
        mut initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        if self.rates.is_empty() {
            tracing::debug!("ShortRateTree::price called before calibration (rates is empty)");
            return Err(Error::internal(
                "short-rate tree must be calibrated before pricing",
            ));
        }
        self.validate_lattice_geometry()?;

        // Ensure initial rate is present
        if !initial_vars.contains_key(state_keys::INTEREST_RATE) {
            if let Some(row) = self.rates.first() {
                if let Some(&r0) = row.first() {
                    initial_vars.insert(state_keys::INTEREST_RATE, r0);
                }
            }
        }

        // Get OAS from initial variables (default to 0)
        let oas = initial_vars.get("oas").copied().unwrap_or(0.0);

        // Black-Karasinski trinomial lattice: per-node probabilities and
        // capped width with branch switching cannot be expressed through the
        // constant-probability recombining engine, so it has a dedicated
        // backward induction.
        if let Some(lattice) = &self.bk_trinomial {
            return self.price_bk_trinomial(
                lattice,
                &initial_vars,
                time_to_maturity,
                market_context,
                valuator,
                oas,
            );
        }

        // Create custom state generator that uses pre-calibrated rates
        // Clone rates (cheap Arc clone) to avoid lifetime issues with closures
        let rates_clone = std::sync::Arc::clone(&self.rates);
        let state_gen: Box<dyn Fn(usize, usize) -> f64> =
            Box::new(move |step: usize, node: usize| -> f64 {
                if step < rates_clone.len() && node < rates_clone[step].len() {
                    rates_clone[step][node]
                } else {
                    0.0 // Fallback
                }
            });

        let rates_clone2 = std::sync::Arc::clone(&self.rates);
        let compounding = self.config.compounding;
        let dt_pricing = time_to_maturity / self.config.steps as f64;
        let rate_gen: Box<dyn Fn(usize, usize) -> f64> =
            Box::new(move |step: usize, node: usize| -> f64 {
                let r = if step < rates_clone2.len() && node < rates_clone2[step].len() {
                    rates_clone2[step][node] + oas / 10000.0
                } else {
                    return 0.0;
                };
                compounding.to_continuous(r, dt_pricing)
            });

        // Set up branching probabilities based on tree type
        let (p_up, p_down, p_middle) = match self.config.branching {
            TreeBranching::Trinomial => {
                // Trinomial: equal probabilities for up/mid/down
                // This provides better numerical stability for mean-reverting models
                (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)
            }
            TreeBranching::Binomial => {
                // Binomial: use calibrated probabilities if available, else 50/50
                let (pu, pd) = self.probs.first().copied().unwrap_or((0.5, 0.5));
                (pu, pd, 0.0)
            }
        };

        price_recombining_tree(RecombiningInputs {
            branching: self.config.branching,
            steps: self.config.steps,
            initial_vars,
            time_to_maturity,
            market_context,
            valuator,
            up_factor: 1.0,   // Not used with custom_state_generator
            down_factor: 1.0, // Not used with custom_state_generator
            middle_factor: if self.config.branching == TreeBranching::Trinomial {
                Some(1.0)
            } else {
                None
            },
            prob_up: p_up,
            prob_down: p_down,
            prob_middle: Some(p_middle),
            interest_rate: 0.0, // Not used with custom_rate_generator
            barrier: None,
            custom_state_generator: Some(&*state_gen),
            custom_rate_generator: Some(&*rate_gen),
        })
    }

    fn calculate_greeks<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
        bump_size: Option<f64>,
    ) -> Result<TreeGreeks> {
        let base_price = self.price(
            initial_vars.clone(),
            time_to_maturity,
            market_context,
            valuator,
        )?;

        let mut greeks = TreeGreeks {
            price: base_price,
            delta: 0.0,
            gamma: 0.0,
            vega: 0.0,
            theta: 0.0,
            rho: 0.0,
        };

        // Default: relative 10% of the calibrated vol, floored at 1 bp. A
        // fixed absolute 0.01 bump was a 100% relative bump for a typical
        // normal σ = 1% short-rate vol, which badly distorts the FD vega.
        // Vega is still reported per 1% (absolute) vol move below.
        let vol_bump = bump_size.unwrap_or((0.1 * self.config.volatility).max(1e-4));
        let curve_id = &self.calibration_curve_id;

        // Vega and theta require recalibrating fresh trees against the discount
        // curve.  The curve is looked up from MarketContext using the CurveId
        // stored during calibrate().
        if let Ok(discount_curve) = market_context.get_discount(curve_id) {
            // --- Vega (central difference with correct denominator) -----------
            let vol_up = self.config.volatility + vol_bump;
            let vol_down = (self.config.volatility - vol_bump).max(1e-6);

            let mut config_up = self.config.clone();
            config_up.volatility = vol_up;
            let mut tree_up = ShortRateTree::new(config_up);
            if tree_up
                .calibrate(curve_id, discount_curve.as_ref(), time_to_maturity)
                .is_ok()
            {
                let price_up = tree_up.price(
                    initial_vars.clone(),
                    time_to_maturity,
                    market_context,
                    valuator,
                )?;

                let mut config_down = self.config.clone();
                config_down.volatility = vol_down;
                let mut tree_down = ShortRateTree::new(config_down);
                if tree_down
                    .calibrate(curve_id, discount_curve.as_ref(), time_to_maturity)
                    .is_ok()
                {
                    let price_down = tree_down.price(
                        initial_vars.clone(),
                        time_to_maturity,
                        market_context,
                        valuator,
                    )?;

                    let actual_span = vol_up - vol_down;
                    greeks.vega = (price_up - price_down) / actual_span * 0.01;
                } else {
                    greeks.vega = (price_up - base_price) / vol_bump * 0.01;
                }
            }

            // --- Theta (recalibrate a fresh tree for bumped maturity) ---------
            let dt_theta = 1.0 / 365.25;
            let ttm_tomorrow = time_to_maturity - dt_theta;
            if ttm_tomorrow > 0.0 {
                let mut tree_tomorrow = ShortRateTree::new(self.config.clone());
                if tree_tomorrow
                    .calibrate(curve_id, discount_curve.as_ref(), ttm_tomorrow)
                    .is_ok()
                {
                    let price_tomorrow = tree_tomorrow.price(
                        initial_vars.clone(),
                        ttm_tomorrow,
                        market_context,
                        valuator,
                    )?;
                    greeks.theta = -(base_price - price_tomorrow) / dt_theta;
                }
            }
        } else {
            tracing::debug!(
                "ShortRateTree::calculate_greeks: discount curve '{}' not found; \
                 vega and theta set to 0",
                curve_id.as_str()
            );
        }

        // Rho: OAS sensitivity (price change per 1 bp parallel spread bump).
        // Note: this measures sensitivity to the option-adjusted spread, not to
        // a parallel shift of the underlying yield curve. For bonds with embedded
        // options the two are not equivalent because an OAS bump does not change
        // the exercise boundary while a curve bump does.
        let mut bumped_vars = initial_vars;
        let base_oas = bumped_vars.get("oas").copied().unwrap_or(0.0);
        bumped_vars.insert("oas", base_oas + 1.0);

        let bumped_price = self.price(bumped_vars, time_to_maturity, market_context, valuator)?;
        greeks.rho = bumped_price - base_price;

        Ok(greeks)
    }
}

/// State variable keys specific to short-rate trees
pub mod short_rate_keys {
    /// Short rate at the current node
    pub const SHORT_RATE: &str = "interest_rate";
    /// Option-Adjusted Spread added to the short rate
    pub const OAS: &str = "oas";
    /// Current tree step
    pub const STEP: &str = "step";
    /// Current node index
    pub const NODE: &str = "node";
    /// Time from valuation date
    pub const TIME: &str = "time";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::trees::tree_framework::NodeState;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::math::volatility::{convert_atm_volatility, VolatilityConvention};
    use time::Month;

    const TEST_CURVE_ID: &str = "USD-OIS";

    fn test_curve_id() -> CurveId {
        CurveId::new(TEST_CURVE_ID)
    }

    fn create_test_curve() -> DiscountCurve {
        DiscountCurve::builder(TEST_CURVE_ID)
            .base_date(
                finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
                    .expect("should succeed"),
            )
            .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94), (5.0, 0.85)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("should succeed")
    }

    fn create_flat_curve(rate: f64) -> DiscountCurve {
        let knots = [0.0, 0.25, 0.5, 1.0, 2.0, 5.0]
            .into_iter()
            .map(|t| (t, (-rate * t).exp()));
        DiscountCurve::builder(TEST_CURVE_ID)
            .base_date(
                finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
                    .expect("should succeed"),
            )
            .knots(knots)
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("should succeed")
    }

    struct ConstantValuator;

    impl TreeValuator for ConstantValuator {
        fn value_at_maturity(&self, _state: &NodeState) -> Result<f64> {
            Ok(1.0)
        }

        fn value_at_node(
            &self,
            _state: &NodeState,
            continuation_value: f64,
            _dt: f64,
        ) -> Result<f64> {
            Ok(continuation_value)
        }
    }

    struct RateCallValuator {
        strike: f64,
    }

    impl TreeValuator for RateCallValuator {
        fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
            let rate = state
                .interest_rate()
                .ok_or_else(|| Error::internal("rate-call node missing interest rate"))?;
            Ok((rate - self.strike).max(0.0))
        }

        fn value_at_node(
            &self,
            _state: &NodeState,
            continuation_value: f64,
            _dt: f64,
        ) -> Result<f64> {
            Ok(continuation_value)
        }
    }

    #[test]
    fn test_ho_lee_tree_creation() {
        let tree = ShortRateTree::ho_lee(50, 0.01);
        assert_eq!(tree.config.steps, 50);
        assert_eq!(tree.config.model, ShortRateModel::HoLee);
        assert_eq!(tree.config.volatility, 0.01);
    }

    #[test]
    fn test_tree_calibration() {
        let mut tree = ShortRateTree::ho_lee(10, 0.015);
        let curve = create_test_curve();

        let result = tree.calibrate(&test_curve_id(), &curve, 2.0);
        assert!(result.is_ok());

        // Tree should have rates at each step
        assert_eq!(tree.rates.len(), 11); // 0 to 10 steps
        assert_eq!(tree.rates[0].len(), 1); // First step has one node
        assert_eq!(tree.rates[10].len(), 11); // Last step has 11 nodes
    }

    #[test]
    fn ho_lee_stored_lattice_prices_zero_coupon_to_calibration_curve() {
        let steps = 12;
        let maturity = 2.0;
        let mut tree = ShortRateTree::ho_lee(steps, 0.015);
        let curve = create_test_curve();
        tree.calibrate(&test_curve_id(), &curve, maturity)
            .expect("Ho-Lee calibration");

        let market = MarketContext::new();
        let actual = tree
            .price(
                HashMap::<&'static str, f64>::default(),
                maturity,
                &market,
                &ConstantValuator,
            )
            .expect("Ho-Lee zero-coupon price");
        let expected = curve.df(maturity);

        assert!(
            (actual - expected).abs() < 1e-8,
            "Ho-Lee stored lattice should price a zero coupon to the calibration curve: actual={actual}, expected={expected}"
        );
    }

    /// calibration must honor `config.compounding`. A
    /// Ho-Lee tree configured with non-continuous compounding must reprice
    /// the calibration curve to <0.1 bp, because `price()` discounts with the
    /// same convention.
    #[test]
    fn ho_lee_noncontinuous_compounding_reprices_curve() {
        for compounding in [
            TreeCompounding::Simple,
            TreeCompounding::SemiAnnual,
            TreeCompounding::Quarterly,
            TreeCompounding::Monthly,
        ] {
            let steps = 24;
            let maturity = 2.0;
            let config = ShortRateTreeConfig::ho_lee(steps, 0.012).with_compounding(compounding);
            let mut tree = ShortRateTree::new(config);
            let curve = create_test_curve();
            tree.calibrate(&test_curve_id(), &curve, maturity)
                .expect("Ho-Lee calibration under non-continuous compounding");

            let quality = tree.calibration_result().expect("quality");
            assert!(
                quality.converged && quality.max_error_bps < 0.1,
                "{compounding:?}: calibration must reprice the curve to <0.1bp, \
                 got {quality:?}"
            );

            let market = MarketContext::new();
            let actual = tree
                .price(
                    HashMap::<&'static str, f64>::default(),
                    maturity,
                    &market,
                    &ConstantValuator,
                )
                .expect("zero-coupon price");
            let expected = curve.df(maturity);
            assert!(
                ((actual - expected) / expected).abs() * 10_000.0 < 0.1,
                "{compounding:?}: zero coupon must reprice to <0.1bp: \
                 actual={actual}, expected={expected}"
            );
        }
    }

    /// `rate_from_df` inverts `df` for every convention.
    #[test]
    fn tree_compounding_rate_from_df_inverts_df() {
        for compounding in [
            TreeCompounding::Continuous,
            TreeCompounding::Simple,
            TreeCompounding::SemiAnnual,
            TreeCompounding::Quarterly,
            TreeCompounding::Monthly,
        ] {
            for rate in [-0.01, 0.0, 0.025, 0.10] {
                let dt = 0.25;
                let df = compounding.df(rate, dt);
                let recovered = compounding.rate_from_df(df, dt);
                assert!(
                    (recovered - rate).abs() < 1e-12,
                    "{compounding:?}: rate_from_df(df({rate})) = {recovered}"
                );
            }
        }
    }

    #[test]
    fn ho_lee_calibration_flags_pathologically_extreme_node_discount_factors() {
        // P0/item-8: Ho-Lee correctly admits negative rates, but with an
        // extreme normal volatility the lattice produces wildly negative node
        // rates whose per-step discount factor `exp(-r*dt)` explodes far above
        // 1. That is a numerical-breakdown signal: the calibration must emit a
        // diagnostic error rather than silently returning an unusable tree.
        //
        // sigma = 8.0 (800%/yr normal vol), 60 steps, T = 30 => dt = 0.5,
        // sigma*sqrt(dt) ~ 5.66 per step; the lowest node after 60 steps sits
        // near -300, so its node DF is exp(150) — astronomically extreme.
        let curve = create_flat_curve(0.03);
        let mut tree = ShortRateTree::ho_lee(60, 8.0);

        let result = tree.calibrate(&test_curve_id(), &curve, 30.0);
        assert!(
            result.is_err(),
            "Ho-Lee calibration that yields pathologically extreme node \
             discount factors must report a diagnostic error"
        );
        let msg = result.expect_err("must error").to_string().to_lowercase();
        assert!(
            msg.contains("discount") || msg.contains("rate") || msg.contains("extreme"),
            "error should explain the extreme-node diagnostic, got: {msg}"
        );
    }

    #[test]
    fn ho_lee_calibration_succeeds_for_a_normal_volatility_tree() {
        // The extreme-node guard must not reject ordinary trees: a normal
        // volatility (1%) Ho-Lee tree must still calibrate cleanly even with
        // many steps and a long horizon.
        let curve = create_test_curve();
        let mut tree = ShortRateTree::ho_lee(60, 0.01);
        tree.calibrate(&test_curve_id(), &curve, 5.0)
            .expect("a normal-volatility Ho-Lee tree must calibrate");
        let quality = tree.calibration_result().expect("quality");
        assert!(quality.converged, "quality={quality:?}");
    }

    #[test]
    fn test_rate_access() {
        let mut tree = ShortRateTree::ho_lee(5, 0.01);
        let curve = create_test_curve();
        tree.calibrate(&test_curve_id(), &curve, 1.0)
            .expect("should succeed");

        // Should be able to access rates at valid nodes
        let r0 = tree.rate_at_node(0, 0).expect("should succeed");
        assert!(r0 > 0.0);

        let r_final = tree.rate_at_node(5, 2).expect("should succeed");
        assert!(r_final.is_finite());

        // Invalid access should error
        assert!(tree.rate_at_node(10, 0).is_err());
        assert!(tree.rate_at_node(0, 5).is_err());
    }

    #[test]
    fn test_bdt_tree_creation() {
        // BDT with realistic 20% lognormal volatility
        let tree = ShortRateTree::black_derman_toy(25, 0.20, 0.03);
        assert_eq!(tree.config.model, ShortRateModel::BlackDermanToy);
        assert_eq!(tree.config.volatility, 0.20);
        assert_eq!(tree.config.mean_reversion, Some(0.03));
    }

    #[test]
    fn test_bdt_calibration_populates_quality_metrics() {
        let mut tree = ShortRateTree::black_derman_toy(6, 0.20, 0.0);
        let curve = create_test_curve();

        tree.calibrate(&test_curve_id(), &curve, 2.0)
            .expect("should succeed");

        assert_eq!(tree.rates.len(), 7);
        assert_eq!(tree.probs.len(), 6);
        assert!(tree.probabilities(0).expect("probabilities").0.is_finite());
        let quality = tree.calibration_result().expect("calibration result");
        assert!(quality.converged);
        assert!(quality.max_error_bps.is_finite());
    }

    #[test]
    fn test_bdt_stored_lattice_prices_zero_coupon_to_calibration_curve() {
        let steps = 8;
        let maturity = 2.0;
        let mut tree = ShortRateTree::black_derman_toy(steps, 0.20, 0.0);
        let curve = create_test_curve();
        tree.calibrate(&test_curve_id(), &curve, maturity)
            .expect("BDT calibration");

        let mut vars = HashMap::<&'static str, f64>::default();
        vars.insert(
            short_rate_keys::SHORT_RATE,
            tree.rate_at_node(0, 0).expect("root rate"),
        );
        let market = MarketContext::new();
        let actual = tree
            .price(vars, maturity, &market, &ConstantValuator)
            .expect("BDT zero coupon price");
        let expected = curve.df(maturity);

        assert!(
            (actual - expected).abs() < 1e-8,
            "BDT stored lattice should price a zero coupon to the calibration curve: actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn test_bdt_config_uses_binomial_branching_matching_calibration_geometry() {
        let config = ShortRateTreeConfig::bdt(6, 0.20, 0.0);
        assert_eq!(config.branching, TreeBranching::Binomial);

        let mut tree = ShortRateTree::new(config);
        let curve = create_test_curve();
        tree.calibrate(&test_curve_id(), &curve, 2.0)
            .expect("BDT calibration");

        for step in 0..=6 {
            assert_eq!(
                tree.rates[step].len(),
                step + 1,
                "BDT calibration is binomial-width at step {step}"
            );
        }
    }

    #[test]
    fn test_short_rate_tree_rejects_branching_geometry_mismatch() {
        let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(6, 0.20, 0.0).with_trinomial());
        let curve = create_test_curve();
        tree.calibrate(&test_curve_id(), &curve, 2.0)
            .expect("BDT calibration");

        let mut vars = HashMap::<&'static str, f64>::default();
        vars.insert(
            short_rate_keys::SHORT_RATE,
            tree.rate_at_node(0, 0).expect("root rate"),
        );
        vars.insert(short_rate_keys::OAS, 0.0);
        let market = MarketContext::new();
        let err = tree
            .price(vars, 2.0, &market, &ConstantValuator)
            .expect_err("pricing must reject missing trinomial nodes instead of using zero rates");

        assert!(
            err.to_string().contains("lattice geometry"),
            "unexpected error: {err}"
        );
    }

    /// Terminal probability distribution over the BK trinomial lattice
    /// (transition-probability measure, no discounting). Returns the node
    /// probabilities and the terminal x-values `x = ln r − a_N`.
    fn bk_terminal_x_distribution(tree: &ShortRateTree) -> (Vec<f64>, Vec<f64>) {
        let lattice = tree.bk_trinomial.as_ref().expect("BK trinomial lattice");
        let steps = tree.config.steps;
        let j_max = lattice.j_max;
        let dt = tree.time_steps[1] - tree.time_steps[0];
        let dx = tree.config.volatility * (3.0 * dt).sqrt();

        let mut dist = vec![1.0];
        for step in 0..steps {
            let curr_j_max = step.min(j_max);
            let next_j_max = (step + 1).min(j_max);
            let boundary = if curr_j_max == next_j_max {
                curr_j_max
            } else {
                usize::MAX
            };
            let mut next = vec![0.0; 2 * next_j_max + 1];
            for (j, &pj) in dist.iter().enumerate() {
                let j_signed = j as i32 - curr_j_max as i32;
                for (offset, p) in
                    HullWhiteTree::transition_offsets(j_signed, boundary, lattice.probs[step][j])
                {
                    if let Some(idx) = HullWhiteTree::transition_index(j_signed, offset, next_j_max)
                    {
                        next[idx] += pj * p;
                    }
                }
            }
            dist = next;
        }

        let term_j_max = steps.min(j_max);
        let xs: Vec<f64> = (0..dist.len())
            .map(|j| (j as i32 - term_j_max as i32) as f64 * dx)
            .collect();
        (xs, dist)
    }

    fn weighted_std(values: &[f64], weights: &[f64]) -> f64 {
        let total: f64 = weights.iter().sum();
        let mean: f64 = values.iter().zip(weights).map(|(v, w)| v * w).sum::<f64>() / total;
        let var: f64 = values
            .iter()
            .zip(weights)
            .map(|(v, w)| w * (v - mean) * (v - mean))
            .sum::<f64>()
            / total;
        var.sqrt()
    }

    /// with κ ≠ 0 the BDT model routes to a genuine
    /// trinomial Black-Karasinski lattice that still reprices the curve and
    /// tightens the (probability-weighted) terminal log-rate dispersion
    /// relative to κ = 0.
    #[test]
    fn test_bdt_mean_reversion_calibrates_and_tightens_rate_dispersion() {
        let steps = 50;
        let mut tree_no_mr = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, 0.20, 0.0));
        let mut tree_mr = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, 0.20, 0.05));
        let curve = create_test_curve();

        let cid = test_curve_id();
        tree_no_mr.calibrate(&cid, &curve, 2.0).expect("BDT(κ=0)");
        tree_mr.calibrate(&cid, &curve, 2.0).expect("BK(κ=0.05)");

        let quality = tree_mr.calibration_result().expect("quality");
        assert!(
            quality.is_acceptable(),
            "BK(κ=0.05) calibration: max_error={:.2}bp",
            quality.max_error_bps
        );

        // Probability-weighted terminal ln-rate dispersion: κ > 0 tightens it.
        // Binomial κ=0 tree: terminal distribution is Binomial(steps, 1/2).
        let ln_rates_no_mr: Vec<f64> = tree_no_mr.rates[steps].iter().map(|r| r.ln()).collect();
        let mut binom_weights = vec![0.0_f64; steps + 1];
        let mut c = 1.0_f64;
        for (k, w) in binom_weights.iter_mut().enumerate() {
            *w = c * 0.5_f64.powi(steps as i32);
            c = c * (steps - k) as f64 / (k + 1) as f64;
        }
        let std_no_mr = weighted_std(&ln_rates_no_mr, &binom_weights);

        let (xs_mr, dist_mr) = bk_terminal_x_distribution(&tree_mr);
        let std_mr = weighted_std(&xs_mr, &dist_mr);

        assert!(
            std_mr < std_no_mr,
            "mean reversion should tighten terminal log-rate dispersion: \
             no_mr={std_no_mr:.6}, mr={std_mr:.6}"
        );

        let market = MarketContext::new();
        let mut vars = HashMap::<&'static str, f64>::default();
        vars.insert(
            short_rate_keys::SHORT_RATE,
            tree_mr.rate_at_node(0, 0).expect("root"),
        );
        let zcb = tree_mr
            .price(vars, 2.0, &market, &ConstantValuator)
            .expect("ZCB price");
        let target = curve.df(2.0);
        assert!(
            (zcb - target).abs() < 1e-6,
            "BK(κ=0.05) should still price ZCBs to curve: got={zcb:.8}, target={target:.8}"
        );
    }

    /// the BK trinomial lattice reprices the calibration
    /// curve to <0.1 bp, both via Arrow-Debreu state prices and via the
    /// dedicated backward induction in `price()`.
    #[test]
    fn bk_trinomial_reprices_curve_to_a_tenth_bp() {
        let steps = 200;
        let maturity = 5.0;
        let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, 0.20, 0.03));
        let curve = create_test_curve();
        tree.calibrate(&test_curve_id(), &curve, maturity)
            .expect("BK calibration");

        let quality = tree.calibration_result().expect("quality");
        assert!(
            quality.converged && quality.max_error_bps < 0.1,
            "BK calibration must reprice the curve to <0.1bp, got {quality:?}"
        );

        let market = MarketContext::new();
        let zcb = tree
            .price(
                HashMap::<&'static str, f64>::default(),
                maturity,
                &market,
                &ConstantValuator,
            )
            .expect("ZCB price");
        let target = curve.df(maturity);
        let error_bps = ((zcb - target) / target).abs() * 10_000.0;
        assert!(
            error_bps < 0.1,
            "BK backward induction must reprice ZCB to <0.1bp: \
             got={zcb:.8}, target={target:.8} ({error_bps:.4}bp)"
        );
    }

    /// as Δt → 0 the terminal log-rate dispersion of the
    /// BK lattice approaches the OU limit `σ√((1−e^{−2κT})/(2κ))` — about
    /// 13% below σ√T at κ = 0.03, T = 10y — instead of growing like σ√T.
    #[test]
    fn bk_terminal_log_rate_dispersion_matches_ou_limit() {
        let steps = 400;
        let maturity = 10.0;
        let sigma = 0.20;
        let kappa = 0.03;
        let curve = create_flat_curve(0.04);

        let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, sigma, kappa));
        tree.calibrate(&test_curve_id(), &curve, maturity)
            .expect("BK calibration");

        let (xs, dist) = bk_terminal_x_distribution(&tree);
        let std_x = weighted_std(&xs, &dist);

        let target = sigma * ((1.0 - (-2.0 * kappa * maturity).exp()) / (2.0 * kappa)).sqrt();
        let no_mr = sigma * maturity.sqrt();

        assert!(
            ((std_x - target) / target).abs() < 0.02,
            "terminal log-rate dispersion should match the OU limit: \
             got {std_x:.6}, target {target:.6} (σ√T = {no_mr:.6})"
        );
        assert!(
            std_x < 0.95 * no_mr,
            "dispersion must be materially below the κ=0 value σ√T: \
             got {std_x:.6} vs σ√T = {no_mr:.6}"
        );
    }

    /// as κ → 0 the trinomial BK lattice converges to the
    /// binomial BDT lattice (same continuous model).
    #[test]
    fn bk_kappa_to_zero_converges_to_bdt() {
        let steps = 200;
        let maturity = 5.0;
        let sigma = 0.20;
        let curve = create_flat_curve(0.04);
        let cid = test_curve_id();
        let market = MarketContext::new().insert(curve.clone());
        let valuator = RateCallValuator { strike: 0.04 };
        let vars = HashMap::<&'static str, f64>::default();

        let mut bdt = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, sigma, 0.0));
        bdt.calibrate(&cid, &curve, maturity).expect("BDT(κ=0)");
        let price_bdt = bdt
            .price(vars.clone(), maturity, &market, &valuator)
            .expect("BDT price");

        let mut bk = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, sigma, 1e-4));
        bk.calibrate(&cid, &curve, maturity).expect("BK(κ→0)");
        assert!(bk.bk_trinomial.is_some(), "κ=1e-4 must route to BK lattice");
        let price_bk = bk
            .price(vars, maturity, &market, &valuator)
            .expect("BK price");

        // Tiny terminal dispersion check: at κ→0 the OU limit is σ√T.
        let (xs, dist) = bk_terminal_x_distribution(&bk);
        let std_x = weighted_std(&xs, &dist);
        let target = sigma * maturity.sqrt();
        assert!(
            ((std_x - target) / target).abs() < 0.01,
            "κ→0 dispersion should approach σ√T: got {std_x:.6}, target {target:.6}"
        );

        assert!(
            price_bdt > 0.0 && price_bk > 0.0,
            "rate-call prices must be positive: bdt={price_bdt}, bk={price_bk}"
        );
        let rel = ((price_bk - price_bdt) / price_bdt).abs();
        assert!(
            rel < 0.05,
            "κ→0 BK lattice should converge to BDT: bdt={price_bdt:.8}, \
             bk={price_bk:.8} (rel diff {rel:.4})"
        );
    }

    #[test]
    fn short_rate_tree_vega_is_per_one_percent_vol_move_for_custom_bump() {
        let steps = 10;
        let maturity = 2.0;
        let bump = 0.02;
        let curve = create_test_curve();
        let curve_id = test_curve_id();
        let market = MarketContext::new().insert(curve.clone());
        let valuator = RateCallValuator { strike: 0.03 };
        let initial_vars = HashMap::<&'static str, f64>::default();

        let config = ShortRateTreeConfig::bdt(steps, 0.20, 0.0);
        let mut tree = ShortRateTree::new(config.clone());
        tree.calibrate(&curve_id, &curve, maturity)
            .expect("base calibration");

        let greeks = tree
            .calculate_greeks(
                initial_vars.clone(),
                maturity,
                &market,
                &valuator,
                Some(bump),
            )
            .expect("short-rate greeks");

        let mut up_config = config.clone();
        up_config.volatility += bump;
        let mut up_tree = ShortRateTree::new(up_config);
        up_tree
            .calibrate(&curve_id, &curve, maturity)
            .expect("up calibration");
        let price_up = up_tree
            .price(initial_vars.clone(), maturity, &market, &valuator)
            .expect("up price");

        let mut down_config = config;
        down_config.volatility = (down_config.volatility - bump).max(1e-6);
        let mut down_tree = ShortRateTree::new(down_config);
        down_tree
            .calibrate(&curve_id, &curve, maturity)
            .expect("down calibration");
        let price_down = down_tree
            .price(initial_vars, maturity, &market, &valuator)
            .expect("down price");

        let expected = (price_up - price_down) / (2.0 * bump) * 0.01;
        assert!(
            (greeks.vega - expected).abs() < 1e-12,
            "vega should be per 1 percentage-point vol move: got={}, expected={}",
            greeks.vega,
            expected
        );
    }

    #[test]
    fn short_rate_tree_default_vol_bump_is_relative() {
        // The default bump must be 10% of the calibrated vol (floored at
        // 1 bp), not a fixed absolute 0.01 — for low-vol configs the fixed
        // bump was a ~100% relative shock that distorted the FD vega.
        let steps = 10;
        let maturity = 2.0;
        let sigma = 0.20;
        let curve = create_test_curve();
        let curve_id = test_curve_id();
        let market = MarketContext::new().insert(curve.clone());
        let valuator = RateCallValuator { strike: 0.03 };
        let initial_vars = HashMap::<&'static str, f64>::default();

        let config = ShortRateTreeConfig::bdt(steps, sigma, 0.0);
        let mut tree = ShortRateTree::new(config);
        tree.calibrate(&curve_id, &curve, maturity)
            .expect("base calibration");

        let default_greeks = tree
            .calculate_greeks(initial_vars.clone(), maturity, &market, &valuator, None)
            .expect("default-bump greeks");
        let explicit_greeks = tree
            .calculate_greeks(
                initial_vars,
                maturity,
                &market,
                &valuator,
                Some((0.1 * sigma).max(1e-4)),
            )
            .expect("explicit-bump greeks");

        assert!(
            (default_greeks.vega - explicit_greeks.vega).abs() < 1e-12,
            "default bump should equal max(0.1·σ, 1bp): default vega={}, explicit vega={}",
            default_greeks.vega,
            explicit_greeks.vega
        );
    }

    // ========================================================================
    // Volatility Conversion Tests
    // ========================================================================

    #[test]
    fn test_normal_to_lognormal_vol_conversion() {
        // Test that conversion produces reasonable lognormal vol and round-trips correctly
        let normal_vol = 0.01; // 100 bps
        let rate_level = 0.05; // 5%

        let lognormal = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            rate_level,
            1.0,
        )
        .expect("valid conversion");

        // Lognormal vol should be in a reasonable range (roughly normal_vol / rate_level)
        assert!(
            lognormal > 0.15 && lognormal < 0.25,
            "lognormal vol {lognormal} out of range"
        );

        // Round-trip should recover original
        let recovered = convert_atm_volatility(
            lognormal,
            VolatilityConvention::Lognormal,
            VolatilityConvention::Normal,
            rate_level,
            1.0,
        )
        .expect("valid conversion");
        assert!(
            (recovered - normal_vol).abs() < 1e-10,
            "Round-trip failed: got {recovered}, expected {normal_vol}"
        );
    }

    #[test]
    fn test_lognormal_to_normal_vol_conversion() {
        // Test that conversion produces reasonable normal vol and round-trips correctly
        let lognormal_vol = 0.20; // 20%
        let rate_level = 0.05; // 5%

        let normal = convert_atm_volatility(
            lognormal_vol,
            VolatilityConvention::Lognormal,
            VolatilityConvention::Normal,
            rate_level,
            1.0,
        )
        .expect("valid conversion");

        // Normal vol should be in a reasonable range (roughly lognormal_vol * rate_level)
        assert!(
            normal > 0.005 && normal < 0.015,
            "normal vol {normal} out of range"
        );

        // Round-trip should recover original
        let recovered = convert_atm_volatility(
            normal,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            rate_level,
            1.0,
        )
        .expect("valid conversion");
        assert!(
            (recovered - lognormal_vol).abs() < 1e-10,
            "Round-trip failed: got {recovered}, expected {lognormal_vol}"
        );
    }

    #[test]
    fn test_vol_conversion_roundtrip() {
        let original_normal = 0.012; // 120 bps
        let rate_level = 0.045; // 4.5%

        let lognormal = convert_atm_volatility(
            original_normal,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            rate_level,
            1.0,
        )
        .expect("valid conversion");
        let back_to_normal = convert_atm_volatility(
            lognormal,
            VolatilityConvention::Lognormal,
            VolatilityConvention::Normal,
            rate_level,
            1.0,
        )
        .expect("valid conversion");

        assert!(
            (back_to_normal - original_normal).abs() < 1e-6,
            "Roundtrip conversion should be exact"
        );
    }

    #[test]
    fn test_normal_to_lognormal_errors_on_zero_rate() {
        let err = convert_atm_volatility(
            0.01,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            0.0,
            1.0,
        )
        .expect_err("should error");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_normal_to_lognormal_errors_on_negative_rate() {
        let err = convert_atm_volatility(
            0.01,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            -0.01,
            1.0,
        )
        .expect_err("should error");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_calibration_result_quality_helpers_cover_thresholds() {
        let good = CalibrationResult {
            max_error_bps: 0.05,
            max_error_step: 2,
            fallback_count: 0,
            converged: true,
        };
        assert!(good.is_good());
        assert!(good.is_acceptable());

        let acceptable_only = CalibrationResult {
            max_error_bps: 0.5,
            max_error_step: 3,
            fallback_count: 0,
            converged: true,
        };
        assert!(!acceptable_only.is_good());
        assert!(acceptable_only.is_acceptable());

        let poor = CalibrationResult {
            max_error_bps: 2.0,
            max_error_step: 1,
            fallback_count: 1,
            converged: true,
        };
        assert!(!poor.is_good());
        assert!(!poor.is_acceptable());
    }

    #[test]
    fn compounding_conventions_stay_finite_for_deeply_negative_rates() {
        for compounding in [
            TreeCompounding::Simple,
            TreeCompounding::SemiAnnual,
            TreeCompounding::Quarterly,
            TreeCompounding::Monthly,
        ] {
            let df = compounding.df(-100.0, 0.5);
            let continuous = compounding.to_continuous(-100.0, 0.5);
            assert!(
                df.is_finite() && df > 0.0,
                "{compounding:?} discount factor should stay positive and finite, got {df}"
            );
            assert!(
                continuous.is_finite(),
                "{compounding:?} continuous equivalent should stay finite, got {continuous}"
            );
        }
    }

    #[test]
    fn bdt_calibrates_near_zero_flat_curve_without_fallbacks() {
        let curve = create_flat_curve(0.0001);
        let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(12, 0.20, 0.0));

        tree.calibrate(&test_curve_id(), &curve, 2.0)
            .expect("near-zero BDT calibration");

        let quality = tree.calibration_result().expect("quality");
        assert_eq!(quality.fallback_count, 0);
        assert!(quality.is_acceptable(), "quality={quality:?}");
        for step in 0..=12 {
            for node in 0..=step {
                let rate = tree.rate_at_node(step, node).expect("rate");
                assert!(rate.is_finite() && rate > 0.0, "rate={rate}");
            }
        }
    }

    #[test]
    fn bdt_calibrates_high_rate_flat_curve_with_finite_rates() {
        let curve = create_flat_curve(0.75);
        let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(12, 0.20, 0.0));

        tree.calibrate(&test_curve_id(), &curve, 2.0)
            .expect("high-rate BDT calibration");

        let quality = tree.calibration_result().expect("quality");
        assert_eq!(quality.fallback_count, 0);
        assert!(quality.is_acceptable(), "quality={quality:?}");
        for step in 0..=12 {
            for node in 0..=step {
                let rate = tree.rate_at_node(step, node).expect("rate");
                assert!(rate.is_finite() && rate > 0.0, "rate={rate}");
            }
        }
    }

    #[test]
    fn bdt_calibration_fails_when_node_rate_clamp_engages_materially() {
        // P0-6: BDT clamps every node rate to `[1e-8, 5.0]` inside the Brent
        // objective. For a tree that is too wide (high vol, many steps, long
        // horizon) the clamp saturates nodes with material Arrow-Debreu
        // weight, the objective stops responding to `alpha`, and the
        // calibrated tree silently *fails* to reprice the curve (here by many
        // thousands of basis points). Calibration must NOT report success in
        // that case — it must return an explicit error rather than a
        // quietly-mispriced tree.
        //
        // sigma = 1.50, 120 steps, T = 60 => step_vol ~ 1.50*sqrt(0.5) ~ 1.06,
        // u ~ 2.89; the lattice is so wide the clamp wrecks repricing.
        let curve = create_flat_curve(0.05);
        let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(120, 1.50, 0.0));

        let result = tree.calibrate(&test_curve_id(), &curve, 60.0);
        assert!(
            result.is_err(),
            "a BDT calibration whose node-rate clamp engages materially must \
             fail explicitly, not return a silently-mispriced tree"
        );
        let msg = result.expect_err("must error").to_string().to_lowercase();
        assert!(
            msg.contains("clamp") || msg.contains("reprice") || msg.contains("calibrat"),
            "error should explain the calibration / clamp failure, got: {msg}"
        );
    }

    #[test]
    fn bdt_calibration_succeeds_for_a_normal_well_posed_tree() {
        // The clamp-engagement guard must not be over-eager: an ordinary
        // BDT tree (moderate vol, moderate horizon) whose node rates stay
        // comfortably inside `[1e-8, 5.0]` must still calibrate cleanly.
        let curve = create_test_curve();
        let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(40, 0.20, 0.0));
        tree.calibrate(&test_curve_id(), &curve, 5.0)
            .expect("a well-posed BDT tree must calibrate");
        let quality = tree.calibration_result().expect("quality");
        assert!(quality.converged, "quality={quality:?}");
        assert!(quality.is_acceptable(), "quality={quality:?}");
    }

    // ========================================================================
    // Config Factory Tests
    // ========================================================================

    #[test]
    fn test_config_ho_lee_factory() {
        let config = ShortRateTreeConfig::ho_lee(100, 0.008);
        assert_eq!(config.steps, 100);
        assert_eq!(config.model, ShortRateModel::HoLee);
        assert_eq!(config.volatility, 0.008);
        assert_eq!(config.mean_reversion, None);
    }

    #[test]
    fn test_config_bdt_factory() {
        let config = ShortRateTreeConfig::bdt(100, 0.20, 0.03);
        assert_eq!(config.steps, 100);
        assert_eq!(config.model, ShortRateModel::BlackDermanToy);
        assert_eq!(config.volatility, 0.20);
        assert_eq!(config.mean_reversion, Some(0.03));
    }

    #[test]
    fn test_config_from_normal_vol_factory() {
        let config = ShortRateTreeConfig::from_normal_vol(100, 0.008, 0.005).expect("valid config");
        assert_eq!(config.model, ShortRateModel::HoLee);

        let config = ShortRateTreeConfig::from_normal_vol(100, 0.01, 0.05).expect("valid config");
        assert_eq!(config.model, ShortRateModel::BlackDermanToy);
        // Vol should be in reasonable range (roughly normal_vol / rate_level ≈ 0.20)
        assert!(
            config.volatility > 0.15 && config.volatility < 0.25,
            "volatility {} out of expected range",
            config.volatility
        );
    }

    #[test]
    fn test_config_default_ho_lee() {
        let config = ShortRateTreeConfig::default_ho_lee(50);
        assert_eq!(config.steps, 50);
        assert_eq!(config.model, ShortRateModel::HoLee);
        assert_eq!(config.volatility, DEFAULT_NORMAL_VOL);
    }

    #[test]
    fn test_config_default_bdt() {
        let config = ShortRateTreeConfig::default_bdt(50);
        assert_eq!(config.steps, 50);
        assert_eq!(config.model, ShortRateModel::BlackDermanToy);
        assert_eq!(config.volatility, DEFAULT_LOGNORMAL_VOL);
    }

    #[test]
    fn test_config_from_normal_vol_low_rates() {
        // Low rate environment → should use Ho-Lee
        let config = ShortRateTreeConfig::from_normal_vol(100, 0.008, 0.005).expect("valid config");
        assert_eq!(config.model, ShortRateModel::HoLee);
        assert_eq!(config.volatility, 0.008); // Unchanged
    }

    #[test]
    fn test_config_from_normal_vol_normal_rates() {
        // Normal rate environment → should use BDT with converted vol
        let config = ShortRateTreeConfig::from_normal_vol(100, 0.01, 0.05).expect("valid config");
        assert_eq!(config.model, ShortRateModel::BlackDermanToy);
        // Vol should be in reasonable range (roughly normal_vol / rate_level ≈ 0.20)
        assert!(
            config.volatility > 0.15 && config.volatility < 0.25,
            "volatility {} out of expected range",
            config.volatility
        );
    }

    #[test]
    fn test_config_branching_helpers_and_normal_vol_boundary() {
        let binomial = ShortRateTreeConfig::bdt(50, 0.20, 0.03).with_binomial();
        assert_eq!(binomial.branching, TreeBranching::Binomial);

        let trinomial = ShortRateTreeConfig::ho_lee(50, 0.01).with_trinomial();
        assert_eq!(trinomial.branching, TreeBranching::Trinomial);

        let boundary = ShortRateTreeConfig::from_normal_vol(50, 0.01, 0.01).expect("valid config");
        assert_eq!(boundary.model, ShortRateModel::BlackDermanToy);
    }

    // ========================================================================
    // Tree Factory Tests
    // ========================================================================

    #[test]
    fn test_tree_default_ho_lee() {
        let tree = ShortRateTree::default_ho_lee(75);
        assert_eq!(tree.config.steps, 75);
        assert_eq!(tree.config.model, ShortRateModel::HoLee);
        assert_eq!(tree.config.volatility, DEFAULT_NORMAL_VOL);
    }

    #[test]
    fn test_tree_default_bdt() {
        let tree = ShortRateTree::default_bdt(75);
        assert_eq!(tree.config.steps, 75);
        assert_eq!(tree.config.model, ShortRateModel::BlackDermanToy);
        assert_eq!(tree.config.volatility, DEFAULT_LOGNORMAL_VOL);
    }

    #[test]
    fn test_probability_and_time_accessors_validate_bounds() {
        let mut tree = ShortRateTree::ho_lee(5, 0.01);
        let curve = create_test_curve();
        tree.calibrate(&test_curve_id(), &curve, 1.0)
            .expect("should succeed");

        assert_eq!(tree.probabilities(0).expect("probabilities"), (0.5, 0.5));
        assert_eq!(tree.time_at_step(0).expect("time"), 0.0);
        assert!(tree.time_at_step(5).expect("time").is_finite());
        assert!(tree.probabilities(10).is_err());
        assert!(tree.time_at_step(10).is_err());
    }

    #[test]
    fn test_price_rejects_uncalibrated_tree() {
        let tree = ShortRateTree::ho_lee(5, 0.01);
        let err = tree
            .price(
                HashMap::<&'static str, f64>::default(),
                1.0,
                &MarketContext::new(),
                &ConstantValuator,
            )
            .expect_err("uncalibrated tree should error");
        assert!(err.to_string().contains("must be calibrated"));
    }

    #[test]
    fn test_ho_lee_rejects_nonzero_mean_reversion() {
        let config = ShortRateTreeConfig {
            steps: 10,
            model: ShortRateModel::HoLee,
            volatility: 0.01,
            mean_reversion: Some(0.05),
            branching: TreeBranching::Binomial,
            compounding: TreeCompounding::default(),
        };
        let mut tree = ShortRateTree::new(config);
        let curve = create_test_curve();
        let err = tree
            .calibrate(&test_curve_id(), &curve, 2.0)
            .expect_err("Ho-Lee with mean reversion must be rejected");
        assert!(
            err.to_string().contains("mean reversion"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_ho_lee_allows_zero_mean_reversion() {
        let config = ShortRateTreeConfig {
            steps: 10,
            model: ShortRateModel::HoLee,
            volatility: 0.01,
            mean_reversion: Some(0.0),
            branching: TreeBranching::Binomial,
            compounding: TreeCompounding::default(),
        };
        let mut tree = ShortRateTree::new(config);
        let curve = create_test_curve();
        tree.calibrate(&test_curve_id(), &curve, 2.0)
            .expect("Ho-Lee with κ=0 should succeed");
    }
}
