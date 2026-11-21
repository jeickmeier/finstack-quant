//! Pricing-engine components for fixed-income bonds.
//!
use crate::bond::Bond;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::pricing::hw1f_params::{
    Hw1fParamFamily, resolve_hw1f_params,
};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;

/// Resolve explicit rates-credit lattice inputs from valuation pricing overrides.
pub(crate) fn resolve_rates_credit_config(
    overrides: &InstrumentPricingOverrides,
    steps: usize,
) -> finstack_quant_core::Result<
    finstack_quant_models::trees::two_factor_rates_credit::RatesCreditConfig,
> {
    use finstack_quant_models::trees::two_factor_rates_credit::{KAPPA_MAX, RatesCreditConfig};

    let model = &overrides.model_config;
    let quotes = &overrides.market_quotes;
    if model.hw1f_sigma_schedule.is_some() {
        return Err(finstack_quant_core::Error::Validation(
            "hw1f_sigma_schedule is not supported on the rates-credit callable path: the \
             two-factor lattice carries a single scalar short-rate volatility. Set \
             hw1f_sigma instead, or price without a credit curve to use a term-structure \
             Hull-White tree."
                .to_string(),
        ));
    }
    if model.hw1f_sigma.is_none() && quotes.implied_volatility.is_some() {
        return Err(finstack_quant_core::Error::Validation(
            "the rates-credit callable path reads short-rate volatility from \
             instrument_pricing_overrides.model_config.hw1f_sigma, not \
             instrument_pricing_overrides.market_quotes.implied_volatility"
                .to_string(),
        ));
    }
    if model.bdt_sigma.is_some() {
        return Err(finstack_quant_core::Error::Validation(
            "instrument_pricing_overrides.model_config.bdt_sigma is a Black-Derman-Toy input; \
             the rates-credit callable path is a Hull-White lattice and reads \
             instrument_pricing_overrides.model_config.hw1f_sigma"
                .to_string(),
        ));
    }
    for (name, value) in [
        ("bk_sigma", model.bk_sigma),
        ("bk_mean_reversion", model.bk_mean_reversion),
    ] {
        if value.is_some() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "instrument_pricing_overrides.model_config.{name} is a Black-Karasinski input; \
                 the rates-credit callable path is a Hull-White lattice and reads \
                 instrument_pricing_overrides.model_config.hw1f_sigma and hw1f_mean_reversion"
            )));
        }
    }

    let rate_vol = model.hw1f_sigma.unwrap_or(0.0);
    let hazard_vol = model.hazard_sigma.unwrap_or(0.0);
    let rate_mean_reversion = model.hw1f_mean_reversion.unwrap_or(0.0);
    let hazard_mean_reversion = model.hazard_mean_reversion.unwrap_or(0.0);
    let correlation = model.rate_credit_correlation.unwrap_or(0.0);

    for (label, value) in [
        ("hw1f_sigma", rate_vol),
        ("hazard_sigma", hazard_vol),
        ("hw1f_mean_reversion", rate_mean_reversion),
        ("hazard_mean_reversion", hazard_mean_reversion),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{label} must be finite and non-negative on the rates-credit path, got {value}"
            )));
        }
    }
    for (label, value) in [
        ("hw1f_mean_reversion", rate_mean_reversion),
        ("hazard_mean_reversion", hazard_mean_reversion),
    ] {
        if value > KAPPA_MAX {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{label} = {value:.4} exceeds the rates-credit binomial-lattice limit \
                 (KAPPA_MAX = {KAPPA_MAX}); reduce it or use HullWhiteTree"
            )));
        }
    }
    if !correlation.is_finite() || !(-1.0..=1.0).contains(&correlation) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "rate_credit_correlation must be finite and in [-1, 1], got {correlation}"
        )));
    }
    if correlation != 0.0 && (rate_vol <= 0.0 || hazard_vol <= 0.0) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "rate_credit_correlation = {correlation} is inert because hw1f_sigma = \
             {rate_vol} and hazard_sigma = {hazard_vol}; set both volatilities \
             positive or leave correlation unset"
        )));
    }

    Ok(RatesCreditConfig {
        steps,
        rate_vol,
        hazard_vol,
        correlation,
        rate_mean_reversion,
        hazard_mean_reversion,
    })
}

/// Choice of short-rate model for the bond pricing tree.
///
/// Controls which interest rate tree is used for backward induction. The
/// private `Default` is `HoLee`, a simple parallel-shift tree appropriate for
/// quick estimates. It does not describe public bond model selection: public
/// bond trees follow `model_config.tree_model`, which defaults to Hull-White
/// with 100 steps and requires fitted or market-resolved parameters.
#[derive(Debug, Clone)]
pub(crate) enum TreeModelChoice {
    /// Ho-Lee model with normal short-rate volatility.
    HoLee {
        /// Annualized absolute rate volatility; `0.01` means 100 basis points.
        sigma: f64,
    },
    /// Hull-White 1-factor with user-specified parameters.
    HullWhite {
        /// Mean reversion speed (e.g., 0.03 for 3%)
        kappa: f64,
        /// Short rate volatility (e.g., 0.01 for 100bp)
        sigma: f64,
    },
    /// Black-Derman-Toy lognormal short-rate model.
    ///
    /// The BDT calibration is binomial and has no mean reversion.
    BlackDermanToy {
        /// Lognormal short-rate volatility (e.g., 0.20 for 20%)
        sigma: f64,
    },
    /// Black-Karasinski mean-reverting lognormal short-rate model.
    ///
    /// Calibrated on a trinomial lattice in `ln r`; as `kappa` tends to zero
    /// its prices approach the Black-Derman-Toy ones.
    BlackKarasinski {
        /// Lognormal short-rate volatility (e.g., 0.20 for 20%)
        sigma: f64,
        /// Mean reversion speed of `ln r` per year (e.g., 0.03 for 3%);
        /// strictly positive
        kappa: f64,
    },
}

impl Default for TreeModelChoice {
    fn default() -> Self {
        Self::HoLee { sigma: 0.01 }
    }
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
/// | Ho-Lee (private default) | Normal/Absolute | σ (rate units) | 50-150 bp (0.005-0.015) |
/// | BDT | Lognormal/Relative | σ (proportion) | 15-30% (0.15-0.30) |
/// | Black-Karasinski | Lognormal/Relative | σ (proportion), κ | 15-30% (0.15-0.30) |
///
/// The private default configuration uses Ho-Lee with **normal volatility**
/// and 200 steps; it is not the public bond selection default.
///
/// ## Volatility Ranges by Model Type
///
/// ### Ho-Lee (Normal Volatility - Private Default)
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
/// | 100 | ~1 bp | Public bond default, most trading |
/// | 200 | < 0.5 bp | Risk reports |
/// | 500 | < 0.2 bp | Regulatory/audit |
///
#[derive(Debug, Clone)]
pub(crate) struct TreePricerConfig {
    /// Number of time steps in the interest rate tree.
    ///
    /// Higher values improve accuracy but increase computation time quadratically.
    /// Recommended: 100 for trading, 200+ for risk reports.
    pub tree_steps: usize,

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

    /// Short-rate model for the pricing tree.
    ///
    /// - `HoLee` (private default): Uses the existing `ShortRateTree` path.
    /// - `HullWhite { kappa, sigma }`: Uses a calibrated HW trinomial tree.
    pub tree_model: TreeModelChoice,

    /// Optional discount curve used only for tree/OAS calibration.
    pub tree_discount_curve_id: Option<CurveId>,
    /// Quote convention used for OAS inputs and outputs.
    pub oas_quote_compounding: Compounding,
    /// Per-node compounding convention for the short-rate tree.
    pub tree_compounding: Compounding,
}

impl Default for TreePricerConfig {
    /// Private default configuration: Ho-Lee with 100 bp normal volatility and
    /// 200 steps. Public bond pricing resolves its own model (Hull-White by
    /// default, 100 steps) and does not use this default.
    ///
    /// This is appropriate for normal rate environments (2-5% rates).
    /// For low/negative rate environments, consider lower volatility.
    /// For BDT model, set `tree_model` to [`TreeModelChoice::BlackDermanToy`] instead.
    fn default() -> Self {
        Self {
            tree_steps: 200,
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            tree_model: TreeModelChoice::default(),
            tree_discount_curve_id: None,
            oas_quote_compounding: Compounding::Continuous,
            tree_compounding: Compounding::Continuous,
        }
    }
}

/// Tree settings shared by both bond lattices, with no short-rate model.
///
/// Carries the step count, OAS conventions and tree discount curve from
/// `bond.instrument_pricing_overrides.model_config`. The model is left at
/// [`TreeModelChoice::HoLee`] with zero volatility: the joint rates-credit
/// lattice takes its factor dynamics from `resolve_rates_credit_config`, and
/// the rates-only tree resolves its model in [`bond_tree_config`].
pub(crate) fn bond_tree_settings(bond: &Bond) -> TreePricerConfig {
    tree_config_with_model(bond, TreeModelChoice::HoLee { sigma: 0.0 })
}

/// Get the rates-only tree pricer configuration for a bond.
///
/// Sources every setting from `bond.instrument_pricing_overrides` so that all
/// tree-based pricing paths (PV, the OAS metric, embedded option value, vega)
/// agree on the model. The short-rate model is explicitly parameterized and
/// does not depend on whether the bond carries exercise rights, so the
/// straight leg of an option decomposition is valued on the same lattice as
/// the optioned bond:
///
/// - `model_config.tree_model = black_derman_toy` selects Black-Derman-Toy with the
///   lognormal volatility `model_config.bdt_sigma`.
/// - `model_config.tree_model = black_karasinski` selects Black-Karasinski with
///   the lognormal volatility `model_config.bk_sigma` and the positive mean
///   reversion `model_config.bk_mean_reversion`; both are required.
/// - Otherwise the Hull-White tree takes `(κ, σ)` from
///   [`resolve_hw1f_params`]: a complete `model_config.hw1f_mean_reversion` /
///   `model_config.hw1f_sigma` pair, or else the complete pre-fitted
///   swaption-family market scalar pair keyed by the tree discount curve.
/// - An explicit `model_config.hw1f_sigma = 0` (with a positive
///   `hw1f_mean_reversion`) selects deterministic rates. This is the only
///   rates-only setting that prices floating coupons.
///
/// `market_quotes.implied_volatility` is an option quote and is never read as
/// a short-rate volatility.
///
/// # Arguments
///
/// * `bond` - Bond whose `instrument_pricing_overrides` supply the model
///   inputs and tree settings.
/// * `market` - Market context searched for pre-fitted Hull-White scalars
///   when the bond carries neither `hw1f_*` override.
///
/// # Returns
///
/// A `TreePricerConfig` for the rates-only tree.
///
/// # Errors
///
/// Returns a validation error when:
/// - BDT is selected but `model_config.bdt_sigma` is missing;
/// - Black-Karasinski is selected but `model_config.bk_sigma` or
///   `model_config.bk_mean_reversion` is missing, or the mean reversion is
///   not finite and strictly positive;
/// - `model_config.bdt_sigma`, `bk_sigma` or `bk_mean_reversion` is set but a
///   different tree model is selected;
/// - the Hull-White parameters are missing, partial, non-positive (other than
///   an explicit zero `hw1f_sigma`) or given as a `hw1f_sigma_schedule`.
///
pub(crate) fn bond_tree_config(
    bond: &Bond,
    market: &MarketContext,
) -> finstack_quant_core::Result<TreePricerConfig> {
    use finstack_quant_valuations_core::instruments::pricing_overrides::ShortRateTreeModel;

    let model = &bond.instrument_pricing_overrides.model_config;
    let selected = model.tree_model.unwrap_or(ShortRateTreeModel::HullWhite);
    let selected_name = match selected {
        ShortRateTreeModel::HullWhite => "hull_white",
        ShortRateTreeModel::BlackDermanToy => "black_derman_toy",
        ShortRateTreeModel::BlackKarasinski => "black_karasinski",
    };
    // A volatility or mean reversion belonging to a model that is not
    // selected would be silently ignored; reject it instead.
    for (field, value, owner, owner_name) in [
        (
            "bdt_sigma",
            model.bdt_sigma,
            ShortRateTreeModel::BlackDermanToy,
            "black_derman_toy",
        ),
        (
            "bk_sigma",
            model.bk_sigma,
            ShortRateTreeModel::BlackKarasinski,
            "black_karasinski",
        ),
        (
            "bk_mean_reversion",
            model.bk_mean_reversion,
            ShortRateTreeModel::BlackKarasinski,
            "black_karasinski",
        ),
    ] {
        if value.is_some() && selected != owner {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Bond '{}' sets instrument_pricing_overrides.model_config.{field} but \
                 selects the {selected_name} tree; set \
                 instrument_pricing_overrides.model_config.tree_model = {owner_name} or \
                 remove {field}",
                bond.id.as_str()
            )));
        }
    }

    let tree_model = match selected {
        ShortRateTreeModel::BlackDermanToy => {
            let Some(sigma) = model.bdt_sigma else {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Bond '{}' selects the Black-Derman-Toy tree \
                     (instrument_pricing_overrides.model_config.tree_model = black_derman_toy) but provides \
                     no instrument_pricing_overrides.model_config.bdt_sigma. BDT requires an \
                     explicit lognormal short-rate volatility (e.g. 0.20 for 20%).",
                    bond.id.as_str()
                )));
            };
            TreeModelChoice::BlackDermanToy { sigma }
        }
        ShortRateTreeModel::BlackKarasinski => {
            let Some(sigma) = model.bk_sigma else {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Bond '{}' selects the Black-Karasinski tree \
                     (instrument_pricing_overrides.model_config.tree_model = black_karasinski) but \
                     provides no instrument_pricing_overrides.model_config.bk_sigma. \
                     Black-Karasinski requires an explicit lognormal short-rate volatility \
                     (e.g. 0.20 for 20%).",
                    bond.id.as_str()
                )));
            };
            match model.bk_mean_reversion {
                Some(kappa) if kappa.is_finite() && kappa > 0.0 => {
                    TreeModelChoice::BlackKarasinski { sigma, kappa }
                }
                kappa => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Bond '{}' selects the Black-Karasinski tree \
                         (instrument_pricing_overrides.model_config.tree_model = black_karasinski) but \
                         instrument_pricing_overrides.model_config.bk_mean_reversion is {kappa:?}; \
                         supply a positive finite mean reversion (e.g. 0.03 for 3% per year), or \
                         select black_derman_toy for a lognormal short rate without mean reversion",
                        bond.id.as_str()
                    )));
                }
            }
        }
        ShortRateTreeModel::HullWhite => {
            if model.hw1f_sigma == Some(0.0) && model.hw1f_sigma_schedule.is_none() {
                deterministic_hull_white(bond)?
            } else {
                let curve_id = model
                    .tree_discount_curve_id
                    .as_ref()
                    .unwrap_or(&bond.discount_curve_id);
                let params = resolve_hw1f_params(
                    Hw1fParamFamily::Swaption,
                    curve_id.as_str(),
                    model,
                    None,
                    &format!("Bond '{}' Hull-White tree", bond.id.as_str()),
                    market,
                )?;
                TreeModelChoice::HullWhite {
                    kappa: params.kappa,
                    sigma: params.sigma,
                }
            }
        }
    };
    Ok(tree_config_with_model(bond, tree_model))
}

/// Hull-White tree with an explicit zero short-rate volatility.
///
/// `model_config.hw1f_sigma = 0` is the deterministic-rates choice on the
/// rates-only tree (the only rates-only setting that supports floating
/// coupons, which that tree preprojects). [`resolve_hw1f_params`] accepts
/// only a positive σ, so this pair is resolved here; the mean reversion is
/// still required, positive and finite, so both Hull-White inputs are always
/// supplied explicitly.
fn deterministic_hull_white(bond: &Bond) -> finstack_quant_core::Result<TreeModelChoice> {
    match bond
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion
    {
        Some(kappa) if kappa.is_finite() && kappa > 0.0 => {
            Ok(TreeModelChoice::HullWhite { kappa, sigma: 0.0 })
        }
        kappa => Err(finstack_quant_core::Error::Validation(format!(
            "Bond '{}' Hull-White tree: instrument_pricing_overrides.model_config.hw1f_sigma = 0 \
             selects deterministic rates but instrument_pricing_overrides.model_config.\
             hw1f_mean_reversion is {kappa:?}; supply a positive finite mean reversion",
            bond.id.as_str()
        ))),
    }
}

fn tree_config_with_model(bond: &Bond, tree_model: TreeModelChoice) -> TreePricerConfig {
    let model = &bond.instrument_pricing_overrides.model_config;
    // Both lognormal lattices use the money-market (simple) per-node
    // convention of the Bloomberg lognormal OAS model, so Black-Karasinski
    // tends to the Black-Derman-Toy price as its mean reversion vanishes.
    let tree_compounding = match &tree_model {
        TreeModelChoice::BlackDermanToy { .. } | TreeModelChoice::BlackKarasinski { .. } => {
            Compounding::Simple
        }
        TreeModelChoice::HoLee { .. } | TreeModelChoice::HullWhite { .. } => {
            Compounding::Continuous
        }
    };
    TreePricerConfig {
        tree_steps: model.tree_steps.unwrap_or(100),
        tolerance: 1e-6,
        max_iterations: 50,
        initial_bracket_size_bp: Some(1000.0),
        tree_model,
        tree_discount_curve_id: model.tree_discount_curve_id.clone(),
        oas_quote_compounding: model.oas_quote_compounding,
        tree_compounding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bond::{CallPut, CallPutSchedule, ReturnFloorSpec};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations_core::instruments::pricing_overrides::ShortRateTreeModel;
    use time::macros::date;

    #[test]
    fn black_lognormal_callable_config_uses_simple_tree_compounding() {
        let mut bond = Bond::fixed(
            "BDT-CALLABLE",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("fixed bond should build");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start: date!(2027 - 01 - 01),
                end: date!(2028 - 01 - 01),
                price_pct_of_par: 101.0,
                make_whole: None,
            }],
            puts: vec![],
        });
        bond.instrument_pricing_overrides.model_config.tree_model =
            Some(ShortRateTreeModel::BlackDermanToy);
        bond.instrument_pricing_overrides.model_config.bdt_sigma = Some(0.20);

        let config = bond_tree_config(&bond, &MarketContext::new())
            .expect("explicit vol should produce a config");

        assert_eq!(config.tree_compounding, Compounding::Simple);
        assert!(matches!(
            config.tree_model,
            TreeModelChoice::BlackDermanToy { sigma, .. } if (sigma - 0.20).abs() < 1e-12
        ));
    }

    fn black_karasinski_bond(model_config: serde_json::Value) -> Bond {
        let mut bond = Bond::fixed(
            "BK-CALLABLE",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("fixed bond should build");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start: date!(2027 - 01 - 01),
                end: date!(2028 - 01 - 01),
                price_pct_of_par: 101.0,
                make_whole: None,
            }],
            puts: vec![],
        });
        bond.instrument_pricing_overrides.model_config =
            serde_json::from_value(model_config).expect("model_config should deserialize");
        bond
    }

    #[test]
    fn black_karasinski_config_reads_bk_sigma_and_mean_reversion() {
        let bond = black_karasinski_bond(serde_json::json!({
            "tree_model": "black_karasinski",
            "bk_sigma": 0.20,
            "bk_mean_reversion": 0.03
        }));
        let config = bond_tree_config(&bond, &MarketContext::new())
            .expect("complete Black-Karasinski inputs should produce a config");

        assert_eq!(config.tree_compounding, Compounding::Simple);
        assert!(matches!(
            config.tree_model,
            TreeModelChoice::BlackKarasinski { sigma, kappa }
                if (sigma - 0.20).abs() < 1e-12 && (kappa - 0.03).abs() < 1e-12
        ));
    }

    #[test]
    fn black_karasinski_config_requires_sigma_and_positive_mean_reversion() {
        for (model_config, expected) in [
            (
                serde_json::json!({ "tree_model": "black_karasinski", "bk_mean_reversion": 0.03 }),
                "no instrument_pricing_overrides.model_config.bk_sigma",
            ),
            (
                serde_json::json!({ "tree_model": "black_karasinski", "bk_sigma": 0.20 }),
                "instrument_pricing_overrides.model_config.bk_mean_reversion is None",
            ),
            (
                serde_json::json!({
                    "tree_model": "black_karasinski",
                    "bk_sigma": 0.20,
                    "bk_mean_reversion": 0.0
                }),
                "instrument_pricing_overrides.model_config.bk_mean_reversion is Some(0.0)",
            ),
        ] {
            let err = bond_tree_config(&black_karasinski_bond(model_config), &MarketContext::new())
                .expect_err("incomplete Black-Karasinski inputs must error");
            assert!(err.to_string().contains(expected), "{err}");
        }
    }

    #[test]
    fn short_rate_inputs_of_an_unselected_model_are_rejected() {
        for (model_config, field, selected) in [
            (
                serde_json::json!({ "bk_sigma": 0.20 }),
                "bk_sigma",
                "hull_white",
            ),
            (
                serde_json::json!({ "tree_model": "hull_white", "bk_mean_reversion": 0.03 }),
                "bk_mean_reversion",
                "hull_white",
            ),
            (
                serde_json::json!({
                    "tree_model": "black_derman_toy",
                    "bdt_sigma": 0.20,
                    "bk_mean_reversion": 0.03
                }),
                "bk_mean_reversion",
                "black_derman_toy",
            ),
            (
                serde_json::json!({
                    "tree_model": "black_karasinski",
                    "bk_sigma": 0.20,
                    "bk_mean_reversion": 0.03,
                    "bdt_sigma": 0.20
                }),
                "bdt_sigma",
                "black_karasinski",
            ),
            (
                serde_json::json!({ "bdt_sigma": 0.20 }),
                "bdt_sigma",
                "hull_white",
            ),
        ] {
            let err = bond_tree_config(&black_karasinski_bond(model_config), &MarketContext::new())
                .expect_err("an input of an unselected model must error");
            let message = err.to_string();
            assert!(
                message.contains(&format!(
                    "sets instrument_pricing_overrides.model_config.{field} but selects the \
                     {selected} tree"
                )),
                "{message}"
            );
        }
    }

    #[test]
    fn black_lognormal_callable_config_errors_without_bdt_sigma() {
        let mut bond = Bond::fixed(
            "BDT-CALLABLE-NO-VOL",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("fixed bond should build");
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start: date!(2027 - 01 - 01),
                end: date!(2028 - 01 - 01),
                price_pct_of_par: 101.0,
                make_whole: None,
            }],
            puts: vec![],
        });
        bond.instrument_pricing_overrides.model_config.tree_model =
            Some(ShortRateTreeModel::BlackDermanToy);

        let err =
            bond_tree_config(&bond, &MarketContext::new()).expect_err("missing BDT vol must error");
        assert!(
            err.to_string()
                .contains("instrument_pricing_overrides.model_config.bdt_sigma")
        );
    }

    #[test]
    fn black_lognormal_return_floor_config_requires_and_uses_bdt_sigma() {
        let mut bond = Bond::fixed(
            "BDT-RETURN-FLOOR",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("fixed bond should build");
        bond.return_floor = Some(ReturnFloorSpec::moic(1.0));
        bond.instrument_pricing_overrides.model_config.tree_model =
            Some(ShortRateTreeModel::BlackDermanToy);

        let err = bond_tree_config(&bond, &MarketContext::new())
            .expect_err("floor-only BDT bond needs a volatility");
        assert!(
            err.to_string()
                .contains("instrument_pricing_overrides.model_config.bdt_sigma")
        );

        bond.instrument_pricing_overrides.model_config.bdt_sigma = Some(0.20);
        let config =
            bond_tree_config(&bond, &MarketContext::new()).expect("floor-only option config");
        assert!(matches!(
            config.tree_model,
            TreeModelChoice::BlackDermanToy { sigma, .. } if (sigma - 0.20).abs() < 1e-12
        ));
    }

    fn rates_credit_overrides(
        rate_vol: Option<f64>,
        hazard_vol: Option<f64>,
        correlation: Option<f64>,
    ) -> InstrumentPricingOverrides {
        let mut overrides = InstrumentPricingOverrides::default();
        overrides.model_config.hw1f_sigma = rate_vol;
        overrides.model_config.hazard_sigma = hazard_vol;
        overrides.model_config.rate_credit_correlation = correlation;
        overrides
    }

    #[test]
    fn resolver_maps_all_four_volatility_regimes() {
        for (rate, hazard, expected_rate, expected_hazard) in [
            (None, None, 0.0, 0.0),
            (Some(0.012), None, 0.012, 0.0),
            (None, Some(0.02), 0.0, 0.02),
            (Some(0.012), Some(0.02), 0.012, 0.02),
        ] {
            let config =
                super::resolve_rates_credit_config(&rates_credit_overrides(rate, hazard, None), 64)
                    .expect("valid rates-credit regime");
            assert_eq!(config.steps, 64);
            assert_eq!(config.rate_vol, expected_rate);
            assert_eq!(config.hazard_vol, expected_hazard);
        }
    }

    #[test]
    fn resolver_rejects_invalid_and_inert_inputs() {
        for (overrides, expected) in [
            (
                rates_credit_overrides(Some(-0.01), None, None),
                "hw1f_sigma",
            ),
            (
                rates_credit_overrides(Some(0.01), Some(f64::NAN), None),
                "hazard_sigma",
            ),
            (
                rates_credit_overrides(Some(0.01), Some(0.02), Some(1.5)),
                "rate_credit_correlation",
            ),
            (rates_credit_overrides(None, Some(0.02), Some(0.5)), "inert"),
        ] {
            let error = super::resolve_rates_credit_config(&overrides, 32)
                .expect_err("invalid rates-credit configuration must fail");
            assert!(error.to_string().contains(expected));
        }

        let mut scheduled = rates_credit_overrides(Some(0.01), None, None);
        scheduled.model_config.hw1f_sigma_schedule = Some(
            finstack_quant_core::math::piecewise::PiecewiseConstantCurve::new(
                vec![0.0, 5.0],
                vec![0.01, 0.012],
            )
            .expect("valid schedule fixture"),
        );
        assert!(
            super::resolve_rates_credit_config(&scheduled, 32)
                .expect_err("unsupported schedule must fail")
                .to_string()
                .contains("hw1f_sigma_schedule")
        );
    }

    #[test]
    fn resolver_rejects_legacy_channels_without_canonical_counterpart() {
        let mut legacy_vol = InstrumentPricingOverrides::default();
        legacy_vol.market_quotes.implied_volatility = Some(0.01);
        assert!(
            super::resolve_rates_credit_config(&legacy_vol, 32)
                .expect_err("legacy vol must fail")
                .to_string()
                .contains("implied_volatility")
        );

        let mut bdt = InstrumentPricingOverrides::default();
        bdt.model_config.bdt_sigma = Some(0.20);
        assert!(
            super::resolve_rates_credit_config(&bdt, 32)
                .expect_err("a BDT volatility is not a rates-credit input")
                .to_string()
                .contains("instrument_pricing_overrides.model_config.bdt_sigma")
        );

        for field in ["bk_sigma", "bk_mean_reversion"] {
            let mut bk = InstrumentPricingOverrides::default();
            match field {
                "bk_sigma" => bk.model_config.bk_sigma = Some(0.20),
                _ => bk.model_config.bk_mean_reversion = Some(0.03),
            }
            assert!(
                super::resolve_rates_credit_config(&bk, 32)
                    .expect_err("a Black-Karasinski input is not a rates-credit input")
                    .to_string()
                    .contains(&format!(
                        "instrument_pricing_overrides.model_config.{field}"
                    ))
            );
        }
    }

    #[test]
    fn canonical_fields_win_when_both_channels_are_set() {
        let mut overrides = InstrumentPricingOverrides::default();
        overrides.market_quotes.implied_volatility = Some(0.35);
        overrides.model_config.hw1f_sigma = Some(0.011);
        overrides.model_config.hw1f_mean_reversion = Some(0.05);
        let config = super::resolve_rates_credit_config(&overrides, 48)
            .expect("canonical channels must take precedence");
        assert_eq!(config.rate_vol, 0.011);
        assert_eq!(config.rate_mean_reversion, 0.05);
    }
}
