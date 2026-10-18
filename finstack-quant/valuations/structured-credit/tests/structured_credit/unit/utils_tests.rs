//! Unit tests for utility functions.
//!
//! Tests cover:
//! - Rating factor tables and lookups

use finstack_quant_core::types::CreditRating;
use finstack_quant_models::credit::{moodys_warf_factor, RatingFactorTable};

// Rating Factor Tests

#[test]
fn test_moodys_warf_factor_aaa() {
    // Act
    let factor = moodys_warf_factor(CreditRating::AAA).unwrap();

    // Assert: AAA should be 1 (best rating)
    assert_eq!(factor, 1.0);
}

#[test]
fn test_moodys_warf_factor_a() {
    // Act
    let factor = moodys_warf_factor(CreditRating::A).unwrap();

    // Assert: A (flat notch / A2) should be 120
    assert_eq!(factor, 120.0);
}

#[test]
fn test_moodys_warf_factor_bb() {
    // Act
    let factor = moodys_warf_factor(CreditRating::BB).unwrap();

    // Assert: BB should be 1350
    assert_eq!(factor, 1350.0);
}

#[test]
fn test_moodys_warf_factor_b() {
    // Act
    let factor = moodys_warf_factor(CreditRating::B).unwrap();

    // Assert: B should be 2720
    assert_eq!(factor, 2720.0);
}

#[test]
fn test_moodys_warf_factor_ccc() {
    // Act
    let factor = moodys_warf_factor(CreditRating::CCC).unwrap();

    // Assert: CCC should be 6500
    assert_eq!(factor, 6500.0);
}

#[test]
fn test_moodys_warf_factor_nr() {
    // Act
    let factor = moodys_warf_factor(CreditRating::NR).unwrap();

    // Assert: Not rated should be 3650 (B-/CCC+ equivalent)
    assert_eq!(factor, 3650.0);
}

#[test]
fn test_rating_factor_table_creation() {
    // Arrange & Act
    let table = RatingFactorTable::moodys_standard().expect("registry table");

    // Assert
    assert_eq!(table.agency(), "Moody's");
    assert_eq!(table.methodology(), "IDEALIZED DEFAULT RATES");
    assert_eq!(table.get_factor(CreditRating::AAA).unwrap(), 1.0);
    assert_eq!(table.get_factor(CreditRating::BB).unwrap(), 1350.0);
    assert_eq!(table.get_factor(CreditRating::AAPlus).unwrap(), 10.0);
}

#[test]
fn test_rating_factor_monotonicity() {
    // Arrange: Better ratings should have lower factors
    let ratings = [
        (CreditRating::AAA, 1.0),
        (CreditRating::AA, 20.0),
        (CreditRating::A, 120.0),
        (CreditRating::BBB, 360.0),
        (CreditRating::BB, 1350.0),
        (CreditRating::B, 2720.0),
        (CreditRating::CCC, 6500.0),
    ];

    // Act & Assert: Factors should increase with worse ratings
    for i in 1..ratings.len() {
        let prev_factor = moodys_warf_factor(ratings[i - 1].0).unwrap();
        let curr_factor = moodys_warf_factor(ratings[i].0).unwrap();
        assert!(
            curr_factor > prev_factor,
            "Rating factors not monotonic: {:?} ({}), {:?} ({})",
            ratings[i - 1].0,
            prev_factor,
            ratings[i].0,
            curr_factor
        );
    }
}

#[test]
fn test_moodys_warf_factor_notches() {
    assert_eq!(moodys_warf_factor(CreditRating::BBPlus).unwrap(), 940.0);
    assert_eq!(moodys_warf_factor(CreditRating::BBMinus).unwrap(), 1766.0);
    assert_eq!(moodys_warf_factor(CreditRating::BBB).unwrap(), 360.0);
}
