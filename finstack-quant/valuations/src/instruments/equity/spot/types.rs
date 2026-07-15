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
use finstack_quant_core::types::{CurveId, InstrumentId};

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
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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
    /// Optional number of shares (defaults to 1 if not specified)
    pub shares: Option<f64>,
    /// Optional price quote (if not provided, will look up from market data)
    pub price_quote: Option<f64>,
    /// Explicit market data identifier to resolve the spot price
    pub price_id: Option<String>,
    /// Explicit market data identifier to resolve the dividend yield
    pub div_yield_id: Option<CurveId>,
    /// Optional discrete cash dividends `(ex_date, amount)` for single-name forwards.
    #[serde(default)]
    #[builder(default)]
    #[schemars(with = "Vec<(String, f64)>")]
    pub discrete_dividends: Vec<(Date, f64)>,
    /// Discount curve ID for pricing
    pub discount_curve_id: CurveId,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl Equity {
    /// Create a canonical example equity for testing and documentation.
    ///
    /// Returns a 100-share position in AAPL with realistic market data IDs.
    pub fn example() -> Self {
        Self::new("EQUITY-AAPL", "AAPL", Currency::USD)
            .with_shares(100.0)
            .with_price_id("AAPL-SPOT")
            .with_dividend_yield_id("AAPL-DIV")
    }

    /// Create a new equity instrument with default 1 share
    pub fn new(id: impl Into<String>, ticker: impl Into<String>, currency: Currency) -> Self {
        let discount_curve_id = CurveId::from(currency.to_string());

        Self {
            id: InstrumentId::new(id.into()),
            ticker: ticker.into(),
            currency,
            shares: None,
            price_quote: None,
            price_id: None,
            div_yield_id: None,
            discrete_dividends: Vec::new(),
            discount_curve_id,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Set the number of shares
    pub fn with_shares(mut self, shares: f64) -> Self {
        self.shares = Some(shares);
        self
    }

    /// Set a price quote
    pub fn with_price(mut self, price: f64) -> Self {
        self.price_quote = Some(price);
        self
    }

    /// Override the market data identifier used to resolve the spot price
    pub fn with_price_id(mut self, price_id: impl Into<String>) -> Self {
        self.price_id = Some(price_id.into());
        self
    }

    /// Override the market data identifier used to resolve the dividend yield
    pub fn with_dividend_yield_id(mut self, div_id: impl Into<CurveId>) -> Self {
        self.div_yield_id = Some(div_id.into());
        self
    }

    /// Set an explicit discrete dividend schedule.
    pub fn with_discrete_dividends(mut self, dividends: Vec<(Date, f64)>) -> Self {
        self.discrete_dividends = dividends;
        self
    }

    pub(crate) fn price_id_candidates(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        let mut push = |candidate: Option<&str>| {
            if let Some(value) = candidate {
                if !value.is_empty() && !ids.iter().any(|existing| existing == value) {
                    ids.push(value.to_string());
                }
            }
        };

        push(self.price_id.as_deref());
        push(self.attributes.get_meta("price_id"));
        push(self.attributes.get_meta("spot_id"));
        push(self.attributes.get_meta("market_price_id"));
        push(Some(self.ticker.as_str()));
        push(Some(self.id.as_str()));
        let ticker_spot = format!("{}-SPOT", self.ticker);
        push(Some(ticker_spot.as_str()));
        let id_spot = format!("{}-SPOT", self.id.as_str());
        push(Some(id_spot.as_str()));
        push(Some("EQUITY-SPOT"));

        ids
    }

    pub(crate) fn dividend_yield_id_candidates(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        let mut push = |candidate: Option<&str>| {
            if let Some(value) = candidate {
                if !value.is_empty() && !ids.iter().any(|existing| existing == value) {
                    ids.push(value.to_string());
                }
            }
        };

        push(self.div_yield_id.as_deref());
        push(self.attributes.get_meta("div_yield_id"));
        push(self.attributes.get_meta("dividend_yield_key"));
        push(self.attributes.get_meta("div_yield_id"));
        let ticker_div = format!("{}-DIVYIELD", self.ticker);
        push(Some(ticker_div.as_str()));
        let id_div = format!("{}-DIVYIELD", self.id.as_str());
        push(Some(id_div.as_str()));
        push(Some("EQUITY-DIVYIELD"));

        ids
    }

    fn money_from_scalar(
        &self,
        scalar: &MarketScalar,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        match scalar {
            MarketScalar::Price(m) => self.convert_price_to_currency(*m, market, as_of),
            MarketScalar::Unitless(v) => Ok(Money::new(*v, self.currency)),
        }
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

    /// Get the effective number of shares (defaults to 1)
    pub fn effective_shares(&self) -> f64 {
        self.shares.unwrap_or(1.0)
    }

    /// Resolve price per share for the equity
    pub fn price_per_share(
        &self,
        curves: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        if let Some(px) = self.price_quote {
            return Ok(Money::new(px, self.currency));
        }

        let candidates = self.price_id_candidates();
        for key in &candidates {
            match curves.get_price(key) {
                Ok(scalar) => {
                    return self.money_from_scalar(scalar, curves, as_of);
                }
                Err(err) => match err {
                    finstack_quant_core::Error::Input(
                        finstack_quant_core::InputError::NotFound { .. },
                    ) => {
                        continue;
                    }
                    _ => return Err(err),
                },
            }
        }

        Err(finstack_quant_core::InputError::NotFound {
            id: format!("equity price (candidates: {})", candidates.join(", ")),
        }
        .into())
    }

    /// Resolve dividend yield (annualized, decimal) for the equity
    pub fn dividend_yield(&self, curves: &MarketContext) -> finstack_quant_core::Result<f64> {
        if let Some(explicit_id) = self.div_yield_id.as_deref() {
            return match curves.get_price(explicit_id)? {
                MarketScalar::Unitless(value) => Ok(*value),
                MarketScalar::Price(_) => Err(finstack_quant_core::Error::Validation(format!(
                    "Equity '{}' dividend yield '{}' must be unitless",
                    self.id, explicit_id
                ))),
            };
        }
        let candidates = self.dividend_yield_id_candidates();
        for key in &candidates {
            match curves.get_price(key) {
                Ok(MarketScalar::Unitless(v)) => return Ok(*v),
                Ok(MarketScalar::Price(_)) => continue,
                Err(err) => match err {
                    finstack_quant_core::Error::Input(
                        finstack_quant_core::InputError::NotFound { .. },
                    ) => continue,
                    _ => return Err(err),
                },
            }
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
        Ok(Money::new(fwd, self.currency))
    }

    /// Calculate forward total value for the position
    pub fn forward_value(
        &self,
        curves: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
        t: f64,
    ) -> finstack_quant_core::Result<Money> {
        let per_share = self.forward_price_per_share(curves, as_of, t)?;
        Ok(Money::new(
            per_share.amount() * self.effective_shares(),
            self.currency,
        ))
    }
}

impl crate::instruments::common_impl::traits::Instrument for Equity {
    impl_instrument_base!(crate::pricer::InstrumentType::Equity);

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        for spot_id in self.price_id_candidates() {
            deps.add_spot_id(spot_id);
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let spot_px = self.price_per_share(market, as_of)?;

        Ok(finstack_quant_core::money::Money::new(
            spot_px.amount() * self.effective_shares(),
            self.currency,
        ))
    }

    fn effective_start_date(&self) -> Option<Date> {
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

impl finstack_quant_cashflows::CashflowScheduleSource for Equity {
    fn notional(&self) -> Option<Money> {
        // Equity notional is shares * price (market value)
        // If price not quoted, return None to avoid incorrect estimation
        self.price_quote
            .map(|p| Money::new(self.effective_shares() * p, self.currency))
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
                notional_hint: self.notional(),
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
            .with_shares(100.0)
            .with_price(150.0);

        assert_eq!(equity.id.as_str(), "AAPL");
        assert_eq!(equity.ticker, "AAPL");
        assert_eq!(equity.currency, Currency::USD);
        assert_eq!(equity.effective_shares(), 100.0);
        assert_eq!(equity.price_quote, Some(150.0));
    }

    #[test]
    fn test_equity_default_shares() {
        let equity = Equity::new("MSFT", "MSFT", Currency::USD);
        assert_eq!(equity.effective_shares(), 1.0);
    }

    #[test]
    fn test_equity_valuation() {
        let equity = Equity::new("AAPL", "AAPL", Currency::USD)
            .with_shares(100.0)
            .with_price(150.0);

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
            .with_shares(50.0)
            .with_price(200.0);

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
