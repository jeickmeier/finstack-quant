use super::super::super::super::types::Bond;
use crate::instruments::pricing_overrides::{OasPriceBasis, OasQuoteCompounding};
use crate::models::trees::TreeCompounding;
use finstack_core::types::CurveId;
use finstack_core::types::Percentage;

/// Choice of short-rate model for the bond pricing tree.
///
/// Controls which interest rate tree is used for backward induction. The default
/// `HoLee` model is a simple parallel-shift tree appropriate for quick estimates.
/// For production callable bond OAS, prefer `HullWhite` with calibrated parameters
/// or `HullWhiteCalibratedToSwaptions` for automatic calibration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "type")]
#[derive(Default)]
pub enum TreeModelChoice {
    /// Ho-Lee / BDT model (current default) with exogenous volatility.
    #[default]
    HoLee,
    /// Hull-White 1-factor with user-specified parameters.
    HullWhite {
        /// Mean reversion speed (e.g., 0.03 for 3%)
        kappa: f64,
        /// Short rate volatility (e.g., 0.01 for 100bp)
        sigma: f64,
    },
    /// Black-Derman-Toy lognormal short-rate model.
    BlackDermanToy {
        /// Mean reversion speed.
        ///
        /// The current BDT calibration is binomial and non-mean-reverting; use
        /// `0.0` here. Nonzero mean reversion is rejected to avoid silently
        /// ignoring a model input.
        mean_reversion: f64,
        /// Lognormal short-rate volatility (e.g., 0.20 for 20%)
        sigma: f64,
    },
    /// Hull-White 1-factor calibrated to co-terminal swaptions.
    ///
    /// Extracts relevant swaption quotes from the market context and calibrates
    /// (kappa, sigma) automatically. This is the recommended choice for
    /// production callable bond OAS.
    HullWhiteCalibratedToSwaptions {
        /// ID of the swaption volatility surface in the market context
        swaption_vol_surface_id: String,
    },
}

/// Configuration for tree-based bond pricing (callable/putable bonds, OAS).
///
/// Controls the tree structure, convergence settings, and solver parameters
/// for option-adjusted spread calculations.
///
/// # Volatility Convention
///
/// ⚠️ **Critical**: The volatility interpretation depends on the underlying model:
///
/// | Model | Vol Type | Parameter | Typical Range |
/// |-------|----------|-----------|---------------|
/// | Ho-Lee (default) | Normal/Absolute | σ (rate units) | 50-150 bps (0.005-0.015) |
/// | BDT | Lognormal/Relative | σ (proportion) | 15-30% (0.15-0.30) |
///
/// The default configuration uses Ho-Lee with **normal volatility**.
///
/// ## Volatility Ranges by Model Type
///
/// ### Ho-Lee (Normal Volatility - Default)
///
/// | Rate Environment | Typical Vol Range | Example |
/// |------------------|-------------------|---------|
/// | Low rates (< 2%) | 50-80 bps | 0.005-0.008 |
/// | Normal rates (2-5%) | 80-120 bps | 0.008-0.012 |
/// | High rates (> 5%) | 100-150 bps | 0.010-0.015 |
/// | Crisis/stress | 150-300 bps | 0.015-0.030 |
///
/// ### Black-Derman-Toy (Lognormal Volatility)
///
/// | Market Condition | Typical Vol Range | Example |
/// |------------------|-------------------|---------|
/// | Low volatility | 10-15% | 0.10-0.15 |
/// | Normal market | 15-25% | 0.15-0.25 |
/// | High vol/stress | 25-40% | 0.25-0.40 |
///
/// ## Calibration Approaches
///
/// | Approach | Description | When to Use |
/// |----------|-------------|-------------|
/// | **Swaption-implied** | Calibrate to ATM swaption vol at bond's maturity | Institutional trading |
/// | **Historical** | Rolling 1Y historical rate vol | Quick estimates |
/// | **Model-implied** | Hull-White or BDT calibration | Full term structure |
///
/// ## Converting Between Conventions
///
/// Use `finstack_core::math::volatility::convert_atm_volatility`:
///
/// ```ignore
/// use finstack_core::math::volatility::{convert_atm_volatility, VolatilityConvention};
///
/// // Normal vol (100 bps) at 5% rate → lognormal vol (20%)
/// let lognormal = convert_atm_volatility(
///     0.01,
///     VolatilityConvention::Normal,
///     VolatilityConvention::Lognormal,
///     0.05,
///     1.0,
/// )?;
///
/// // Lognormal vol (20%) at 5% rate → normal vol (100 bps)
/// let normal = convert_atm_volatility(
///     0.20,
///     VolatilityConvention::Lognormal,
///     VolatilityConvention::Normal,
///     0.05,
///     1.0,
/// )?;
/// # Ok::<(), finstack_core::Error>(())
/// ```
///
/// # Tree Resolution
///
/// The `tree_steps` parameter controls pricing accuracy vs computation time:
///
/// | Steps | Accuracy | Use Case |
/// |-------|----------|----------|
/// | 50 | ~2-5 bp | Quick screening |
/// | 100 | ~1 bp | Default, most trading |
/// | 200 | < 0.5 bp | Risk reports |
/// | 500 | < 0.2 bp | Regulatory/audit |
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricerConfig;
///
/// // Default configuration using Ho-Lee with 100 bps normal vol
/// let default = TreePricerConfig::default();
///
/// // Production configuration with calibrated normal volatility
/// let production = TreePricerConfig::production_ho_lee(0.01); // 100 bps
///
/// // BDT model with lognormal volatility
/// let bdt = TreePricerConfig::production_bdt(0.20); // 20% lognormal
///
/// // High-precision configuration for regulatory reporting
/// let audit = TreePricerConfig::high_precision(0.01);
///
/// // Hull-White model (production recommended for callable bonds)
/// let hw = TreePricerConfig::hull_white(0.03, 0.01);
/// ```
#[derive(Debug, Clone)]
pub struct TreePricerConfig {
    /// Number of time steps in the interest rate tree.
    ///
    /// Higher values improve accuracy but increase computation time quadratically.
    /// Recommended: 100 for trading, 200+ for risk reports.
    pub tree_steps: usize,

    /// Short rate volatility (annualized).
    ///
    /// ⚠️ **Interpretation depends on model type**:
    /// - **Ho-Lee (default)**: Normal volatility in rate units (0.01 = 100 bps)
    /// - **BDT**: Lognormal volatility as proportion (0.20 = 20%)
    ///
    /// The default value of 100 bps (0.01) is appropriate for Ho-Lee model
    /// in normal rate environments. For BDT, use 15-25% (0.15-0.25).
    ///
    /// See struct-level documentation for calibration guidance and typical ranges.
    pub volatility: f64,

    /// Convergence tolerance for iterative solvers (OAS root finding).
    ///
    /// Default: `1e-6` (0.01 bp precision on OAS).
    /// Tighter tolerances increase iterations but improve precision.
    pub tolerance: f64,

    /// Maximum iterations for root finding algorithms.
    ///
    /// The OAS solver uses Brent's method which typically converges
    /// in 10-20 iterations. The cap prevents infinite loops on
    /// pathological inputs.
    pub max_iterations: usize,

    /// Initial bracket size (in basis points) for the OAS root solver.
    ///
    /// Wider brackets handle distressed/high-spread bonds but may
    /// slow convergence for tight spreads. Default: 1000 bp.
    pub initial_bracket_size_bp: Option<f64>,

    /// Mean reversion speed (annualized).
    ///
    /// Used by `HullWhite` and `BlackDermanToy` tree models. Ignored by
    /// `HoLee` — for mean-reverting normal models, use `HullWhite` instead.
    ///
    /// - `None` (default): no mean reversion
    /// - `Some(0.03)`: 3% annual mean reversion (moderate)
    /// - `Some(0.10)`: 10% annual mean reversion (strong)
    pub mean_reversion: Option<f64>,

    /// Short-rate model for the pricing tree.
    ///
    /// - `HoLee` (default): Uses the existing `ShortRateTree` path.
    /// - `HullWhite { kappa, sigma }`: Uses a calibrated HW trinomial tree.
    /// - `HullWhiteCalibratedToSwaptions { .. }`: Auto-calibrates HW params
    ///   from swaption vol data (preferred for production callable bond OAS).
    pub tree_model: TreeModelChoice,

    /// Optional discount curve used only for tree/OAS calibration.
    pub tree_discount_curve_id: Option<CurveId>,
    /// Quote convention used for OAS inputs and outputs.
    pub oas_quote_compounding: OasQuoteCompounding,
    /// Price/accrual target convention for OAS inversion.
    pub oas_price_basis: OasPriceBasis,
    /// Per-node compounding convention for the short-rate tree.
    pub tree_compounding: TreeCompounding,
}

impl Default for TreePricerConfig {
    /// Default configuration using Ho-Lee model with 100 bps normal volatility.
    ///
    /// This is appropriate for normal rate environments (2-5% rates).
    /// For low/negative rate environments, consider lower volatility.
    /// For BDT model, use [`TreePricerConfig::default_bdt`] instead.
    fn default() -> Self {
        Self {
            tree_steps: 200,
            volatility: 0.01, // 100 bps normal vol - appropriate for Ho-Lee
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: None,
            tree_model: TreeModelChoice::default(),
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }
}

/// Get the tree pricer configuration for a bond.
///
/// This centralized function sources tree config from `bond.pricing_overrides`
/// when present, otherwise returns defaults. Use this instead of constructing
/// `TreePricerConfig::default()` directly to ensure consistent configuration
/// across all tree-based pricing paths (OAS metric, price_from_oas, embedded
/// option value, etc.).
///
/// # Arguments
///
/// * `bond` - The bond to get tree config for
///
/// # Returns
///
/// A `TreePricerConfig` with values from pricing_overrides or defaults.
///
/// # Examples
///
/// ```ignore
/// use finstack_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_valuations::instruments::fixed_income::bond::pricing::engine::tree::bond_tree_config;
///
/// let bond = Bond::example().unwrap();
/// let config = bond_tree_config(&bond);
/// ```
pub fn bond_tree_config(bond: &Bond) -> TreePricerConfig {
    let volatility = bond
        .pricing_overrides
        .market_quotes
        .implied_volatility
        .unwrap_or(0.01);

    let uses_black_lognormal = matches!(
        bond.pricing_overrides.model_config.vol_model,
        Some(crate::instruments::common_impl::parameters::VolatilityModel::Black)
    );

    // For callable/putable bonds, default to Hull-White with reasonable parameters.
    // HullWhiteCalibratedToSwaptions should be preferred when swaption vol data
    // is available in the market context.
    let tree_model = if bond.call_put.is_some() {
        if uses_black_lognormal {
            let mean_reversion = bond
                .pricing_overrides
                .model_config
                .mean_reversion
                .unwrap_or(0.0);
            TreeModelChoice::BlackDermanToy {
                mean_reversion,
                sigma: volatility,
            }
        } else {
            let mean_reversion = bond
                .pricing_overrides
                .model_config
                .mean_reversion
                .unwrap_or(0.03);
            TreeModelChoice::HullWhite {
                kappa: mean_reversion,
                sigma: volatility,
            }
        }
    } else {
        TreeModelChoice::HoLee
    };
    let tree_compounding = if matches!(&tree_model, TreeModelChoice::BlackDermanToy { .. }) {
        TreeCompounding::Simple
    } else {
        TreeCompounding::default()
    };

    TreePricerConfig {
        tree_steps: bond
            .pricing_overrides
            .model_config
            .tree_steps
            .unwrap_or(100),
        volatility,
        tolerance: 1e-6,
        max_iterations: 50,
        initial_bracket_size_bp: Some(1000.0),
        mean_reversion: bond.pricing_overrides.model_config.mean_reversion,
        tree_model,
        tree_discount_curve_id: bond
            .pricing_overrides
            .model_config
            .tree_discount_curve_id
            .clone(),
        oas_quote_compounding: bond.pricing_overrides.model_config.oas_quote_compounding,
        oas_price_basis: bond.pricing_overrides.model_config.oas_price_basis,
        tree_compounding,
    }
}

impl TreePricerConfig {
    // ========================================================================
    // Model-Specific Factory Methods
    // ========================================================================

    /// Create a production configuration for Ho-Lee model with normal volatility.
    ///
    /// Uses 100 tree steps which provides ~1 bp OAS accuracy for most bonds.
    /// Suitable for trading and daily risk reporting.
    ///
    /// # Arguments
    ///
    /// * `normal_vol` - Normal (absolute) volatility in rate units
    ///   (e.g., 0.01 = 100 bps/yr)
    ///
    /// # Typical Values
    ///
    /// - Low rates (<2%): 50-80 bps (0.005-0.008)
    /// - Normal rates (2-5%): 80-120 bps (0.008-0.012)
    /// - High rates (>5%): 100-150 bps (0.010-0.015)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricerConfig;
    ///
    /// // Use 100 bps normal vol calibrated from swaption market
    /// let config = TreePricerConfig::production_ho_lee(0.01);
    /// ```
    pub fn production_ho_lee(normal_vol: f64) -> Self {
        Self {
            tree_steps: 100,
            volatility: normal_vol,
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: None,
            tree_model: TreeModelChoice::HoLee,
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::Simple,
        }
    }

    /// Create a production configuration for Ho-Lee model with typed volatility.
    pub fn production_ho_lee_pct(normal_vol: Percentage) -> Self {
        Self {
            tree_steps: 100,
            volatility: normal_vol.as_decimal(),
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: None,
            tree_model: TreeModelChoice::HoLee,
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::Simple,
        }
    }

    /// Create a production configuration for BDT model with lognormal volatility.
    ///
    /// Uses 100 tree steps which provides ~1 bp OAS accuracy for most bonds.
    /// BDT is preferred for positive rate environments where lognormal
    /// distribution better matches market conventions.
    ///
    /// # Arguments
    ///
    /// * `lognormal_vol` - Lognormal (relative) volatility as proportion
    ///   (e.g., 0.20 = 20%/yr)
    ///
    /// # Typical Values
    ///
    /// - Low volatility: 10-15% (0.10-0.15)
    /// - Normal market: 15-25% (0.15-0.25)
    /// - High vol/stress: 25-40% (0.25-0.40)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricerConfig;
    ///
    /// // Use 20% lognormal vol (equivalent to ~100 bps at 5% rates)
    /// let config = TreePricerConfig::production_bdt(0.20);
    /// ```
    pub fn production_bdt(lognormal_vol: f64) -> Self {
        Self {
            tree_steps: 100,
            volatility: lognormal_vol,
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: Some(0.0),
            tree_model: TreeModelChoice::BlackDermanToy {
                mean_reversion: 0.0,
                sigma: lognormal_vol,
            },
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }

    /// Create a production configuration for BDT model with typed volatility.
    pub fn production_bdt_pct(lognormal_vol: Percentage) -> Self {
        Self {
            tree_steps: 100,
            volatility: lognormal_vol.as_decimal(),
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: Some(0.0),
            tree_model: TreeModelChoice::BlackDermanToy {
                mean_reversion: 0.0,
                sigma: lognormal_vol.as_decimal(),
            },
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }

    /// Create default configuration for BDT model with 20% lognormal volatility.
    ///
    /// This is appropriate for normal positive rate environments.
    pub fn default_bdt() -> Self {
        Self::production_bdt(0.20)
    }

    /// Create a high-precision configuration for regulatory/audit purposes.
    ///
    /// Uses 200 tree steps for < 0.5 bp OAS accuracy and tighter convergence
    /// tolerance. Approximately 4x slower than production configuration.
    ///
    /// # Arguments
    ///
    /// * `calibrated_vol` - Annualized short rate volatility from market calibration
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricerConfig;
    ///
    /// // High precision for regulatory reporting
    /// let config = TreePricerConfig::high_precision(0.012);
    /// ```
    pub fn high_precision(calibrated_vol: f64) -> Self {
        Self {
            tree_steps: 200,
            volatility: calibrated_vol,
            tolerance: 1e-8,
            max_iterations: 100,
            initial_bracket_size_bp: Some(1500.0),
            mean_reversion: None,
            tree_model: TreeModelChoice::HoLee,
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }

    /// Create a high-precision configuration using typed volatility.
    pub fn high_precision_pct(calibrated_vol: Percentage) -> Self {
        Self {
            tree_steps: 200,
            volatility: calibrated_vol.as_decimal(),
            tolerance: 1e-8,
            max_iterations: 100,
            initial_bracket_size_bp: Some(1500.0),
            mean_reversion: None,
            tree_model: TreeModelChoice::HoLee,
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }

    /// Create a fast configuration for screening large portfolios.
    ///
    /// Uses 50 tree steps for ~2-5 bp accuracy. Approximately 4x faster
    /// than production configuration. Suitable for quick screening and
    /// relative value analysis where precision is less critical.
    ///
    /// # Arguments
    ///
    /// * `calibrated_vol` - Annualized short rate volatility from market calibration
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricerConfig;
    ///
    /// // Fast screening of 10,000 bond universe
    /// let config = TreePricerConfig::fast(0.012);
    /// ```
    pub fn fast(calibrated_vol: f64) -> Self {
        Self {
            tree_steps: 50,
            volatility: calibrated_vol,
            tolerance: 1e-4,
            max_iterations: 30,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: None,
            tree_model: TreeModelChoice::HoLee,
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }

    /// Create a fast configuration using typed volatility.
    pub fn fast_pct(calibrated_vol: Percentage) -> Self {
        Self {
            tree_steps: 50,
            volatility: calibrated_vol.as_decimal(),
            tolerance: 1e-4,
            max_iterations: 30,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: None,
            tree_model: TreeModelChoice::HoLee,
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }

    /// Create a configuration using a Hull-White 1-factor tree.
    ///
    /// Recommended for production callable bond OAS when swaption calibration
    /// is not available. Uses the specified (kappa, sigma) directly.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean reversion speed (e.g., 0.03 for 3%)
    /// * `sigma` - Short rate volatility (e.g., 0.01 for 100bp)
    pub fn hull_white(kappa: f64, sigma: f64) -> Self {
        Self {
            tree_steps: 100,
            volatility: sigma,
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: Some(kappa),
            tree_model: TreeModelChoice::HullWhite { kappa, sigma },
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }

    /// Create a configuration using a Hull-White 1-factor tree calibrated
    /// to swaption volatilities from the market context.
    ///
    /// This is the recommended choice for production callable bond OAS.
    ///
    /// # Arguments
    ///
    /// * `swaption_vol_surface_id` - ID of the swaption vol surface in market context
    pub fn hull_white_calibrated(swaption_vol_surface_id: String) -> Self {
        Self {
            tree_steps: 100,
            volatility: 0.01, // placeholder; overridden by calibrated sigma
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: None,
            tree_model: TreeModelChoice::HullWhiteCalibratedToSwaptions {
                swaption_vol_surface_id,
            },
            tree_discount_curve_id: None,
            oas_quote_compounding: OasQuoteCompounding::Continuous,
            oas_price_basis: OasPriceBasis::SettlementDirty,
            tree_compounding: TreeCompounding::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::VolatilityModel;
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;
    use time::macros::date;

    #[test]
    fn black_lognormal_callable_config_uses_simple_tree_compounding() {
        let mut bond = Bond::fixed(
            "BDT-CALLABLE",
            Money::new(1_000.0, Currency::USD),
            0.05,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("fixed bond should build");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: date!(2027 - 01 - 01),
                end_date: date!(2028 - 01 - 01),
                price_pct_of_par: 101.0,
                make_whole: None,
            }],
            puts: vec![],
        });
        bond.pricing_overrides.model_config.vol_model = Some(VolatilityModel::Black);

        let config = bond_tree_config(&bond);

        assert_eq!(config.tree_compounding, TreeCompounding::Simple);
        assert!(matches!(
            config.tree_model,
            TreeModelChoice::BlackDermanToy { .. }
        ));
    }
}
