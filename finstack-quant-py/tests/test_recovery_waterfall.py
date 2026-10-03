"""Behavioral tests for the Python recovery-waterfall bindings."""

from __future__ import annotations

import pytest

from finstack_quant.models.credit.recovery_waterfall import (
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
            0.0,
            0.0,
            0.25,
            collateral_value=60.0,
        ),
        RecoveryClaim("peer", "first_lien", 1, 100.0, 0.0, 0.0, 0.0),
        RecoveryClaim("junior", "subordinated", 2, 50.0, 0.0, 0.0, 0.0),
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
    claim = RecoveryClaim("bad", "first_lien", 1, -1.0, 0.0, 0.0, 0.0)

    with pytest.raises(ValueError, match="principal"):
        allocate_recovery(10.0, [claim])


def test_recovery_waterfall_accepts_decimal_collateral_rounding() -> None:
    claims = [
        RecoveryClaim("first", "first_lien", 1, 0.1, 0.0, 0.0, 0.0, collateral_value=0.1),
        RecoveryClaim("second", "first_lien", 1, 0.2, 0.0, 0.0, 0.0, collateral_value=0.2),
    ]

    result = allocate_recovery(0.3, claims)

    recovered = sum(allocation.total_recovery for allocation in result.allocations)
    assert recovered + result.undistributed_estate == pytest.approx(0.3, abs=1.0e-15)
    assert all(allocation.total_recovery <= allocation.total_claim for allocation in result.allocations)


def test_recovery_claim_and_allocation_round_trip() -> None:
    import pickle

    claim = RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.25, collateral_value=60.0)
    assert RecoveryClaim.from_json(claim.to_json()) == claim
    assert pickle.loads(pickle.dumps(claim))  # noqa: S301 - trusted in-process round trip == claim
    assert repr(claim) == 'RecoveryClaim(id="SEN", seniority="secured", priority=1, total_claim=100)'

    allocation = allocate_recovery(100.0, [claim]).allocations[0]
    assert RecoveryAllocation.from_json(allocation.to_json()) == allocation
    frame = allocation.to_dataframe()
    assert len(frame) == 1
    assert frame["id"].iloc[0] == "SEN"
    assert frame["collateral_recovery"].iloc[0] == pytest.approx(45.0)


def test_recovery_waterfall_rejects_duplicate_trimmed_ids() -> None:
    claims = [
        RecoveryClaim("duplicate", "first_lien", 1, 1.0, 0.0, 0.0, 0.0),
        RecoveryClaim(" duplicate ", "subordinated", 2, 1.0, 0.0, 0.0, 0.0),
    ]

    with pytest.raises(
        ValueError,
        match="duplicate recovery claim id after trimming: 'duplicate'",
    ):
        allocate_recovery(2.0, claims)


def test_recovery_claim_requires_accrued_penalties_and_haircut() -> None:
    """MODA-006: no binding-invented zeros; the Rust wire form requires all three."""
    with pytest.raises(TypeError):
        RecoveryClaim("SEN", "secured", 1, 100.0, collateral_value=50.0)  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0)  # type: ignore[call-arg]
    with pytest.raises(ValueError, match="collateral_haircut"):
        RecoveryClaim.from_json(
            '{"id":"A","seniority":"s","priority":1,"principal":100.0,"accrued":0.0,"penalties":0.0}'
        )
    claim = RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.3, collateral_value=50.0)
    other = RecoveryClaim("JUN", "unsecured", 2, 100.0, 0.0, 0.0, 0.0)
    assert allocate_recovery(60.0, [claim, other]).allocations[0].collateral_recovery == pytest.approx(35.0)
