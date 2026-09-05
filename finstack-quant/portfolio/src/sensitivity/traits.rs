//! Finite-difference and repricing utilities for portfolio sensitivities.
//!
use crate::dependencies::{flatten_dependencies, MarketFactorKey};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::{CurveStorage, MarketContext};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_models::factor::{FactorDefinition, MarketMapping, SensitivityMatrix};
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
        MarketMapping::VolShift {
            vol_surface_ids, ..
        } => {
            for vol_surface_id in vol_surface_ids {
                // Generic curve bumps resolve curves before surfaces. A
                // same-named curve therefore makes a VolShift mapping
                // ambiguous even when a vol surface is also present.
                if market.curve(vol_surface_id.as_str()).is_some()
                    || market.get_surface(vol_surface_id.as_str()).is_err()
                {
                    return None;
                }
                push_unique_key(
                    &mut keys,
                    MarketFactorKey::vol_surface(vol_surface_id.clone()),
                );
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
/// dependency are proven unaffected for **native** pricing and receive an
/// exact zero unless the factor is an FX mapping: engines still reconvert
/// those rows through the bumped spot matrix so translation P&L is not
/// dropped.
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
                    .map(|dependencies| flatten_dependencies(&dependencies))
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

/// Price an instrument in its native currency, then convert on `market` at `as_of`.
///
/// Factor stress, delta, and full-reprice endpoints all use this path so FX
/// factors flow through the bumped market's spot matrix rather than an
/// implied PV ratio. Same-currency amounts short-circuit inside
/// the portfolio spot FX helper. Non-finite native PVs are returned
/// unchanged so callers can emit their position-specific validation error
/// without constructing `Money`. Missing FX for a cross-currency position
/// fails the same way NAV does.
///
/// # Arguments
///
/// * `instrument` - Instrument to price in its native currency.
/// * `market` - Market used for both native pricing and the spot FX lookup.
///   Callers must pass the **bumped** market when computing a shocked PV.
/// * `as_of` - Valuation date for pricing and the FX matrix query.
/// * `base_currency` - Reporting currency; same-currency amounts are identity.
///
/// # Errors
///
/// Propagates instrument pricing failures and portfolio spot FX conversion
/// errors (missing FX matrix or missing pair).
pub(crate) fn raw_pv_in_base(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    base_currency: Currency,
) -> Result<f64> {
    let (amount, currency) = instrument.value_raw_with_currency(market, as_of)?;
    if !amount.is_finite() {
        return Ok(amount);
    }
    Ok(
        crate::fx::convert_to_base(Money::new(amount, currency)?, as_of, market, base_currency)?
            .amount(),
    )
}

/// Whether a factor bump can change the FX matrix used by [`raw_pv_in_base`].
///
/// Instruments that do not declare an FX dependency still have a translation
/// effect when their native currency differs from the reporting currency, so
/// FX mappings must reprice (or at least reconvert) every position.
///
/// # Arguments
///
/// * `mapping` - Factor-to-market mapping whose bump target is inspected.
pub(crate) fn mapping_bumps_fx(mapping: &MarketMapping) -> bool {
    matches!(mapping, MarketMapping::FxRate { .. })
}

/// Engine for computing per-position, per-factor sensitivities.
pub trait FactorSensitivityEngine: Send + Sync {
    /// Compute a sensitivity matrix for `positions` against `factors`.
    ///
    /// Each cell is a central difference of **base-currency** PVs:
    /// `(PV_up_base − PV_down_base) / (2h) * weight`. Native PVs are converted
    /// with the portfolio spot FX helper on the **bumped** market at `as_of`.
    /// When the caller wraps a [`crate::Portfolio`], `weight` is
    /// [`crate::position::Position::scale_factor`].
    ///
    /// # Arguments
    ///
    /// * `positions` - `(id, instrument, weight)` rows in matrix order.
    /// * `factors` - Factor definitions that select the market bumps.
    /// * `market` - Unbumped market snapshot; engines bump it per factor.
    /// * `as_of` - Valuation date for pricing and spot FX lookup.
    /// * `base_currency` - Reporting currency for every converted PV.
    fn compute_sensitivities(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        market: &MarketContext,
        as_of: Date,
        base_currency: Currency,
    ) -> Result<SensitivityMatrix>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_models::factor::{FactorId, FactorType};
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
