//! FX option instrument implementation using Garman–Kohlhagen model.
//!
//! # ATM Convention
//!
//! **Important**: This implementation does not include an ATM strike calculation.
//! When constructing FX options, the strike must be provided explicitly.
//!
//! In professional FX option markets, "ATM" typically refers to the **Delta-Neutral
//! Straddle (DNS)** strike, not the forward rate. The DNS strike is defined as the
//! strike where the call delta equals the negative of the put delta:
//!
//! ```text
//! ATM DNS: Strike where Δ_call = -Δ_put
//! ```
//!
//! For forward delta (interbank convention), this gives a strike slightly different
//! from the forward rate due to vol smile effects.
//!
//! If you need to construct an ATM option, you should:
//! 1. Compute the forward rate: `F = S × DF_foreign / DF_domestic`
//! 2. Use the forward rate as the strike for approximate ATM (ATMF convention)
//! 3. For precise ATM DNS, solve for the strike where `Δ_call = -Δ_put`
//!
//! # Delta Convention
//!
//! The calculator provides both:
//! - **Spot delta** (`delta`): Bloomberg default, includes foreign rate discounting
//! - **Forward delta** (`delta_forward`): Interbank convention, no discounting
//!
//! Use `delta_forward` for professional FX option hedging and vol surface interpolation.
//!
//! # Volatility Surface Parameterization
//!
//! **Important**: The vol surface lookup in this implementation uses **absolute strike**
//! as the moneyness dimension (via `finstack_quant_models::volatility::get_surface_vol_clamped(&vol_surface, t, strike)`). This is
//! a simpler parameterization than the delta-based quoting convention used in
//! professional FX interbank markets, where the vol surface is typically quoted in
//! terms of delta (e.g., 25Δ put, ATM DNS, 25Δ call) and interpolated in delta space.
//!
//! For most use cases (flat or moderately shaped surfaces), strike-based lookup is
//! adequate. For precise smile-sensitive pricing with market-standard FX vol surfaces,
//! a delta-to-strike conversion layer may be needed on top of the surface provider.

use crate::instruments::common_impl::parameters::FxUnderlyingParams;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::OptionType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
// Pricing/greeks live in pricing engine; keep types minimal.
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;

use super::pricer;
use crate::impl_instrument_base;

fn default_fx_underlying(base_currency: Currency, quote_currency: Currency) -> FxUnderlyingParams {
    // Fall back to currency-aware OIS curves instead of hardwiring USD legs.
    let domestic = CurveId::new(format!("{}-OIS", quote_currency));
    let foreign = CurveId::new(format!("{}-OIS", base_currency));
    FxUnderlyingParams::new(base_currency, quote_currency, domestic, foreign)
}

/// Pair/venue FX delta quoting convention selected by the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FxDeltaConventionKind {
    /// Unadjusted spot delta.
    Spot,
    /// Unadjusted forward delta.
    Forward,
    /// Premium-adjusted spot delta.
    PremiumAdjustedSpot,
    /// Premium-adjusted forward delta.
    PremiumAdjustedForward,
}

impl std::fmt::Display for FxDeltaConventionKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Spot => "spot",
            Self::Forward => "forward",
            Self::PremiumAdjustedSpot => "premium_adjusted_spot",
            Self::PremiumAdjustedForward => "premium_adjusted_forward",
        })
    }
}

impl std::str::FromStr for FxDeltaConventionKind {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "spot" => Ok(Self::Spot),
            "forward" => Ok(Self::Forward),
            "premium_adjusted_spot" => Ok(Self::PremiumAdjustedSpot),
            "premium_adjusted_forward" => Ok(Self::PremiumAdjustedForward),
            _ => Err(format!(
                "invalid FX delta convention '{value}'; expected spot, forward, \
                 premium_adjusted_spot, or premium_adjusted_forward"
            )),
        }
    }
}
/// Explicit venue and premium-currency convention for FX delta reporting.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FxDeltaConvention {
    /// Delta convention quoted by the venue.
    pub kind: FxDeltaConventionKind,
    /// Currency in which the option premium is paid.
    pub premium_currency: Currency,
    /// Non-empty market venue or quoting-source identifier.
    pub venue: String,
}

impl FxDeltaConvention {
    /// Create an explicit FX delta convention.
    ///
    /// # Arguments
    ///
    /// * `kind` - Spot, forward, or premium-adjusted quoting convention.
    /// * `premium_currency` - Currency in which premium is paid.
    /// * `venue` - Non-empty venue or quoting-source identifier.
    pub fn new(
        kind: FxDeltaConventionKind,
        premium_currency: Currency,
        venue: impl Into<String>,
    ) -> finstack_quant_core::Result<Self> {
        let venue = venue.into();
        if venue.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "FX delta convention venue must not be blank".to_string(),
            ));
        }
        Ok(Self {
            kind,
            premium_currency,
            venue,
        })
    }
}

/// European FX option with same-day cash settlement at expiry.
///
/// Physical delivery is intentionally not represented: exercised payoff is a
/// quote-currency cash amount on `expiry`.
#[derive(
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FxOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Base currency (foreign currency)
    pub base_currency: Currency,
    /// Quote currency (domestic currency)
    pub quote_currency: Currency,
    /// Strike exchange rate (quote per base).
    ///
    /// **Note on ATM convention**: Professional FX markets define ATM as the
    /// Delta-Neutral Straddle (DNS) strike, not the forward rate. See module
    /// documentation for details. If constructing an "ATM" option, compute
    /// the forward rate or DNS strike externally.
    pub strike: f64,
    /// Option type (call or put on base currency)
    pub option_type: OptionType,
    /// Pair/venue delta convention and premium currency.
    pub delta_convention: FxDeltaConvention,
    /// Option expiry date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Day count convention
    #[serde(default = "crate::serde_defaults::day_count_act365f")]
    #[builder(default = finstack_quant_core::dates::DayCount::Act365F)]
    pub day_count: finstack_quant_core::dates::DayCount,
    /// Notional amount in base currency
    pub notional: Money,
    /// Domestic currency discount curve ID
    pub domestic_discount_curve_id: CurveId,
    /// Foreign currency discount curve ID
    pub foreign_discount_curve_id: CurveId,
    /// FX volatility surface ID
    pub vol_surface_id: CurveId,
    /// Pricing overrides (manual price, yield, spread)
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

// Declare canonical market dependencies for the DV01 calculator.
// FxOption uses both domestic and foreign curves for Garman-Kohlhagen pricing
/// Delta conventions relevant for FX ATM DNS strikes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FxAtmDeltaConvention {
    /// Unadjusted spot delta convention.
    Spot,
    /// Unadjusted forward delta convention.
    Forward,
    /// Premium-adjusted spot delta convention.
    PremiumAdjustedSpot,
    /// Premium-adjusted forward delta convention.
    PremiumAdjustedForward,
}

impl FxOption {
    /// Validate FX option currency invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FxOption",
        )?;
        if self.notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: self.notional.currency(),
            });
        }
        if !matches!(
            self.delta_convention.premium_currency,
            currency if currency == self.base_currency || currency == self.quote_currency
        ) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxOption premium currency {} must be either base {} or quote {} currency",
                self.delta_convention.premium_currency, self.base_currency, self.quote_currency
            )));
        }
        if self.delta_convention.venue.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "FxOption delta convention venue must not be blank".to_string(),
            ));
        }
        crate::instruments::common_impl::validation::validate_money_finite(
            self.notional,
            "FxOption notional",
        )?;
        crate::instruments::common_impl::validation::validate_money_gt(
            self.notional,
            0.0,
            "FxOption notional",
        )?;
        Ok(())
    }

    /// Create a canonical example FX option for testing and documentation.
    ///
    /// Returns an EUR/USD call expiring on the project-wide stable example
    /// epoch (`crate::instruments::common_impl::EXAMPLE_FAR_EXPIRY`). The
    /// example is intentionally future-dated so docs and demos hit the live
    /// pricing path rather than the expired-option intrinsic-only branch.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("FXOPT-EURUSD-CALL"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .strike(1.12)
            .option_type(OptionType::Call)
            .delta_convention(FxDeltaConvention::new(
                FxDeltaConventionKind::Forward,
                Currency::USD,
                "generic_interbank",
            )?)
            .expiry(crate::instruments::common_impl::example_constants::FAR_EXPIRY)
            .day_count(DayCount::Act365F)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
    }

    /// Create a European FX option on a pair with standard conventions.
    ///
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    #[allow(clippy::too_many_arguments)]
    pub fn european(
        id: impl Into<InstrumentId>,
        base_currency: Currency,
        quote_currency: Currency,
        strike: f64,
        expiry: Date,
        notional: Money,
        vol_surface_id: impl Into<CurveId>,
        option_type: OptionType,
        delta_convention: FxDeltaConvention,
    ) -> finstack_quant_core::Result<Self> {
        let fx_underlying = if quote_currency == Currency::USD && base_currency == Currency::EUR {
            FxUnderlyingParams::usd_eur()
        } else if quote_currency == Currency::USD && base_currency == Currency::GBP {
            FxUnderlyingParams::gbp_usd()
        } else {
            default_fx_underlying(base_currency, quote_currency)
        };
        Self::builder()
            .id(id.into())
            .base_currency(fx_underlying.base_currency)
            .quote_currency(fx_underlying.quote_currency)
            .strike(strike)
            .option_type(option_type)
            .delta_convention(delta_convention)
            .expiry(expiry)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .notional(notional)
            .domestic_discount_curve_id(fx_underlying.domestic_discount_curve_id.to_owned())
            .foreign_discount_curve_id(fx_underlying.foreign_discount_curve_id)
            .vol_surface_id(vol_surface_id.into())
            .attributes(Attributes::new())
            .build()
    }

    /// Compute present value using Garman–Kohlhagen model.
    pub fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        pricer::compute_pv(self, market, as_of)
    }

    /// Solve for implied volatility.
    pub fn implied_vol(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
        target_price: f64,
    ) -> Result<f64> {
        pricer::implied_vol(self, curves, as_of, target_price)
    }

    /// Calculate the Delta-Neutral Straddle (DNS) strike.
    ///
    /// The DNS strike is the strike where the call delta equals the negative of
    /// the put delta. This is the interbank convention for "ATM" options.
    ///
    /// For unadjusted spot or forward delta conventions:
    /// ```text
    /// K_DNS = F × exp(0.5 × σ² × T)
    /// ```
    ///
    /// For premium-adjusted spot or forward delta conventions:
    /// ```text
    /// K_DNS = F × exp(-0.5 × σ² × T)
    /// ```
    ///
    /// # Arguments
    ///
    /// * `forward` - Forward FX rate `S · DF_foreign / DF_domestic`
    /// * `vol` - ATM volatility (decimal, e.g., 0.10 for 10%)
    /// * `time_to_expiry` - Time to expiry in years
    /// * `convention` - ATM delta convention determining the DNS formula variant
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_valuations::instruments::fx::fx_option::{
    ///     FxAtmDeltaConvention, FxOption,
    /// };
    ///
    /// let forward = 1.111;
    /// let vol = 0.10; // 10% vol
    /// let t = 0.5; // 6 months
    ///
    /// // Spot delta DNS (Bloomberg default)
    /// let k_dns_spot = FxOption::atm_dns_strike_for_convention(
    ///     forward, vol, t, FxAtmDeltaConvention::Spot,
    /// );
    ///
    /// // Forward delta DNS (interbank standard)
    /// let k_dns_fwd = FxOption::atm_dns_strike_for_convention(
    ///     forward, vol, t, FxAtmDeltaConvention::Forward,
    /// );
    ///
    /// // Both DNS strikes sit above the forward by half the variance.
    /// assert!(k_dns_spot > forward);
    /// assert!(k_dns_fwd > forward);
    /// ```
    ///
    /// # References
    ///
    /// - Wystup, U. (2006). *FX Options and Structured Products*. Chapter 2. `docs/REFERENCES.md#wystup-fx-options`
    /// - Clark, I. J. (2011). *Foreign Exchange Option Pricing*. Chapter 3. `docs/REFERENCES.md#clark-fx-options`
    pub fn atm_dns_strike_for_convention(
        forward: f64,
        vol: f64,
        time_to_expiry: f64,
        convention: FxAtmDeltaConvention,
    ) -> f64 {
        let variance = vol * vol * time_to_expiry;
        match convention {
            FxAtmDeltaConvention::Spot | FxAtmDeltaConvention::Forward => {
                forward * (0.5_f64 * variance).exp()
            }
            FxAtmDeltaConvention::PremiumAdjustedSpot
            | FxAtmDeltaConvention::PremiumAdjustedForward => forward * (-0.5_f64 * variance).exp(),
        }
    }
}

impl crate::instruments::common_impl::traits::Instrument for FxOption {
    impl_instrument_base!(crate::pricer::InstrumentType::FxOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    /// `ModelKey::Black76` is the library-wide registry key for lognormal
    /// option pricing. The FX option pricer registered under this key is the
    /// Garman-Kohlhagen *spot-form* model (BSM with domestic rate `r_d` and
    /// foreign rate `r_f` as the carry), which is Black-76 applied to the
    /// CIP forward `F = S·e^{(r_d−r_f)T}` — the key is kept for wire-format
    /// stability, not because pricing uses the forward-form Black-76 inputs.
    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.domestic_discount_curve_id.clone());
        deps.add_discount_curve(self.foreign_discount_curve_id.clone());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                None,
                Some(self.strike),
            ),
        );
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        pricer::compute_pv(self, curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
    }

    fn valuation_details(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> Option<crate::results::ValuationDetails> {
        // FX policy stamp: record whether the spot was a direct quote or
        // triangulated through the FX matrix pivot. FxOption always
        // resolves its spot via the matrix (`FxSpotSource::Matrix`), so
        // there is no `fx_rate_id` short-circuit to consider.
        use finstack_quant_core::money::fx::FxQuery;
        let fx_triangulated = market
            .fx()
            .and_then(|fx| {
                fx.rate(FxQuery::new(self.base_currency, self.quote_currency, as_of))
                    .ok()
            })
            .map(|q| q.triangulated);
        Some(crate::results::ValuationDetails::Fx(
            crate::results::FxValuationDetails { fx_triangulated },
        ))
    }

    crate::impl_focused_pricing_overrides!();
}

impl crate::instruments::common_impl::traits::OptionGreeksProvider for FxOption {
    // Batch the six standard greeks through one canonical pricer call; the
    // metric layer's caching then makes follow-up greek requests free.
    fn option_greeks(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        request: &crate::instruments::common_impl::traits::OptionGreeksRequest,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::OptionGreeks> {
        use crate::instruments::common_impl::traits::{OptionGreekKind, OptionGreeks};

        match request.greek {
            OptionGreekKind::Delta
            | OptionGreekKind::Gamma
            | OptionGreekKind::Vega
            | OptionGreekKind::Theta
            | OptionGreekKind::Rho
            | OptionGreekKind::ForeignRho => {
                let greeks = pricer::compute_greeks(self, market, as_of)?;
                Ok(OptionGreeks {
                    delta: Some(greeks.delta),
                    gamma: Some(greeks.gamma),
                    vega: Some(greeks.vega),
                    theta: Some(greeks.theta),
                    rho_bp: Some(greeks.rho_domestic / 100.0),
                    foreign_rho_bp: Some(greeks.rho_foreign / 100.0),
                    ..OptionGreeks::default()
                })
            }
            OptionGreekKind::Vanna => Ok(OptionGreeks {
                vanna: self.option_vanna(market, as_of)?,
                ..OptionGreeks::default()
            }),
            OptionGreekKind::Volga => Ok(OptionGreeks {
                volga: self.option_volga(market, as_of, request.require_base_pv()?)?,
                ..OptionGreeks::default()
            }),
        }
    }

    fn option_vanna(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        let t = self
            .day_count
            .year_fraction(
                as_of,
                self.expiry,
                finstack_quant_core::dates::DayCountContext::default(),
            )?
            .max(0.0);
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let sigma = self
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility
            .map(Ok)
            .unwrap_or_else(|| {
                let surface = market.get_surface(self.vol_surface_id.as_str())?;
                Ok::<_, finstack_quant_core::Error>(
                    finstack_quant_models::volatility::get_surface_vol_clamped(
                        &surface,
                        t,
                        self.strike,
                    ),
                )
            })?;
        if sigma <= 0.0 {
            return Ok(Some(0.0));
        }
        let delta_sigma = crate::metrics::bump_sizes::VOLATILITY.min(sigma * 0.5);
        let (up, curves_up) = crate::metrics::bump_active_volatility(
            self,
            market,
            self.vol_surface_id.as_str(),
            delta_sigma,
        )?;
        let (down, curves_dn) = crate::metrics::bump_active_volatility(
            self,
            market,
            self.vol_surface_id.as_str(),
            -delta_sigma,
        )?;

        let delta_up = pricer::compute_greeks(&up, &curves_up, as_of)?.delta;
        let delta_dn = pricer::compute_greeks(&down, &curves_dn, as_of)?.delta;

        // Report vanna per **vol point** on the σ axis (consistent with vega
        // and `MetricId::Vanna`): normalize by the bump width expressed in
        // vol points.
        let width = 2.0 * delta_sigma * crate::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((delta_up - delta_dn) / width))
    }

    fn option_volga(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        _base_pv: f64,
    ) -> finstack_quant_core::Result<Option<f64>> {
        let t = self
            .day_count
            .year_fraction(
                as_of,
                self.expiry,
                finstack_quant_core::dates::DayCountContext::default(),
            )?
            .max(0.0);
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let sigma = self
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility
            .map(Ok)
            .unwrap_or_else(|| {
                let surface = market.get_surface(self.vol_surface_id.as_str())?;
                Ok::<_, finstack_quant_core::Error>(
                    finstack_quant_models::volatility::get_surface_vol_clamped(
                        &surface,
                        t,
                        self.strike,
                    ),
                )
            })?;
        if sigma <= 0.0 {
            return Ok(Some(0.0));
        }
        let delta_sigma = crate::metrics::bump_sizes::VOLATILITY.min(sigma * 0.5);
        let (up, curves_up) = crate::metrics::bump_active_volatility(
            self,
            market,
            self.vol_surface_id.as_str(),
            delta_sigma,
        )?;
        let (down, curves_dn) = crate::metrics::bump_active_volatility(
            self,
            market,
            self.vol_surface_id.as_str(),
            -delta_sigma,
        )?;

        // Volga = d²V/dσ² scaled to "per 1% vol move" convention.
        // The raw second derivative d(vega)/dσ is divided by the bump and then
        // multiplied by 0.01 to express the result per 1 vol-point (1%) change,
        // consistent with the vega convention used across the library (see
        // closed_form::greeks::bs_vega which also scales by 0.01).
        let vega_up = pricer::compute_greeks(&up, &curves_up, as_of)?.vega;
        let vega_dn = pricer::compute_greeks(&down, &curves_dn, as_of)?.vega;
        Ok(Some((vega_up - vega_dn) / (2.0 * delta_sigma) * 0.01))
    }
}

crate::impl_empty_cashflow_provider!(
    FxOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn canonical_dependencies_preserve_fx_orientation_and_strike() {
        let option = FxOption::example().expect("example");
        let deps =
            crate::instruments::Instrument::market_dependencies(&option).expect("dependencies");

        assert_eq!(
            deps.curves.discount_curves.as_slice(),
            &[
                option.domestic_discount_curve_id.clone(),
                option.foreign_discount_curve_id.clone(),
            ]
        );
        assert_eq!(deps.fx_pairs.len(), 1);
        assert_eq!(deps.fx_pairs[0].base, option.base_currency);
        assert_eq!(deps.fx_pairs[0].quote, option.quote_currency);
        assert_eq!(deps.volatility_dependencies.len(), 1);
        assert_eq!(
            deps.volatility_dependencies[0].reference_strike,
            Some(option.strike)
        );
    }

    #[test]
    fn builder_rejects_same_base_and_quote_currency() {
        let result = FxOption::builder()
            .id(InstrumentId::new("FXOPT-USDUSD"))
            .base_currency(Currency::USD)
            .quote_currency(Currency::USD)
            .strike(1.0)
            .option_type(OptionType::Call)
            .delta_convention(
                FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                    .expect("valid delta convention"),
            )
            .expiry(crate::instruments::common_impl::example_constants::FAR_EXPIRY)
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("USDUSD-VOL"))
            .attributes(Attributes::new())
            .build();

        assert!(
            result.is_err(),
            "FX option builder must reject identical base and quote currencies"
        );
    }
}
