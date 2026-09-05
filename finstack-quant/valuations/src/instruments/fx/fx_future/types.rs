//! Exchange-listed FX futures under a deterministic-rate CIP approximation.
//!
//! The model sets the futures mark equal to the corresponding FX forward.
//! It does not include forward/futures convexity from stochastic domestic
//! rates, foreign rates, or their correlations with FX.

use crate::impl_instrument_base;
use crate::instruments::common_impl::listed::ListedFutureTerms;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fx::shared::{covered_interest_parity_forward, FxForwardRateRequest};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Exchange-listed future on a deliverable currency pair.
///
/// Prices are quote-currency units per one base-currency unit. The listed
/// multiplier is the base-currency contract size, so a one-unit price move is
/// worth `multiplier` units of the quote currency per contract.
///
/// # Model limitation
///
/// [`Self::fair_price`] is a deterministic-rate approximation: it equals the
/// covered-interest-parity forward. Use it only when forward/futures convexity
/// is immaterial or handled outside this instrument.
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
pub struct FxFuture {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Base currency, the numerator of the quoted pair.
    pub base_currency: Currency,
    /// Quote and variation-margin currency.
    pub quote_currency: Currency,
    /// Standard listed position and lifecycle terms.
    pub terms: ListedFutureTerms,
    /// Quote-currency discount curve.
    pub domestic_discount_curve_id: CurveId,
    /// Base-currency discount curve.
    pub foreign_discount_curve_id: CurveId,
    /// Optional spot override in quote currency per base currency.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_rate_override: Option<f64>,
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

impl FxFuture {
    /// Validate currency, price, and position invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.terms.validate()?;
        if self.base_currency == self.quote_currency {
            return Err(finstack_quant_core::Error::Validation(
                "FxFuture base_currency and quote_currency must differ".to_string(),
            ));
        }
        if self.terms.currency != self.quote_currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxFuture terms currency {} must equal quote_currency {}",
                self.terms.currency, self.quote_currency
            )));
        }
        for (name, value) in [
            ("entry_price", Some(self.terms.entry_price)),
            ("quoted_price", self.terms.quoted_price),
            ("settlement_price", self.terms.settlement_price),
            ("spot_rate_override", self.spot_rate_override),
        ] {
            if value.is_some_and(|rate| !rate.is_finite() || rate <= 0.0) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FxFuture {name} must be finite and positive"
                )));
            }
        }
        Ok(())
    }

    /// Create a canonical CME EUR/USD future example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use crate::instruments::Position;
        use time::macros::date;

        Self::builder()
            .id(InstrumentId::new("CME-6E-DEC26"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .terms(ListedFutureTerms::new(
                4.0,
                125_000.0,
                Currency::USD,
                1.10,
                date!(2026 - 12 - 14),
                date!(2026 - 12 - 16),
                Position::Long,
            )?)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate the fair futures price from spot and the two discount curves.
    ///
    /// This is the deterministic-rate approximation `F = S × DF_base / DF_quote`.
    /// No stochastic-rate/FX convexity adjustment is applied.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the FX matrix and both discount curves.
    /// * `as_of` - Valuation date used for spot and date-based discount factors.
    pub fn fair_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        covered_interest_parity_forward(FxForwardRateRequest {
            market,
            as_of,
            maturity: self.terms.settlement_date,
            base_currency: self.base_currency,
            quote_currency: self.quote_currency,
            domestic_discount_curve_id: &self.domestic_discount_curve_id,
            foreign_discount_curve_id: &self.foreign_discount_curve_id,
            spot_rate_override: self.spot_rate_override,
            context: "FxFuture",
        })
    }

    /// Resolve the live quote, model mark, or official final settlement price.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing spot and rate inputs when a live model mark is needed.
    /// * `as_of` - Valuation date controlling live versus post-trading state.
    pub fn mark_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.terms
            .resolve_mark(self.id.as_str(), as_of, || self.fair_price(market, as_of))
    }

    /// Calculate variation-margin P&L without forward-style discounting.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing spot and rate inputs.
    /// * `as_of` - Valuation date controlling the contract lifecycle.
    pub fn npv_raw(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.terms
            .npv_from_model_price(self.id.as_str(), as_of, || self.fair_price(market, as_of))
    }

    /// Sensitivity to a one-unit move in the quoted FX futures price.
    pub fn futures_price_delta(&self) -> finstack_quant_core::Result<f64> {
        self.terms.point_delta()
    }
}

impl crate::instruments::Instrument for FxFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::FxFuture);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        let mut dependencies = crate::instruments::MarketDependencies::new();
        dependencies.add_discount_curve(self.domestic_discount_curve_id.clone());
        dependencies.add_discount_curve(self.foreign_discount_curve_id.clone());
        dependencies.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(dependencies)
    }

    fn fx_exposure(&self) -> Option<(Currency, Currency)> {
        Some((self.base_currency, self.quote_currency))
    }

    fn base_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Money::new(self.npv_raw(market, as_of)?, self.quote_currency)
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
    FxFuture,
    crate::cashflow::builder::CashflowRepresentation::NoResidual
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::Position;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    #[test]
    fn fx_future_uses_cirp_and_does_not_discount_variation_margin_pnl() {
        let as_of = date!(2026 - 01 - 01);
        let settlement = date!(2027 - 01 - 01);
        let domestic = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
            .build()
            .expect("domestic curve");
        let foreign = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.02_f64).exp())])
            .build()
            .expect("foreign curve");
        let market = MarketContext::new().insert(domestic).insert(foreign);
        let future = FxFuture::builder()
            .id(InstrumentId::new("6E"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .terms(
                ListedFutureTerms::new(
                    2.0,
                    125_000.0,
                    Currency::USD,
                    1.10,
                    date!(2026 - 12 - 30),
                    settlement,
                    Position::Long,
                )
                .expect("terms"),
            )
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .spot_rate_override(1.10)
            .attributes(Attributes::new())
            .build()
            .expect("future");

        let fair = future.fair_price(&market, as_of).expect("fair price");
        let expected_fair = 1.10 * (-0.02_f64).exp() / (-0.04_f64).exp();
        assert!((fair - expected_fair).abs() < 1.0e-12);
        let expected_pnl = 2.0 * 125_000.0 * (expected_fair - 1.10);
        assert!((future.npv_raw(&market, as_of).expect("pv") - expected_pnl).abs() < 1.0e-8);
    }
}
