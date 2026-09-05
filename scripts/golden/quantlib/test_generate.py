"""Tests for deterministic QuantLib golden generation."""

from __future__ import annotations

import json
import math

import pytest

from .common import SCHEMA, flat_forward_curve
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
    assert fixture["schema"] == SCHEMA
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


def test_generate_attribution_bond_is_deterministic_and_complete(tmp_path) -> None:
    """Attribution bond fixtures are deterministic and carry T0/T1 + expected P&L."""
    [path] = generate("bond", tmp_path, family="attribution")
    first = path.read_bytes()
    generate("bond", tmp_path, family="attribution")
    assert path.read_bytes() == first
    assert path.name == "bond_5pct_10y_usd.json"

    fixture = json.loads(first)
    assert fixture["instrument"] == "FixedRateBond"
    assert set(fixture) >= {
        "conventions",
        "expected_attribution",
        "quantlib",
        "scenario",
        "spec",
    }
    assert set(fixture["quantlib"]) == {"t0", "t1", "version"}
    assert {"npv", "dv01", "theta_one_day"} <= set(fixture["quantlib"]["t0"])
    assert "npv" in fixture["quantlib"]["t1"]
    assert {
        "actual_pnl",
        "carry_pnl",
        "rates_pnl_first_order",
        "residual_first_order",
    } <= set(fixture["expected_attribution"])
    assert all(math.isfinite(value) for value in fixture["expected_attribution"].values())


def test_attribution_check_detects_fixture_drift(tmp_path) -> None:
    """Attribution check mode fails when a written fixture differs."""
    [path] = generate("bond", tmp_path, family="attribution")
    generate("bond", tmp_path, family="attribution", check=True)
    path.write_text("{}\n", encoding="utf-8")
    with pytest.raises(RuntimeError, match="stale"):
        generate("bond", tmp_path, family="attribution", check=True)


@pytest.mark.parametrize(
    ("product", "filename", "instrument"),
    [
        ("bond", "bond_5pct_10y_usd.json", "FixedRateBond"),
        ("irs", "irs_5y_usd.json", "VanillaSwap"),
        ("fx_forward", "fx_forward_1y_eurusd.json", "FxForward"),
    ],
)
def test_generate_attribution_products_are_complete(
    tmp_path,
    product: str,
    filename: str,
    instrument: str,
) -> None:
    """Each attribution product emits finite expected-attribution factors."""
    [path] = generate(product, tmp_path, family="attribution")
    assert path.name == filename
    fixture = json.loads(path.read_text(encoding="utf-8"))
    assert fixture["instrument"] == instrument
    assert "expected_attribution" in fixture
    assert "actual_pnl" in fixture["expected_attribution"]
    assert all(math.isfinite(value) for value in fixture["expected_attribution"].values())


def test_generate_attribution_all_writes_three_fixtures(tmp_path) -> None:
    """Family attribution --product all regenerates the full committed set."""
    paths = generate("all", tmp_path, family="attribution")
    assert sorted(path.name for path in paths) == [
        "bond_5pct_10y_usd.json",
        "fx_forward_1y_eurusd.json",
        "irs_5y_usd.json",
    ]


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
    if product in {"single_name_cds", "fixed_hazard_bond", "floating_hazard_bond"}:
        assert all(not metric.startswith("cs01") for metric in fixture["expected"])
    assert all(math.isfinite(value) for value in fixture["expected"].values())
