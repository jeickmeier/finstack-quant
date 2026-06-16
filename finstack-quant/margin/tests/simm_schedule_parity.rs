//! SIMM embedded schedule parity test.
//!
//! This suite pins a subset of the ISDA SIMM v2.6 risk weights, correlations,
//! and concentration thresholds to their published values. Any accidental edit
//! to `data/margin/simm.v1.json` or `registry/embedded.rs` that mutates these
//! numbers will trip the test and force an explicit review.
//!
//! # Regulatory context
//!
//! ISDA publishes the SIMM methodology and schedules annually. The embedded
//! copy in this crate is frozen at a specific version and must be re-verified
//! against the ISDA source document on every schedule update.
//!
//! | Embedded version | ISDA source                                         | Frozen on   | Next review    |
//! |------------------|-----------------------------------------------------|-------------|-----------------|
//! | `SimmVersion::V2_6` | ISDA SIMM Methodology Version 2.6 (Dec 2023)     | 2024-01-15  | 2025-01 cycle  |
//! | `SimmVersion::V2_5` | ISDA SIMM Methodology Version 2.5 (Dec 2022)     | 2023-01-15  | (historical)   |
//!
//! When adopting a new ISDA release:
//!
//! 1. Download the SIMM Methodology PDF from isda.org.
//! 2. Add a new variant to [`finstack_quant_margin::SimmVersion`] if the schema
//!    changed.
//! 3. Add a new JSON entry to `data/margin/simm.v1.json` with all risk
//!    weights, correlations, and thresholds from the ISDA tables.
//! 4. Add a new `golden_values_for_*` function to this file with the
//!    ISDA-sourced values for the new version.
//! 5. Update the "Frozen on" and "Next review" rows above.
//!
//! # What this test catches
//!
//! - Accidental edits to the embedded JSON that change a risk weight.
//! - Silent schedule drift introduced by a refactor of the registry loader.
//! - Structural changes that drop, rename, or duplicate SIMM keys.
//!
//! # What this test does NOT catch
//!
//! - Legitimate ISDA schedule updates (by design — those require a reviewer
//!   to bless the new numbers by updating BOTH this file and the JSON).
//! - Formula-level bugs in the SIMM calculator itself (covered by the
//!   calculator's unit tests).

use finstack_quant_margin::{SimmCalculator, SimmVersion};

/// Golden SIMM v2.6 values sourced from the ISDA Methodology document.
///
/// Every entry MUST be backed by a specific page/table reference in the ISDA
/// PDF. Add a reviewer comment when changing any value.
struct SimmV26GoldenValues;

impl SimmV26GoldenValues {
    // ISDA SIMM v2.6 — Section E.1, Table 1: Interest Rate risk weights (bps)
    fn ir_delta_weights() -> &'static [(&'static str, f64)] {
        &[
            ("2w", 109.0),
            ("1m", 105.0),
            ("3m", 80.0),
            ("6m", 67.0),
            ("1y", 61.0),
            ("2y", 52.0),
            ("3y", 49.0),
            ("5y", 51.0),
            ("10y", 51.0),
            ("15y", 51.0),
            ("20y", 54.0),
            ("30y", 62.0),
        ]
    }

    // ISDA SIMM v2.6 — Section E.3, Table 5: Credit Qualifying delta weights
    fn cq_delta_weights() -> &'static [(&'static str, f64)] {
        &[
            ("sovereigns", 85.0),
            ("financials", 85.0),
            ("corporates", 73.0),
        ]
    }

    // ISDA SIMM v2.6 — Section E.3, Table 6: Credit Non-Qualifying delta weight
    fn cnq_delta_weight() -> f64 {
        500.0
    }

    // ISDA SIMM v2.6 — Section E.4, Table 9: Equity delta risk weight
    fn equity_delta_weight() -> f64 {
        32.0
    }

    // ISDA SIMM v2.6 — Section E.6, Table 14: FX delta weight
    fn fx_delta_weight() -> f64 {
        8.4
    }

    // ISDA SIMM v2.6 — Section E.6: FX delta intra-bucket correlation
    fn fx_intra_bucket_correlation() -> f64 {
        0.5
    }

    // ISDA SIMM v2.6 — Section E.2, Table 4: Inter-tenor IR correlations
    // Spot-checks along the diagonal and off-diagonal to anchor the matrix.
    fn ir_tenor_correlations() -> &'static [(&'static str, &'static str, f64)] {
        &[
            // Adjacent tenors — highest correlation
            ("2w", "1m", 0.99),
            ("10y", "15y", 0.98),
            ("20y", "30y", 0.99),
            // Mid-distance pairs
            ("1y", "5y", 0.88),
            ("2y", "10y", 0.88),
            // Wide tenor gaps — lowest correlation
            ("2w", "30y", 0.51),
            ("1m", "30y", 0.54),
            ("3m", "30y", 0.59),
        ]
    }

    // ISDA SIMM v2.6 — Margin Period of Risk (uncollateralized bilateral)
    fn mpor_days() -> u32 {
        10
    }

    // ISDA SIMM v2.6 — Section E.7: Concentration thresholds (USD)
    fn concentration_ir_threshold() -> f64 {
        230_000_000.0
    }
}

/// Small epsilon for f64 equality — SIMM values are exact decimal quantities
/// stored as f64, so direct equality would also work, but a tolerance guards
/// against JSON parser rounding on non-binary-representable values.
const EPS: f64 = 1e-9;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPS
}

#[test]
fn simm_v2_6_ir_delta_weights_match_isda_schedule() {
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry loads");
    for (tenor, expected) in SimmV26GoldenValues::ir_delta_weights() {
        let actual = calc
            .params
            .ir_delta_weights
            .get(*tenor)
            .copied()
            .unwrap_or_else(|| panic!("SIMM v2.6 missing IR delta weight for tenor '{tenor}'"));
        assert!(
            close(actual, *expected),
            "SIMM v2.6 IR delta weight drift at tenor '{tenor}': expected {expected}, \
             got {actual}. Update this test only if an ISDA schedule update has been reviewed."
        );
    }
}

#[test]
fn simm_v2_6_cq_delta_weights_match_isda_schedule() {
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry loads");
    for (bucket, expected) in SimmV26GoldenValues::cq_delta_weights() {
        let actual = calc
            .params
            .cq_delta_weights
            .get(*bucket)
            .copied()
            .unwrap_or_else(|| panic!("SIMM v2.6 missing CQ delta weight for bucket '{bucket}'"));
        assert!(
            close(actual, *expected),
            "SIMM v2.6 CQ delta weight drift at bucket '{bucket}': expected {expected}, \
             got {actual}. Update this test only if an ISDA schedule update has been reviewed."
        );
    }
}

#[test]
fn simm_v2_6_scalar_weights_match_isda_schedule() {
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry loads");

    assert!(
        close(
            calc.params.cnq_delta_weight,
            SimmV26GoldenValues::cnq_delta_weight()
        ),
        "SIMM v2.6 CNQ delta weight drift: expected {}, got {}",
        SimmV26GoldenValues::cnq_delta_weight(),
        calc.params.cnq_delta_weight
    );
    assert!(
        close(
            calc.params.equity_delta_weight,
            SimmV26GoldenValues::equity_delta_weight()
        ),
        "SIMM v2.6 equity delta weight drift: expected {}, got {}",
        SimmV26GoldenValues::equity_delta_weight(),
        calc.params.equity_delta_weight
    );
    assert!(
        close(
            calc.params.fx_delta_weight,
            SimmV26GoldenValues::fx_delta_weight()
        ),
        "SIMM v2.6 FX delta weight drift: expected {}, got {}",
        SimmV26GoldenValues::fx_delta_weight(),
        calc.params.fx_delta_weight
    );
    assert!(
        close(
            calc.params.fx_intra_bucket_correlation,
            SimmV26GoldenValues::fx_intra_bucket_correlation()
        ),
        "SIMM v2.6 FX intra-bucket correlation drift: expected {}, got {}",
        SimmV26GoldenValues::fx_intra_bucket_correlation(),
        calc.params.fx_intra_bucket_correlation
    );
    assert_eq!(
        calc.params.mpor_days,
        SimmV26GoldenValues::mpor_days(),
        "SIMM v2.6 MPOR drift: expected {}, got {}",
        SimmV26GoldenValues::mpor_days(),
        calc.params.mpor_days
    );
}

#[test]
fn simm_v2_6_ir_tenor_correlation_golden_samples() {
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry loads");

    for (a, b, expected) in SimmV26GoldenValues::ir_tenor_correlations() {
        // The registry stores the map keyed by an ordered `(String, String)`
        // tenor pair. Try both orderings because the canonical form is not
        // guaranteed by every loader version.
        let key1 = (a.to_string(), b.to_string());
        let key2 = (b.to_string(), a.to_string());
        let actual = calc
            .params
            .ir_tenor_correlations
            .get(&key1)
            .or_else(|| calc.params.ir_tenor_correlations.get(&key2))
            .copied()
            .unwrap_or_else(|| panic!("SIMM v2.6 missing IR tenor correlation for ({a}, {b})"));
        assert!(
            close(actual, *expected),
            "SIMM v2.6 IR tenor correlation drift at ({a}, {b}): expected {expected}, \
             got {actual}. Update this test only if an ISDA schedule update has been reviewed."
        );
    }
}

#[test]
fn simm_v2_6_ir_concentration_threshold_matches_isda() {
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry loads");
    // The registry stores concentration thresholds keyed by risk class.
    // This test specifically pins the IR threshold since it's the largest
    // and most frequently misconfigured.
    let thresholds = &calc.params.concentration_thresholds;
    let found = thresholds.iter().find_map(|(k, v)| {
        if format!("{k:?}").to_lowercase().contains("interest")
            || format!("{k:?}").to_lowercase().contains("rate")
        {
            Some(*v)
        } else {
            None
        }
    });
    let Some(actual) = found else {
        panic!(
            "SIMM v2.6 concentration_thresholds missing IR key (candidates: {:?})",
            thresholds.keys().collect::<Vec<_>>()
        );
    };
    assert!(
        close(actual, SimmV26GoldenValues::concentration_ir_threshold()),
        "SIMM v2.6 IR concentration threshold drift: expected {}, got {actual}",
        SimmV26GoldenValues::concentration_ir_threshold()
    );
}

#[test]
fn simm_v2_6_ir_delta_weight_count_matches_isda_tenor_set() {
    // ISDA SIMM v2.6 defines exactly 12 IR tenor buckets. Guard against
    // accidental addition or removal of buckets that wouldn't be caught
    // by the value-by-value test above (since missing-key also raises).
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry loads");
    assert_eq!(
        calc.params.ir_delta_weights.len(),
        12,
        "SIMM v2.6 expects exactly 12 IR tenor buckets; registry has {}",
        calc.params.ir_delta_weights.len()
    );
}
