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
