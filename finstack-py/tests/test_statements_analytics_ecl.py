"""Statements analytics ECL binding tests."""

from __future__ import annotations

import pytest


def test_compute_ecl_weighted_validates_scenario_weights() -> None:
    """Weighted ECL rejects malformed scenario probabilities from Rust."""
    from finstack.statements_analytics import compute_ecl_weighted

    scenarios = [
        (0.75, [(0.0, 0.0), (1.0, 0.02)]),
        (0.20, [(0.0, 0.0), (1.0, 0.05)]),
    ]

    with pytest.raises(ValueError, match=r"scenario weights must sum to 1\.0"):
        compute_ecl_weighted(1_000_000.0, scenarios, 0.45, 0.06, 1.0)


def test_compute_ecl_weighted_returns_probability_weighted_ecl() -> None:
    """Weighted ECL binding delegates the scenario aggregation to Rust."""
    from finstack.statements_analytics import compute_ecl, compute_ecl_weighted

    base_curve = [(0.0, 0.0), (1.0, 0.02)]
    downside_curve = [(0.0, 0.0), (1.0, 0.05)]

    base = compute_ecl(1_000_000.0, base_curve, 0.45, 0.06, 1.0)
    downside = compute_ecl(1_000_000.0, downside_curve, 0.45, 0.06, 1.0)
    weighted = compute_ecl_weighted(
        1_000_000.0,
        [(0.70, base_curve), (0.30, downside_curve)],
        0.45,
        0.06,
        1.0,
    )

    assert weighted == pytest.approx(0.70 * base + 0.30 * downside, rel=1e-12)


def test_compute_ecl_weighted_anchors_unanchored_schedules() -> None:
    """Both ECL entry points accept schedules without an explicit (0, 0) knot."""
    from finstack.statements_analytics import compute_ecl, compute_ecl_weighted

    anchored = [(0.0, 0.0), (1.0, 0.02)]
    unanchored = [(1.0, 0.02)]

    assert compute_ecl(1_000_000.0, unanchored, 0.45, 0.06, 1.0) == pytest.approx(
        compute_ecl(1_000_000.0, anchored, 0.45, 0.06, 1.0)
    )
    assert compute_ecl_weighted(1_000_000.0, [(1.0, unanchored)], 0.45, 0.06, 1.0) == pytest.approx(
        compute_ecl_weighted(1_000_000.0, [(1.0, anchored)], 0.45, 0.06, 1.0)
    )


def test_compute_ecl_ead_schedule_reduces_lifetime_ecl() -> None:
    """An amortizing EAD profile lowers ECL versus a constant balance."""
    from finstack.statements_analytics import compute_ecl

    curve = [(0.0, 0.0), (5.0, 0.10)]
    bullet = compute_ecl(1_000_000.0, curve, 0.45, 0.06, 5.0, stage="stage2")
    amortizing = compute_ecl(
        1_000_000.0,
        curve,
        0.45,
        0.06,
        5.0,
        stage="stage2",
        ead_schedule=[(0.0, 1_000_000.0), (5.0, 0.0)],
    )
    assert amortizing < bullet


def test_compute_ecl_stage3_time_to_recovery() -> None:
    """Stage 3 ECL is discounted LGD x EAD over the recovery horizon."""
    from finstack.statements_analytics import compute_ecl

    curve = [(0.0, 0.0), (1.0, 0.02)]
    fast = compute_ecl(1_000_000.0, curve, 0.45, 0.06, 1.0, stage="stage3", stage3_time_to_recovery_years=0.5)
    slow = compute_ecl(1_000_000.0, curve, 0.45, 0.06, 1.0, stage="stage3", stage3_time_to_recovery_years=3.0)
    # Longer time to recovery discounts the loss more heavily.
    assert slow < fast
    assert fast <= 0.45 * 1_000_000.0
