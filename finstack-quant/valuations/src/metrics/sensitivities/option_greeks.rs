//! Generic option greeks metric adapters.
//!
//! These calculators eliminate the per-instrument explosion of:
//! `metrics/{delta,gamma,vega,rho,theta,vanna,volga}.rs`
//! by delegating to the consolidated [`OptionGreeksProvider`] trait.

use std::marker::PhantomData;

use crate::instruments::common_impl::traits::{
    Instrument, OptionGreekKind, OptionGreeks, OptionGreeksProvider, OptionGreeksRequest,
};
use crate::metrics::{metric_not_found, MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;

fn extract_delta(greeks: OptionGreeks) -> Option<f64> {
    greeks.delta
}

fn extract_gamma(greeks: OptionGreeks) -> Option<f64> {
    greeks.gamma
}

fn extract_vega(greeks: OptionGreeks) -> Option<f64> {
    greeks.vega
}

fn extract_theta(greeks: OptionGreeks) -> Option<f64> {
    greeks.theta
}

fn extract_rho(greeks: OptionGreeks) -> Option<f64> {
    greeks.rho_bp
}

fn extract_foreign_rho(greeks: OptionGreeks) -> Option<f64> {
    greeks.foreign_rho_bp
}

fn extract_vanna(greeks: OptionGreeks) -> Option<f64> {
    greeks.vanna
}

fn extract_volga(greeks: OptionGreeks) -> Option<f64> {
    greeks.volga
}

pub(crate) struct OptionGreekCalculator<I> {
    kind: OptionGreekKind,
    metric_id: MetricId,
    base_pv: fn(&MetricContext) -> Option<f64>,
    extract: fn(OptionGreeks) -> Option<f64>,
    _phantom: PhantomData<I>,
}

impl<I> OptionGreekCalculator<I> {
    fn new(
        kind: OptionGreekKind,
        metric_id: MetricId,
        base_pv: fn(&MetricContext) -> Option<f64>,
        extract: fn(OptionGreeks) -> Option<f64>,
    ) -> Self {
        Self {
            kind,
            metric_id,
            base_pv,
            extract,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn delta() -> Self {
        Self::new(
            OptionGreekKind::Delta,
            MetricId::Delta,
            |_| None,
            extract_delta,
        )
    }

    pub(crate) fn gamma() -> Self {
        Self::new(
            OptionGreekKind::Gamma,
            MetricId::Gamma,
            |_| None,
            extract_gamma,
        )
    }

    pub(crate) fn vega() -> Self {
        Self::new(
            OptionGreekKind::Vega,
            MetricId::Vega,
            |_| None,
            extract_vega,
        )
    }

    pub(crate) fn theta() -> Self {
        Self::new(
            OptionGreekKind::Theta,
            MetricId::Theta,
            |_| None,
            extract_theta,
        )
    }

    pub(crate) fn rho() -> Self {
        Self::new(OptionGreekKind::Rho, MetricId::Rho, |_| None, extract_rho)
    }

    pub(crate) fn foreign_rho() -> Self {
        Self::new(
            OptionGreekKind::ForeignRho,
            MetricId::ForeignRho,
            |_| None,
            extract_foreign_rho,
        )
    }

    pub(crate) fn vanna() -> Self {
        Self::new(
            OptionGreekKind::Vanna,
            MetricId::Vanna,
            |_| None,
            extract_vanna,
        )
    }

    pub(crate) fn volga() -> Self {
        Self::new(
            OptionGreekKind::Volga,
            MetricId::Volga,
            |context| Some(context.base_value.amount()),
            extract_volga,
        )
    }
}

impl<I> MetricCalculator for OptionGreekCalculator<I>
where
    I: Instrument + OptionGreeksProvider + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        if let Some(value) = context.computed.get(&self.metric_id) {
            return Ok(*value);
        }

        let base_pv = (self.base_pv)(context);
        let inst: &I = context.instrument_as()?;
        let greeks = inst.option_greeks(
            &context.curves,
            context.as_of,
            &OptionGreeksRequest {
                greek: self.kind,
                base_pv,
            },
        )?;
        store_available_greeks(context, greeks);
        (self.extract)(greeks).ok_or_else(|| metric_not_found(self.metric_id.clone()))
    }
}

fn store_available_greeks(context: &mut MetricContext, greeks: OptionGreeks) {
    for (metric, value) in [
        (MetricId::Delta, greeks.delta),
        (MetricId::Gamma, greeks.gamma),
        (MetricId::Vega, greeks.vega),
        (MetricId::Theta, greeks.theta),
        (MetricId::Rho, greeks.rho_bp),
        (MetricId::ForeignRho, greeks.foreign_rho_bp),
        (MetricId::Vanna, greeks.vanna),
        (MetricId::Volga, greeks.volga),
    ] {
        if let Some(value) = value {
            context.computed.entry(metric).or_insert(value);
        }
    }
}
