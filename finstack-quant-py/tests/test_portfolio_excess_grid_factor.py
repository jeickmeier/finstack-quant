"""Bindings for Task 8: credit excess returns, grid attribution, factor-Brinson.

Covers ``finstack_quant.portfolio.{cell_returns_from_reference,
cell_returns_from_curves, excess_returns, grid_attribution, grid_carino_link,
factor_brinson_attribution}`` and ``finstack_quant.analytics.constrained_least_squares``.

Every function gets: one golden re-pin against a value independently verified
in the Rust test suite, one keyword-argument call using the exact
``text_signature`` names, one malformed-JSON -> ``ValueError`` case (where the
function takes JSON), one domain error -> ``PortfolioError``/``AnalyticsError``
with the message content asserted, and one ``deny_unknown_fields`` rejection
(where the function takes JSON).
"""

from __future__ import annotations

from datetime import date
import json
import math
import sys

import pytest

from finstack_quant.analytics import AnalyticsError, constrained_least_squares
from finstack_quant.core.market_data import DiscountCurve
from finstack_quant.portfolio import (
    PortfolioError,
    cell_returns_from_curves,
    cell_returns_from_reference,
    excess_returns,
    factor_brinson_attribution,
    grid_attribution,
    grid_carino_link,
)

# Shared fixtures

_BASE_DATE = date(2024, 1, 1)


def _ref(duration: float, total_return: float) -> dict[str, float]:
    return {"duration": duration, "total_return": total_return}


def _lehman_b1_cells() -> list[dict[str, float]]:
    """Populated cells of Lehman Figure B-1 (May 1997 Treasury returns)."""
    cells = [
        (0.25, 0.0054),
        (0.75, 0.0059),
        (1.25, 0.0066),
        (1.75, 0.0071),
        (2.25, 0.0073),
        (2.75, 0.0075),
        (3.25, 0.0078),
        (3.75, 0.0078),
        (4.25, 0.0080),
        (4.75, 0.0092),
        (5.25, 0.0097),
        (5.75, 0.0103),
        (6.25, 0.0111),
        (6.75, 0.0105),
        (7.25, 0.0105),
        (9.25, 0.0110),
        (9.75, 0.0111),
        (10.25, 0.0111),
        (10.75, 0.0115),
        (11.25, 0.0118),
        (11.75, 0.0121),
        (12.25, 0.0116),
    ]
    return [_ref(d, r) for d, r in cells]


def _lehman_b1_table_json() -> str:
    return cell_returns_from_reference(
        json.dumps(_lehman_b1_cells()),
        "UST",
        json.dumps({"width": 0.5}),
    ).to_json()


def _gpos(cell: str, sector: str, weight: float, total_return: float) -> dict[str, float | str]:
    return {"cell": cell, "sector": sector, "weight": weight, "total_return": total_return}


def _grid_golden_portfolio() -> list[dict[str, float | str]]:
    return [
        _gpos("0.0-3.0", "GOVT", 0.20, 0.012),
        _gpos("0.0-3.0", "CORP", 0.20, 0.025),
        _gpos("3.0-6.0", "GOVT", 0.30, 0.028),
        _gpos("3.0-6.0", "CORP", 0.30, 0.045),
    ]


def _grid_golden_benchmark() -> list[dict[str, float | str]]:
    return [
        _gpos("0.0-3.0", "GOVT", 0.30, 0.010),
        _gpos("0.0-3.0", "CORP", 0.20, 0.020),
        _gpos("3.0-6.0", "GOVT", 0.25, 0.030),
        _gpos("3.0-6.0", "CORP", 0.25, 0.040),
    ]


def _factor_brinson_binary_input() -> dict[str, object]:
    """Paper Exhibits 1-2 (Jeet & Partani 2023), binary sector-indicator exposures."""
    return {
        "asset_ids": ["A", "B", "C"],
        "asset_returns": [0.05, 0.02, 0.01],
        # Rows: A -> Healthcare only, B -> Energy only, C -> Healthcare only.
        "exposures": [0.0, 1.0, 1.0, 0.0, 0.0, 1.0],
        "factor_names": ["Energy", "Healthcare"],
        "portfolio_weights": [1.25, -0.30, 0.05],
        "benchmark_weights": [0.60, 0.30, 0.10],
    }


# cell_returns_from_reference tests


def test_cell_returns_from_reference_matches_rust_interpolation_golden() -> None:
    """Re-pins the Rust `cell_table_interpolates_interior_and_extrapolates_ends` golden."""
    reference = [_ref(0.25, 0.01), _ref(2.25, 0.05)]
    table = json.loads(cell_returns_from_reference(json.dumps(reference), "UST", json.dumps({"width": 0.5})).to_json())

    assert table["base_label"] == "UST"
    assert len(table["cells"]) == 5
    assert table["cells"][1]["label"] == "0.5-1.0"
    returns = [c["base_return"] for c in table["cells"]]
    for got, want in zip(returns, [0.01, 0.02, 0.03, 0.04, 0.05], strict=True):
        assert got == pytest.approx(want, abs=1e-12)
    observed = [c["observed"] for c in table["cells"]]
    assert observed == [True, False, False, False, True]


def test_cell_returns_from_reference_accepts_keyword_arguments() -> None:
    """The advertised ``text_signature`` names are the real keyword names."""
    reference = [_ref(0.25, 0.01), _ref(2.25, 0.05)]
    table = json.loads(
        cell_returns_from_reference(
            reference_json=json.dumps(reference),
            base_label="UST",
            config_json=json.dumps({"width": 0.5}),
        ).to_json()
    )
    assert table["base_label"] == "UST"


def test_cell_returns_from_reference_rejects_malformed_json() -> None:
    """Malformed JSON in either JSON argument raises ``ValueError``."""
    with pytest.raises(ValueError, match="reference JSON"):
        cell_returns_from_reference("not json", "UST", json.dumps({"width": 0.5})).to_json()
    with pytest.raises(ValueError, match="config JSON"):
        cell_returns_from_reference(json.dumps([_ref(1.0, 0.01)]), "UST", "not json").to_json()


def test_cell_returns_from_reference_rejects_colliding_width_labels() -> None:
    """A width like 0.03 collides on the one-decimal label; the error names both."""
    reference = [_ref(0.1, 0.01)]
    with pytest.raises(PortfolioError) as exc_info:
        cell_returns_from_reference(json.dumps(reference), "UST", json.dumps({"width": 0.03})).to_json()
    message = str(exc_info.value)
    assert "0.03" in message
    assert "0.1-0.1" in message


def test_cell_returns_from_reference_rejects_astronomically_large_duration() -> None:
    """FFI-level regression for the Minor-2 finding: a units mistake (e.g.

    days instead of years) producing an astronomically large `duration`
    must fail closed with a clear `PortfolioError`, not crash the process
    with a Rust panic ("capacity overflow") while allocating the duration
    grid. Re-pins the Rust `astronomically_large_duration_fails_closed_not_panics`
    fixture through the Python binding.
    """
    reference = [_ref(1e30, 0.01)]
    with pytest.raises(PortfolioError, match="sanity bound"):
        cell_returns_from_reference(json.dumps(reference), "UST", json.dumps({"width": 1.0})).to_json()


def test_cell_returns_from_reference_denies_unknown_fields() -> None:
    """An unknown field on a ``ReferenceReturn`` entry fails closed."""
    bad_reference = [{"duration": 1.0, "total_return": 0.01, "surprise": 1.0}]
    with pytest.raises(ValueError, match="reference JSON"):
        cell_returns_from_reference(json.dumps(bad_reference), "UST", json.dumps({"width": 0.5})).to_json()


# cell_returns_from_curves tests


def _flat_ust_curve() -> DiscountCurve:
    # 4% flat: matches the Rust `flat_curve_cell_returns_equal_pure_carry` fixture.
    knots = [
        (0.0, 1.0),
        (0.25, 0.99004983),
        (0.5, 0.98019867),
        (1.25, 0.95122942),
        (1.5, 0.94176453),
    ]
    return DiscountCurve("UST", _BASE_DATE, knots)


def test_cell_returns_from_curves_matches_rust_flat_curve_golden() -> None:
    """Re-pins the Rust `flat_curve_cell_returns_equal_pure_carry` golden."""
    curve = _flat_ust_curve()
    table = json.loads(cell_returns_from_curves(curve, curve, 0.25, 2.0, "UST", json.dumps({"width": 1.0})).to_json())

    assert table["base_label"] == "UST"
    assert len(table["cells"]) == 2
    for cell in table["cells"]:
        assert cell["base_return"] == pytest.approx(0.01005017, abs=1e-6)
        assert cell["observed"] is True


def test_cell_returns_from_curves_distinguishes_start_and_end_curves() -> None:
    """`start` and `end` are not interchangeable: re-pins the Rust `rising_rates_hurt_the_longer_cell_more` golden.

    Uses genuinely different start/end curves (unlike the flat-curve golden
    above, where ``start == end`` and a ``start``/``end`` argument swap would
    be invisible). Start 4% flat, end 5% flat.
    """
    start = DiscountCurve("UST", _BASE_DATE, [(0.0, 1.0), (0.5, 0.98019867), (1.5, 0.94176453)])
    end = DiscountCurve("UST", _BASE_DATE, [(0.0, 1.0), (0.25, 0.98757780), (1.25, 0.93941306)])
    table = json.loads(cell_returns_from_curves(start, end, 0.25, 2.0, "UST", json.dumps({"width": 1.0})).to_json())

    assert table["cells"][0]["base_return"] == pytest.approx(0.0075281983, abs=1e-8)
    assert table["cells"][1]["base_return"] == pytest.approx(-0.00249688, abs=1e-8)


def test_cell_returns_from_curves_accepts_keyword_arguments() -> None:
    """The advertised ``text_signature`` names are the real keyword names."""
    curve = _flat_ust_curve()
    table = json.loads(
        cell_returns_from_curves(
            start=curve,
            end=curve,
            horizon_years=0.25,
            max_duration=2.0,
            base_label="UST",
            config_json=json.dumps({"width": 1.0}),
        ).to_json()
    )
    assert len(table["cells"]) == 2


def test_cell_returns_from_curves_rejects_malformed_config_json() -> None:
    """Malformed ``config_json`` raises ``ValueError``."""
    curve = _flat_ust_curve()
    with pytest.raises(ValueError, match="config JSON"):
        cell_returns_from_curves(curve, curve, 0.25, 2.0, "UST", "not json").to_json()


def test_cell_returns_from_curves_rejects_cell_maturing_inside_period() -> None:
    """A cell whose midpoint sits inside the holding period fails closed."""
    knots = [(0.0, 1.0), (0.25, 0.99004983), (0.5, 0.98019867)]
    curve = DiscountCurve("UST", _BASE_DATE, knots)
    with pytest.raises(PortfolioError, match="mature"):
        cell_returns_from_curves(curve, curve, 0.25, 0.5, "UST", json.dumps({"width": 0.25})).to_json()


def test_cell_returns_from_curves_denies_unknown_config_fields() -> None:
    """An unknown field on ``CellConfig`` fails closed."""
    curve = _flat_ust_curve()
    with pytest.raises(ValueError, match="config JSON"):
        cell_returns_from_curves(curve, curve, 0.25, 2.0, "UST", json.dumps({"width": 1.0, "surprise": 1.0})).to_json()


# excess_returns tests


def test_excess_returns_matches_lehman_figure_b2_golden() -> None:
    """Reproduces Dynkin, Hyman & Vankudre (1998), Figure B-2 exactly."""
    table_json = _lehman_b1_table_json()
    positions = [
        {"id": "Colombia", "weight": 0.2, "duration": 5.16, "total_return": 0.0225},
        {"id": "RiteAid", "weight": 0.2, "duration": 6.71, "total_return": 0.0172},
        {"id": "NewsAM", "weight": 0.2, "duration": 9.58, "total_return": 0.0182},
        {"id": "Delta", "weight": 0.2, "duration": 9.81, "total_return": 0.0102},
        {"id": "Quebec", "weight": 0.2, "duration": 11.08, "total_return": 0.0185},
    ]
    result = json.loads(excess_returns(json.dumps(positions), table_json).to_json())

    expected = [0.0128, 0.0067, 0.0071, -0.0009, 0.0067]
    for got, want in zip(result["positions"], expected, strict=True):
        assert got["excess_return"] == pytest.approx(want, abs=1e-12)
    assert result["positions"][0]["cell"] == "5.0-5.5"
    assert result["portfolio_excess_return"] == pytest.approx(0.00648, abs=1e-12)
    assert result["portfolio_total_return"] == pytest.approx(0.01732, abs=1e-12)
    assert result["portfolio_base_return"] == pytest.approx(0.01084, abs=1e-12)


def test_excess_returns_accepts_keyword_arguments() -> None:
    """The advertised ``text_signature`` names are the real keyword names."""
    table_json = _lehman_b1_table_json()
    positions = [{"id": "Colombia", "weight": 1.0, "duration": 5.16, "total_return": 0.0225}]
    result = json.loads(excess_returns(positions_json=json.dumps(positions), table_json=table_json).to_json())
    assert result["positions"][0]["id"] == "Colombia"


def test_excess_returns_rejects_malformed_json() -> None:
    """Malformed JSON in either argument raises ``ValueError``."""
    table_json = _lehman_b1_table_json()
    with pytest.raises(ValueError, match="positions JSON"):
        excess_returns("not json", table_json).to_json()
    positions = [{"id": "A", "weight": 1.0, "duration": 1.0, "total_return": 0.01}]
    with pytest.raises(ValueError, match="table JSON"):
        excess_returns(json.dumps(positions), "not json").to_json()


def test_excess_returns_rejects_duration_outside_table_range() -> None:
    """A position duration outside the table's covered range names the position."""
    table_json = _lehman_b1_table_json()
    positions = [{"id": "X", "weight": 1.0, "duration": 99.0, "total_return": 0.01}]
    with pytest.raises(PortfolioError, match="X"):
        excess_returns(json.dumps(positions), table_json).to_json()


def test_excess_returns_validates_inbound_cell_table_structure() -> None:
    """Hand-built tables are validated before any duration lookup."""
    positions = [{"id": "P", "weight": 1.0, "duration": 0.75, "total_return": 0.03}]
    overlapping = {
        "base_label": "CUSTOM",
        "cells": [
            {"label": "first", "lower": 0.0, "upper": 1.0, "base_return": 0.01, "observed": True},
            {"label": "second", "lower": 0.5, "upper": 1.5, "base_return": 0.02, "observed": True},
        ],
    }
    with pytest.raises(PortfolioError, match=r"cell\[1\].*overlaps"):
        excess_returns(json.dumps(positions), json.dumps(overlapping)).to_json()


@pytest.mark.parametrize(
    ("labels", "expected"),
    [
        (["", "long"], r"cell\[0\].*non-empty"),
        (["duplicate", "duplicate"], r"cell\[1\].*unique"),
    ],
)
def test_excess_returns_rejects_invalid_inbound_cell_labels(labels: list[str], expected: str) -> None:
    """Inbound cell labels are non-empty and unique before duration lookup."""
    positions = [{"id": "P", "weight": 1.0, "duration": 0.25, "total_return": 0.03}]
    table = {
        "base_label": "CUSTOM",
        "cells": [
            {"label": labels[0], "lower": 0.0, "upper": 1.0, "base_return": 0.01, "observed": True},
            {"label": labels[1], "lower": 1.0, "upper": 2.0, "base_return": 0.02, "observed": True},
        ],
    }

    with pytest.raises(PortfolioError, match=expected):
        excess_returns(json.dumps(positions), json.dumps(table)).to_json()


def test_excess_returns_denies_unknown_position_fields() -> None:
    """An unknown field on an ``ExcessReturnPosition`` entry fails closed."""
    table_json = _lehman_b1_table_json()
    bad_positions = [{"id": "A", "weight": 1.0, "duration": 1.0, "total_return": 0.01, "surprise": 1.0}]
    with pytest.raises(ValueError, match="positions JSON"):
        excess_returns(json.dumps(bad_positions), table_json).to_json()


# grid_attribution tests


def test_grid_attribution_matches_hand_derived_golden() -> None:
    """Re-pins the Rust `grid_attribution_matches_hand_derived_golden` fixture."""
    result = json.loads(
        grid_attribution(json.dumps(_grid_golden_portfolio()), json.dumps(_grid_golden_benchmark())).to_json()
    )

    assert result["portfolio_return"] == pytest.approx(0.0293, abs=1e-12)
    assert result["benchmark_return"] == pytest.approx(0.0245, abs=1e-12)
    assert result["active_return"] == pytest.approx(0.0048, abs=1e-12)
    assert result["total_curve"] == pytest.approx(0.0021, abs=1e-12)
    assert result["total_sector"] == pytest.approx(0.0004, abs=1e-12)
    assert result["total_selection"] == pytest.approx(0.0023, abs=1e-12)
    reconstructed = result["total_curve"] + result["total_sector"] + result["total_selection"]
    assert reconstructed == pytest.approx(result["active_return"], abs=1e-12)


def test_grid_attribution_accepts_keyword_arguments() -> None:
    """The advertised ``text_signature`` names are the real keyword names."""
    result = json.loads(
        grid_attribution(
            portfolio_json=json.dumps(_grid_golden_portfolio()),
            benchmark_json=json.dumps(_grid_golden_benchmark()),
        ).to_json()
    )
    assert result["active_return"] == pytest.approx(0.0048, abs=1e-12)


def test_grid_attribution_rejects_malformed_json() -> None:
    """Malformed JSON in either argument raises ``ValueError``."""
    with pytest.raises(ValueError, match="grid portfolio JSON"):
        grid_attribution("not json", json.dumps(_grid_golden_benchmark())).to_json()
    with pytest.raises(ValueError, match="grid benchmark JSON"):
        grid_attribution(json.dumps(_grid_golden_portfolio()), "not json").to_json()


def test_grid_attribution_rejects_zero_net_weight_bucket() -> None:
    """A bucket netting to exactly zero weight fails closed, naming bucket and side."""
    portfolio = [
        _gpos("0.0-3.0", "GOVT", 0.5, 0.01),
        _gpos("0.0-3.0", "GOVT", -0.5, 0.02),
        _gpos("0.0-3.0", "CORP", 1.0, 0.03),
    ]
    benchmark = [_gpos("0.0-3.0", "CORP", 1.0, 0.025)]

    with pytest.raises(PortfolioError) as exc_info:
        grid_attribution(json.dumps(portfolio), json.dumps(benchmark)).to_json()
    message = str(exc_info.value)
    assert "0.0-3.0" in message
    assert "GOVT" in message
    assert "Portfolio" in message


def test_grid_attribution_rejects_near_zero_net_weight_bucket() -> None:
    """FFI-level regression for the whole-branch review's Important-1 finding.

    Re-pins the Rust `near_zero_net_weight_bucket_fails_closed_before_rate_blows_up`
    fixture through the Python binding: a benchmark cell "X" holding GOVT
    +0.5@2% and CORP -(0.5 - eps)@1% nets to `eps`, not `0.0`. An
    exact-equality-only guard would let this through and
    `weighted_return / weight` would then blow up without bound as `eps`
    shrinks. `eps=1e-8` (ratio ~1e-8, far below the 1e-6 relative bound)
    must be rejected, naming the cell and side.
    """
    portfolio = [_gpos("X", "GOVT", 0.5, 0.018), _gpos("Y", "GOVT", 0.5, 0.02)]
    eps = 1e-8
    benchmark = [
        _gpos("X", "GOVT", 0.5, 0.02),
        _gpos("X", "CORP", -(0.5 - eps), 0.01),
        _gpos("Y", "GOVT", 1.0 - eps, 0.015),
    ]

    with pytest.raises(PortfolioError) as exc_info:
        grid_attribution(json.dumps(portfolio), json.dumps(benchmark)).to_json()
    message = str(exc_info.value)
    assert "bucket 'X'" in message
    assert "Benchmark" in message


def test_grid_attribution_denies_unknown_position_fields() -> None:
    """An unknown field on a ``GridPosition`` entry fails closed."""
    bad_portfolio = [{**_grid_golden_portfolio()[0], "surprise": 1.0}]
    with pytest.raises(ValueError, match="grid portfolio JSON"):
        grid_attribution(json.dumps(bad_portfolio), json.dumps(_grid_golden_benchmark())).to_json()


# grid_carino_link tests


def _grid_golden_period_json() -> str:
    return grid_attribution(json.dumps(_grid_golden_portfolio()), json.dumps(_grid_golden_benchmark())).to_json()


def test_grid_carino_link_reconstructs_compounded_active_return() -> None:
    """Re-pins the Rust `linked_effects_reconstruct_compounded_active_return` fixture."""
    period = json.loads(_grid_golden_period_json())
    linked = json.loads(grid_carino_link(json.dumps([period, period])).to_json())

    assert linked["portfolio_return_compounded"] == pytest.approx(0.05945849, abs=1e-8)
    assert linked["benchmark_return_compounded"] == pytest.approx(0.04960025, abs=1e-8)

    total = linked["linked_curve"] + linked["linked_sector"] + linked["linked_selection"]
    geometric_active = linked["portfolio_return_compounded"] - linked["benchmark_return_compounded"]
    assert total == pytest.approx(geometric_active, abs=1e-12)

    # Carino smoothing must not be a no-op: arithmetic != geometric active return.
    arithmetic_active = 2.0 * 0.0048
    assert abs(arithmetic_active - geometric_active) > 1e-6

    # Scale recomputed independently from the Carino formula, matching the Rust
    # test's own recomputation rather than a value pulled from the code under
    # test. Pins each linked total against its per-period effect x scale, which
    # also catches a linked_sector/linked_selection swap (0.0004 != 0.0023).
    k = (math.log(1.0293) - math.log(1.0245)) / 0.0048
    kk = (math.log(1.05945849) - math.log(1.04960025)) / (0.05945849 - 0.04960025)
    scale = k / kk
    assert linked["linked_curve"] == pytest.approx(2.0 * scale * 0.0021, abs=1e-10)
    assert linked["linked_sector"] == pytest.approx(2.0 * scale * 0.0004, abs=1e-10)
    assert linked["linked_selection"] == pytest.approx(2.0 * scale * 0.0023, abs=1e-10)
    assert len(linked["periods"]) == 2


def test_grid_carino_link_accepts_keyword_arguments() -> None:
    """The advertised ``text_signature`` name is the real keyword name."""
    period = json.loads(_grid_golden_period_json())
    linked = json.loads(grid_carino_link(periods_json=json.dumps([period, period])).to_json())
    assert len(linked["periods"]) == 2


def test_grid_carino_link_rejects_malformed_json() -> None:
    """Malformed JSON raises ``ValueError``."""
    with pytest.raises(ValueError, match="grid periods JSON"):
        grid_carino_link("not json").to_json()


def test_grid_carino_link_rejects_empty_periods() -> None:
    """An empty periods array fails closed, naming the requirement."""
    with pytest.raises(PortfolioError, match="at least one period"):
        grid_carino_link(json.dumps([])).to_json()


def test_grid_carino_link_rejects_inconsistent_period_contract() -> None:
    """Finite but tampered returns/effects fail before Carino scaling."""
    period = json.loads(_grid_golden_period_json())

    bad_active = {**period, "active_return": period["active_return"] + 0.001}
    with pytest.raises(PortfolioError, match="active_return"):
        grid_carino_link(json.dumps([bad_active])).to_json()

    bad_total = {**period, "total_selection": period["total_selection"] + 0.001}
    with pytest.raises(PortfolioError, match="effect totals"):
        grid_carino_link(json.dumps([bad_total])).to_json()


def test_grid_carino_link_denies_unknown_period_fields() -> None:
    """An unknown top-level field on a ``GridAttributionResult`` period fails closed."""
    period = json.loads(_grid_golden_period_json())
    bad_period = {**period, "surprise": 1.0}
    with pytest.raises(ValueError, match="grid periods JSON"):
        grid_carino_link(json.dumps([bad_period])).to_json()


# factor_brinson_attribution tests


def test_factor_brinson_attribution_matches_binary_golden() -> None:
    """Re-pins the Rust `binary_factors_reproduce_brinson_fachler_exactly` golden."""
    input_json = json.dumps(_factor_brinson_binary_input())
    f_b = [0.02, 0.31 / 7.0]
    result = json.loads(factor_brinson_attribution(input_json, f_b).to_json())

    assert result["active_return"] == pytest.approx(0.02, abs=1e-12)
    assert result["allocation"] == pytest.approx(0.0145714285714286, abs=1e-12)
    assert result["selection"] == pytest.approx(0.0054285714285714, abs=1e-12)
    assert result["allocation"] + result["selection"] == pytest.approx(result["active_return"], abs=1e-12)


def test_factor_brinson_attribution_accepts_keyword_arguments() -> None:
    """The advertised ``text_signature`` names are the real keyword names."""
    input_json = json.dumps(_factor_brinson_binary_input())
    result = json.loads(factor_brinson_attribution(input_json=input_json, factor_returns=[0.02, 0.31 / 7.0]).to_json())
    assert result["allocation"] == pytest.approx(0.0145714285714286, abs=1e-12)


def test_factor_brinson_attribution_rejects_malformed_json() -> None:
    """Malformed ``input_json`` raises ``ValueError``."""
    with pytest.raises(ValueError, match="factor-Brinson input JSON"):
        factor_brinson_attribution("not json", [0.02, 0.01]).to_json()


def test_factor_brinson_attribution_rejects_incomplete_factor_returns() -> None:
    """`factor_returns` that fail the completeness condition fail closed."""
    input_data = {
        "asset_ids": ["A", "B", "C"],
        "asset_returns": [0.05, 0.02, 0.01],
        "exposures": [1.2, -0.8, 0.5, 1.2, -0.7, 0.7],
        "factor_names": ["Energy", "Healthcare"],
        "portfolio_weights": [1.25, -0.30, 0.05],
        "benchmark_weights": [0.60, 0.30, 0.10],
    }
    with pytest.raises(PortfolioError, match="constrained_least_squares"):
        factor_brinson_attribution(json.dumps(input_data), [0.05, 0.01]).to_json()


def test_factor_brinson_attribution_denies_unknown_input_fields() -> None:
    """An unknown field on ``FactorBrinsonInput`` fails closed."""
    bad_input = {**_factor_brinson_binary_input(), "surprise": 1.0}
    with pytest.raises(ValueError, match="factor-Brinson input JSON"):
        factor_brinson_attribution(json.dumps(bad_input), [0.02, 0.31 / 7.0]).to_json()


# analytics constrained_least_squares tests


def test_constrained_least_squares_matches_binary_golden() -> None:
    """Re-pins the Rust `binary_factor_case_matches_hand_closed_form` golden."""
    exposures = [0.0, 1.0, 1.0, 0.0, 0.0, 1.0]
    f = constrained_least_squares(exposures, 2, [0.05, 0.02, 0.01], [0.6, 0.3, 0.1])

    assert f[0] == pytest.approx(0.0289552239, abs=1e-9)
    assert f[1] == pytest.approx(0.0404477612, abs=1e-9)


def test_constrained_least_squares_accepts_keyword_arguments() -> None:
    """The advertised ``text_signature`` names are the real keyword names."""
    f = constrained_least_squares(
        exposures=[0.0, 1.0, 1.0, 0.0, 0.0, 1.0],
        n_factors=2,
        returns=[0.05, 0.02, 0.01],
        weights=[0.6, 0.3, 0.1],
    )
    assert f[0] == pytest.approx(0.0289552239, abs=1e-9)


def test_constrained_least_squares_documents_integer_conversion_errors() -> None:
    """The host-language integer contract names its pre-Rust failures."""
    doc = constrained_least_squares.__doc__ or ""
    assert "positive integer" in doc
    assert "TypeError" in doc
    assert "OverflowError" in doc

    with pytest.raises(TypeError):
        constrained_least_squares([], 1.5, [], [])
    with pytest.raises(OverflowError):
        constrained_least_squares([], -1, [], [])


def test_constrained_least_squares_rejects_orthogonal_weights() -> None:
    """`w` orthogonal to the constraint direction cannot be restored: fails closed."""
    with pytest.raises(AnalyticsError, match=r"(?i)constraint"):
        constrained_least_squares([1.0, -1.0], 1, [0.02, 0.01], [0.5, 0.5])


def test_constrained_least_squares_returns_ols_when_constraint_is_already_met() -> None:
    """A degenerate correction direction is harmless when OLS already closes."""
    f = constrained_least_squares([1.0, -1.0], 1, [0.02, -0.02], [0.5, 0.5])
    assert f == pytest.approx([0.02], abs=1e-15)


def test_constrained_least_squares_maps_overflowing_dimensions() -> None:
    """A valid Python ``usize`` that overflows the matrix product stays a domain error."""
    with pytest.raises(AnalyticsError, match=r"(?i)dimension"):
        constrained_least_squares([], sys.maxsize + 1, [0.02, 0.01], [0.5, 0.5])


def test_constrained_least_squares_rejects_dimension_mismatch() -> None:
    """A weights vector of the wrong length fails closed, naming the mismatch."""
    x = [0.0, 1.0, 1.0, 0.0, 0.0, 1.0]
    with pytest.raises(AnalyticsError, match=r"(?i)dimension"):
        constrained_least_squares(x, 2, [0.05, 0.02, 0.01], [0.6, 0.4])


# End-to-end: constrained_least_squares feeding factor_brinson_attribution


def test_constrained_factor_returns_satisfy_factor_brinson_completeness() -> None:
    """The intended workflow: fit `f_b` then feed it into factor-Brinson attribution.

    Uses the continuous-exposure fixture from the Rust
    `continuous_factors_match_hand_derived_estimator_output` test, fitting
    `f_b` with `constrained_least_squares` against *benchmark* weights (as
    the Rust module docs require for the completeness condition) rather than
    hand-copying the rounded literal.
    """
    exposures = [1.2, -0.8, 0.5, 1.2, -0.7, 0.7]
    returns = [0.05, 0.02, 0.01]
    benchmark_weights = [0.60, 0.30, 0.10]
    f_b = constrained_least_squares(exposures, 2, returns, benchmark_weights)

    input_data = {
        "asset_ids": ["A", "B", "C"],
        "asset_returns": returns,
        "exposures": exposures,
        "factor_names": ["Energy", "Healthcare"],
        "portfolio_weights": [1.25, -0.30, 0.05],
        "benchmark_weights": benchmark_weights,
    }
    result = json.loads(factor_brinson_attribution(json.dumps(input_data), f_b).to_json())

    assert result["allocation"] + result["selection"] == pytest.approx(result["active_return"], abs=1e-12)
