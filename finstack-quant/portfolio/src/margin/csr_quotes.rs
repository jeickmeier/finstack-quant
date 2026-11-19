use super::regulatory_quotes::RegulatoryQuoteReprice;
use crate::valuation::{PortfolioValuationOptions, RequestedMetrics, value_portfolio_at};
use crate::{Error, Portfolio, Result};
use finstack_quant_calibration::api::schema::HazardCurveParams;
use finstack_quant_calibration::quotes::{CdsQuote, Pillar};
use finstack_quant_calibration::recalibration::{
    CachedRecalibrationProvider, CalibrationQuoteCoordinate,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::Seniority;
use finstack_quant_core::types::CurveId;
use finstack_quant_margin::regulatory::frtb::FrtbSensitivities;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::recalibration::{
    HazardRecalibrationAction, HazardRecalibrationRequest, RecalibrationProvider,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const CSR_VERTICES: [&str; 5] = ["0.5Y", "1Y", "3Y", "5Y", "10Y"];

/// Sourced identity and approved non-securitisation CSR risk bucket.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CsrQuoteClassification {
    /// Authoritative issuer identifier matching the executed calibration quotes.
    pub issuer: String,
    /// Payment currency of the CDS spread and contractual claims.
    pub currency: Currency,
    /// Seniority of the funded reference claim.
    pub seniority: Seniority,
    /// One-based prescribed non-securitisation CSR bucket.
    pub bucket: u8,
    /// Identifier of the classification evidence admitted for this date.
    pub source_id: String,
    /// Information cut of the supplied issuer, bucket and seniority assignment.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
}

/// Exact market source and quote grid for one classified issuer CDS curve.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CsrQuoteSource {
    /// Exact hazard curve in the issuer market.
    pub curve_id: CurveId,
    /// Authoritative, dated identity and CSR bucket for the source.
    pub classification: CsrQuoteClassification,
    /// One executed coordinate per prescribed CSR vertex.
    pub vertices: std::collections::BTreeMap<String, CalibrationQuoteCoordinate>,
}

struct PreparedCsrQuotes<'a> {
    request: HazardRecalibrationRequest,
    indices: Vec<(&'a str, usize)>,
}

impl<'a> PreparedCsrQuotes<'a> {
    fn new(
        source: &'a CsrQuoteSource,
        market: &Arc<MarketContext>,
        date: Date,
        provider: &CachedRecalibrationProvider,
    ) -> Result<Self> {
        if source.classification.issuer.trim().is_empty()
            || source.classification.source_id.trim().is_empty()
            || source.classification.as_of > date
            || !(1..=18).contains(&source.classification.bucket)
        {
            return Err(Error::invalid_input(
                "Invalid or future-dated non-sec CSR classification",
            ));
        }
        if source.vertices.len() != CSR_VERTICES.len()
            || CSR_VERTICES
                .iter()
                .any(|vertex| !source.vertices.contains_key(*vertex))
        {
            return Err(Error::invalid_input(
                "Non-sec CSR requires every exact 0.5Y, 1Y, 3Y, 5Y and 10Y quote",
            ));
        }
        let hazard = market.get_hazard(source.curve_id.as_str())?;
        let discount_curve_id = provider.get_hazard_discount_curve_id(&hazard)?;
        let bindings = provider.hazard_spread_risk_buckets(&hazard)?;
        let recipe = hazard
            .hazard_calibration()
            .ok_or_else(|| Error::invalid_input("Missing exact issuer spread-risk recipe"))?;
        if recipe.calibration_inputs() != recipe.spread_risk_inputs() {
            return Err(Error::invalid_input(format!(
                "CSR curve '{}' requires executed par-spread quotes for the full replay strip",
                source.curve_id
            )));
        }
        let calibration: HazardCurveParams = serde_json::from_value(recipe.hazard_params().clone())
            .map_err(|err| {
                Error::invalid_input(format!("Invalid issuer calibration evidence: {err}"))
            })?;
        if calibration.entity != source.classification.issuer
            || calibration.currency != source.classification.currency
            || calibration.seniority != source.classification.seniority
        {
            return Err(Error::invalid_input(format!(
                "CSR issuer, currency or seniority differs from the executed curve '{}'",
                source.curve_id
            )));
        }
        let mut indices = Vec::with_capacity(CSR_VERTICES.len());
        for vertex in CSR_VERTICES {
            let quote = source.vertices.get(vertex).ok_or_else(|| {
                Error::invalid_input(format!(
                    "Missing CSR vertex {vertex} on curve '{}'",
                    source.curve_id
                ))
            })?;
            let mut matches = bindings.iter().filter(|binding| {
                binding.quote_id == quote.quote_id && binding.pillar_date == quote.pillar_date
            });
            let Some(binding) = matches.next() else {
                return Err(Error::invalid_input(format!(
                    "Missing executed CSR quote '{}' on {}",
                    quote.quote_id, source.curve_id
                )));
            };
            if matches.next().is_some()
                || indices
                    .iter()
                    .any(|(_, index)| *index == binding.quote_index)
            {
                return Err(Error::invalid_input(
                    "Ambiguous or duplicate CSR quote mapping",
                ));
            }
            let stored = recipe
                .spread_risk_inputs()
                .get(binding.quote_index)
                .ok_or_else(|| {
                    Error::invalid_input("CSR quote index is outside the stored replay recipe")
                })?;
            let quote: CdsQuote = serde_json::from_value(stored.quote.clone()).map_err(|err| {
                Error::invalid_input(format!("Invalid CSR quote evidence: {err}"))
            })?;
            check_vertex(vertex, &quote, source)?;
            indices.push((vertex, binding.quote_index));
        }
        let request = HazardRecalibrationRequest {
            hazard,
            source_market: Arc::clone(market),
            target_market: Arc::clone(market),
            discount_curve_id,
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
            action: HazardRecalibrationAction::SpreadRiskCenterReplay,
        };
        Ok(Self { request, indices })
    }

    fn market(
        &self,
        provider: &CachedRecalibrationProvider,
        action: HazardRecalibrationAction,
    ) -> Result<MarketContext> {
        let request = HazardRecalibrationRequest {
            action,
            ..self.request.clone()
        };
        let curve = provider.rebuild_hazard_curve(&request)?;
        Ok(request
            .target_market
            .as_ref()
            .clone()
            .try_insert(curve.as_ref().clone())?)
    }
}

fn check_vertex(vertex: &str, quote: &CdsQuote, source: &CsrQuoteSource) -> Result<()> {
    let invalid = || {
        Error::invalid_input(format!(
            "CSR quote '{}' does not establish the exact {vertex} issuer vertex for '{}'",
            quote.id(),
            source.classification.issuer
        ))
    };
    let CdsQuote::CdsParSpread {
        entity,
        convention,
        pillar: Pillar::Tenor(tenor),
        ..
    } = quote
    else {
        return Err(invalid());
    };
    let months = match tenor.unit() {
        TenorUnit::Years => tenor.count().checked_mul(12).ok_or_else(invalid)?,
        TenorUnit::Months => tenor.count(),
        _ => return Err(invalid()),
    };
    let expected = match vertex {
        "0.5Y" => 6,
        "1Y" => 12,
        "3Y" => 36,
        "5Y" => 60,
        "10Y" => 120,
        _ => return Err(invalid()),
    };
    if entity != &source.classification.issuer
        || convention.currency != source.classification.currency
        || months != expected
    {
        return Err(invalid());
    }
    Ok(())
}

/// Position reprices at an exact issuer credit spread vertex.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CsrQuoteRisk {
    /// Issuer spread curve that was rebuilt on the selected quote.
    pub curve_id: CurveId,
    /// Exact admitted CSR non-securitisation vertex.
    pub vertex: String,
    /// Position changes with the held reporting FX evidence.
    pub positions: Vec<RegulatoryQuoteReprice>,
}

/// Exact ordinary non-securitisation CDS input and its position evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CsrQuoteDelta {
    /// Portfolio valuation date for the classification and pricing.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Trading book to which all positions and reporting FX belong.
    pub portfolio_id: String,
    /// Immutable original market fingerprint, before center or quote replay.
    pub market_hash: String,
    /// Fingerprint of the configuration shared by every center and shocked price.
    pub pricing_hash: String,
    /// Qualified issuer, currency, seniority, curve and exact quote evidence.
    pub sources: Vec<CsrQuoteSource>,
    /// Signed position-level changes before summing by regulated CSR factor.
    pub quote_risk: Vec<CsrQuoteRisk>,
    /// Ordinary non-securitisation credit spread aggregation inputs.
    pub sensitivities: FrtbSensitivities,
}

/// Reprice live ordinary single-name CDS against a qualified issuer spread grid.
///
/// # Arguments
/// * `portfolio` - Trading book in one reporting currency with its selected valuation date.
/// * `market` - Held market with the original accepted issuer recipes and the applicable currency basis.
/// * `pricing` - Shared pricing configuration for each prepared center and its quote reprices.
/// * `sources` - Dated authoritative classifications and exact spread quotes covering each live issuer CDS curve.
///
/// # Errors
/// Rejects missing or inconsistent curve bindings, pricing/FX errors, incomplete
/// classification or coverage, or non-securitisation claims that this CDS route
/// cannot qualify. Index and tranche risk require their own admission.
pub fn csr_quote_delta(
    portfolio: &Portfolio,
    market: &MarketContext,
    pricing: &FinstackConfig,
    sources: &[CsrQuoteSource],
) -> Result<CsrQuoteDelta> {
    let date = portfolio.as_of;
    let market = Arc::new(market.clone());
    let provider = CachedRecalibrationProvider::default();
    let mut positions_by_source = finstack_quant_core::FxHashMap::<
        &CurveId,
        Vec<(usize, &crate::types::PositionId)>,
    >::default();
    for (index, position) in portfolio.positions.iter().enumerate() {
        if position.quantity == 0.0 {
            continue;
        }
        let deps = position.instrument.market_dependencies()?;
        if deps.curves.credit_curves.is_empty() && deps.credit_index_ids.is_empty() {
            continue;
        }
        let cds = position
            .instrument
            .as_any()
            .downcast_ref::<CreditDefaultSwap>()
            .ok_or_else(|| {
                Error::invalid_input(format!(
                    "Position '{}' has credit exposure outside the ordinary issuer CDS route",
                    position.position_id
                ))
            })?;
        if date >= cds.premium_leg.end {
            continue;
        }
        let curve_id = &cds.protection_leg.credit_curve_id;
        let source = sources
            .iter()
            .find(|s| &s.curve_id == curve_id)
            .ok_or_else(|| {
                Error::invalid_input(format!(
                    "Missing qualified CSR source for position '{}' and curve '{curve_id}'",
                    position.position_id
                ))
            })?;
        if cds.notional.currency() != source.classification.currency {
            return Err(Error::invalid_input(format!(
                "CSR quote currency differs from the protection currency on '{}'",
                position.position_id
            )));
        }
        positions_by_source
            .entry(curve_id)
            .or_default()
            .push((index, &position.position_id));
    }

    let mut seen = finstack_quant_core::FxHashSet::default();
    let mut sensitivities = FrtbSensitivities::new(portfolio.base_currency);
    let mut quote_risk = Vec::new();
    for source in sources {
        if !seen.insert(source.curve_id.as_str()) {
            return Err(Error::invalid_input(format!(
                "Duplicate CSR classification for '{}'",
                source.curve_id
            )));
        }
        let positions = positions_by_source.get(&source.curve_id).ok_or_else(|| {
            Error::invalid_input(format!(
                "No live CDS position is exposed to '{}'",
                source.curve_id
            ))
        })?;
        let prepared = PreparedCsrQuotes::new(source, &market, date, &provider)?;
        let center_market =
            prepared.market(&provider, HazardRecalibrationAction::SpreadRiskCenterReplay)?;
        let center = value_portfolio_at(
            portfolio,
            &center_market,
            pricing,
            &PortfolioValuationOptions {
                strict_risk: true,
                metrics: RequestedMetrics::Only(Vec::new()),
            },
            date,
        )?;

        for &(vertex, quote_index) in &prepared.indices {
            let shocked_market = prepared.market(
                &provider,
                HazardRecalibrationAction::ExactQuoteIndexBump {
                    quote_index,
                    bump_bp: 1.0,
                },
            )?;
            let reprices = super::regulatory_quotes::reprice_rows(
                portfolio,
                &shocked_market,
                pricing,
                &center,
                positions,
            )?;
            for position in &reprices {
                sensitivities.add_csr_nonsec_delta(
                    &source.classification.issuer,
                    source.classification.bucket,
                    vertex,
                    source.curve_id.as_str(),
                    position.pnl_base_per_pp,
                );
            }
            quote_risk.push(CsrQuoteRisk {
                curve_id: source.curve_id.clone(),
                vertex: vertex.into(),
                positions: reprices,
            });
        }
    }
    sensitivities.validate()?;
    Ok(CsrQuoteDelta {
        as_of: date,
        portfolio_id: portfolio.id.clone(),
        market_hash: finstack_quant_core::canonical::content_hash(market.as_ref())?,
        pricing_hash: finstack_quant_core::canonical::content_hash(pricing)?,
        sources: sources.to_vec(),
        quote_risk,
        sensitivities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::Entity;
    use finstack_quant_calibration::CalibrationMethod;
    use finstack_quant_calibration::api::engine::calibrate;
    use finstack_quant_calibration::api::market_datum::MarketDatum;
    use finstack_quant_calibration::api::prior_market::PriorMarketObject;
    use finstack_quant_calibration::api::schema::{
        CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams,
    };
    use finstack_quant_calibration::quotes::QuoteId;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    use time::macros::date;

    fn fit(as_of: Date, bump_five_years: f64) -> MarketContext {
        let terms = [
            ("0.5Y", 6, TenorUnit::Months, 80.0),
            ("1Y", 1, TenorUnit::Years, 85.0),
            ("3Y", 3, TenorUnit::Years, 95.0),
            ("5Y", 5, TenorUnit::Years, 105.0 + bump_five_years),
            ("10Y", 10, TenorUnit::Years, 115.0),
        ];
        let market_data = terms
            .into_iter()
            .map(|(name, count, unit, spread_bp)| {
                MarketDatum::CdsQuote(CdsQuote::CdsParSpread {
                    id: QuoteId::new(name),
                    entity: "CORP".into(),
                    convention: CdsConventionKey {
                        currency: Currency::USD,
                        doc_clause: CdsDocClause::IsdaNa,
                    },
                    pillar: Pillar::Tenor(Tenor::new(count, unit).expect("valid test tenor")),
                    spread_bp,
                    recovery_rate: 0.40,
                })
            })
            .collect();
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.81), (10.0, 0.65)])
            .interp(InterpStyle::LogLinear)
            .day_count(DayCount::Act365F)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .expect("USD discount curve");
        let plan = CalibrationPlan {
            id: "csr".into(),
            description: None,
            settings: Default::default(),
            quote_sets: [(
                "issuer".into(),
                CSR_VERTICES.iter().copied().map(QuoteId::new).collect(),
            )]
            .into_iter()
            .collect(),
            steps: vec![CalibrationStep {
                id: "hazard".into(),
                quote_set: "issuer".into(),
                params: StepParams::Hazard(HazardCurveParams {
                    curve_id: "CORP-HAZARD".into(),
                    entity: "CORP".into(),
                    seniority: Seniority::Senior,
                    currency: Currency::USD,
                    base_date: as_of,
                    discount_curve_id: "USD-OIS".into(),
                    recovery_rate: 0.40,
                    notional: 1.0,
                    method: CalibrationMethod::Bootstrap,
                    doc_clause: Some("isda_na".into()),
                    cds_valuation_convention: None,
                }),
            }],
        };
        let envelope = CalibrationEnvelope::new(
            plan,
            market_data,
            vec![PriorMarketObject::DiscountCurve(discount)],
        );
        MarketContext::try_from(calibrate(&envelope).expect("execution").result.final_market)
            .expect("market")
    }

    #[test]
    fn exact_csr_quote_delta_uses_the_declared_vertex_and_signed_position_weight() {
        let as_of = date!(2024 - 03 - 20);
        let market = fit(as_of, 0.0);
        let provider = CachedRecalibrationProvider::default();
        let hazard = market
            .get_hazard("CORP-HAZARD")
            .expect("issuer hazard curve");
        let vertices = provider
            .hazard_spread_risk_buckets(&hazard)
            .expect("risk bindings")
            .into_iter()
            .map(|quote| {
                (
                    quote.quote_id.clone(),
                    CalibrationQuoteCoordinate {
                        quote_id: quote.quote_id,
                        pillar_date: quote.pillar_date,
                    },
                )
            })
            .collect();
        let mut source = CsrQuoteSource {
            curve_id: "CORP-HAZARD".into(),
            classification: CsrQuoteClassification {
                issuer: "CORP".into(),
                currency: Currency::USD,
                seniority: Seniority::Senior,
                bucket: 5,
                source_id: "issuer-classification-review".into(),
                as_of,
            },
            vertices,
        };
        let cds = CreditDefaultSwap::example().expect("standard CDS");
        let position = Position::new(
            "SHORT",
            "CREDIT",
            cds.id(),
            Arc::new(cds.clone()),
            -2.0,
            PositionUnit::Units,
        )
        .expect("signed short position");
        let portfolio = Portfolio::builder("credit")
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("CREDIT"))
            .position(position)
            .build()
            .expect("book");
        let config = FinstackConfig::default();
        let market_hash =
            finstack_quant_core::canonical::content_hash(&market).expect("market fingerprint");
        let result = csr_quote_delta(&portfolio, &market, &config, &[source.clone()])
            .expect("CSR quote grid");
        assert_eq!(result.market_hash, market_hash);
        assert_eq!(
            result
                .quote_risk
                .iter()
                .map(|row| row.vertex.as_str())
                .collect::<Vec<_>>(),
            CSR_VERTICES
        );
        let expected = -2.0
            * (cds
                .value(&fit(as_of, 1.0), as_of)
                .expect("direct repriced mark")
                .amount()
                - cds
                    .value(&market, as_of)
                    .expect("direct base mark")
                    .amount());
        let row = &result.quote_risk[3].positions[0];
        assert!(expected.abs() > 1.0, "material quote sensitivity");
        assert!((row.pnl_native_per_bp.amount() - expected).abs() < 0.02);
        assert!((row.pnl_base_per_pp - 100.0 * expected).abs() < 2.0);
        assert_eq!(
            finstack_quant_core::canonical::content_hash(&market).unwrap(),
            market_hash
        );
        source.classification.seniority = Seniority::Junior;
        assert!(csr_quote_delta(&portfolio, &market, &config, &[source.clone()]).is_err());
        source.classification.seniority = Seniority::Senior;
        let mut swapped = source.clone();
        swapped
            .vertices
            .insert("3Y".into(), source.vertices["5Y"].clone());
        swapped
            .vertices
            .insert("5Y".into(), source.vertices["3Y"].clone());
        assert!(csr_quote_delta(&portfolio, &market, &config, &[swapped]).is_err());
        source.vertices.remove("3Y");
        assert!(csr_quote_delta(&portfolio, &market, &config, &[source]).is_err());
    }
}
