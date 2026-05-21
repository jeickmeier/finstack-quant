//! Quanto option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::OptionType;
use crate::instruments::PricingOverrides;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId, PriceId};

/// Quanto option instrument.
///
/// Quanto options have payoffs that depend on an underlying asset in one currency
/// but are settled in another currency, creating FX exposure.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = QuantoOption::validate)]
#[serde(deny_unknown_fields, try_from = "QuantoOptionUnchecked")]
pub struct QuantoOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying equity ticker symbol
    pub underlying_ticker: crate::instruments::equity::spot::Ticker,
    /// Strike price for equity option
    pub equity_strike: Money,
    /// Option type (call or put)
    pub option_type: OptionType,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Strike-equivalent domestic reference notional.
    ///
    /// When `underlying_quantity` and `payoff_fx_rate` are supplied, this must
    /// equal `underlying_quantity * payoff_fx_rate * equity_strike.amount()`.
    pub notional: Money,
    /// Number of underlying units covered by the option payoff.
    ///
    /// Quanto pricing is performed per unit of the underlying in the base currency.
    /// This quantity converts the per-unit price into contract-level exposure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub underlying_quantity: Option<f64>,
    /// Fixed payoff FX conversion rate from base-currency payoff into quote currency.
    ///
    /// Example: for a JPY-underlying option settled in USD at a fixed 140 JPY/USD
    /// conversion, `payoff_fx_rate` is `1.0 / 140.0` USD per JPY.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub payoff_fx_rate: Option<f64>,
    /// Base currency (equity denomination)
    pub base_currency: Currency,
    /// Quote currency (payment/settlement currency)
    pub quote_currency: Currency,
    /// Correlation between equity price and FX rate
    pub correlation: f64, // Correlation between equity and FX
    /// Day count convention
    pub day_count: finstack_core::dates::DayCount,
    /// Discount curve ID (domestic currency)
    pub domestic_discount_curve_id: CurveId,
    /// Discount curve ID (foreign currency)
    pub foreign_discount_curve_id: CurveId,
    /// Equity spot price identifier
    pub spot_id: PriceId,
    /// Equity volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional dividend yield curve ID
    pub div_yield_id: Option<CurveId>,
    /// Optional FX rate identifier
    pub fx_rate_id: Option<String>,
    /// Optional FX volatility surface ID
    pub fx_vol_id: Option<CurveId>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct QuantoOptionUnchecked {
    /// Unique instrument identifier.
    id: InstrumentId,
    /// Underlying equity ticker symbol.
    underlying_ticker: crate::instruments::equity::spot::Ticker,
    /// Strike price for equity option.
    equity_strike: Money,
    /// Option type (call or put).
    option_type: OptionType,
    /// Option expiry date.
    #[schemars(with = "String")]
    expiry: Date,
    /// Strike-equivalent domestic reference notional.
    notional: Money,
    /// Number of underlying units covered by the option payoff.
    #[serde(default)]
    underlying_quantity: Option<f64>,
    /// Fixed payoff FX conversion rate from base-currency payoff into quote currency.
    #[serde(default)]
    payoff_fx_rate: Option<f64>,
    /// Base currency (equity denomination).
    base_currency: Currency,
    /// Quote currency (payment/settlement currency).
    quote_currency: Currency,
    /// Correlation between equity price and FX rate.
    correlation: f64,
    /// Day count convention.
    day_count: finstack_core::dates::DayCount,
    /// Discount curve ID (domestic currency).
    domestic_discount_curve_id: CurveId,
    /// Discount curve ID (foreign currency).
    foreign_discount_curve_id: CurveId,
    /// Equity spot price identifier.
    spot_id: PriceId,
    /// Equity volatility surface ID.
    vol_surface_id: CurveId,
    /// Optional dividend yield curve ID.
    #[serde(default)]
    div_yield_id: Option<CurveId>,
    /// Optional FX rate identifier.
    #[serde(default)]
    fx_rate_id: Option<String>,
    /// Optional FX volatility surface ID.
    #[serde(default)]
    fx_vol_id: Option<CurveId>,
    /// Pricing overrides (manual price, yield, spread).
    #[serde(default)]
    pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping.
    attributes: Attributes,
}

impl TryFrom<QuantoOptionUnchecked> for QuantoOption {
    type Error = finstack_core::Error;

    fn try_from(value: QuantoOptionUnchecked) -> std::result::Result<Self, Self::Error> {
        let quanto = Self {
            id: value.id,
            underlying_ticker: value.underlying_ticker,
            equity_strike: value.equity_strike,
            option_type: value.option_type,
            expiry: value.expiry,
            notional: value.notional,
            underlying_quantity: value.underlying_quantity,
            payoff_fx_rate: value.payoff_fx_rate,
            base_currency: value.base_currency,
            quote_currency: value.quote_currency,
            correlation: value.correlation,
            day_count: value.day_count,
            domestic_discount_curve_id: value.domestic_discount_curve_id,
            foreign_discount_curve_id: value.foreign_discount_curve_id,
            spot_id: value.spot_id,
            vol_surface_id: value.vol_surface_id,
            div_yield_id: value.div_yield_id,
            fx_rate_id: value.fx_rate_id,
            fx_vol_id: value.fx_vol_id,
            pricing_overrides: value.pricing_overrides,
            attributes: value.attributes,
        };
        quanto.validate()?;
        Ok(quanto)
    }
}

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for QuantoOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.domestic_discount_curve_id.clone())
            .discount(self.foreign_discount_curve_id.clone())
            .build()
    }
}

impl QuantoOption {
    /// Create a canonical example quanto equity option (Nikkei in USD)
    /// expiring on the project-wide stable example epoch.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use finstack_core::dates::DayCount;
        QuantoOption::builder()
            .id(InstrumentId::new("QUANTO-NKY-USD-CALL"))
            .underlying_ticker("NKY".to_string())
            .equity_strike(Money::new(35000.0, Currency::JPY))
            .option_type(crate::instruments::OptionType::Call)
            .expiry(crate::instruments::common_impl::example_constants::FAR_EXPIRY)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .underlying_quantity_opt(Some(4_000.0))
            .payoff_fx_rate_opt(Some(1.0 / 140.0))
            .base_currency(Currency::JPY)
            .quote_currency(Currency::USD)
            .correlation(-0.2)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("JPY-OIS"))
            .spot_id("NKY-SPOT".into())
            .vol_surface_id(CurveId::new("NKY-VOL"))
            .div_yield_id_opt(Some(CurveId::new("NKY-DIV")))
            .fx_rate_id_opt(Some("USDJPY-SPOT".to_string()))
            .fx_vol_id_opt(Some(CurveId::new("USDJPY-VOL")))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("Example QuantoOption construction should not fail")
    }

    /// Validate quanto option economics at construction boundaries.
    pub fn validate(&self) -> finstack_core::Result<()> {
        crate::instruments::common_impl::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "QuantoOption",
        )?;
        if !self.correlation.is_finite() || !(-1.0..=1.0).contains(&self.correlation) {
            return Err(finstack_core::Error::Validation(format!(
                "QuantoOption correlation must be a finite value in [-1, 1], got {}",
                self.correlation
            )));
        }
        if self.equity_strike.currency() != self.base_currency {
            return Err(finstack_core::Error::Validation(format!(
                "QuantoOption equity_strike currency ({}) must match base_currency ({})",
                self.equity_strike.currency(),
                self.base_currency
            )));
        }
        if self.notional.currency() != self.quote_currency {
            return Err(finstack_core::Error::Validation(format!(
                "QuantoOption notional currency ({}) must match quote_currency ({})",
                self.notional.currency(),
                self.quote_currency
            )));
        }
        if !self.equity_strike.amount().is_finite() || self.equity_strike.amount() <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "QuantoOption equity_strike must be positive and finite; got {}",
                self.equity_strike.amount()
            )));
        }
        if !self.notional.amount().is_finite() || self.notional.amount() <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "QuantoOption notional must be positive and finite; got {}",
                self.notional.amount()
            )));
        }
        match (self.underlying_quantity, self.payoff_fx_rate) {
            (Some(quantity), Some(fx_rate)) => {
                if !quantity.is_finite() || quantity <= 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "QuantoOption underlying_quantity must be positive and finite; got {}",
                        quantity
                    )));
                }
                if !fx_rate.is_finite() || fx_rate <= 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "QuantoOption payoff_fx_rate must be positive and finite; got {}",
                        fx_rate
                    )));
                }
                let expected = quantity * fx_rate * self.equity_strike.amount();
                let tolerance = 1e-8 * expected.abs().max(self.notional.amount().abs()).max(1.0);
                if (self.notional.amount() - expected).abs() > tolerance {
                    return Err(finstack_core::Error::Validation(format!(
                        "QuantoOption notional ({}) must match underlying_quantity * payoff_fx_rate * equity_strike ({})",
                        self.notional.amount(),
                        expected
                    )));
                }
            }
            (None, None) => {}
            _ => {
                return Err(finstack_core::Error::Validation(
                    "QuantoOption requires both underlying_quantity and payoff_fx_rate when either is supplied".to_string(),
                ));
            }
        }
        Ok(())
    }
    /// Calculate the net present value using Monte Carlo.
    ///
    /// **Note:** Monte Carlo pricing is intentionally unsupported for quanto options.
    /// The analytical quanto model uses a drift adjustment that doesn't translate
    /// directly to an MC payoff without a 2D correlated process. Use
    /// [`crate::instruments::Instrument::value`] for analytical pricing instead.
    ///
    /// # Errors
    ///
    /// Always returns an error indicating MC is not supported.
    pub fn npv_mc(
        &self,
        _curves: &finstack_core::market_data::context::MarketContext,
        _as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        Err(finstack_core::Error::Validation(
            "Monte Carlo pricing is not supported for QuantoOption. \
             The analytical quanto model uses a drift adjustment that cannot be \
             correctly represented in a simple 1D MC simulation. Use npv() for \
             analytical pricing instead."
                .to_string(),
        ))
    }
}

// ================================================================================================
// Option risk metric providers (metrics adapters)
// ================================================================================================

impl crate::instruments::common_impl::traits::OptionGreeksProvider for QuantoOption {
    // QuantoOption's analytical pricer does not support Theta; fail fast so the
    // metric layer reports an actionable error rather than `metric_not_found`.
    fn option_theta(
        &self,
        _market: &finstack_core::market_data::context::MarketContext,
        _as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        Err(finstack_core::Error::Validation(format!(
            "QuantoOption {}: Theta is not supported by the analytical quanto \
             pricer. Supported: Delta, Gamma, Vega, Rho, ForeignRho, Vanna, Volga.",
            self.id
        )))
    }

    fn option_delta(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }
        Ok(Some(crate::metrics::central_diff_scalar_relative(
            self,
            market,
            as_of,
            &self.spot_id,
            crate::metrics::bump_sizes::SPOT,
        )?))
    }

    fn option_gamma(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();

        let spot_scalar = market.get_price(&self.spot_id)?;
        let current_spot = crate::metrics::scalar_numeric_value(spot_scalar);
        let bump_size = current_spot * crate::metrics::bump_sizes::SPOT;
        if bump_size <= 0.0 {
            return Ok(Some(0.0));
        }

        let up = crate::metrics::bump_scalar_price(
            market,
            &self.spot_id,
            crate::metrics::bump_sizes::SPOT,
        )?;
        let pv_up = self.value(&up, as_of)?.amount();
        let dn = crate::metrics::bump_scalar_price(
            market,
            &self.spot_id,
            -crate::metrics::bump_sizes::SPOT,
        )?;
        let pv_dn = self.value(&dn, as_of)?.amount();

        Ok(Some(
            (pv_up - 2.0 * base_pv + pv_dn) / (bump_size * bump_size),
        ))
    }

    fn option_vega(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let bumped = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            crate::metrics::bump_sizes::VOLATILITY,
        )?;
        let pv_bumped = self.value(&bumped, as_of)?.amount();
        Ok(Some(
            (pv_bumped - base_pv) / crate::metrics::bump_sizes::VOLATILITY,
        ))
    }

    fn option_rho_bp(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let bump_bp = self.pricing_overrides.rho_bump_bp();
        let bumped = crate::metrics::bump_discount_curve_parallel(
            market,
            &self.domestic_discount_curve_id,
            bump_bp,
        )?;
        let pv_bumped = self.value(&bumped, as_of)?.amount();
        Ok(Some((pv_bumped - base_pv) / bump_bp))
    }

    fn option_foreign_rho_bp(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let bump_bp = self.pricing_overrides.rho_bump_bp();
        let bumped = crate::metrics::bump_discount_curve_parallel(
            market,
            &self.foreign_discount_curve_id,
            bump_bp,
        )?;
        let pv_bumped = self.value(&bumped, as_of)?.amount();
        Ok(Some((pv_bumped - base_pv) / bump_bp))
    }

    fn option_vanna(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let spot_scalar = market.get_price(&self.spot_id)?;
        let current_spot = crate::metrics::scalar_numeric_value(spot_scalar);
        let spot_bump = current_spot * crate::metrics::bump_sizes::SPOT;
        if spot_bump <= 0.0 {
            return Ok(Some(0.0));
        }
        let vol_bump = crate::metrics::bump_sizes::VOLATILITY;

        // Delta at vol_up
        let vol_up = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            vol_bump,
        )?;
        let up = crate::metrics::bump_scalar_price(
            &vol_up,
            &self.spot_id,
            crate::metrics::bump_sizes::SPOT,
        )?;
        let dn = crate::metrics::bump_scalar_price(
            &vol_up,
            &self.spot_id,
            -crate::metrics::bump_sizes::SPOT,
        )?;
        let pv_up = self.value(&up, as_of)?.amount();
        let pv_dn = self.value(&dn, as_of)?.amount();
        let delta_up = (pv_up - pv_dn) / (2.0 * spot_bump);

        // Delta at vol_down
        let vol_dn = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let up = crate::metrics::bump_scalar_price(
            &vol_dn,
            &self.spot_id,
            crate::metrics::bump_sizes::SPOT,
        )?;
        let dn = crate::metrics::bump_scalar_price(
            &vol_dn,
            &self.spot_id,
            -crate::metrics::bump_sizes::SPOT,
        )?;
        let pv_up = self.value(&up, as_of)?.amount();
        let pv_dn = self.value(&dn, as_of)?.amount();
        let delta_dn = (pv_up - pv_dn) / (2.0 * spot_bump);

        Ok(Some((delta_up - delta_dn) / (2.0 * vol_bump)))
    }

    fn option_volga(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
        base_pv: f64,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let vol_bump = crate::metrics::bump_sizes::VOLATILITY;
        let up = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            vol_bump,
        )?;
        let dn = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let pv_up = self.value(&up, as_of)?.amount();
        let pv_dn = self.value(&dn, as_of)?.amount();
        Ok(Some(
            (pv_up - 2.0 * base_pv + pv_dn) / (vol_bump * vol_bump),
        ))
    }
}

impl crate::instruments::common_impl::traits::Instrument for QuantoOption {
    impl_instrument_base!(crate::pricer::InstrumentType::QuantoOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::QuantoBS
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        deps.add_spot_id(self.spot_id.as_str());
        deps.add_vol_surface_id(self.vol_surface_id.as_str());
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::fx::quanto_option::pricer::QuantoOptionAnalyticalPricer;
        use crate::pricer::Pricer;

        let pricer = QuantoOptionAnalyticalPricer::new();
        let result = pricer
            .price_dyn(self, market, as_of)
            .map_err(|e| finstack_core::Error::Validation(e.to_string()))?;
        Ok(result.value)
    }

    fn effective_start_date(&self) -> Option<finstack_core::dates::Date> {
        None
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

crate::impl_empty_cashflow_provider!(
    QuantoOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::CurveDependencies;

    #[test]
    fn test_quanto_option_example_creation() {
        let option = QuantoOption::example();
        assert_eq!(option.id.as_str(), "QUANTO-NKY-USD-CALL");
        assert_eq!(option.quote_currency, Currency::USD);
        assert_eq!(option.base_currency, Currency::JPY);
        assert_eq!(option.underlying_quantity, Some(4_000.0));
        assert!(option.payoff_fx_rate.is_some());
        assert!(option.correlation < 0.0); // Negative correlation in example
    }

    #[test]
    fn test_quanto_option_curve_dependencies() {
        let option = QuantoOption::example();
        let deps = option.curve_dependencies().expect("curve_dependencies");

        // Should include both domestic and foreign discount curves
        assert_eq!(deps.discount_curves.len(), 2);
        assert!(deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"));
        assert!(deps.discount_curves.iter().any(|c| c.as_str() == "JPY-OIS"));
    }

    #[test]
    fn test_quanto_option_serde_rejects_inconsistent_notional() {
        let option = QuantoOption::example();
        let mut json = serde_json::to_value(&option).expect("serialize");
        json["notional"]["amount"] = serde_json::json!(2_000_000.0);

        let err = serde_json::from_value::<QuantoOption>(json)
            .expect_err("inconsistent notional should fail during deserialization");
        assert!(
            err.to_string().contains("notional"),
            "error should mention notional consistency: {}",
            err
        );
    }

    #[test]
    fn test_quanto_option_mc_is_unsupported() {
        use finstack_core::market_data::context::MarketContext;

        let option = QuantoOption::example();
        let market = MarketContext::new();
        let as_of =
            Date::from_calendar_date(2024, time::Month::January, 15).expect("valid test date");

        let result = option.npv_mc(&market, as_of);

        // MC should fail with a clear error message
        assert!(result.is_err());
        let err = result.expect_err("expected MC error");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("Monte Carlo pricing is not supported"),
            "Error message should indicate MC is unsupported: {}",
            err_msg
        );
        assert!(
            err_msg.contains("npv()"),
            "Error should suggest using npv() instead: {}",
            err_msg
        );
    }
}
