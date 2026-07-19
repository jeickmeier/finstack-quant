//! Finite-difference and repricing utilities for portfolio sensitivities.
//!
use crate::dependencies::{flatten_dependencies, MarketFactorKey};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::{CurveStorage, MarketContext};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};
use finstack_quant_factor_model::sensitivity_matrix::SensitivityMatrix;
use finstack_quant_factor_model::{FactorDefinition, MarketMapping};
use finstack_quant_valuations::instruments::{Instrument, RatesCurveKind};

/// Resolve one factor definition to the exact dependency keys its market bump
/// can change.
///
/// Curve mappings resolve each identifier against the actual market storage;
/// the declarative factor type is not sufficient because factor-model configs
/// do not require it to match the stored curve role. An unsupported or missing
/// storage classification returns `None`, which tells callers to
/// conservatively reprice every position. `curve_ids_override` is used by
/// assignment-driven credit factors whose concrete curve IDs are discovered
/// from the portfolio rather than stored in the definition.
pub(crate) fn exact_factor_market_keys(
    factor: &FactorDefinition,
    market: &MarketContext,
    curve_ids_override: Option<&[CurveId]>,
) -> Option<Vec<MarketFactorKey>> {
    let mut keys = Vec::new();
    match &factor.market_mapping {
        MarketMapping::CurveParallel { curve_ids, .. } => {
            let curve_ids = curve_ids_override.unwrap_or(curve_ids);
            for curve_id in curve_ids {
                push_unique_key(&mut keys, exact_curve_key(market, curve_id)?);
            }
        }
        MarketMapping::CurveBucketed { curve_id, .. } => {
            push_unique_key(&mut keys, exact_curve_key(market, curve_id)?);
        }
        MarketMapping::EquitySpot { tickers } => {
            for ticker in tickers {
                push_unique_key(&mut keys, exact_spot_or_series_key(market, ticker)?);
            }
        }
        MarketMapping::FxRate { pair } => {
            push_unique_key(&mut keys, MarketFactorKey::fx(pair.0, pair.1));
        }
        MarketMapping::VolShift { surface_ids, .. } => {
            for surface_id in surface_ids {
                // Generic curve bumps resolve curves before surfaces. A
                // same-named curve therefore makes a VolShift mapping
                // ambiguous even when a vol surface is also present.
                if market.curve(surface_id.as_str()).is_some()
                    || market.get_surface(surface_id.as_str()).is_err()
                {
                    return None;
                }
                push_unique_key(&mut keys, MarketFactorKey::vol_surface(surface_id.clone()));
            }
        }
    }
    Some(keys)
}

fn exact_curve_key(market: &MarketContext, curve_id: &CurveId) -> Option<MarketFactorKey> {
    let kind = match market.curve(curve_id.as_str())? {
        CurveStorage::Discount(_) => RatesCurveKind::Discount,
        CurveStorage::Forward(_) => RatesCurveKind::Forward,
        CurveStorage::Hazard(_) => RatesCurveKind::Credit,
        CurveStorage::Inflation(_) => RatesCurveKind::Inflation,
        CurveStorage::BaseCorrelation(_)
        | CurveStorage::Price(_)
        | CurveStorage::VolIndex(_)
        | CurveStorage::BasisSpread(_)
        | CurveStorage::Parametric(_) => return None,
    };
    Some(MarketFactorKey::curve(curve_id.clone(), kind))
}

fn exact_spot_or_series_key(market: &MarketContext, id: &str) -> Option<MarketFactorKey> {
    // MarketContext's generic Curve bump resolves in the order curve,
    // surface, scalar price, then time series. Only the latter two have exact
    // portfolio dependency keys for an EquitySpot mapping.
    if market.curve(id).is_some() || market.get_surface(id).is_ok() {
        return None;
    }
    if market.get_price(id).is_ok() {
        return Some(MarketFactorKey::spot(id));
    }
    if market.get_series(id).is_ok() {
        return Some(MarketFactorKey::series(id));
    }
    None
}

fn push_unique_key(keys: &mut Vec<MarketFactorKey>, key: MarketFactorKey) {
    if !keys.contains(&key) {
        keys.push(key);
    }
}

fn dependencies_intersect_factor(
    dependencies: &finstack_quant_core::HashSet<MarketFactorKey>,
    factor_keys: &[MarketFactorKey],
) -> bool {
    // FxMatrix rebuilds derived cross rates after any quote bump. A position
    // depending on EUR/JPY can therefore move when USD/EUR changes even
    // though neither the direct nor reverse dependency key matches. Keep the
    // routing conservative for FX while retaining exact matching elsewhere.
    if factor_keys
        .iter()
        .any(|key| matches!(key, MarketFactorKey::Fx { .. }))
        && dependencies
            .iter()
            .any(|key| matches!(key, MarketFactorKey::Fx { .. }))
    {
        return true;
    }

    factor_keys.iter().any(|key| {
        dependencies.contains(key)
            || matches!(
                key,
                MarketFactorKey::Fx { base, quote }
                    if dependencies.contains(&MarketFactorKey::fx(*quote, *base))
            )
    })
}

/// Precomputed position routing for an ordered set of factors.
///
/// The plan is request-local. Instruments with dependency-introspection
/// failures are included in every non-empty factor mapping, and ambiguous
/// factor mappings include every position. Resolved positions with no matching
/// dependency are proven unaffected and receive an exact zero without a
/// repricing call.
pub(crate) struct FactorRepricingPlan {
    affected_by_factor: Vec<Vec<bool>>,
}

impl FactorRepricingPlan {
    pub(crate) fn build(
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        market: &MarketContext,
    ) -> Self {
        let position_dependencies: Vec<_> = positions
            .iter()
            .map(|(_, instrument, _)| {
                instrument
                    .market_dependencies()
                    .ok()
                    .and_then(|dependencies| {
                        let resolved = flatten_dependencies(&dependencies);
                        // The Instrument trait's compatibility default is an
                        // empty dependency set, so emptiness cannot prove that
                        // a custom/legacy instrument is market-independent.
                        // Treat it as unresolved and retain full repricing.
                        (!resolved.is_empty()).then_some(resolved)
                    })
            })
            .collect();

        let affected_by_factor = factors
            .iter()
            .map(
                |factor| match exact_factor_market_keys(factor, market, None) {
                    None => vec![true; positions.len()],
                    Some(keys) => position_dependencies
                        .iter()
                        .map(|dependencies| {
                            dependencies.as_ref().map_or(!keys.is_empty(), |resolved| {
                                dependencies_intersect_factor(resolved, &keys)
                            })
                        })
                        .collect(),
                },
            )
            .collect();

        Self { affected_by_factor }
    }

    pub(crate) fn affected(&self, factor_index: usize) -> &[bool] {
        &self.affected_by_factor[factor_index]
    }
}

/// Validate that every position prices in the same native currency.
///
/// The factor sensitivity engines build deltas from raw native-currency PVs
/// and the downstream decomposers column-sum them across positions. Mixing
/// currencies would silently add e.g. USD and EUR DV01s unit-for-unit,
/// violating the workspace no-implicit-cross-currency invariant. This check
/// errors loudly instead of converting; callers with multi-currency
/// portfolios must convert positions to a common base currency upstream.
///
/// # Errors
///
/// Returns [`Error::Validation`] naming the two positions whose pricing
/// currencies differ, or propagates any pricing error from `base_value`.
pub(crate) fn validate_single_currency(
    positions: &[(String, &dyn Instrument, f64)],
    market: &MarketContext,
    as_of: Date,
) -> Result<()> {
    let mut first: Option<(&str, finstack_quant_core::currency::Currency)> = None;
    for (position_id, instrument, _) in positions {
        let currency = instrument.value(market, as_of)?.currency();
        match first {
            None => first = Some((position_id.as_str(), currency)),
            Some((first_id, first_currency)) if first_currency != currency => {
                return Err(Error::Validation(format!(
                    "Factor sensitivity engine requires a single pricing currency: \
                     position '{first_id}' prices in {first_currency} but position \
                     '{position_id}' prices in {currency}; convert positions to a \
                     common base currency before computing factor sensitivities"
                )));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Engine for computing per-position, per-factor sensitivities.
pub trait FactorSensitivityEngine: Send + Sync {
    /// Compute a sensitivity matrix for `positions` against `factors`.
    fn compute_sensitivities(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        market: &MarketContext,
        as_of: Date,
    ) -> Result<SensitivityMatrix>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_factor_model::{FactorId, FactorType};
    use time::macros::date;

    #[test]
    fn curve_routing_uses_actual_market_storage_not_declared_factor_type() {
        let curve_id = CurveId::new("USD-OIS");
        let discount = DiscountCurve::builder(curve_id.clone())
            .base_date(date!(2025 - 01 - 01))
            .knots([(0.0, 1.0), (1.0, 0.96)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(discount);
        let factor = FactorDefinition {
            id: FactorId::new("misclassified-credit"),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![curve_id.clone()],
                units: BumpUnits::RateBp,
            },
            description: None,
        };

        assert_eq!(
            exact_factor_market_keys(&factor, &market, None),
            Some(vec![MarketFactorKey::curve(
                curve_id,
                RatesCurveKind::Discount,
            )]),
        );
    }

    #[test]
    fn missing_curve_storage_requests_conservative_full_repricing() {
        let factor = FactorDefinition {
            id: FactorId::new("missing"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("MISSING")],
                units: BumpUnits::RateBp,
            },
            description: None,
        };

        assert_eq!(
            exact_factor_market_keys(&factor, &MarketContext::new(), None),
            None,
        );
    }

    #[test]
    fn any_fx_quote_affects_triangulated_fx_dependencies() {
        let dependencies = finstack_quant_core::HashSet::from_iter([MarketFactorKey::fx(
            finstack_quant_core::currency::Currency::EUR,
            finstack_quant_core::currency::Currency::JPY,
        )]);

        assert!(dependencies_intersect_factor(
            &dependencies,
            &[MarketFactorKey::fx(
                finstack_quant_core::currency::Currency::USD,
                finstack_quant_core::currency::Currency::EUR,
            )],
        ));
    }
}
