//! Exchange-listed equity futures, including fixed-currency quanto contracts.

use crate::impl_instrument_base;
use crate::instruments::common_impl::dependencies::VolatilityDependency;
use crate::instruments::common_impl::listed::ListedFutureTerms;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};

/// Market inputs for a fixed-currency quanto equity future.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EquityFutureQuantoSpec {
    /// Settlement-currency discount curve used to form the ATM FX forward.
    pub settlement_discount_curve_id: CurveId,
    /// Equity volatility surface in decimal volatility per square-root year.
    pub equity_vol_surface_id: CurveId,
    /// FX volatility surface for settlement currency per underlying currency.
    pub fx_vol_surface_id: CurveId,
    /// FX spot scalar in settlement currency per underlying currency.
    pub fx_spot_id: PriceId,
    /// Correlation between equity returns and the settlement-per-underlying FX rate.
    pub correlation: f64,
}

impl EquityFutureQuantoSpec {
    /// Construct validated quanto market inputs.
    ///
    /// # Arguments
    ///
    /// * `settlement_discount_curve_id` - Discount curve for the variation-margin currency.
    /// * `equity_vol_surface_id` - Equity volatility surface identifier.
    /// * `fx_vol_surface_id` - FX volatility surface identifier.
    /// * `fx_spot_id` - Spot scalar for settlement currency per underlying currency.
    /// * `correlation` - Finite equity/FX return correlation in `[-1, 1]`.
    pub fn new(
        settlement_discount_curve_id: impl Into<CurveId>,
        equity_vol_surface_id: impl Into<CurveId>,
        fx_vol_surface_id: impl Into<CurveId>,
        fx_spot_id: impl Into<PriceId>,
        correlation: f64,
    ) -> finstack_quant_core::Result<Self> {
        let spec = Self {
            settlement_discount_curve_id: settlement_discount_curve_id.into(),
            equity_vol_surface_id: equity_vol_surface_id.into(),
            fx_vol_surface_id: fx_vol_surface_id.into(),
            fx_spot_id: fx_spot_id.into(),
            correlation,
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Validate the quanto correlation.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.correlation.is_finite() || !(-1.0..=1.0).contains(&self.correlation) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityFuture quanto correlation must be finite and in [-1, 1], got {}",
                self.correlation
            )));
        }
        Ok(())
    }
}

/// Exchange-listed future on an equity, equity index, or fixed-currency quanto index.
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
pub struct EquityFuture {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Equity or index ticker.
    pub underlying_ticker: String,
    /// Currency in which the underlying spot and dividends are quoted.
    pub underlying_currency: Currency,
    /// Standard listed position and lifecycle terms.
    pub terms: ListedFutureTerms,
    /// Underlying-currency discount curve used for equity carry.
    pub discount_curve_id: CurveId,
    /// Current equity or index level.
    pub spot_id: PriceId,
    /// Optional continuous dividend-yield scalar in decimal annual units.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub div_yield_id: Option<PriceId>,
    /// Optional discrete dividends `(ex_date, amount)` in index points.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(with = "finstack_quant_core::wire::dated_f64_values")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, f64)>")
    )]
    pub discrete_dividends: Vec<(Date, f64)>,
    /// Required quanto adjustment when settlement and underlying currencies differ.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quanto: Option<EquityFutureQuantoSpec>,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
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
    /// Attributes for selection and reporting.
    #[builder(default)]
    #[serde(default)]
    pub attributes: Attributes,
}

impl EquityFuture {
    /// Validate position, currency, dividend, and quanto invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.terms.validate()?;
        if self.underlying_ticker.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "EquityFuture underlying_ticker must not be empty".to_string(),
            ));
        }
        for (date, amount) in &self.discrete_dividends {
            if !amount.is_finite() || *amount <= 0.0 || *date > self.terms.settlement_date {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "EquityFuture dividend ({date}, {amount}) must be positive, finite, and no later than settlement"
                )));
            }
        }
        if self.underlying_currency == self.terms.currency {
            if self.quanto.is_some() {
                return Err(finstack_quant_core::Error::Validation(
                    "EquityFuture quanto inputs require different underlying and settlement currencies"
                        .to_string(),
                ));
            }
        } else {
            self.quanto
                .as_ref()
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "EquityFuture requires quanto inputs when underlying and settlement currencies differ"
                            .to_string(),
                    )
                })?
                .validate()?;
        }
        Ok(())
    }

    /// Create a canonical Eurex EURO STOXX 50 quanto future example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use crate::instruments::Position;
        use time::macros::date;

        Self::builder()
            .id(InstrumentId::new("EUREX-FESQ-DEC26"))
            .underlying_ticker("SX5E".to_string())
            .underlying_currency(Currency::EUR)
            .terms(ListedFutureTerms::new(
                5.0,
                10.0,
                Currency::USD,
                5_200.0,
                date!(2026 - 12 - 18),
                date!(2026 - 12 - 21),
                Position::Long,
            )?)
            .discount_curve_id(CurveId::new("EUR-OIS"))
            .spot_id(PriceId::new("SX5E-SPOT"))
            .div_yield_id_opt(Some(PriceId::new("SX5E-DIV")))
            .quanto_opt(Some(EquityFutureQuantoSpec::new(
                "USD-OIS",
                "SX5E-VOL",
                "EURUSD-VOL",
                "EURUSD-SPOT",
                -0.25,
            )?))
            .attributes(Attributes::new())
            .build()
    }

    fn spot(&self, market: &MarketContext) -> finstack_quant_core::Result<f64> {
        let spot = crate::instruments::common_impl::helpers::scalar_price_amount(
            market.get_price(&self.spot_id)?,
            self.underlying_currency,
        )?;
        if !spot.is_finite() || spot <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityFuture '{}' spot must be finite and positive, got {spot}",
                self.id
            )));
        }
        Ok(spot)
    }

    fn dividend_yield(&self, market: &MarketContext) -> finstack_quant_core::Result<f64> {
        self.div_yield_id.as_ref().map_or(Ok(0.0), |id| {
            let yield_value = match market.get_price(id)? {
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
                finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "EquityFuture '{}' dividend yield '{}' must be unitless, got Price({})",
                        self.id,
                        id,
                        money.currency()
                    )));
                }
            };
            if !yield_value.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "EquityFuture '{}' dividend yield must be finite",
                    self.id
                )));
            }
            Ok(yield_value)
        })
    }

    fn domestic_projection(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(f64, f64, f64)> {
        self.validate()?;
        let spot = self.spot(market)?;
        let discount = market.get_discount(&self.discount_curve_id)?;
        let df = discount.df_between_dates(as_of, self.terms.settlement_date)?;
        if !df.is_finite() || df <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "EquityFuture discount factor must be finite and positive".to_string(),
            ));
        }
        let t = DayCount::Act365F
            .year_fraction(
                as_of,
                self.terms.settlement_date,
                DayCountContext::default(),
            )?
            .max(0.0);
        let (forward, spot_derivative) = if self.discrete_dividends.is_empty() {
            let carry = (-self.dividend_yield(market)? * t).exp() / df;
            (spot * carry, carry)
        } else {
            let mut pv_dividends = finstack_quant_core::math::NeumaierAccumulator::new();
            for (date, amount) in &self.discrete_dividends {
                if *date > as_of {
                    pv_dividends.add(amount * discount.df_between_dates(as_of, *date)?);
                }
            }
            ((spot - pv_dividends.total()) / df, 1.0 / df)
        };
        if !forward.is_finite() || forward <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityFuture '{}' domestic forward must be finite and positive after dividends, got {forward}",
                self.id
            )));
        }
        Ok((forward, spot_derivative, t))
    }

    fn apply_quanto_adjustment(
        &self,
        market: &MarketContext,
        as_of: Date,
        domestic_forward: f64,
        t: f64,
    ) -> finstack_quant_core::Result<f64> {
        let Some(quanto) = &self.quanto else {
            return Ok(domestic_forward);
        };
        let equity_surface = market.get_surface(&quanto.equity_vol_surface_id)?;
        let equity_vol = finstack_quant_models::volatility::get_surface_vol_clamped(
            &equity_surface,
            t,
            domestic_forward,
        );
        let fx_spot = match market.get_price(&quanto.fx_spot_id)? {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "EquityFuture quanto FX spot '{}' must be unitless settlement-per-underlying, got Price({})",
                    quanto.fx_spot_id,
                    money.currency()
                )));
            }
        };
        if !fx_spot.is_finite() || fx_spot <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "EquityFuture quanto FX spot must be finite and positive".to_string(),
            ));
        }
        let underlying_discount = market.get_discount(&self.discount_curve_id)?;
        let underlying_df =
            underlying_discount.df_between_dates(as_of, self.terms.settlement_date)?;
        let settlement_discount = market.get_discount(&quanto.settlement_discount_curve_id)?;
        let settlement_df =
            settlement_discount.df_between_dates(as_of, self.terms.settlement_date)?;
        if !settlement_df.is_finite() || settlement_df <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "EquityFuture settlement discount factor must be finite and positive".to_string(),
            ));
        }
        let fx_forward = fx_spot * underlying_df / settlement_df;
        let fx_surface = market.get_surface(&quanto.fx_vol_surface_id)?;
        let fx_vol =
            finstack_quant_models::volatility::get_surface_vol_clamped(&fx_surface, t, fx_forward);
        if !equity_vol.is_finite() || equity_vol < 0.0 || !fx_vol.is_finite() || fx_vol < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "EquityFuture quanto volatilities must be finite and non-negative".to_string(),
            ));
        }
        let forward = domestic_forward * (-quanto.correlation * equity_vol * fx_vol * t).exp();
        if !forward.is_finite() || forward <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "EquityFuture quanto-adjusted forward must be finite and positive".to_string(),
            ));
        }
        Ok(forward)
    }

    /// Calculate cost-of-carry or quanto-adjusted fair futures price.
    ///
    /// The domestic formula is `(S − PV(dividends)) / DF` or
    /// `S × exp((r − q)T)`. A fixed-currency quanto additionally multiplies
    /// by `exp(−rho × equity_vol × fx_vol × T)`.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing spot, curves, dividends, and quanto surfaces.
    /// * `as_of` - Valuation date used for carry and volatility time.
    pub fn fair_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let (domestic_forward, _, t) = self.domestic_projection(market, as_of)?;
        self.apply_quanto_adjustment(market, as_of, domestic_forward, t)
    }

    /// Resolve the live quote, model mark, or official final settlement price.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing carry and quanto inputs when a live model mark is needed.
    /// * `as_of` - Valuation date controlling live versus post-trading state.
    pub fn mark_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.terms
            .resolve_mark(self.id.as_str(), as_of, || self.fair_price(market, as_of))
    }

    /// Calculate variation-margin P&L versus the entry futures price.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing equity carry and quanto inputs.
    /// * `as_of` - Valuation date controlling the contract lifecycle.
    pub fn npv_raw(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.terms
            .npv_from_model_price(self.id.as_str(), as_of, || self.fair_price(market, as_of))
    }

    /// Equity spot delta of the model futures P&L.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing spot and carry inputs.
    /// * `as_of` - Valuation date for the fair-price carry ratio.
    pub fn spot_delta(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let (domestic_forward, domestic_spot_derivative, t) =
            self.domestic_projection(market, as_of)?;
        let adjusted_forward = self.apply_quanto_adjustment(market, as_of, domestic_forward, t)?;
        let quanto_factor = adjusted_forward / domestic_forward;
        Ok(self.terms.point_delta()? * domestic_spot_derivative * quanto_factor)
    }
}

impl crate::instruments::Instrument for EquityFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::EquityFuture);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        let mut dependencies = crate::instruments::MarketDependencies::new();
        dependencies.add_discount_curve(self.discount_curve_id.clone());
        dependencies.add_market_scalar_id(self.spot_id.as_str());
        if let Some(id) = &self.div_yield_id {
            dependencies.add_market_scalar_id(id.as_str());
        }
        if let Some(quanto) = &self.quanto {
            dependencies.add_discount_curve(quanto.settlement_discount_curve_id.clone());
            dependencies.add_market_scalar_id(quanto.fx_spot_id.as_str());
            dependencies.add_volatility_dependency(VolatilityDependency::new(
                quanto.equity_vol_surface_id.clone(),
                Some(self.spot_id.clone()),
                None,
            ));
            dependencies.add_volatility_dependency(VolatilityDependency::new(
                quanto.fx_vol_surface_id.clone(),
                Some(quanto.fx_spot_id.clone()),
                None,
            ));
        }
        Ok(dependencies)
    }

    fn base_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Money::new(self.npv_raw(market, as_of)?, self.terms.currency)
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.terms.settlement_date)
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    EquityFuture,
    crate::cashflow::builder::CashflowRepresentation::NoResidual
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::Position;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    fn flat_discount(id: &str, as_of: Date, rate: f64) -> DiscountCurve {
        DiscountCurve::builder(id)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-rate).exp())])
            .build()
            .expect("discount curve")
    }

    fn flat_vol(id: &str, strike: f64, volatility: f64) -> VolSurface {
        VolSurface::builder(id)
            .expiries(&[1.0])
            .strikes(&[strike])
            .row(&[volatility])
            .build()
            .expect("vol surface")
    }

    #[test]
    fn domestic_equity_future_has_cost_of_carry_pnl_and_spot_delta() {
        let as_of = date!(2026 - 01 - 01);
        let settlement = date!(2027 - 01 - 01);
        let market = MarketContext::new()
            .insert(flat_discount("USD-OIS", as_of, 0.05))
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
            .insert_price("SPX-DIV", MarketScalar::Unitless(0.02));
        let future = EquityFuture::builder()
            .id(InstrumentId::new("ES"))
            .underlying_ticker("SPX".to_string())
            .underlying_currency(Currency::USD)
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    50.0,
                    Currency::USD,
                    100.0,
                    date!(2026 - 12 - 31),
                    settlement,
                    Position::Long,
                )
                .expect("terms"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id(PriceId::new("SPX-SPOT"))
            .div_yield_id(PriceId::new("SPX-DIV"))
            .attributes(Attributes::new())
            .build()
            .expect("future");

        let fair = 100.0 * 0.03_f64.exp();
        assert!((future.fair_price(&market, as_of).expect("fair") - fair).abs() < 1.0e-12);
        assert!(
            (future.spot_delta(&market, as_of).expect("delta") - 500.0 * 0.03_f64.exp()).abs()
                < 1.0e-10
        );
        assert!(
            (future.npv_raw(&market, as_of).expect("pv") - 500.0 * (fair - 100.0)).abs() < 1.0e-10
        );
    }

    #[test]
    fn quanto_equity_future_applies_equity_fx_covariance_drift() {
        let as_of = date!(2026 - 01 - 01);
        let settlement = date!(2027 - 01 - 01);
        let market = MarketContext::new()
            .insert(flat_discount("EUR-OIS", as_of, 0.03))
            .insert(flat_discount("USD-OIS", as_of, 0.05))
            .insert_price("SX5E-SPOT", MarketScalar::Unitless(100.0))
            .insert_price("SX5E-DIV", MarketScalar::Unitless(0.01))
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10))
            .insert_surface(flat_vol("SX5E-VOL", 100.0, 0.20))
            .insert_surface(flat_vol("EURUSD-VOL", 1.10, 0.10));
        let future = EquityFuture::builder()
            .id(InstrumentId::new("FESQ"))
            .underlying_ticker("SX5E".to_string())
            .underlying_currency(Currency::EUR)
            .terms(
                ListedFutureTerms::new(
                    1.0,
                    10.0,
                    Currency::USD,
                    100.0,
                    date!(2026 - 12 - 31),
                    settlement,
                    Position::Long,
                )
                .expect("terms"),
            )
            .discount_curve_id(CurveId::new("EUR-OIS"))
            .spot_id(PriceId::new("SX5E-SPOT"))
            .div_yield_id(PriceId::new("SX5E-DIV"))
            .quanto(
                EquityFutureQuantoSpec::new(
                    "USD-OIS",
                    "SX5E-VOL",
                    "EURUSD-VOL",
                    "EURUSD-SPOT",
                    0.5,
                )
                .expect("quanto"),
            )
            .attributes(Attributes::new())
            .build()
            .expect("future");

        let expected = 100.0 * (0.03_f64 - 0.01 - 0.5 * 0.20 * 0.10).exp();
        assert!((future.fair_price(&market, as_of).expect("fair") - expected).abs() < 1.0e-12);
    }

    #[test]
    fn discrete_dividends_cannot_exhaust_the_spot_prepaid_forward() {
        let as_of = date!(2026 - 01 - 01);
        let settlement = date!(2027 - 01 - 01);
        let market = MarketContext::new()
            .insert(flat_discount("USD-OIS", as_of, 0.05))
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0));
        let future = EquityFuture::builder()
            .id(InstrumentId::new("ES-INVALID-DIVIDENDS"))
            .underlying_ticker("SPX".to_string())
            .underlying_currency(Currency::USD)
            .terms(
                ListedFutureTerms::new(
                    1.0,
                    50.0,
                    Currency::USD,
                    100.0,
                    date!(2026 - 12 - 31),
                    settlement,
                    Position::Long,
                )
                .expect("terms"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id(PriceId::new("SPX-SPOT"))
            .discrete_dividends(vec![(date!(2026 - 06 - 01), 110.0)])
            .attributes(Attributes::new())
            .build()
            .expect("future");

        let fair_error = future
            .fair_price(&market, as_of)
            .expect_err("non-positive prepaid forward");
        assert!(fair_error
            .to_string()
            .contains("forward must be finite and positive"));
        assert!(future.spot_delta(&market, as_of).is_err());
    }

    #[test]
    fn dividend_yield_rejects_money_scalar() {
        let as_of = date!(2026 - 01 - 01);
        let future = EquityFuture::example().expect("example future");
        let market = MarketContext::new()
            .insert(flat_discount("EUR-OIS", as_of, 0.03))
            .insert_price("SX5E-SPOT", MarketScalar::Unitless(100.0))
            .insert_price(
                "SX5E-DIV",
                MarketScalar::Price(Money::new(0.02, Currency::EUR).expect("valid money fixture")),
            );

        let error = future
            .fair_price(&market, as_of)
            .expect_err("money-valued dividend yield must fail");
        assert!(error.to_string().contains("must be unitless"));
    }
}
