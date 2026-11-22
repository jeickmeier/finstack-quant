//! Pricing overrides for market-quoted instruments.

mod market_quotes;
mod metric_overrides;
mod model_config;

#[cfg(test)]
mod tests;

pub use market_quotes::MarketQuoteOverrides;
pub use metric_overrides::{
    BondRiskBasis, BumpConfig, MetricPricingOverrides, ScenarioPricingOverrides, ThetaDayBasis,
};
pub use model_config::{ModelConfig, ShortRateTreeModel, VolSurfaceExtrapolation};

/// Check a batch of optional scalars for finiteness (and optional non-negativity).
///
/// Each entry is `(value, must_be_nonneg)`: an unset `value` is skipped. A
/// `must_be_nonneg = false` field need only be finite (failing with
/// `Error::Validation`); a `must_be_nonneg = true` field must be both finite
/// and `>= 0` (failing with `InputError::NegativeValue`).
pub(super) fn check_finite_fields(
    fields: &[(Option<f64>, bool)],
) -> finstack_quant_core::Result<()> {
    use finstack_quant_core::InputError;
    for &(value, must_be_nonneg) in fields {
        if let Some(v) = value {
            if must_be_nonneg {
                if !(v.is_finite() && v >= 0.0) {
                    return Err(InputError::NegativeValue.into());
                }
            } else if !v.is_finite() {
                return Err(finstack_quant_core::Error::Validation(
                    "pricing override values must be finite".to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// Instrument-owned pricing inputs that can materially change valuation.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct InstrumentPricingOverrides {
    /// Market-quoted values (prices, implied vol, spreads, upfront payments).
    #[serde(skip_serializing_if = "MarketQuoteOverrides::is_empty")]
    pub market_quotes: MarketQuoteOverrides,
    /// Model selection and tree pricing parameters.
    #[serde(skip_serializing_if = "ModelConfig::is_empty")]
    pub model_config: ModelConfig,
}

impl InstrumentPricingOverrides {
    /// Return whether no instrument-level override is configured.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Set the quoted clean price in percent of par.
    ///
    /// # Arguments
    ///
    /// * `price_pct` - Clean price in percent of par (e.g. `99.5` = 99.5% of
    ///   par); inflation-linked bonds quote it per 100 of real face.
    pub fn with_quoted_clean_price_pct(mut self, price_pct: f64) -> Self {
        self.market_quotes.quoted_clean_price_pct = Some(price_pct);
        self
    }

    /// Set the observed option premium that `ImpliedVol` inverts.
    ///
    /// # Arguments
    ///
    /// * `premium` - Total trade PV in the instrument currency (notional,
    ///   contract multiplier and position included); it does not drive PV.
    pub fn with_quoted_premium(mut self, premium: f64) -> Self {
        self.market_quotes.quoted_premium = Some(premium);
        self
    }

    /// Set quoted dirty price in the instrument currency.
    ///
    /// # Arguments
    ///
    /// * `price_currency` - Dirty bond price in native currency units.
    pub fn with_quoted_dirty_price_currency(mut self, price_currency: f64) -> Self {
        self.market_quotes.quoted_dirty_price_currency = Some(price_currency);
        self
    }

    /// Set quoted yield-to-maturity in decimal form.
    ///
    /// # Arguments
    ///
    /// * `ytm` - Decimal yield-to-maturity (e.g. `0.055` = 5.5%).
    pub fn with_quoted_ytm(mut self, ytm: f64) -> Self {
        self.market_quotes.quoted_ytm = Some(ytm);
        self
    }

    /// Set quoted yield-to-worst in decimal form.
    ///
    /// # Arguments
    ///
    /// * `ytw` - Decimal yield-to-worst.
    pub fn with_quoted_ytw(mut self, ytw: f64) -> Self {
        self.market_quotes.quoted_ytw = Some(ytw);
        self
    }

    /// Set quoted Z-spread in decimal form.
    ///
    /// # Arguments
    ///
    /// * `z_spread` - Decimal Z-spread (e.g. `0.0125` = 125 bp).
    pub fn with_quoted_z_spread(mut self, z_spread: f64) -> Self {
        self.market_quotes.quoted_z_spread = Some(z_spread);
        self
    }

    /// Set quoted OAS in decimal form.
    ///
    /// # Arguments
    ///
    /// * `oas` - Decimal option-adjusted spread.
    pub fn with_quoted_oas(mut self, oas: f64) -> Self {
        self.market_quotes.quoted_oas = Some(oas);
        self
    }

    /// Set quoted discount margin in decimal form.
    ///
    /// # Arguments
    ///
    /// * `dm` - Decimal floating-rate note discount margin.
    pub fn with_quoted_discount_margin(mut self, dm: f64) -> Self {
        self.market_quotes.quoted_discount_margin = Some(dm);
        self
    }

    /// Set quoted I-spread in decimal form.
    ///
    /// # Arguments
    ///
    /// * `i_spread` - Decimal interpolated spread over swap curve.
    pub fn with_quoted_i_spread(mut self, i_spread: f64) -> Self {
        self.market_quotes.quoted_i_spread = Some(i_spread);
        self
    }

    /// Set quoted asset-swap spread in decimal form.
    ///
    /// # Arguments
    ///
    /// * `asw` - Decimal market-convention asset-swap spread.
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
    /// * `vol` - Option-implied volatility as a decimal (e.g. `0.35`); this
    ///   is an option-quote channel, not a short-rate σ — the bond lattices
    ///   read [`Self::with_hw1f_sigma`], `model_config.bdt_sigma` or
    ///   `model_config.bk_sigma` instead.
    pub fn with_implied_volatility(mut self, vol: f64) -> Self {
        self.market_quotes.implied_volatility = Some(vol);
        self
    }

    /// Set the Hull-White short-rate volatility σ (annualised, absolute).
    ///
    /// This is the Hull-White short-rate volatility read by the bond
    /// lattices; [`Self::with_implied_volatility`] sets an option implied
    /// volatility, which no short-rate model reads.
    ///
    /// # Arguments
    ///
    /// * `sigma` - Annualised absolute short-rate volatility (e.g. `0.01` is
    ///   100 bp/yr).
    pub fn with_hw1f_sigma(mut self, sigma: f64) -> Self {
        self.model_config.hw1f_sigma = Some(sigma);
        self
    }

    /// Set the Hull-White mean-reversion speed κ (annualised).
    ///
    /// Companion to [`Self::with_hw1f_sigma`]: the rates-only bond tree and
    /// the other Hull-White pricers require the complete pair.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean-reversion speed per year (e.g. `0.03` is 3%/yr).
    pub fn with_hw1f_mean_reversion(mut self, kappa: f64) -> Self {
        self.model_config.hw1f_mean_reversion = Some(kappa);
        self
    }

    /// Set the hazard-rate volatility σ_λ for the rates-credit callable
    /// lattice (annualised, absolute decimal hazard points per √year).
    ///
    /// # Arguments
    ///
    /// * `sigma` - Annualised absolute hazard volatility (e.g. `0.0105` for
    ///   a 35% fractional vol on a 3% hazard).
    pub fn with_hazard_sigma(mut self, sigma: f64) -> Self {
        self.model_config.hazard_sigma = Some(sigma);
        self
    }

    /// Set the number of time steps for tree-based pricing.
    ///
    /// # Arguments
    ///
    /// * `steps` - Positive lattice step count.
    pub fn with_tree_steps(mut self, steps: usize) -> Self {
        self.model_config.tree_steps = Some(steps);
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

    /// Set the Monte Carlo seed label.
    ///
    /// # Arguments
    ///
    /// * `scenario` - Label hashed with the instrument id to derive the RNG
    ///   seed; the same label replays the same random streams.
    #[must_use]
    pub fn with_mc_seed_scenario(mut self, scenario: impl Into<String>) -> Self {
        self.model_config.mc_seed_scenario = Some(scenario.into());
        self
    }

    /// Validate instrument-owned override fields.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.market_quotes.validate()?;
        self.model_config.validate()?;
        Ok(())
    }
}
