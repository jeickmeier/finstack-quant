//! Generic option greeks metric adapters.
//!
//! These calculators eliminate the per-instrument explosion of:
//! `metrics/{delta,gamma,vega,rho,theta,vanna,volga}.rs`
//! by delegating to the consolidated [`OptionGreeksProvider`] trait.
//! Equity's native provider is used only for its Black-Scholes pricing model;
//! explicitly selected alternate models use the context's repricing dispatch.

use std::marker::PhantomData;
use std::sync::Arc;

use crate::instruments::{
    GreekBumps, Instrument, OptionGreekKind, OptionGreeks, OptionGreeksProvider,
    OptionGreeksRequest,
};
use crate::metrics::sensitivities::theta::{theta_period, GenericThetaAny};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::Result;

/// Hull-White 1F rate delta or gamma, when the request selected that model.
///
/// Returns `None` unless the metric request prices with
/// [`ModelKey::HullWhite1F`](crate::pricer::ModelKey::HullWhite1F), so the
/// caller falls through to its closed-form greek. Otherwise returns the
/// central finite-difference derivative of the selected model's raw PV to a
/// parallel shift of `curve_id`, in PV per unit decimal rate (first order) or
/// per unit decimal rate squared (second order). HW single-curve products
/// bump the discount curve; multi-curve caps bump their projection curve while
/// holding discounting fixed. Parameters are held or recalibrated exactly as
/// in the selected pricer's normal market-input resolution.
///
/// # Arguments
///
/// * `context` - Metric context whose pricing dispatch, market and bump
///   configuration (`rate_bump_bp`, in basis points) drive the repricing.
/// * `curve_id` - Curve shifted in parallel by ±`rate_bump_bp`.
/// * `second` - `false` for delta `(PV₊ − PV₋) / 2h`, `true` for gamma
///   `(PV₊ − 2·PV + PV₋) / h²`, with `h` the bump as a decimal rate.
///
/// # Errors
///
/// The inner result fails when the bump configuration cannot be resolved,
/// the curve cannot be bumped, or a repricing fails.
pub fn hw1f_rate_greek(
    context: &MetricContext,
    curve_id: &finstack_quant_core::types::CurveId,
    second: bool,
) -> Option<Result<f64>> {
    if context.pricing_model() != Some(crate::pricer::ModelKey::HullWhite1F) {
        return None;
    }
    Some(rate_greek(context, curve_id, second))
}

fn rate_greek(
    context: &MetricContext,
    curve_id: &finstack_quant_core::types::CurveId,
    second: bool,
) -> Result<f64> {
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    let config = super::config::resolve(context)?;
    let bump_bp = config.rate_bump_bp;
    let h = bump_bp * 1e-4;
    let price = |direction: f64| {
        let market = context.curves.bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp * direction),
        }])?;
        context.reprice_raw(&market, context.as_of)
    };
    let up = price(1.0)?;
    let down = price(-1.0)?;
    if second {
        let base = context.reprice_raw(&context.curves, context.as_of)?;
        Ok((up - 2.0 * base + down) / (h * h))
    } else {
        Ok((up - down) / (2.0 * h))
    }
}

pub struct OptionGreekCalculator<I> {
    kind: OptionGreekKind,
    _phantom: PhantomData<I>,
}

impl<I> OptionGreekCalculator<I> {
    fn new(kind: OptionGreekKind) -> Self {
        Self {
            kind,
            _phantom: PhantomData,
        }
    }

    pub fn delta() -> Self {
        Self::new(OptionGreekKind::Delta)
    }

    pub fn gamma() -> Self {
        Self::new(OptionGreekKind::Gamma)
    }

    pub fn vega() -> Self {
        Self::new(OptionGreekKind::Vega)
    }

    pub fn theta() -> Self {
        Self::new(OptionGreekKind::Theta)
    }

    pub fn rho() -> Self {
        Self::new(OptionGreekKind::Rho)
    }

    pub fn foreign_rho() -> Self {
        Self::new(OptionGreekKind::ForeignRho)
    }

    pub fn vanna() -> Self {
        Self::new(OptionGreekKind::Vanna)
    }

    pub fn volga() -> Self {
        Self::new(OptionGreekKind::Volga)
    }
}

impl<I> MetricCalculator for OptionGreekCalculator<I>
where
    I: Instrument + OptionGreeksProvider + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let metric_id = self.kind.metric_id();
        if let Some(value) = context.computed.get(&metric_id) {
            return Ok(*value);
        }
        // Analytic theta is a one-day number; other horizons need the
        // roll-forward theta that honors `theta_period`.
        if matches!(self.kind, OptionGreekKind::Theta) && theta_period(context) != Tenor::daily() {
            return GenericThetaAny.calculate(context);
        }

        let native_model = context.instrument.default_model();
        if context
            .pricing_model()
            .is_some_and(|model| model != native_model)
        {
            context.instrument_as::<I>()?;
            let instrument = Arc::clone(&context.instrument);
            if let Some(inst) = instrument.as_any().downcast_ref::<I>() {
                if let Some(value) = inst.selected_model_greek(self.kind, context)? {
                    context.computed.insert(metric_id, value);
                    return Ok(value);
                }
            }
        }

        let base_pv =
            matches!(self.kind, OptionGreekKind::Volga).then(|| context.base_value.amount());
        let bumps = GreekBumps::from(&super::config::resolve(context)?);
        let inst: &I = context.instrument_as()?;
        let greeks = inst.option_greeks(
            &context.curves,
            context.as_of,
            &OptionGreeksRequest {
                greek: self.kind,
                base_pv,
                bumps,
            },
        )?;
        store_available_greeks(context, self.kind, greeks);
        context.computed_metric(&metric_id)
    }
}

/// Cache every greek the provider returned so later metric requests reuse it.
///
/// Theta is cached only when it is the requested greek: asking for Delta must
/// not fill the theta breakdown that the generic theta calculator would
/// otherwise produce. Analytic theta is a one-day value (on the
/// `theta_day_basis` day basis) with no cashflow component, so it is reported
/// entirely as roll-down over a one-day period.
fn store_available_greeks(
    context: &mut MetricContext,
    requested_kind: OptionGreekKind,
    greeks: OptionGreeks,
) {
    for (metric, value) in [
        (MetricId::Delta, greeks.delta),
        (MetricId::Gamma, greeks.gamma),
        (MetricId::Vega, greeks.vega),
        (MetricId::Rho, greeks.rho_bp),
        (MetricId::ForeignRho, greeks.foreign_rho_bp),
        (MetricId::Vanna, greeks.vanna),
        (MetricId::Volga, greeks.volga),
    ] {
        if let Some(value) = value {
            context.computed.entry(metric).or_insert(value);
        }
    }
    if matches!(requested_kind, OptionGreekKind::Theta) {
        if let Some(theta) = greeks.theta {
            context.computed.entry(MetricId::Theta).or_insert(theta);
            context.computed.entry(MetricId::ThetaCarry).or_insert(0.0);
            context
                .computed
                .entry(MetricId::ThetaRollDown)
                .or_insert(theta);
            context
                .computed
                .entry(MetricId::ThetaPeriodDays)
                .or_insert(1.0);
        }
    }
}
