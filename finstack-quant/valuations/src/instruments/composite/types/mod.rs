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
    CompositeLegSpec, CompositeMarketObservation, CompositeState, RebalanceRule,
    ResolvedCompositeLeg, WeightingMethod, MAX_COMPOSITE_DEPTH, MAX_COMPOSITE_LEGS,
};

#[cfg(test)]
mod tests {
    use super::spec_support::normalized_scores;
    use super::*;
    use crate::instruments::{Instrument, InstrumentEnvelope, InstrumentJson, PricingOptions};
    use crate::metrics::MetricId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Tenor;
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

    fn equity_leg(id: &str, shares: f64, price: f64, score: f64) -> CompositeLegSpec {
        CompositeLegSpec::new(
            id,
            InstrumentJson::Equity(
                crate::instruments::Equity::new(id, id, Currency::USD)
                    .with_quantity(shares)
                    .with_quoted_spot(price),
            ),
            score,
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
                            .with_quantity(1.0)
                            .with_quoted_spot(100.0),
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
                        .with_quantity(1.0)
                        .with_quoted_spot(1.0),
                ),
                -1.0,
            ),
            CompositeLegSpec::new(
                "B",
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("B", "B", Currency::USD)
                        .with_quantity(1.0)
                        .with_quoted_spot(1.0),
                ),
                1.0,
            ),
            CompositeLegSpec::new(
                "C",
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("C", "C", Currency::USD)
                        .with_quantity(1.0)
                        .with_quoted_spot(1.0),
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
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("A", "A", Currency::USD).with_spot_id("A"),
                ),
                1.0,
            ),
            CompositeLegSpec::new(
                "B",
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("B", "B", Currency::USD).with_spot_id("B"),
                ),
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
                annualization_factor: 252.0,
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
    fn rebalance_result_wire_carries_the_canonical_instrument_envelope() -> Result<()> {
        let composite = CompositeInstrument::example()?;
        let result = composite.spec.initialize_fixed(date!(2025 - 01 - 01))?;
        let json =
            serde_json::to_value(&result).map_err(|error| Error::Internal(error.to_string()))?;
        assert_eq!(json["instrument"]["schema"], "finstack_quant.instrument/1");
        assert_eq!(json["instrument"]["instrument"]["type"], "composite");
        // The envelope is accepted by the typed instrument loader as-is.
        let envelope = serde_json::to_string(&json["instrument"])
            .map_err(|error| Error::Internal(error.to_string()))?;
        crate::pricer::parse_typed_instrument_json::<CompositeInstrument>(&envelope)?;
        let back: CompositeRebalanceResult = serde_json::from_value(json.clone())
            .map_err(|error| Error::Internal(error.to_string()))?;
        assert_eq!(
            serde_json::to_value(&back).map_err(|error| Error::Internal(error.to_string()))?,
            json
        );

        let mut wrong = json;
        wrong["instrument"] = serde_json::to_value(crate::instruments::InstrumentEnvelope::new(
            crate::instruments::Equity::example()?.into(),
        ))
        .map_err(|error| Error::Internal(error.to_string()))?;
        let error = serde_json::from_value::<CompositeRebalanceResult>(wrong)
            .expect_err("a non-composite envelope must be rejected");
        assert!(error
            .to_string()
            .contains("expected instrument type `composite`, got `equity`"));
        Ok(())
    }

    #[test]
    fn execution_rejects_conflicting_primitive_definitions_between_states() -> Result<()> {
        let previous = CompositeInstrument::example()?;
        let mut changed_spec = previous.spec.clone();
        let InstrumentJson::Equity(equity) = changed_spec.legs[0].instrument.as_mut() else {
            return Err(Error::Internal("expected equity example leg".to_string()));
        };
        equity.quoted_spot = Some(101.0);
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
            outer.primitive_exposures(&MarketContext::new(), date!(2025 - 01 - 02), &[])?;
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
            .primitive_exposures(
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

    fn deposit_pair_composite(
        own_theta: &str,
    ) -> Result<(
        CompositeInstrument,
        crate::instruments::Deposit,
        MarketContext,
    )> {
        let as_of = date!(2024 - 01 - 02);
        let curve =
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
                .build()?;
        let market = MarketContext::new().insert(curve);
        let mut own = crate::instruments::Deposit::example()?;
        own.id = InstrumentId::new("DEP-OWN");
        own.metric_pricing_overrides.theta_period = Some(Tenor::parse(own_theta)?);
        let mut inherit = crate::instruments::Deposit::example()?;
        inherit.id = InstrumentId::new("DEP-INHERIT");
        let spec = CompositeSpec::new(
            "DEP-PAIR",
            Currency::USD,
            Money::from((100_000_i64, Currency::USD)),
            vec![
                CompositeLegSpec::new("DEP-OWN", InstrumentJson::Deposit(own), 1.0),
                CompositeLegSpec::new("DEP-INHERIT", InstrumentJson::Deposit(inherit.clone()), 1.0),
            ],
            WeightingMethod::FixedQuantity,
            RebalanceRule::Manual,
        );
        Ok((spec.initialize_fixed(as_of)?.instrument, inherit, market))
    }

    fn theta_of(instrument: &dyn Instrument, market: &MarketContext) -> Result<f64> {
        let result = instrument.price_with_metrics(
            market,
            date!(2024 - 01 - 02),
            &[MetricId::Theta],
            PricingOptions::default(),
        )?;
        result
            .measures
            .get(&MetricId::Theta)
            .copied()
            .ok_or_else(|| Error::Internal("theta missing".to_string()))
    }

    #[test]
    fn composite_metric_overrides_are_leg_defaults_and_leg_settings_win() -> Result<()> {
        let (mut composite, inherit, market) = deposit_pair_composite("1D")?;
        composite.metric_pricing_overrides.theta_period = Some(Tenor::monthly());

        let result = composite.price_with_metrics(
            &market,
            date!(2024 - 01 - 02),
            &[MetricId::Theta],
            PricingOptions::default(),
        )?;
        let Some(crate::results::ValuationDetails::Composite(details)) = result.details else {
            return Err(Error::Internal("composite details missing".to_string()));
        };
        let leg_theta = |index: usize| -> Result<f64> {
            details.leg_results[index]
                .valuation
                .measures
                .get(&MetricId::Theta)
                .copied()
                .ok_or_else(|| Error::Internal("leg theta missing".to_string()))
        };

        // Reference: each leg priced standalone with the overrides it should see.
        // Same code path and inputs, so the values must agree exactly.
        let own = match composite.spec.legs[0].instrument.as_ref() {
            InstrumentJson::Deposit(deposit) => deposit.clone(),
            _ => return Err(Error::Internal("expected deposit leg".to_string())),
        };
        let mut inherit_with_default = inherit.clone();
        inherit_with_default.metric_pricing_overrides.theta_period = Some(Tenor::monthly());
        let own_expected = theta_of(&own, &market)?;
        let inherit_expected = theta_of(&inherit_with_default, &market)?;
        assert_eq!(
            leg_theta(0)?,
            own_expected,
            "leg's own 1D theta period wins"
        );
        assert_eq!(
            leg_theta(1)?,
            inherit_expected,
            "leg inherits the 1M default"
        );
        assert!(
            (inherit_expected - theta_of(&inherit, &market)?).abs() > 1.0e-6,
            "the 1M default must differ from the leg's unset (1D) theta"
        );

        let total = result
            .measures
            .get(&MetricId::Theta)
            .copied()
            .ok_or_else(|| Error::Internal("composite theta missing".to_string()))?;
        assert!(
            (total - (own_expected + inherit_expected)).abs() <= 1.0e-9 * total.abs().max(1.0),
            "primitive paths use the same defaults: {total}"
        );
        Ok(())
    }

    #[test]
    fn composite_rejects_instrument_pricing_overrides() -> Result<()> {
        let mut composite = CompositeInstrument::example()?;
        composite
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct = Some(99.0);
        let error = composite
            .validate_invariants()
            .expect_err("composite instrument overrides must be empty");
        assert!(
            error
                .to_string()
                .contains("instrument.spec.instrument_pricing_overrides"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn composite_overrides_live_at_payload_root_and_survive_rebalance() -> Result<()> {
        let mut composite = CompositeInstrument::example()?;
        composite.metric_pricing_overrides.theta_period = Some(Tenor::weekly());
        composite
            .scenario_pricing_overrides
            .scenario_price_shock_decimal = Some(-0.1);
        let value =
            serde_json::to_value(&composite).map_err(|error| Error::Internal(error.to_string()))?;
        assert_eq!(
            value["metric_pricing_overrides"]["theta_period"],
            serde_json::json!({"count": 1, "unit": "weeks"})
        );
        assert!(value["spec"].get("metric_pricing_overrides").is_none());

        let rebalanced = composite.rebalance(&MarketContext::new(), date!(2025 - 01 - 03), &[])?;
        assert_eq!(
            rebalanced.instrument.metric_pricing_overrides,
            composite.metric_pricing_overrides
        );
        assert_eq!(
            rebalanced.instrument.scenario_pricing_overrides,
            composite.scenario_pricing_overrides
        );
        Ok(())
    }

    #[test]
    fn composite_spec_rejects_retired_override_keys() -> Result<()> {
        // The three override containers moved from `spec.spec.*` to the payload root.
        for (key, retired) in [
            // schema-rejection-test
            ("instrument_pricing_overrides", serde_json::json!({})),
            // schema-rejection-test
            (
                "metric_pricing_overrides",
                serde_json::json!({"theta_period": {"count": 1, "unit": "weeks"}}),
            ),
            // schema-rejection-test
            ("scenario_pricing_overrides", serde_json::json!({})),
        ] {
            let mut value = serde_json::to_value(CompositeInstrument::example()?)
                .map_err(|error| Error::Internal(error.to_string()))?;
            value["spec"][key] = retired;
            let error = serde_json::from_value::<CompositeInstrument>(value)
                .expect_err("retired spec.spec override key must be rejected");
            assert!(
                error
                    .to_string()
                    .contains(&format!("unknown field `{key}`")),
                "spec.{key}: {error}"
            );
        }
        Ok(())
    }

    fn price_history(prices: &[f64]) -> Vec<CompositeMarketObservation> {
        prices
            .iter()
            .enumerate()
            .map(|(offset, price)| {
                let date = date!(2025 - 01 - 01) + time::Duration::days(offset as i64);
                let market = MarketContext::new()
                    .insert_price("A", MarketScalar::Unitless(*price))
                    .insert_price("B", MarketScalar::Unitless(*price));
                CompositeMarketObservation::new(date, &market)
            })
            .collect()
    }

    #[test]
    fn user_defined_volatility_column_uses_annualization_factor() -> Result<()> {
        // Unit P&L increments of 100, 102, 99, 103 are 2, -3, 4: mean 1, sum of
        // squared deviations 26, sample variance 13. With 52 periods per year the
        // annualized volatility is sqrt(13 * 52) = 26 exactly (hand-computed).
        let legs = vec![
            CompositeLegSpec::new(
                "A",
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("A", "A", Currency::USD).with_spot_id("A"),
                ),
                1.0,
            ),
            CompositeLegSpec::new(
                "B",
                InstrumentJson::Equity(
                    crate::instruments::Equity::new("B", "B", Currency::USD).with_spot_id("B"),
                ),
                -1.0,
            ),
        ];
        let expressions = IndexMap::from([
            ("A".to_string(), Expr::column("leg.A.volatility")),
            ("B".to_string(), Expr::literal(-1.0)),
        ]);
        let spec = CompositeSpec::new(
            "EXPR-VOL",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            legs,
            WeightingMethod::UserDefined {
                required_metrics: Vec::new(),
                quantity_expressions: expressions,
                annualization_factor: 52.0,
            },
            RebalanceRule::Manual,
        );
        let history = price_history(&[100.0, 102.0, 99.0, 103.0]);
        let market = history
            .last()
            .ok_or_else(|| Error::Internal("test history is empty".to_string()))?
            .restore()?;
        let resolved = spec.initialize(&market, date!(2025 - 01 - 04), &history)?;
        // Tolerance covers only floating-point summation error.
        assert!((resolved.instrument.state.resolved_legs[0].quantity - 26.0).abs() < 1.0e-9);
        assert!(
            (resolved.instrument.state.weighting_inputs["leg.A.volatility"] - 26.0).abs() < 1.0e-9
        );
        Ok(())
    }

    #[test]
    fn user_defined_rejects_non_positive_annualization_factor() {
        let spec = CompositeSpec::new(
            "EXPR-BAD",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                equity_leg("A", 1.0, 100.0, 1.0),
                equity_leg("B", 1.0, 100.0, -1.0),
            ],
            WeightingMethod::UserDefined {
                required_metrics: Vec::new(),
                quantity_expressions: IndexMap::from([
                    ("A".to_string(), Expr::literal(1.0)),
                    ("B".to_string(), Expr::literal(-1.0)),
                ]),
                annualization_factor: 0.0,
            },
            RebalanceRule::Manual,
        );
        assert!(spec.validate().is_err());
    }

    #[test]
    fn notional_weighting_records_signed_notional() -> Result<()> {
        // Leg A is short two shares at 100: signed notional -200. The recorded
        // input keeps the sign; only the quantity formula uses |notional|.
        let spec = CompositeSpec::new(
            "SIGNED-NOTIONAL",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                equity_leg("A", -2.0, 100.0, 1.0),
                equity_leg("B", 1.0, 50.0, -1.0),
            ],
            WeightingMethod::NotionalWeighted {
                gross_notional: Money::from((1_000_i64, Currency::USD)),
            },
            RebalanceRule::Manual,
        );
        let resolved = spec.initialize(&MarketContext::new(), date!(2025 - 01 - 01), &[])?;
        let state = &resolved.instrument.state;
        assert_eq!(state.weighting_inputs["leg.A.notional"], -200.0);
        assert_eq!(state.weighting_inputs["leg.B.notional"], 50.0);
        // q_A = +1 * 1000 * 0.5 / 200, q_B = -1 * 1000 * 0.5 / 50.
        assert_eq!(state.resolved_legs[0].quantity, 2.5);
        assert_eq!(state.resolved_legs[1].quantity, -10.0);
        Ok(())
    }

    #[test]
    fn calendar_rebalance_accepts_any_tenor_cadence() -> Result<()> {
        let rule = RebalanceRule::Calendar {
            start: date!(2025 - 01 - 01),
            end: Some(date!(2026 - 01 - 01)),
            frequency: Tenor::parse("6M")?,
            calendar_id: "weekends_only".to_string(),
            business_day_convention: finstack_quant_core::dates::BusinessDayConvention::Following,
        };
        rule.validate()?;
        let dates = rule.dates_through(date!(2026 - 01 - 01))?;
        assert_eq!(
            dates,
            vec![
                date!(2025 - 01 - 01),
                date!(2025 - 07 - 01),
                date!(2026 - 01 - 01)
            ]
        );
        Ok(())
    }

    #[test]
    fn retired_composite_wire_keys_are_rejected() -> Result<()> {
        let composite = CompositeInstrument::example()?;
        let mut leg = serde_json::to_value(&composite.spec.legs[0])
            .map_err(|error| Error::Internal(error.to_string()))?;
        let object = leg
            .as_object_mut()
            .ok_or_else(|| Error::Internal("leg is not an object".to_string()))?;
        let score = object
            .remove("score")
            .ok_or_else(|| Error::Internal("leg has no score".to_string()))?;
        // schema-rejection-test: the retired `weight` spelling of `score`.
        object.insert("weight".to_string(), score);
        assert!(serde_json::from_value::<CompositeLegSpec>(leg).is_err());

        let calendar = serde_json::json!({
            "kind": "calendar",
            "start": "2025-01-01",
            // schema-rejection-test: the retired `RebalanceFrequency` string cadence.
            "frequency": "monthly",
            "calendar_id": "weekends_only",
            "business_day_convention": "following"
        });
        assert!(serde_json::from_value::<RebalanceRule>(calendar).is_err());

        let user_defined = serde_json::json!({
            "kind": "user_defined",
            "required_metrics": [],
            "quantity_expressions": {}
        });
        assert!(serde_json::from_value::<WeightingMethod>(user_defined).is_err());

        let result = composite.price_with_metrics(
            &MarketContext::new(),
            date!(2025 - 01 - 02),
            &[],
            PricingOptions::default(),
        )?;
        let Some(crate::results::ValuationDetails::Composite(details)) = result.details else {
            return Err(Error::Internal("composite details are missing".to_string()));
        };
        let mut details =
            serde_json::to_value(&details).map_err(|error| Error::Internal(error.to_string()))?;
        let object = details
            .as_object_mut()
            .ok_or_else(|| Error::Internal("details is not an object".to_string()))?;
        let exposures = object
            .remove("exposures")
            .ok_or_else(|| Error::Internal("details has no exposures".to_string()))?;
        // schema-rejection-test: the retired `exposure_report` spelling of `exposures`.
        object.insert("exposure_report".to_string(), exposures);
        assert!(serde_json::from_value::<CompositeValuationDetails>(details).is_err());
        Ok(())
    }
}
