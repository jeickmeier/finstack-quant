"""Portfolio Brinson-Fachler binding tests."""

from __future__ import annotations

import json

import pytest


def _period(
    wp_a: float,
    wb_a: float,
    rp_a: float,
    rb_a: float,
    rp_b: float,
    rb_b: float,
) -> list[dict[str, float | str]]:
    return [
        {
            "sector": "A",
            "portfolio_weight": wp_a,
            "benchmark_weight": wb_a,
            "portfolio_return": rp_a,
            "benchmark_return": rb_a,
        },
        {
            "sector": "B",
            "portfolio_weight": 1.0 - wp_a,
            "benchmark_weight": 1.0 - wb_a,
            "portfolio_return": rp_b,
            "benchmark_return": rb_b,
        },
    ]


def test_brinson_fachler_binding_reconstructs_active_return() -> None:
    """The Python binding exposes Brinson-Fachler attribution as JSON."""
    from finstack.portfolio import brinson_fachler

    result = json.loads(brinson_fachler(json.dumps(_period(0.60, 0.40, 0.08, 0.06, 0.01, 0.03))))

    reconstructed = result["total_allocation"] + result["total_selection"] + result["total_interaction"]
    assert reconstructed == pytest.approx(result["total_excess_return"], abs=1e-12)
    assert [sector["sector"] for sector in result["sectors"]] == ["A", "B"]


def test_carino_link_binding_reconstructs_compounded_active_return() -> None:
    """The Python binding exposes multi-period Carino linked attribution."""
    from finstack.portfolio import carino_link

    periods = [
        _period(0.70, 0.50, 0.10, 0.06, 0.04, 0.05),
        _period(0.60, 0.50, 0.02, 0.03, -0.01, 0.00),
    ]
    result = json.loads(carino_link(json.dumps(periods)))

    geometric_active = result["portfolio_return_compounded"] - result["benchmark_return_compounded"]
    reconstructed = result["linked_allocation"] + result["linked_selection"] + result["linked_interaction"]
    assert reconstructed == pytest.approx(geometric_active, abs=1e-10)
    assert [sector["sector"] for sector in result["linked_sectors"]] == ["A", "B"]
