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
//! as the moneyness dimension (via `vol_surface.value_clamped(t, strike)`). This is
//! a simpler parameterization than the delta-based quoting convention used in
//! professional FX interbank markets, where the vol surface is typically quoted in
//! terms of delta (e.g., 25Δ put, ATM DNS, 25Δ call) and interpolated in delta space.
//!
//! For most use cases (flat or moderately shaped surfaces), strike-based lookup is
//! adequate. For precise smile-sensitive pricing with market-standard FX vol surfaces,
//! a delta-to-strike conversion layer may be needed on top of the surface provider.

use crate::instruments::common_impl::parameters::FxUnderlyingParams;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::PricingOverrides;
use crate::instruments::{ExerciseStyle, OptionType, SettlementType};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
// Pricing/greeks live in pricing engine; keep types minimal.
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_core::Result;

use super::pricer::{self, FxOptionGreeks};
use crate::impl_instrument_base;

fn default_fx_underlying(base_currency: Currency, quote_currency: Currency) -> FxUnderlyingParams {
    // Fall back to currency-aware OIS curves instead of hardwiring USD legs.
    let domestic = CurveId::new(format!("{}-OIS", quote_currency));
    let foreign = CurveId::new(format!("{}-OIS", base_currency));
    FxUnderlyingParams::new(base_currency, quote_currency, domestic, foreign)
}

/// FX option instrument (Garman-Kohlhagen model)
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = FxOption::validate)]
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
    /// Exercise style (European or American)
    #[serde(default)]
    #[builder(default)]
    pub exercise_style: ExerciseStyle,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Day count convention
    #[serde(default = "crate::serde_defaults::day_count_act365f")]
    #[builder(default = finstack_core::dates::DayCount::Act365F)]
    pub day_count: finstack_core::dates::DayCount,
    /// Notional amount in base currency
    pub notional: Money,
    /// Settlement type (physical or cash)
    #[serde(default = "crate::serde_defaults::settlement_cash")]
    #[builder(default = SettlementType::Cash)]
    pub settlement: SettlementType,
    /// Domestic currency discount curve ID
    pub domestic_discount_curve_id: CurveId,
    /// Foreign currency discount curve ID
    pub foreign_discount_curve_id: CurveId,
    /// FX volatility surface ID
    pub vol_surface_id: CurveId,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

// Implement CurveDependencies for DV01 calculator
// FxOption uses both domestic and foreign curves for Garman-Kohlhagen pricing
impl crate::instruments::common_impl::traits::CurveDependencies for FxOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.domestic_discount_curve_id.clone())
            .discount(self.foreign_discount_curve_id.clone())
            .build()
    }
}

/// Delta conventions relevant for FX ATM DNS strikes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
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
    pub fn validate(&self) -> finstack_core::Result<()> {
        crate::instruments::common_impl::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FxOption",
        )?;
        if self.notional.currency() != self.base_currency {
            return Err(finstack_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: self.notional.currency(),
            });
        }
        Ok(())
    }

    fn price_internal(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        pricer::compute_pv(self, market, as_of)
    }

    fn greeks_internal(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> Result<FxOptionGreeks> {
        pricer::compute_greeks(self, market, as_of)
    }

    fn implied_vol_internal(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
        target_price: f64,
        initial_guess: Option<f64>,
    ) -> Result<f64> {
        pricer::implied_vol(self, curves, as_of, target_price, initial_guess)
    }

    /// Create a canonical example FX option for testing and documentation.
    ///
    /// Returns an EUR/USD call expiring on the project-wide stable example
    /// epoch (`crate::instruments::common_impl::EXAMPLE_FAR_EXPIRY`). The
    /// example is intentionally future-dated so docs and demos hit the live
    /// pricing path rather than the expired-option intrinsic-only branch.
    pub fn example() -> finstack_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("FXOPT-EURUSD-CALL"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .strike(1.12)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(crate::instruments::common_impl::example_constants::FAR_EXPIRY)
            .day_count(DayCount::Act365F)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .settlement(SettlementType::Cash)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
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
    ) -> finstack_core::Result<Self> {
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
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .day_count(finstack_core::dates::DayCount::Act365F)
            .notional(notional)
            .settlement(SettlementType::Cash)
            .domestic_discount_curve_id(fx_underlying.domestic_discount_curve_id.to_owned())
            .foreign_discount_curve_id(fx_underlying.foreign_discount_curve_id.to_owned())
            .vol_surface_id(vol_surface_id.into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
    }

    /// Create a European option from trade date using joint calendar spot roll and tenor.
    ///
    /// `spot_lag_days` defaults to T+2 in most markets. The expiry is rolled on the
    /// joint base/quote calendars using the provided business day convention.
    #[allow(clippy::too_many_arguments)]
    pub fn european_from_trade_date(
        id: impl Into<InstrumentId>,
        base_currency: Currency,
        quote_currency: Currency,
        strike: f64,
        trade_date: Date,
        expiry_tenor_days: i64,
        notional: Money,
        vol_surface_id: impl Into<CurveId>,
        base_calendar_id: Option<String>,
        quote_calendar_id: Option<String>,
        spot_lag_days: u32,
        bdc: finstack_core::dates::BusinessDayConvention,
        option_type: OptionType,
    ) -> finstack_core::Result<Self> {
        use crate::instruments::common_impl::fx_dates::{
            adjust_joint_calendar, fx_spot_date_for_pair,
        };
        // CLS-consistent spot roll: a US holiday on an intermediate day does not
        // delay a USD pair's spot date (2026-06-09 core quant review, FX spot
        // finding).
        let spot_settle = fx_spot_date_for_pair(
            trade_date,
            spot_lag_days,
            base_currency,
            quote_currency,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;
        let expiry = adjust_joint_calendar(
            spot_settle + time::Duration::days(expiry_tenor_days),
            bdc,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;

        let fx_underlying = if quote_currency == Currency::USD && base_currency == Currency::EUR {
            FxUnderlyingParams::usd_eur()
        } else if quote_currency == Currency::USD && base_currency == Currency::GBP {
            FxUnderlyingParams::gbp_usd()
        } else {
            super::types::default_fx_underlying(base_currency, quote_currency)
        };

        Self::builder()
            .id(id.into())
            .base_currency(fx_underlying.base_currency)
            .quote_currency(fx_underlying.quote_currency)
            .strike(strike)
            .option_type(option_type)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .day_count(finstack_core::dates::DayCount::Act365F)
            .notional(notional)
            .settlement(SettlementType::Cash)
            .domestic_discount_curve_id(fx_underlying.domestic_discount_curve_id.to_owned())
            .foreign_discount_curve_id(fx_underlying.foreign_discount_curve_id.to_owned())
            .vol_surface_id(vol_surface_id.into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
    }

    /// Compute present value using Garman–Kohlhagen model.
    pub fn base_value(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        self.price_internal(market, as_of)
    }

    /// Solve for implied volatility.
    pub fn implied_vol(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
        target_price: f64,
        initial_guess: Option<f64>,
    ) -> Result<f64> {
        self.implied_vol_internal(curves, as_of, target_price, initial_guess)
    }

    /// Calculate the at-the-money forward (ATMF) strike.
    ///
    /// The ATMF strike is the forward FX rate, calculated using covered interest
    /// rate parity:
    ///
    /// ```text
    /// K_ATMF = S × DF_foreign(T) / DF_domestic(T)
    /// ```
    ///
    /// This is **not** the same as the Delta-Neutral Straddle (DNS) strike used
    /// in professional FX markets. For precise ATM DNS, use [`atm_dns_strike_for_convention`](Self::atm_dns_strike_for_convention).
    ///
    /// # Arguments
    ///
    /// * `spot` - Current spot FX rate (domestic per foreign)
    /// * `df_domestic` - Domestic discount factor to expiry
    /// * `df_foreign` - Foreign discount factor to expiry
    ///
    /// # Example
    ///
    /// ```ignore
    /// let spot = 1.10; // EUR/USD
    /// let df_domestic = 0.97; // USD discount factor
    /// let df_foreign = 0.98; // EUR discount factor
    ///
    /// let k_atmf = FxOption::atm_forward_strike(spot, df_domestic, df_foreign);
    /// // k_atmf ≈ 1.111 (forward premium for EUR vs USD)
    /// ```
    pub fn atm_forward_strike(spot: f64, df_domestic: f64, df_foreign: f64) -> f64 {
        spot * df_foreign / df_domestic
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
    /// * `forward` - Forward FX rate (use [`atm_forward_strike`](Self::atm_forward_strike))
    /// * `vol` - ATM volatility (decimal, e.g., 0.10 for 10%)
    /// * `time_to_expiry` - Time to expiry in years
    /// * `convention` - ATM delta convention determining the DNS formula variant
    ///
    /// # Example
    ///
    /// ```ignore
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
    /// ```
    ///
    /// # References
    ///
    /// - Wystup, U. (2006). *FX Options and Structured Products*. Chapter 2.
    /// - Clark, I. J. (2011). *Foreign Exchange Option Pricing*. Chapter 3.
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
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        deps.add_vol_surface_id(self.vol_surface_id.as_str());
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        self.price_internal(curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_core::dates::Date> {
        None
    }

    fn valuation_details(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Option<crate::results::ValuationDetails> {
        // FX policy stamp: record whether the spot was a direct quote or
        // triangulated through the FX matrix pivot. FxOption always
        // resolves its spot via the matrix (`FxSpotSource::Matrix`), so
        // there is no `fx_rate_id` short-circuit to consider.
        use finstack_core::money::fx::FxQuery;
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

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl crate::instruments::common_impl::traits::OptionGreeksProvider for FxOption {
    // Override `option_greeks` to batch the 6 standard greeks via a single
    // `greeks_internal` call; the metric layer's caching then makes follow-up
    // greek requests free.
    fn option_greeks(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
        request: &crate::instruments::common_impl::traits::OptionGreeksRequest,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::OptionGreeks> {
        use crate::instruments::common_impl::traits::{OptionGreekKind, OptionGreeks};

        match request.greek {
            OptionGreekKind::Delta
            | OptionGreekKind::Gamma
            | OptionGreekKind::Vega
            | OptionGreekKind::Theta
            | OptionGreekKind::Rho
            | OptionGreekKind::ForeignRho => {
                let greeks = self.greeks_internal(market, as_of)?;
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
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        let t = self
            .day_count
            .year_fraction(
                as_of,
                self.expiry,
                finstack_core::dates::DayCountContext::default(),
            )?
            .max(0.0);
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        // Match the existing FX option vanna/volga metric conventions (tests rely on this):
        // - bump a single surface point by ±1% (relative to the surface value at (t, K))
        // - divide by the corresponding absolute Δσ = sigma * bump_pct
        let surf = market.get_surface(self.vol_surface_id.as_str())?;
        let sigma = surf.value_clamped(t, self.strike);
        if sigma <= 0.0 {
            return Ok(Some(0.0));
        }

        let vol_bump_pct: f64 = 0.01;
        let delta_sigma = (sigma * vol_bump_pct).abs().max(1e-12);

        let curves_up = {
            let bumped = surf.bump_point(t, self.strike, vol_bump_pct)?;
            market.clone().insert_surface(bumped)
        };
        let curves_dn = {
            let bumped = surf.bump_point(t, self.strike, -vol_bump_pct)?;
            market.clone().insert_surface(bumped)
        };

        let delta_up = self.greeks_internal(&curves_up, as_of)?.delta;
        let delta_dn = self.greeks_internal(&curves_dn, as_of)?.delta;

        Ok(Some((delta_up - delta_dn) / (2.0 * delta_sigma)))
    }

    fn option_volga(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
        _base_pv: f64,
    ) -> finstack_core::Result<Option<f64>> {
        let t = self
            .day_count
            .year_fraction(
                as_of,
                self.expiry,
                finstack_core::dates::DayCountContext::default(),
            )?
            .max(0.0);
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let surf = market.get_surface(self.vol_surface_id.as_str())?;
        let sigma = surf.value_clamped(t, self.strike);
        if sigma <= 0.0 {
            return Ok(Some(0.0));
        }

        let vol_bump_pct: f64 = 0.01;
        let delta_sigma = (sigma * vol_bump_pct).abs().max(1e-12);

        let curves_up = {
            let bumped = surf.bump_point(t, self.strike, vol_bump_pct)?;
            market.clone().insert_surface(bumped)
        };
        let curves_dn = {
            let bumped = surf.bump_point(t, self.strike, -vol_bump_pct)?;
            market.clone().insert_surface(bumped)
        };

        // Volga = d²V/dσ² scaled to "per 1% vol move" convention.
        // The raw second derivative d(vega)/dσ is divided by the bump and then
        // multiplied by 0.01 to express the result per 1 vol-point (1%) change,
        // consistent with the vega convention used across the library (see
        // closed_form::greeks::bs_vega which also scales by 0.01).
        let vega_up = self.greeks_internal(&curves_up, as_of)?.vega;
        let vega_dn = self.greeks_internal(&curves_dn, as_of)?.vega;
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
    fn builder_rejects_same_base_and_quote_currency() {
        let result = FxOption::builder()
            .id(InstrumentId::new("FXOPT-USDUSD"))
            .base_currency(Currency::USD)
            .quote_currency(Currency::USD)
            .strike(1.0)
            .option_type(OptionType::Call)
            .expiry(crate::instruments::common_impl::example_constants::FAR_EXPIRY)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("USDUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build();

        assert!(
            result.is_err(),
            "FX option builder must reject identical base and quote currencies"
        );
    }
}
