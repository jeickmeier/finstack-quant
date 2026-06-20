use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::metrics::effective::{
    option_risk_bond_and_base_price, option_risk_curve_id,
};
use crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides;
use crate::instruments::Bond;
use crate::instruments::BondRiskBasis;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::market_data::context::BumpSpec;
use finstack_quant_core::types::CurveId;
use std::sync::Arc;

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
            if !bond.pricing_overrides.market_quotes.has_price_driver() && !has_options {
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

        if !has_options || !bond.pricing_overrides.market_quotes.has_price_driver() {
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

/// Build a bond clone whose price is pinned by a calibrated spread (OAS for
/// callables, Z-spread for plain bonds) rather than the raw quoted price, so a
/// curve bump moves the PV. Returns the calibrated clone; the caller swaps it
/// into the metric context for the bump-and-reprice loop.
///
/// The plain-bond branch reads the already-computed `MetricId::ZSpread` (which,
/// for a quoted bond, is the spread that reproduces the quote), so callers MUST
/// list `MetricId::ZSpread` in their `dependencies()`.
pub(crate) fn price_driven_risk_bond(
    context: &MetricContext,
    has_options: bool,
) -> finstack_quant_core::Result<Bond> {
    let bond: &Bond = context.instrument_as()?;
    if has_options {
        let (risk_bond, _) =
            option_risk_bond_and_base_price(bond, context.curves.as_ref(), context.as_of)?;
        return Ok(risk_bond);
    }
    let z = context
        .computed
        .get(&MetricId::ZSpread)
        .copied()
        .ok_or_else(|| crate::metrics::metric_not_found(MetricId::ZSpread))?;
    let mut risk_bond = bond.clone();
    clear_price_driving_overrides(&mut risk_bond);
    risk_bond.pricing_overrides.market_quotes.quoted_z_spread = Some(z);
    Ok(risk_bond)
}

/// Bucketed DV01 calculator for bonds that handles the case where a market price
/// quote (e.g. `quoted_clean_price`) is set.
///
/// When no price driver is present the behaviour is identical to
/// `UnifiedDv01Calculator` with `triangular_key_rate` config. When a price
/// driver is present the bond is first converted to a spread-pinned clone
/// (Z-spread for plain bonds, OAS for callables) so that curve bumps in the
/// key-rate loop move the PV rather than returning the fixed quoted price.
pub(crate) struct BondBucketedDv01Calculator;

impl MetricCalculator for BondBucketedDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let (has_driver, has_options) = {
            let bond: &Bond = context.instrument_as()?;
            (
                bond.pricing_overrides.market_quotes.has_price_driver(),
                bond.call_put.as_ref().is_some_and(|cp| cp.has_options()),
            )
        };
        // No price driver → unchanged behavior (curve already responds to bumps).
        if !has_driver {
            return crate::metrics::UnifiedDv01Calculator::<Bond>::new(
                crate::metrics::Dv01CalculatorConfig::triangular_key_rate(),
            )
            .calculate(context);
        }
        // Price-pinned bond: bump-and-reprice a spread-pinned clone instead.
        let risk_bond = price_driven_risk_bond(context, has_options)?;
        let original = Arc::clone(&context.instrument);
        context.instrument =
            Arc::new(risk_bond) as Arc<dyn crate::instruments::common_impl::traits::Instrument>;
        let result = crate::metrics::UnifiedDv01Calculator::<Bond>::new(
            crate::metrics::Dv01CalculatorConfig::triangular_key_rate(),
        )
        .calculate(context);
        context.instrument = original;
        result
    }
}
