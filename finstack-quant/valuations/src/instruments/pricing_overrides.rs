//! Pricing overrides for market-quoted instruments.

use crate::instruments::common_impl::parameters::VolatilityModel;
use crate::instruments::fixed_income::term_loan::TermLoanOverrides;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_models::credit::pool::PoolGranularity;

/// Policy for evaluating volatility surfaces outside their calibrated grid.
///
/// Market-standard production systems typically make this choice explicit because
/// extrapolation can materially affect PV and greeks.
///
/// # Market Standards
///
/// - **Error**: Conservative approach for production systems; forces explicit handling.
/// - **Clamp**: Simple flat extrapolation; common for quick prototyping.
/// - **LinearInVariance**: Market-standard for equity/FX; preserves no-arbitrage conditions
///   better than linear-in-vol by extrapolating in total variance space (σ²T).
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
    /// Linear extrapolation in total variance space (σ²T).
    ///
    /// This is the market-standard approach for equity and FX volatility surfaces
    /// because it preserves the no-arbitrage condition that total variance must
    /// increase with time. The extrapolated volatility is computed as:
    ///
    /// ```text
    /// σ(T_extrap) = sqrt(σ²(T_edge) * T_edge / T_extrap + slope * (T_extrap - T_edge) / T_extrap)
    /// ```
    ///
    /// where `slope` is derived from the variance gradient at the edge.
    ///
    /// # When to Use
    ///
    /// - Long-dated option pricing where expiries exceed the calibrated grid
    /// - Scenario analysis requiring extrapolation to extreme tenors
    /// - Bootstrapping procedures that need consistent variance behavior
    ///
    /// # References
    ///
    /// - Gatheral, J. (2006). *The Volatility Surface*. Chapter 3. `docs/REFERENCES.md#gatheral-volatility-surface`
    /// - Fengler, M. R. (2009). "Arbitrage-free smoothing of the implied volatility surface."
    LinearInVariance,
}

/// Quote convention used when reporting or consuming OAS values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OasQuoteCompounding {
    /// Continuous additive spread, matching the tree's internal short-rate shift.
    #[default]
    Continuous,
    /// Semiannual bond-equivalent OAS quote.
    SemiAnnual,
}

impl OasQuoteCompounding {
    /// Convert an internal continuous spread in decimal form to the quote convention.
    pub(crate) fn quote_from_continuous_decimal(self, spread: f64) -> f64 {
        match self {
            Self::Continuous => spread,
            Self::SemiAnnual => 2.0 * ((spread / 2.0).exp() - 1.0),
        }
    }

    /// Convert a quoted spread in decimal form to the internal continuous convention.
    pub(crate) fn continuous_from_quote_decimal(self, spread: f64) -> f64 {
        match self {
            Self::Continuous => spread,
            Self::SemiAnnual => 2.0 * (1.0 + spread / 2.0).ln(),
        }
    }
}

/// Price/accrual convention used for OAS inversion targets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OasPriceBasis {
    /// Target the full settlement dirty price.
    #[default]
    SettlementDirty,
    /// Target clean price plus only the forward accrued amount from valuation to settlement.
    ForwardAccruedClean,
}

// Shared numeric validation helper

/// Check a batch of optional scalars for finiteness (and optional non-negativity).
///
/// Each entry is `(value, must_be_nonneg)`: an unset `value` is skipped. A
/// `must_be_nonneg = false` field need only be finite (failing with
/// `InputError::Invalid`); a `must_be_nonneg = true` field must be both finite
/// and `>= 0` (failing with `InputError::NegativeValue`). Shared by the numeric
/// `validate()` impls below so the per-field `if let Some` bodies are not repeated.
fn check_finite_fields(fields: &[(Option<f64>, bool)]) -> finstack_quant_core::Result<()> {
    use finstack_quant_core::InputError;
    for &(value, must_be_nonneg) in fields {
        if let Some(v) = value {
            if must_be_nonneg {
                if !(v.is_finite() && v >= 0.0) {
                    return Err(InputError::NegativeValue.into());
                }
            } else if !v.is_finite() {
                return Err(InputError::Invalid.into());
            }
        }
    }
    Ok(())
}

// Sub-struct: Market quote overrides

/// Overrides for market-quoted values (prices, vols, spreads, upfront payments).
///
/// # Price-driving fields
///
/// The following fields, when set, override the model PV returned by
/// [`Instrument::base_value`](crate::instruments::common_impl::traits::Instrument::base_value)
/// for bonds. At most one may be set at a time — [`Self::validate`] enforces this.
/// Precedence (applied top-to-bottom inside `Bond::base_value`):
///
/// 1. `quoted_dirty_price_currency` — currency units (bond native currency)
/// 2. `quoted_clean_price` — percentage of par
/// 3. `quoted_ytm` — decimal YTM (e.g. `0.055` = 5.5%)
/// 4. `quoted_ytw` — decimal yield-to-worst
/// 5. `quoted_z_spread` — decimal Z-spread
/// 6. `quoted_oas` — decimal OAS
/// 7. `quoted_discount_margin` — decimal DM (FRNs)
/// 8. `quoted_i_spread` — decimal I-spread
/// 9. `quoted_asw_market` — decimal ASW (market convention)
/// 10. `quoted_japanese_simple_yield` — decimal Tokyo simple yield (単利)
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct MarketQuoteOverrides {
    /// Quoted clean price as a percentage of par (e.g., `99.5` = 99.5% of par).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_clean_price: Option<f64>,

    /// Quoted dirty price in the bond's currency units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_dirty_price_currency: Option<f64>,

    /// Quoted yield-to-maturity in decimal (e.g., `0.055` = 5.5%).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_ytm: Option<f64>,

    /// Quoted yield-to-worst in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_ytw: Option<f64>,

    /// Quoted Z-spread in decimal (e.g., `0.0125` = 125bp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_z_spread: Option<f64>,

    /// Quoted OAS (option-adjusted spread) in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_oas: Option<f64>,

    /// Quoted discount margin (for FRNs) in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_discount_margin: Option<f64>,

    /// Quoted I-spread in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_i_spread: Option<f64>,

    /// Quoted asset-swap spread (market convention) in decimal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_asw_market: Option<f64>,

    /// Quoted Japanese simple yield (単利) in decimal.
    ///
    /// Seeds a JGB from the Tokyo quoted yield without touching Street
    /// [`Self::quoted_ytm`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_japanese_simple_yield: Option<f64>,

    /// Implied volatility (overrides vol surface). When set on surface-driven
    /// pricers, it is used as a flat σ across tenor and strike.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implied_volatility: Option<f64>,

    /// CDS par-spread quote in basis points (for CDS and CDS index pricers).
    ///
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cds_quote_bp: Option<f64>,

    /// PV adjustment at valuation date (primarily credit-instrument upfront quotes).
    ///
    /// This is an **already-discounted** adjustment to the net present value.
    /// It is added directly to the NPV without further discounting.
    ///
    /// # Sign Convention
    ///
    /// For CDS, CDS index, and CDS tranche instruments, a positive amount is
    /// paid by the protection buyer: it decreases buyer NPV and increases
    /// seller NPV. Other instrument families may treat the amount as an
    /// explicitly signed PV adjustment and document that convention locally.
    ///
    /// # Relationship to CDS Dated Upfront
    ///
    /// For CDS, this is distinct from `CreditDefaultSwap.upfront: Option<(Date, Money)>`:
    /// - **`upfront_payment`**: PV adjustment at `as_of`, added directly
    /// - **`CreditDefaultSwap.upfront`**: Dated cashflow, discounted from payment date
    ///
    /// Both can be set simultaneously without double-counting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upfront_payment: Option<Money>,
}

impl MarketQuoteOverrides {
    /// Return whether no market quote override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Return the number of price-driving fields that are currently set.
    ///
    /// The price-driving fields are mutually exclusive inside `Bond::base_value`
    /// (only the first one in the precedence chain would take effect), so
    /// [`Self::validate`] enforces that at most one is set.
    fn price_driver_count(&self) -> usize {
        [
            self.quoted_clean_price.is_some(),
            self.quoted_dirty_price_currency.is_some(),
            self.quoted_ytm.is_some(),
            self.quoted_ytw.is_some(),
            self.quoted_z_spread.is_some(),
            self.quoted_oas.is_some(),
            self.quoted_discount_margin.is_some(),
            self.quoted_i_spread.is_some(),
            self.quoted_asw_market.is_some(),
            self.quoted_japanese_simple_yield.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count()
    }

    /// Whether any price-driving quote other than `quoted_z_spread` is set.
    ///
    /// Used by scenario spread-shock routing: a shock composes additively with
    /// a quoted Z-spread, but is ambiguous against price-pinning quotes
    /// (clean/dirty price, YTM/YTW, OAS, DM, I-spread, ASW).
    pub(crate) fn has_non_z_price_driver(&self) -> bool {
        self.price_driver_count() > usize::from(self.quoted_z_spread.is_some())
    }

    /// Whether any market quote field should drive bond quote-date economics.
    ///
    /// Bond market quotes are interpreted at the quote date (settlement date
    /// when a settlement convention is present), so accrued interest and
    /// clean/dirty price relationships must use the same date anchor whenever
    /// one of these fields is set.
    pub(crate) fn has_price_driver(&self) -> bool {
        self.price_driver_count() > 0
    }

    /// Validate market quote values for finiteness, non-negativity, and
    /// mutual exclusivity among price-driving fields.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::InputError;

        // Yields, spreads, and clean prices may be negative but all fields
        // must be finite. Dirty bond prices are additionally positive below;
        // implied volatility and CDS spreads are non-negative.
        check_finite_fields(&[
            (self.quoted_clean_price, false),
            (self.quoted_dirty_price_currency, false),
            (self.quoted_ytm, false),
            (self.quoted_ytw, false),
            (self.quoted_z_spread, false),
            (self.quoted_oas, false),
            (self.quoted_discount_margin, false),
            (self.quoted_i_spread, false),
            (self.quoted_asw_market, false),
            (self.quoted_japanese_simple_yield, false),
            (self.implied_volatility, true),
            (self.cds_quote_bp, true),
        ])?;

        if self
            .quoted_dirty_price_currency
            .is_some_and(|price| price <= 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(
                "quoted_dirty_price_currency must be positive".to_string(),
            ));
        }

        // Mutual exclusivity: at most one price-driving field set at a time.
        if self.price_driver_count() > 1 {
            return Err(InputError::Invalid.into());
        }

        Ok(())
    }
}

// Sub-struct: Bump configuration

/// Bump sizes for finite-difference sensitivity calculations.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct BumpConfig {
    /// Rho bump size in **decimal rate** units (default `0.0001 = 1bp`).
    ///
    /// Note: internal curve-bump APIs often take bump sizes in **bp** units (`1.0 = 1bp`).
    /// Prefer using [`MetricPricingOverrides::rho_bump_bp`] when wiring into `BumpSpec::parallel_bp`
    /// or `metrics::bump_discount_curve_parallel` to avoid unit mistakes.
    pub rho_bump_decimal: Option<f64>,
    /// Vega bump size in decimal (default 0.01 = 1%)
    pub vega_bump_decimal: Option<f64>,
    /// Optional YTM bump size for numerical metrics (e.g., convexity/duration), in decimal (1 bp = 1e-4)
    pub ytm_bump_decimal: Option<f64>,
    /// Custom spot bump size override (as percentage, e.g., 0.01 for 1%)
    ///
    /// When set, overrides both standard and adaptive spot bump calculations.
    pub spot_bump_pct: Option<f64>,
    /// Custom volatility bump size override (as absolute vol, e.g., 0.01 for 1% vol)
    ///
    /// When set, overrides both standard and adaptive volatility bump calculations.
    pub vol_bump_pct: Option<f64>,
    /// Custom rate bump size override (in basis points, e.g., 1.0 for 1bp)
    ///
    /// When set, overrides both standard and adaptive rate bump calculations.
    pub rate_bump_bp: Option<f64>,
    /// Custom credit spread bump size override (in basis points, e.g., 1.0 for 1bp).
    ///
    /// Used by CS01 calculations that bump par spreads / hazard calibration quotes.
    pub credit_spread_bump_bp: Option<f64>,
    /// Enable adaptive bump sizes based on volatility and moneyness
    ///
    /// When true, bump sizes are scaled based on:
    /// - Volatility level (higher vol → larger bumps)
    /// - Time to expiry (longer dated → larger bumps)
    /// - Moneyness (deep ITM/OTM → smaller bumps)
    ///
    /// Default: false (use fixed bump sizes)
    pub adaptive_bumps: bool,
}

impl BumpConfig {
    /// Return whether no finite-difference bump override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Validate bump sizes for non-negativity.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Every bump size must be finite and non-negative.
        check_finite_fields(&[
            (self.ytm_bump_decimal, true),
            (self.spot_bump_pct, true),
            (self.vol_bump_pct, true),
            (self.rate_bump_bp, true),
            (self.rho_bump_decimal, true),
            (self.vega_bump_decimal, true),
            (self.credit_spread_bump_bp, true),
        ])
    }
}

// Sub-struct: Model configuration

/// Merton Monte Carlo configuration stored on the bond for registry-based pricing.
///
/// This is a wrapper around
/// [`crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig`]
/// that allows the pricer registry to access the MC configuration from
/// [`InstrumentPricingOverrides`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct MertonMcOverride(
    pub crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig,
);

/// Model selection and tree pricing parameters.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ModelConfig {
    /// Volatility surface extrapolation policy when `implied_volatility` is not set.
    #[serde(default)]
    pub vol_surface_extrapolation: VolSurfaceExtrapolation,
    /// Volatility model choice for option pricing.
    ///
    /// When set, overrides the default Black (lognormal) model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vol_model: Option<VolatilityModel>,
    /// Number of time steps for tree-based pricing (e.g., 100)
    pub tree_steps: Option<usize>,
    /// Merton Monte Carlo configuration for structural credit PIK pricing.
    ///
    /// When set (via flat JSON under `pricing_overrides.merton_mc_config` or the
    /// Rust builder), the `MertonMc` pricer in the registry uses this config.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merton_mc_config: Option<MertonMcOverride>,
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
    /// Mean reversion speed for Hull-White tree model (annualized).
    ///
    /// When set with Ho-Lee model, transforms the tree into Hull-White 1F:
    /// `dr = [theta(t) - a*r] dt + sigma dW`
    ///
    /// Typical values: 0.01-0.10 (1-10% per year). Higher values produce
    /// tighter rate dispersion at long maturities.
    /// When `None` or zero, the tree uses pure Ho-Lee dynamics (no mean reversion).
    pub mean_reversion: Option<f64>,
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
    /// pre-fitted pair/schedule in the market context); partial inputs are
    /// rejected and no volatility surface is queried.
    ///
    /// This is the canonical short-rate volatility field for the
    /// **rates-credit** callable path (`credit_curve_id` set): that path reads
    /// only `hw1f_sigma`/`hw1f_mean_reversion` and rejects the legacy
    /// `implied_volatility`/`mean_reversion` channel rather than silently
    /// reinterpreting it. Its mean reversion is additionally capped by
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
    pub hazard_volatility: Option<f64>,
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
    /// Quote compounding convention for OAS inputs and outputs.
    #[serde(default)]
    pub oas_quote_compounding: OasQuoteCompounding,
    /// Price/accrual target convention for OAS inversion.
    #[serde(default)]
    pub oas_price_basis: OasPriceBasis,
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
    /// Optional absolute target for the Monte Carlo confidence-interval
    /// half-width in instrument currency.
    ///
    /// Engines that support adaptive sampling may stop before `mc_paths` after
    /// their minimum sample count when this positive finite target is reached.
    /// The rates-credit bond engine always consumes its fixed estimator budget
    /// and validates this target against the final 95% confidence interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_target_ci_half_width: Option<f64>,
    /// Apply ISDA half-day accrual-on-default bias.
    ///
    /// Adds half a day of premium accrual in the default-accrual integral.
    /// Used by the CDS option pricer to model the Bloomberg CDSO underlying
    /// convention (and matches QuantLib's `IsdaCdsEngine::HalfDayBias`).
    #[serde(default)]
    pub cds_aod_half_day_bias: bool,
    /// Add one calendar day to *every* Act/360 premium accrual period.
    ///
    /// Used by the CDS option pricer to model the ISDA pre-Big-Bang
    /// option underlying convention (and matches QuantLib's
    /// `Actual360(true)` day-count). The Bloomberg CDSW convention only
    /// treats the *final* coupon period as inclusive of the maturity date,
    /// so this is not the default for production single-name CDS pricing.
    #[serde(default)]
    pub cds_act360_include_last_day: bool,
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

impl ModelConfig {
    /// Return whether every model setting is at its default.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Validate model config (tree steps > 0, non-negative vol/friction).
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::InputError;
        if let Some(steps) = self.tree_steps {
            if steps == 0 {
                return Err(InputError::Invalid.into());
            }
        }
        if let Some(paths) = self.mc_paths {
            if paths == 0 {
                return Err(InputError::Invalid.into());
            }
        }
        if let Some(target) = self.mc_target_ci_half_width {
            if !target.is_finite() || target <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "mc_target_ci_half_width must be finite and positive, got {target}"
                )));
            }
        }
        // Friction, volatilities, and mean reversions must be finite and
        // non-negative. Correlation is signed, so it is range-checked
        // separately.
        check_finite_fields(&[
            (self.call_friction_cents, true),
            (self.mean_reversion, true),
            (self.hw1f_sigma, true),
            (self.hw1f_mean_reversion, true),
            (self.hazard_volatility, true),
            (self.hazard_mean_reversion, true),
        ])?;
        if let Some(base_vol) = self.lmm_base_vol {
            if !base_vol.is_finite() || base_vol <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "lmm_base_vol must be positive and finite, got {base_vol}"
                )));
            }
        }
        if let Some(rho) = self.rate_credit_correlation {
            if !rho.is_finite() || !(-1.0..=1.0).contains(&rho) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "rate_credit_correlation must be finite and in [-1, 1], got {rho}"
                )));
            }
        }
        Ok(())
    }
}

// Sub-struct: Instrument-owned pricing inputs

/// Instrument-owned pricing inputs that can materially change valuation.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct InstrumentPricingOverrides {
    /// Market-quoted values (prices, implied vol, spreads, upfront payments).
    #[serde(default, skip_serializing_if = "MarketQuoteOverrides::is_empty")]
    pub market_quotes: MarketQuoteOverrides,
    /// Model selection and tree pricing parameters.
    #[serde(default, skip_serializing_if = "ModelConfig::is_empty")]
    pub model_config: ModelConfig,
    /// Term loan specific overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term_loan: Option<TermLoanOverrides>,
}

impl InstrumentPricingOverrides {
    /// Return whether no instrument-level override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Create empty instrument-owned pricing inputs.
    pub fn none() -> Self {
        Self::default()
    }

    /// Set quoted clean price as a percentage of par.
    pub fn with_quoted_clean_price(mut self, price_pct: f64) -> Self {
        self.market_quotes.quoted_clean_price = Some(price_pct);
        self
    }

    /// Set quoted dirty price in the instrument currency.
    pub fn with_quoted_dirty_price(mut self, price_currency: f64) -> Self {
        self.market_quotes.quoted_dirty_price_currency = Some(price_currency);
        self
    }

    /// Set quoted yield-to-maturity in decimal form.
    pub fn with_quoted_ytm(mut self, ytm: f64) -> Self {
        self.market_quotes.quoted_ytm = Some(ytm);
        self
    }

    /// Set quoted yield-to-worst in decimal form.
    pub fn with_quoted_ytw(mut self, ytw: f64) -> Self {
        self.market_quotes.quoted_ytw = Some(ytw);
        self
    }

    /// Set quoted Z-spread in decimal form.
    pub fn with_quoted_z_spread(mut self, z_spread: f64) -> Self {
        self.market_quotes.quoted_z_spread = Some(z_spread);
        self
    }

    /// Set quoted OAS in decimal form.
    pub fn with_quoted_oas(mut self, oas: f64) -> Self {
        self.market_quotes.quoted_oas = Some(oas);
        self
    }

    /// Set quoted discount margin in decimal form.
    pub fn with_quoted_discount_margin(mut self, dm: f64) -> Self {
        self.market_quotes.quoted_discount_margin = Some(dm);
        self
    }

    /// Set quoted I-spread in decimal form.
    pub fn with_quoted_i_spread(mut self, i_spread: f64) -> Self {
        self.market_quotes.quoted_i_spread = Some(i_spread);
        self
    }

    /// Set quoted asset-swap spread in decimal form.
    pub fn with_quoted_asw_market(mut self, asw: f64) -> Self {
        self.market_quotes.quoted_asw_market = Some(asw);
        self
    }

    /// Set quoted Japanese simple yield (単利) in decimal form.
    ///
    /// # Arguments
    ///
    /// * `simple_yield` - Tokyo simple yield as a decimal (e.g. `0.02` for 2%).
    pub fn with_quoted_japanese_simple_yield(mut self, simple_yield: f64) -> Self {
        self.market_quotes.quoted_japanese_simple_yield = Some(simple_yield);
        self
    }

    /// Set implied volatility (flat σ across tenor and strike).
    ///
    /// # Arguments
    ///
    /// * `vol` - option-implied volatility as a decimal (e.g. `0.35`); this
    ///   is an option-quote channel, not a short-rate σ — the rates-credit
    ///   callable path rejects it in favour of [`Self::with_hw1f_sigma`]
    pub fn with_implied_vol(mut self, vol: f64) -> Self {
        self.market_quotes.implied_volatility = Some(vol);
        self
    }

    /// Set the Hull-White short-rate volatility σ (annualised, absolute).
    ///
    /// This is the canonical short-rate volatility channel, and the only one
    /// the rates-credit callable path accepts. Prefer it over
    /// [`Self::with_implied_vol`] whenever the intent is a short-rate σ rather
    /// than an option implied volatility.
    ///
    /// # Arguments
    ///
    /// * `sigma` - annualised absolute short-rate volatility (e.g. `0.01` is
    ///   100 bp/yr)
    pub fn with_hw1f_sigma(mut self, sigma: f64) -> Self {
        self.model_config.hw1f_sigma = Some(sigma);
        self
    }

    /// Set the hazard-rate volatility σ_λ for the rates-credit callable
    /// lattice (annualised, absolute decimal hazard points per √year).
    ///
    /// # Arguments
    ///
    /// * `sigma` - annualised absolute hazard volatility (e.g. `0.0105` for
    ///   a 35% fractional vol on a 3% hazard)
    pub fn with_hazard_volatility(mut self, sigma: f64) -> Self {
        self.model_config.hazard_volatility = Some(sigma);
        self
    }

    /// Set the CDS par-spread quote in basis points.
    pub fn with_cds_quote_bp(mut self, spread_bp: f64) -> Self {
        self.market_quotes.cds_quote_bp = Some(spread_bp);
        self
    }

    /// Set the upfront payment used by credit-derivative pricers.
    pub fn with_upfront(mut self, upfront: Money) -> Self {
        self.market_quotes.upfront_payment = Some(upfront);
        self
    }

    /// Set the volatility-surface extrapolation policy.
    pub fn with_vol_surface_extrapolation(mut self, policy: VolSurfaceExtrapolation) -> Self {
        self.model_config.vol_surface_extrapolation = policy;
        self
    }

    /// Use linear-in-variance extrapolation for volatility surfaces.
    pub fn with_linear_in_variance_extrapolation(mut self) -> Self {
        self.model_config.vol_surface_extrapolation = VolSurfaceExtrapolation::LinearInVariance;
        self
    }

    /// Set the number of time steps for tree-based pricing.
    pub fn with_tree_steps(mut self, steps: usize) -> Self {
        self.model_config.tree_steps = Some(steps);
        self
    }

    /// Set the discount curve used by tree-based pricing.
    pub fn with_tree_discount_curve_id(mut self, curve_id: impl Into<CurveId>) -> Self {
        self.model_config.tree_discount_curve_id = Some(curve_id.into());
        self
    }

    /// Set the forward curve used by asset-swap metrics.
    pub fn with_asw_forward_curve_id(mut self, curve_id: impl Into<CurveId>) -> Self {
        self.model_config.asw_forward_curve_id = Some(curve_id.into());
        self
    }

    /// Set issuer/borrower call friction in cents per 100 of par.
    pub fn with_call_friction_cents(mut self, cents: f64) -> Self {
        self.model_config.call_friction_cents = Some(cents);
        self
    }

    /// Set the Merton Monte Carlo configuration.
    pub fn with_merton_mc(
        mut self,
        config: crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig,
    ) -> Self {
        self.model_config.merton_mc_config = Some(MertonMcOverride(config));
        self
    }

    /// Set the independent-estimator count for Monte Carlo pricing.
    ///
    /// Antithetic sampling evaluates two factor paths per estimator. The
    /// rates-credit bond engine uses the full count independently in its policy
    /// training and pricing stages.
    ///
    /// # Arguments
    ///
    /// * `paths` - Positive number of independent Monte Carlo estimators.
    pub fn with_mc_paths(mut self, paths: usize) -> Self {
        self.model_config.mc_paths = Some(paths);
        self
    }

    /// Enable or disable antithetic variates for Monte Carlo pricing.
    ///
    /// # Arguments
    ///
    /// * `enabled` - `true` pairs each estimator with sign-flipped shocks.
    #[must_use]
    pub fn with_mc_antithetic(mut self, enabled: bool) -> Self {
        self.model_config.mc_antithetic = Some(enabled);
        self
    }

    /// Set an absolute Monte Carlo confidence-interval half-width target.
    ///
    /// Rates-credit bond pricing evaluates this requirement only after consuming
    /// the configured fixed estimator budget.
    ///
    /// # Arguments
    ///
    /// * `target` - Positive finite target in the instrument's reporting currency.
    #[must_use]
    pub fn with_mc_target_ci_half_width(mut self, target: f64) -> Self {
        self.model_config.mc_target_ci_half_width = Some(target);
        self
    }

    /// Validate instrument-owned override fields.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.market_quotes.validate()?;
        self.model_config.validate()?;
        Ok(())
    }
}

/// Resolve explicit rates-credit lattice inputs from valuation pricing overrides.
pub(crate) fn resolve_rates_credit_config(
    overrides: &InstrumentPricingOverrides,
    steps: usize,
) -> finstack_quant_core::Result<
    finstack_quant_models::trees::two_factor_rates_credit::RatesCreditConfig,
> {
    use finstack_quant_models::trees::two_factor_rates_credit::{RatesCreditConfig, KAPPA_MAX};

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
             model_config.hw1f_sigma, not market_quotes.implied_volatility"
                .to_string(),
        ));
    }
    if model.hw1f_mean_reversion.is_none() && model.mean_reversion.is_some_and(|value| value != 0.0)
    {
        return Err(finstack_quant_core::Error::Validation(
            "the rates-credit callable path reads mean reversion from \
             model_config.hw1f_mean_reversion, not model_config.mean_reversion"
                .to_string(),
        ));
    }

    let rate_vol = model.hw1f_sigma.unwrap_or(0.0);
    let hazard_vol = model.hazard_volatility.unwrap_or(0.0);
    let rate_mean_reversion = model.hw1f_mean_reversion.unwrap_or(0.0);
    let hazard_mean_reversion = model.hazard_mean_reversion.unwrap_or(0.0);
    let correlation = model.rate_credit_correlation.unwrap_or(0.0);

    for (label, value) in [
        ("hw1f_sigma", rate_vol),
        ("hazard_volatility", hazard_vol),
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
             {rate_vol} and hazard_volatility = {hazard_vol}; set both volatilities \
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

// Sub-struct: Metric configuration

use super::breakeven::BreakevenConfig;

/// Basis used for bond duration, convexity, and DV01-style risk metrics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BondRiskBasis {
    /// Use maturity/workout cashflows under the quoted-yield convention.
    ///
    /// This matches Bloomberg YAS "Workout" risk fields and is the default for
    /// public bond risk metrics.
    #[default]
    BulletDiscountable,
    /// Use callable/putable option model repricing under the bond's OAS/tree configuration.
    CallableOas,
}

/// Metric-time overrides derived from an instrument's pricing metadata.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct MetricPricingOverrides {
    /// Bump sizes for finite-difference sensitivities.
    #[serde(default, skip_serializing_if = "BumpConfig::is_empty")]
    pub bump_config: BumpConfig,
    /// MC seed scenario override for deterministic greek calculations.
    ///
    /// When computing greeks via finite differences, this allows specifying
    /// a scenario name (e.g., "delta_up", "vega_down") to derive deterministic
    /// seeds. If `None`, the pricer derives a stable default seed.
    pub mc_seed_scenario: Option<String>,
    /// Theta period for time decay calculations (e.g., "1D", "1W", "1M", "3M").
    pub theta_period: Option<String>,
    /// Breakeven configuration: which parameter to solve for and solve mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakeven_config: Option<BreakevenConfig>,
    /// Basis used for bond duration, convexity, and DV01-style risk metrics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_risk_basis: Option<BondRiskBasis>,
    /// Historical VaR / Expected Shortfall configuration override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var_config: Option<crate::metrics::risk::VarConfig>,

    /// Externally-quoted price as a percentage of original balance (100.0 = par).
    ///
    /// Structured-credit spread metrics require this external target to avoid
    /// the circular objective `PV(curve + z) == PV(curve)`. They return an error
    /// when it is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quoted_price_pct: Option<f64>,
}

impl MetricPricingOverrides {
    /// Return whether no metric-level override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Validate metric override fields.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::InputError;
        self.bump_config.validate()?;
        if let Some(ref s) = self.theta_period {
            // The downstream consumer (`parse_theta_period`) uppercases the unit
            // suffix before matching, so a lowercase form such as "1d" prices
            // correctly at runtime. Normalize case here too so this JSON-boundary
            // validation does not reject an input the pricer would accept.
            let ok = s.len() >= 2
                && s[..s.len() - 1].chars().all(|c| c.is_ascii_digit())
                && matches!(
                    s.chars().last().map(|c| c.to_ascii_uppercase()),
                    Some('D' | 'W' | 'M' | 'Y')
                );
            if !ok {
                return Err(InputError::Invalid.into());
            }
        }
        if let Some(var_config) = &self.var_config {
            var_config.validate()?;
        }
        Ok(())
    }

    /// Rho bump size in basis points for curve-bump APIs.
    pub fn rho_bump_bp(&self) -> f64 {
        self.bump_config.rho_bump_decimal.unwrap_or(0.0001) * 10_000.0
    }

    /// Bond risk basis, defaulting to Bloomberg-style workout/bullet risk.
    pub fn bond_risk_basis_or_default(&self) -> BondRiskBasis {
        self.bond_risk_basis.unwrap_or_default()
    }

    /// Set custom spot bump size (as percentage, e.g., 0.01 for 1%).
    pub fn with_spot_bump(mut self, bump_pct: f64) -> Self {
        self.bump_config.spot_bump_pct = Some(bump_pct);
        self
    }

    /// Set custom volatility bump size (as absolute vol, e.g., 0.01 for 1% vol).
    pub fn with_vol_bump(mut self, bump_pct: f64) -> Self {
        self.bump_config.vol_bump_pct = Some(bump_pct);
        self
    }

    /// Set custom rate bump size (in basis points, e.g., 1.0 for 1bp).
    pub fn with_rate_bump(mut self, bump_bp: f64) -> Self {
        self.bump_config.rate_bump_bp = Some(bump_bp);
        self
    }

    /// Set custom credit spread bump size (in basis points, e.g., 1.0 for 1bp).
    pub fn with_credit_spread_bump(mut self, bump_bp: f64) -> Self {
        self.bump_config.credit_spread_bump_bp = Some(bump_bp);
        self
    }

    /// Set custom YTM bump size in decimal form. For one basis point, pass `1e-4`.
    pub fn with_ytm_bump_decimal(mut self, bump: f64) -> Self {
        self.bump_config.ytm_bump_decimal = Some(bump);
        self
    }

    /// Enable or disable adaptive bump sizes for Greek calculations.
    pub fn with_adaptive_bumps(mut self, enable: bool) -> Self {
        self.bump_config.adaptive_bumps = enable;
        self
    }

    /// Set theta period for time decay calculations.
    pub fn with_theta_period(mut self, period: impl Into<String>) -> Self {
        self.theta_period = Some(period.into());
        self
    }

    /// Set breakeven configuration.
    pub fn with_breakeven_config(mut self, config: BreakevenConfig) -> Self {
        self.breakeven_config = Some(config);
        self
    }

    /// Set MC seed scenario for deterministic greek calculations.
    pub fn with_mc_seed_scenario(mut self, scenario: impl Into<String>) -> Self {
        self.mc_seed_scenario = Some(scenario.into());
        self
    }

    /// Set bond risk basis for duration, convexity, and DV01-style metrics.
    pub fn with_bond_risk_basis(mut self, basis: BondRiskBasis) -> Self {
        self.bond_risk_basis = Some(basis);
        self
    }

    /// Set Historical VaR / Expected Shortfall configuration.
    pub fn with_var_config(mut self, config: crate::metrics::risk::VarConfig) -> Self {
        self.var_config = Some(config);
        self
    }
}

// Sub-struct: Scenario adjustments

/// Scenario-only valuation adjustments.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ScenarioPricingOverrides {
    /// Scenario price shock as decimal percentage (e.g., -0.05 for -5% price shock).
    ///
    /// When set, valuation helpers apply it as a multiplier: `price * (1 + shock_pct)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_price_shock_pct: Option<f64>,

    /// Scenario spread shock in basis points (e.g., `150.0` for +150 bp widening).
    ///
    /// Applied as an additional flat Z-spread during valuation by pricers that
    /// support spread-based revaluation. Currently consumed by `Bond::base_value`
    /// for bonds without embedded options, without an assigned credit curve, and
    /// without a price-pinning quote override other than `quoted_z_spread`
    /// (where the shock is additive on the quoted spread). See
    /// [`Instrument::scenario_spread_shock_supported`](crate::instruments::common_impl::traits::Instrument::scenario_spread_shock_supported).
    ///
    /// Setting this on an unsupported configuration produces a validation error
    /// at pricing time rather than a silent no-op. For hazard-priced (credit
    /// curve) bonds, shock the hazard curve instead (e.g. a par-CDS curve bump).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_spread_shock_bp: Option<f64>,
}

impl ScenarioPricingOverrides {
    /// Return whether no scenario-level override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Apply a scenario price shock as a decimal percentage.
    pub fn with_price_shock_pct(mut self, shock_pct: f64) -> Self {
        self.scenario_price_shock_pct = Some(shock_pct);
        self
    }

    /// Apply a scenario spread shock in basis points.
    pub fn with_spread_shock_bp(mut self, shock_bp: f64) -> Self {
        self.scenario_spread_shock_bp = Some(shock_bp);
        self
    }

    /// Clear all scenario shocks.
    pub fn clear_scenario_shocks(&mut self) {
        self.scenario_price_shock_pct = None;
        self.scenario_spread_shock_bp = None;
    }

    /// Return whether any scenario shock is configured.
    pub fn has_scenario_shock(&self) -> bool {
        self.scenario_price_shock_pct.is_some() || self.scenario_spread_shock_bp.is_some()
    }

    /// Validate scenario shocks for finiteness.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        // Shocks may be negative (downside / tightening scenarios) but must be finite.
        check_finite_fields(&[
            (self.scenario_price_shock_pct, false),
            (self.scenario_spread_shock_bp, false),
        ])
    }

    /// Apply the configured price shock to a present value.
    pub fn apply_to_value(&self, value: Money) -> Money {
        let Some(shock) = self.scenario_price_shock_pct else {
            return value;
        };
        Money::new(value.amount() * (1.0 + shock), value.currency())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rates_credit_overrides(
        rate_vol: Option<f64>,
        hazard_vol: Option<f64>,
        correlation: Option<f64>,
    ) -> InstrumentPricingOverrides {
        let mut overrides = InstrumentPricingOverrides::default();
        overrides.model_config.hw1f_sigma = rate_vol;
        overrides.model_config.hazard_volatility = hazard_vol;
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
                resolve_rates_credit_config(&rates_credit_overrides(rate, hazard, None), 64)
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
                "hazard_volatility",
            ),
            (
                rates_credit_overrides(Some(0.01), Some(0.02), Some(1.5)),
                "rate_credit_correlation",
            ),
            (rates_credit_overrides(None, Some(0.02), Some(0.5)), "inert"),
        ] {
            let error = resolve_rates_credit_config(&overrides, 32)
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
        assert!(resolve_rates_credit_config(&scheduled, 32)
            .expect_err("unsupported schedule must fail")
            .to_string()
            .contains("hw1f_sigma_schedule"));
    }

    #[test]
    fn resolver_rejects_legacy_channels_without_canonical_counterpart() {
        let mut legacy_vol = InstrumentPricingOverrides::default();
        legacy_vol.market_quotes.implied_volatility = Some(0.01);
        assert!(resolve_rates_credit_config(&legacy_vol, 32)
            .expect_err("legacy vol must fail")
            .to_string()
            .contains("implied_volatility"));

        let mut legacy_reversion = InstrumentPricingOverrides::default();
        legacy_reversion.model_config.mean_reversion = Some(0.03);
        assert!(resolve_rates_credit_config(&legacy_reversion, 32)
            .expect_err("legacy reversion must fail")
            .to_string()
            .contains("hw1f_mean_reversion"));
    }

    #[test]
    fn canonical_fields_win_when_both_channels_are_set() {
        let mut overrides = InstrumentPricingOverrides::default();
        overrides.market_quotes.implied_volatility = Some(0.35);
        overrides.model_config.mean_reversion = Some(0.09);
        overrides.model_config.hw1f_sigma = Some(0.011);
        overrides.model_config.hw1f_mean_reversion = Some(0.05);
        let config = resolve_rates_credit_config(&overrides, 48)
            .expect("canonical channels must take precedence");
        assert_eq!(config.rate_vol, 0.011);
        assert_eq!(config.rate_mean_reversion, 0.05);
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    #[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
    #[serde(deny_unknown_fields)]
    struct FocusedWireFixture {
        id: String,
        #[serde(default, skip_serializing_if = "InstrumentPricingOverrides::is_empty")]
        instrument_pricing_overrides: InstrumentPricingOverrides,
        #[serde(default, skip_serializing_if = "MetricPricingOverrides::is_empty")]
        metric_pricing_overrides: MetricPricingOverrides,
        #[serde(default, skip_serializing_if = "ScenarioPricingOverrides::is_empty")]
        scenario_pricing_overrides: ScenarioPricingOverrides,
    }

    #[test]
    fn focused_categories_validate_independently() {
        let instrument = InstrumentPricingOverrides::default().with_quoted_clean_price(100.0);
        let metrics = MetricPricingOverrides::default()
            .with_ytm_bump_decimal(1e-4)
            .with_spot_bump(0.01)
            .with_vol_bump(0.01)
            .with_rate_bump(1.0);
        let scenario = ScenarioPricingOverrides::default().with_price_shock_pct(-0.05);

        assert!(instrument.validate().is_ok());
        assert!(metrics.validate().is_ok());
        assert!(scenario.validate().is_ok());
        assert!(MetricPricingOverrides::default()
            .with_vol_bump(-0.01)
            .validate()
            .is_err());
    }

    #[test]
    fn instrument_vol_surface_extrapolation_builders_roundtrip() {
        for policy in [
            VolSurfaceExtrapolation::Error,
            VolSurfaceExtrapolation::Clamp,
            VolSurfaceExtrapolation::LinearInVariance,
        ] {
            let overrides =
                InstrumentPricingOverrides::default().with_vol_surface_extrapolation(policy);
            let json = serde_json::to_string(&overrides).expect("serialize");
            let roundtrip: InstrumentPricingOverrides =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(roundtrip.model_config.vol_surface_extrapolation, policy);
        }

        let overrides =
            InstrumentPricingOverrides::default().with_linear_in_variance_extrapolation();
        assert_eq!(
            overrides.model_config.vol_surface_extrapolation,
            VolSurfaceExtrapolation::LinearInVariance
        );
    }

    #[test]
    fn focused_wire_uses_only_three_closed_canonical_fields() {
        let fixture = FocusedWireFixture {
            id: "fixture".to_string(),
            instrument_pricing_overrides: InstrumentPricingOverrides::default()
                .with_quoted_clean_price(99.5),
            metric_pricing_overrides: MetricPricingOverrides::default().with_theta_period("1W"),
            scenario_pricing_overrides: ScenarioPricingOverrides::default()
                .with_price_shock_pct(-0.05),
        };

        let value = serde_json::to_value(&fixture).expect("serialize focused fixture");
        let object = value.as_object().expect("fixture object");
        assert!(object.contains_key("instrument_pricing_overrides"));
        assert!(object.contains_key("metric_pricing_overrides"));
        assert!(object.contains_key("scenario_pricing_overrides"));
        assert!(!object.contains_key("pricing_overrides"));

        let roundtrip: FocusedWireFixture =
            serde_json::from_value(value).expect("deserialize focused fixture");
        assert_eq!(
            roundtrip
                .instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price,
            Some(99.5)
        );

        for field in [
            "instrument_pricing_overrides",
            "metric_pricing_overrides",
            "scenario_pricing_overrides",
        ] {
            let mut invalid = serde_json::json!({"id": "fixture"});
            invalid[field] = serde_json::Value::Null;
            assert!(serde_json::from_value::<FocusedWireFixture>(invalid).is_err());
        }
        assert!(
            serde_json::from_value::<FocusedWireFixture>(serde_json::json!({
                "id": "fixture",
                "pricing_overrides": {}
            }))
            .is_err()
        );

        let schema = serde_json::to_value(schemars::schema_for!(FocusedWireFixture))
            .expect("serialize schema");
        let properties = schema["properties"].as_object().expect("schema properties");
        assert!(properties.contains_key("instrument_pricing_overrides"));
        assert!(properties.contains_key("metric_pricing_overrides"));
        assert!(properties.contains_key("scenario_pricing_overrides"));
        assert!(!properties.contains_key("pricing_overrides"));
    }

    #[test]
    fn theta_period_validation_is_case_insensitive_but_strict() {
        for period in ["1d", "1D", "2w", "3M", "1y", "10Y", "12m"] {
            assert!(MetricPricingOverrides::default()
                .with_theta_period(period)
                .validate()
                .is_ok());
        }
        for period in ["1x", "D", "abc", "1", "1.5d", "-1d", ""] {
            assert!(MetricPricingOverrides::default()
                .with_theta_period(period)
                .validate()
                .is_err());
        }
    }

    #[test]
    fn monte_carlo_accuracy_controls_validate() {
        let controls = InstrumentPricingOverrides::default()
            .with_mc_paths(10_000)
            .with_mc_antithetic(false)
            .with_mc_target_ci_half_width(1.25);
        assert!(controls.validate().is_ok());
        assert_eq!(controls.model_config.mc_antithetic, Some(false));
        assert_eq!(controls.model_config.mc_target_ci_half_width, Some(1.25));

        for target in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(InstrumentPricingOverrides::default()
                .with_mc_target_ci_half_width(target)
                .validate()
                .is_err());
        }
    }
}
