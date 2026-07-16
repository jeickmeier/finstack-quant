//! Behavioral tests for absolute-priority recovery allocation.

use finstack_quant_core::credit::recovery_waterfall::{allocate_recovery, RecoveryClaim};

fn claim(
    id: &str,
    priority: u32,
    principal: f64,
    collateral_value: Option<f64>,
    collateral_haircut: f64,
) -> RecoveryClaim {
    RecoveryClaim {
        id: id.to_string(),
        seniority: format!("priority_{priority}"),
        priority,
        principal,
        accrued: 0.0,
        penalties: 0.0,
        collateral_value,
        collateral_haircut,
    }
}

#[test]
fn estate_includes_collateral_and_equal_priority_deficiencies_share_residual_pro_rata() {
    let claims = vec![
        claim("secured", 1, 100.0, Some(60.0), 0.25),
        claim("unsecured_peer", 1, 100.0, None, 0.0),
        claim("junior", 2, 50.0, None, 0.0),
    ];

    let result = allocate_recovery(100.0, &claims).expect("valid waterfall");

    assert!((result.total_distributed - 100.0).abs() < 1e-12);
    assert_eq!(result.undistributed_estate, 0.0);
    assert!(result.apr_satisfied);
    assert_eq!(
        result
            .allocations
            .iter()
            .map(|allocation| allocation.id.as_str())
            .collect::<Vec<_>>(),
        vec!["secured", "unsecured_peer", "junior"]
    );

    let secured = &result.allocations[0];
    let peer = &result.allocations[1];
    assert!((secured.collateral_recovery - 45.0).abs() < 1e-12);
    assert!((secured.general_recovery - 55.0 * 55.0 / 155.0).abs() < 1e-12);
    assert!((peer.general_recovery - 55.0 * 100.0 / 155.0).abs() < 1e-12);
    assert_eq!(result.allocations[2].total_recovery, 0.0);
}

#[test]
fn waterfall_conserves_estate_and_reports_excess_as_undistributed() {
    let claims = vec![
        claim("first", 1, 20.0, None, 0.0),
        claim("second", 2, 30.0, None, 0.0),
    ];

    let result = allocate_recovery(75.0, &claims).expect("valid waterfall");

    assert_eq!(result.total_distributed, 50.0);
    assert_eq!(result.undistributed_estate, 25.0);
    assert!(result
        .allocations
        .iter()
        .all(|allocation| allocation.total_recovery <= allocation.total_claim));
}

#[test]
fn waterfall_is_stable_by_priority_then_input_order() {
    let claims = vec![
        claim("junior", 20, 10.0, None, 0.0),
        claim("peer_b", 10, 10.0, None, 0.0),
        claim("peer_a", 10, 10.0, None, 0.0),
    ];

    let result = allocate_recovery(15.0, &claims).expect("valid waterfall");
    assert_eq!(
        result
            .allocations
            .iter()
            .map(|allocation| allocation.id.as_str())
            .collect::<Vec<_>>(),
        vec!["peer_b", "peer_a", "junior"]
    );
    assert_eq!(result.allocations[0].total_recovery, 7.5);
    assert_eq!(result.allocations[1].total_recovery, 7.5);
    assert_eq!(result.allocations[2].total_recovery, 0.0);
}

#[test]
fn collateral_rounding_residue_is_accepted_and_reconciled_conservatively() {
    let claims = vec![
        claim("first", 1, 0.1, Some(0.1), 0.0),
        claim("second", 1, 0.2, Some(0.2), 0.0),
    ];

    let result = allocate_recovery(0.3, &claims).expect("sub-ulp collateral excess is valid");
    let allocated = result
        .allocations
        .iter()
        .map(|allocation| allocation.total_recovery)
        .sum::<f64>();

    assert!((allocated + result.undistributed_estate - 0.3).abs() <= 1.0e-15);
    assert!(result
        .allocations
        .iter()
        .all(|allocation| allocation.total_recovery <= allocation.total_claim));
}

#[test]
fn duplicate_trimmed_claim_ids_are_rejected_deterministically() {
    let claims = vec![
        claim("duplicate", 1, 1.0, None, 0.0),
        claim(" duplicate ", 2, 1.0, None, 0.0),
    ];

    let error = allocate_recovery(2.0, &claims).expect_err("duplicate ids must fail");
    assert_eq!(
        error.to_string(),
        "Validation error: duplicate recovery claim id after trimming: 'duplicate'"
    );
}

#[test]
fn zero_estate_and_empty_or_zero_claims_are_well_defined() {
    let zero_estate = allocate_recovery(0.0, &[claim("zero", 1, 0.0, Some(0.0), 1.0)])
        .expect("zero values and inclusive haircut bounds are valid");
    assert_eq!(zero_estate.total_distributed, 0.0);
    assert_eq!(zero_estate.undistributed_estate, 0.0);
    assert_eq!(zero_estate.allocations[0].recovery_rate, 0.0);

    let no_claims = allocate_recovery(12.5, &[]).expect("empty claims are valid");
    assert_eq!(no_claims.total_distributed, 0.0);
    assert_eq!(no_claims.undistributed_estate, 12.5);
}

#[test]
fn invalid_amounts_haircuts_ids_and_collateral_accounting_are_rejected() {
    for estate in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(allocate_recovery(estate, &[]).is_err());
    }

    let mut invalid = claim("bad", 1, 1.0, None, 0.0);
    for amount in [-1.0, f64::NAN, f64::INFINITY] {
        invalid.principal = amount;
        assert!(allocate_recovery(10.0, &[invalid.clone()]).is_err());
    }

    invalid = claim("bad", 1, 1.0, Some(1.0), 0.0);
    for haircut in [-f64::EPSILON, 1.0 + f64::EPSILON, f64::NAN] {
        invalid.collateral_haircut = haircut;
        assert!(allocate_recovery(10.0, &[invalid.clone()]).is_err());
    }

    invalid = claim("", 1, 1.0, None, 0.0);
    assert!(allocate_recovery(10.0, &[invalid]).is_err());

    let collateral_exceeds_inclusive_estate = claim("secured", 1, 10.0, Some(10.0), 0.0);
    assert!(allocate_recovery(5.0, &[collateral_exceeds_inclusive_estate]).is_err());
}
