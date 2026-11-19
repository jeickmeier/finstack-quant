use crate::evaluation::{EvaluationProfile, PortfolioEvaluationPlan, PositionInvalidation};
use crate::types::{EntityId, PositionId};
use crate::valuation::{
    PortfolioValuation, PortfolioValuationOptions, RequestedMetrics, value_portfolio_at,
};
use crate::{Error, Portfolio, Result};
use finstack_quant_calibration::recalibration::{
    RateQuoteCoordinate, bump_discount_rate_quote, bump_forward_rate_quote,
    bump_linked_ois_rate_quote,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    RateCalibrationPillar, RateCalibrationQuote, RateCalibrationRecipe,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_margin::regulatory::frtb::{
    FrtbSensitivities, params::girr::GIRR_DELTA_RISK_WEIGHTS,
};
use finstack_quant_valuations::instruments::MarketDependencies;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::collections::HashSet;

/// The held and rebuilt views of an ordinary GIRR yield curve.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GirrQuoteSource {
    /// An independent discount curve, holding its pricing dependencies fixed.
    Discount {
        /// Exact discount-curve identifier.
        curve_id: CurveId,
    },
    /// A term projection curve, holding its discounting curve fixed.
    Projection {
        /// Exact term projection-curve identifier.
        curve_id: CurveId,
    },
    /// Reciprocal discount and projection views of the same OIS curve.
    LinkedOis {
        /// Discount representation of the OIS curve.
        discount_curve_id: CurveId,
        /// Projection representation of the same OIS curve.
        forward_curve_id: CurveId,
    },
}

/// Source and grouping for one qualified ordinary GIRR delta vertex.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GirrQuoteTarget {
    /// Risk currency used to bucket the factor, independent of PV currency.
    pub risk_currency: Currency,
    /// Exact grouping ID for this yield curve, preserving the regulated subcurve.
    pub risk_curve_id: CurveId,
    /// Qualified regulatory vertex, matching an admitted ordinary GIRR label.
    pub vertex: String,
    /// Curve representation, roles, and dependency replay basis.
    pub source: GirrQuoteSource,
    /// Recorded quote ID and exact contractual pillar of the target source.
    pub quote: RateQuoteCoordinate,
}

/// One signed position-level repricing difference for the requested GIRR vertex.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct GirrQuoteReprice {
    /// Position whose risk is represented, after applying its signed quantity.
    pub position_id: PositionId,
    /// Owner of the position within the trading book.
    pub entity_id: EntityId,
    /// Signed scaling from the instrument price to the held position.
    pub position_scale: f64,
    /// Held reporting FX rate in units of reporting currency per native unit.
    /// Absent when the currencies match.
    pub fx_rate: Option<f64>,
    /// Whether the held FX rate was triangulated through the market pivot.
    /// Absent when the currencies match.
    pub fx_triangulated: Option<bool>,
    /// Baseline value on the held valuation basis in the instrument's currency.
    pub center_native: Money,
    /// Value on the same basis after an upward one-basis-point quote replay.
    pub shocked_native: Money,
    /// Signed upward reprice difference in instrument currency per basis point.
    pub pnl_native_per_bp: Money,
    /// Signed upward reprice difference in portfolio currency per one percentage point.
    pub pnl_base_per_pp: f64,
}

impl GirrQuoteSource {
    fn shock(&self, market: &MarketContext, quote: &RateQuoteCoordinate) -> Result<MarketContext> {
        Ok(match self {
            Self::Discount { curve_id } => {
                let curve = market.get_discount(curve_id.as_str())?;
                market
                    .clone()
                    .try_insert(bump_discount_rate_quote(&curve, market, quote, 1.0)?)?
            }
            Self::Projection { curve_id } => {
                let curve = market.get_forward(curve_id.as_str())?;
                market
                    .clone()
                    .try_insert(bump_forward_rate_quote(&curve, market, quote, 1.0)?)?
            }
            Self::LinkedOis {
                discount_curve_id,
                forward_curve_id,
            } => {
                bump_linked_ois_rate_quote(market, discount_curve_id, forward_curve_id, quote, 1.0)?
            }
        })
    }
}

fn repriced_girr_rows(
    portfolio: &Portfolio,
    market: &MarketContext,
    pricing: &FinstackConfig,
    target: &GirrQuoteTarget,
    center: &PortfolioValuation,
    affected: &[(usize, &PositionId)],
) -> Result<Vec<GirrQuoteReprice>> {
    let shocked_market = target.source.shock(market, &target.quote)?;
    let mut plan = PortfolioEvaluationPlan::new(pricing);
    let market_state = plan.register_market(&shocked_market, center.as_of);
    let portfolio_state = plan.register_portfolio(portfolio);
    let reprice = plan.register_selective_evaluation(
        market_state,
        portfolio_state,
        EvaluationProfile::strict_metrics(&[]),
        center,
        PositionInvalidation::new(affected.iter().map(|(index, _)| *index).collect(), false),
    )?;
    let shocked = plan.execute().take_valuation(reprice)?;

    affected
        .iter()
        .map(|&(_, id)| {
            let old = center
                .position_values
                .get(id)
                .ok_or_else(|| Error::invalid_input(format!("Missing center position '{id}'")))?;
            let new = shocked
                .position_values
                .get(id)
                .ok_or_else(|| Error::invalid_input(format!("Missing shocked position '{id}'")))?;
            if old.fx_rate != new.fx_rate || old.fx_triangulated != new.fx_triangulated {
                return Err(Error::invalid_input(format!(
                    "GIRR quote replay changed the held FX basis of position '{id}'"
                )));
            }
            let difference = new.value_native.checked_sub(old.value_native)?;
            let held_fx = match old.fx_rate {
                Some(rate) if rate.is_finite() && rate > 0.0 => rate,
                None if old.value_native.currency() == portfolio.base_currency => 1.0,
                _ => {
                    return Err(Error::invalid_input(format!(
                        "Missing valid held FX for position '{id}'"
                    )));
                }
            };
            let per_pp = difference.amount() * held_fx * 100.0;
            if !per_pp.is_finite() {
                return Err(Error::invalid_input(format!(
                    "Non-finite GIRR delta for position '{id}'"
                )));
            }
            Ok(GirrQuoteReprice {
                position_id: id.clone(),
                entity_id: old.entity_id.clone(),
                position_scale: old.metric_scale,
                fx_rate: old.fx_rate,
                fx_triangulated: old.fx_triangulated,
                center_native: old.value_native,
                shocked_native: new.value_native,
                pnl_native_per_bp: difference,
                pnl_base_per_pp: per_pp,
            })
        })
        .collect()
}

impl GirrQuoteSource {
    fn recipe(&self, market: &MarketContext) -> Result<RateCalibrationRecipe> {
        let recipe = match self {
            Self::Discount { curve_id }
            | Self::LinkedOis {
                discount_curve_id: curve_id,
                ..
            } => market
                .get_discount(curve_id.as_str())?
                .rate_calibration()
                .cloned(),
            Self::Projection { curve_id } => market
                .get_forward(curve_id.as_str())?
                .rate_calibration()
                .cloned(),
        };
        recipe.ok_or_else(|| {
            Error::invalid_input(format!(
                "No executed rate quote recipe on '{}'",
                self.primary_curve()
            ))
        })
    }

    fn primary_curve(&self) -> &CurveId {
        match self {
            Self::Discount { curve_id } | Self::Projection { curve_id } => curve_id,
            Self::LinkedOis {
                discount_curve_id, ..
            } => discount_curve_id,
        }
    }

    fn includes_discount(&self, id: &CurveId) -> bool {
        match self {
            Self::Discount { curve_id }
            | Self::LinkedOis {
                discount_curve_id: curve_id,
                ..
            } => curve_id == id,
            Self::Projection { .. } => false,
        }
    }

    fn includes_forward(&self, id: &CurveId) -> bool {
        match self {
            Self::Projection { curve_id }
            | Self::LinkedOis {
                forward_curve_id: curve_id,
                ..
            } => curve_id == id,
            Self::Discount { .. } => false,
        }
    }

    fn matches(&self, dependencies: &MarketDependencies) -> bool {
        dependencies
            .curves
            .discount_curves
            .iter()
            .any(|id| self.includes_discount(id))
            || dependencies
                .curves
                .forward_curves
                .iter()
                .any(|id| self.includes_forward(id))
    }
}

/// Signed FRTB GIRR contributions for one qualified quote and their position reprices.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct GirrQuoteRisk {
    /// Exact regulatory target and source binding for every included reprice.
    pub target: GirrQuoteTarget,
    /// Position basis and signed contributions before aggregation.
    pub positions: Vec<GirrQuoteReprice>,
}

/// Exact quote yield-delta input and auditable position evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct GirrQuoteDelta {
    /// Valuation date held fixed across all reprices.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Identifier of the trading book whose positions and sign were priced.
    pub portfolio_id: String,
    /// Content fingerprint of the center market, including its calibration evidence.
    pub market_hash: String,
    /// Content fingerprint of the chosen pricing configuration.
    pub pricing_hash: String,
    /// Aggregation inputs for the ordinary yield-curve component of GIRR.
    pub sensitivities: FrtbSensitivities,
    /// Position-level signed differences in input target order.
    pub quote_risk: Vec<GirrQuoteRisk>,
}

/// Derive the ordinary yield-curve component of FRTB GIRR delta from exact rate quotes.
///
/// # Arguments
/// * `portfolio` - Trading-book positions to value together at their stored as-of date and base currency.
/// * `market` - Held pricing and FX observations with accepted calibration executions for every requested rate quote.
/// * `pricing` - Valuation configuration shared by the unshocked and shocked trades.
/// * `targets` - Approved quote mappings covering every regulatory vertex for each dependent yield curve.
///
/// # Errors
/// Rejects duplicate coordinates, missing curves or regulated vertices, and incomplete repricing.
/// Other components such as inflation and cross-currency basis require separately qualified inputs.
pub fn girr_quote_delta(
    portfolio: &Portfolio,
    market: &MarketContext,
    pricing: &FinstackConfig,
    targets: &[GirrQuoteTarget],
) -> Result<GirrQuoteDelta> {
    let groups = validate_targets(targets)?;
    for source in &groups {
        let recipe = source.recipe(market)?;
        for target in targets.iter().filter(|target| &target.source == *source) {
            validate_quote_vertex(target, &recipe)?;
        }
    }
    let dependencies = portfolio
        .positions
        .iter()
        .map(|position| {
            Ok((
                &position.position_id,
                position.instrument.market_dependencies()?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    for (id, deps) in &dependencies {
        for discount in &deps.curves.discount_curves {
            if !groups
                .iter()
                .any(|source| source.includes_discount(discount))
            {
                return Err(Error::invalid_input(format!(
                    "Missing GIRR delta source for discount curve '{discount}' on position '{id}'"
                )));
            }
        }
        for forward in &deps.curves.forward_curves {
            if !groups.iter().any(|source| source.includes_forward(forward)) {
                return Err(Error::invalid_input(format!(
                    "Missing GIRR delta source for projection curve '{forward}' on position '{id}'"
                )));
            }
        }
    }

    let date = portfolio.as_of;
    let options = PortfolioValuationOptions {
        strict_risk: true,
        metrics: RequestedMetrics::Only(Vec::new()),
    };
    let center = value_portfolio_at(portfolio, market, pricing, &options, date)?;
    let quote_risk = targets
        .iter()
        .map(|target| {
            let affected = dependencies
                .iter()
                .enumerate()
                .filter_map(|(index, (id, deps))| {
                    target.source.matches(deps).then_some((index, *id))
                })
                .collect::<Vec<_>>();
            if affected.is_empty() {
                return Err(Error::invalid_input(format!(
                    "No positions depend on GIRR delta curve '{}'",
                    target.risk_curve_id
                )));
            }
            Ok(GirrQuoteRisk {
                target: target.clone(),
                positions: repriced_girr_rows(
                    portfolio, market, pricing, target, &center, &affected,
                )?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let sensitivities = girr_frtb_sensitivities(portfolio.base_currency, &quote_risk)?;
    Ok(GirrQuoteDelta {
        as_of: date,
        portfolio_id: portfolio.id.clone(),
        market_hash: finstack_quant_core::canonical::content_hash(market)?,
        pricing_hash: finstack_quant_core::canonical::content_hash(pricing)?,
        sensitivities,
        quote_risk,
    })
}

fn validate_targets(targets: &[GirrQuoteTarget]) -> Result<Vec<&GirrQuoteSource>> {
    let mut groups = Vec::new();
    let mut source_vertices = HashSet::new();
    let mut bindings = HashSet::new();
    let allowed: BTreeSet<_> = GIRR_DELTA_RISK_WEIGHTS
        .iter()
        .map(|(vertex, _)| *vertex)
        .collect();
    for target in targets {
        if &target.risk_curve_id != target.source.primary_curve()
            || !allowed.contains(target.vertex.as_str())
            || !source_vertices.insert((&target.source, target.vertex.as_str()))
            || !bindings.insert((&target.source, target.quote.quote_id.as_str()))
        {
            return Err(Error::invalid_input(format!(
                "Invalid or duplicate GIRR quote coordinate on curve '{}' at {}",
                target.risk_curve_id, target.vertex
            )));
        }
        if !groups.contains(&&target.source) {
            groups.push(&target.source);
        }
    }
    for source in &groups {
        for other in &groups {
            if source == other {
                continue;
            }
            if source.primary_curve() == other.primary_curve()
                || matches!(source, GirrQuoteSource::LinkedOis { forward_curve_id, .. } if other.includes_forward(forward_curve_id))
            {
                return Err(Error::invalid_input(format!(
                    "Overlapping GIRR quote replay sources on '{}'",
                    source.primary_curve()
                )));
            }
        }
        let mut currency = None;
        for target in targets.iter().filter(|target| &&target.source == source) {
            if currency
                .replace(target.risk_currency)
                .is_some_and(|observed| observed != target.risk_currency)
            {
                return Err(Error::invalid_input(format!(
                    "Conflicting GIRR risk currencies on curve '{}'",
                    source.primary_curve()
                )));
            }
        }
        for vertex in &allowed {
            if !source_vertices.contains(&(*source, *vertex)) {
                return Err(Error::invalid_input(format!(
                    "Missing GIRR delta vertex {vertex} on curve '{}'",
                    source.primary_curve()
                )));
            }
        }
    }
    Ok(groups)
}

fn girr_frtb_sensitivities(
    base_currency: Currency,
    rows: &[GirrQuoteRisk],
) -> Result<FrtbSensitivities> {
    let mut sensitivities = FrtbSensitivities::new(base_currency);
    let mut seen = HashSet::new();
    for row in rows {
        let target = &row.target;
        if !seen.insert((target.risk_currency, &target.risk_curve_id, &target.vertex)) {
            return Err(Error::invalid_input("Duplicate GIRR output coordinate"));
        }
        for position in &row.positions {
            sensitivities.add_girr_delta(
                target.risk_currency,
                target.risk_curve_id.clone(),
                &target.vertex,
                position.pnl_base_per_pp,
            )?;
        }
    }
    Ok(sensitivities)
}

fn validate_quote_vertex(target: &GirrQuoteTarget, recipe: &RateCalibrationRecipe) -> Result<()> {
    let invalid = || {
        Error::invalid_input(format!(
            "Quote '{}' does not establish the {} regulated vertex in {} on curve '{}'",
            target.quote.quote_id, target.vertex, target.risk_currency, target.risk_curve_id
        ))
    };
    if recipe.currency != target.risk_currency {
        return Err(invalid());
    }
    let execution = recipe
        .execution
        .as_ref()
        .filter(|execution| execution.accepted)
        .ok_or_else(invalid)?;
    let position = execution
        .bindings
        .iter()
        .position(|binding| {
            binding.quote_id == target.quote.quote_id
                && binding.pillar_date == target.quote.pillar_date
        })
        .ok_or_else(invalid)?;
    let Some(
        RateCalibrationQuote::Deposit { pillar, .. } | RateCalibrationQuote::Swap { pillar, .. },
    ) = recipe.quotes.get(position)
    else {
        return Err(invalid());
    };
    let RateCalibrationPillar::Tenor(tenor) = pillar else {
        return Err(invalid());
    };
    let months = match tenor.unit() {
        TenorUnit::Months => tenor.count(),
        TenorUnit::Years => tenor.count() * 12,
        _ => return Err(invalid()),
    };
    let expected = match target.vertex.as_str() {
        "0.25Y" => 3,
        "0.5Y" => 6,
        years => years
            .strip_suffix('Y')
            .and_then(|text| text.parse::<u32>().ok())
            .and_then(|value| value.checked_mul(12))
            .ok_or_else(invalid)?,
    };
    if months != expected {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::Entity;
    use finstack_quant_calibration::api::{engine::calibrate, schema::CalibrationEnvelope};
    use finstack_quant_core::math::Compounding;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use serde_json::json;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn one_year_quote_reprices_a_signed_position_and_preserves_the_market() {
        let as_of = date!(2025 - 01 - 02);
        let pillars = [
            ("0.25Y", 3, "months"),
            ("0.5Y", 6, "months"),
            ("1Y", 1, "years"),
            ("2Y", 2, "years"),
            ("3Y", 3, "years"),
            ("5Y", 5, "years"),
            ("10Y", 10, "years"),
            ("15Y", 15, "years"),
            ("20Y", 20, "years"),
            ("30Y", 30, "years"),
        ];
        let market_data = pillars.map(|(label, count, unit)| {
            json!({
                "kind": "rate_quote", "type": "deposit", "id": label, "index": "GBP-SONIA-OIS",
                "pillar": {"tenor": {"count": count, "unit": unit}}, "rate": 0.057
            })
        });
        let calibration: CalibrationEnvelope = serde_json::from_value(json!({
            "schema": "finstack_quant.calibration/1",
            "plan": {
                "id": "gbp", "quote_sets": {"gbp": pillars.map(|(label, _, _)| label)},
                "steps": [{
                    "id": "GBP-SONIA", "quote_set": "gbp", "kind": "discount",
                    "curve_id": "GBP-SONIA", "currency": "GBP", "base_date": "2025-01-02",
                    "method": "bootstrap", "interpolation": "log_linear", "extrapolation": "extend",
                    "conventions": {"curve_day_count": "act_365f"}
                }],
                "settings": {}
            },
            "market_data": market_data
        }))
        .expect("valid schema");
        let market = MarketContext::try_from(
            calibrate(&calibration)
                .expect("qualified deposit fit")
                .result
                .final_market,
        )
        .expect("market context");
        let discount = market.get_discount("GBP-SONIA").expect("discount");
        let execution = discount
            .rate_calibration()
            .expect("recipe")
            .execution
            .as_ref()
            .expect("execution");
        let mut targets = execution
            .bindings
            .iter()
            .map(|binding| GirrQuoteTarget {
                risk_currency: Currency::GBP,
                risk_curve_id: "GBP-SONIA".into(),
                vertex: binding.quote_id.clone(),
                source: GirrQuoteSource::Discount {
                    curve_id: "GBP-SONIA".into(),
                },
                quote: RateQuoteCoordinate {
                    quote_id: binding.quote_id.clone(),
                    pillar_date: binding.pillar_date,
                },
            })
            .collect::<Vec<_>>();

        let principal = Money::new(100_000.0, Currency::GBP).expect("principal");
        let bond = Bond::zero_coupon(
            "ZC",
            principal,
            as_of,
            date!(2026 - 01 - 02),
            Compounding::Periodic(std::num::NonZeroU32::MIN),
            "GBP-SONIA",
        )
        .expect("bond");
        let position = Position::new(
            "SHORT",
            "RATES",
            "ZC",
            Arc::new(bond),
            -2.0,
            PositionUnit::Units,
        )
        .expect("short position");
        let portfolio = Portfolio::builder("rates")
            .base_currency(Currency::GBP)
            .as_of(as_of)
            .entity(Entity::new("RATES"))
            .position(position)
            .build()
            .expect("book");
        let pricing = FinstackConfig::default();
        let before = finstack_quant_core::canonical::content_hash(&market).expect("market hash");

        let result = girr_quote_delta(&portfolio, &market, &pricing, &targets)
            .expect("exact quote repricing");
        assert_eq!(result.market_hash, before);
        assert_eq!(result.sensitivities.girr_delta.len(), 10);
        let one_year = result
            .quote_risk
            .iter()
            .find(|row| row.target.vertex == "1Y")
            .expect("1Y risk");
        let expected = -200_000.0 * (1.0 / 1.0571 - 1.0 / 1.057);
        assert!((one_year.positions[0].pnl_native_per_bp.amount() - expected).abs() < 0.02);
        assert!((one_year.positions[0].pnl_base_per_pp - expected * 100.0).abs() < 2.0);
        assert_eq!(
            finstack_quant_core::canonical::content_hash(&market).expect("unchanged hash"),
            before
        );

        let original_quote = targets[0].quote.clone();
        targets[0].quote = targets[1].quote.clone();
        targets[1].quote = original_quote;
        assert!(
            girr_quote_delta(&portfolio, &market, &pricing, &targets)
                .expect_err("swapped binding")
                .to_string()
                .contains("regulated vertex")
        );
        targets.pop();
        assert!(
            girr_quote_delta(&portfolio, &market, &pricing, &targets)
                .expect_err("missing vertex")
                .to_string()
                .contains("Missing GIRR delta vertex")
        );
    }
}
