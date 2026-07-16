"""Behavioral tests for the Python recovery-waterfall bindings."""

from __future__ import annotations

import pytest

from finstack_quant.core.credit.recovery_waterfall import (
    RecoveryAllocation,
    RecoveryClaim,
    RecoveryWaterfallResult,
    allocate_recovery,
)


def test_recovery_waterfall_delegates_collateral_and_priority_allocation() -> None:
    claims = [
        RecoveryClaim(
            "secured",
            "first_lien",
            1,
            100.0,
            collateral=(60.0, 0.25),
        ),
        RecoveryClaim("peer", "first_lien", 1, 100.0),
        RecoveryClaim("junior", "subordinated", 2, 50.0),
    ]

    result = allocate_recovery(100.0, claims)

    assert isinstance(result, RecoveryWaterfallResult)
    assert result.total_distributed == pytest.approx(100.0)
    assert result.undistributed_estate == 0.0
    assert result.apr_satisfied
    assert [allocation.id for allocation in result.allocations] == [
        "secured",
        "peer",
        "junior",
    ]
    assert all(isinstance(allocation, RecoveryAllocation) for allocation in result.allocations)
    assert result.allocations[0].collateral_recovery == pytest.approx(45.0)
    assert result.allocations[0].total_claim == 100.0
    assert result.allocations[2].total_recovery == 0.0


def test_recovery_waterfall_maps_rust_validation_errors() -> None:
    claim = RecoveryClaim("bad", "first_lien", 1, -1.0)

    with pytest.raises(ValueError, match="principal"):
        allocate_recovery(10.0, [claim])


def test_recovery_waterfall_accepts_decimal_collateral_rounding() -> None:
    claims = [
        RecoveryClaim("first", "first_lien", 1, 0.1, collateral=(0.1, 0.0)),
        RecoveryClaim("second", "first_lien", 1, 0.2, collateral=(0.2, 0.0)),
    ]

    result = allocate_recovery(0.3, claims)

    recovered = sum(allocation.total_recovery for allocation in result.allocations)
    assert recovered + result.undistributed_estate == pytest.approx(0.3, abs=1.0e-15)
    assert all(allocation.total_recovery <= allocation.total_claim for allocation in result.allocations)


def test_recovery_waterfall_rejects_duplicate_trimmed_ids() -> None:
    claims = [
        RecoveryClaim("duplicate", "first_lien", 1, 1.0),
        RecoveryClaim(" duplicate ", "subordinated", 2, 1.0),
    ]

    with pytest.raises(
        ValueError,
        match="duplicate recovery claim id after trimming: 'duplicate'",
    ):
        allocate_recovery(2.0, claims)
