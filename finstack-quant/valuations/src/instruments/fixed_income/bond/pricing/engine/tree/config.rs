//! Pricing-engine components for fixed-income bonds.
//!
use super::super::super::super::types::Bond;
use crate::instruments::pricing_overrides::{OasPriceBasis, OasQuoteCompounding};
use finstack_quant_core::types::CurveId;
use finstack_quant_models::trees::TreeCompounding;

/// Choice of short-rate model for the bond pricing tree.
///
/// Controls which interest rate tree is used for backward induction. The default
/// `HoLee` model is a simple parallel-shift tree appropriate for quick estimates.
/// For production callable bond OAS, use `HullWhite` with parameters fitted
/// before pricing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
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
/// | Ho-Lee (default) | Normal/Absolute | σ (rate units) | 50-150 bp (0.005-0.015) |
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
/// | Low rates (< 2%) | 50-80 bp | 0.005-0.008 |
/// | Normal rates (2-5%) | 80-120 bp | 0.008-0.012 |
/// | High rates (> 5%) | 100-150 bp | 0.010-0.015 |
/// | Crisis/stress | 150-300 bp | 0.015-0.030 |
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
/// Use `finstack_quant_models::volatility::convert_atm_volatility`:
///
/// ```
/// use finstack_quant_models::volatility::{convert_atm_volatility, VolatilityConvention};
///
/// // Normal vol (100 bp) at 5% rate → lognormal vol (20%)
/// let lognormal = convert_atm_volatility(
///     0.01,
///     VolatilityConvention::Normal,
///     VolatilityConvention::Lognormal,
///     0.05,
///     1.0,
/// )?;
///
/// // Lognormal vol (20%) at 5% rate → normal vol (100 bp)
/// let normal = convert_atm_volatility(
///     0.20,
///     VolatilityConvention::Lognormal,
///     VolatilityConvention::Normal,
///     0.05,
///     1.0,
/// )?;
/// # Ok::<(), finstack_quant_core::Error>(())
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
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricerConfig;
///
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreeModelChoice;
///
/// // Default configuration using Ho-Lee with 100 bp normal vol
/// let default = TreePricerConfig::default();
///
/// // Hull-White model (production recommended for callable bonds)
/// let hw = TreePricerConfig {
///     tree_steps: 100,
///     volatility: 0.01,
///     mean_reversion: Some(0.03),
///     tree_model: TreeModelChoice::HullWhite { kappa: 0.03, sigma: 0.01 },
///     ..Default::default()
/// };
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
    /// - **Ho-Lee (default)**: Normal volatility in rate units (0.01 = 100 bp)
    /// - **BDT**: Lognormal volatility as proportion (0.20 = 20%)
    ///
    /// The default value of 100 bp (0.01) is appropriate for Ho-Lee model
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
    /// Default configuration using Ho-Lee model with 100 bp normal volatility.
    ///
    /// This is appropriate for normal rate environments (2-5% rates).
    /// For low/negative rate environments, consider lower volatility.
    /// For BDT model, set `tree_model` to [`TreeModelChoice::BlackDermanToy`] instead.
    fn default() -> Self {
        Self {
            tree_steps: 200,
            volatility: 0.01, // 100 bp normal vol - appropriate for Ho-Lee
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
/// # Errors
///
/// Returns a validation error when the bond selects the Black-Derman-Toy
/// model (`vol_model = Black` with embedded options) but provides no
/// `implied_volatility`: BDT calibrates to a *lognormal* short-rate vol, and
/// silently defaulting it (the old behavior used 1%) materially misprices the
/// embedded option.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::bond_tree_config;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let bond = Bond::example()?;
/// let config = bond_tree_config(&bond)?;
/// # Ok(())
/// # }
/// ```
pub fn bond_tree_config(bond: &Bond) -> finstack_quant_core::Result<TreePricerConfig> {
    let implied_volatility = bond
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility;
    // Optionality-bearing models below require an explicit vol; a bullet bond on
    // Ho-Lee has no optionality, so the stamped value is inert.
    let volatility = implied_volatility.unwrap_or(0.0);

    let uses_black_lognormal = matches!(
        bond.instrument_pricing_overrides.model_config.vol_model,
        Some(crate::instruments::common_impl::parameters::VolatilityModel::Black)
    );

    // Embedded exercise rights, including a return floor that will be lowered
    // to an issuer call schedule on deterministic paths, select only
    // explicitly parameterized models.
    let tree_model = if bond.call_put.is_some() || bond.return_floor.is_some() {
        if uses_black_lognormal {
            let Some(sigma) = implied_volatility else {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Bond '{}' selects the Black-Derman-Toy tree (vol_model = Black) but \
                     provides no implied_volatility in pricing_overrides.market_quotes. BDT \
                     requires an explicit lognormal short-rate volatility (e.g. 0.20 for 20%).",
                    bond.id.as_str()
                )));
            };
            let mean_reversion = bond
                .instrument_pricing_overrides
                .model_config
                .mean_reversion
                .unwrap_or(0.0);
            TreeModelChoice::BlackDermanToy {
                mean_reversion,
                sigma,
            }
        } else {
            // `hw1f_sigma` is the calibrated-parameter channel for this lattice
            // and takes precedence; `implied_volatility` is the quote channel.
            // Absent both, rates are deterministic (sigma = 0) rather than the
            // invented 100 bp this used to supply, which produced a confident
            // option value out of nothing.
            let sigma = bond
                .instrument_pricing_overrides
                .model_config
                .hw1f_sigma
                .or(implied_volatility)
                .unwrap_or(0.0);
            let mean_reversion = bond
                .instrument_pricing_overrides
                .model_config
                .mean_reversion
                .unwrap_or(0.03);
            TreeModelChoice::HullWhite {
                kappa: mean_reversion,
                sigma,
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

    Ok(TreePricerConfig {
        tree_steps: bond
            .instrument_pricing_overrides
            .model_config
            .tree_steps
            .unwrap_or(100),
        volatility,
        tolerance: 1e-6,
        max_iterations: 50,
        initial_bracket_size_bp: Some(1000.0),
        mean_reversion: bond
            .instrument_pricing_overrides
            .model_config
            .mean_reversion,
        tree_model,
        tree_discount_curve_id: bond
            .instrument_pricing_overrides
            .model_config
            .tree_discount_curve_id
            .clone(),
        oas_quote_compounding: bond
            .instrument_pricing_overrides
            .model_config
            .oas_quote_compounding,
        oas_price_basis: bond
            .instrument_pricing_overrides
            .model_config
            .oas_price_basis,
        tree_compounding,
    })
}

impl TreePricerConfig {
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
    /// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricerConfig;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::VolatilityModel;
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule, ReturnFloorSpec};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn black_lognormal_callable_config_uses_simple_tree_compounding() {
        let mut bond = Bond::fixed(
            "BDT-CALLABLE",
            Money::new(1_000.0, Currency::USD),
            finstack_quant_core::types::Rate::from_decimal(0.05),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
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
        bond.instrument_pricing_overrides.model_config.vol_model = Some(VolatilityModel::Black);
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);

        let config = bond_tree_config(&bond).expect("explicit vol should produce a config");

        assert_eq!(config.tree_compounding, TreeCompounding::Simple);
        assert!(matches!(
            config.tree_model,
            TreeModelChoice::BlackDermanToy { sigma, .. } if (sigma - 0.20).abs() < 1e-12
        ));
    }

    #[test]
    fn black_lognormal_callable_config_errors_without_implied_vol() {
        let mut bond = Bond::fixed(
            "BDT-CALLABLE-NO-VOL",
            Money::new(1_000.0, Currency::USD),
            finstack_quant_core::types::Rate::from_decimal(0.05),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
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
        bond.instrument_pricing_overrides.model_config.vol_model = Some(VolatilityModel::Black);

        let err = bond_tree_config(&bond).expect_err("missing BDT vol must error");
        assert!(err.to_string().contains("implied_volatility"));
    }

    #[test]
    fn black_lognormal_return_floor_config_requires_and_uses_implied_vol() {
        let mut bond = Bond::fixed(
            "BDT-RETURN-FLOOR",
            Money::new(1_000.0, Currency::USD),
            finstack_quant_core::types::Rate::from_decimal(0.05),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("fixed bond should build");
        bond.return_floor = Some(ReturnFloorSpec::moic(1.0));
        bond.instrument_pricing_overrides.model_config.vol_model = Some(VolatilityModel::Black);

        let err = bond_tree_config(&bond).expect_err("floor-only BDT bond needs a volatility");
        assert!(err.to_string().contains("implied_volatility"));

        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);
        let config = bond_tree_config(&bond).expect("floor-only option config");
        assert!(matches!(
            config.tree_model,
            TreeModelChoice::BlackDermanToy { sigma, .. } if (sigma - 0.20).abs() < 1e-12
        ));
    }
}
