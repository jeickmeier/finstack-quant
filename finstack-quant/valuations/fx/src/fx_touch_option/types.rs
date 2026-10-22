//! FX touch option (American binary option) instrument definition.

use super::pricer;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations_core::impl_instrument_base;

/// Touch type: one-touch (pays if barrier is hit) or no-touch (pays if not hit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TouchType {
    /// Pays if the spot rate touches the barrier at any time before expiry.
    OneTouch,
    /// Pays if the spot rate does NOT touch the barrier before expiry.
    NoTouch,
}

use finstack_quant_core::types::{BarrierDirection, PayoutTiming};

/// FX touch option (American binary option).
///
/// Touch options pay a fixed amount if the spot rate touches a barrier
/// level at any time before expiry:
/// - One-touch: pays if barrier is touched
/// - No-touch: pays if barrier is NOT touched
///
/// # Pricing
///
/// Uses closed-form pricing for continuous monitoring (Rubinstein & Reiner 1991):
///
/// **Down-and-in one-touch (S > H, pay at expiry)**:
/// ```text
/// P = e^{-r_d T} × [(S/H)^{-(μ+λ)} × N(η·z) + (S/H)^{-(μ-λ)} × N(η·z')]
/// ```
///
/// where:
/// - μ = (r_d - r_f - σ²/2) / σ²
/// - λ = sqrt(μ² + 2r_d/σ²)
/// - z = ln(H/S)/(σ√T) + λσ√T
/// - z' = ln(H/S)/(σ√T) - λσ√T
/// - η = +1 for down barrier, -1 for up barrier
///
/// **No-touch**: P_no_touch = e^{-r_d T} × payout - P_one_touch
///
/// # References
///
/// - Rubinstein, M., & Reiner, E. (1991). "Unscrambling the Binary Code."
///   *Risk Magazine*, 4(9), 75-83. `docs/REFERENCES.md#reiner-rubinstein-1991`
/// - Wystup, U. (2006). *FX Options and Structured Products*. Wiley. `docs/REFERENCES.md#wystup-fx-options`
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
pub struct FxTouchOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Base currency (foreign currency)
    pub base_currency: Currency,
    /// Quote currency (domestic currency)
    pub quote_currency: Currency,
    /// Barrier level (exchange rate that triggers the touch)
    pub barrier: f64,
    /// Touch type (one-touch or no-touch)
    pub touch_type: TouchType,
    /// Barrier direction (up or down)
    pub barrier_direction: BarrierDirection,
    /// Fixed payout amount
    pub payout_amount: Money,
    /// Payout timing (at hit or at expiry)
    pub payout_timing: PayoutTiming,
    /// Option expiry date
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// First date on which barrier monitoring is active. When set, a live
    /// valuation after this date requires `observed_barrier_breached`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub monitoring_start_date: Option<Date>,
    /// Day count convention (defaults to ACT/365F).
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::day_count_act365f")]
    #[builder(default = finstack_quant_core::dates::DayCount::Act365F)]
    pub day_count: DayCount,
    /// Domestic currency discount curve ID
    pub domestic_discount_curve_id: CurveId,
    /// Foreign currency discount curve ID
    pub foreign_discount_curve_id: CurveId,
    /// FX volatility surface ID
    pub vol_surface_id: CurveId,
    /// Observed barrier event state for expired valuations.
    ///
    /// `Some(true)` means the barrier was touched during the option life,
    /// `Some(false)` means it was observed not to have touched, and `None`
    /// means the historical touch state is unavailable.
    ///
    /// This is only required once the option has expired. Without it, a
    /// touched-and-reverted path cannot be distinguished from an untouched path
    /// using the terminal spot alone.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_barrier_breached: Option<bool>,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

impl FxTouchOption {
    /// Validate FX touch option input invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        finstack_quant_core::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FxTouchOption",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.barrier,
            "FxTouchOption barrier",
        )?;
        finstack_quant_core::validation::validate_f64_finite(
            self.barrier,
            "FxTouchOption barrier",
        )?;
        finstack_quant_core::validation::validate_f64_non_negative(
            self.payout_amount.amount(),
            "FxTouchOption payout_amount",
        )?;
        finstack_quant_core::validation::validate_money_finite(
            self.payout_amount,
            "FxTouchOption payout_amount",
        )?;
        // Pricing discounts the payout at the domestic (quote-currency) rate;
        // a base-currency payout would need the foreign-cash touch formula.
        // Reject rather than silently mispricing.
        if self.payout_amount.currency() != self.quote_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.quote_currency,
                actual: self.payout_amount.currency(),
            });
        }
        if self.touch_type == TouchType::NoTouch && self.payout_timing == PayoutTiming::AtHit {
            return Err(finstack_quant_core::Error::Validation(
                "FxTouchOption no-touch payout_timing must be at_expiry".to_string(),
            ));
        }
        let start = self.monitoring_start_date.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "FxTouchOption requires monitoring_start_date".to_string(),
            )
        })?;
        if start > self.expiry {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxTouchOption monitoring_start_date ({start}) must not be after expiry ({})",
                self.expiry
            )));
        }
        Ok(())
    }

    /// Create a canonical example FX touch option expiring on the
    /// project-wide stable example epoch.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        Self::builder()
            .id(InstrumentId::new("FXTOUCH-EURUSD-OT"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier(1.05)
            .touch_type(TouchType::OneTouch)
            .barrier_direction(BarrierDirection::Down)
            .payout_amount(Money::from((1_000_000_i64, Currency::USD)))
            .payout_timing(PayoutTiming::AtExpiry)
            .expiry(finstack_quant_valuations_core::instruments::common_impl::example_constants::FAR_EXPIRY)
            .monitoring_start_date_opt(Some(date!(2024 - 01 - 01)))
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
    }
}

impl finstack_quant_valuations_core::instruments::Instrument for FxTouchOption {
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::FxTouchOption,
        crate::FX
    );

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> finstack_quant_valuations_core::pricer::ModelKey {
        finstack_quant_valuations_core::pricer::ModelKey::Black76
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.domestic_discount_curve_id.clone());
        deps.add_discount_curve(self.foreign_discount_curve_id.clone());
        deps.add_volatility_dependency(
            finstack_quant_valuations_core::instruments::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                None,
                Some(self.barrier),
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

    /// Roll the observed touch state with spot held at its `as_of` level.
    ///
    /// When the roll passes `monitoring_start_date` (or reaches expiry) and
    /// no state is recorded, the copy records the barrier as touched exactly
    /// when the `as_of` FX spot is at or beyond it in `barrier_direction`.
    ///
    /// # Arguments
    /// * `market` - Market at `as_of` supplying the FX spot held over the roll.
    /// * `as_of` - Valuation date the roll starts from.
    /// * `rolled_date` - Date theta reprices at, capped at expiry.
    fn theta_observed_state(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
        rolled_date: Date,
    ) -> finstack_quant_core::Result<
        Option<Box<dyn finstack_quant_valuations_core::instruments::Instrument>>,
    > {
        if self.observed_barrier_breached.is_some() || as_of > self.expiry {
            return Ok(None);
        }
        let monitored = rolled_date >= self.expiry
            || self
                .monitoring_start_date
                .is_some_and(|start| start < rolled_date);
        if !monitored {
            return Ok(None);
        }
        let spot = pricer::fx_spot(self, market, as_of)?;
        let touched = match self.barrier_direction {
            BarrierDirection::Up => spot >= self.barrier,
            BarrierDirection::Down => spot <= self.barrier,
        };
        let mut observed = self.clone();
        observed.observed_barrier_breached = Some(touched);
        Ok(Some(Box::new(observed)))
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

// Touch options use finite-difference Greeks (barrier discontinuities make
// analytical Greeks unreliable near the barrier).

impl finstack_quant_valuations_core::instruments::OptionGreeksProvider for FxTouchOption {
    fn option_greeks(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        request: &finstack_quant_valuations_core::instruments::OptionGreeksRequest,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::OptionGreeks>
    {
        let mut greeks = finstack_quant_valuations_core::instruments::OptionGreeks::default();
        match request.greek {
            finstack_quant_valuations_core::instruments::OptionGreekKind::Delta => {
                greeks.delta = self.option_delta(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Gamma => {
                greeks.gamma = self.option_gamma(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Vega => {
                greeks.vega = self.option_vega(market, as_of, request.bumps)?
            }
            finstack_quant_valuations_core::instruments::OptionGreekKind::Rho => {
                greeks.rho_bp = self.option_rho_bp(market, as_of, request.bumps)?
            }
            _ => {}
        }
        Ok(greeks)
    }
}
impl FxTouchOption {
    /// Quote-per-base spot read from the FX matrix, as touch pricing reads it.
    fn matrix_spot(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok(market
            .fx_required()?
            .rate(finstack_quant_core::money::fx::FxQuery::new(
                self.base_currency,
                self.quote_currency,
                as_of,
            ))?
            .rate)
    }

    /// Market with the FX matrix spot bumped relatively by `bump_pct`; the
    /// base market is left untouched.
    fn spot_bumped_market(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bump_pct: f64,
    ) -> finstack_quant_core::Result<finstack_quant_core::market_data::context::MarketContext> {
        crate::shared::fx_matrix_bumped_market(
            market,
            self.base_currency,
            self.quote_currency,
            as_of,
            bump_pct,
        )
    }

    fn option_delta(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        if crate::shared::greeks_expired(self.day_count, as_of, self.expiry)? {
            return Ok(Some(0.0));
        }

        let current_spot = self.matrix_spot(market, as_of)?;
        if current_spot * bumps.spot_bump_decimal <= 0.0 {
            return Ok(Some(0.0));
        }
        Ok(Some(crate::shared::spot_delta(
            self,
            as_of,
            current_spot,
            bumps.spot_bump_decimal,
            |pct| self.spot_bumped_market(market, as_of, pct),
        )?))
    }
    fn option_gamma(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use finstack_quant_valuations_core::instruments::Instrument;

        if crate::shared::greeks_expired(self.day_count, as_of, self.expiry)? {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let current_spot = self.matrix_spot(market, as_of)?;
        Ok(Some(crate::shared::spot_gamma(
            self,
            as_of,
            base_pv,
            current_spot,
            bumps.spot_bump_decimal,
            |pct| self.spot_bumped_market(market, as_of, pct),
        )?))
    }
    fn option_vega(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use finstack_quant_valuations_core::instruments::Instrument;

        if crate::shared::greeks_expired(self.day_count, as_of, self.expiry)? {
            return Ok(Some(0.0));
        }

        if self
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility
            == Some(0.0)
        {
            let bump = bumps.vol_bump_decimal;
            let (up, market_up) = finstack_quant_valuations_core::metrics::bump_active_volatility(
                self,
                market,
                self.vol_surface_id.as_str(),
                bump,
            )?;
            return Ok(Some(
                (up.value(&market_up, as_of)?.amount() - self.value(market, as_of)?.amount())
                    / (bump * finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL),
            ));
        }

        let (vol_bump, (up, market_up), (down, market_down)) = crate::shared::active_vol_scenarios(
            self,
            &self.instrument_pricing_overrides,
            market,
            self.vol_surface_id.as_str(),
            bumps.vol_bump_decimal,
        )?;
        let pv_up = up.value(&market_up, as_of)?.amount();
        let pv_down = down.value(&market_down, as_of)?.amount();
        let width =
            2.0 * vol_bump * finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((pv_up - pv_down) / width))
    }
    fn option_rho_bp(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bumps: finstack_quant_valuations_core::instruments::GreekBumps,
    ) -> finstack_quant_core::Result<Option<f64>> {
        if crate::shared::greeks_expired(self.day_count, as_of, self.expiry)? {
            return Ok(Some(0.0));
        }

        Ok(Some(crate::shared::discount_rho_bp(
            self,
            market,
            as_of,
            &self.domestic_discount_curve_id,
            bumps.rate_bump_bp,
        )?))
    }
}

finstack_quant_valuations_core::impl_empty_cashflow_provider!(
    FxTouchOption,
    finstack_quant_cashflows::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_type_serde_accepts_only_canonical_snake_case() {
        for (variant, canonical, rejected) in [
            (TouchType::OneTouch, "one_touch", "OneTouch"),
            (TouchType::NoTouch, "no_touch", "NoTouch"),
        ] {
            assert_eq!(
                serde_json::to_string(&variant).expect("serialize touch type"),
                format!("\"{canonical}\"")
            );
            assert!(serde_json::from_str::<TouchType>(&format!("\"{rejected}\"")).is_err());
        }
    }

    // Validation tests

    fn base_touch_builder() -> crate::fx_touch_option::types::FxTouchOptionBuilder {
        use finstack_quant_core::types::{CurveId, InstrumentId};
        FxTouchOption::builder()
            .id(InstrumentId::new("FXTOUCH-VALID"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier(1.05)
            .touch_type(TouchType::OneTouch)
            .barrier_direction(BarrierDirection::Down)
            .payout_amount(Money::from((1_000_000_i64, Currency::USD)))
            .payout_timing(PayoutTiming::AtExpiry)
            .monitoring_start_date(time::macros::date!(2024 - 01 - 01))
            .expiry(time::macros::date!(2027 - 01 - 15))
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(finstack_quant_core::types::Attributes::new())
    }

    #[test]
    fn validation_valid_touch_option_builds_ok() {
        assert!(base_touch_builder().build().is_ok());
    }

    #[test]
    fn validation_rejects_base_currency_payout() {
        // Pricing discounts the payout at the domestic (quote) rate; a base
        // (foreign) currency payout would be silently mispriced.
        let result = base_touch_builder()
            .payout_amount(Money::from((1_000_000_i64, Currency::EUR)))
            .build();
        assert!(
            result.is_err(),
            "FxTouchOption must reject a payout_amount not in the quote currency"
        );
    }

    #[test]
    fn validation_rejects_same_currencies() {
        let result = base_touch_builder()
            .base_currency(Currency::USD)
            .quote_currency(Currency::USD)
            .build();
        assert!(
            result.is_err(),
            "FxTouchOption must reject identical base and quote currencies"
        );
    }

    #[test]
    fn validation_rejects_non_positive_barrier_level() {
        let result = base_touch_builder().barrier(0.0).build();
        assert!(result.is_err(), "FxTouchOption must reject barrier = 0");
        let result = base_touch_builder().barrier(-1.0).build();
        assert!(
            result.is_err(),
            "FxTouchOption must reject negative barrier"
        );
    }

    #[test]
    fn validation_rejects_nan_barrier_level() {
        let result = base_touch_builder().barrier(f64::NAN).build();
        assert!(result.is_err(), "FxTouchOption must reject NaN barrier");
    }

    #[test]
    fn validation_rejects_inf_barrier_level() {
        let result = base_touch_builder().barrier(f64::INFINITY).build();
        assert!(
            result.is_err(),
            "FxTouchOption must reject infinite barrier"
        );
    }

    #[test]
    fn validation_rejects_negative_payout_amount() {
        let result = base_touch_builder()
            .payout_amount(Money::from((-1_i64, Currency::USD)))
            .build();
        assert!(
            result.is_err(),
            "FxTouchOption must reject negative payout_amount"
        );
    }

    #[test]
    fn validation_rejects_nan_payout_amount() {
        // Construction rejects NaN before the builder can receive a payout amount.
        assert!(Money::new(f64::NAN, Currency::USD).is_err());
    }

    #[test]
    fn validation_accepts_zero_payout_amount() {
        // Zero payout is legitimate (no-touch semantics allow it)
        let result = base_touch_builder()
            .payout_amount(Money::from((0_i64, Currency::USD)))
            .build();
        assert!(
            result.is_ok(),
            "FxTouchOption must accept zero payout_amount"
        );
    }

    /// Item 8 regression: the FD spot bumps in `option_delta`/`option_gamma`
    /// rebuild `FxMatrix` via `provider().clone()` (an `Arc` clone — the
    /// provider is shared). This test pins that the bumped scenarios do NOT
    /// corrupt the base market's FX spot: `FxMatrix::set_quote` writes a
    /// matrix-local explicit quote and `FxMatrix` never mutates the provider.
    #[test]
    fn touch_fd_greeks_do_not_corrupt_base_market_fx_spot() {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::Date;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::surfaces::VolSurface;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::money::fx::{FxMatrix, FxQuery, SimpleFxProvider};
        use finstack_quant_valuations_core::instruments::{Instrument, OptionGreeksProvider};
        use std::sync::Arc;
        use time::Month;

        let as_of = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        let expiry = Date::from_calendar_date(2026, Month::January, 2).expect("date");

        let usd = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
            .build()
            .expect("usd curve");
        let eur = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.02_f64).exp())])
            .build()
            .expect("eur curve");
        let surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0])
            .strikes(&[1.05])
            .row(&[0.11])
            .build()
            .expect("vol surface");
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        let market = MarketContext::new()
            .insert(usd)
            .insert(eur)
            .insert_surface(surface)
            .insert_fx(FxMatrix::new(provider));

        let touch = FxTouchOption::builder()
            .id(InstrumentId::new("FXTOUCH-CORRUPT-CHECK"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier(1.05)
            .touch_type(TouchType::OneTouch)
            .barrier_direction(BarrierDirection::Down)
            .payout_amount(Money::from((1_000_000_i64, Currency::USD)))
            .payout_timing(PayoutTiming::AtExpiry)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
            .expect("touch option");

        let spot_before = market
            .fx()
            .expect("fx")
            .rate(FxQuery::new(Currency::EUR, Currency::USD, as_of))
            .expect("spot")
            .rate;

        // Running the FD gamma exercises the bump-and-rebuild path.
        let _gamma = OptionGreeksProvider::option_greeks(
            &touch,
            &market,
            as_of,
            &finstack_quant_valuations_core::instruments::OptionGreeksRequest {
                greek: finstack_quant_valuations_core::instruments::OptionGreekKind::Gamma,
                base_pv: None,
                bumps: finstack_quant_valuations_core::instruments::GreekBumps::default(),
            },
        )
        .map(|greeks| greeks.gamma)
        .expect("gamma");

        let spot_after = market
            .fx()
            .expect("fx")
            .rate(FxQuery::new(Currency::EUR, Currency::USD, as_of))
            .expect("spot")
            .rate;
        assert!(
            (spot_before - spot_after).abs() < 1e-15,
            "base market FX spot must be unchanged after FD bumps: \
             before={spot_before} after={spot_after}"
        );

        // The base PV must also be reproducible (unchanged) after the bumps.
        let pv1 = touch.value(&market, as_of).expect("pv1").amount();
        let pv2 = touch.value(&market, as_of).expect("pv2").amount();
        assert!(
            (pv1 - pv2).abs() < 1e-9,
            "base PV must be stable after FD Greeks ran: pv1={pv1} pv2={pv2}"
        );

        let vol_bump = 0.01;
        let up = finstack_quant_valuations_core::metrics::bump_surface_vol_absolute(
            &market,
            touch.vol_surface_id.as_str(),
            vol_bump,
        )
        .expect("up vol bump");
        let down = finstack_quant_valuations_core::metrics::bump_surface_vol_absolute(
            &market,
            touch.vol_surface_id.as_str(),
            -vol_bump,
        )
        .expect("down vol bump");
        let expected_vega = (touch.value(&up, as_of).expect("up pv").amount()
            - touch.value(&down, as_of).expect("down pv").amount())
            / (2.0
                * vol_bump
                * finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL);
        let vega = OptionGreeksProvider::option_greeks(
            &touch,
            &market,
            as_of,
            &finstack_quant_valuations_core::instruments::OptionGreeksRequest {
                greek: finstack_quant_valuations_core::instruments::OptionGreekKind::Vega,
                base_pv: None,
                bumps: finstack_quant_valuations_core::instruments::GreekBumps::default(),
            },
        )
        .map(|greeks| greeks.vega)
        .expect("vega")
        .expect("touch vega");
        assert!(
            (vega - expected_vega).abs() <= 1e-9 * expected_vega.abs().max(1.0),
            "touch vega must be cash PV per vol point: expected {expected_vega}, got {vega}"
        );
    }
}
