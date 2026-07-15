//! Pricing overrides for market-quoted instruments.

use crate::instruments::common_impl::parameters::{SABRParameters, VolatilityModel};
use crate::instruments::fixed_income::term_loan::TermLoanOverrides;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;

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
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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
    /// - Gatheral, J. (2006). *The Volatility Surface*. Chapter 3.
    /// - Fengler, M. R. (2009). "Arbitrage-free smoothing of the implied volatility surface."
    LinearInVariance,
}

/// Quote convention used when reporting or consuming OAS values.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OasPriceBasis {
    /// Target the full settlement dirty price.
    #[default]
    SettlementDirty,
    /// Target clean price plus only the forward accrued amount from valuation to settlement.
    ForwardAccruedClean,
}

// ---------------------------------------------------------------------------
// Shared numeric validation helper
// ---------------------------------------------------------------------------

/// Check a batch of optional scalars for finiteness (and optional non-negativity).
///
/// Each entry is `(value, must_be_nonneg)`: an unset `value` is skipped. A
/// `must_be_nonneg = false` field need only be finite (failing with
/// [`InputError::Invalid`]); a `must_be_nonneg = true` field must be both finite
/// and `>= 0` (failing with [`InputError::NegativeValue`]). Shared by the numeric
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

// ---------------------------------------------------------------------------
// Sub-struct: Market quote overrides
// ---------------------------------------------------------------------------

/// Overrides for market-quoted values (prices, vols, spreads, upfront payments).
///
/// # Price-driving fields
///
/// The following fields, when set, override the model PV returned by
/// [`Instrument::base_value`](crate::instruments::common_impl::traits::Instrument::base_value)
/// for bonds. At most one may be set at a time — [`Self::validate`] enforces this.
/// Precedence (applied top-to-bottom inside `Bond::base_value`):
///
/// 1. `quoted_dirty_price_ccy` — currency units (bond native currency)
/// 2. `quoted_clean_price` — percentage of par
/// 3. `quoted_ytm` — decimal YTM (e.g. `0.055` = 5.5%)
/// 4. `quoted_ytw` — decimal yield-to-worst
/// 5. `quoted_z_spread` — decimal Z-spread
/// 6. `quoted_oas` — decimal OAS
/// 7. `quoted_discount_margin` — decimal DM (FRNs)
/// 8. `quoted_i_spread` — decimal I-spread
/// 9. `quoted_asw_market` — decimal ASW (market convention)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct MarketQuoteOverrides {
    /// Quoted clean price as a percentage of par (e.g., `99.5` = 99.5% of par).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_clean_price: Option<f64>,

    /// Quoted dirty price in the bond's currency units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_dirty_price_ccy: Option<f64>,

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

    /// Implied volatility (overrides vol surface). When set on surface-driven
    /// pricers, it is used as a flat σ across tenor and strike.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implied_volatility: Option<f64>,

    /// CDS par-spread quote in basis points (for CDS and CDS index pricers).
    ///
    /// Renamed from `quoted_spread_bp`; the legacy JSON field name is still
    /// accepted as a serde alias for backward compatibility.
    #[serde(alias = "quoted_spread_bp", skip_serializing_if = "Option::is_none")]
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
    /// Return the number of price-driving fields that are currently set.
    ///
    /// The price-driving fields are mutually exclusive inside `Bond::base_value`
    /// (only the first one in the precedence chain would take effect), so
    /// [`Self::validate`] enforces that at most one is set.
    fn price_driver_count(&self) -> usize {
        [
            self.quoted_clean_price.is_some(),
            self.quoted_dirty_price_ccy.is_some(),
            self.quoted_ytm.is_some(),
            self.quoted_ytw.is_some(),
            self.quoted_z_spread.is_some(),
            self.quoted_oas.is_some(),
            self.quoted_discount_margin.is_some(),
            self.quoted_i_spread.is_some(),
            self.quoted_asw_market.is_some(),
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

        // Prices, spreads and yields may be negative (e.g. deep-distress) but
        // must be finite; implied vol and CDS spreads must be non-negative.
        check_finite_fields(&[
            (self.quoted_clean_price, false),
            (self.quoted_dirty_price_ccy, false),
            (self.quoted_ytm, false),
            (self.quoted_ytw, false),
            (self.quoted_z_spread, false),
            (self.quoted_oas, false),
            (self.quoted_discount_margin, false),
            (self.quoted_i_spread, false),
            (self.quoted_asw_market, false),
            (self.implied_volatility, true),
            (self.cds_quote_bp, true),
        ])?;

        // Mutual exclusivity: at most one price-driving field set at a time.
        if self.price_driver_count() > 1 {
            return Err(InputError::Invalid.into());
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Sub-struct: Bump configuration
// ---------------------------------------------------------------------------

/// Bump sizes for finite-difference sensitivity calculations.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct BumpConfig {
    /// Rho bump size in **decimal rate** units (default `0.0001 = 1bp`).
    ///
    /// Note: internal curve-bump APIs often take bump sizes in **bp** units (`1.0 = 1bp`).
    /// Prefer using [`PricingOverrides::rho_bump_bp`] when wiring into `BumpSpec::parallel_bp`
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

// ---------------------------------------------------------------------------
// Sub-struct: Model configuration
// ---------------------------------------------------------------------------

/// Merton Monte Carlo configuration stored on the bond for registry-based pricing.
///
/// This is a wrapper around
/// [`crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig`]
/// that allows the pricer registry to access the MC configuration from
/// `PricingOverrides`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct MertonMcOverride(
    pub crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig,
);

/// Model selection and tree pricing parameters.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct ModelConfig {
    /// Volatility surface extrapolation policy when `implied_volatility` is not set.
    #[serde(default)]
    pub vol_surface_extrapolation: VolSurfaceExtrapolation,
    /// Volatility model choice for option pricing.
    ///
    /// When set, overrides the default Black (lognormal) model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vol_model: Option<VolatilityModel>,
    /// Optional SABR volatility model parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sabr_params: Option<SABRParameters>,
    /// Number of time steps for tree-based pricing (e.g., 100)
    pub tree_steps: Option<usize>,
    /// Use Gobet-Miri discrete monitoring correction for barrier options.
    ///
    /// When true, uses a Monte Carlo correction for discrete monitoring.
    /// When false, uses analytical continuous monitoring pricing.
    #[serde(default)]
    pub use_gobet_miri: bool,
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
    /// A σ override must be set together with [`Self::hw1f_mean_reversion`].
    /// A κ-only cap/floor override is valid when a normal-vol surface is
    /// available: κ is held fixed while σ is calibrated from market quotes.
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
    /// Companion to [`Self::hw1f_sigma`]. Both values define an explicit HW1F
    /// parameter pair; cap/floor pricing may also hold κ fixed and calibrate σ
    /// from a normal-vol surface. Typical values: 0.01–0.10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hw1f_mean_reversion: Option<f64>,
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
    /// Optional Monte Carlo path count for path-dependent GBM pricers (Asians, lookbacks, autocallables, etc.).
    ///
    /// When set, overrides the default simulation size (typically 100,000 paths). Intended for tests,
    /// benchmarks, and controlled revaluation—not a market quote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_paths: Option<usize>,
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
    /// [`PoolGranularity::PerName`](crate::instruments::fixed_income::structured_credit::PoolGranularity)
    /// finite-pool simulation. Pass
    /// `PoolGranularity::LargeHomogeneous` to opt into the closed-form LHP
    /// fast-path for genuinely granular pools. Ignored by non-copula default
    /// models and by non-structured-credit instruments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_credit_pool_granularity:
        Option<crate::instruments::fixed_income::structured_credit::PoolGranularity>,
}

impl ModelConfig {
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
        // Friction and mean reversion must be finite and non-negative.
        check_finite_fields(&[
            (self.call_friction_cents, true),
            (self.mean_reversion, true),
            (self.hw1f_sigma, true),
            (self.hw1f_mean_reversion, true),
        ])
    }
}

// ---------------------------------------------------------------------------
// Sub-struct: Instrument-owned pricing inputs
// ---------------------------------------------------------------------------

/// Instrument-owned pricing inputs that can materially change valuation.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct InstrumentPricingOverrides {
    /// Market-quoted values (prices, implied vol, spreads, upfront payments).
    #[serde(flatten)]
    pub market_quotes: MarketQuoteOverrides,
    /// Model selection and tree pricing parameters.
    #[serde(flatten)]
    pub model_config: ModelConfig,
    /// Term loan specific overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term_loan: Option<TermLoanOverrides>,
}

impl InstrumentPricingOverrides {
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
    pub fn with_quoted_dirty_price(mut self, price_ccy: f64) -> Self {
        self.market_quotes.quoted_dirty_price_ccy = Some(price_ccy);
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

    /// Set implied volatility (flat σ across tenor and strike).
    pub fn with_implied_vol(mut self, vol: f64) -> Self {
        self.market_quotes.implied_volatility = Some(vol);
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

    /// Set the path count for path-dependent Monte Carlo pricing.
    pub fn with_mc_paths(mut self, paths: usize) -> Self {
        self.model_config.mc_paths = Some(paths);
        self
    }

    /// Validate instrument-owned override fields.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.market_quotes.validate()?;
        self.model_config.validate()?;
        Ok(())
    }
}

impl From<&PricingOverrides> for InstrumentPricingOverrides {
    fn from(pricing_overrides: &PricingOverrides) -> Self {
        pricing_overrides.instrument.clone()
    }
}

// ---------------------------------------------------------------------------
// Sub-struct: Metric configuration
// ---------------------------------------------------------------------------

// Breakeven types live in the breakeven calculator module; re-exported here
// for backward compatibility (they ship as part of the overrides public API).
pub use crate::metrics::sensitivities::breakeven::{
    BreakevenConfig, BreakevenMode, BreakevenTarget,
};

/// Basis used for bond duration, convexity, and DV01-style risk metrics.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct MetricPricingOverrides {
    /// Bump sizes for finite-difference sensitivities.
    #[serde(flatten)]
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
}

impl MetricPricingOverrides {
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

impl From<&PricingOverrides> for MetricPricingOverrides {
    fn from(pricing_overrides: &PricingOverrides) -> Self {
        pricing_overrides.metrics.clone()
    }
}

// ---------------------------------------------------------------------------
// Sub-struct: Scenario adjustments
// ---------------------------------------------------------------------------

/// Scenario-only valuation adjustments.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
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

impl From<&PricingOverrides> for ScenarioPricingOverrides {
    fn from(pricing_overrides: &PricingOverrides) -> Self {
        pricing_overrides.scenario.clone()
    }
}

// ---------------------------------------------------------------------------
// Stable wire representation
// ---------------------------------------------------------------------------

/// Stable legacy wire representation used while runtime storage is split by owner.
///
/// Serialization stays flat. Deserialization additionally accepts focused
/// nested objects named `instrument`, `metrics`, or `scenario` (and their
/// explicit `*_pricing_overrides` aliases), merging them over flat fields.
#[derive(Debug, Clone, Default, serde::Serialize, schemars::JsonSchema)]
#[serde(default)]
pub(crate) struct PricingOverridesWire {
    #[serde(flatten)]
    pub(crate) instrument: InstrumentPricingOverrides,
    #[serde(flatten)]
    pub(crate) metrics: MetricPricingOverrides,
    #[serde(flatten)]
    pub(crate) scenario: ScenarioPricingOverrides,
}

impl<'de> serde::Deserialize<'de> for PricingOverridesWire {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| serde::de::Error::custom("pricing overrides must be a JSON object"))?;
        if object.contains_key("tree_volatility") {
            return Err(serde::de::Error::custom(
                "`tree_volatility` was removed; use `implied_volatility`",
            ));
        }

        for key in [
            "instrument",
            "instrument_pricing_overrides",
            "metrics",
            "metric_pricing_overrides",
            "scenario",
            "scenario_pricing_overrides",
        ] {
            let Some(nested) = object.remove(key) else {
                continue;
            };
            if nested.is_null() {
                continue;
            }
            let nested = nested.as_object().ok_or_else(|| {
                serde::de::Error::custom(format!("`{key}` must be a JSON object"))
            })?;
            for (field, value) in nested {
                object.insert(field.clone(), value.clone());
            }
        }
        if object.contains_key("tree_volatility") {
            return Err(serde::de::Error::custom(
                "`tree_volatility` was removed; use `implied_volatility`",
            ));
        }

        #[derive(Default, serde::Deserialize)]
        #[serde(default)]
        struct FlatWire {
            #[serde(flatten)]
            instrument: InstrumentPricingOverrides,
            #[serde(flatten)]
            metrics: MetricPricingOverrides,
            #[serde(flatten)]
            scenario: ScenarioPricingOverrides,
        }

        let flat = FlatWire::deserialize(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            instrument: flat.instrument,
            metrics: flat.metrics,
            scenario: flat.scenario,
        })
    }
}

// ---------------------------------------------------------------------------
// Temporary full-bag compatibility type
// ---------------------------------------------------------------------------

/// Optional parameters that override model pricing with market quotes.
///
/// Fields are organized into focused sub-structs:
/// - [`market_quotes`](MarketQuoteOverrides): quoted prices, implied vol, spreads, upfront payments
/// - [`metrics`](MetricPricingOverrides): bump sizes, theta period, deterministic MC seeds
/// - [`model_config`](ModelConfig): vol model selection, tree pricing, exercise friction
/// - [`scenario`](ScenarioPricingOverrides): scenario-only revaluation adjustments
///
/// # Serde Compatibility
///
/// Sub-struct fields are flattened so existing JSON payloads with flat field names
/// continue to round-trip correctly.
#[derive(Debug, Clone, Default, serde::Serialize, schemars::JsonSchema)]
#[serde(default)]
pub struct PricingOverrides {
    /// Instrument-owned pricing inputs.
    #[serde(flatten)]
    pub instrument: InstrumentPricingOverrides,
    /// Metric-time controls (bumps, theta horizon, deterministic MC seeds).
    #[serde(flatten)]
    pub metrics: MetricPricingOverrides,
    /// Scenario-only price and spread adjustments.
    #[serde(flatten)]
    pub scenario: ScenarioPricingOverrides,
}

impl<'de> serde::Deserialize<'de> for PricingOverrides {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        PricingOverridesWire::deserialize(deserializer).map(Self::from)
    }
}

impl From<PricingOverridesWire> for PricingOverrides {
    fn from(wire: PricingOverridesWire) -> Self {
        Self {
            instrument: wire.instrument,
            metrics: wire.metrics,
            scenario: wire.scenario,
        }
    }
}

impl From<&PricingOverrides> for PricingOverridesWire {
    fn from(overrides: &PricingOverrides) -> Self {
        Self {
            instrument: overrides.instrument.clone(),
            metrics: overrides.metrics.clone(),
            scenario: overrides.scenario.clone(),
        }
    }
}

impl std::ops::Deref for PricingOverrides {
    type Target = InstrumentPricingOverrides;

    fn deref(&self) -> &Self::Target {
        &self.instrument
    }
}

impl std::ops::DerefMut for PricingOverrides {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instrument
    }
}

impl PricingOverrides {
    /// Create empty pricing overrides
    pub fn none() -> Self {
        Self::default()
    }

    /// Rho bump size expressed in **basis points** (bp) suitable for curve bump APIs.
    ///
    /// Conversions:
    /// - `0.0001` (decimal) = `1.0` (bp)
    /// - `0.0010` (decimal) = `10.0` (bp)
    ///
    /// This helper exists to prevent accidental \(10{,}000\times\) unit errors when
    /// calling APIs that expect bp units.
    pub fn rho_bump_bp(&self) -> f64 {
        self.metrics.rho_bump_bp()
    }

    // -- Market Quote builders -----------------------------------------------

    /// Set quoted clean price as a percentage of par (e.g., `99.5` = 99.5% of par).
    pub fn with_quoted_clean_price(mut self, price_pct: f64) -> Self {
        self.market_quotes.quoted_clean_price = Some(price_pct);
        self
    }

    /// Set quoted dirty price in the bond's currency units.
    pub fn with_quoted_dirty_price(mut self, price_ccy: f64) -> Self {
        self.market_quotes.quoted_dirty_price_ccy = Some(price_ccy);
        self
    }

    /// Set quoted yield-to-maturity in decimal (e.g., `0.055` = 5.5%).
    pub fn with_quoted_ytm(mut self, ytm: f64) -> Self {
        self.market_quotes.quoted_ytm = Some(ytm);
        self
    }

    /// Set quoted yield-to-worst in decimal.
    pub fn with_quoted_ytw(mut self, ytw: f64) -> Self {
        self.market_quotes.quoted_ytw = Some(ytw);
        self
    }

    /// Set quoted Z-spread in decimal (e.g., `0.0125` = 125bp).
    pub fn with_quoted_z_spread(mut self, z_spread: f64) -> Self {
        self.market_quotes.quoted_z_spread = Some(z_spread);
        self
    }

    /// Set quoted OAS in decimal.
    pub fn with_quoted_oas(mut self, oas: f64) -> Self {
        self.market_quotes.quoted_oas = Some(oas);
        self
    }

    /// Set quoted discount margin (for FRNs) in decimal.
    pub fn with_quoted_discount_margin(mut self, dm: f64) -> Self {
        self.market_quotes.quoted_discount_margin = Some(dm);
        self
    }

    /// Set quoted I-spread in decimal.
    pub fn with_quoted_i_spread(mut self, i_spread: f64) -> Self {
        self.market_quotes.quoted_i_spread = Some(i_spread);
        self
    }

    /// Set quoted asset-swap spread (market convention) in decimal.
    pub fn with_quoted_asw_market(mut self, asw: f64) -> Self {
        self.market_quotes.quoted_asw_market = Some(asw);
        self
    }

    /// Set implied volatility (flat σ across tenor and strike).
    pub fn with_implied_vol(mut self, vol: f64) -> Self {
        self.market_quotes.implied_volatility = Some(vol);
        self
    }

    /// Set the CDS par-spread quote (basis points, for CDS and CDS index pricers).
    pub fn with_cds_quote_bp(mut self, spread_bp: f64) -> Self {
        self.market_quotes.cds_quote_bp = Some(spread_bp);
        self
    }

    /// Set upfront payment.
    pub fn with_upfront(mut self, upfront: Money) -> Self {
        self.market_quotes.upfront_payment = Some(upfront);
        self
    }

    // -- Model Config builders -----------------------------------------------

    /// Set volatility surface extrapolation policy.
    pub fn with_vol_surface_extrapolation(mut self, policy: VolSurfaceExtrapolation) -> Self {
        self.model_config.vol_surface_extrapolation = policy;
        self
    }

    /// Use linear-in-variance extrapolation for vol surfaces.
    ///
    /// This is the market-standard approach for equity/FX volatility surfaces
    /// when extrapolation is required beyond the calibrated grid.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::pricing_overrides::PricingOverrides;
    ///
    /// let overrides = PricingOverrides::none().with_linear_in_variance_extrapolation();
    /// ```
    pub fn with_linear_in_variance_extrapolation(mut self) -> Self {
        self.model_config.vol_surface_extrapolation = VolSurfaceExtrapolation::LinearInVariance;
        self
    }

    /// Set number of time steps for tree-based pricing.
    pub fn with_tree_steps(mut self, steps: usize) -> Self {
        self.model_config.tree_steps = Some(steps);
        self
    }

    /// Set the discount curve used for tree-based option/OAS pricing.
    pub fn with_tree_discount_curve_id(mut self, curve_id: impl Into<CurveId>) -> Self {
        self.model_config.tree_discount_curve_id = Some(curve_id.into());
        self
    }

    /// Set the forward curve used for asset-swap spread metrics.
    pub fn with_asw_forward_curve_id(mut self, curve_id: impl Into<CurveId>) -> Self {
        self.model_config.asw_forward_curve_id = Some(curve_id.into());
        self
    }

    /// Set issuer/borrower call exercise friction, in **cents per 100** of par.
    pub fn with_call_friction_cents(mut self, cents: f64) -> Self {
        self.model_config.call_friction_cents = Some(cents);
        self
    }

    /// Set the Merton Monte Carlo configuration for structural credit pricing.
    pub fn with_merton_mc(
        mut self,
        config: crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig,
    ) -> Self {
        self.model_config.merton_mc_config = Some(MertonMcOverride(config));
        self
    }

    /// Set Monte Carlo path count for path-dependent GBM pricing (when supported by the instrument pricer).
    pub fn with_mc_paths(mut self, paths: usize) -> Self {
        self.model_config.mc_paths = Some(paths);
        self
    }

    // -- Bump Config builders ------------------------------------------------

    /// Set custom YTM bump size (decimal). For 1 bp, pass 1e-4.
    pub fn with_ytm_bump_decimal(mut self, bump: f64) -> Self {
        self.metrics.bump_config.ytm_bump_decimal = Some(bump);
        self
    }

    /// Enable adaptive bump sizes for greek calculations.
    ///
    /// Adaptive bumps scale based on volatility, time to expiry, and moneyness
    /// to improve numerical stability for extreme parameter values.
    pub fn with_adaptive_bumps(mut self, enable: bool) -> Self {
        self.metrics.bump_config.adaptive_bumps = enable;
        self
    }

    // The bump/metric builders below delegate to the identical
    // `MetricPricingOverrides` methods so the builder bodies live in one place.

    /// Set custom spot bump size (as percentage, e.g., 0.01 for 1%).
    ///
    /// Overrides both standard and adaptive calculations when set.
    pub fn with_spot_bump(mut self, bump_pct: f64) -> Self {
        self.metrics = self.metrics.with_spot_bump(bump_pct);
        self
    }

    /// Set custom volatility bump size (as absolute vol, e.g., 0.01 for 1% vol).
    ///
    /// Overrides both standard and adaptive calculations when set.
    pub fn with_vol_bump(mut self, bump_pct: f64) -> Self {
        self.metrics = self.metrics.with_vol_bump(bump_pct);
        self
    }

    /// Set custom rate bump size (in basis points, e.g., 1.0 for 1bp).
    ///
    /// Overrides both standard and adaptive calculations when set.
    pub fn with_rate_bump(mut self, bump_bp: f64) -> Self {
        self.metrics = self.metrics.with_rate_bump(bump_bp);
        self
    }

    /// Set custom credit spread bump size (in basis points, e.g., 1.0 for 1bp).
    pub fn with_credit_spread_bump(mut self, bump_bp: f64) -> Self {
        self.metrics = self.metrics.with_credit_spread_bump(bump_bp);
        self
    }

    // -- Scenario builders ---------------------------------------------------

    /// Set theta period for time decay calculations.
    pub fn with_theta_period(mut self, period: impl Into<String>) -> Self {
        self.metrics = self.metrics.with_theta_period(period);
        self
    }

    /// Set breakeven configuration.
    pub fn with_breakeven_config(mut self, config: BreakevenConfig) -> Self {
        self.metrics = self.metrics.with_breakeven_config(config);
        self
    }

    /// Set MC seed scenario for deterministic greek calculations.
    ///
    /// The scenario name (e.g., "delta_up", "vega_down") is used to derive
    /// a deterministic seed from the instrument ID, ensuring reproducibility.
    pub fn with_mc_seed_scenario(mut self, scenario: impl Into<String>) -> Self {
        self.metrics = self.metrics.with_mc_seed_scenario(scenario);
        self
    }

    /// Set bond risk basis for duration, convexity, and DV01-style metrics.
    pub fn with_bond_risk_basis(mut self, basis: BondRiskBasis) -> Self {
        self.metrics = self.metrics.with_bond_risk_basis(basis);
        self
    }

    /// Set Historical VaR / Expected Shortfall configuration.
    pub fn with_var_config(mut self, config: crate::metrics::risk::VarConfig) -> Self {
        self.metrics = self.metrics.with_var_config(config);
        self
    }

    /// Apply a scenario price shock (as decimal percentage).
    ///
    /// The shock is applied as a multiplier: `price * (1 + shock_pct)`.
    /// For example, -0.05 represents a -5% price shock.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_valuations::instruments::pricing_overrides::PricingOverrides;
    ///
    /// // Apply a -10% price shock
    /// let overrides = PricingOverrides::none().with_price_shock_pct(-0.10);
    /// assert_eq!(overrides.scenario.scenario_price_shock_pct, Some(-0.10));
    /// ```
    pub fn with_price_shock_pct(mut self, shock_pct: f64) -> Self {
        self.scenario.scenario_price_shock_pct = Some(shock_pct);
        self
    }

    /// Apply a scenario spread shock in basis points.
    ///
    /// The shock is applied as an additional flat Z-spread by pricers that
    /// support spread-based revaluation (see
    /// [`ScenarioPricingOverrides::scenario_spread_shock_bp`]).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_valuations::instruments::pricing_overrides::PricingOverrides;
    ///
    /// // Apply a +150 bp spread widening
    /// let overrides = PricingOverrides::none().with_spread_shock_bp(150.0);
    /// assert_eq!(overrides.scenario.scenario_spread_shock_bp, Some(150.0));
    /// ```
    pub fn with_spread_shock_bp(mut self, shock_bp: f64) -> Self {
        self.scenario.scenario_spread_shock_bp = Some(shock_bp);
        self
    }

    /// Clear any scenario shocks applied to this override.
    pub fn clear_scenario_shocks(&mut self) {
        self.scenario.scenario_price_shock_pct = None;
        self.scenario.scenario_spread_shock_bp = None;
    }

    /// Check if any scenario shock is applied.
    pub fn has_scenario_shock(&self) -> bool {
        self.scenario.scenario_price_shock_pct.is_some()
            || self.scenario.scenario_spread_shock_bp.is_some()
    }

    /// Validate override values for finiteness and non-negativity; basic `theta_period` sanity.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.market_quotes.validate()?;
        self.metrics.validate()?;
        self.model_config.validate()?;
        self.scenario.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[derive(Debug, Clone, finstack_quant_valuations_macros::FocusedPricingOverrides)]
    #[serde(deny_unknown_fields)]
    struct FocusedWireFixture {
        id: String,
        instrument_pricing_overrides: InstrumentPricingOverrides,
        metric_pricing_overrides: MetricPricingOverrides,
        scenario_pricing_overrides: ScenarioPricingOverrides,
    }

    #[test]
    fn validate_accepts_default_and_positive_values() {
        let po = PricingOverrides::default()
            .with_quoted_clean_price(100.0)
            .with_ytm_bump_decimal(1e-4)
            .with_spot_bump(0.01)
            .with_vol_bump(0.01)
            .with_rate_bump(1.0);
        assert!(po.validate().is_ok());
    }

    #[test]
    fn validate_rejects_negative_values() {
        let po = PricingOverrides::default().with_vol_bump(-0.01);
        let err = po.validate().expect_err("should fail");
        match err {
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::NegativeValue) => {}
            e => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_vol_surface_extrapolation_policies() {
        let po = PricingOverrides::default();
        assert_eq!(
            po.model_config.vol_surface_extrapolation,
            VolSurfaceExtrapolation::Error
        );

        let po =
            PricingOverrides::none().with_vol_surface_extrapolation(VolSurfaceExtrapolation::Clamp);
        assert_eq!(
            po.model_config.vol_surface_extrapolation,
            VolSurfaceExtrapolation::Clamp
        );

        let po = PricingOverrides::none().with_linear_in_variance_extrapolation();
        assert_eq!(
            po.model_config.vol_surface_extrapolation,
            VolSurfaceExtrapolation::LinearInVariance
        );

        let po = PricingOverrides::none()
            .with_vol_surface_extrapolation(VolSurfaceExtrapolation::LinearInVariance);
        assert_eq!(
            po.model_config.vol_surface_extrapolation,
            VolSurfaceExtrapolation::LinearInVariance
        );
    }

    #[test]
    fn test_vol_surface_extrapolation_serde() {
        for policy in [
            VolSurfaceExtrapolation::Error,
            VolSurfaceExtrapolation::Clamp,
            VolSurfaceExtrapolation::LinearInVariance,
        ] {
            let po = PricingOverrides::none().with_vol_surface_extrapolation(policy);
            let json = serde_json::to_string(&po).expect("serialize");
            let roundtrip: PricingOverrides = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(roundtrip.model_config.vol_surface_extrapolation, policy);
        }

        let po = PricingOverrides::none().with_linear_in_variance_extrapolation();
        let json = serde_json::to_string(&po).expect("serialize");
        assert!(
            json.contains("linear_in_variance"),
            "Should serialize as snake_case: {}",
            json
        );
    }

    #[test]
    fn serde_deserializes_flat_fields() {
        let json = r#"{
            "quoted_clean_price": 99.5,
            "implied_volatility": 0.2,
            "rate_bump_bp": 1.0,
            "tree_steps": 100,
            "scenario_price_shock_pct": -0.05,
            "theta_period": "1D"
        }"#;
        let po: PricingOverrides = serde_json::from_str(json).expect("deserialize flat fields");
        assert_eq!(po.market_quotes.quoted_clean_price, Some(99.5));
        assert_eq!(po.market_quotes.implied_volatility, Some(0.2));
        assert_eq!(po.metrics.bump_config.rate_bump_bp, Some(1.0));
        assert_eq!(po.model_config.tree_steps, Some(100));
        assert_eq!(po.scenario.scenario_price_shock_pct, Some(-0.05));
        assert_eq!(po.metrics.theta_period.as_deref(), Some("1D"));
    }

    #[test]
    fn serde_accepts_focused_nested_fields_and_serializes_flat() {
        let json = r#"{
            "instrument": {
                "quoted_clean_price": 99.5,
                "hw1f_mean_reversion": 0.03,
                "hw1f_sigma_schedule": {
                    "times": [0.0, 1.0],
                    "values": [0.01, 0.012]
                }
            },
            "metrics": {
                "rate_bump_bp": 2.0,
                "theta_period": "1W"
            },
            "scenario": {
                "scenario_price_shock_pct": -0.05
            }
        }"#;

        let po: PricingOverrides =
            serde_json::from_str(json).expect("deserialize focused nested fields");
        assert_eq!(po.market_quotes.quoted_clean_price, Some(99.5));
        assert_eq!(po.model_config.hw1f_mean_reversion, Some(0.03));
        let schedule = po
            .model_config
            .hw1f_sigma_schedule
            .as_ref()
            .expect("HW1F schedule");
        assert_eq!(schedule.times(), &[0.0, 1.0]);
        assert_eq!(schedule.values(), &[0.01, 0.012]);
        assert_eq!(po.metrics.bump_config.rate_bump_bp, Some(2.0));
        assert_eq!(po.metrics.theta_period.as_deref(), Some("1W"));
        assert_eq!(po.scenario.scenario_price_shock_pct, Some(-0.05));

        let serialized = serde_json::to_value(&po).expect("serialize flat fields");
        let object = serialized.as_object().expect("pricing overrides object");
        assert!(!object.contains_key("instrument"));
        assert!(!object.contains_key("metrics"));
        assert!(!object.contains_key("scenario"));
        assert_eq!(
            object.get("quoted_clean_price"),
            Some(&serde_json::json!(99.5))
        );
        assert_eq!(object.get("rate_bump_bp"), Some(&serde_json::json!(2.0)));
        assert_eq!(
            object.get("scenario_price_shock_pct"),
            Some(&serde_json::json!(-0.05))
        );
    }

    #[test]
    fn focused_instrument_derive_preserves_legacy_property() {
        let fixture = FocusedWireFixture {
            id: "fixture".to_string(),
            instrument_pricing_overrides: InstrumentPricingOverrides {
                market_quotes: MarketQuoteOverrides {
                    quoted_clean_price: Some(99.5),
                    ..Default::default()
                },
                ..Default::default()
            },
            metric_pricing_overrides: MetricPricingOverrides::default().with_theta_period("1W"),
            scenario_pricing_overrides: ScenarioPricingOverrides {
                scenario_price_shock_pct: Some(-0.05),
                ..Default::default()
            },
        };

        let value = serde_json::to_value(&fixture).expect("serialize focused fixture");
        let object = value.as_object().expect("fixture object");
        let wire = object
            .get("pricing_overrides")
            .and_then(serde_json::Value::as_object)
            .expect("legacy pricing_overrides property");
        assert_eq!(
            wire.get("quoted_clean_price"),
            Some(&serde_json::json!(99.5))
        );
        assert_eq!(wire.get("theta_period"), Some(&serde_json::json!("1W")));
        assert_eq!(
            wire.get("scenario_price_shock_pct"),
            Some(&serde_json::json!(-0.05))
        );

        let roundtrip: FocusedWireFixture =
            serde_json::from_value(value).expect("deserialize focused fixture");
        assert_eq!(roundtrip.id, "fixture");
        assert_eq!(
            roundtrip
                .instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price,
            Some(99.5)
        );
        assert_eq!(
            roundtrip.metric_pricing_overrides.theta_period.as_deref(),
            Some("1W")
        );
        assert_eq!(
            roundtrip
                .scenario_pricing_overrides
                .scenario_price_shock_pct,
            Some(-0.05)
        );

        let schema = schemars::schema_for!(FocusedWireFixture);
        let schema_value = serde_json::to_value(schema).expect("serialize schema");
        assert!(schema_value.to_string().contains("pricing_overrides"));
    }

    #[test]
    fn serde_nested_fields_override_flat_fields() {
        let json = r#"{
            "quoted_clean_price": 98.0,
            "rate_bump_bp": 1.0,
            "scenario_price_shock_pct": -0.01,
            "instrument_pricing_overrides": {"quoted_clean_price": 101.0},
            "metric_pricing_overrides": {"rate_bump_bp": 3.0},
            "scenario_pricing_overrides": {"scenario_price_shock_pct": -0.08}
        }"#;

        let po: PricingOverrides = serde_json::from_str(json).expect("deserialize mixed fields");
        assert_eq!(po.market_quotes.quoted_clean_price, Some(101.0));
        assert_eq!(po.metrics.bump_config.rate_bump_bp, Some(3.0));
        assert_eq!(po.scenario.scenario_price_shock_pct, Some(-0.08));
    }

    #[test]
    fn serde_deserializes_var_config_override() {
        let json = r#"{
            "var_config": {
                "confidence_level": 0.99,
                "method": "taylor_approximation",
                "reporting_currency": "USD"
            }
        }"#;
        let po: PricingOverrides = serde_json::from_str(json).expect("deserialize var config");
        let var_config = po.metrics.var_config.expect("var config should be present");

        assert_eq!(var_config.confidence_level, 0.99);
        assert_eq!(
            var_config.method,
            crate::metrics::risk::VarMethod::TaylorApproximation
        );
        assert_eq!(var_config.reporting_currency, Some(Currency::USD));
    }

    #[test]
    fn serde_rejects_removed_tree_volatility_field() {
        let err = serde_json::from_str::<PricingOverrides>(r#"{"tree_volatility":0.15}"#)
            .expect_err("tree_volatility was removed; use implied_volatility");

        assert!(
            err.to_string().contains("tree_volatility"),
            "error should name removed field, got {err}"
        );

        let err =
            serde_json::from_str::<PricingOverrides>(r#"{"instrument":{"tree_volatility":0.15}}"#)
                .expect_err("nested tree_volatility was removed; use implied_volatility");
        assert!(
            err.to_string().contains("tree_volatility"),
            "error should name nested removed field, got {err}"
        );
    }

    #[test]
    fn serde_roundtrip_preserves_all_fields() {
        let po = PricingOverrides::none()
            .with_quoted_clean_price(100.0)
            .with_implied_vol(0.25)
            .with_cds_quote_bp(50.0)
            .with_rate_bump(2.0)
            .with_spot_bump(0.01)
            .with_tree_steps(200)
            .with_price_shock_pct(-0.10)
            .with_theta_period("1W");

        let json = serde_json::to_string(&po).expect("serialize");
        let rt: PricingOverrides = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(rt.market_quotes.quoted_clean_price, Some(100.0));
        assert_eq!(rt.market_quotes.implied_volatility, Some(0.25));
        assert_eq!(rt.market_quotes.cds_quote_bp, Some(50.0));
        assert_eq!(rt.metrics.bump_config.rate_bump_bp, Some(2.0));
        assert_eq!(rt.metrics.bump_config.spot_bump_pct, Some(0.01));
        assert_eq!(rt.model_config.tree_steps, Some(200));
        assert_eq!(rt.scenario.scenario_price_shock_pct, Some(-0.10));
        assert_eq!(rt.metrics.theta_period.as_deref(), Some("1W"));
    }

    #[test]
    fn metric_overrides_extract_theta_and_bumps_without_scenario_shocks() {
        let po = PricingOverrides::none()
            .with_theta_period("1W")
            .with_mc_seed_scenario("theta_roll")
            .with_rate_bump(2.0)
            .with_spot_bump(0.01)
            .with_var_config(crate::metrics::risk::VarConfig::var_99())
            .with_price_shock_pct(-0.10);

        let metric_overrides = MetricPricingOverrides::from(&po);

        assert_eq!(metric_overrides.theta_period.as_deref(), Some("1W"));
        assert_eq!(
            metric_overrides.mc_seed_scenario.as_deref(),
            Some("theta_roll")
        );
        assert_eq!(metric_overrides.bump_config.rate_bump_bp, Some(2.0));
        assert_eq!(metric_overrides.bump_config.spot_bump_pct, Some(0.01));
        assert_eq!(
            metric_overrides
                .var_config
                .as_ref()
                .map(|config| config.confidence_level),
            Some(0.99)
        );
    }

    /// `MetricPricingOverrides::validate` must accept a lowercase `theta_period`
    /// unit suffix (e.g. `"1d"`). The downstream `parse_theta_period` consumer
    /// uppercases the suffix, so a lowercase form prices fine at runtime; the
    /// JSON-boundary validation must not reject what the pricer would accept.
    #[test]
    fn validate_accepts_case_insensitive_theta_period() {
        for period in ["1d", "1D", "2w", "3M", "1y", "10Y", "12m"] {
            let overrides = MetricPricingOverrides::default().with_theta_period(period);
            assert!(
                overrides.validate().is_ok(),
                "theta_period {period:?} should validate (case-insensitive unit)"
            );
        }
    }

    /// Genuinely malformed `theta_period` values must still be rejected — the
    /// case-normalization fix must not weaken validation of bad input.
    #[test]
    fn validate_rejects_malformed_theta_period() {
        for period in ["1x", "D", "abc", "1", "1.5d", "-1d", ""] {
            let overrides = MetricPricingOverrides::default().with_theta_period(period);
            assert!(
                overrides.validate().is_err(),
                "malformed theta_period {period:?} must be rejected"
            );
        }
    }

    #[test]
    fn scenario_overrides_extract_only_price_shock() {
        let mut po = PricingOverrides::none()
            .with_theta_period("1W")
            .with_mc_seed_scenario("theta_roll")
            .with_price_shock_pct(-0.10);

        po.model_config.use_gobet_miri = true;

        let scenario_overrides = ScenarioPricingOverrides::from(&po);

        assert_eq!(scenario_overrides.scenario_price_shock_pct, Some(-0.10));
    }
}
