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
    from finstack_quant.portfolio import brinson_fachler

    result = json.loads(brinson_fachler(json.dumps(_period(0.60, 0.40, 0.08, 0.06, 0.01, 0.03))).to_json())

    reconstructed = result["total_allocation"] + result["total_selection"] + result["total_interaction"]
    assert reconstructed == pytest.approx(result["total_excess_return"], abs=1e-12)
    assert [sector["sector"] for sector in result["sectors"]] == ["A", "B"]


def test_carino_link_binding_reconstructs_compounded_active_return() -> None:
    """Per-period Brinson-Fachler results, Carino-linked, reconstruct the geometric active return."""
    from finstack_quant.portfolio import brinson_fachler, carino_link

    periods = [
        _period(0.70, 0.50, 0.10, 0.06, 0.04, 0.05),
        _period(0.60, 0.50, 0.02, 0.03, -0.01, 0.00),
    ]
    result = json.loads(carino_link([brinson_fachler(json.dumps(p)) for p in periods]).to_json())

    geometric_active = result["portfolio_return_compounded"] - result["benchmark_return_compounded"]
    reconstructed = result["linked_allocation"] + result["linked_selection"] + result["linked_interaction"]
    assert reconstructed == pytest.approx(geometric_active, abs=1e-10)
    assert [sector["sector"] for sector in result["linked_sectors"]] == ["A", "B"]


def test_carino_link_links_precomputed_results_not_raw_sectors() -> None:
    """``carino_link`` binds Rust ``carino_link`` (precomputed period results)."""
    from finstack_quant.portfolio import brinson_fachler, carino_link

    periods = [
        _period(0.70, 0.50, 0.10, 0.06, 0.04, 0.05),
        _period(0.60, 0.50, 0.02, 0.03, -0.01, 0.00),
    ]
    results = [json.loads(brinson_fachler(json.dumps(sectors)).to_json()) for sectors in periods]
    linked = carino_link(json.dumps(results))
    typed = carino_link([brinson_fachler(json.dumps(sectors)) for sectors in periods])
    assert linked.to_json() == typed.to_json()
    with pytest.raises(ValueError, match="Brinson period results"):
        carino_link(json.dumps(periods))


def test_brinson_linked_reconstructs_compounded_active_return() -> None:
    """``brinson_linked`` attributes raw sector periods and links them in one call."""
    from finstack_quant.portfolio import (
        CarinoLinkedAttribution,
        PortfolioError,
        brinson_fachler,
        brinson_linked,
        carino_link,
    )

    periods = [
        _period(0.70, 0.50, 0.10, 0.06, 0.04, 0.05),
        _period(0.60, 0.50, 0.02, 0.03, -0.01, 0.00),
        _period(0.40, 0.55, -0.03, -0.02, 0.05, 0.01),
    ]
    report = brinson_linked(periods)
    assert isinstance(report, CarinoLinkedAttribution)

    growth_p = growth_b = 1.0
    for sectors in periods:
        growth_p *= 1.0 + sum(s["portfolio_weight"] * s["portfolio_return"] for s in sectors)
        growth_b *= 1.0 + sum(s["benchmark_weight"] * s["benchmark_return"] for s in sectors)
    active = growth_p - growth_b
    assert report.portfolio_return_compounded - report.benchmark_return_compounded == pytest.approx(active, abs=1e-15)
    linked = report.linked_allocation + report.linked_selection + report.linked_interaction
    assert linked == pytest.approx(active, abs=1e-12)
    assert sum(s["total"] for s in report.linked_sectors) == pytest.approx(active, abs=1e-12)
    assert len(report.periods) == 3

    reference = carino_link([brinson_fachler(json.dumps(p)) for p in periods])
    assert report.to_json() == reference.to_json()
    assert brinson_linked(json.dumps(periods)).to_json() == report.to_json()

    bad = _period(0.60, 0.50, 0.02, 0.03, -0.01, 0.00)
    bad[0]["portfolio_weight"] = 0.9
    with pytest.raises(PortfolioError, match="period 1"):
        brinson_linked([periods[0], bad])
    with pytest.raises(PortfolioError, match="at least one period"):
        brinson_linked([])
    with pytest.raises(ValueError, match="Brinson sector periods"):
        brinson_linked([[{**periods[0][0], "bogus": 1.0}]])
