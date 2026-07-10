//! Inflation01 calculator for inflation-linked bonds.
//!
//! Computes inflation sensitivity using finite differences.
//! Inflation01 measures the change in PV for a 1 basis point (0.0001) shift in the
//! inflation curve — analogous to DV01 and CS01 elsewhere in the codebase.
//!
//! # Methodology Note
//!
//! The bump is applied as a **uniform multiplicative CPI level scale** across all
//! maturities via `BumpSpec::inflation_shift_pct`. This is **not** equivalent to a
//! parallel 1bp shift in zero-coupon inflation rates (which would produce
//! `I(t) → I(t) × exp(Δπ × t)`). For most practical purposes the difference is
//! small for near-term cashflows but can diverge for long-dated linkers.
//!
//! # Formula
//!
//! A symmetric ±1bp central bump yields the per-1bp PV change directly:
//!
//! ```text
//! Inflation01 = (PV(inflation_curve + 1bp) − PV(inflation_curve − 1bp)) / 2
//! ```
//!
//! Dividing by 2 (not by 2 × bump_size) produces the PV change for a **1bp move**,
//! consistent with the DV01/CS01 convention throughout the workspace.
//!
//! # Note
//! For bonds backed by inflation indices, this bumps the underlying inflation curve
//! (which drives projected CPI). For index-based sources, we bump the curve that's
//! implicitly constructed from the index.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::Result;

/// Standard inflation curve bump: 1bp (0.0001)
const INFLATION_BUMP_BP: f64 = 0.0001;

/// Inflation01 calculator for inflation-linked bonds.
pub(crate) struct Inflation01Calculator;

impl MetricCalculator for Inflation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &InflationLinkedBond = context.instrument_as()?;
        let as_of = context.as_of;
        let _base_pv = context.base_value.amount();

        // Check if we have an inflation curve (preferred) or index
        let inflation_curve_id = &bond.inflation_index_id;

        // Use MarketContext::bump() API to bump the inflation curve
        // Bump by 1bp using parallel shift
        let bump_spec = BumpSpec::inflation_shift_pct(INFLATION_BUMP_BP * 100.0); // Convert bp to percent
        let curves_up = context.curves.as_ref().bump([MarketBump::Curve {
            id: inflation_curve_id.clone(),
            spec: bump_spec,
        }])?;
        let pv_up = bond.value(&curves_up, as_of)?.amount();

        // Bump down
        let bump_spec_down = BumpSpec::inflation_shift_pct(-INFLATION_BUMP_BP * 100.0);
        let curves_down = context.curves.as_ref().bump([MarketBump::Curve {
            id: inflation_curve_id.clone(),
            spec: bump_spec_down,
        }])?;
        let pv_down = bond.value(&curves_down, as_of)?.amount();

        // Inflation01 = (PV_up − PV_down) / 2
        //
        // The curve was bumped by exactly ±1bp (INFLATION_BUMP_BP = 0.0001).
        // A symmetric ±1bp central difference already produces the PV change for
        // a 1bp move; no further normalization is needed. Dividing by 2 (not by
        // 2 × bump_size) is the correct per-1bp convention — consistent with
        // DV01 and CS01 in the workspace (see `sensitivity_central_diff` where
        // bump_bp is expressed in bp-units = 1.0, so the divisor is 2·1 = 2).
        let inflation01 = (pv_up - pv_down) / 2.0;

        Ok(inflation01)
    }
}

#[cfg(test)]
mod tests {
    use super::INFLATION_BUMP_BP;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
    use crate::instruments::PricingOptions;
    use crate::metrics::MetricId;
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use finstack_quant_core::types::CurveId;
    use time::macros::date;

    fn market(as_of: finstack_quant_core::dates::Date) -> MarketContext {
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
        bond.day_count = finstack_quant_core::dates::DayCount::Thirty360;
        bond.discount_curve_id = CurveId::new("US-CPI-DISC");
        bond.inflation_index_id = CurveId::new("US-CPI");
        bond
    }

    /// Inflation01 must equal `(pv_up - pv_down) / 2`, not `(pv_up - pv_down) / (2 * bump_size)`.
    ///
    /// The "…01" convention throughout the workspace (DV01, CS01, Inflation01) is
    /// PV-change-per-1bp. A ±1bp central bump produces that directly as
    /// `(pv_up - pv_down) / 2`. Dividing by the decimal bump value (0.0002) instead
    /// yields the per-*unit* derivative — 10 000× too large — which is the bug this
    /// test guards against.
    #[test]
    fn inflation01_is_per_1bp_not_per_unit() {
        let as_of = date!(2024 - 01 - 15);
        let bond = sample_bond();
        let market = market(as_of);

        // Compute the two bumped PVs the same way the calculator does.
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

        // Expected: per-1bp sensitivity — the half-difference.
        let expected_per_1bp = (pv_up - pv_down) / 2.0;
        // The old (buggy) value would be 10 000× larger.
        let buggy_per_unit = (pv_up - pv_down) / (2.0 * bump);

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Inflation01],
                PricingOptions::default(),
            )
            .expect("price with metrics");
        let reported = *result
            .measures
            .get("inflation01")
            .expect("inflation01 measure");

        // Must match the per-1bp value.
        assert!(
            (reported - expected_per_1bp).abs() < 1e-8 * expected_per_1bp.abs().max(1.0),
            "Inflation01 {reported} should equal per-1bp value {expected_per_1bp}"
        );
        // Must NOT match the per-unit (buggy) value.
        assert!(
            (reported - buggy_per_unit).abs() > expected_per_1bp.abs() * 100.0,
            "Inflation01 {reported} must NOT equal the per-unit (10 000×-inflated) value {buggy_per_unit}"
        );
    }

    /// Inflation01 sign: a long inflation-linked bond benefits from rising inflation,
    /// so Inflation01 must be positive for a standard long position.
    #[test]
    fn inflation01_sign_positive_for_long_ilb() {
        let as_of = date!(2024 - 01 - 15);
        let bond = sample_bond();
        let market = market(as_of);

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Inflation01],
                PricingOptions::default(),
            )
            .expect("price with metrics");
        let reported = *result
            .measures
            .get("inflation01")
            .expect("inflation01 measure");

        assert!(
            reported > 0.0,
            "Long ILB Inflation01 should be positive (higher inflation → higher PV), got {reported}"
        );
    }

    #[test]
    fn inflation01_supports_index_backed_linker() {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::DateExt;
        use finstack_quant_core::market_data::scalars::{
            InflationIndex, InflationInterpolation, InflationLag,
        };

        let as_of = date!(2024 - 01 - 15);
        let discount = DiscountCurve::builder("US-CPI-DISC")
            .base_date(as_of)
            .knots([(0.0, 1.0), (15.0, 0.65)])
            .build()
            .expect("discount curve");
        let first = date!(2023 - 10 - 01);
        let observations = (0..=122)
            .map(|month| (first.add_months(month), 100.0 + f64::from(month) * 0.2))
            .collect();
        let index = InflationIndex::new("US-CPI", observations, Currency::USD)
            .expect("inflation index")
            .with_interpolation(InflationInterpolation::Linear)
            .with_lag(InflationLag::None);
        let market = MarketContext::new()
            .insert(discount)
            .insert_inflation_index("US-CPI", index);
        let bond = sample_bond();

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Inflation01],
                PricingOptions::default(),
            )
            .expect("index-backed Inflation01");
        let inflation01 = result
            .measures
            .get(MetricId::Inflation01.as_str())
            .copied()
            .expect("Inflation01");
        assert!(inflation01.is_finite());
        assert!(inflation01 > 0.0);

        let projected_curve = InflationCurve::builder("US-CPI")
            .base_date(as_of)
            .base_cpi(100.6)
            .knots([(0.0, 100.6), (10.0, 125.0)])
            .build()
            .expect("projected inflation curve");
        let hybrid_market = market.insert(projected_curve);
        let hybrid = bond
            .price_with_metrics(
                &hybrid_market,
                as_of,
                &[MetricId::Inflation01],
                PricingOptions::default(),
            )
            .expect("hybrid index/curve Inflation01");
        assert!(
            hybrid
                .measures
                .get(MetricId::Inflation01.as_str())
                .copied()
                .expect("hybrid Inflation01")
                > 0.0
        );
    }
}
