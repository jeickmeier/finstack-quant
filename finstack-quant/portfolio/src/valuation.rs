//! Portfolio valuation and aggregation.

use crate::error::Result;
use crate::portfolio::Portfolio;
use crate::types::{EntityId, PositionId};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Result of valuing a single position.
///
/// Holds both native-currency and base-currency valuations along with
/// the underlying [`ValuationResult`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionValue {
    /// Position identifier
    pub position_id: PositionId,

    /// Entity that owns this position
    pub entity_id: EntityId,

    /// Value in the instrument's native currency
    pub value_native: Money,

    /// Value converted to portfolio base currency
    pub value_base: Money,

    /// Linear scaling factor to apply to summable risk measures.
    ///
    /// This mirrors the economic position size and sign used for PV scaling,
    /// but is kept separate so non-summable metrics such as YTM remain
    /// unscaled at the position drill-down level.
    pub metric_scale: f64,

    /// Whether all requested risk metrics were computed successfully.
    pub risk_metrics_complete: bool,

    /// Original metrics failure message when the valuation fell back to PV-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_error: Option<String>,

    /// Full valuation result with metrics (including computed risk measures).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valuation_result: Option<ValuationResult>,
}

/// Complete portfolio valuation results.
///
/// Provides per-position valuations, totals by entity, and the grand total.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioValuation {
    /// Valuation date carried through from the portfolio.
    pub as_of: finstack_quant_core::dates::Date,

    /// Values for each position
    pub position_values: IndexMap<PositionId, PositionValue>,

    /// Total portfolio value in base currency
    pub total_base_ccy: Money,

    /// Aggregated values by entity
    pub by_entity: IndexMap<EntityId, Money>,

    /// Positions whose valuation fell back to PV-only because requested risk
    /// metrics could not be computed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded_positions: Vec<PositionId>,

    /// FX policy applied when collapsing position values to the base currency.
    ///
    /// Base-currency rollups use an explicit spot-equivalent conversion at
    /// [`as_of`](Self::as_of) through the market FX matrix; this records the
    /// applied [`FxConversionPolicy`] so the result envelope satisfies the
    /// policy-visibility invariant (the FX strategy is stamped, not implied).
    #[serde(default = "default_fx_collapse_policy")]
    pub fx_collapse_policy: FxConversionPolicy,

    /// Request-local compatibility stamp used only for safe selective reuse.
    ///
    /// The stamp is deliberately omitted from serialization: a deserialized
    /// valuation cannot prove that it still belongs to the same immutable
    /// portfolio state and therefore falls back to a full evaluation.
    #[serde(skip)]
    pub(crate) provenance: Option<crate::evaluation::EvaluationProvenance>,
}

/// Default FX policy stamped on a [`PortfolioValuation`]: the spot-equivalent
/// `as_of` conversion used by [`crate::fx::convert_to_base`].
fn default_fx_collapse_policy() -> FxConversionPolicy {
    FxConversionPolicy::CashflowDate
}

impl PortfolioValuation {
    /// Look up the value for a single position by string id.
    ///
    /// The underlying `position_values` field is `IndexMap<PositionId, _>`
    /// where [`PositionId`] is a newtype around `String`. Callers that have a
    /// `&str` (e.g. from a CLI argument, a binding, or a foreign string)
    /// would otherwise have to round-trip through `PositionId::new` / a
    /// `Borrow<str>` lookup; this accessor provides the borrow-aware path
    /// directly. Internal callers (selective repricing, liquidity scoring,
    /// FX checks) all rely on it.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Identifier to query.
    pub fn get_position_value(&self, position_id: &str) -> Option<&PositionValue> {
        self.position_values.get(position_id)
    }

    /// Look up the total value for a single entity by string id.
    ///
    /// Same `&str` ergonomics rationale as [`Self::get_position_value`] —
    /// avoids a `EntityId::new` allocation at every call site. Used by
    /// margin and metric-aggregation paths that already hold borrowed entity
    /// strings.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - Entity identifier to query (accepts `&str` or `&EntityId`).
    pub fn get_entity_value(&self, entity_id: &str) -> Option<&Money> {
        self.by_entity.get(entity_id)
    }

    /// Whether any position fell back to PV-only because its risk metrics
    /// failed.
    ///
    /// Equivalent to `!self.degraded_positions.is_empty()`. Provided as a
    /// named predicate so risk-report dashboards can read intent off the
    /// call site instead of reasoning about empty-vector semantics.
    pub fn has_degraded_risk(&self) -> bool {
        !self.degraded_positions.is_empty()
    }

    /// Borrow the list of position ids whose valuation fell back to PV-only.
    ///
    /// Returns a `&[PositionId]` rather than exposing the raw `Vec` so the
    /// public API does not commit to vector semantics (size, order, growth)
    /// that internal aggregation may want to change.
    pub fn degraded_positions(&self) -> &[PositionId] {
        &self.degraded_positions
    }
}

/// Which metric set to request for every position in the portfolio.
///
/// This replaces the legacy tri-state combination of `additional_metrics`
/// and `replace_standard_metrics` with a single explicit enum that has one
/// obvious interpretation for each variant.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "mode", content = "metrics", rename_all = "snake_case")]
pub enum RequestedMetrics {
    /// Standard portfolio metric set only.
    #[default]
    Standard,
    /// Standard set plus the listed extra metrics (de-duplicated).
    StandardPlus(Vec<MetricId>),
    /// Only the listed metrics; the standard set is not included.
    Only(Vec<MetricId>),
}

/// Options controlling portfolio valuation behaviour.
///
/// By default, risk metrics are treated as best-effort: if metrics fail for
/// a position, the engine falls back to PV-only valuation for that position.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PortfolioValuationOptions {
    /// When `true`, any failure to compute requested risk metrics for a
    /// position causes the entire portfolio valuation to fail.
    ///
    /// When `false` (default), the engine falls back to PV-only
    /// valuation for that position if metrics fail, preserving
    /// aggregate PV but potentially leaving some risk metrics missing.
    pub strict_risk: bool,

    /// Which metric set to request. See [`RequestedMetrics`].
    #[serde(default)]
    pub metrics: RequestedMetrics,
}

/// Value all positions in a portfolio with full metrics.
///
/// This function:
/// 1. Iterates through all positions (in parallel if enabled)
/// 2. Prices each instrument with metrics
/// 3. Converts values to base currency using FX rates
/// 4. Aggregates by entity
///
/// Portfolio valuation uses compensated summation during aggregation and treats
/// `PositionUnit` as part of the pricing contract, so the reported portfolio
/// totals reflect scaled holdings rather than raw instrument PVs.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to value.
/// * `market` - Market data context supplying curves and FX.
/// * `config` - Runtime configuration for the valuation engine.
/// * `options` - Portfolio valuation options controlling risk behaviour.
///
/// # Returns
///
/// [`Result`] containing [`PortfolioValuation`] on success.
///
/// # Errors
///
/// Returns [`crate::error::Error`] in the following cases:
///
/// - [`crate::error::Error::ValuationError`] - Instrument pricing failed for a position
/// - [`crate::error::Error::MissingMarketData`] - FX matrix unavailable for cross-currency conversion
/// - [`crate::error::Error::FxConversionFailed`] - Required FX rate not found in the matrix
/// - [`crate::error::Error::Core`] - Monetary arithmetic overflow during aggregation
///
/// # Parallelism
///
/// Position valuations are computed in parallel using rayon. Results are
/// deterministically reduced to ensure consistency across runs (per-position
/// results are collected in input order, then a serial Neumaier fold produces
/// the aggregate totals).
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_core::config::FinstackConfig;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_portfolio::valuation::{value_portfolio, PortfolioValuationOptions};
///
/// # fn main() -> finstack_quant_portfolio::Result<()> {
/// # let portfolio: finstack_quant_portfolio::Portfolio = unimplemented!("Provide a portfolio");
/// # let market: MarketContext = unimplemented!("Provide market data");
/// let config = FinstackConfig::default();
/// let valuation = value_portfolio(
///     &portfolio,
///     &market,
///     &config,
///     &PortfolioValuationOptions::default(),
/// )?;
///
/// println!("Total base PV: {}", valuation.total_base_ccy);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Numerically stable aggregation:
///   `docs/REFERENCES.md#kahan-1965`
pub fn value_portfolio(
    portfolio: &Portfolio,
    market: &MarketContext,
    config: &FinstackConfig,
    options: &PortfolioValuationOptions,
) -> Result<PortfolioValuation> {
    value_portfolio_at(portfolio, market, config, options, portfolio.as_of)
}

/// Value a portfolio at an explicit valuation date.
///
/// This is useful for replay and historical what-if workflows where the
/// portfolio definition has a static book date but each market snapshot must
/// be priced and FX-converted at the snapshot date.
///
/// The result is stamped with `as_of`; `options.strict_risk` determines whether
/// a failed risk-metric calculation aborts the valuation or is recorded while
/// retaining a PV-only position value.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to price, including positions, base currency, and
///   static book metadata.
/// * `market` - Market snapshot supplying curves, quotes, FX, and other data
///   required by each instrument valuation.
/// * `config` - Library configuration controlling market-data lookup and
///   financial-convention behavior.
/// * `options` - Valuation and risk-metric selection; `strict_risk` controls
///   whether an unavailable requested metric invalidates the full valuation.
/// * `as_of` - Explicit valuation date stamped on the result and used for
///   pricing, cashflow eligibility, and FX conversion.
///
/// # Errors
///
/// Returns a position valuation error when pricing fails (or metric pricing
/// fails in strict-risk mode), and propagates missing/invalid FX data needed to
/// convert a native PV to the portfolio base currency. It also returns errors
/// while assembling the portfolio-level totals.
pub fn value_portfolio_at(
    portfolio: &Portfolio,
    market: &MarketContext,
    config: &FinstackConfig,
    options: &PortfolioValuationOptions,
    as_of: Date,
) -> Result<PortfolioValuation> {
    value_portfolio_with_execution_at(
        portfolio,
        market,
        config,
        options,
        as_of,
        crate::evaluation::PositionExecution::Auto,
    )
}

fn value_portfolio_with_execution_at(
    portfolio: &Portfolio,
    market: &MarketContext,
    config: &FinstackConfig,
    options: &PortfolioValuationOptions,
    as_of: Date,
    execution: crate::evaluation::PositionExecution,
) -> Result<PortfolioValuation> {
    let mut plan = crate::evaluation::PortfolioEvaluationPlan::new(config);
    let market_state = plan.register_market(market, as_of);
    let portfolio_state = plan.register_portfolio(portfolio);
    let profile = crate::evaluation::EvaluationProfile::from_options(options);
    let evaluation =
        plan.register_evaluation_with_execution(market_state, portfolio_state, profile, execution)?;
    plan.execute().into_valuation(evaluation)
}

// =============================================================================
// Selective Repricing
// =============================================================================

/// Revalue only the positions affected by a set of changed market factor keys.
///
/// This function consults the portfolio's [`crate::dependencies::DependencyIndex`] to determine which
/// positions depend on the supplied keys, reprices only those positions against
/// the (updated) market context, and patches the prior valuation with fresh
/// results.  Unaffected positions retain their prior values.  Positions whose
/// dependencies could not be resolved are always repriced as a conservative
/// fallback.
///
/// The resulting [`PortfolioValuation`] is fully recomputed (totals, entity
/// rollups, degraded-risk tracking) and is identical to what
/// [`value_portfolio`] would produce if the same updated market
/// were used for a full revaluation.
///
/// # Arguments
///
/// * `portfolio` - Portfolio whose positions to selectively reprice.
/// * `market` - Market data context **after** the factor change has been applied.
/// * `config` - Runtime configuration forwarded to the pricing engine.
/// * `options` - Valuation options (strict_risk, metrics, etc.).
/// * `prior` - Previous full valuation whose unaffected positions are reused.
/// * `changed` - Market factor keys that moved; only positions depending on
///   at least one of these keys will be repriced.
///
/// # Returns
///
/// [`Result`] containing the patched [`PortfolioValuation`].
///
/// # Errors
///
/// Propagates any pricing or FX conversion errors encountered when revaluing
/// affected positions (same error semantics as [`value_portfolio`]).
///
/// # References
///
/// - Numerically stable aggregation:
///   `docs/REFERENCES.md#kahan-1965`
pub fn revalue_affected(
    portfolio: &Portfolio,
    market: &MarketContext,
    config: &FinstackConfig,
    options: &PortfolioValuationOptions,
    prior: &PortfolioValuation,
    changed: &[crate::dependencies::MarketFactorKey],
) -> Result<PortfolioValuation> {
    debug_assert_eq!(
        portfolio.dependency_index().indexed_position_count(),
        portfolio.positions.len(),
        "dependency index is stale: positions were mutated without updating \
         the index — call Portfolio::rebuild_index after direct mutation"
    );
    let affected_indices = portfolio.dependency_index().affected_positions(changed);
    let refresh_base_currency = changed
        .iter()
        .any(|key| matches!(key, crate::dependencies::MarketFactorKey::Fx { .. }));
    let profile = crate::evaluation::EvaluationProfile::from_options(options);
    let mut plan = crate::evaluation::PortfolioEvaluationPlan::new(config);
    let market_state = plan.register_market(market, portfolio.as_of);
    let portfolio_state = plan.register_portfolio(portfolio);
    let evaluation = plan.register_selective_evaluation(
        market_state,
        portfolio_state,
        profile,
        crate::evaluation::ParentResult::External(prior),
        crate::evaluation::PositionInvalidation::new(affected_indices, refresh_base_currency),
    )?;
    plan.execute().into_valuation(evaluation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PortfolioBuilder;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market;
    use crate::types::{Entity, DUMMY_ENTITY_ID};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn test_value_single_position() {
        let as_of = date!(2024 - 01 - 01);

        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_001",
            DUMMY_ENTITY_ID,
            "DEP_1M",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .expect("test should succeed");

        let market = build_test_market();
        let config = FinstackConfig::default();

        let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())
            .expect("test should succeed");

        assert_eq!(valuation.position_values.len(), 1);
        // Note: With flat curve, deposit PV is small but should be present
        assert!(valuation.total_base_ccy.amount().abs() >= 0.0);
        assert_eq!(valuation.by_entity.len(), 1);
    }

    #[test]
    fn test_value_multiple_entities() {
        let as_of = date!(2024 - 01 - 01);

        let dep1 = Deposit::builder()
            .id("DEP_1".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let dep2 = Deposit::builder()
            .id("DEP_2".into())
            .notional(Money::new(500_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 03 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let pos1 = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1",
            Arc::new(dep1),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let pos2 = Position::new(
            "POS_002",
            "ENTITY_B",
            "DEP_2",
            Arc::new(dep2),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .entity(Entity::new("ENTITY_B"))
            .position(pos1)
            .position(pos2)
            .build()
            .expect("test should succeed");

        let market = build_test_market();
        let config = FinstackConfig::default();

        let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())
            .expect("test should succeed");

        assert_eq!(valuation.position_values.len(), 2);
        assert_eq!(valuation.by_entity.len(), 2);
        assert!(valuation.get_entity_value("ENTITY_A").is_some());
        assert!(valuation.get_entity_value("ENTITY_B").is_some());
    }
}
