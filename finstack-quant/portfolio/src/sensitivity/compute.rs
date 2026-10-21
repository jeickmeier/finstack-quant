//! Portfolio-keyed sensitivity and P&L-profile entry points.
//!
//! These free functions run the shared engines ([`DeltaBasedEngine`],
//! [`FullRepricingEngine`]) over a [`Portfolio`]: one row per position, in
//! portfolio order, weighted by [`crate::position::Position::scale_factor`],
//! with every PV converted to `portfolio.base_currency` on the bumped market.
//! [`crate::factor_model::FactorModel::compute_sensitivities`] builds its rows
//! with the same helper, so both paths produce identical cells for any factor
//! that maps directly onto market data.

use super::{
    DeltaBasedEngine, FactorPnlProfile, FactorSensitivityEngine, FullRepricingEngine,
    DEFAULT_PNL_SCENARIO_POINTS,
};
use crate::Portfolio;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};
use finstack_quant_factor::{
    BumpSizeConfig, FactorDefinition, FactorType, MarketMapping, SensitivityMatrix,
};
use finstack_quant_valuations::instruments::Instrument;

/// Engine rows for `portfolio`: `(position_id, instrument, scale_factor)` in
/// portfolio position order.
///
/// # Arguments
///
/// * `portfolio` - Portfolio whose positions become the engine rows; the
///   returned instruments borrow from it.
pub(crate) fn portfolio_rows(portfolio: &Portfolio) -> Vec<(String, &dyn Instrument, f64)> {
    portfolio
        .positions
        .iter()
        .map(|position| {
            (
                position.position_id.to_string(),
                position.instrument.as_ref() as &dyn Instrument,
                position.scale_factor(),
            )
        })
        .collect()
}

/// Whether `factor` is a hierarchy credit factor: a [`FactorType::Credit`]
/// factor whose [`MarketMapping::CurveParallel`] names no curves.
///
/// Such a factor has no market bump of its own; its exposure comes from the
/// per-position issuer CS01 scaled by the calibrated beta of the factor
/// assignment, which only [`crate::factor_model::FactorModel`] holds.
///
/// # Arguments
///
/// * `factor` - Factor definition to classify.
pub(crate) fn uses_assignment_driven_credit_shock(factor: &FactorDefinition) -> bool {
    matches!(factor.factor_type, FactorType::Credit)
        && matches!(
            factor.market_mapping,
            MarketMapping::CurveParallel { ref curve_ids, .. } if curve_ids.is_empty()
        )
}

/// Reject hierarchy credit factors, whose exposure the free engines cannot
/// compute (every cell would silently be zero).
fn reject_assignment_driven_credit_factors(factors: &[FactorDefinition]) -> Result<()> {
    match factors
        .iter()
        .find(|factor| uses_assignment_driven_credit_shock(factor))
    {
        Some(factor) => Err(Error::Validation(format!(
            "credit factor '{}' maps to no curve (a credit hierarchy factor); its exposure \
             comes from the factor assignment, so compute it with \
             FactorModel::compute_sensitivities",
            factor.id
        ))),
        None => Ok(()),
    }
}

/// Compute the position-by-factor sensitivity matrix of `portfolio`.
///
/// Each cell is the central difference of the position's base-currency PV
/// under an up and a down bump of the factor (bump size and units from
/// `bump_config`), weighted by the position's
/// [`crate::position::Position::scale_factor`]. Native PVs are converted to
/// `portfolio.base_currency` with spot FX on each bumped market at `as_of`.
/// Rows follow portfolio position order and columns follow `factors`.
///
/// Credit hierarchy factors ([`FactorType::Credit`] with an empty
/// [`MarketMapping::CurveParallel`] curve list, as emitted by credit-factor
/// calibration) are rejected: their exposure is the issuer CS01 scaled by the
/// calibrated beta of each factor assignment, which needs a
/// [`crate::factor_model::FactorModel`] and its
/// [`compute_sensitivities`](crate::factor_model::FactorModel::compute_sensitivities).
///
/// # Arguments
///
/// * `portfolio` - Portfolio to analyze; its positions are the rows and its
///   `base_currency` is the reporting currency of every cell.
/// * `factors` - Factor definitions to bump, in column order. Each must map to
///   market data (curves, FX pairs, equity spots, vol surfaces, ...).
/// * `market` - Unbumped market snapshot, including the FX matrix required to
///   convert cross-currency positions.
/// * `as_of` - Valuation date for pricing and spot FX lookup.
/// * `bump_config` - Bump sizes per factor; `None` selects
///   [`BumpSizeConfig::default`] (1 bp for rates/credit, 1 % for relative
///   factors).
///
/// # Returns
///
/// Sensitivity matrix with one row per position and one column per factor, in
/// `portfolio.base_currency` per factor bump unit.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a credit hierarchy factor, a non-finite
/// position weight, PV, or weighted sensitivity. Propagates market-bump,
/// instrument-pricing, and FX-conversion failures.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_portfolio::sensitivity::compute_factor_sensitivities;
/// use finstack_quant_portfolio::Portfolio;
/// use time::macros::date;
///
/// let as_of = date!(2025 - 01 - 15);
/// let portfolio = Portfolio::builder("EMPTY")
///     .base_currency(Currency::USD)
///     .as_of(as_of)
///     .build()?;
/// let matrix =
///     compute_factor_sensitivities(&portfolio, &[], &MarketContext::new(), as_of, None)?;
/// assert_eq!(matrix.n_positions(), 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compute_factor_sensitivities(
    portfolio: &Portfolio,
    factors: &[FactorDefinition],
    market: &MarketContext,
    as_of: Date,
    bump_config: Option<BumpSizeConfig>,
) -> Result<SensitivityMatrix> {
    reject_assignment_driven_credit_factors(factors)?;
    DeltaBasedEngine::new(bump_config.unwrap_or_default()).compute_sensitivities(
        &portfolio_rows(portfolio),
        factors,
        market,
        as_of,
        portfolio.base_currency,
    )
}

/// Compute full-repricing scenario P&L profiles of `portfolio`, one per factor.
///
/// Each factor is shifted across a symmetric grid of `n_points` multiples of
/// its bump size (5 points gives `[-2, -1, 0, 1, 2]`). A profile row holds
/// `scale_factor * (PV_bumped - PV_base)` for every position, in
/// `portfolio.base_currency` converted with spot FX on each scenario market at
/// `as_of`. Position columns follow portfolio position order.
///
/// Credit hierarchy factors are rejected for the reason given on
/// [`compute_factor_sensitivities`]; no profile path carries their
/// assignment-driven exposure.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to reprice; its positions are the profile
///   columns and its `base_currency` is the reporting currency.
/// * `factors` - Factor definitions to shock, in returned profile order.
/// * `market` - Unbumped market snapshot reused by every scenario.
/// * `as_of` - Valuation date for pricing and spot FX lookup.
/// * `bump_config` - Bump sizes per factor; `None` selects
///   [`BumpSizeConfig::default`].
/// * `n_points` - Odd number of grid points, including the unshifted base,
///   in `3..=1001`; `None` selects [`DEFAULT_PNL_SCENARIO_POINTS`] (5).
///
/// # Returns
///
/// One [`FactorPnlProfile`] per factor, in `factors` order.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a credit hierarchy factor, an invalid
/// scenario grid, a non-finite weight, PV, or P&L. Propagates market-bump,
/// instrument-pricing, and FX-conversion failures.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_portfolio::sensitivity::compute_pnl_profiles;
/// use finstack_quant_portfolio::Portfolio;
/// use time::macros::date;
///
/// let as_of = date!(2025 - 01 - 15);
/// let portfolio = Portfolio::builder("EMPTY")
///     .base_currency(Currency::USD)
///     .as_of(as_of)
///     .build()?;
/// let profiles =
///     compute_pnl_profiles(&portfolio, &[], &MarketContext::new(), as_of, None, None)?;
/// assert!(profiles.is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compute_pnl_profiles(
    portfolio: &Portfolio,
    factors: &[FactorDefinition],
    market: &MarketContext,
    as_of: Date,
    bump_config: Option<BumpSizeConfig>,
    n_points: Option<usize>,
) -> Result<Vec<FactorPnlProfile>> {
    reject_assignment_driven_credit_factors(factors)?;
    FullRepricingEngine::new(
        bump_config.unwrap_or_default(),
        n_points.unwrap_or(DEFAULT_PNL_SCENARIO_POINTS),
    )?
    .compute_pnl_profiles(
        &portfolio_rows(portfolio),
        factors,
        market,
        as_of,
        portfolio.base_currency,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor_model::FactorModel;
    use crate::position::{Position, PositionUnit};
    use crate::types::DUMMY_ENTITY_ID;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::types::CurveId;
    use finstack_quant_factor::MatchingConfig;
    use finstack_quant_factor::{
        FactorCovarianceMatrix, FactorId, FactorModelConfig, RiskMeasure, UnmatchedPolicy,
    };
    use finstack_quant_valuations::instruments::equity::spot::Equity;
    use std::sync::Arc;

    fn as_of() -> Date {
        time::macros::date!(2025 - 01 - 01)
    }

    fn fx_market() -> MarketContext {
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
            .expect("valid quote");
        MarketContext::new().insert_fx(FxMatrix::new(provider))
    }

    fn eurusd_factor() -> FactorDefinition {
        FactorDefinition {
            id: FactorId::new("EURUSD"),
            factor_type: FactorType::Fx,
            market_mapping: MarketMapping::FxRate {
                pair: (Currency::EUR, Currency::USD),
            },
            description: None,
        }
    }

    fn equity_position(id: &str, currency: Currency, quantity: f64) -> Position {
        let mut equity = Equity::new(id, id, currency);
        equity.quoted_spot = Some(100.0);
        Position::new(
            id,
            DUMMY_ENTITY_ID,
            id,
            Arc::new(equity),
            quantity,
            PositionUnit::Units,
        )
        .expect("position builds")
    }

    fn portfolio(positions: Vec<Position>) -> Portfolio {
        positions
            .into_iter()
            .fold(
                Portfolio::builder("P")
                    .base_currency(Currency::USD)
                    .as_of(as_of()),
                |builder, position| builder.position(position),
            )
            .build()
            .expect("portfolio builds")
    }

    #[test]
    fn rows_follow_portfolio_order_and_reporting_currency_is_the_base_currency() {
        let market = fx_market();
        let factors = vec![eurusd_factor()];
        let forward = portfolio(vec![
            equity_position("USD", Currency::USD, 1.0),
            equity_position("EUR", Currency::EUR, 1.0),
        ]);
        let reversed = portfolio(vec![
            equity_position("EUR", Currency::EUR, 1.0),
            equity_position("USD", Currency::USD, 1.0),
        ]);
        let first = compute_factor_sensitivities(&forward, &factors, &market, as_of(), None)
            .expect("sensitivities");
        let second = compute_factor_sensitivities(&reversed, &factors, &market, as_of(), None)
            .expect("sensitivities");
        assert_eq!(first.position_ids(), ["USD", "EUR"]);
        assert_eq!(first.delta(0, 0).expect("in range"), 0.0);
        assert!(first.delta(1, 0).expect("in range").abs() > 1.0);
        assert_eq!(
            first.delta(1, 0).expect("in range"),
            second.delta(0, 0).expect("in range")
        );
        let profiles = compute_pnl_profiles(&forward, &factors, &market, as_of(), None, Some(3))
            .expect("profiles");
        assert_eq!(profiles[0].base_currency, Currency::USD);
        assert_eq!(profiles[0].position_ids, vec!["USD", "EUR"]);
        assert_eq!(profiles[0].shifts.len(), 3);
    }

    #[test]
    fn free_function_matches_factor_model_for_market_mapped_factors() {
        let market = fx_market();
        let factor = eurusd_factor();
        let book = portfolio(vec![
            equity_position("EUR-A", Currency::EUR, 3.0),
            equity_position("USD-B", Currency::USD, 2.0),
            equity_position("EUR-C", Currency::EUR, -5.0),
        ]);
        let model = FactorModel::from_config(FactorModelConfig {
            factors: vec![factor.clone()],
            covariance: FactorCovarianceMatrix::new(vec![factor.id.clone()], vec![0.01])
                .expect("covariance"),
            matching: MatchingConfig::MappingTable(Vec::new()),
            risk_measure: RiskMeasure::Variance,
            bump_config: None,
            unmatched_policy: Some(UnmatchedPolicy::Warn),
        })
        .expect("model");
        let from_model = model
            .compute_sensitivities(&book, &market, as_of())
            .expect("model sensitivities");
        let free = compute_factor_sensitivities(&book, &[factor], &market, as_of(), None)
            .expect("free sensitivities");
        assert_eq!(free, from_model);
        // Weights are the position scale factors: EUR-C is -5/3 of EUR-A.
        let a = free.delta(0, 0).expect("in range");
        let c = free.delta(2, 0).expect("in range");
        assert!(a.abs() > 1.0);
        assert!((c / a + 5.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn credit_hierarchy_factors_are_rejected_on_both_paths() {
        let factors = vec![FactorDefinition {
            id: FactorId::new("credit::generic"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: Vec::<CurveId>::new(),
                units: BumpUnits::RateBp,
            },
            description: None,
        }];
        let book = portfolio(Vec::new());
        let market = MarketContext::new();
        for err in [
            compute_factor_sensitivities(&book, &factors, &market, as_of(), None).err(),
            compute_pnl_profiles(&book, &factors, &market, as_of(), None, None).err(),
        ] {
            let err = err.expect("hierarchy credit factor must be rejected");
            assert!(matches!(err, Error::Validation(_)), "{err}");
            assert!(err.to_string().contains("credit::generic"), "{err}");
        }
    }
}
