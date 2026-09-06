//! Primitive exposure reporting across direct and composite positions.

use crate::error::{Error, Result};
#[cfg(not(target_arch = "wasm32"))]
use crate::evaluation::POSITION_PARALLEL_MIN_POSITIONS;
use crate::fx::convert_to_base;
use crate::portfolio::Portfolio;
use crate::position::Position;
use crate::types::PositionId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::composite::CompositeInstrument;
use finstack_quant_valuations::instruments::{InstrumentEnvelope, PricingOptions};
use finstack_quant_valuations::metrics::{is_additive_metric, MetricId};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One primitive exposure path traced back to its owning portfolio position.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioPrimitivePath {
    /// Portfolio position containing the direct instrument or root composite.
    pub position_id: PositionId,
    /// Composite and leg identifiers ending at the primitive instrument.
    pub path: Vec<String>,
    /// Primitive instrument identifier.
    pub instrument_id: InstrumentId,
    /// Canonical primitive instrument type discriminator.
    pub instrument_type: String,
    /// Signed primitive quantity after position and nested-leg scaling.
    pub quantity: f64,
    /// Signed primitive value in portfolio base currency.
    pub value: Money,
    /// Additive primitive risk measures in portfolio base currency.
    pub measures: IndexMap<MetricId, f64>,
}

/// Net and gross portfolio exposure for one primitive instrument identifier.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioPrimitiveAggregate {
    /// Primitive instrument identifier.
    pub instrument_id: InstrumentId,
    /// Canonical primitive instrument type discriminator.
    pub instrument_type: String,
    /// Algebraic primitive quantity across all positions and paths.
    pub net_quantity: f64,
    /// Sum of absolute primitive path quantities.
    pub gross_quantity: f64,
    /// Algebraic primitive value in portfolio base currency.
    pub net_value: Money,
    /// Sum of absolute primitive path values in portfolio base currency.
    pub gross_value: Money,
    /// Algebraic additive risk by metric.
    pub net_measures: IndexMap<MetricId, f64>,
    /// Sum of absolute additive risk by metric.
    pub gross_measures: IndexMap<MetricId, f64>,
}

/// Portfolio primitive decomposition retaining both path and concentration views.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioPrimitiveExposureReport {
    /// Portfolio reporting currency used for every value and risk amount.
    pub base_currency: Currency,
    /// Position-aware primitive paths before overlap netting.
    pub paths: Vec<PortfolioPrimitivePath>,
    /// Net and gross aggregates ordered by primitive identifier.
    pub aggregates: Vec<PortfolioPrimitiveAggregate>,
}

/// Decompose every portfolio position into primitive economic exposures.
///
/// Direct instruments produce one primitive path. Composite positions recurse
/// through frozen resolved quantities, and the portfolio position scale
/// multiplies every resulting quantity, value, and additive risk amount.
/// Instruments sharing an identifier must also share an identical canonical
/// definition so concentration cannot silently combine unlike contracts.
///
/// # Arguments
///
/// * `portfolio` - Portfolio whose direct and composite positions are decomposed.
/// * `market` - Complete market context used for primitive valuation and FX.
/// * `metrics` - Additive risk metrics to report; non-additive metrics are rejected.
///
/// # Errors
///
/// Returns an error for conflicting primitive definitions, non-additive metrics,
/// missing market or FX inputs, invalid composites, or valuation failures.
pub fn primitive_exposure_report(
    portfolio: &Portfolio,
    market: &MarketContext,
    metrics: &[MetricId],
) -> Result<PortfolioPrimitiveExposureReport> {
    if let Some(metric) = metrics.iter().find(|metric| !is_additive_metric(metric)) {
        return Err(Error::invalid_input(format!(
            "metric '{metric}' is non-additive and cannot be aggregated as a primitive portfolio exposure"
        )));
    }

    let report_position = |position: &Position| -> Result<PositionPrimitiveReport> {
        position_primitive_report(portfolio, position, market, metrics)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let per_position: Vec<Result<PositionPrimitiveReport>> =
        if portfolio.positions().len() >= POSITION_PARALLEL_MIN_POSITIONS {
            use rayon::prelude::*;
            portfolio
                .positions()
                .par_iter()
                .map(report_position)
                .collect()
        } else {
            portfolio.positions().iter().map(report_position).collect()
        };

    #[cfg(target_arch = "wasm32")]
    let per_position: Vec<Result<PositionPrimitiveReport>> =
        portfolio.positions().iter().map(report_position).collect();

    let mut paths = Vec::new();
    let mut definitions = BTreeMap::<String, String>::new();
    for report in per_position {
        let report = report?;
        for (instrument_id, definition) in report.definitions {
            register_definition(&mut definitions, &instrument_id, &definition)?;
        }
        paths.extend(report.paths);
    }

    Ok(PortfolioPrimitiveExposureReport {
        base_currency: portfolio.base_currency,
        aggregates: aggregate_paths(portfolio.base_currency, &paths)?,
        paths,
    })
}

struct PositionPrimitiveReport {
    paths: Vec<PortfolioPrimitivePath>,
    definitions: Vec<(
        InstrumentId,
        finstack_quant_valuations::instruments::InstrumentJson,
    )>,
}

fn position_primitive_report(
    portfolio: &Portfolio,
    position: &Position,
    market: &MarketContext,
    metrics: &[MetricId],
) -> Result<PositionPrimitiveReport> {
    let position_scale = position.scale_factor();
    if let Some(composite) = position
        .instrument
        .as_any()
        .downcast_ref::<CompositeInstrument>()
    {
        let report = composite
            .primitive_exposure_report(market, portfolio.as_of, metrics)
            .map_err(|err| Error::valuation(position.position_id.clone(), err.to_string()))?;
        let mut paths = Vec::with_capacity(report.paths.len());
        let mut definitions = Vec::with_capacity(report.paths.len());
        for exposure in report.paths {
            if let Some(definition) = exposure.instrument_definition() {
                definitions.push((exposure.instrument_id.clone(), definition.clone()));
            }
            let value = convert_to_base(
                exposure.value,
                portfolio.as_of,
                market,
                portfolio.base_currency,
            )?;
            let fx_rate = reporting_fx_rate(
                exposure.value.currency(),
                portfolio.base_currency,
                portfolio.as_of,
                market,
            )?;
            paths.push(PortfolioPrimitivePath {
                position_id: position.position_id.clone(),
                path: exposure.path,
                instrument_id: exposure.instrument_id,
                instrument_type: exposure.instrument_type,
                quantity: exposure.quantity * position_scale,
                value: Money::new(value.amount() * position_scale, portfolio.base_currency)?,
                measures: exposure
                    .measures
                    .into_iter()
                    .map(|(metric, amount)| (metric, amount * fx_rate * position_scale))
                    .collect(),
            });
        }
        return Ok(PositionPrimitiveReport { paths, definitions });
    }

    let definition = position.instrument.to_instrument_json().ok_or_else(|| {
        Error::valuation(
            position.position_id.clone(),
            "instrument is not registered for canonical serialization",
        )
    })?;
    let instrument_id = InstrumentId::new(position.instrument.id());
    let result = position
        .instrument
        .price_with_metrics(market, portfolio.as_of, metrics, PricingOptions::default())
        .map_err(|err| Error::valuation(position.position_id.clone(), err.to_string()))?;
    let value = convert_to_base(
        result.value,
        portfolio.as_of,
        market,
        portfolio.base_currency,
    )?;
    let fx_rate = reporting_fx_rate(
        result.value.currency(),
        portfolio.base_currency,
        portfolio.as_of,
        market,
    )?;
    Ok(PositionPrimitiveReport {
        definitions: vec![(instrument_id.clone(), definition.clone())],
        paths: vec![PortfolioPrimitivePath {
            position_id: position.position_id.clone(),
            path: vec![position.instrument.id().to_string()],
            instrument_id,
            instrument_type: definition.type_tag().to_string(),
            quantity: position_scale,
            value: Money::new(value.amount() * position_scale, portfolio.base_currency)?,
            measures: result
                .measures
                .into_iter()
                .filter(|(metric, _)| is_additive_metric(metric))
                .map(|(metric, amount)| (metric, amount * fx_rate * position_scale))
                .collect(),
        }],
    })
}

fn register_definition(
    definitions: &mut BTreeMap<String, String>,
    instrument_id: &InstrumentId,
    definition: &finstack_quant_valuations::instruments::InstrumentJson,
) -> Result<()> {
    let hash = InstrumentEnvelope::new(definition.clone()).content_hash()?;
    match definitions.get(instrument_id.as_str()) {
        Some(existing) if existing != &hash => Err(Error::validation(format!(
            "primitive instrument id '{instrument_id}' has conflicting definitions across portfolio positions"
        ))),
        Some(_) => Ok(()),
        None => {
            definitions.insert(instrument_id.to_string(), hash);
            Ok(())
        }
    }
}

fn reporting_fx_rate(
    from: Currency,
    to: Currency,
    as_of: finstack_quant_core::dates::Date,
    market: &MarketContext,
) -> Result<f64> {
    Ok(convert_to_base(Money::from((1_i64, from)), as_of, market, to)?.amount())
}

fn aggregate_paths(
    base_currency: Currency,
    paths: &[PortfolioPrimitivePath],
) -> finstack_quant_core::Result<Vec<PortfolioPrimitiveAggregate>> {
    let mut aggregates = BTreeMap::<String, PortfolioPrimitiveAggregate>::new();
    for path in paths {
        let aggregate = aggregates
            .entry(path.instrument_id.to_string())
            .or_insert_with(|| PortfolioPrimitiveAggregate {
                instrument_id: path.instrument_id.clone(),
                instrument_type: path.instrument_type.clone(),
                net_quantity: 0.0,
                gross_quantity: 0.0,
                net_value: Money::from((0_i64, base_currency)),
                gross_value: Money::from((0_i64, base_currency)),
                net_measures: IndexMap::new(),
                gross_measures: IndexMap::new(),
            });
        aggregate.net_quantity += path.quantity;
        aggregate.gross_quantity += path.quantity.abs();
        aggregate.net_value = Money::new(
            aggregate.net_value.amount() + path.value.amount(),
            base_currency,
        )?;
        aggregate.gross_value = Money::new(
            aggregate.gross_value.amount() + path.value.amount().abs(),
            base_currency,
        )?;
        for (metric, amount) in &path.measures {
            *aggregate.net_measures.entry(metric.clone()).or_default() += *amount;
            *aggregate.gross_measures.entry(metric.clone()).or_default() += amount.abs();
        }
    }
    Ok(aggregates.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::Entity;
    use finstack_quant_valuations::instruments::composite::CompositeInstrument;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn overlapping_composite_and_direct_positions_report_net_and_gross() -> Result<()> {
        let composite = CompositeInstrument::example()?;
        let direct = composite.spec.legs[0]
            .instrument
            .as_ref()
            .clone()
            .into_boxed()?;
        let direct_id = direct.id().to_string();
        let composite_position = Position::new(
            "P-COMPOSITE",
            "ENTITY",
            composite.spec.id.to_string(),
            Arc::new(composite),
            2.0,
            PositionUnit::Units,
        )?;
        let direct_position = Position::new(
            "P-DIRECT",
            "ENTITY",
            direct_id,
            Arc::from(direct),
            -2.0,
            PositionUnit::Units,
        )?;
        let portfolio = Portfolio::builder("PORTFOLIO")
            .base_currency(Currency::USD)
            .as_of(date!(2025 - 01 - 01))
            .entity(Entity::new("ENTITY"))
            .position(composite_position)
            .position(direct_position)
            .build()?;

        let report = primitive_exposure_report(&portfolio, &MarketContext::new(), &[])?;
        let long = report
            .aggregates
            .iter()
            .find(|aggregate| aggregate.instrument_id.as_str() == "COMPOSITE-LONG")
            .ok_or_else(|| Error::validation("missing long primitive aggregate"))?;
        assert_eq!(long.net_quantity, 0.0);
        assert_eq!(long.gross_quantity, 4.0);
        assert_eq!(long.net_value.amount(), 0.0);
        assert_eq!(long.gross_value.amount(), 400.0);
        Ok(())
    }
}
