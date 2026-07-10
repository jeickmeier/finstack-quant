//! Inflation01 calculator for inflation cap/floor options.
//!
//! Computes inflation sensitivity using central finite differences on the inflation curve.
//!
//! # Methodology
//!
//! Inflation01 measures the change in PV for a 1 basis point (0.01%) parallel shift
//! in the inflation curve. Uses central differences for O(h²) accuracy:
//!
//! ```text
//! Inflation01 = (PV_up - PV_down) / 2
//! ```
//!
//! Dividing by 2 (not by 2 × bump_size_decimal) produces the PV change for a
//! **1bp move**, consistent with the DV01/CS01 convention throughout the
//! workspace (see `sensitivity_central_diff` where the divisor is 2·1 = 2 for
//! a ±1bp bump expressed in bp-units).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::inflation_cap_floor::InflationCapFloor;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::Result;

/// Inflation curve bump size: 1bp = 0.01% for `BumpSpec::inflation_shift_pct`.
/// The BumpSpec expects percentage terms, so 0.01 means 0.01% = 1bp.
pub(crate) const INFLATION_BUMP_PCT: f64 = 0.01;

/// Inflation01 calculator for inflation cap/floor options.
///
/// Computes the present value sensitivity to a 1bp parallel shift in inflation expectations.
pub(crate) struct Inflation01Calculator;

impl MetricCalculator for Inflation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &InflationCapFloor = context.instrument_as()?;
        let as_of = context.as_of;

        // Bump up by 1bp (0.01%)
        let bump_spec_up = BumpSpec::inflation_shift_pct(INFLATION_BUMP_PCT);
        let curves_up = context.curves.as_ref().bump([MarketBump::Curve {
            id: option.inflation_index_id.clone(),
            spec: bump_spec_up,
        }])?;
        let pv_up = option.value(&curves_up, as_of)?.amount();

        // Bump down by 1bp (-0.01%)
        let bump_spec_down = BumpSpec::inflation_shift_pct(-INFLATION_BUMP_PCT);
        let curves_down = context.curves.as_ref().bump([MarketBump::Curve {
            id: option.inflation_index_id.clone(),
            spec: bump_spec_down,
        }])?;
        let pv_down = option.value(&curves_down, as_of)?.amount();

        // Central difference: Inflation01 = (PV_up - PV_down) / 2.
        //
        // The curve was bumped by exactly ±1bp (INFLATION_BUMP_PCT = 0.01% = 0.0001 decimal).
        // A symmetric ±1bp central difference already produces the PV change for
        // a 1bp move; no further normalization by the decimal bump value is needed.
        // Dividing by 2 (not by 2 × 0.0001) is the correct per-1bp convention —
        // consistent with DV01 and CS01 in the workspace.
        Ok((pv_up - pv_down) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::INFLATION_BUMP_PCT;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::rates::inflation_cap_floor::InflationCapFloor;
    use crate::instruments::PricingOptions;
    use crate::metrics::MetricId;
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::InflationIndex;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use finstack_quant_core::types::CurveId;
    use time::macros::date;

    fn market(as_of: finstack_quant_core::dates::Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
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
        // Flat vol surface at 20% so options have positive time value.
        let vol = 0.20_f64;
        let vol_surface = VolSurface::builder(CurveId::new("USD-INFL-VOL"))
            .expiries(&[0.5, 1.0, 2.0, 5.0])
            .strikes(&[0.0, 0.01, 0.02, 0.03, 0.05])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .build()
            .expect("vol surface");
        let index = InflationIndex::new(
            "US-CPI",
            vec![
                (date!(2000 - 01 - 01), 100.0),
                (date!(2035 - 01 - 01), 100.0),
            ],
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("inflation index");
        MarketContext::new()
            .insert(discount)
            .insert(inflation)
            .insert_inflation_index("US-CPI", index)
            .insert_surface(vol_surface)
    }

    fn sample_cap() -> InflationCapFloor {
        let mut cap = InflationCapFloor::example();
        cap.discount_curve_id = CurveId::new("USD-OIS");
        cap.inflation_index_id = CurveId::new("US-CPI");
        cap.vol_surface_id = CurveId::new("USD-INFL-VOL");
        cap
    }

    /// Inflation01 must equal `(pv_up - pv_down) / 2`, not
    /// `(pv_up - pv_down) / (2 * bump_decimal)`.
    ///
    /// The "…01" convention throughout the workspace (DV01, CS01, Inflation01)
    /// is PV-change-per-1bp. A ±1bp central bump produces that directly as
    /// `(pv_up - pv_down) / 2`. Dividing by the decimal bump value (0.0002)
    /// instead yields the per-*unit* derivative — 10 000× too large — which is
    /// the bug this test guards against.
    #[test]
    fn inflation01_is_per_1bp_not_per_unit() {
        let as_of = date!(2024 - 01 - 15);
        let cap = sample_cap();
        let market = market(as_of);

        // Decimal equivalent of the PCT bump used by the calculator.
        let bump_decimal = INFLATION_BUMP_PCT / 100.0; // 0.01% → 0.0001

        // Compute the two bumped PVs the same way the calculator does.
        let curves_up = market
            .bump([MarketBump::Curve {
                id: cap.inflation_index_id.clone(),
                spec: BumpSpec::inflation_shift_pct(INFLATION_BUMP_PCT),
            }])
            .expect("bump up");
        let curves_down = market
            .bump([MarketBump::Curve {
                id: cap.inflation_index_id.clone(),
                spec: BumpSpec::inflation_shift_pct(-INFLATION_BUMP_PCT),
            }])
            .expect("bump down");
        let pv_up = cap.value(&curves_up, as_of).expect("pv up").amount();
        let pv_down = cap.value(&curves_down, as_of).expect("pv down").amount();

        // Expected: per-1bp sensitivity — the half-difference.
        let expected_per_1bp = (pv_up - pv_down) / 2.0;
        // The old (buggy) value would be 10 000× larger.
        let buggy_per_unit = (pv_up - pv_down) / (2.0 * bump_decimal);

        let result = cap
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

    /// Inflation01 sign: a long inflation cap benefits from rising inflation
    /// (higher forward rates increase the caplet payoffs), so Inflation01 > 0.
    #[test]
    fn inflation01_sign_positive_for_long_cap() {
        let as_of = date!(2024 - 01 - 15);
        let cap = sample_cap();
        let market = market(as_of);

        let result = cap
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
            "Long inflation cap Inflation01 should be positive (higher inflation → higher PV), got {reported}"
        );
    }
}
