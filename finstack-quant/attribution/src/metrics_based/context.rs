use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    measure_credit_curve_shift, measure_fx_shift, measure_scalar_absolute_shift,
    measure_scalar_shift, TenorSamplingMethod,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_models::volatility::measure_vol_surface_shift;
use finstack_quant_valuations::instruments::{Instrument, MarketDependencies};
use finstack_quant_valuations::results::ValuationResult;
use std::sync::Arc;

use super::shifts::{average_over, measure_rate_curve_shift_bp};

pub(super) struct MarketShifts {
    pub(super) avg_rate_shift_bp: Option<f64>,
    pub(super) rate_curves_measured: usize,
    pub(super) avg_credit_shift_bp: Option<f64>,
    pub(super) credit_curves_measured: usize,
    pub(super) avg_vol_shift_abs: Option<f64>,
    pub(super) vol_shift_error: Option<String>,
    pub(super) fx_shift_pct: Option<f64>,
    pub(super) avg_spot_shift_pct: Option<f64>,
}

pub(super) struct AttributionInputs<'a> {
    pub(super) instrument: &'a Arc<dyn Instrument>,
    pub(super) market_t0: &'a MarketContext,
    pub(super) market_t1: &'a MarketContext,
    pub(super) val_t0: &'a ValuationResult,
    pub(super) val_t1: &'a ValuationResult,
    pub(super) time_period_days: f64,
    pub(super) ccy: Currency,
    pub(super) market_deps: MarketDependencies,
    pub(super) rates_curve_ids: Vec<CurveId>,
    pub(super) shifts: MarketShifts,
}

impl<'a> AttributionInputs<'a> {
    pub(super) fn new(
        instrument: &'a Arc<dyn Instrument>,
        market_t0: &'a MarketContext,
        market_t1: &'a MarketContext,
        val_t0: &'a ValuationResult,
        val_t1: &'a ValuationResult,
        as_of_t0: Date,
        as_of_t1: Date,
    ) -> Result<Self> {
        let market_deps = instrument.market_dependencies()?;

        // ─── Preamble: compute market-shift averages ONCE ──────────────────────
        //
        // Each `measure_*_shift` helper is pure: same inputs → same output, so
        // computing the per-factor averages once up-front and threading them
        // through the per-factor blocks keeps the computation deterministic and
        // avoids the former pattern of redundant second loops over the same
        // curves.
        //
        // Iteration order is identical to the previous per-block loops:
        //   - discount_curves / credit_curves / market_scalar_ids in the order returned
        //     by `market_deps.curves` / `market_deps.market_scalar_ids`
        //     (preserve the existing HashMap/Vec iteration order — do NOT sort).
        //   - FX exposure / vol surface: single-valued, no ordering concern.
        //
        // A failed (Err) shift measurement skips that curve from the average,
        // matching the prior behavior.

        // All rates curves the instrument depends on: discount AND
        // forward/projection (multi-curve swaps carry a joint
        // discount+forward DV01, and basis moves require measuring both
        // families). Order: discount first, then forward — deterministic.
        //
        // Deduped preserving order (first occurrence wins): a curve that is
        // both the discount and the projection curve (standard single-curve
        // OIS/SOFR IRS, FRNs) must contribute to rates P&L — and to
        // `average_rates` below — exactly once, not twice.
        let mut rates_curve_ids: Vec<CurveId> = Vec::with_capacity(
            market_deps.curves.discount_curves.len() + market_deps.curves.forward_curves.len(),
        );
        for id in market_deps
            .curves
            .discount_curves
            .iter()
            .chain(market_deps.curves.forward_curves.iter())
        {
            if !rates_curve_ids.contains(id) {
                rates_curve_ids.push(id.clone());
            }
        }
        let (avg_rate_shift_bp, rate_curves_measured) =
            average_rates(&rates_curve_ids, market_t0, market_t1);
        let (avg_credit_shift_bp, credit_curves_measured) =
            average_credit(&market_deps, market_t0, market_t1);
        let (avg_vol_shift_abs, vol_shift_error) = match volatility_shift(
            instrument.as_ref(),
            &market_deps,
            market_t0,
            market_t1,
            as_of_t0,
        ) {
            Ok(movement) => (movement, None),
            Err(error) => (None, Some(error.to_string())),
        };
        let fx_shift_pct = instrument
            .fx_exposure()
            .and_then(|(base_currency, quote_currency)| {
                measure_fx_shift(
                    base_currency,
                    quote_currency,
                    market_t0,
                    market_t1,
                    as_of_t0,
                    as_of_t1,
                )
                .ok()
            });
        let avg_spot_shift_pct = average_spot(&market_deps, market_t0, market_t1);
        Ok(Self {
            instrument,
            market_t0,
            market_t1,
            val_t0,
            val_t1,
            time_period_days: (as_of_t1 - as_of_t0).whole_days() as f64,
            ccy: val_t1.value.currency(),
            market_deps,
            rates_curve_ids,
            shifts: MarketShifts {
                avg_rate_shift_bp,
                rate_curves_measured,
                avg_credit_shift_bp,
                credit_curves_measured,
                avg_vol_shift_abs,
                vol_shift_error,
                fx_shift_pct,
                avg_spot_shift_pct,
            },
        })
    }
}

fn average_rates(
    curves: &[CurveId],
    t0: &MarketContext,
    t1: &MarketContext,
) -> (Option<f64>, usize) {
    average_over(curves, |id| {
        measure_rate_curve_shift_bp(id.as_str(), t0, t1)
    })
}

fn average_credit(
    deps: &MarketDependencies,
    t0: &MarketContext,
    t1: &MarketContext,
) -> (Option<f64>, usize) {
    average_over(&deps.curves.credit_curves, |id| {
        measure_credit_curve_shift(id.as_str(), t0, t1, TenorSamplingMethod::Standard).ok()
    })
}

fn average_spot(deps: &MarketDependencies, t0: &MarketContext, t1: &MarketContext) -> Option<f64> {
    deps.market_scalar_ids
        .first()
        .and_then(|id| measure_scalar_shift(id, t0, t1).ok())
}

fn volatility_shift(
    instrument: &dyn Instrument,
    deps: &MarketDependencies,
    t0: &MarketContext,
    t1: &MarketContext,
    as_of_t0: Date,
) -> Result<Option<f64>> {
    let expiry = instrument
        .expiry()
        .map(|date| (date - as_of_t0).whole_days() as f64 / 365.0)
        .filter(|years| *years > 0.0);
    let mut result: Option<f64> = None;
    for id in deps.unique_vol_surface_ids() {
        let dependency = deps
            .volatility_dependencies
            .iter()
            .find(|d| d.vol_surface_id == id)
            .ok_or_else(|| {
                finstack_quant_core::Error::Internal("missing volatility dependency".into())
            })?;
        let shift = if let Ok(surface) = t0.get_surface(id.as_str()) {
            if expiry.is_some() && dependency.reference_strike.is_some() {
                measure_vol_surface_shift(id.as_str(), t0, t1, expiry, dependency.reference_strike)?
            } else {
                // Aggregate Vega only determines a parallel move without a
                // contractual sampling point. Verify uniformity, never average a twist away.
                let closing_surface = t1.get_surface(id.as_str())?;
                let mut uniform: Option<f64> = None;
                for &tenor in surface.expiries().iter().chain(closing_surface.expiries()) {
                    for &strike in surface.strikes().iter().chain(closing_surface.strikes()) {
                        let movement = measure_vol_surface_shift(
                            id.as_str(),
                            t0,
                            t1,
                            Some(tenor),
                            Some(strike),
                        )?;
                        if uniform.is_some_and(|first| (first - movement).abs() > 1e-9) {
                            return Err(finstack_quant_core::Error::Validation(format!(
                                "Vega for '{id}' requires reference expiry/strike or bucketed risk for a non-parallel surface move"
                            )));
                        }
                        uniform = Some(movement);
                    }
                }
                uniform.unwrap_or(0.0)
            }
        } else if t0.get_price(id.as_str()).is_ok() {
            // Scalar volatility has the same decimal convention as surface values.
            measure_scalar_absolute_shift(id.as_str(), t0, t1)? * 100.0
        } else {
            continue;
        };
        if result.is_some_and(|first| (first - shift).abs() > 1e-9) {
            return Err(finstack_quant_core::Error::Validation(
                "Aggregate Vega cannot distinguish different moves on multiple volatility sources; supply per-source risk".into()
            ));
        }
        result = Some(shift);
    }
    Ok(result)
}
