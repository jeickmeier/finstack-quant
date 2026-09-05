//! Linear futures on a directly observable or averaged price.

use crate::impl_instrument_base;
use crate::instruments::common_impl::listed::ListedFutureTerms;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Exchange final-settlement rule for a linear commodity future.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommodityFutureSettlement {
    /// One official observation at the supplied date.
    Single {
        /// Date whose forward or realized price determines final settlement.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        observation_date: Date,
        /// Official observed price once the observation date has passed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        realized_price: Option<f64>,
    },
    /// Equal-weight arithmetic average over explicit exchange observation dates.
    ArithmeticAverage {
        /// Ordered, unique exchange observation dates.
        #[serde(with = "finstack_quant_core::wire::dates")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
        )]
        fixing_dates: Vec<Date>,
        /// Official prices already fixed, keyed by observation date.
        #[serde(default, with = "finstack_quant_core::wire::dated_f64_values")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<(finstack_quant_core::wire::DateWire, f64)>")
        )]
        realized_fixings: Vec<(Date, f64)>,
    },
}

/// Exchange-listed future whose settlement is one price or an average of prices.
///
/// This covers linear commodity and price-index futures, including monthly
/// average-settled contracts such as iron ore, energy, and freight futures.
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
pub struct CommodityFuture {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Exchange symbol or underlying label.
    pub underlying: String,
    /// Standard listed position and lifecycle terms.
    pub terms: ListedFutureTerms,
    /// Price-curve identifier used for projected observations.
    pub price_curve_id: CurveId,
    /// Official final-settlement observation rule.
    pub settlement: CommodityFutureSettlement,
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

impl CommodityFuture {
    /// Validate settlement dates, fixings, and listed terms.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.terms.validate()?;
        if self.underlying.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "CommodityFuture underlying must not be empty".to_string(),
            ));
        }
        match &self.settlement {
            CommodityFutureSettlement::Single {
                observation_date,
                realized_price,
            } => {
                if *observation_date > self.terms.settlement_date {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CommodityFuture observation_date {observation_date} must not be after settlement_date {}",
                        self.terms.settlement_date
                    )));
                }
                if realized_price.is_some_and(|price| !price.is_finite()) {
                    return Err(finstack_quant_core::Error::Validation(
                        "CommodityFuture realized_price must be finite when supplied".to_string(),
                    ));
                }
            }
            CommodityFutureSettlement::ArithmeticAverage {
                fixing_dates,
                realized_fixings,
            } => {
                if fixing_dates.is_empty() {
                    return Err(finstack_quant_core::Error::Validation(
                        "CommodityFuture arithmetic average requires at least one fixing date"
                            .to_string(),
                    ));
                }
                if fixing_dates.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err(finstack_quant_core::Error::Validation(
                        "CommodityFuture fixing_dates must be strictly increasing and unique"
                            .to_string(),
                    ));
                }
                if fixing_dates
                    .last()
                    .is_some_and(|date| *date > self.terms.settlement_date)
                {
                    return Err(finstack_quant_core::Error::Validation(
                        "CommodityFuture fixing_dates must not extend past settlement_date"
                            .to_string(),
                    ));
                }
                for (date, price) in realized_fixings {
                    if !fixing_dates.contains(date) || !price.is_finite() {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "CommodityFuture realized fixing ({date}, {price}) must be finite and match a fixing date"
                        )));
                    }
                }
                let mut realized_dates = realized_fixings
                    .iter()
                    .map(|(date, _)| *date)
                    .collect::<Vec<_>>();
                realized_dates.sort_unstable();
                if realized_dates.windows(2).any(|pair| pair[0] == pair[1]) {
                    return Err(finstack_quant_core::Error::Validation(
                        "CommodityFuture realized fixings must have unique dates".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Create a canonical monthly average-settled commodity future.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use crate::instruments::Position;
        use finstack_quant_core::currency::Currency;
        use time::macros::date;

        Self::builder()
            .id(InstrumentId::new("SGX-IRON-ORE-DEC26"))
            .underlying("TSI-62-FE".to_string())
            .terms(ListedFutureTerms::new(
                10.0,
                100.0,
                Currency::USD,
                105.25,
                date!(2026 - 12 - 31),
                date!(2027 - 01 - 04),
                Position::Long,
            )?)
            .price_curve_id(CurveId::new("IRON-ORE-FORWARD"))
            .settlement(CommodityFutureSettlement::ArithmeticAverage {
                fixing_dates: vec![
                    date!(2026 - 12 - 29),
                    date!(2026 - 12 - 30),
                    date!(2026 - 12 - 31),
                ],
                realized_fixings: Vec::new(),
            })
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate the model final-settlement price from realized and projected observations.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the configured price curve.
    /// * `as_of` - Valuation date; observations strictly before it must be realized.
    pub fn model_settlement_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        match &self.settlement {
            CommodityFutureSettlement::Single {
                observation_date,
                realized_price,
            } => {
                if *observation_date < as_of {
                    realized_price.ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "CommodityFuture '{}' requires realized_price for observation {} before as_of {}",
                            self.id, observation_date, as_of
                        ))
                    })
                } else {
                    market
                        .get_price_curve(self.price_curve_id.as_str())?
                        .price_on_date(*observation_date)
                }
            }
            CommodityFutureSettlement::ArithmeticAverage {
                fixing_dates,
                realized_fixings,
            } => {
                let curve = if fixing_dates.iter().any(|date| *date >= as_of) {
                    Some(market.get_price_curve(self.price_curve_id.as_str())?)
                } else {
                    None
                };
                let mut sum = finstack_quant_core::math::NeumaierAccumulator::new();
                for fixing_date in fixing_dates {
                    let price = if *fixing_date < as_of {
                        realized_fixings
                            .iter()
                            .find_map(|(date, price)| (date == fixing_date).then_some(*price))
                            .ok_or_else(|| {
                                finstack_quant_core::Error::Validation(format!(
                                    "CommodityFuture '{}' requires realized fixing for {} before as_of {}",
                                    self.id, fixing_date, as_of
                                ))
                            })?
                    } else {
                        curve
                            .as_ref()
                            .ok_or_else(|| {
                                finstack_quant_core::Error::Validation(
                                    "CommodityFuture projected fixing requires price curve"
                                        .to_string(),
                                )
                            })?
                            .price_on_date(*fixing_date)?
                    };
                    sum.add(price);
                }
                Ok(sum.total() / fixing_dates.len() as f64)
            }
        }
    }

    /// Resolve the live quote, model mark, or official final settlement price.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the configured price curve when a live model mark is needed.
    /// * `as_of` - Valuation date controlling live versus post-trading state.
    pub fn mark_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.terms.resolve_mark(self.id.as_str(), as_of, || {
            self.model_settlement_price(market, as_of)
        })
    }

    /// Calculate variation-margin P&L versus the trade fill.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the configured price curve.
    /// * `as_of` - Valuation date controlling projected versus realized observations.
    pub fn npv_raw(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.terms
            .npv_from_model_price(self.id.as_str(), as_of, || {
                self.model_settlement_price(market, as_of)
            })
    }

    /// Sensitivity to a parallel one-unit increase in every projected price observation.
    ///
    /// Realized observations carry no curve risk. For an arithmetic average,
    /// each remaining observation contributes its equal settlement weight.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date separating realized from projected observations.
    pub fn price_curve_delta(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        let projected_weight = match &self.settlement {
            CommodityFutureSettlement::Single {
                observation_date, ..
            } => f64::from(*observation_date >= as_of),
            CommodityFutureSettlement::ArithmeticAverage { fixing_dates, .. } => {
                fixing_dates.iter().filter(|date| **date >= as_of).count() as f64
                    / fixing_dates.len() as f64
            }
        };
        Ok(self.terms.point_delta()? * projected_weight)
    }
}

impl crate::instruments::Instrument for CommodityFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::CommodityFuture);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        let mut dependencies = crate::instruments::MarketDependencies::new();
        dependencies.add_forward_curve(self.price_curve_id.clone());
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
    CommodityFuture,
    crate::cashflow::builder::CashflowRepresentation::NoResidual
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::Position;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::PriceCurve;
    use time::macros::date;

    fn average_future(realized_fixings: Vec<(Date, f64)>) -> CommodityFuture {
        CommodityFuture::builder()
            .id(InstrumentId::new("AVG"))
            .underlying("INDEX".to_string())
            .terms(
                ListedFutureTerms::new(
                    2.0,
                    100.0,
                    Currency::USD,
                    90.0,
                    date!(2026 - 01 - 05),
                    date!(2026 - 01 - 06),
                    Position::Long,
                )
                .expect("valid terms"),
            )
            .price_curve_id(CurveId::new("INDEX-FWD"))
            .settlement(CommodityFutureSettlement::ArithmeticAverage {
                fixing_dates: vec![
                    date!(2026 - 01 - 02),
                    date!(2026 - 01 - 05),
                    date!(2026 - 01 - 06),
                ],
                realized_fixings,
            })
            .attributes(Attributes::new())
            .build()
            .expect("valid future")
    }

    fn flat_market() -> MarketContext {
        let curve = PriceCurve::builder("INDEX-FWD")
            .base_date(date!(2026 - 01 - 01))
            .spot_price(100.0)
            .knots([(0.0, 100.0), (1.0, 100.0)])
            .build()
            .expect("valid price curve");
        MarketContext::new().insert(curve)
    }

    #[test]
    fn average_settlement_combines_realized_and_projected_prices() {
        let future = average_future(vec![(date!(2026 - 01 - 02), 80.0)]);
        let as_of = date!(2026 - 01 - 05);
        let settlement = future
            .model_settlement_price(&flat_market(), as_of)
            .expect("settlement");
        assert!((settlement - (80.0 + 100.0 + 100.0) / 3.0).abs() < 1.0e-12);
        assert!(
            (future.price_curve_delta(as_of).expect("delta") - 200.0 * 2.0 / 3.0).abs() < 1.0e-12
        );
    }

    #[test]
    fn missing_realized_average_fixing_is_rejected() {
        let future = average_future(Vec::new());
        let error = future
            .model_settlement_price(&flat_market(), date!(2026 - 01 - 05))
            .expect_err("missing fixing must fail");
        assert!(error.to_string().contains("2026-01-02"));
    }

    #[test]
    fn fully_realized_average_does_not_require_price_curve() {
        let future = average_future(vec![
            (date!(2026 - 01 - 02), 80.0),
            (date!(2026 - 01 - 05), 90.0),
            (date!(2026 - 01 - 06), 100.0),
        ]);
        let settlement = future
            .model_settlement_price(&MarketContext::new(), date!(2026 - 01 - 07))
            .expect("fully realized settlement");
        assert_eq!(settlement, 90.0);
    }
}
