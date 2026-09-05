//! Listed equity total-return futures using exchange clearing notation.

use crate::impl_instrument_base;
use crate::instruments::common_impl::listed::ListedFutureTerms;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{InstrumentId, PriceId};

/// Exchange-listed equity or index total-return future.
///
/// The clearing price follows the Eurex-style decomposition
/// `TRF = spot + accrued_distributions - accrued_funding + basis`, where
/// `basis = spot × spread_basis_points × 1e-4 × year_fraction(as_of, settlement)`.
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
pub struct EquityTotalReturnFuture {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Equity or equity-index ticker.
    pub underlying_ticker: String,
    /// Standard listed position and lifecycle terms.
    pub terms: ListedFutureTerms,
    /// Current underlying close or exchange-prescribed reference level.
    pub spot_id: PriceId,
    /// Cumulative distribution points published by the exchange.
    pub accrued_distributions_id: PriceId,
    /// Cumulative funding points published by the exchange.
    pub accrued_funding_id: PriceId,
    /// Current annualized TRF spread scalar expressed in basis points.
    pub spread_basis_points_id: PriceId,
    /// Day-count basis used to convert the annualized spread into index points.
    pub spread_day_count: DayCount,
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

/// Model price and analytical first-order risks for one TRF mark.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TrfProjection {
    price: f64,
    spot_derivative: f64,
    spread_basis_points_derivative: f64,
}

impl EquityTotalReturnFuture {
    /// Validate contract and listed-position invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.terms.validate()?;
        if self.underlying_ticker.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "EquityTotalReturnFuture underlying_ticker must not be empty".to_string(),
            ));
        }
        Ok(())
    }

    /// Create a canonical Eurex EURO STOXX 50 TRF example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use crate::instruments::Position;
        use finstack_quant_core::currency::Currency;
        use time::macros::date;

        Self::builder()
            .id(InstrumentId::new("EUREX-TESX-DEC27"))
            .underlying_ticker("SX5E".to_string())
            .terms(ListedFutureTerms::new(
                100.0,
                10.0,
                Currency::EUR,
                5_250.0,
                date!(2027 - 12 - 16),
                date!(2027 - 12 - 17),
                Position::Long,
            )?)
            .spot_id(PriceId::new("SX5E-CLOSE"))
            .accrued_distributions_id(PriceId::new("TESX-ACCRUED-DISTRIBUTIONS"))
            .accrued_funding_id(PriceId::new("TESX-ACCRUED-FUNDING"))
            .spread_basis_points_id(PriceId::new("TESX-SPREAD-BPS"))
            .spread_day_count(DayCount::Act360)
            .attributes(Attributes::new())
            .build()
    }

    fn point_scalar(
        &self,
        market: &MarketContext,
        id: &PriceId,
        label: &str,
    ) -> finstack_quant_core::Result<f64> {
        let value = crate::instruments::common_impl::helpers::scalar_price_amount(
            market.get_price(id)?,
            self.terms.currency,
        )?;
        if !value.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityTotalReturnFuture {label} must be finite"
            )));
        }
        Ok(value)
    }

    fn unitless_scalar(
        market: &MarketContext,
        id: &PriceId,
        label: &str,
    ) -> finstack_quant_core::Result<f64> {
        let value = match market.get_price(id)? {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "EquityTotalReturnFuture {label} must be unitless, got Price({})",
                    money.currency()
                )));
            }
        };
        if !value.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityTotalReturnFuture {label} must be finite"
            )));
        }
        Ok(value)
    }

    fn projection(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<TrfProjection> {
        self.validate()?;
        let spot = self.point_scalar(market, &self.spot_id, "spot")?;
        if spot <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "EquityTotalReturnFuture spot must be positive".to_string(),
            ));
        }
        let distributions = self.point_scalar(
            market,
            &self.accrued_distributions_id,
            "accrued distributions",
        )?;
        let funding = self.point_scalar(market, &self.accrued_funding_id, "accrued funding")?;
        let spread_basis_points =
            Self::unitless_scalar(market, &self.spread_basis_points_id, "spread basis points")?;
        let year_fraction = self
            .spread_day_count
            .year_fraction(
                as_of,
                self.terms.settlement_date,
                DayCountContext::default(),
            )?
            .max(0.0);
        let spread_decimal = spread_basis_points * 1.0e-4;
        Ok(TrfProjection {
            price: spot + distributions - funding + spot * spread_decimal * year_fraction,
            spot_derivative: 1.0 + spread_decimal * year_fraction,
            spread_basis_points_derivative: spot * 1.0e-4 * year_fraction,
        })
    }

    /// Calculate the exchange clearing price in index points.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing spot, cumulative distributions, funding, and spread.
    /// * `as_of` - Valuation date used for the remaining spread accrual fraction.
    pub fn fair_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok(self.projection(market, as_of)?.price)
    }

    /// Resolve the live quote, model mark, or official final settlement price.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing TRF clearing inputs when a live model mark is needed.
    /// * `as_of` - Valuation date controlling live versus post-trading state.
    pub fn mark_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.terms
            .resolve_mark(self.id.as_str(), as_of, || self.fair_price(market, as_of))
    }

    /// Calculate variation-margin P&L versus the entry clearing price.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing TRF clearing inputs.
    /// * `as_of` - Valuation date controlling lifecycle and remaining spread accrual.
    pub fn npv_raw(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.terms
            .npv_from_model_price(self.id.as_str(), as_of, || self.fair_price(market, as_of))
    }

    /// P&L sensitivity to a one-point increase in the underlying index level.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the current TRF spread.
    /// * `as_of` - Valuation date for the remaining spread accrual.
    pub fn spot_delta(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok(self.terms.point_delta()? * self.projection(market, as_of)?.spot_derivative)
    }

    /// P&L sensitivity to a one-basis-point increase in the quoted TRF spread.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the current underlying level.
    /// * `as_of` - Valuation date for the remaining spread accrual.
    pub fn spread01(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok(self.terms.point_delta()?
            * self
                .projection(market, as_of)?
                .spread_basis_points_derivative)
    }

    /// P&L sensitivity to one accrued distribution index point.
    pub fn distribution_delta(&self) -> finstack_quant_core::Result<f64> {
        self.terms.point_delta()
    }

    /// P&L sensitivity to one accrued funding index point.
    pub fn funding_delta(&self) -> finstack_quant_core::Result<f64> {
        Ok(-self.terms.point_delta()?)
    }
}

impl crate::instruments::Instrument for EquityTotalReturnFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::EquityTotalReturnFuture);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        let mut dependencies = crate::instruments::MarketDependencies::new();
        dependencies.add_market_scalar_id(self.spot_id.as_str());
        dependencies.add_market_scalar_id(self.accrued_distributions_id.as_str());
        dependencies.add_market_scalar_id(self.accrued_funding_id.as_str());
        dependencies.add_market_scalar_id(self.spread_basis_points_id.as_str());
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
    EquityTotalReturnFuture,
    crate::cashflow::builder::CashflowRepresentation::NoResidual
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::Position;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use time::macros::date;

    #[test]
    fn clearing_price_and_risks_match_exchange_decomposition() {
        let as_of = date!(2026 - 01 - 01);
        let settlement = date!(2027 - 01 - 01);
        let market = MarketContext::new()
            .insert_price("SPOT", MarketScalar::Unitless(100.0))
            .insert_price("DIST", MarketScalar::Unitless(8.0))
            .insert_price("FUND", MarketScalar::Unitless(3.0))
            .insert_price("SPREAD", MarketScalar::Unitless(50.0));
        let future = EquityTotalReturnFuture::builder()
            .id(InstrumentId::new("TESX"))
            .underlying_ticker("SX5E".to_string())
            .terms(
                ListedFutureTerms::new(
                    2.0,
                    10.0,
                    Currency::EUR,
                    100.0,
                    date!(2026 - 12 - 31),
                    settlement,
                    Position::Long,
                )
                .expect("terms"),
            )
            .spot_id(PriceId::new("SPOT"))
            .accrued_distributions_id(PriceId::new("DIST"))
            .accrued_funding_id(PriceId::new("FUND"))
            .spread_basis_points_id(PriceId::new("SPREAD"))
            .spread_day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("future");

        let expected_price = 100.0 + 8.0 - 3.0 + 100.0 * 50.0e-4;
        assert!(
            (future.fair_price(&market, as_of).expect("fair") - expected_price).abs() < 1.0e-12
        );
        assert!(
            (future.spot_delta(&market, as_of).expect("spot delta") - 20.0 * 1.005).abs() < 1.0e-12
        );
        assert!(
            (future.spread01(&market, as_of).expect("spread01") - 20.0 * 100.0e-4).abs() < 1.0e-12
        );
        assert_eq!(
            future.distribution_delta().expect("distribution delta"),
            20.0
        );
        assert_eq!(future.funding_delta().expect("funding delta"), -20.0);
    }

    #[test]
    fn spread_rejects_money_scalar() {
        let as_of = date!(2026 - 01 - 01);
        let future = EquityTotalReturnFuture::example().expect("example future");
        let market = MarketContext::new()
            .insert_price("SX5E-CLOSE", MarketScalar::Unitless(100.0))
            .insert_price("TESX-ACCRUED-DISTRIBUTIONS", MarketScalar::Unitless(8.0))
            .insert_price("TESX-ACCRUED-FUNDING", MarketScalar::Unitless(3.0))
            .insert_price(
                "TESX-SPREAD-BPS",
                MarketScalar::Price(Money::from((50_i64, Currency::EUR))),
            );

        let error = future
            .fair_price(&market, as_of)
            .expect_err("money-valued spread must fail");
        assert!(error.to_string().contains("must be unitless"));
    }
}
