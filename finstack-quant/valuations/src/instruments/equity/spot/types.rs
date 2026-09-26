//! Equity types and implementations.
//!
//! Defines the `Equity` instrument shape and integrates with the standard
//! instrument macro. Pricing is delegated to `pricing::EquityPricer` and
//! metrics live under `metrics/`.

use crate::impl_instrument_base;
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};

/// Simple equity (spot) instrument.
///
/// Represents a spot equity position that can be priced using market data.
/// The price can come from direct market quotes or be computed from
/// underlying fundamentals.
///
/// See unit tests and `examples/` for usage.
#[derive(
    Clone,
    Debug,
    PartialEq,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
// Note: JsonSchema derive requires finstack-quant-core types to implement JsonSchema
// #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Equity {
    /// Unique identifier for the equity
    pub id: InstrumentId,
    /// Ticker symbol (e.g., "AAPL", "MSFT")
    pub ticker: String,
    /// Currency in which the equity is quoted
    pub currency: Currency,
    /// Optional number of shares held (defaults to 1 if not specified).
    pub quantity: Option<f64>,
    /// Optional quoted spot price per share in `currency`. When set it wins
    /// over the `spot_id` market lookup.
    pub quoted_spot: Option<f64>,
    /// Market-scalar id (`MarketContext::get_price`) of the spot price per
    /// share. Required unless `quoted_spot` is set; no id is ever derived from
    /// the ticker or instrument id.
    pub spot_id: Option<PriceId>,
    /// Market-scalar id of the unitless continuous dividend yield (decimal,
    /// 0.02 = 2%). `None` means a zero dividend yield.
    pub div_yield_id: Option<PriceId>,
    /// Optional discrete cash dividends `(ex_date, amount)` for single-name forwards.
    #[serde(default)]
    #[builder(default)]
    #[serde(with = "finstack_quant_core::wire::dated_f64_values")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, f64)>")
    )]
    pub discrete_dividends: Vec<(Date, f64)>,
    /// Discount curve ID for pricing
    pub discount_curve_id: CurveId,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
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
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl Equity {
    fn validate(&self) -> finstack_quant_core::Result<()> {
        use crate::instruments::common_impl::validation;

        if self.ticker.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "Equity requires a non-empty ticker".to_string(),
            ));
        }
        if let Some(quantity) = self.quantity {
            validation::validate_f64_finite(quantity, "Equity quantity")?;
        }
        if let Some(price) = self.quoted_spot {
            validation::validate_f64_non_negative(price, "Equity quoted_spot")?;
        }
        if self
            .spot_id
            .as_ref()
            .is_some_and(|id| id.as_str().trim().is_empty())
        {
            return Err(finstack_quant_core::Error::Validation(
                "Equity spot_id must not be empty when supplied".to_string(),
            ));
        }
        for (date, amount) in &self.discrete_dividends {
            validation::validate_f64_non_negative(*amount, "Equity discrete-dividend amount")
                .map_err(|error| {
                    finstack_quant_core::Error::Validation(format!(
                        "Equity '{}' dividend on {date} is invalid: {error}",
                        self.id
                    ))
                })?;
        }
        Ok(())
    }

    /// Create a canonical example equity for testing and documentation.
    ///
    /// Returns a 100-share position in AAPL with realistic market data IDs.
    pub fn example() -> Self {
        Self::new("EQUITY-AAPL", "AAPL", Currency::USD)
            .with_quantity(100.0)
            .with_spot_id("AAPL-SPOT")
            .with_dividend_yield_id("AAPL-DIV")
    }

    /// Create a new equity instrument with default 1 share
    pub fn new(id: impl Into<String>, ticker: impl Into<String>, currency: Currency) -> Self {
        let discount_curve_id = CurveId::from(currency.to_string());

        Self {
            id: InstrumentId::new(id.into()),
            ticker: ticker.into(),
            currency,
            quantity: None,
            quoted_spot: None,
            spot_id: None,
            div_yield_id: None,
            discrete_dividends: Vec::new(),
            discount_curve_id,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    /// Set the number of shares held.
    ///
    /// # Arguments
    ///
    /// * `quantity` - Share count (finite; negative for a short position).
    pub fn with_quantity(mut self, quantity: f64) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Set a quoted spot price that replaces the `spot_id` market lookup.
    ///
    /// # Arguments
    ///
    /// * `price` - Spot price per share in the equity's `currency` (non-negative).
    pub fn with_quoted_spot(mut self, price: f64) -> Self {
        self.quoted_spot = Some(price);
        self
    }

    /// Set the market-scalar id used to resolve the spot price.
    ///
    /// # Arguments
    ///
    /// * `spot_id` - `MarketContext::get_price` id of the spot price per share.
    pub fn with_spot_id(mut self, spot_id: impl Into<PriceId>) -> Self {
        self.spot_id = Some(spot_id.into());
        self
    }

    /// Override the scalar identifier used to resolve the dividend yield.
    pub fn with_dividend_yield_id(mut self, div_id: impl Into<PriceId>) -> Self {
        self.div_yield_id = Some(div_id.into());
        self
    }

    /// Set an explicit discrete dividend schedule.
    pub fn with_discrete_dividends(mut self, dividends: Vec<(Date, f64)>) -> Self {
        self.discrete_dividends = dividends;
        self
    }

    fn money_from_scalar(
        &self,
        scalar: &MarketScalar,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        let price = match scalar {
            MarketScalar::Price(m) => self.convert_price_to_currency(*m, market, as_of),
            MarketScalar::Unitless(v) => Ok(Money::new(*v, self.currency)?),
        }?;
        crate::instruments::common_impl::validation::validate_f64_non_negative(
            price.amount(),
            "Equity market price",
        )?;
        Ok(price)
    }

    fn convert_price_to_currency(
        &self,
        price: Money,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        if price.currency() == self.currency {
            return Ok(price);
        }

        let matrix = market.fx().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "fx_matrix".to_string(),
            })
        })?;

        struct MatrixProvider<'a> {
            m: &'a finstack_quant_core::money::fx::FxMatrix,
        }
        impl finstack_quant_core::money::fx::FxProvider for MatrixProvider<'_> {
            fn rate(
                &self,
                from: finstack_quant_core::currency::Currency,
                to: finstack_quant_core::currency::Currency,
                on: finstack_quant_core::dates::Date,
                policy: finstack_quant_core::money::fx::FxConversionPolicy,
            ) -> finstack_quant_core::Result<f64> {
                let r = self
                    .m
                    .rate(finstack_quant_core::money::fx::FxQuery::with_policy(
                        from, to, on, policy,
                    ))?;
                Ok(r.rate)
            }
        }

        let provider = MatrixProvider { m: matrix.as_ref() };
        price.convert(
            self.currency,
            as_of,
            &provider,
            finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
        )
    }

    /// Get the effective number of shares held (defaults to 1).
    pub fn effective_quantity(&self) -> f64 {
        self.quantity.unwrap_or(1.0)
    }

    /// Resolve price per share for the equity.
    ///
    /// A direct `quoted_spot` has highest priority; otherwise the `spot_id`
    /// scalar is read. Missing or invalid data returns an error.
    ///
    /// # Arguments
    ///
    /// * `curves` - Market context holding the `spot_id` scalar (and the FX
    ///   matrix when the scalar is a price in another currency).
    /// * `as_of` - Valuation date used for any FX conversion.
    pub fn price_per_share(
        &self,
        curves: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        self.validate()?;
        if let Some(px) = self.quoted_spot {
            return Money::new(px, self.currency);
        }
        let spot_id = self.spot_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "Equity '{}' requires either quoted_spot or spot_id",
                self.id
            ))
        })?;
        self.money_from_scalar(curves.get_price(spot_id)?, curves, as_of)
    }

    /// Resolve dividend yield (annualized, decimal) for the equity
    pub fn dividend_yield(&self, curves: &MarketContext) -> finstack_quant_core::Result<f64> {
        if let Some(explicit_id) = self.div_yield_id.as_deref() {
            return match curves.get_price(explicit_id)? {
                MarketScalar::Unitless(value) if value.is_finite() => Ok(*value),
                MarketScalar::Unitless(value) => {
                    Err(finstack_quant_core::Error::Validation(format!(
                        "Equity '{}' dividend yield '{}' must be finite, got {value}",
                        self.id, explicit_id
                    )))
                }
                MarketScalar::Price(_) => Err(finstack_quant_core::Error::Validation(format!(
                    "Equity '{}' dividend yield '{}' must be unitless",
                    self.id, explicit_id
                ))),
            };
        }
        Ok(0.0)
    }

    /// Calculate forward price per share using continuous-compound approximation
    pub fn forward_price_per_share(
        &self,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
        t: f64,
    ) -> finstack_quant_core::Result<Money> {
        let s0 = self.price_per_share(market, as_of)?;
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        if !t.is_finite() || t < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Equity '{}' forward horizon must be finite and non-negative, got {t}",
                self.id
            )));
        }

        // `t` is a horizon from `as_of`, while `DiscountCurve::df(t)` is
        // anchored at the curve base date. Rebase the terminal discount factor
        // explicitly so a seasoned valuation is invariant to how the same term
        // structure is dated.
        let curve_time_to_as_of = disc.day_count().year_fraction(
            disc.base_date(),
            as_of,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let df_as_of = disc.df(curve_time_to_as_of);
        let df_terminal = disc.df(curve_time_to_as_of + t);
        if !df_as_of.is_finite()
            || df_as_of <= 0.0
            || !df_terminal.is_finite()
            || df_terminal <= 0.0
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Equity '{}' discount curve returned invalid rebased discount factors",
                self.id
            )));
        }
        let df_horizon = df_terminal / df_as_of;
        let fwd = if !self.discrete_dividends.is_empty() {
            let mut pv_dividends = 0.0;
            for (ex_date, amount) in &self.discrete_dividends {
                let t_div = disc.day_count().year_fraction(
                    as_of,
                    *ex_date,
                    finstack_quant_core::dates::DayCountContext::default(),
                )?;
                if t_div > 0.0 && t_div <= t {
                    pv_dividends += amount * disc.df_between_dates(as_of, *ex_date)?;
                }
            }
            (s0.amount() - pv_dividends) / df_horizon
        } else {
            let dy = self.dividend_yield(market)?;
            s0.amount() / df_horizon * (-dy * t).exp()
        };
        Money::new(fwd, self.currency)
    }

    /// Calculate forward total value for the position
    pub fn forward_value(
        &self,
        curves: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
        t: f64,
    ) -> finstack_quant_core::Result<Money> {
        let per_share = self.forward_price_per_share(curves, as_of, t)?;
        Money::new(
            per_share.amount() * self.effective_quantity(),
            self.currency,
        )
    }
}

impl crate::instruments::common_impl::traits::Instrument for Equity {
    impl_instrument_base!(crate::pricer::InstrumentType::Equity);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(spot_id) = &self.spot_id {
            deps.add_market_scalar_id(spot_id);
        }
        if let Some(dividend_yield_id) = &self.div_yield_id {
            deps.add_market_scalar_id(dividend_yield_id);
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let spot_px = self.price_per_share(market, as_of)?;

        finstack_quant_core::money::Money::new(
            spot_px.amount() * self.effective_quantity(),
            self.currency,
        )
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for Equity {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        // Equity notional is shares * price (market value)
        // If price not quoted, return None to avoid incorrect estimation
        self.quoted_spot
            .map(|p| Money::new(self.effective_quantity() * p, self.currency))
            .transpose()
    }

    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            finstack_quant_core::dates::DayCount::Act365F, // Standard for equity spot
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional()?,
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::NoResidual,
                    ..Default::default()
                },
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_cashflows::CashflowProvider as _;
    use time::Month;

    #[test]
    fn test_equity_creation() {
        let equity = Equity::new("AAPL", "AAPL", Currency::USD)
            .with_quantity(100.0)
            .with_quoted_spot(150.0);

        assert_eq!(equity.id.as_str(), "AAPL");
        assert_eq!(equity.ticker, "AAPL");
        assert_eq!(equity.currency, Currency::USD);
        assert_eq!(equity.effective_quantity(), 100.0);
        assert_eq!(equity.quoted_spot, Some(150.0));
    }

    #[test]
    fn explicit_market_dependencies_exclude_fallback_candidates() {
        let equity = Equity::example();
        let deps =
            crate::instruments::Instrument::market_dependencies(&equity).expect("dependencies");

        assert_eq!(deps.market_scalar_ids.len(), 2);
        assert!(deps.market_scalar_ids.iter().any(|id| id == "AAPL-SPOT"));
        assert!(deps.market_scalar_ids.iter().any(|id| id == "AAPL-DIV"));
        assert!(deps.series_ids.is_empty());
    }

    #[test]
    fn missing_explicit_price_id_does_not_use_global_fallback() {
        let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_spot_id("AAPL-PRIMARY");
        let market = MarketContext::new().insert_price(
            "EQUITY-SPOT",
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(999.0),
        );
        let as_of = Date::from_calendar_date(2025, Month::January, 2).expect("valid date");

        let error = equity
            .price_per_share(&market, as_of)
            .expect_err("missing explicit quote must fail");
        assert!(error.to_string().contains("AAPL-PRIMARY"));
    }

    #[test]
    fn test_equity_default_shares() {
        let equity = Equity::new("MSFT", "MSFT", Currency::USD);
        assert_eq!(equity.effective_quantity(), 1.0);
    }

    #[test]
    fn test_equity_valuation() {
        let equity = Equity::new("AAPL", "AAPL", Currency::USD)
            .with_quantity(100.0)
            .with_quoted_spot(150.0);

        let curves = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        use crate::instruments::common_impl::traits::Instrument;
        let value = equity.value(&curves, as_of).expect("should succeed");
        assert_eq!(value.amount(), 15_000.0);
        assert_eq!(value.currency(), Currency::USD);
    }

    #[test]
    fn test_equity_no_cashflows() {
        let equity = Equity::new("AAPL", "AAPL", Currency::USD);
        let curves = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let flows = equity
            .dated_cashflows(&curves, as_of)
            .expect("should succeed");
        assert!(flows.is_empty());
    }

    #[test]
    fn test_equity_metrics() {
        let equity = Equity::new("AAPL", "AAPL", Currency::USD)
            .with_quantity(50.0)
            .with_quoted_spot(200.0);

        let curves = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        use crate::instruments::common_impl::traits::Instrument;
        let result = equity
            .price_with_metrics(
                &curves,
                as_of,
                &[
                    crate::metrics::MetricId::EquityPricePerShare,
                    crate::metrics::MetricId::EquityShares,
                ],
                crate::instruments::PricingOptions::default(),
            )
            .expect("should succeed");
        assert_eq!(result.value.amount(), 10_000.0); // This is the market value (PV)
        assert_eq!(result.measures.get("equity_price_per_share"), Some(&200.0));
        assert_eq!(result.measures.get("equity_shares"), Some(&50.0));
    }
}
