//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::metrics::effective::{
    option_risk_bond_and_base_price, option_risk_curve_id,
};
use crate::instruments::Bond;
use crate::instruments::BondRiskBasis;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::market_data::context::BumpSpec;
use finstack_quant_core::types::CurveId;

/// Calculates option-aware bond DV01.
///
/// Callable/putable bonds with market price quotes must not reprice bumped
/// scenarios from the fixed clean price. Convert the quote into the equivalent
/// constant-OAS model input, then bump the tree curve and reprice.
pub(crate) struct BondDv01Calculator;

impl MetricCalculator for BondDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::DurationMod, MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let has_options = bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        let basis = super::bond_risk_basis(context);

        if basis != BondRiskBasis::CallableOas {
            if !bond
                .instrument_pricing_overrides
                .market_quotes
                .has_price_driver()
                && !has_options
            {
                return crate::metrics::UnifiedDv01Calculator::<Bond>::new(
                    crate::metrics::Dv01CalculatorConfig::parallel_combined(),
                )
                .calculate(context);
            }

            // Default (Workout) basis: a callable/putable bond is handled by
            // the yield-basis DV01, the dollar analogue of `DurationMod`. This
            // keeps it consistent with `DurationMod`, `Convexity` and
            // `YieldDv01` on this basis (they all use the quoted-yield /
            // workout convention), and — when a market price is quoted — the
            // workout path feeding `DurationMod` captures the embedded option.
            //
            // The previous behaviour cloned the bond with `call_put = None`
            // and curve-bumped that bullet, which silently discarded the
            // option and reported a curve sensitivity inconsistent in both
            // methodology and units with the rest of the Workout-basis family.
            let duration_mod = context
                .computed
                .get(&MetricId::DurationMod)
                .copied()
                .ok_or_else(|| crate::metrics::metric_not_found(MetricId::DurationMod))?;
            let ytm = context
                .computed
                .get(&MetricId::Ytm)
                .copied()
                .ok_or_else(|| crate::metrics::metric_not_found(MetricId::Ytm))?;
            return super::yield_dv01::yield_basis_dv01(bond, context, duration_mod, ytm);
        }

        if !has_options
            || !bond
                .instrument_pricing_overrides
                .market_quotes
                .has_price_driver()
        {
            return crate::metrics::UnifiedDv01Calculator::<Bond>::new(
                crate::metrics::Dv01CalculatorConfig::parallel_combined(),
            )
            .calculate(context);
        }

        let (risk_bond, _) =
            option_risk_bond_and_base_price(bond, context.curves.as_ref(), context.as_of)?;
        let curve_id = option_risk_curve_id(&risk_bond);

        curve_bump_dv01(&risk_bond, context, &curve_id)
    }
}

fn curve_bump_dv01(
    bond: &Bond,
    context: &MetricContext,
    curve_id: &CurveId,
) -> finstack_quant_core::Result<f64> {
    let defaults = crate::metrics::sensitivities::config::from_context_or_default(
        context.config(),
        context.get_metric_overrides(),
    )?;
    let bump_bp = defaults.rate_bump_bp;
    if bump_bp.abs() <= f64::EPSILON {
        return Ok(0.0);
    }

    let market_up = context.curves.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(bump_bp),
    }])?;
    let market_down = context.curves.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(-bump_bp),
    }])?;

    let pv_up = bond.value_raw(&market_up, context.as_of)?;
    let pv_down = bond.value_raw(&market_down, context.as_of)?;
    Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
}

/// Bucketed DV01 calculator for bonds that handles the case where a market price
/// quote (e.g. `quoted_clean_price`) is set.
///
/// When no price driver is present the behaviour is identical to
/// `UnifiedDv01Calculator` with `triangular_key_rate` config. When a price
/// driver is present the bond is first converted to a spread-pinned clone
/// (Z-spread for plain bonds, OAS for callables, hazard-shifted for credit)
/// so that curve bumps in the key-rate loop move the PV rather than returning
/// the fixed quoted price.
pub(crate) struct BondBucketedDv01Calculator;

impl MetricCalculator for BondBucketedDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        super::risk_view::with_bond_risk_view(context, |ctx| {
            crate::metrics::UnifiedDv01Calculator::<Bond>::new(
                crate::metrics::Dv01CalculatorConfig::triangular_key_rate(),
            )
            .calculate(ctx)
        })
    }
}
