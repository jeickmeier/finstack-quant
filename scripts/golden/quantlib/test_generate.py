"""Tests for deterministic QuantLib golden generation."""

from __future__ import annotations

import json
import math

import pytest

from .common import SCHEMA_VERSION, flat_forward_curve
from .generate import generate


def test_flat_forward_curve_records_contractual_projection_dates() -> None:
    """Term-forward fixtures preserve their contractual reset grid."""
    curve = flat_forward_curve(
        "USD-SOFR-3M",
        0.043,
        projection_dates=["2026-08-03", "2026-11-03"],
    )

    assert curve["projection_grid"] == [
        0.0,
        95.0 / 360.0,
        187.0 / 360.0,
        30.0,
    ]


def test_generate_fra_is_deterministic_and_complete(tmp_path) -> None:
    """The FRA generator emits deterministic schema-complete JSON."""
    [path] = generate("fra", tmp_path)
    first = path.read_bytes()
    generate("fra", tmp_path)
    assert path.read_bytes() == first

    fixture = json.loads(first)
    assert fixture["schema_version"] == SCHEMA_VERSION
    assert fixture["metadata"]["source"] == "quantlib"
    assert fixture["metadata"]["valuation_date"] == "2026-04-30"
    assert set(fixture["expected"]) == set(fixture["tolerances"])
    assert {"npv", "par_rate", "dv01"} == set(fixture["expected"])
    assert all(math.isfinite(value) for value in fixture["expected"].values())


def test_check_detects_fixture_drift(tmp_path) -> None:
    """Check mode fails when a committed fixture differs."""
    [path] = generate("fra", tmp_path)
    generate("fra", tmp_path, check=True)
    path.write_text("{}\n", encoding="utf-8")
    with pytest.raises(RuntimeError, match="stale"):
        generate("fra", tmp_path, check=True)


@pytest.mark.parametrize(
    "product",
    [
        "black_caplet",
        "black_cap",
        "bachelier_floorlet",
        "black_swaption",
        "bachelier_swaption",
    ],
)
def test_analytical_rate_tolerances_are_numerically_tight(tmp_path, product: str) -> None:
    """Analytical rate fixtures retain strict absolute parity tolerances."""
    [path] = generate(product, tmp_path)
    fixture = json.loads(path.read_text(encoding="utf-8"))

    for metric in fixture["expected"]:
        tolerance = fixture["tolerances"][metric]
        assert tolerance["abs"] == 1e-7
        assert "rel" not in tolerance


@pytest.mark.parametrize(
    "product",
    [
        "irs",
        "deposit",
        "single_name_cds",
        "sofr_future",
        "fixed_risk_free_bond",
        "fixed_hazard_bond",
        "fixed_callable_oas_bond",
        "floating_risk_free_bond",
        "floating_hazard_bond",
        "european_equity_option",
        "european_fx_option",
        "black_caplet",
        "black_cap",
        "bachelier_floorlet",
        "black_swaption",
        "bachelier_swaption",
        "fx_forward",
        "fx_digital_option",
        "fx_barrier_option",
        "quanto_option",
        "barrier_option",
        "arithmetic_asian_option",
        "geometric_asian_option",
        "fixed_lookback_option",
        "floating_lookback_option",
    ],
)
def test_generate_other_rates_is_complete(tmp_path, product: str) -> None:
    """Each rates builder emits finite expected values with complete tolerances."""
    [path] = generate(product, tmp_path)
    fixture = json.loads(path.read_text(encoding="utf-8"))
    assert set(fixture["expected"]) == set(fixture["tolerances"])
    if product not in {
        "barrier_option",
        "european_equity_option",
        "european_fx_option",
        "fx_digital_option",
        "fx_barrier_option",
        "quanto_option",
        "fixed_lookback_option",
        "floating_lookback_option",
        "geometric_asian_option",
        "arithmetic_asian_option",
    }:
        assert "dv01" in fixture["expected"]
    if product == "fixed_callable_oas_bond":
        assert "oas" in fixture["expected"]
    assert all(math.isfinite(value) for value in fixture["expected"].values())
