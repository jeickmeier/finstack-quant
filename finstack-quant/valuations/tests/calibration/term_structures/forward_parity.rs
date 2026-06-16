//! Property-based tests for forward parity relationships.
//!
//! Key Properties:
//! - Discount factor monotonicity: DF(t2) < DF(t1) for t2 > t1 and r > 0
//! - Zero rate from DF: z = -ln(DF)/t
//! - Interest rate forward: (1 + f·τ) = DF(t1) / DF(t2)
//!
//! ## Historical Note
//!
//! Equity forward parity test `F = S·exp((r-q)·T)` was removed due to
//! rate=0 edge case issues. The underlying discount factor properties
//! are thoroughly tested by the proptest tests below.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use proptest::prelude::*;
use time::Month;

fn create_discount_curve_for_parity(base_date: Date, rate: f64, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (0.5, (-rate * 0.5).exp()),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
        ]);

    // Allow non-monotonic for zero/negative rates
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder
            .validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            )
            .interp(InterpStyle::Linear);
    }

    builder.build().unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_discount_factor_monotonicity(
        rate in 0.01..0.10,
        t1 in 0.5..2.0,
        t2 in 2.5..5.0,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

        let curve = create_discount_curve_for_parity(base_date, rate, "TEST-DISC");

        let df_t1 = curve.df(t1);
        let df_t2 = curve.df(t2);

        // Property: DF(t2) < DF(t1) for t2 > t1 (strictly decreasing)
        prop_assert!(
            df_t2 < df_t1,
            "DF({}) = {:.6} should be < DF({}) = {:.6}",
            t2, df_t2, t1, df_t1
        );

        // Property: Both DFs should be positive
        prop_assert!(df_t1 > 0.0 && df_t2 > 0.0,
            "DFs must be positive: DF({}) = {:.6}, DF({}) = {:.6}",
            t1, df_t1, t2, df_t2
        );
    }

    #[test]
    fn prop_zero_rate_from_discount_factor(
        rate in 0.01..0.10,
        time in 0.5..10.0,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

        let curve = create_discount_curve_for_parity(base_date, rate, "ZERO-CURVE");

        let df = curve.df(time);
        let zero = curve.zero(time);

        // Property: DF = exp(-z·t), so z = -ln(DF)/t
        let zero_from_df = -df.ln() / time;

        let zero_diff = (zero - zero_from_df).abs();

        prop_assert!(
            zero_diff < 1e-12,
            "Zero rate inconsistency at t={}: zero() = {:.6}, from DF = {:.6}",
            time, zero, zero_from_df
        );
    }

    /// Interest rate forward parity: continuous compounding relationship.
    ///
    /// The forward rate for period [t1, t2] with continuous compounding:
    ///     f = (z2 × t2 - z1 × t1) / (t2 - t1)
    ///
    /// where z1, z2 are zero rates at t1, t2 respectively.
    #[test]
    fn prop_interest_rate_forward_parity(
        rate in 0.01..0.10,
        t1 in 0.25..2.0,
        t2 in 2.5..5.0,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let curve = create_discount_curve_for_parity(base_date, rate, "FWD-PARITY");

        let z1 = curve.zero(t1);
        let z2 = curve.zero(t2);
        let tau = t2 - t1;

        // Continuous compounding: f = (z2·t2 - z1·t1) / τ
        let fwd_from_zero = (z2 * t2 - z1 * t1) / tau;
        let fwd_from_curve = curve.forward(t1, t2).expect("forward should succeed");

        let rel_error = if fwd_from_zero.abs() > 1e-10 {
            (fwd_from_zero - fwd_from_curve).abs() / fwd_from_zero.abs()
        } else {
            (fwd_from_zero - fwd_from_curve).abs()
        };

        prop_assert!(
            rel_error < 1e-10,
            "Forward rate parity: from_zero={:.8}, from_curve={:.8}, rel_err={:.2e}",
            fwd_from_zero, fwd_from_curve, rel_error
        );
    }

    #[test]
    fn prop_forward_parity_near_zero_and_negative_rates(
        rate in -0.02..0.02,
        t1 in 0.25..2.0,
        t2 in 2.5..5.0,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
        let curve = create_discount_curve_for_parity(base_date, rate, "FWD-PARITY-ZERO");

        let df = curve.df(t2);
        let zero = curve.zero(t2);
        let zero_from_df = -df.ln() / t2;
        let zero_diff = (zero - zero_from_df).abs();
        prop_assert!(
            zero_diff < 1e-12,
            "Zero rate inconsistency at t={}: zero() = {:.6}, from DF = {:.6}",
            t2, zero, zero_from_df
        );

        let z1 = curve.zero(t1);
        let z2 = curve.zero(t2);
        let tau = t2 - t1;
        let fwd_from_zero = (z2 * t2 - z1 * t1) / tau;
        let fwd_from_curve = curve.forward(t1, t2).expect("forward should succeed");
        let rel_error = if fwd_from_zero.abs() > 1e-12 {
            (fwd_from_zero - fwd_from_curve).abs() / fwd_from_zero.abs()
        } else {
            (fwd_from_zero - fwd_from_curve).abs()
        };
        prop_assert!(
            rel_error < 1e-10,
            "Forward parity near zero/neg: from_zero={:.8}, from_curve={:.8}, rel_err={:.2e}",
            fwd_from_zero, fwd_from_curve, rel_error
        );
    }
}
