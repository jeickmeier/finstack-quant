//! Model selection, tree/lattice settings, and Monte Carlo controls.

use super::check_finite_fields;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::types::CurveId;
use finstack_quant_models::credit::pool::PoolGranularity;

/// Short-rate lattice used by the rates-only bond tree.
///
/// Selected by `instrument_pricing_overrides.model_config.tree_model`; when the
/// field is absent the bond tree uses Hull-White.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ShortRateTreeModel {
    /// Hull-White one-factor (normal short rate) with `(κ, σ)` from
    /// `model_config.hw1f_mean_reversion` / `model_config.hw1f_sigma` or the
    /// pre-fitted market scalars.
    HullWhite,
    /// Black-Derman-Toy (lognormal short rate) with volatility
    /// `model_config.bdt_sigma`.
    BlackDermanToy,
    /// Black-Karasinski (mean-reverting lognormal short rate) with `(κ, σ)`
    /// from `model_config.bk_mean_reversion` / `model_config.bk_sigma`.
    BlackKarasinski,
}

/// Policy for evaluating volatility surfaces outside their calibrated grid.
///
/// Market-standard production systems typically make this choice explicit because
/// extrapolation can materially affect PV and greeks.
///
/// # Market Standards
///
/// - **Error**: Conservative approach for production systems; forces explicit handling.
/// - **Clamp**: Simple flat extrapolation; common for quick prototyping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VolSurfaceExtrapolation {
    /// Fail fast if `(expiry, strike)` is out of bounds.
    #[default]
    Error,
    /// Flat extrapolation to the nearest edge (clamp to grid).
    Clamp,
}

/// Wire path of the OAS quote-compounding field, quoted in errors.
const OAS_QUOTE_COMPOUNDING_PATH: &str =
    "instrument_pricing_overrides.model_config.oas_quote_compounding";

const fn default_oas_quote_compounding() -> Compounding {
    Compounding::Continuous
}

/// Reject OAS quote bases other than continuous and semiannual.
fn validate_oas_quote_compounding(compounding: Compounding) -> finstack_quant_core::Result<()> {
    if compounding == Compounding::Continuous || compounding == Compounding::SEMI_ANNUAL {
        return Ok(());
    }
    Err(finstack_quant_core::Error::Validation(format!(
        "{OAS_QUOTE_COMPOUNDING_PATH} must be \"continuous\" or {{\"periodic\": 2}}, got {compounding}"
    )))
}

/// Model selection and tree pricing parameters.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ModelConfig {
    /// Volatility surface extrapolation policy when `implied_volatility` is not set.
    #[serde(default)]
    pub vol_surface_extrapolation: VolSurfaceExtrapolation,
    /// Short-rate lattice for the rates-only bond tree (`hull_white`,
    /// `black_derman_toy` or `black_karasinski`). `None` selects Hull-White.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_model: Option<ShortRateTreeModel>,
    /// Number of time steps for tree-based pricing (e.g., 100).
    pub tree_steps: Option<usize>,
    /// Merton Monte Carlo configuration for structural credit PIK pricing.
    ///
    /// When set (at `instrument_pricing_overrides.model_config.merton_mc_config`
    /// or via the Rust builder), the `MertonMc` pricer in the registry uses this
    /// model and grid; the path count, antithetic flag and seed label come from
    /// `mc_paths`, `mc_antithetic` and `mc_seed_scenario` on this config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merton_mc_config: Option<crate::instruments::MertonMcConfig>,
    /// Exercise friction cost for issuer/borrower calls, expressed as **cents per 100 of par**.
    ///
    /// This models the real-world costs of refinancing / reissue (fees, OID, documentation),
    /// by requiring the issuer/borrower to see sufficient economic benefit before exercising.
    ///
    /// ## Convention
    /// - `0.0` (or `None`) means frictionless optimal exercise (pure model)
    /// - `50.0` means **$0.50 per $100** of outstanding principal (0.50 points)
    /// - `200.0` means **$2.00 per $100** of outstanding principal (2.00 points)
    ///
    /// The friction affects the **exercise decision threshold**, but redemption still occurs
    /// at the contractual call price.
    pub call_friction_cents: Option<f64>,
    /// Hull-White 1F short-rate absolute volatility override (σ), in annual decimal units.
    ///
    /// This is the **short-rate** σ used directly in the HW1F stochastic differential
    /// equation `dr = [θ(t) − κr] dt + σ dW`. It is **not** an option implied
    /// volatility (Black/Normal) and must not be confused with `implied_volatility`.
    ///
    /// Typical values: 0.005–0.015 (50–150 bp/year annualised short-rate vol).
    /// A value of 0.20 (a typical lognormal swaption vol) would be approximately
    /// 13–40× too large and would produce a wildly mis-priced HW tree.
    ///
    /// This override is valid only with [`Self::hw1f_mean_reversion`]. Pricing
    /// requires a complete, positive, finite parameter pair (or a complete
    /// pre-fitted pair/schedule in the market context), except for the
    /// explicit zero the bond lattices accept (below); partial inputs are
    /// rejected and no volatility surface is queried.
    ///
    /// This is the only short-rate volatility input of both bond lattices:
    /// the rates-only Hull-White tree (callable bond without
    /// `credit_curve_id`) and the **rates-credit** callable path. Neither
    /// reads `implied_volatility`, which is an option quote; the rates-credit
    /// path rejects it outright. On both bond lattices an explicit `0.0`
    /// selects deterministic rates (the rates-only tree still requires a
    /// positive `hw1f_mean_reversion`); it is the only rates-only setting
    /// that prices floating coupons. The rates-credit mean reversion is
    /// additionally capped by
    /// [`KAPPA_MAX`](finstack_quant_models::trees::two_factor_rates_credit::KAPPA_MAX);
    /// Hull-White trees on other paths keep their own wider range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hw1f_sigma: Option<f64>,
    /// Optional piecewise-constant Hull-White short-rate volatility schedule.
    ///
    /// When supplied, this replaces the scalar [`Self::hw1f_sigma`] override.
    /// The schedule is left-continuous, starts at time zero, and carries
    /// absolute annual short-rate volatilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hw1f_sigma_schedule: Option<finstack_quant_core::math::piecewise::PiecewiseConstantCurve>,
    /// Hull-White 1F mean-reversion speed override (κ), in annualised units.
    ///
    /// Companion to [`Self::hw1f_sigma`]. The two values must be supplied
    /// together unless the market context contains a complete pre-fitted
    /// scalar pair or volatility schedule. Typical values: 0.01–0.10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hw1f_mean_reversion: Option<f64>,
    /// Black-Derman-Toy lognormal short-rate volatility (σ), as an annual
    /// decimal proportion of the short rate (`0.20` = 20%).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bdt_sigma: Option<f64>,
    /// Black-Karasinski lognormal short-rate volatility (σ), as an annual
    /// decimal proportion of the short rate (`0.20` = 20%).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bk_sigma: Option<f64>,
    /// Black-Karasinski mean-reversion speed (κ) of the log short rate, in
    /// annualised units (`0.03` = 3% per year).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bk_mean_reversion: Option<f64>,
    /// Pre-calibrated flat LMM forward-rate volatility loading scale.
    ///
    /// This is the positive annualized decimal scale applied to the Bermudan
    /// LMM loading shape. Obtain it from the top-level calibration helper;
    /// pricing never reads or fits a volatility surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lmm_base_vol: Option<f64>,
    /// Credit hazard-rate volatility for the two-factor rates-credit callable
    /// lattice (σ_λ), annualised, in **absolute** decimal hazard-rate points
    /// per √year.
    ///
    /// This is an additive-normal hazard volatility on the same scale as the
    /// hazard rate itself: `0.02` means the instantaneous hazard diffuses by
    /// about 2 percentage points of hazard per √year. It is **not** a relative
    /// or lognormal credit-spread volatility — inserting a CDS-option quote
    /// such as `0.35` here would be roughly an order of magnitude too large.
    /// Convert a fractional spread vol first (see
    /// [`models::credit::market_anchored`](finstack_quant_models::credit::market_anchored)):
    /// `σ_λ = σ_fractional · λ_ref`.
    ///
    /// `None` and `0.0` are equivalent and both mean a **deterministic** credit
    /// factor: the lattice still reprices the survival curve exactly, it just
    /// carries no hazard diffusion. Requires `credit_curve_id` on the
    /// instrument; setting it without one is a validation error rather than a
    /// silent no-op.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hazard_sigma: Option<f64>,
    /// Mean-reversion speed of the hazard factor (κ_λ) on the rates-credit
    /// lattice, annualised.
    ///
    /// `None` and `0.0` both mean no reversion. Capped by
    /// [`KAPPA_MAX`](finstack_quant_models::trees::two_factor_rates_credit::KAPPA_MAX),
    /// above which the binomial lattice's conditional variance collapses far
    /// enough to distort option values. Requires `credit_curve_id`.
    ///
    /// Note that mean reversion narrows the feasible correlation range: it
    /// skews the per-node marginal transition probabilities away from ½, and
    /// two Bernoulli marginals admit only correlations inside their Fréchet
    /// bounds. At `KAPPA_MAX` on both factors over a five-year lattice the
    /// feasible `|ρ|` can fall to around `0.12`. Calibration rejects an
    /// unattainable correlation and reports the lattice-wide maximum, which is
    /// also available up front from
    /// [`RatesCreditTree::max_feasible_correlation`](finstack_quant_models::trees::two_factor_rates_credit::RatesCreditTree::max_feasible_correlation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hazard_mean_reversion: Option<f64>,
    /// Instantaneous correlation between the short-rate and hazard-rate
    /// shocks on the rates-credit lattice, in `[-1, 1]`.
    ///
    /// `None` and `0.0` both mean independent factors. A non-zero value is
    /// only meaningful when **both** factor volatilities are positive;
    /// otherwise it is rejected as an inert input rather than ignored.
    /// Requires `credit_curve_id`.
    ///
    /// Feasibility depends on the mean-reversion settings — see
    /// [`Self::hazard_mean_reversion`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_credit_correlation: Option<f64>,
    /// Optional discount curve identifier for tree-based option/OAS models.
    ///
    /// Some vendor OAS screens use a model curve distinct from the bond's pricing
    /// or spread curve. When set, tree pricers calibrate to this curve while
    /// non-tree spread metrics continue to use the instrument's discount curve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_discount_curve_id: Option<CurveId>,
    /// Optional forward curve identifier for asset-swap spread metrics.
    ///
    /// When set, ASW par/market metrics project the floating receiver leg from
    /// this forward curve instead of using a discount-curve par-rate proxy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asw_forward_curve_id: Option<CurveId>,
    /// Contractual floating index used for asset-swap accruals and historical resets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asw_index_id: Option<finstack_quant_core::types::IndexId>,
    /// Optional government discount curve identifier for the bond `g_spread` metric.
    ///
    /// The metric subtracts this curve's par yield from the bond's yield. It
    /// enters no price, and `g_spread` is an error when it is unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub government_curve_id: Option<CurveId>,
    /// Quote compounding convention for OAS inputs and outputs.
    ///
    /// The OAS is added to each short rate on this basis: a semiannual
    /// (`{"periodic": 2}`) OAS discounts at `1 + (z + oas) / 2` per half
    /// year, the bond-equivalent convention of the Bloomberg OAS screen.
    #[serde(default = "default_oas_quote_compounding")]
    pub oas_quote_compounding: Compounding,
    /// Optional independent-estimator count for Monte Carlo pricers.
    ///
    /// When set, overrides the selected pricer's default simulation size.
    /// Antithetic sampling evaluates two factor paths per independent estimator.
    /// The rates-credit bond engine defaults to 20,000 estimators per training
    /// and pricing stage; other pricers retain their own defaults. Intended for
    /// tests, benchmarks, and controlled revaluation, not as a market quote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_paths: Option<usize>,
    /// Optional antithetic-variates override for Monte Carlo pricing.
    ///
    /// `None` keeps the selected pricer's default. Structured equity pricers
    /// default to `true`; set `Some(false)` only for controlled diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_antithetic: Option<bool>,
    /// Optional Monte Carlo seed label.
    ///
    /// Monte Carlo pricers derive their RNG seed as
    /// `derive_seed(instrument_id, label)`, so the same label always replays
    /// the same random streams for the same instrument. `None` seeds with the
    /// pricer's base label. Finite-difference Greeks set a fixed label on the
    /// repriced clone so base and bumped legs share common random numbers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_seed_scenario: Option<String>,
    /// Optional absolute target for the Monte Carlo confidence-interval
    /// half-width in instrument currency.
    ///
    /// Engines that support adaptive sampling may stop before `mc_paths` after
    /// their minimum sample count when this positive finite target is reached.
    /// The rates-credit bond engine always consumes its fixed estimator budget
    /// and validates this target against the final 95% confidence interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_target_ci_half_width: Option<f64>,
    /// Pool-granularity policy for structured-credit copula default models.
    ///
    /// When set, overrides the default
    /// [`PoolGranularity::PerName`]
    /// finite-pool simulation. Pass
    /// `PoolGranularity::LargeHomogeneous` to opt into the closed-form LHP
    /// fast-path for genuinely granular pools. Ignored by non-copula default
    /// models and by non-structured-credit instruments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_credit_pool_granularity: Option<PoolGranularity>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            vol_surface_extrapolation: VolSurfaceExtrapolation::default(),
            tree_model: None,
            tree_steps: None,
            merton_mc_config: None,
            call_friction_cents: None,
            hw1f_sigma: None,
            hw1f_sigma_schedule: None,
            hw1f_mean_reversion: None,
            bdt_sigma: None,
            bk_sigma: None,
            bk_mean_reversion: None,
            lmm_base_vol: None,
            hazard_sigma: None,
            hazard_mean_reversion: None,
            rate_credit_correlation: None,
            tree_discount_curve_id: None,
            asw_forward_curve_id: None,
            asw_index_id: None,
            government_curve_id: None,
            oas_quote_compounding: default_oas_quote_compounding(),
            mc_paths: None,
            mc_antithetic: None,
            mc_seed_scenario: None,
            mc_target_ci_half_width: None,
            structured_credit_pool_granularity: None,
        }
    }
}

impl ModelConfig {
    /// Return whether every model setting is at its default.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Monte Carlo seed for `instrument_id` under this configuration.
    ///
    /// Derives `derive_seed(instrument_id, mc_seed_scenario)`, with the
    /// `"base"` label when no scenario is set, so every Monte Carlo pricer
    /// replays the same streams for the same instrument and label.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Identifier of the instrument being priced; part of
    ///   the seed so distinct instruments never share random streams.
    pub fn mc_seed(&self, instrument_id: &finstack_quant_core::types::InstrumentId) -> u64 {
        finstack_quant_models::monte_carlo::seed::derive_seed(
            instrument_id,
            self.mc_seed_scenario.as_deref().unwrap_or("base"),
        )
    }

    /// Validate model config (tree steps > 0, non-negative vol/friction).
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if let Some(steps) = self.tree_steps
            && steps == 0
        {
            return Err(finstack_quant_core::Error::Validation(
                "tree_steps must be greater than zero".to_string(),
            ));
        }
        if let Some(paths) = self.mc_paths
            && paths == 0
        {
            return Err(finstack_quant_core::Error::Validation(
                "mc_paths must be greater than zero".to_string(),
            ));
        }
        if let Some(target) = self.mc_target_ci_half_width
            && (!target.is_finite() || target <= 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "mc_target_ci_half_width must be finite and positive, got {target}"
            )));
        }
        check_finite_fields(&[
            (self.call_friction_cents, true),
            (self.hw1f_sigma, true),
            (self.hw1f_mean_reversion, true),
            (self.bdt_sigma, true),
            (self.bk_sigma, true),
            (self.bk_mean_reversion, true),
            (self.hazard_sigma, true),
            (self.hazard_mean_reversion, true),
        ])?;
        if let Some(base_vol) = self.lmm_base_vol
            && (!base_vol.is_finite() || base_vol <= 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "lmm_base_vol must be positive and finite, got {base_vol}"
            )));
        }
        validate_oas_quote_compounding(self.oas_quote_compounding)?;
        if let Some(rho) = self.rate_credit_correlation
            && (!rho.is_finite() || !(-1.0..=1.0).contains(&rho))
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "rate_credit_correlation must be finite and in [-1, 1], got {rho}"
            )));
        }
        Ok(())
    }
}
