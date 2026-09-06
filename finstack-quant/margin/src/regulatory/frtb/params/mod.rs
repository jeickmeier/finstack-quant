//! Prescribed FRTB risk weights, correlations, and other regulatory parameters.
//!
//! The per-risk-class submodules ([`commodity`], [`csr`], [`equity`], [`fx`],
//! [`girr`]) expose `pub const` tables transcribed from the Basel
//! standardised approach, and are read directly by the charge-calculation
//! helpers in [`delta`](super::delta), [`vega`](super::vega) and
//! [`curvature`](super::curvature).
//!
//! # Provenance
//!
//! | Item | Value |
//! |------|-------|
//! | Source document | Basel Committee on Banking Supervision, *Minimum capital requirements for market risk* (BCBS **d457**) |
//! | Publication date | 14 January 2019; corrected version published 25 February 2019 |
//! | Consolidated as | Basel Framework chapter **MAR21**, "Standardised approach: sensitivities-based method" |
//! | MAR21 version | Effective 1 January 2023 (implementation date revised 27 March 2020); text incorporates the FAQs published 5 July 2024 and 23 March 2026 |
//! | Primary sources verified | <https://www.bis.org/bcbs/publ/d457.pdf> and the BIS consolidated-framework PDF export <https://www.bis.org/baselframework/BaselFramework.pdf> |
//! | Last reviewed | 2026-08-20 |
//! | Review procedure | See `data/margin/README.md`, "FRTB parameter review" |
//!
pub mod commodity;
pub mod correlation_scenarios;
pub mod csr;
pub mod equity;
pub mod fx;
pub mod girr;

use super::types::FrtbRiskClass;

/// Name/tranche correlation before tenor, basis and scenario adjustments.
pub(super) fn name_correlation(class: FrtbRiskClass, bucket: u8) -> f64 {
    match class {
        FrtbRiskClass::Equity => match bucket {
            1..=4 => 0.15,
            5..=8 => 0.25,
            9 => 0.075,
            10 => 0.125,
            12..=13 => 0.8,
            _ => 0.0,
        },
        FrtbRiskClass::Commodity => [0.55, 0.95, 0.4, 0.8, 0.6, 0.65, 0.55, 0.45, 0.15, 0.4, 0.15]
            .get(usize::from(bucket.wrapping_sub(1)))
            .copied()
            .unwrap_or(f64::NAN),
        FrtbRiskClass::CsrNonSec | FrtbRiskClass::CsrSecCtp => {
            if bucket >= 17 {
                0.8
            } else {
                0.35
            }
        }
        FrtbRiskClass::CsrSecNonCtp => 0.4,
        _ => 1.0,
    }
}

/// Buckets whose constituents receive no intra-bucket diversification.
pub(super) fn other_bucket(class: FrtbRiskClass, bucket: u8) -> bool {
    matches!(
        (class, bucket),
        (FrtbRiskClass::Equity, 11)
            | (FrtbRiskClass::CsrNonSec | FrtbRiskClass::CsrSecCtp, 16)
            | (FrtbRiskClass::CsrSecNonCtp, 25)
    )
}

/// Basel MAR21.57 sector matrix, with rating adjustment for IG versus HY.
fn credit_inter_correlation(b: u8, c: u8) -> f64 {
    const MATRIX: [[f64; 11]; 11] = [
        [1., 0.75, 0.10, 0.20, 0.25, 0.20, 0.15, 0.10, 0., 0.45, 0.45],
        [0.75, 1., 0.05, 0.15, 0.20, 0.15, 0.10, 0.10, 0., 0.45, 0.45],
        [0.10, 0.05, 1., 0.05, 0.15, 0.20, 0.05, 0.20, 0., 0.45, 0.45],
        [0.20, 0.15, 0.05, 1., 0.20, 0.25, 0.05, 0.05, 0., 0.45, 0.45],
        [0.25, 0.20, 0.15, 0.20, 1., 0.25, 0.05, 0.15, 0., 0.45, 0.45],
        [0.20, 0.15, 0.20, 0.25, 0.25, 1., 0.05, 0.20, 0., 0.45, 0.45],
        [0.15, 0.10, 0.05, 0.05, 0.05, 0.05, 1., 0.05, 0., 0.45, 0.45],
        [0.10, 0.10, 0.20, 0.05, 0.15, 0.20, 0.05, 1., 0., 0.45, 0.45],
        [0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0.],
        [0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0., 1., 0.75],
        [0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0.45, 0., 0.75, 1.],
    ];
    let sector = |x: u8| {
        Some(usize::from(match x {
            1..=8 => x - 1,
            9..=15 => x - 9,
            16..=18 => x - 8,
            _ => return None,
        }))
    };
    let rating = if b <= 15 && c <= 15 && (b <= 8) != (c <= 8) {
        0.5
    } else {
        1.0
    };
    match (sector(b), sector(c)) {
        (Some(i), Some(j)) => rating * MATRIX[i][j],
        _ => f64::NAN,
    }
}

/// Correlation between distinct Basel buckets before scenario scaling.
pub(super) fn bucket_correlation(class: FrtbRiskClass, b: u8, c: u8) -> f64 {
    match class {
        FrtbRiskClass::Equity => {
            if b == 11 || c == 11 {
                0.0
            } else if b <= 10 && c <= 10 {
                0.15
            } else if b >= 12 && c >= 12 {
                0.75
            } else {
                0.45
            }
        }
        FrtbRiskClass::Commodity => {
            if b == 11 || c == 11 {
                0.0
            } else {
                0.2
            }
        }
        FrtbRiskClass::CsrNonSec | FrtbRiskClass::CsrSecCtp => credit_inter_correlation(b, c),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::{commodity, csr, equity, fx, girr};

    /// Exact-equality tolerance for a transcribed decimal constant. These are
    /// literals, not computed values, so they must match to the bit.
    fn assert_exact(actual: f64, expected: f64, what: &str) {
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "{what}: expected {expected}, got {actual}"
        );
    }

    /// Assert a `(bucket, weight)` table has exactly the expected entries, in
    /// order, with no gaps or duplicates.
    fn assert_bucket_table(actual: &[(u8, f64)], expected: &[(u8, f64)], what: &str) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "{what}: expected {} buckets, got {}",
            expected.len(),
            actual.len()
        );
        for (index, (&(actual_bucket, actual_weight), &(expected_bucket, expected_weight))) in
            actual.iter().zip(expected.iter()).enumerate()
        {
            assert_eq!(
                actual_bucket, expected_bucket,
                "{what}: entry {index} is bucket {actual_bucket}, expected {expected_bucket}"
            );
            assert_exact(
                actual_weight,
                expected_weight,
                &format!("{what} bucket {expected_bucket}"),
            );
        }
    }

    // -----------------------------------------------------------------
    // GIRR - MAR21.42, .43, .46, .48, .49, .50, .92
    // -----------------------------------------------------------------

    #[test]
    fn girr_delta_risk_weights_match_mar21_42_table_1() {
        let expected: &[(&str, f64)] = &[
            ("0.25Y", 1.7),
            ("0.5Y", 1.7),
            ("1Y", 1.6),
            ("2Y", 1.3),
            ("3Y", 1.2),
            ("5Y", 1.1),
            ("10Y", 1.1),
            ("15Y", 1.1),
            ("20Y", 1.1),
            ("30Y", 1.1),
        ];
        assert_eq!(
            girr::GIRR_DELTA_RISK_WEIGHTS.len(),
            expected.len(),
            "MAR21.42 Table 1 has 10 tenor buckets"
        );
        for (&(actual_tenor, actual_weight), &(expected_tenor, expected_weight)) in
            girr::GIRR_DELTA_RISK_WEIGHTS.iter().zip(expected.iter())
        {
            assert_eq!(actual_tenor, expected_tenor);
            assert_exact(
                actual_weight,
                expected_weight,
                &format!("GIRR delta RW {expected_tenor}"),
            );
        }
    }

    #[test]
    fn girr_inflation_and_xccy_risk_weights_match_mar21_43() {
        assert_exact(girr::GIRR_INFLATION_RISK_WEIGHT, 1.6, "GIRR inflation RW");
        assert_exact(
            girr::GIRR_XCCY_BASIS_RISK_WEIGHT,
            1.6,
            "GIRR cross-currency basis RW",
        );
    }

    #[test]
    fn girr_correlations_match_mar21_46_48_49_50() {
        assert_exact(
            girr::GIRR_TENOR_CORRELATION_THETA,
            0.03,
            "GIRR tenor correlation theta (MAR21.46 fn 13)",
        );
        assert_exact(
            girr::GIRR_TENOR_CORRELATION_FLOOR,
            0.40,
            "GIRR tenor correlation floor (MAR21.46)",
        );
        assert_exact(
            girr::GIRR_INFLATION_CORRELATION,
            0.40,
            "GIRR delta-vs-inflation correlation (MAR21.48)",
        );
        assert_exact(
            girr::GIRR_XCCY_BASIS_CORRELATION,
            0.0,
            "GIRR cross-currency basis correlation (MAR21.49)",
        );
        assert_exact(
            girr::GIRR_INTER_BUCKET_CORRELATION,
            0.50,
            "GIRR inter-bucket correlation (MAR21.50)",
        );
    }

    #[test]
    fn girr_tenor_correlation_matches_mar21_46_footnote_13() {
        // MAR21.46 footnote 13 publishes the worked example: the correlation
        // between a 1-year and a 5-year GIRR tenor is
        //   max(exp(-3% * |1 - 5| / min(1, 5)), 40%)
        //   = max(exp(-0.12), 0.40) = 0.886920... = 88.69%.
        // This simultaneously pins theta, the floor, and the choice of
        // min(T_k, T_l) in the denominator: a max(T_k, T_l) denominator would
        // give exp(-0.024) = 97.63% and contradict the published example.
        let rho = girr::girr_tenor_correlation(1.0, 5.0);
        assert!(
            (rho - 0.886_920_436_717_157_5).abs() < 1e-12,
            "MAR21.46 fn 13 worked example: expected 88.69%, got {rho}"
        );
        // Symmetry, and the identity case.
        assert!((girr::girr_tenor_correlation(5.0, 1.0) - rho).abs() < 1e-12);
        assert_exact(girr::girr_tenor_correlation(5.0, 5.0), 1.0, "same tenor");
        // The floor binds for widely separated tenors: 0.25Y vs 30Y gives
        // exp(-0.03 * 29.75 / 0.25) = exp(-3.57), far below 40%.
        assert_exact(
            girr::girr_tenor_correlation(0.25, 30.0),
            0.40,
            "MAR21.46 correlation floor",
        );
    }

    #[test]
    fn girr_tenor_labels_map_to_the_mar21_42_grid() {
        let expected = [
            ("0.25Y", 0.25),
            ("0.5Y", 0.5),
            ("1Y", 1.0),
            ("2Y", 2.0),
            ("3Y", 3.0),
            ("5Y", 5.0),
            ("10Y", 10.0),
            ("15Y", 15.0),
            ("20Y", 20.0),
            ("30Y", 30.0),
        ];
        for (label, years) in expected {
            let actual = girr::tenor_to_years(label)
                .unwrap_or_else(|| panic!("tenor label {label} must be recognised"));
            assert_exact(actual, years, label);
        }
        assert!(girr::tenor_to_years("6M").is_none());
        assert!(girr::tenor_to_years("0.25y").is_none());
    }

    // -----------------------------------------------------------------
    // CSR - MAR21.53, .54, .57, .59, .60, .61, .64, .68, .70
    // -----------------------------------------------------------------

    #[test]
    fn csr_nonsec_risk_weights_match_mar21_53_table_4() {
        // All 18 buckets match MAR21.53 Table 4 as published.
        // MAR21.53 footnote 17 permits a *discretionary* 1.5% for covered
        // bonds rated AA- or better; this table carries the standard 2.5%.
        assert_bucket_table(
            csr::CSR_NONSEC_RISK_WEIGHTS,
            &[
                (1, 0.5),
                (2, 1.0),
                (3, 5.0),
                (4, 3.0),
                (5, 3.0),
                (6, 2.0),
                (7, 1.5),
                (8, 2.5),
                (9, 2.0),
                (10, 4.0),
                (11, 12.0),
                (12, 7.0),
                (13, 8.5),
                (14, 5.5),
                (15, 5.0),
                (16, 12.0),
                (17, 1.5),
                (18, 5.0),
            ],
            "CSR non-sec delta risk weights",
        );
        // Bucket 8 is the value this table was previously wrong about, so
        // assert it through the public accessor as well as the raw table —
        // a lookup that silently fell through to a default would otherwise
        // pass the table check above.
        assert_exact(
            csr::csr_nonsec_risk_weight(8),
            2.5,
            "CSR non-sec bucket 8 (covered bonds), MAR21.53 Table 4",
        );
        assert_exact(
            csr::csr_nonsec_risk_weight(9),
            2.0,
            "CSR non-sec bucket 9, MAR21.53 Table 4",
        );
    }

    #[test]
    fn csr_nonsec_correlations_match_basel() {
        assert_exact(
            super::name_correlation(super::FrtbRiskClass::CsrNonSec, 1),
            0.35,
            "name correlation",
        );
        assert_exact(
            super::bucket_correlation(super::FrtbRiskClass::CsrNonSec, 1, 3),
            0.1,
            "sector correlation",
        );
    }

    #[test]
    fn csr_sec_ctp_risk_weights_match_mar21_59_table_6() {
        assert_bucket_table(
            csr::CSR_SEC_CTP_RISK_WEIGHTS,
            &[
                (1, 4.0),
                (2, 4.0),
                (3, 8.0),
                (4, 5.0),
                (5, 4.0),
                (6, 3.0),
                (7, 2.0),
                (8, 6.0),
                (9, 13.0),
                (10, 13.0),
                (11, 16.0),
                (12, 10.0),
                (13, 12.0),
                (14, 12.0),
                (15, 12.0),
                (16, 13.0),
            ],
            "CSR sec CTP delta risk weights",
        );
    }

    #[test]
    fn csr_sec_ctp_correlations_match_basel() {
        assert_exact(
            super::name_correlation(super::FrtbRiskClass::CsrSecCtp, 1),
            0.35,
            "name correlation",
        );
        assert_exact(
            super::bucket_correlation(super::FrtbRiskClass::CsrSecCtp, 1, 3),
            0.1,
            "sector correlation",
        );
    }

    #[test]
    fn csr_sec_nonctp_risk_weights_match_mar21_64_through_67() {
        // MAR21.64 Table 8 publishes eight senior investment-grade weights
        //   [0.9, 1.5, 2.0, 2.0, 0.8, 1.2, 1.2, 1.4]%
        // and derives the rest: buckets 9-16 are 1.25x those (MAR21.65),
        // buckets 17-24 are 1.75x (MAR21.66), and bucket 25 = 3.5% (MAR21.67).
        // The expectations use the same products as the table.
        assert_bucket_table(
            csr::CSR_SEC_NONCTP_RISK_WEIGHTS,
            &[
                (1, 0.9),
                (2, 1.5),
                (3, 2.0),
                (4, 2.0),
                (5, 0.8),
                (6, 1.2),
                (7, 1.2),
                (8, 1.4),
                (9, 0.9 * 1.25),
                (10, 1.5 * 1.25),
                (11, 2.0 * 1.25),
                (12, 2.0 * 1.25),
                (13, 0.8 * 1.25),
                (14, 1.2 * 1.25),
                (15, 1.2 * 1.25),
                (16, 1.4 * 1.25),
                (17, 0.9 * 1.75),
                (18, 1.5 * 1.75),
                (19, 2.0 * 1.75),
                (20, 2.0 * 1.75),
                (21, 0.8 * 1.75),
                (22, 1.2 * 1.75),
                (23, 1.2 * 1.75),
                (24, 1.4 * 1.75),
                (25, 3.5),
            ],
            "CSR sec non-CTP risk weights (MAR21.64-21.67)",
        );
    }

    #[test]
    fn csr_sec_nonctp_correlations_match_basel() {
        assert_exact(
            super::name_correlation(super::FrtbRiskClass::CsrSecNonCtp, 1),
            0.4,
            "name correlation",
        );
        assert_exact(
            super::bucket_correlation(super::FrtbRiskClass::CsrSecNonCtp, 1, 5),
            0.0,
            "sector correlation",
        );
    }

    // -----------------------------------------------------------------
    // Equity - MAR21.77, .78, .80, .92
    // -----------------------------------------------------------------

    #[test]
    fn equity_delta_risk_weights_match_mar21_77_table_10_spot_column() {
        assert_bucket_table(
            equity::EQUITY_RISK_WEIGHTS,
            &[
                (1, 55.0),
                (2, 60.0),
                (3, 45.0),
                (4, 55.0),
                (5, 30.0),
                (6, 35.0),
                (7, 40.0),
                (8, 50.0),
                (9, 70.0),
                (10, 50.0),
                (11, 70.0),
                (12, 15.0),
                (13, 25.0),
            ],
            "equity spot delta risk weights",
        );
    }

    #[test]
    fn equity_correlations_match_basel() {
        for (bucket, expected) in [(1, 0.15), (5, 0.25), (9, 0.075), (10, 0.125), (12, 0.8)] {
            assert_exact(
                super::name_correlation(super::FrtbRiskClass::Equity, bucket),
                expected,
                "equity name correlation",
            );
        }
        assert_exact(
            super::bucket_correlation(super::FrtbRiskClass::Equity, 12, 13),
            0.75,
            "index correlation",
        );
        assert_exact(
            super::bucket_correlation(super::FrtbRiskClass::Equity, 1, 11),
            0.0,
            "other sector correlation",
        );
    }

    #[test]
    fn equity_vega_risk_weight_matches_mar21_92() {
        for b in 1..=13 {
            let expected = if (9..=11).contains(&b) {
                1.0
            } else {
                0.55 * 2.0_f64.sqrt()
            };
            assert_exact(
                equity::equity_vega_risk_weight(b),
                expected,
                "equity vega liquidity horizon",
            );
        }
    }

    // -----------------------------------------------------------------
    // Commodity - MAR21.82, .83, .85, .92
    // -----------------------------------------------------------------

    #[test]
    fn commodity_delta_risk_weights_match_mar21_82_table_11() {
        assert_bucket_table(
            commodity::COMMODITY_RISK_WEIGHTS,
            &[
                (1, 30.0),
                (2, 35.0),
                (3, 60.0),
                (4, 80.0),
                (5, 40.0),
                (6, 45.0),
                (7, 20.0),
                (8, 35.0),
                (9, 25.0),
                (10, 35.0),
                (11, 50.0),
            ],
            "commodity delta risk weights",
        );
    }

    #[test]
    fn commodity_correlations_match_basel() {
        for (i, rho) in [0.55, 0.95, 0.4, 0.8, 0.6, 0.65, 0.55, 0.45, 0.15, 0.4, 0.15]
            .into_iter()
            .enumerate()
        {
            assert_exact(
                super::name_correlation(super::FrtbRiskClass::Commodity, (i + 1) as u8),
                rho,
                "commodity name correlation",
            );
        }
        assert_exact(
            super::bucket_correlation(super::FrtbRiskClass::Commodity, 1, 11),
            0.0,
            "other commodity correlation",
        );
    }

    // -----------------------------------------------------------------
    // FX - MAR21.87, .89, .92
    // -----------------------------------------------------------------

    #[test]
    fn fx_parameters_match_mar21_87_and_89() {
        assert_exact(
            fx::FX_DELTA_RISK_WEIGHT,
            15.0,
            "FX delta risk weight (MAR21.87)",
        );
        assert_exact(
            fx::FX_INTER_PAIR_CORRELATION,
            0.60,
            "FX inter-bucket correlation (MAR21.89)",
        );
    }

    // -----------------------------------------------------------------
    // Vega risk weights - MAR21.92 footnote 24 and Table 13
    // -----------------------------------------------------------------

    #[test]
    fn vega_risk_weights_match_the_mar21_92_liquidity_horizon_formula() {
        // MAR21.92 footnote 24:
        //   RW_k = min( RW_sigma * sqrt(LH_risk class) / sqrt(10), 100% )
        // with RW_sigma = 55% and LH from MAR21.92 Table 13. Recomputing the
        // constants from the published formula ties them to the standard
        // rather than to themselves: if someone edits a constant without
        // editing the liquidity horizon, this fails.
        const RW_SIGMA: f64 = 0.55;
        let weight_for = |liquidity_horizon_days: f64| -> f64 {
            f64::min(RW_SIGMA * (liquidity_horizon_days / 10.0).sqrt(), 1.0)
        };

        // Table 13 liquidity horizons, in days.
        assert_exact(
            girr::GIRR_VEGA_RISK_WEIGHT,
            weight_for(60.0),
            "GIRR vega RW (LH = 60 days)",
        );
        assert_exact(
            csr::CSR_NONSEC_VEGA_RISK_WEIGHT,
            weight_for(120.0),
            "CSR non-sec vega RW (LH = 120 days)",
        );
        assert_exact(
            csr::CSR_SEC_CTP_VEGA_RISK_WEIGHT,
            weight_for(120.0),
            "CSR sec CTP vega RW (LH = 120 days)",
        );
        assert_exact(
            csr::CSR_SEC_NONCTP_VEGA_RISK_WEIGHT,
            weight_for(120.0),
            "CSR sec non-CTP vega RW (LH = 120 days)",
        );
        assert_exact(
            commodity::COMMODITY_VEGA_RISK_WEIGHT,
            weight_for(120.0),
            "commodity vega RW (LH = 120 days)",
        );
        assert_exact(
            fx::FX_VEGA_RISK_WEIGHT,
            weight_for(40.0),
            "FX vega RW (LH = 40 days)",
        );

        // All five of the above bind at the 100% cap, which is why they read
        // 1.00. That is the published Table 13 value, not a placeholder.
        assert_exact(weight_for(60.0), 1.0, "GIRR vega weight binds at the cap");
        assert_exact(weight_for(40.0), 1.0, "FX vega weight binds at the cap");

        // Equity is the only risk class whose vega weight does not bind at
        // the cap, and it is also the one that deviates -- see
        // `equity_vega_risk_weight_pins_known_deviation_from_mar21_92`.
        assert!(
            weight_for(20.0) < 1.0,
            "equity large-cap vega weight must be below the cap"
        );
    }

    // -----------------------------------------------------------------
    // Lookup fallbacks - library conventions, not Basel-published values
    // -----------------------------------------------------------------

    #[test]
    fn unmapped_bucket_lookups_use_the_documented_fallback_weights() {
        // These fallbacks multiply a capital charge whenever a caller
        // supplies an out-of-range bucket, so they are pinned even though
        // MAR21 publishes no such value. Note that
        // `FrtbSensitivities::validate` rejects unknown buckets before the
        // engine reaches these lookups; the fallbacks only apply to direct
        // callers of the `params` helpers.
        const UNMAPPED: u8 = 200;
        assert_exact(
            csr::csr_nonsec_risk_weight(UNMAPPED),
            5.0,
            "CSR non-sec fallback risk weight",
        );
        assert_exact(
            csr::csr_sec_ctp_risk_weight(UNMAPPED),
            8.0,
            "CSR sec CTP fallback risk weight",
        );
        assert_exact(
            csr::csr_sec_nonctp_risk_weight(UNMAPPED),
            5.0,
            "CSR sec non-CTP fallback risk weight",
        );
        assert!(equity::equity_risk_weight(UNMAPPED).is_nan());
        assert!(commodity::commodity_risk_weight(UNMAPPED).is_nan());
    }

    #[test]
    fn every_mapped_bucket_lookup_returns_its_table_entry() {
        // Guards the LazyLock index construction against silently dropping or
        // duplicating an entry.
        for &(bucket, weight) in csr::CSR_NONSEC_RISK_WEIGHTS {
            assert_exact(
                csr::csr_nonsec_risk_weight(bucket),
                weight,
                &format!("CSR non-sec lookup bucket {bucket}"),
            );
        }
        for &(bucket, weight) in csr::CSR_SEC_CTP_RISK_WEIGHTS {
            assert_exact(
                csr::csr_sec_ctp_risk_weight(bucket),
                weight,
                &format!("CSR sec CTP lookup bucket {bucket}"),
            );
        }
        for &(bucket, weight) in csr::CSR_SEC_NONCTP_RISK_WEIGHTS {
            assert_exact(
                csr::csr_sec_nonctp_risk_weight(bucket),
                weight,
                &format!("CSR sec non-CTP lookup bucket {bucket}"),
            );
        }
        for &(bucket, weight) in equity::EQUITY_RISK_WEIGHTS {
            assert_exact(
                equity::equity_risk_weight(bucket),
                weight,
                &format!("equity lookup bucket {bucket}"),
            );
        }
        for &(bucket, weight) in commodity::COMMODITY_RISK_WEIGHTS {
            assert_exact(
                commodity::commodity_risk_weight(bucket),
                weight,
                &format!("commodity lookup bucket {bucket}"),
            );
        }
    }
}
