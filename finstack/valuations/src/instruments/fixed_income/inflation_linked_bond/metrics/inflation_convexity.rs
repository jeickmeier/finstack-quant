//! Inflation convexity calculator for inflation-linked bonds.
//!
//! Calculates the (dimensionless) second derivative of the bond value with
//! respect to parallel inflation curve shifts, normalized by the base PV —
//! consistent with the bond `convexity.rs` definition `Convexity = (1/P) · d²P/dy²`.
//!
//! Uses numerical differentiation with 1bp bumps to the inflation curve.

use crate::constants::numerical::ZERO_TOLERANCE;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_core::Result;

/// Standard inflation curve bump: 1bp (0.0001)
const INFLATION_BUMP_BP: f64 = 0.0001;

/// Calculates inflation convexity for inflation-linked bonds.
pub(crate) struct InflationConvexityCalculator;

impl MetricCalculator for InflationConvexityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &InflationLinkedBond = context.instrument_as()?;
        let as_of = context.as_of;

        // Get base value
        let base_pv = context.base_value.amount();

        // Bump size: 1bp for numerical convexity
        let bump_bp = INFLATION_BUMP_BP;

        // Get the inflation index/curve ID
        let inflation_curve_id = &bond.inflation_index_id;

        // Create bumped curves (up)
        let bump_spec_up = BumpSpec::inflation_shift_pct(bump_bp * 100.0); // Convert bp to percent
        let curves_up = context.curves.bump([MarketBump::Curve {
            id: inflation_curve_id.clone(),
            spec: bump_spec_up,
        }])?;
        let pv_up = bond.value(&curves_up, as_of)?.amount();

        // Create bumped curves (down)
        let bump_spec_down = BumpSpec::inflation_shift_pct(-bump_bp * 100.0);
        let curves_down = context.curves.bump([MarketBump::Curve {
            id: inflation_curve_id.clone(),
            spec: bump_spec_down,
        }])?;
        let pv_down = bond.value(&curves_down, as_of)?.amount();

        if base_pv.abs() < ZERO_TOLERANCE {
            return Ok(0.0);
        }

        // InflationConvexity = (PV_up + PV_down - 2×PV_base) / (bump² × PV_base)
        // Normalizing by PV_base yields a dimensionless convexity, consistent with
        // the bond `convexity.rs` definition `Convexity = (1/P) · d²P/dy²`.
        let inflation_convexity =
            (pv_up + pv_down - 2.0 * base_pv) / (bump_bp * bump_bp) / base_pv;

        Ok(inflation_convexity)
    }
}

#[cfg(test)]
mod tests {
    use super::INFLATION_BUMP_BP;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
    use crate::instruments::{PricingOptions, PricingOverrides};
    use crate::metrics::MetricId;
    use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use finstack_core::types::CurveId;
    use time::macros::date;

    fn market(as_of: finstack_core::dates::Date) -> MarketContext {
        let discount = DiscountCurve::builder("US-CPI-DISC")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 0.7)])
            .build()
            .expect("discount curve");
        let inflation = InflationCurve::builder("US-CPI")
            .base_date(as_of)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (5.0, 112.0), (10.0, 125.0)])
            .build()
            .expect("inflation curve");
        MarketContext::new().insert(discount).insert(inflation)
    }

    fn sample_bond() -> InflationLinkedBond {
        let mut bond = InflationLinkedBond::example();
        // `example()` uses ActActIsma which needs an explicit period frequency;
        // use a self-contained day count for the metric test.
        bond.day_count = finstack_core::dates::DayCount::Thirty360;
        bond.discount_curve_id = CurveId::new("US-CPI-DISC");
        bond.inflation_index_id = CurveId::new("US-CPI");
        bond
    }

    /// W-28(b): the reported `InflationConvexity` is dimensionless — it equals the
    /// numerical second derivative divided by `bump²` AND by the base PV, matching
    /// the bond `convexity.rs` definition rather than a currency-per-bp² figure.
    #[test]
    fn inflation_convexity_is_dimensionless() {
        let as_of = date!(2024 - 01 - 15);
        let bond = sample_bond();
        let market = market(as_of);

        let base_pv = bond.value(&market, as_of).expect("base pv").amount();

        let bump = INFLATION_BUMP_BP;
        let curves_up = market
            .bump([MarketBump::Curve {
                id: bond.inflation_index_id.clone(),
                spec: BumpSpec::inflation_shift_pct(bump * 100.0),
            }])
            .expect("bump up");
        let curves_down = market
            .bump([MarketBump::Curve {
                id: bond.inflation_index_id.clone(),
                spec: BumpSpec::inflation_shift_pct(-bump * 100.0),
            }])
            .expect("bump down");
        let pv_up = bond.value(&curves_up, as_of).expect("pv up").amount();
        let pv_down = bond.value(&curves_down, as_of).expect("pv down").amount();

        // Dimensionless reference (matches bond convexity: (1/P)·d²P/dy²).
        let expected = (pv_up + pv_down - 2.0 * base_pv) / (bump * bump) / base_pv;
        // The buggy currency-per-bp² figure (no /base_pv) would be ~base_pv× larger.
        let buggy = (pv_up + pv_down - 2.0 * base_pv) / (bump * bump);

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::InflationConvexity],
                PricingOptions::default(),
            )
            .expect("price with metrics");
        let reported = *result
            .measures
            .get("inflation_convexity")
            .expect("inflation_convexity measure");

        assert!(
            (reported - expected).abs() < 1e-6 * expected.abs().max(1.0),
            "reported {reported} should match dimensionless {expected}"
        );
        assert!(
            (reported - buggy).abs() > 1.0,
            "reported {reported} must NOT equal the currency-per-bp² figure {buggy}"
        );
    }

    /// W-28(a): a near-zero base PV is handled via a tolerance, not an exact
    /// `== 0.0` compare. A bond whose PV is a tiny non-zero value still returns 0.0.
    #[test]
    fn inflation_convexity_near_zero_base_pv_uses_tolerance() {
        let as_of = date!(2024 - 01 - 15);
        let mut bond = sample_bond();
        // Force a tiny non-zero base PV via a tiny notional.
        bond.notional = finstack_core::money::Money::new(1e-12, bond.notional.currency());
        bond.pricing_overrides = PricingOverrides::default();
        let market = market(as_of);

        let base_pv = bond.value(&market, as_of).expect("base pv").amount();
        assert!(
            base_pv.abs() < super::ZERO_TOLERANCE && base_pv.abs() > 0.0,
            "test precondition: base PV {base_pv} is tiny but non-zero"
        );

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::InflationConvexity],
                PricingOptions::default(),
            )
            .expect("price with metrics");
        let reported = *result
            .measures
            .get("inflation_convexity")
            .expect("inflation_convexity measure");

        assert_eq!(
            reported, 0.0,
            "near-zero base PV must be caught by the tolerance guard"
        );
    }
}
