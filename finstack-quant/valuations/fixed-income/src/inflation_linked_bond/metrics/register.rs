//! Metric registration and calculators.

use super::Inflation01Calculator;
use super::inflation01::bumped_inflation_market;
use crate::inflation_linked_bond::InflationLinkedBond;
use finstack_quant_core::Result;
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::ZERO_TOLERANCE;
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::metrics::sensitivities::bump_config;

/// Register all ILB metrics with the registry
pub(crate) fn register_ilb_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::InflationLinkedBond,
        metrics: [
            (RealYield, RealYieldCalculator),
            (IndexRatio, IndexRatioCalculator),
            (RealDuration, RealDurationCalculator),
            (BreakevenInflation, BreakevenInflationCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            // Custom metric: projected inflation-rate sensitivity per 1bp.
            (Inflation01, Inflation01Calculator),
            // Custom metric: InflationConvexity (second-order inflation sensitivity)
            (InflationConvexity, InflationConvexityCalculator),
        ]
    };
    Ok(())
}

/// Breakeven inflation calculator for ILB.
///
/// Derives the nominal-yield leg of the Fisher identity from the bond's
/// `discount_curve_id`. That field is a **nominal** curve by contract (see the
/// `InflationLinkedBond::discount_curve_id` field docs), so the annually
/// compounded zero rate read off it at the bond's maturity is a genuine nominal
/// yield and the Fisher computation is correct. If a real curve is wired in
/// despite the contract, the breakeven collapses toward zero; callers needing a
/// different nominal-yield source should use
/// [`InflationLinkedBond::breakeven_inflation`] directly with an explicit nominal yield.
pub(crate) struct BreakevenInflationCalculator;

impl MetricCalculator for BreakevenInflationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        let curves = context.curves.as_ref();
        let disc_curve = curves.get_discount(ilb.discount_curve_id.as_str())?;

        let day_count = disc_curve.day_count();
        // Provide the coupon frequency and quasi-coupon grid reference so an
        // ACT/ACT (ISMA) discount-curve day count slices cleanly across periods;
        // ignored by other conventions.
        let t = if context.as_of < ilb.maturity {
            let ref_period = finstack_quant_cashflows::builder::icma_reference_period(
                day_count,
                ilb.frequency,
                [ilb.maturity],
                context.as_of,
            );
            finstack_quant_cashflows::builder::icma_year_fraction(
                day_count,
                Some(ilb.frequency),
                ref_period,
                context.as_of,
                ilb.maturity,
            )?
        } else {
            0.0
        };
        // Use annually-compounded (effective-annual) forward zero rate from
        // `as_of` to maturity so that the nominal yield passed to
        // `breakeven_inflation` is anchored at `as_of` on the same basis as
        // the real yield after its Street→Annual conversion inside that method.
        let nominal_yield = if t > 0.0 {
            let df = disc_curve.df_between_dates(context.as_of, ilb.maturity)?;
            df.powf(-1.0 / t) - 1.0
        } else {
            0.0
        };

        ilb.breakeven_inflation(nominal_yield, context.as_of)
    }
}

/// Index ratio calculator for ILB
pub(crate) struct IndexRatioCalculator;

impl MetricCalculator for IndexRatioCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        ilb.index_ratio_from_market(context.as_of, context.curves.as_ref(), context.as_of)
    }
}

/// Calculates inflation convexity for inflation-linked bonds.
///
/// Calculates the **raw dollar second derivative** `d²PV/dπ²` of the bond
/// value with respect to parallel inflation curve shifts (π in decimal) —
/// the `InflationConvexity` MetricId convention shared with the inflation
/// swap producer and consumed by P&L attribution as
/// `½ × InflationConvexity × (Δi_decimal)²` with no P₀ factor. The former
/// `(1/P)`-normalized figure understated ILB inflation convexity P&L by a
/// factor of ~P₀.
///
/// Uses numerical differentiation with 1bp bumps to the inflation curve.
pub(crate) struct InflationConvexityCalculator;

impl MetricCalculator for InflationConvexityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &InflationLinkedBond = context.instrument_as()?;
        let as_of = context.as_of;

        let base_pv = context.base_value.amount();

        // Bump size: 1bp (decimal 0.0001) for numerical convexity
        let bump = ONE_BASIS_POINT;

        let bump_spec_up = BumpSpec::inflation_shift_pct(bump * 100.0); // decimal -> percent
        let curves_up =
            bumped_inflation_market(context.curves.as_ref(), bond, as_of, bump_spec_up)?;
        let pv_up = bond.value(&curves_up, as_of)?.amount();

        let bump_spec_down = BumpSpec::inflation_shift_pct(-bump * 100.0);
        let curves_down =
            bumped_inflation_market(context.curves.as_ref(), bond, as_of, bump_spec_down)?;
        let pv_down = bond.value(&curves_down, as_of)?.amount();

        if base_pv.abs() < ZERO_TOLERANCE {
            return Ok(0.0);
        }

        // InflationConvexity = (PV_up + PV_down - 2×PV_base) / bump²
        // — the raw dollar second derivative d²PV/dπ² ($ per decimal²),
        // matching the inflation swap producer and the attribution consumer
        // (one MetricId = one unit).
        let inflation_convexity = (pv_up + pv_down - 2.0 * base_pv) / (bump * bump);

        Ok(inflation_convexity)
    }
}

/// Default real-yield shock for the central difference: 1bp.
const DEFAULT_YTM_BUMP_BP: f64 = 1.0;

/// Real duration calculator for ILB.
///
/// Shocks the real yield by `metric_pricing_overrides.bump_config.ytm_bump_bp`
/// when set, otherwise by 1bp.
pub(crate) struct RealDurationCalculator;

impl MetricCalculator for RealDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bump_bp = bump_config::resolve(context)?
            .ytm_bump_bp
            .unwrap_or(DEFAULT_YTM_BUMP_BP);
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        ilb.real_duration(context.as_of, bump_bp)
    }
}

/// Real yield calculator for ILB
pub(crate) struct RealYieldCalculator;

impl MetricCalculator for RealYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        let clean_price = ilb.quoted_clean_price_pct("Real yield")?;
        ilb.real_yield(clean_price, context.as_of)
    }
}
