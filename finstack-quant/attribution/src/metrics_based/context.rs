use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    FxQuoteDiffParams, ProportionalDiffScale, measure_fx_quote_diff,
    measure_scalar_proportional_diff,
};
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, InputError, Result};
use finstack_quant_models::volatility::measure_vol_surface_shift;
use finstack_quant_valuations::instruments::{Instrument, MarketDependencies};
use finstack_quant_valuations::results::ValuationResult;
use std::sync::Arc;

use super::shifts::{
    CreditKeyRateBucket, RateCurveFamily, average_over, extract_credit_keyrates,
    extract_hazard_intensity_keyrates, extract_keyrate_per_curve, measure_credit_curve_shift_bp,
    measure_hazard_intensity_shift_bp, measure_per_tenor_rate_shift, measure_rate_curve_shift_bp,
    measure_scalar_native_amount_diff,
};

pub(super) struct MarketShifts {
    pub(super) avg_rate_shift_bp: Option<f64>,
    pub(super) rate_curves_measured: usize,
    pub(super) avg_credit_shift_bp: Option<f64>,
    pub(super) credit_curves_measured: usize,
    pub(super) avg_hazard_shift_bp: Option<f64>,
    pub(super) hazard_curves_measured: usize,
    pub(super) avg_vol_shift_abs: Option<f64>,
    pub(super) vol_shift_error: Option<String>,
    pub(super) fx_shift_pct: Option<f64>,
    pub(super) avg_spot_shift_pct: Option<f64>,
}

pub(super) struct MetricsContext<'a> {
    pub(super) instrument: &'a Arc<dyn Instrument>,
    pub(super) market_t0: &'a MarketContext,
    pub(super) market_t1: &'a MarketContext,
    pub(super) val_t0: &'a ValuationResult,
    pub(super) time_period_days: f64,
    pub(super) ccy: Currency,
    pub(super) market_deps: MarketDependencies,
    pub(super) rates_curve_ids: Vec<CurveId>,
    pub(super) rates_curves: Vec<(CurveId, RateCurveFamily)>,
    pub(super) rate_keyrates: finstack_quant_core::FxHashMap<CurveId, Vec<(f64, f64, f64)>>,
    pub(super) credit_keyrates: finstack_quant_core::FxHashMap<CurveId, Vec<CreditKeyRateBucket>>,
    pub(super) hazard_keyrates: finstack_quant_core::FxHashMap<CurveId, Vec<CreditKeyRateBucket>>,
    pub(super) shifts: MarketShifts,
}

impl<'a> MetricsContext<'a> {
    /// Factor P&L in the attribution currency. A non-finite `amount` is noted
    /// on `attribution`, sets `invalid`, and yields zero.
    pub(super) fn money(
        &self,
        amount: f64,
        label: &str,
        attribution: &mut crate::PnlAttribution,
        invalid: &mut bool,
    ) -> finstack_quant_core::money::Money {
        crate::helpers::factor_money_or_invalid(
            amount,
            self.ccy,
            label,
            &mut attribution.meta.notes,
            invalid,
        )
    }

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

        // Discount first, then forward; first occurrence wins so a curve that
        // is both discount and projection is measured once, while preserving
        // its explicit RateCurveFamily (no cross-family fallback).
        let total_rate_len =
            market_deps.curves.discount_curves.len() + market_deps.curves.forward_curves.len();
        let mut rates_curve_ids: Vec<CurveId> = Vec::with_capacity(total_rate_len);
        let mut rates_curves: Vec<(CurveId, RateCurveFamily)> = Vec::with_capacity(total_rate_len);
        for (id, family) in market_deps
            .curves
            .discount_curves
            .iter()
            .map(|id| (id, RateCurveFamily::Discount))
            .chain(
                market_deps
                    .curves
                    .forward_curves
                    .iter()
                    .map(|id| (id, RateCurveFamily::Forward)),
            )
        {
            if !market_deps.curves.credit_curves.contains(id) && !rates_curve_ids.contains(id) {
                rates_curve_ids.push(id.clone());
                rates_curves.push((id.clone(), family));
            }
        }
        let mut rate_shifts = Vec::with_capacity(rates_curves.len());
        for (id, family) in &rates_curves {
            if let Some(shift) = crate::helpers::measured_unless_missing(
                measure_rate_curve_shift_bp(id.as_str(), *family, market_t0, market_t1),
            )? {
                rate_shifts.push(shift);
            }
        }
        let (avg_rate_shift_bp, rate_curves_measured) = average_over(rate_shifts, Some);
        let mut rate_keyrates =
            extract_keyrate_per_curve(&val_t0.measures, &rates_curve_ids, "bucketed_dv01")?;
        let mut paired_rate_keyrates = finstack_quant_core::FxHashMap::default();
        for (curve_id, buckets) in rate_keyrates.drain() {
            let (_, family) = rates_curves
                .iter()
                .find(|(id, _)| id == &curve_id)
                .ok_or_else(|| Error::Internal("missing declared rate curve family".into()))?;
            let tenors: Vec<_> = buckets.iter().map(|(tenor, _)| *tenor).collect();
            let moves = match measure_per_tenor_rate_shift(
                curve_id.as_str(),
                *family,
                market_t0,
                market_t1,
                &tenors,
            ) {
                Ok(moves) if moves.iter().all(|movement| movement.is_finite()) => moves,
                Ok(_)
                | Err(Error::Input(
                    InputError::MissingCurve { .. } | InputError::NotFound { .. },
                )) => {
                    return Err(Error::Validation(format!(
                        "bucketed_dv01 for '{curve_id}' has no complete measurable curve movement"
                    )));
                }
                Err(error) => return Err(error),
            };
            paired_rate_keyrates.insert(
                curve_id,
                buckets
                    .into_iter()
                    .zip(moves)
                    .map(|((tenor, sensitivity), movement)| (tenor, sensitivity, movement))
                    .collect(),
            );
        }
        let credit_keyrates = extract_credit_keyrates(
            &val_t0.measures,
            &market_deps.curves.credit_curves,
            market_t0,
            market_t1,
            &finstack_quant_calibration::recalibration::CachedRecalibrationProvider::default(),
        )?;
        let hazard_keyrates = extract_hazard_intensity_keyrates(
            &val_t0.measures,
            &market_deps.curves.credit_curves,
            market_t0,
            market_t1,
        )?;
        let require_par_spread = val_t0
            .measures
            .contains_key(finstack_quant_valuations::metrics::MetricId::Cs01.as_str())
            || !credit_keyrates.is_empty()
            || val_t0
                .measures
                .contains_key(finstack_quant_valuations::metrics::MetricId::CsGamma.as_str())
            || val_t0.measures.contains_key(
                finstack_quant_valuations::metrics::MetricId::CrossGammaRatesCredit.as_str(),
            )
            || val_t0.measures.contains_key(
                finstack_quant_valuations::metrics::MetricId::CrossGammaSpotCredit.as_str(),
            )
            || val_t0.measures.contains_key(
                finstack_quant_valuations::metrics::MetricId::CrossGammaCreditVol.as_str(),
            );
        let require_hazard_intensity = val_t0
            .measures
            .contains_key(finstack_quant_valuations::metrics::MetricId::Hazard01.as_str());
        let (avg_credit_shift_bp, credit_curves_measured) =
            average_credit(&market_deps, market_t0, market_t1, require_par_spread)?;
        let (avg_hazard_shift_bp, hazard_curves_measured) =
            average_hazard_intensity(&market_deps, market_t0, market_t1, require_hazard_intensity)?;
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
        let fx_shift_pct = match instrument.fx_exposure() {
            Some((base_currency, quote_currency)) => crate::helpers::measured_unless_missing(
                measure_fx_quote_diff(
                    &FxQuoteDiffParams {
                        base: base_currency,
                        quote: quote_currency,
                        date_t0: as_of_t0,
                        policy_t0: FxConversionPolicy::CashflowDate,
                        date_t1: as_of_t1,
                        policy_t1: FxConversionPolicy::CashflowDate,
                        scale: ProportionalDiffScale::Percent,
                    },
                    market_t0,
                    market_t1,
                )
                .map(|m| m.diff),
            )?,
            None => None,
        };
        let avg_spot_shift_pct = average_spot(&market_deps, market_t0, market_t1)?;
        Ok(Self {
            instrument,
            market_t0,
            market_t1,
            val_t0,
            time_period_days: (as_of_t1 - as_of_t0).whole_days() as f64,
            ccy: val_t1.value.currency(),
            market_deps,
            rates_curve_ids,
            rates_curves,
            rate_keyrates: paired_rate_keyrates,
            credit_keyrates,
            hazard_keyrates,
            shifts: MarketShifts {
                avg_rate_shift_bp,
                rate_curves_measured,
                avg_credit_shift_bp,
                credit_curves_measured,
                avg_hazard_shift_bp,
                hazard_curves_measured,
                avg_vol_shift_abs,
                vol_shift_error,
                fx_shift_pct,
                avg_spot_shift_pct,
            },
        })
    }
}

fn average_credit(
    deps: &MarketDependencies,
    t0: &MarketContext,
    t1: &MarketContext,
    require_par_spread: bool,
) -> finstack_quant_core::Result<(Option<f64>, usize)> {
    let mut shifts = Vec::with_capacity(deps.curves.credit_curves.len());
    for id in &deps.curves.credit_curves {
        if !require_par_spread
            && let Ok(h0) = t0.get_hazard(id.as_str())
            && h0.hazard_calibration().is_none()
        {
            continue;
        }
        if let Some(shift) = crate::helpers::measured_unless_missing(
            measure_credit_curve_shift_bp(id.as_str(), t0, t1),
        )? {
            shifts.push(shift);
        }
    }
    Ok(average_over(shifts, Some))
}

fn average_hazard_intensity(
    deps: &MarketDependencies,
    t0: &MarketContext,
    t1: &MarketContext,
    require_hazard_intensity: bool,
) -> finstack_quant_core::Result<(Option<f64>, usize)> {
    let mut shifts = Vec::with_capacity(deps.curves.credit_curves.len());
    for id in &deps.curves.credit_curves {
        if t0.get_hazard(id.as_str()).is_err() && t0.get_discount(id.as_str()).is_ok() {
            continue;
        }
        if !require_hazard_intensity
            && let (Ok(h0), Ok(h1)) = (t0.get_hazard(id.as_str()), t1.get_hazard(id.as_str()))
            && (h0.source_base_date() != h1.source_base_date() || h0.day_count() != h1.day_count())
        {
            continue;
        }
        if let Some(shift) = crate::helpers::measured_unless_missing(
            measure_hazard_intensity_shift_bp(id.as_str(), t0, t1),
        )? {
            shifts.push(shift);
        }
    }
    Ok(average_over(shifts, Some))
}

fn average_spot(
    deps: &MarketDependencies,
    t0: &MarketContext,
    t1: &MarketContext,
) -> Result<Option<f64>> {
    let Some(id) = deps.market_scalar_ids.first() else {
        return Ok(None);
    };
    crate::helpers::measured_unless_missing(measure_scalar_proportional_diff(
        id,
        id,
        t0,
        t1,
        ProportionalDiffScale::Percent,
    ))
}

fn volatility_shift(
    instrument: &dyn Instrument,
    deps: &MarketDependencies,
    t0: &MarketContext,
    t1: &MarketContext,
    as_of_t0: Date,
) -> Result<Option<f64>> {
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
            let closing_surface = t1.get_surface(id.as_str())?;
            if let Some((expiry, strike)) = instrument.expiry().zip(dependency.reference_strike)
                && surface.secondary_axis()
                    == finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike
            {
                let (opening, closing) = crate::helpers::volatility_expiry_times(
                    &surface,
                    &closing_surface,
                    expiry,
                    as_of_t0,
                )?;
                measure_vol_surface_shift(id.as_str(), t0, t1, opening, closing, strike)?
            } else {
                // Aggregate Vega without a contractual reference requires a parallel move.
                let mut uniform: Option<f64> = None;
                for &tenor in surface.expiries().iter().chain(closing_surface.expiries()) {
                    for &strike in surface
                        .secondary_values()
                        .iter()
                        .chain(closing_surface.secondary_values())
                    {
                        let movement =
                            measure_vol_surface_shift(id.as_str(), t0, t1, tenor, tenor, strike)?;
                        if uniform.is_some_and(|first| (first - movement).abs() > 1e-11) {
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
            measure_scalar_native_amount_diff(id.as_str(), t0, t1)?
        } else {
            continue;
        };
        // The reported Vega is cash per 0.01 native sigma for both scalars and surfaces.
        let shift = 100.0 * shift;
        if !shift.is_finite() {
            return Err(Error::Validation(format!(
                "volatility shift overflowed for '{id}'"
            )));
        }
        if result.is_some_and(|first| (first - shift).abs() > 1e-9) {
            return Err(finstack_quant_core::Error::Validation(
                "Aggregate Vega cannot distinguish different moves on multiple volatility sources; supply per-source risk".into()
            ));
        }
        result = Some(shift);
    }
    Ok(result)
}
