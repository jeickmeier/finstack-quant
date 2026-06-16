//! Build the initial `MarketContext` for a v3 `CalibrationEnvelope`.
//!
//! Replaces the legacy `MarketContext::try_from(MarketContextState)` path used
//! by the v2 envelope. The v3 envelope splits market inputs into:
//!
//! - `prior_market`: pre-built calibrated curves and surfaces — applied first
//!   so credit-index reconstruction (which resolves hazard / base-correlation
//!   curves out of the populated context) sees every curve it might need.
//! - `market_data`: flat id-addressable inputs (FX spots, prices, fixings,
//!   dividends, credit indices, vol surfaces / cubes, CSA collateral mappings,
//!   and quote variants).
//!
//! Quote variants (`*Quote`) are intentionally skipped here — they're consumed
//! per-step by the engine (`resolve_step_quotes`), not stored on the context.
//!
//! Ordering rationale: curves go in first, then market-data buckets are applied
//! in an order where credit indices come *last* so their `get_hazard` /
//! `get_base_correlation` lookups always resolve against the fully populated
//! context (matching the legacy `try_from` semantics).

use crate::calibration::api::market_datum::MarketDatum;
use crate::calibration::api::prior_market::PriorMarketObject;
use crate::calibration::config::CalibrationConfig;
use finstack_quant_core::market_data::context::{
    build_snapshot_fx_matrix, CurveState, CurveStorage, MarketContext,
};
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use std::sync::Arc;

/// Materialize a [`MarketContext`] from the v3 envelope inputs.
///
/// Mirrors the semantics of `MarketContext::try_from(MarketContextState)` but
/// operates on the flat `(prior, data, settings)` shape used by
/// [`crate::calibration::api::schema::CalibrationEnvelope`].
///
/// # Errors
/// Returns any error surfaced by [`build_snapshot_fx_matrix`] (during FX matrix
/// construction) or by `MarketContext::get_hazard` /
/// `MarketContext::get_base_correlation` lookups when reconstructing credit
/// indices.
pub fn build_initial_context(
    prior: &[PriorMarketObject],
    data: &[MarketDatum],
    settings: &CalibrationConfig,
) -> finstack_quant_core::Result<MarketContext> {
    let mut ctx = MarketContext::new();

    // -------------------------------------------------------------------------
    // 1. Apply prior curves and surfaces first so credit-index reconstruction
    //    (later, from `market_data`) can resolve hazard / base-correlation
    //    references out of the populated context.
    // -------------------------------------------------------------------------
    for obj in prior {
        match obj {
            PriorMarketObject::DiscountCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::Discount(c.clone())));
            }
            PriorMarketObject::ForwardCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::Forward(c.clone())));
            }
            PriorMarketObject::HazardCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::Hazard(c.clone())));
            }
            PriorMarketObject::InflationCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::Inflation(c.clone())));
            }
            PriorMarketObject::BaseCorrelationCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::BaseCorrelation(
                    c.clone(),
                )));
            }
            PriorMarketObject::BasisSpreadCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::BasisSpread(c.clone())));
            }
            PriorMarketObject::ParametricCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::Parametric(c.clone())));
            }
            PriorMarketObject::PriceCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::Price(c.clone())));
            }
            PriorMarketObject::VolatilityIndexCurve(c) => {
                ctx.insert_mut(CurveStorage::from_state(CurveState::VolIndex(c.clone())));
            }
            PriorMarketObject::VolSurface(s) => {
                ctx.insert_surface_mut(s.clone());
            }
        }
    }

    // -------------------------------------------------------------------------
    // 2. Bucket `MarketDatum` entries by kind so credit indices can be applied
    //    after everything else (they need curves resolvable via the context).
    // -------------------------------------------------------------------------
    let mut fx_quotes: Vec<(
        finstack_quant_core::currency::Currency,
        finstack_quant_core::currency::Currency,
        f64,
    )> = Vec::new();
    let mut credit_states: Vec<&finstack_quant_core::market_data::context::CreditIndexState> =
        Vec::new();

    for datum in data {
        match datum {
            MarketDatum::FxSpot(fx) => {
                fx_quotes.push((fx.from, fx.to, fx.rate));
            }
            MarketDatum::Price(p) => {
                ctx.insert_price_mut(CurveId::from(p.id.clone()), p.scalar.clone());
            }
            MarketDatum::DividendSchedule(d) => {
                ctx.insert_dividends_mut(d.schedule.clone());
            }
            MarketDatum::FixingSeries(s) => {
                ctx.insert_series_mut(s.clone());
            }
            MarketDatum::InflationFixings(i) => {
                ctx.insert_inflation_index_mut(i.id.clone(), i.clone());
            }
            MarketDatum::CreditIndex(c) => {
                // Defer until all curves are in place.
                credit_states.push(c);
            }
            MarketDatum::FxVolSurface(s) => {
                ctx.insert_fx_delta_vol_surface_mut(s.clone());
            }
            MarketDatum::VolCube(c) => {
                ctx.insert_vol_cube_mut(c.clone());
            }
            MarketDatum::Collateral(c) => {
                ctx.map_collateral_mut(c.id.to_string(), CurveId::from(c.csa_currency.to_string()));
            }
            // Quote variants are consumed per-step by the engine, not stored
            // on the market context.
            MarketDatum::RateQuote(_)
            | MarketDatum::CdsQuote(_)
            | MarketDatum::CdsTrancheQuote(_)
            | MarketDatum::FxQuote(_)
            | MarketDatum::InflationQuote(_)
            | MarketDatum::VolQuote(_)
            | MarketDatum::XccyQuote(_)
            | MarketDatum::BondQuote(_) => {}
        }
    }

    // -------------------------------------------------------------------------
    // 3. Materialize the FX matrix from the bucketed spot quotes (if any).
    //    Uses the same quote-only snapshot provider path as
    //    `MarketContext::try_from(MarketContextState)`.
    // -------------------------------------------------------------------------
    if !fx_quotes.is_empty() {
        tracing::info!(
            explicit_quote_count = fx_quotes.len(),
            "building MarketContext FX as quote-only snapshot (v3 envelope)"
        );
        let matrix = build_snapshot_fx_matrix(settings.fx, fx_quotes)?;
        ctx.insert_fx_mut(matrix);
    }

    // -------------------------------------------------------------------------
    // 4. Reconstruct credit indices last — `get_hazard` /
    //    `get_base_correlation` need every relevant curve already present.
    //    Mirrors `state_serde::try_from(MarketContextState)`.
    // -------------------------------------------------------------------------
    for credit_state in credit_states {
        let index_curve = ctx.get_hazard(&credit_state.index_credit_curve_id)?;
        let base_corr = ctx.get_base_correlation(&credit_state.base_correlation_curve_id)?;
        let issuer_curves = if let Some(issuer_ids) = credit_state.issuer_credit_curve_ids.as_ref()
        {
            let mut map = HashMap::default();
            for (issuer, curve_id) in issuer_ids {
                let curve = ctx.get_hazard(curve_id)?;
                map.insert(issuer.clone(), curve);
            }
            Some(map)
        } else {
            None
        };

        let data = CreditIndexData {
            num_constituents: credit_state.num_constituents,
            recovery_rate: credit_state.recovery_rate,
            index_credit_curve: Arc::clone(&index_curve),
            base_correlation_curve: Arc::clone(&base_corr),
            issuer_credit_curves: issuer_curves,
            issuer_recovery_rates: credit_state
                .issuer_recovery_rates
                .as_ref()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect()),
            issuer_weights: credit_state
                .issuer_weights
                .as_ref()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect()),
        };

        ctx.insert_credit_index_mut(&credit_state.id, data);
    }

    // -------------------------------------------------------------------------
    // 5. Attach hierarchy if configured.
    // -------------------------------------------------------------------------
    if let Some(h) = settings.hierarchy.clone() {
        ctx.set_hierarchy(h);
    }

    Ok(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_inputs_build_empty_context() {
        let cfg = CalibrationConfig::default();
        let ctx = build_initial_context(&[], &[], &cfg).expect("empty build succeeds");
        // No public field accessors; use the stats helper for a coarse-grained
        // emptiness check.
        let stats = ctx.stats();
        assert_eq!(stats.total_curves, 0);
        assert_eq!(stats.surface_count, 0);
        assert!(ctx.fx().is_none());
        assert!(ctx.hierarchy().is_none());
    }

    #[test]
    fn prior_discount_curve_is_inserted() {
        use finstack_quant_core::dates::Date;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::interp::InterpStyle;
        use time::Month;

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("valid date"))
            .knots([(0.0, 1.0), (5.0, 0.9)])
            .interp(InterpStyle::MonotoneConvex)
            .build()
            .expect("DiscountCurve builder should succeed");
        let prior = vec![PriorMarketObject::DiscountCurve(curve)];
        let cfg = CalibrationConfig::default();
        let ctx = build_initial_context(&prior, &[], &cfg).expect("build succeeds");
        assert!(ctx.get_discount("USD-OIS").is_ok());
    }

    #[test]
    fn fx_spot_datum_yields_fx_matrix() {
        use crate::calibration::api::market_datum::FxSpotDatum;
        use finstack_quant_core::currency::Currency;

        let data = vec![MarketDatum::FxSpot(FxSpotDatum {
            id: "EUR/USD".into(),
            from: Currency::EUR,
            to: Currency::USD,
            rate: 1.085,
        })];
        let cfg = CalibrationConfig::default();
        let ctx = build_initial_context(&[], &data, &cfg).expect("build succeeds");
        assert!(ctx.fx().is_some());
    }
}
