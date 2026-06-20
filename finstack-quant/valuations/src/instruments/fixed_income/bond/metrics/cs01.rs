//! Bond-specific CS01 calculators with z-spread fallback.
//!
//! When a bond has an associated credit (hazard) curve, CS01 follows the
//! [canonical CS01 convention][canonical] — a parallel 1 bp shock to par CDS
//! spreads with a symmetric (central) finite difference — by delegating to
//! [`GenericParallelCs01`] / [`GenericBucketedCs01`].
//!
//! When **no credit curve is configured**, no par CDS curve is available to
//! bump, so CS01 falls back to the market-standard z-spread bump for vanilla
//! bonds:
//!
//! ```text
//! CS01_fallback = PV(z + 1bp) - PV(z)        (forward difference)
//! ```
//!
//! where `PV(z) = Σ CF_i · DF_i · exp(-z · t_i)`. This is a deliberate
//! deviation from the canonical methodology in two respects, called out here
//! so consumers can audit the regime:
//!
//! 1. The shock is applied to the bond's z-spread rather than to par CDS
//!    spreads (no hazard curve exists to re-bootstrap).
//! 2. A forward difference is used in place of the canonical central
//!    difference `(PV(z+1bp) − PV(z−1bp)) / 2`. The two agree to
//!    `O(bump²) ≈ 10⁻⁸` of CS01 magnitude for a 1 bp shock; the forward
//!    form is preserved for deterministic golden parity.
//!
//! # Settlement-anchored repricing
//!
//! The bumped and base PVs are obtained from
//! [`price_from_z_spread`], which discounts on the same settlement
//! (`quote_date`) time axis and with the same compounding-aware spread shift
//! that [`ZSpreadCalculator`] used to *solve* the z-spread. This guarantees
//! `PV(z)` equals the dirty price the z-spread was calibrated to, so the
//! finite difference `PV(z+1bp) − PV(z)` is taken around the correct point on
//! the correct curve — even when the discount curve's base date differs from
//! the valuation date or the bond has a non-zero settlement lag.
//!
//! Sign convention is identical to the canonical reference:
//! - Long bond → CS01 negative (wider spreads reduce PV).
//! - Short bond → CS01 positive.
//!
//! [canonical]: crate::metrics::sensitivities::cs01
//! [`price_from_z_spread`]: crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread
//! [`ZSpreadCalculator`]: crate::instruments::fixed_income::bond::ZSpreadCalculator

use super::risk_view::with_bond_risk_view;
use crate::constants::ONE_BASIS_POINT;
use crate::instruments::common_impl::traits::{CurveDependencies, Instrument};
use crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

/// Bond parallel CS01 with z-spread fallback.
///
/// Delegates to [`GenericParallelCs01`] when the bond references a credit
/// curve; otherwise computes CS01 by bumping the z-spread by 1 bp.
/// The result is keyed by credit curve ID or instrument ID.
pub(crate) struct BondCs01Calculator;

impl MetricCalculator for BondCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let has_credit = {
            let bond: &Bond = context.instrument_as()?;
            !bond.curve_dependencies()?.credit_curves.is_empty()
        };

        if has_credit {
            return with_bond_risk_view(context, |ctx| {
                crate::metrics::GenericParallelCs01::<Bond>::default().calculate(ctx)
            });
        }

        let bond: &Bond = context.instrument_as()?;
        let inst_id = bond.id();

        let base_spread = context
            .computed
            .get(&MetricId::ZSpread)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:ZSpread".to_string(),
                })
            })?;

        let bumped_spread = base_spread + ONE_BASIS_POINT;

        let base_npv = price_from_z_spread(bond, &context.curves, context.as_of, base_spread)?;
        let bumped_npv = price_from_z_spread(bond, &context.curves, context.as_of, bumped_spread)?;

        let cs01 = bumped_npv - base_npv;

        context
            .computed
            .insert(MetricId::custom(format!("cs01::{}", inst_id)), cs01);

        Ok(cs01)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }
}

/// Bond bucketed CS01 with z-spread fallback.
///
/// Delegates to [`GenericBucketedCs01`] when the bond references a credit
/// curve; otherwise returns the parallel z-spread CS01 keyed by instrument ID.
pub(crate) struct BondBucketedCs01Calculator;

impl MetricCalculator for BondBucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let has_credit = {
            let bond: &Bond = context.instrument_as()?;
            !bond.curve_dependencies()?.credit_curves.is_empty()
        };

        if has_credit {
            return with_bond_risk_view(context, |ctx| {
                crate::metrics::GenericBucketedCs01::<Bond>::default().calculate(ctx)
            });
        }

        let cs01 = context
            .computed
            .get(&MetricId::Cs01)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Cs01".to_string(),
                })
            })?;

        Ok(cs01)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Cs01]
    }
}
