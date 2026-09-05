//! Composite instrument types: specification, resolved instrument, and reporting.

mod instrument;
mod reporting;
mod spec;
mod spec_support;

pub use instrument::CompositeInstrument;
pub use reporting::{
    CompositeExposureReport, CompositeLegValuation, CompositeRebalanceResult, CompositeTrade,
    CompositeValuationDetails, PrimitiveAggregate, PrimitiveExposure,
};
pub use spec::CompositeSpec;
pub(crate) use spec_support::{cashflows_between, validate_history};
pub use spec_support::{
    CompositeLegSpec, CompositeMarketObservation, CompositeState, RebalanceFrequency,
    RebalanceRule, ResolvedCompositeLeg, WeightingMethod, MAX_COMPOSITE_DEPTH, MAX_COMPOSITE_LEGS,
};

#[cfg(test)]
mod tests {
    use super::spec_support::normalized_scores;
    use super::*;
    use crate::instruments::{Instrument, InstrumentEnvelope, InstrumentJson, PricingOptions};
    use crate::metrics::MetricId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::expr::Expr;
    use finstack_quant_core::expr::UnaryOp;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_core::{Error, Result};
    use indexmap::IndexMap;
    use std::sync::Arc;
    use time::macros::date;

    fn equity_leg(id: &str, shares: f64, price: f64, weight: f64) -> CompositeLegSpec {
        CompositeLegSpec::new(
            id,
            InstrumentJson::Equity(
                crate::instruments::Equity::new(id, id, Currency::USD)
                    .with_shares(shares)
                    .with_price(price),
            ),
            weight,
        )
    }

    #[test]
    fn fixed_composite_values_and_decomposes() -> Result<()> {
        let composite = CompositeInstrument::example()?;
        let value = composite.value(&MarketContext::new(), date!(2025 - 01 - 02))?;
        assert_eq!(value.currency(), Currency::USD);
        assert!((value.amount() - 10.0).abs() < 1.0e-9);

        let primitives = composite.flatten_primitives()?;
        assert_eq!(primitives.len(), 2);
        assert_eq!(primitives[0].quantity, 1.0);
        assert_eq!(primitives[1].quantity, -1.0);
        Ok(())
    }

    #[test]
    fn cross_currency_values_and_dependencies_use_reporting_fx() -> Result<()> {
        let spec = CompositeSpec::new(
            "USD-EUR",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                equity_leg("USD-LEG", 1.0, 100.0, 1.0),
                CompositeLegSpec::new(
                    "EUR-LEG",
                    InstrumentJson::Equity(
                        crate::instruments::Equity::new("EUR-LEG", "EUR-LEG", Currency::EUR)
                            .with_shares(1.0)
                            .with_price(100.0),
                    ),
                    1.0,
                ),
            ],
            WeightingMethod::FixedQuantity,
            RebalanceRule::Manual,
        );
        let composite = spec.initialize_fixed(date!(2025 - 01 - 01))?.instrument;
        let provider = Arc::new(SimpleFxProvider::new());
        provider.set_quote(Currency::EUR, Currency::USD, 1.2)?;
        let market = MarketContext::new().insert_fx(FxMatrix::new(provider));

        let dependencies = composite.market_dependencies()?;
        assert!(dependencies
            .fx_pairs
            .iter()
            .any(|pair| { pair.base == Currency::EUR && pair.quote == Currency::USD }));
        let result = composite.price_with_metrics(
            &market,
            date!(2025 - 01 - 02),
            &[],
            PricingOptions::default(),
        )?;
        assert_eq!(result.value.amount(), 220.0);
        let Some(crate::results::ValuationDetails::Composite(details)) = result.details else {
            return Err(Error::Internal(
                "cross-currency composite details are missing".to_string(),
            ));
        };
        assert_eq!(details.leg_results[1].native_value.amount(), 100.0);
        assert_eq!(
            details.leg_results[1].native_value.currency(),
            Currency::EUR
        );
        assert_eq!(details.leg_results[1].reporting_value.amount(), 120.0);
        Ok(())
    }

    #[test]
    fn neutral_scores_split_butterfly_wings() -> Result<()> {
        let legs = vec![
            CompositeLegSpec::new(
                "A",
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("A", "A", Currency::USD)
                        .with_shares(1.0)
                        .with_price(1.0),
                ),
                -1.0,
            ),
            CompositeLegSpec::new(
                "B",
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("B", "B", Currency::USD)
                        .with_shares(1.0)
                        .with_price(1.0),
                ),
                1.0,
            ),
            CompositeLegSpec::new(
                "C",
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("C", "C", Currency::USD)
                        .with_shares(1.0)
                        .with_price(1.0),
                ),
                -3.0,
            ),
        ];
        assert_eq!(normalized_scores(&legs, true)?, vec![-0.25, 1.0, -0.75]);
        Ok(())
    }

    #[test]
    fn notional_weighting_normalizes_requested_gross() -> Result<()> {
        let spec = CompositeSpec::new(
            "NOTIONAL",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                equity_leg("A", 1.0, 100.0, 1.0),
                equity_leg("B", 2.0, 50.0, -3.0),
            ],
            WeightingMethod::NotionalWeighted {
                gross_notional: Money::from((300_i64, Currency::USD)),
            },
            RebalanceRule::Manual,
        );
        let resolved = spec.initialize(&MarketContext::new(), date!(2025 - 01 - 01), &[])?;
        assert_eq!(resolved.instrument.state.resolved_legs[0].quantity, 0.75);
        assert_eq!(resolved.instrument.state.resolved_legs[1].quantity, -2.25);
        let gross = resolved
            .instrument
            .state
            .resolved_legs
            .iter()
            .map(|leg| leg.quantity.abs() * 100.0)
            .sum::<f64>();
        assert!((gross - 300.0).abs() < 1.0e-12);
        Ok(())
    }

    #[test]
    fn delta_neutral_weighting_uses_unit_metrics_and_anchor_scale() -> Result<()> {
        let spec = CompositeSpec::new(
            "DELTA",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                equity_leg("A", 2.0, 100.0, 1.0),
                equity_leg("B", 4.0, 100.0, -1.0),
            ],
            WeightingMethod::delta_neutral("A", 1.0),
            RebalanceRule::Manual,
        );
        let resolved = spec.initialize(&MarketContext::new(), date!(2025 - 01 - 01), &[])?;
        assert_eq!(resolved.instrument.state.resolved_legs[0].quantity, 1.0);
        assert_eq!(resolved.instrument.state.resolved_legs[1].quantity, -0.5);
        assert!((1.0_f64 * 2.0 + -0.5 * 4.0).abs() < 1.0e-12);
        Ok(())
    }

    #[test]
    fn volatility_weighting_uses_one_unit_total_pnl() -> Result<()> {
        let legs = vec![
            CompositeLegSpec::new(
                "A",
                InstrumentJson::Equity(crate::instruments::Equity::new("A", "A", Currency::USD)),
                1.0,
            ),
            CompositeLegSpec::new(
                "B",
                InstrumentJson::Equity(crate::instruments::Equity::new("B", "B", Currency::USD)),
                -1.0,
            ),
        ];
        let spec = CompositeSpec::new(
            "VOL",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            legs,
            WeightingMethod::volatility_weighted("A", 1.0, 3, 3, 252.0),
            RebalanceRule::Manual,
        );
        let observations = [(100.0, 100.0), (102.0, 104.0), (99.0, 98.0), (103.0, 106.0)]
            .into_iter()
            .enumerate()
            .map(|(offset, (a, b))| {
                let date = date!(2025 - 01 - 01) + time::Duration::days(offset as i64);
                let market = MarketContext::new()
                    .insert_price("A", MarketScalar::Unitless(a))
                    .insert_price("B", MarketScalar::Unitless(b));
                CompositeMarketObservation::new(date, &market)
            })
            .collect::<Vec<_>>();
        let market = observations
            .last()
            .ok_or_else(|| Error::Internal("test history is empty".to_string()))?
            .restore()?;
        let resolved = spec.initialize(&market, date!(2025 - 01 - 04), &observations)?;
        assert!((resolved.instrument.state.resolved_legs[0].quantity - 1.0).abs() < 1.0e-12);
        assert!((resolved.instrument.state.resolved_legs[1].quantity + 0.5).abs() < 1.0e-12);
        Ok(())
    }

    #[test]
    fn user_defined_expressions_resolve_quantities() -> Result<()> {
        let expressions = IndexMap::from([
            ("A".to_string(), Expr::literal(2.0)),
            (
                "B".to_string(),
                Expr::unary_op(UnaryOp::Neg, Expr::literal(3.0)),
            ),
        ]);
        let spec = CompositeSpec::new(
            "EXPR",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                equity_leg("A", 1.0, 100.0, 1.0),
                equity_leg("B", 1.0, 100.0, -1.0),
            ],
            WeightingMethod::UserDefined {
                required_metrics: Vec::new(),
                quantity_expressions: expressions,
            },
            RebalanceRule::Manual,
        );
        let resolved = spec.initialize(&MarketContext::new(), date!(2025 - 01 - 01), &[])?;
        assert_eq!(resolved.instrument.state.resolved_legs[0].quantity, 2.0);
        assert_eq!(resolved.instrument.state.resolved_legs[1].quantity, -3.0);
        Ok(())
    }

    #[test]
    fn fixed_state_rejects_mismatched_identifier() -> Result<()> {
        let mut composite = CompositeInstrument::example()?;
        composite.state.resolved_legs[0].instrument_id = InstrumentId::new("WRONG");
        assert!(composite.validate_invariants().is_err());
        Ok(())
    }

    #[test]
    fn execution_rejects_conflicting_primitive_definitions_between_states() -> Result<()> {
        let previous = CompositeInstrument::example()?;
        let mut changed_spec = previous.spec.clone();
        let InstrumentJson::Equity(equity) = changed_spec.legs[0].instrument.as_mut() else {
            return Err(Error::Internal("expected equity example leg".to_string()));
        };
        equity.price_quote = Some(101.0);
        let current = changed_spec
            .initialize_fixed(date!(2025 - 01 - 02))?
            .instrument;
        let error = current
            .execution_trades(Some(&previous))
            .expect_err("same primitive ID with changed economics must be rejected");
        assert!(error.to_string().contains("conflicting definitions"));
        Ok(())
    }

    #[test]
    fn nested_composites_report_net_and_gross_repeated_primitives() -> Result<()> {
        let a = equity_leg("A", 1.0, 100.0, 1.0);
        let inner = CompositeSpec::new(
            "INNER",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![a.clone(), equity_leg("B", 1.0, 90.0, -1.0)],
            WeightingMethod::FixedQuantity,
            RebalanceRule::Manual,
        )
        .initialize_fixed(date!(2025 - 01 - 01))?
        .instrument;
        let outer = CompositeSpec::new(
            "OUTER",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                CompositeLegSpec::new("INNER", InstrumentJson::Composite(Box::new(inner)), 2.0),
                CompositeLegSpec::new("A", (*a.instrument).clone(), -1.0),
            ],
            WeightingMethod::FixedQuantity,
            RebalanceRule::Manual,
        )
        .initialize_fixed(date!(2025 - 01 - 01))?
        .instrument;

        let report =
            outer.primitive_exposure_report(&MarketContext::new(), date!(2025 - 01 - 02), &[])?;
        assert_eq!(report.paths.len(), 3);
        let a = report
            .aggregates
            .iter()
            .find(|aggregate| aggregate.instrument_id.as_str() == "A")
            .ok_or_else(|| Error::Internal("missing repeated primitive A".to_string()))?;
        assert_eq!(a.net_quantity, 1.0);
        assert_eq!(a.gross_quantity, 3.0);
        assert_eq!(a.net_value.amount(), 100.0);
        assert_eq!(a.gross_value.amount(), 300.0);
        Ok(())
    }

    #[test]
    fn metric_pricing_never_changes_resolved_quantities() -> Result<()> {
        let composite = CompositeInstrument::example()?;
        let before = serde_json::to_value(&composite.state).map_err(|error| {
            Error::Internal(format!("failed to serialize composite state: {error}"))
        })?;
        let result = composite.price_with_metrics(
            &MarketContext::new(),
            date!(2025 - 01 - 02),
            &[],
            PricingOptions::default(),
        )?;
        let Some(crate::results::ValuationDetails::Composite(details)) = result.details else {
            return Err(Error::Internal(
                "composite valuation did not retain structured details".to_string(),
            ));
        };
        assert_eq!(details.resolved_legs.len(), 2);
        assert_eq!(details.leg_results.len(), 2);
        assert_eq!(details.leg_results[0].native_value.amount(), 100.0);
        assert_eq!(details.leg_results[0].reporting_value.amount(), 100.0);
        assert_eq!(details.leg_results[1].native_value.amount(), -90.0);
        assert_eq!(
            details.leg_results[1].valuation.instrument_id,
            "COMPOSITE-SHORT"
        );
        let after = serde_json::to_value(&composite.state).map_err(|error| {
            Error::Internal(format!("failed to serialize composite state: {error}"))
        })?;
        assert_eq!(before, after);
        Ok(())
    }

    #[test]
    fn non_additive_metrics_are_rejected_at_composite_level() -> Result<()> {
        let composite = CompositeInstrument::example()?;
        let error = composite
            .primitive_exposure_report(
                &MarketContext::new(),
                date!(2025 - 01 - 02),
                &[MetricId::DurationMod],
            )
            .expect_err("modified duration is non-additive");
        assert!(error.to_string().contains("not additive"));
        Ok(())
    }

    #[test]
    fn composite_envelope_round_trips_through_strict_loader() -> Result<()> {
        let composite = CompositeInstrument::example()?;
        let envelope = InstrumentEnvelope::new(InstrumentJson::Composite(Box::new(composite)));
        let json = serde_json::to_vec(&envelope).map_err(|error| {
            Error::Internal(format!("failed to serialize composite envelope: {error}"))
        })?;
        let (loaded, report) = InstrumentEnvelope::from_slice_strict(
            &json,
            &finstack_quant_core::LoadLimits::default(),
        )
        .map_err(|error| Error::Validation(error.to_string()))?;
        assert!(!report.has_errors());
        assert_eq!(loaded.id(), "COMPOSITE-EXAMPLE");
        assert_eq!(loaded.key(), crate::pricer::InstrumentType::Composite);
        Ok(())
    }
}
