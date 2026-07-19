//! Portfolio-level factor assignment, decomposition, and what-if analysis.
//!
use super::model::FactorModel;
use super::RiskDecomposition;
use crate::error::{Error, Result};
use crate::evaluation::{
    evaluate_raw_portfolio, PositionExecution, RawEvaluationInput, RawSelectiveSeed,
};
use crate::position::Position;
use crate::sensitivity::SensitivityMatrix;
use crate::types::PositionId;
use crate::Portfolio;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_factor_model::FactorId;

/// Base/after delta for a single factor contribution.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FactorContributionDelta {
    /// Factor identifier whose contribution changed.
    pub factor_id: FactorId,
    /// Absolute change in the reported risk contribution.
    pub absolute_change: f64,
    /// Relative change in the reported risk contribution.
    pub relative_change: f64,
}

/// Result of a position what-if scenario.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WhatIfResult {
    /// Baseline decomposition used as the comparison point.
    pub before: RiskDecomposition,
    /// Decomposition after applying the requested position changes.
    pub after: RiskDecomposition,
    /// Per-factor changes between `before` and `after`.
    pub delta: Vec<FactorContributionDelta>,
}

/// Result of a factor-stress scenario.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StressResult {
    /// Total portfolio P&L under the stressed market.
    pub total_pnl: f64,
    /// Per-position P&L contributions.
    pub position_pnl: Vec<(PositionId, f64)>,
    /// Risk decomposition recomputed under the stressed market.
    pub stressed_decomposition: RiskDecomposition,
}

/// Position edits supported by `WhatIfEngine::position_what_if`.
#[derive(Debug, Clone)]
pub enum PositionChange {
    /// Add a new position. This currently requires recomputing sensitivities from scratch.
    Add {
        /// Position to add to the scenario portfolio.
        position: Box<Position>,
    },
    /// Remove an existing position by identifier.
    Remove {
        /// Position identifier to remove from the scenario.
        position_id: PositionId,
    },
    /// Resize an existing position to a new quantity.
    Resize {
        /// Position identifier to resize.
        position_id: PositionId,
        /// Replacement quantity for the position.
        new_quantity: f64,
    },
}

/// Scenario engine built from a baseline factor-model analysis.
pub struct WhatIfEngine<'a> {
    model: &'a FactorModel,
    base_decomposition: &'a RiskDecomposition,
    base_sensitivities: &'a SensitivityMatrix,
    portfolio: &'a Portfolio,
    market: &'a MarketContext,
    as_of: Date,
}

impl<'a> WhatIfEngine<'a> {
    /// Create a what-if engine from a previously computed baseline.
    #[must_use]
    pub fn new(
        model: &'a FactorModel,
        base_decomposition: &'a RiskDecomposition,
        base_sensitivities: &'a SensitivityMatrix,
        portfolio: &'a Portfolio,
        market: &'a MarketContext,
        as_of: Date,
    ) -> Self {
        Self {
            model,
            base_decomposition,
            base_sensitivities,
            portfolio,
            market,
            as_of,
        }
    }

    /// Reallocate existing sensitivity rows to simulate remove or resize scenarios.
    ///
    /// A removal zeroes that position's sensitivity row; a resize scales it in
    /// proportion to the original nonzero quantity. The method then recomputes
    /// risk decomposition and credit residual risk. Adding a new position is
    /// intentionally unsupported because it requires repricing fresh
    /// sensitivities rather than reallocating existing rows.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported add, unknown position, non-finite
    /// replacement quantity, or an attempt to proportionally resize a zero
    /// quantity. Propagates decomposition, portfolio-update, and credit-risk
    /// calculation errors.
    pub fn position_what_if(&self, changes: &[PositionChange]) -> Result<WhatIfResult> {
        let mut sensitivities = self.base_sensitivities.clone();
        let mut scenario_positions = self.portfolio.positions().to_vec();

        for change in changes {
            match change {
                PositionChange::Add { .. } => {
                    return Err(Error::invalid_input(
                        "PositionChange::Add is not supported yet; recompute sensitivities against a cloned Portfolio".to_string(),
                    ));
                }
                PositionChange::Remove { position_id } => {
                    let Some(position_idx) = self.position_index(position_id) else {
                        return Err(Error::invalid_input(format!(
                            "Unknown position '{}'",
                            position_id
                        )));
                    };
                    for factor_idx in 0..sensitivities.n_factors() {
                        sensitivities.set_delta(position_idx, factor_idx, 0.0);
                    }
                    scenario_positions.retain(|position| position.position_id != *position_id);
                }
                PositionChange::Resize {
                    position_id,
                    new_quantity,
                } => {
                    if !new_quantity.is_finite() {
                        return Err(Error::invalid_input(format!(
                            "PositionChange::Resize new_quantity must be finite for position '{}', got {}",
                            position_id, new_quantity
                        )));
                    }
                    let Some(position_idx) = self.position_index(position_id) else {
                        return Err(Error::invalid_input(format!(
                            "Unknown position '{}'",
                            position_id
                        )));
                    };
                    let Some(position) = self.portfolio.get_position(position_id.as_str()) else {
                        return Err(Error::invalid_input(format!(
                            "Unknown position '{}'",
                            position_id
                        )));
                    };
                    if position.quantity.abs() < f64::EPSILON {
                        return Err(Error::invalid_input(format!(
                            "Position '{}' has zero quantity and cannot be resized proportionally",
                            position_id
                        )));
                    }
                    let scale = *new_quantity / position.quantity;
                    let row = sensitivities.position_deltas(position_idx).to_vec();
                    for (factor_idx, delta) in row.into_iter().enumerate() {
                        sensitivities.set_delta(position_idx, factor_idx, delta * scale);
                    }
                    if let Some(scenario_position) = scenario_positions
                        .iter_mut()
                        .find(|current| current.position_id == *position_id)
                    {
                        scenario_position.quantity = *new_quantity;
                    }
                }
            }
        }

        let mut after = self.model.decomposer().decompose(
            &sensitivities,
            self.model.covariance(),
            self.model.risk_measure(),
        )?;
        let mut scenario_portfolio = self.portfolio.clone();
        scenario_portfolio.set_positions(scenario_positions)?;
        self.model.add_credit_residual_risk(
            &mut after,
            &scenario_portfolio,
            self.market,
            self.as_of,
        )?;

        Ok(WhatIfResult {
            before: self.base_decomposition.clone(),
            delta: factor_deltas(self.base_decomposition, &after),
            after,
        })
    }

    /// Shock factors, reprice positions, and recompute the stressed decomposition.
    ///
    /// Returns each position's stressed-minus-base PV and a portfolio total in
    /// the portfolio base currency. The helper does not apply an FX conversion
    /// policy: every position must already price in that base currency.
    ///
    /// # Errors
    ///
    /// Propagates invalid factor shocks, market-stress construction, pricing,
    /// and decomposition errors. Returns validation errors for non-finite PVs
    /// or a position whose pricing currency differs from the portfolio base.
    pub fn factor_stress(&self, stresses: &[(FactorId, f64)]) -> Result<StressResult> {
        factor_stress(
            self.model,
            self.portfolio,
            self.market,
            self.as_of,
            stresses,
        )
    }

    fn position_index(&self, position_id: &PositionId) -> Option<usize> {
        self.base_sensitivities
            .position_ids()
            .iter()
            .position(|current| current == position_id.as_str())
    }
}

pub(super) fn factor_stress(
    model: &FactorModel,
    portfolio: &Portfolio,
    market: &MarketContext,
    as_of: Date,
    stresses: &[(FactorId, f64)],
) -> Result<StressResult> {
    let (stressed_market, changed_factor_keys) =
        model.stressed_market_with_factor_keys(portfolio, market, as_of, stresses)?;

    let base_valuation = evaluate_raw_portfolio(RawEvaluationInput {
        portfolio,
        market,
        as_of,
        execution: PositionExecution::Auto,
        seed: None,
    })?;

    let affected_indices = changed_factor_keys.as_ref().map(|changed_factor_keys| {
        portfolio
            .dependency_index()
            .affected_positions(changed_factor_keys)
    });
    let stressed_valuation = if let Some(affected_indices) = &affected_indices {
        evaluate_raw_portfolio(RawEvaluationInput {
            portfolio,
            market: &stressed_market,
            as_of,
            execution: PositionExecution::Auto,
            seed: Some(RawSelectiveSeed {
                prior: &base_valuation,
                reprice_indices: affected_indices,
            }),
        })?
    } else {
        evaluate_raw_portfolio(RawEvaluationInput {
            portfolio,
            market: &stressed_market,
            as_of,
            execution: PositionExecution::Auto,
            seed: None,
        })?
    };

    let mut position_pnl = Vec::with_capacity(portfolio.positions.len());
    for (position_index, position) in portfolio.positions.iter().enumerate() {
        let base_endpoint = base_valuation.endpoint(position_index).ok_or_else(|| {
            Error::validation(format!(
                "Factor stress base endpoint is missing position '{}'",
                position.position_id
            ))
        })?;
        let stressed_endpoint = stressed_valuation.endpoint(position_index).ok_or_else(|| {
            Error::validation(format!(
                "Factor stress stressed endpoint is missing position '{}'",
                position.position_id
            ))
        })?;
        let base_value = base_endpoint.amount;
        let stressed_value = stressed_endpoint.amount;
        position_pnl.push((
            position.position_id.clone(),
            (stressed_value - base_value) * position.scale_factor(),
        ));
    }

    let mut total_pnl_acc = NeumaierAccumulator::new();
    for (_, pnl) in &position_pnl {
        total_pnl_acc.add(*pnl);
    }

    let stressed_decomposition = model.analyze(portfolio, &stressed_market, as_of)?;

    Ok(StressResult {
        total_pnl: total_pnl_acc.total(),
        position_pnl,
        stressed_decomposition,
    })
}

fn factor_deltas(
    before: &RiskDecomposition,
    after: &RiskDecomposition,
) -> Vec<FactorContributionDelta> {
    let after_by_id: std::collections::HashMap<&FactorId, &super::types::FactorContribution> =
        after
            .factor_contributions
            .iter()
            .map(|fc| (&fc.factor_id, fc))
            .collect();

    before
        .factor_contributions
        .iter()
        .map(|before_factor| {
            let (abs_after, rel_after) = after_by_id
                .get(&before_factor.factor_id)
                .map(|af| (af.absolute_risk, af.relative_risk))
                .unwrap_or((0.0, 0.0));
            FactorContributionDelta {
                factor_id: before_factor.factor_id.clone(),
                absolute_change: abs_after - before_factor.absolute_risk,
                relative_change: rel_after - before_factor.relative_risk,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor_model::{FactorModel, FactorModelBuilder};
    use crate::position::{Position, PositionUnit};
    use crate::sensitivity::{FactorSensitivityEngine, SensitivityMatrix};
    use crate::test_utils::build_test_market_at;
    use crate::types::{PositionId, DUMMY_ENTITY_ID};
    use crate::Portfolio;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{Attributes, CurveId};
    use finstack_quant_factor_model::matching::{DependencyFilter, MappingRule, MatchingConfig};
    use finstack_quant_factor_model::{
        CurveType, DependencyType, FactorCovarianceMatrix, FactorDefinition, FactorId,
        FactorModelConfig, FactorType, MarketMapping, PricingMode, RiskMeasure, UnmatchedPolicy,
    };
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::MarketDependencies;
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use time::macros::date;

    #[test]
    fn test_position_resize_scales_total_risk() {
        let setup = build_test_model();
        assert!(setup.is_some());
        let Some((model, portfolio, market)) = setup else {
            return;
        };
        let base_result = model.analyze(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(base_result.is_ok());
        let Ok(base) = base_result else {
            return;
        };
        let sensitivities_result =
            model.compute_sensitivities(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(sensitivities_result.is_ok());
        let Ok(sensitivities) = sensitivities_result else {
            return;
        };

        let result = model
            .what_if(
                &base,
                &sensitivities,
                &portfolio,
                &market,
                date!(2024 - 01 - 01),
            )
            .position_what_if(&[PositionChange::Resize {
                position_id: PositionId::new("pos-1"),
                new_quantity: 4.0,
            }]);
        assert!(result.is_ok());
        let Ok(result) = result else {
            return;
        };

        assert!(result.after.total_risk > result.before.total_risk);
    }

    #[test]
    fn test_position_remove_zeroes_risk() {
        let setup = build_test_model();
        assert!(setup.is_some());
        let Some((model, portfolio, market)) = setup else {
            return;
        };
        let base_result = model.analyze(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(base_result.is_ok());
        let Ok(base) = base_result else {
            return;
        };
        let sensitivities_result =
            model.compute_sensitivities(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(sensitivities_result.is_ok());
        let Ok(sensitivities) = sensitivities_result else {
            return;
        };

        let result = model
            .what_if(
                &base,
                &sensitivities,
                &portfolio,
                &market,
                date!(2024 - 01 - 01),
            )
            .position_what_if(&[PositionChange::Remove {
                position_id: PositionId::new("pos-1"),
            }]);
        assert!(result.is_ok());
        let Ok(result) = result else {
            return;
        };

        assert!((result.after.total_risk).abs() < 1e-12);
    }

    #[test]
    fn test_position_add_is_not_supported_yet() {
        let setup = build_test_model();
        assert!(setup.is_some());
        let Some((model, portfolio, market)) = setup else {
            return;
        };
        let base_result = model.analyze(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(base_result.is_ok());
        let Ok(base) = base_result else {
            return;
        };
        let sensitivities_result =
            model.compute_sensitivities(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(sensitivities_result.is_ok());
        let Ok(sensitivities) = sensitivities_result else {
            return;
        };

        let added_position_result = Position::new(
            "pos-2",
            DUMMY_ENTITY_ID,
            "inst-2",
            Arc::new(MockInstrument::new("inst-2", "USD-OIS", 100.0)),
            1.0,
            PositionUnit::Units,
        );
        assert!(added_position_result.is_ok());
        let Ok(added_position) = added_position_result else {
            return;
        };

        let result = model
            .what_if(
                &base,
                &sensitivities,
                &portfolio,
                &market,
                date!(2024 - 01 - 01),
            )
            .position_what_if(&[PositionChange::Add {
                position: Box::new(added_position),
            }]);
        assert!(result.is_err());
    }

    #[test]
    fn test_m1_position_resize_rejects_non_finite_quantity() {
        let Some((model, portfolio, market)) = build_test_model() else {
            panic!("setup");
        };
        let base = model
            .analyze(&portfolio, &market, date!(2024 - 01 - 01))
            .expect("analysis");
        let sensitivities = model
            .compute_sensitivities(&portfolio, &market, date!(2024 - 01 - 01))
            .expect("sensitivities");

        let result = model
            .what_if(
                &base,
                &sensitivities,
                &portfolio,
                &market,
                date!(2024 - 01 - 01),
            )
            .position_what_if(&[PositionChange::Resize {
                position_id: PositionId::new("pos-1"),
                new_quantity: f64::NAN,
            }]);

        let err = result.expect_err("M-1 non-finite resize quantity must fail at input boundary");
        assert!(
            err.to_string().contains("new_quantity"),
            "resize validation should name new_quantity, got {err}"
        );
    }

    #[test]
    fn test_factor_stress_returns_position_pnl() {
        let setup = build_test_model();
        assert!(setup.is_some());
        let Some((model, portfolio, market)) = setup else {
            return;
        };
        let base_result = model.analyze(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(base_result.is_ok());
        let Ok(base) = base_result else {
            return;
        };
        let sensitivities_result =
            model.compute_sensitivities(&portfolio, &market, date!(2024 - 01 - 01));
        assert!(sensitivities_result.is_ok());
        let Ok(sensitivities) = sensitivities_result else {
            return;
        };

        let stress_result = model
            .what_if(
                &base,
                &sensitivities,
                &portfolio,
                &market,
                date!(2024 - 01 - 01),
            )
            .factor_stress(&[(FactorId::new("Rates"), 1.0)]);
        assert!(stress_result.is_ok());
        let Ok(stress_result) = stress_result else {
            return;
        };

        assert_eq!(stress_result.position_pnl.len(), 1);
        assert!(stress_result.total_pnl.is_finite());
    }

    #[test]
    fn test_factor_stress_percentage_unit_scales_by_one_hundredth() {
        // Regression for C5: `factor_stress` previously multiplied by
        // `position.quantity` directly, which over-scaled Percentage
        // positions by 100x. Routing through `scale_factor` must make
        // quantity=50.0/Percentage produce the same P&L as
        // quantity=0.5/Units (both represent a 0.5 effective multiplier).
        let Some((model_u, portfolio_u, market)) =
            build_test_model_with_unit(0.5, PositionUnit::Units)
        else {
            panic!("units setup");
        };
        let Some((model_p, portfolio_p, _)) =
            build_test_model_with_unit(50.0, PositionUnit::Percentage)
        else {
            panic!("percentage setup");
        };

        let run = |model: &FactorModel, portfolio: &Portfolio| -> f64 {
            let base = model
                .analyze(portfolio, &market, date!(2024 - 01 - 01))
                .expect("analyze");
            let sens = model
                .compute_sensitivities(portfolio, &market, date!(2024 - 01 - 01))
                .expect("sensitivities");
            model
                .what_if(&base, &sens, portfolio, &market, date!(2024 - 01 - 01))
                .factor_stress(&[(FactorId::new("Rates"), 1.0)])
                .expect("stress")
                .total_pnl
        };

        let pnl_units = run(&model_u, &portfolio_u);
        let pnl_pct = run(&model_p, &portfolio_p);
        assert!(
            (pnl_units - pnl_pct).abs() < 1e-9,
            "units={pnl_units}, percentage={pnl_pct}"
        );
    }

    #[test]
    fn factor_stress_preserves_raw_pv_precision_in_shared_executor() {
        let Some((model, _, market)) = build_test_model() else {
            panic!("setup");
        };
        let as_of = date!(2024 - 01 - 01);
        let position = Position::new(
            "pos-raw",
            DUMMY_ENTITY_ID,
            "inst-raw",
            Arc::new(
                MockInstrument::new("inst-raw", "USD-OIS", 100.0)
                    .with_reported_value_override(42.0),
            ),
            2.0,
            PositionUnit::Units,
        )
        .expect("position");
        let portfolio = Portfolio::builder("raw-factor-stress")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .expect("portfolio");

        let result = model
            .factor_stress(&portfolio, &market, as_of, &[(FactorId::new("Rates"), 1.0)])
            .expect("factor stress");
        let (stressed_market, _) = model
            .stressed_market_with_factor_keys(
                &portfolio,
                &market,
                as_of,
                &[(FactorId::new("Rates"), 1.0)],
            )
            .expect("stressed market");
        let base_raw = portfolio.positions[0]
            .instrument
            .value_raw(&market, as_of)
            .expect("base raw");
        let stressed_raw = portfolio.positions[0]
            .instrument
            .value_raw(&stressed_market, as_of)
            .expect("stressed raw");
        let expected = (stressed_raw - base_raw) * portfolio.positions[0].scale_factor();

        assert!(expected.abs() > 1e-12);
        assert!((result.total_pnl - expected).abs() < 1e-12);
    }

    #[test]
    fn factor_stress_subtracts_raw_endpoints_before_position_scaling() {
        let Some((model, _, market)) = build_test_model() else {
            panic!("setup");
        };
        let as_of = date!(2024 - 01 - 01);
        let position = Position::new(
            "pos-cancellation",
            DUMMY_ENTITY_ID,
            "inst-cancellation",
            Arc::new(
                MockInstrument::new("inst-cancellation", "USD-OIS", 20_000.0)
                    .with_raw_offset(1.0e16),
            ),
            0.1,
            PositionUnit::Units,
        )
        .expect("position");
        let portfolio = Portfolio::builder("cancellation-factor-stress")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .expect("portfolio");

        let result = model
            .factor_stress(&portfolio, &market, as_of, &[(FactorId::new("Rates"), 1.0)])
            .expect("factor stress");
        let (stressed_market, _) = model
            .stressed_market_with_factor_keys(
                &portfolio,
                &market,
                as_of,
                &[(FactorId::new("Rates"), 1.0)],
            )
            .expect("stressed market");
        let base_raw = portfolio.positions[0]
            .instrument
            .value_raw(&market, as_of)
            .expect("base raw");
        let stressed_raw = portfolio.positions[0]
            .instrument
            .value_raw(&stressed_market, as_of)
            .expect("stressed raw");
        let expected = (stressed_raw - base_raw) * portfolio.positions[0].scale_factor();

        assert!(expected.abs() > 1e-12);
        assert_eq!(result.total_pnl, expected);
    }

    #[test]
    fn factor_stress_rejects_non_finite_raw_pv_without_panicking() {
        let Some((model, _, market)) = build_test_model() else {
            panic!("setup");
        };
        let as_of = date!(2024 - 01 - 01);
        let position = Position::new(
            "pos-non-finite",
            DUMMY_ENTITY_ID,
            "inst-non-finite",
            Arc::new(MockInstrument::new("inst-non-finite", "USD-OIS", f64::NAN)),
            1.0,
            PositionUnit::Units,
        )
        .expect("position");
        let portfolio = Portfolio::builder("non-finite-factor-stress")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .expect("portfolio");

        let error = model
            .factor_stress(&portfolio, &market, as_of, &[(FactorId::new("Rates"), 1.0)])
            .expect_err("non-finite raw PV must be a validation error");

        assert!(
            error.to_string().contains("M-1"),
            "unexpected error: {error}"
        );
        assert!(
            error.to_string().contains("pos-non-finite"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn m2_factor_stress_rejects_non_base_currency_positions() {
        let Some((model, _portfolio, market)) = build_test_model() else {
            panic!("setup");
        };
        let position = Position::new(
            "pos-eur",
            DUMMY_ENTITY_ID,
            "inst-eur",
            Arc::new(
                MockInstrument::new("inst-eur", "USD-OIS", 100.0).with_currency(Currency::EUR),
            ),
            1.0,
            PositionUnit::Units,
        )
        .expect("position");
        let portfolio = Portfolio::builder("portfolio-eur")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .position(position)
            .build()
            .expect("portfolio");
        let base = model
            .analyze(&portfolio, &market, date!(2024 - 01 - 01))
            .expect("base analysis");
        let sensitivities = model
            .compute_sensitivities(&portfolio, &market, date!(2024 - 01 - 01))
            .expect("sensitivities");

        let err = model
            .what_if(
                &base,
                &sensitivities,
                &portfolio,
                &market,
                date!(2024 - 01 - 01),
            )
            .factor_stress(&[(FactorId::new("Rates"), 1.0)])
            .expect_err("M-2: cross-currency factor stress must fail fast");
        assert!(err.to_string().contains("M-2"), "unexpected error: {err}");
    }

    #[test]
    fn factor_stress_applies_credit_hierarchy_fixed_bp_shocks_in_model_order() {
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
        use finstack_quant_factor_model::credit::hierarchy::{
            AdderVolSource, CreditHierarchySpec, HierarchyDimension, IssuerBetaMode, IssuerBetaRow,
            IssuerBetas, IssuerTags,
        };
        use finstack_quant_factor_model::matching::{CreditHierarchicalConfig, ISSUER_ID_META_KEY};
        use std::collections::BTreeMap;

        let as_of = date!(2024 - 01 - 01);
        let curve_id = CurveId::new("ISSUER-B-HAZ");
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.05_f64).exp()),
                (5.0, (-0.25_f64).exp()),
            ])
            .build()
            .expect("discount");
        let hazard = HazardCurve::builder(curve_id.clone())
            .base_date(as_of)
            .knots([(1.0, 0.01), (5.0, 0.01)])
            .build()
            .expect("hazard");
        let market = MarketContext::new().insert(discount).insert(hazard);
        let factors = vec![
            FactorDefinition {
                id: FactorId::new("credit::level0::Rating::B"),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
            FactorDefinition {
                id: FactorId::new("credit::generic"),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
        ];
        let covariance = FactorCovarianceMatrix::new(
            factors.iter().map(|factor| factor.id.clone()).collect(),
            vec![1.0, 0.0, 0.0, 1.0],
        )
        .expect("covariance");
        let mut tags = BTreeMap::new();
        tags.insert("rating".to_string(), "B".to_string());
        let model = FactorModelBuilder::new()
            .config(FactorModelConfig {
                factors,
                covariance,
                matching: MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
                    dependency_filter: Default::default(),
                    hierarchy: CreditHierarchySpec {
                        levels: vec![HierarchyDimension::Rating],
                    },
                    issuer_betas: vec![IssuerBetaRow {
                        issuer_id: finstack_quant_core::types::IssuerId::new("ISSUER-B"),
                        tags: IssuerTags(tags),
                        mode: IssuerBetaMode::IssuerBeta,
                        betas: IssuerBetas {
                            pc: 9.0,
                            levels: vec![11.0],
                        },
                        adder_at_anchor: 0.0,
                        adder_vol_annualized: 0.0,
                        adder_vol_source: AdderVolSource::Default,
                        fit_quality: None,
                    }],
                }),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .with_custom_sensitivity_engine(FixedSensitivityEngine)
            .build()
            .expect("model");
        let mut bond = finstack_quant_valuations::instruments::Bond::fixed(
            "BOND-ISSUER-B",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("bond");
        bond.credit_curve_id = Some(curve_id);
        bond.attributes = Attributes::new().with_meta(ISSUER_ID_META_KEY, "ISSUER-B");
        let position = Position::new(
            "pos-credit",
            DUMMY_ENTITY_ID,
            "bond-credit",
            Arc::new(bond),
            1.0,
            PositionUnit::Units,
        )
        .expect("position");
        let portfolio = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .expect("portfolio");
        let base = model.analyze(&portfolio, &market, as_of).expect("base");
        let sensitivities = model
            .compute_sensitivities(&portfolio, &market, as_of)
            .expect("sensitivities");

        let result = model
            .what_if(&base, &sensitivities, &portfolio, &market, as_of)
            .factor_stress(&[
                (FactorId::new("credit::level0::Rating::B"), 25.0),
                (FactorId::new("credit::generic"), 5.0),
            ])
            .expect("stress");
        let (manually_stressed, _) = model
            .stressed_market_with_factor_keys(
                &portfolio,
                &market,
                as_of,
                &[
                    (FactorId::new("credit::level0::Rating::B"), 25.0),
                    (FactorId::new("credit::generic"), 5.0),
                ],
            )
            .expect("manual stress");
        let base_value = portfolio.positions[0]
            .instrument
            .value_raw(&market, as_of)
            .expect("base value");
        let stressed_value = portfolio.positions[0]
            .instrument
            .value_raw(&manually_stressed, as_of)
            .expect("stressed value");

        assert!((result.total_pnl - (stressed_value - base_value)).abs() < 1e-8);
        assert!(result.total_pnl.abs() > 1e-8);
    }

    #[test]
    fn factor_stress_reuses_unaffected_prepared_endpoint() {
        let Some((model, _, base_market)) = build_test_model() else {
            panic!("model setup");
        };
        let as_of = date!(2024 - 01 - 01);
        let other_curve = DiscountCurve::builder("USD-OTHER")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("other discount curve");
        let market = base_market.insert(other_curve);
        let affected_calls = Arc::new(AtomicUsize::new(0));
        let unaffected_calls = Arc::new(AtomicUsize::new(0));
        let affected = Position::new(
            "pos-affected",
            DUMMY_ENTITY_ID,
            "inst-affected",
            Arc::new(
                MockInstrument::new("inst-affected", "USD-OIS", 100.0)
                    .with_call_counter(Arc::clone(&affected_calls)),
            ),
            1.0,
            PositionUnit::Units,
        )
        .expect("affected position");
        let unaffected = Position::new(
            "pos-unaffected",
            DUMMY_ENTITY_ID,
            "inst-unaffected",
            Arc::new(
                MockInstrument::new("inst-unaffected", "USD-OTHER", 100.0)
                    .with_call_counter(Arc::clone(&unaffected_calls)),
            ),
            1.0,
            PositionUnit::Units,
        )
        .expect("unaffected position");
        let portfolio = Portfolio::builder("selective-factor-stress")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(affected)
            .position(unaffected)
            .build()
            .expect("portfolio");
        let result = model
            .factor_stress(&portfolio, &market, as_of, &[(FactorId::new("Rates"), 1.0)])
            .expect("factor stress");

        assert_eq!(
            result
                .position_pnl
                .iter()
                .map(|(position_id, _)| position_id.as_str())
                .collect::<Vec<_>>(),
            vec!["pos-affected", "pos-unaffected"]
        );
        assert_eq!(affected_calls.load(Ordering::SeqCst), 2);
        assert_eq!(unaffected_calls.load(Ordering::SeqCst), 1);
        assert_eq!(result.position_pnl[1].1, 0.0);
    }

    #[test]
    fn factor_stress_reprices_trait_default_empty_dependencies() {
        let Some((model, _, market)) = build_test_model() else {
            panic!("model setup");
        };
        let as_of = date!(2024 - 01 - 01);
        let calls = Arc::new(AtomicUsize::new(0));
        let instrument = DefaultDependencyInstrument(
            MockInstrument::new("default-deps", "USD-OIS", 100.0)
                .with_call_counter(Arc::clone(&calls))
                .with_reported_value_override(42.0),
        );
        let position = Position::new(
            "pos-default-deps",
            DUMMY_ENTITY_ID,
            "default-deps",
            Arc::new(instrument),
            1.0,
            PositionUnit::Units,
        )
        .expect("position");
        let portfolio = Portfolio::builder("default-dependency-factor-stress")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .expect("portfolio");

        let result = model
            .factor_stress(&portfolio, &market, as_of, &[(FactorId::new("Rates"), 1.0)])
            .expect("factor stress");

        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "trait-default empty dependencies must not reuse the base endpoint"
        );
        assert!(result.total_pnl.abs() > 1e-12);
    }

    fn build_test_model() -> Option<(FactorModel, Portfolio, MarketContext)> {
        build_test_model_with_unit(2.0, PositionUnit::Units)
    }

    fn build_test_model_with_unit(
        quantity: f64,
        unit: PositionUnit,
    ) -> Option<(FactorModel, Portfolio, MarketContext)> {
        let covariance_result =
            FactorCovarianceMatrix::new(vec![FactorId::new("Rates")], vec![0.04]);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return None;
        };

        let model_result = FactorModelBuilder::new()
            .config(FactorModelConfig {
                factors: vec![FactorDefinition {
                    id: FactorId::new("Rates"),
                    factor_type: FactorType::Rates,
                    market_mapping: MarketMapping::CurveParallel {
                        curve_ids: vec![CurveId::new("USD-OIS")],
                        units: BumpUnits::RateBp,
                    },
                    description: None,
                }],
                covariance,
                matching: MatchingConfig::MappingTable(vec![MappingRule {
                    dependency_filter: DependencyFilter {
                        dependency_type: Some(DependencyType::Discount),
                        curve_type: Some(CurveType::Discount),
                        id: None,
                    },
                    attribute_filter: finstack_quant_factor_model::AttributeFilter::default(),
                    factor_id: FactorId::new("Rates"),
                }]),
                pricing_mode: PricingMode::DeltaBased,
                risk_measure: RiskMeasure::Variance,
                bump_size: None,
                unmatched_policy: Some(UnmatchedPolicy::Residual),
            })
            .with_custom_sensitivity_engine(FixedSensitivityEngine)
            .build();
        assert!(model_result.is_ok());
        let Ok(model) = model_result else {
            return None;
        };

        let position_result = Position::new(
            "pos-1",
            DUMMY_ENTITY_ID,
            "inst-1",
            Arc::new(MockInstrument::new("inst-1", "USD-OIS", 100.0)),
            quantity,
            unit,
        );
        assert!(position_result.is_ok());
        let Ok(position) = position_result else {
            return None;
        };

        let portfolio_result = Portfolio::builder("portfolio")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .position(position)
            .build();
        assert!(portfolio_result.is_ok());
        let Ok(portfolio) = portfolio_result else {
            return None;
        };

        Some((
            model,
            portfolio,
            build_test_market_at(date!(2024 - 01 - 01)),
        ))
    }

    #[derive(Clone)]
    struct MockInstrument {
        id: String,
        attributes: Attributes,
        discount_curve: CurveId,
        scale: f64,
        currency: Currency,
        call_counter: Option<Arc<AtomicUsize>>,
        reported_value_override: Option<f64>,
        raw_offset: f64,
    }

    impl MockInstrument {
        fn new(id: &str, discount_curve: &str, scale: f64) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::default(),
                discount_curve: CurveId::new(discount_curve),
                scale,
                currency: Currency::USD,
                call_counter: None,
                reported_value_override: None,
                raw_offset: 0.0,
            }
        }

        fn with_currency(mut self, currency: Currency) -> Self {
            self.currency = currency;
            self
        }

        fn with_call_counter(mut self, call_counter: Arc<AtomicUsize>) -> Self {
            self.call_counter = Some(call_counter);
            self
        }

        fn with_reported_value_override(mut self, amount: f64) -> Self {
            self.reported_value_override = Some(amount);
            self
        }

        fn with_raw_offset(mut self, amount: f64) -> Self {
            self.raw_offset = amount;
            self
        }
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        MockInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for MockInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            market: &MarketContext,
            _as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<Money> {
            if let Some(call_counter) = &self.call_counter {
                call_counter.fetch_add(1, Ordering::SeqCst);
            }
            if let Some(amount) = self.reported_value_override {
                return Ok(Money::new(amount, self.currency));
            }
            let pv = self.raw_offset
                + market.get_discount(self.discount_curve.as_str())?.zero(1.0) * self.scale;
            Ok(Money::new(pv, self.currency))
        }

        fn base_value_raw(
            &self,
            market: &MarketContext,
            _as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<f64> {
            Ok(self.raw_offset
                + market.get_discount(self.discount_curve.as_str())?.zero(1.0) * self.scale)
        }

        fn base_value_raw_with_currency(
            &self,
            market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<(f64, Currency)> {
            if let Some(call_counter) = &self.call_counter {
                call_counter.fetch_add(1, Ordering::SeqCst);
            }
            Ok((self.base_value_raw(market, as_of)?, self.currency))
        }

        fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
            let mut dependencies = MarketDependencies::new();
            dependencies
                .curves
                .discount_curves
                .push(self.discount_curve.clone());
            Ok(dependencies)
        }
    }

    #[derive(Clone)]
    struct DefaultDependencyInstrument(MockInstrument);

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        DefaultDependencyInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for DefaultDependencyInstrument {
        fn id(&self) -> &str {
            self.0.id()
        }

        fn key(&self) -> InstrumentType {
            self.0.key()
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            self.0.attributes()
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            self.0.attributes_mut()
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<Money> {
            self.0.base_value(market, as_of)
        }

        fn base_value_raw(
            &self,
            market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<f64> {
            self.0.base_value_raw(market, as_of)
        }
    }

    struct FixedSensitivityEngine;

    impl FactorSensitivityEngine for FixedSensitivityEngine {
        fn compute_sensitivities(
            &self,
            positions: &[(String, &dyn Instrument, f64)],
            factors: &[FactorDefinition],
            _market: &MarketContext,
            _as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<SensitivityMatrix> {
            let mut matrix = SensitivityMatrix::zeros(
                positions
                    .iter()
                    .map(|(position_id, _, _)| position_id.clone())
                    .collect(),
                factors.iter().map(|factor| factor.id.clone()).collect(),
            );
            if !factors.is_empty() {
                for position_index in 0..positions.len() {
                    matrix.set_delta(position_index, 0, 10.0);
                }
            }
            Ok(matrix)
        }
    }
}
