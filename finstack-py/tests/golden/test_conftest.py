"""Smoke tests for golden pytest helpers."""

from __future__ import annotations

from copy import deepcopy

import pytest

from .conftest import (
    DATA_ROOTS,
    discover_fixtures,
    fixture_path,
    validate_fixture,
)
from .schema import GoldenFixture, ToleranceEntry
from .tolerance import compare


def test_fixture_path_pricing() -> None:
    path = fixture_path("pricing/irs/foo.json")
    assert path.parts[-3:] == ("pricing", "irs", "foo.json")
    assert "valuations" in str(path)


def test_fixture_path_analytics() -> None:
    path = fixture_path("analytics/returns/foo.json")
    assert "analytics" in str(path)


def test_fixture_path_unknown_domain_raises() -> None:
    with pytest.raises(ValueError, match="known top-level domain"):
        fixture_path("bogus/foo.json")


def test_discover_fixtures_empty_dir() -> None:
    assert discover_fixtures("pricing/nonexistent") == []


def test_abs_or_rel_tolerances_allow_either_by_default() -> None:
    result = compare("npv", 1_000_000.5, 1_000_000.0, ToleranceEntry(abs=0.01, rel=1e-6))

    assert result.passed


def test_abs_or_rel_tolerance_does_not_require_explicit_reason() -> None:
    result = compare(
        "npv",
        1_000_000.5,
        1_000_000.0,
        ToleranceEntry(
            abs=0.01,
            rel=1e-6,
            tolerance_reason="abs-or-rel tolerance reflects vendor screen rounding",
        ),
    )

    assert result.passed


def test_zero_risk_metric_requires_tolerance_reason() -> None:
    path, fixture = _deposit_fixture()
    fixture.expected["dv01"] = 0.0
    fixture.tolerances["dv01"] = ToleranceEntry(abs=1e-9)

    with pytest.raises(AssertionError, match="dv01"):
        validate_fixture(path, fixture)


def test_pricing_validation_rejects_invalid_instrument() -> None:
    path, fixture = _deposit_fixture()
    fixture.body["instrument"] = {
        "schema": "finstack.instrument/1",
        "instrument": {
            "type": "deposit",
            "spec": {},
        },
    }

    with pytest.raises(AssertionError, match="instrument"):
        validate_fixture(path, fixture)


def test_pricing_validation_rejects_unknown_metric_name() -> None:
    path, fixture = _deposit_fixture()
    fixture.expected = {"npv": 1.0, "dv01x": 1.0}
    fixture.tolerances = {metric: ToleranceEntry(abs=1e-9) for metric in fixture.expected}

    with pytest.raises(AssertionError, match="dv01x"):
        validate_fixture(path, fixture)


def test_pricing_validation_allows_dynamic_metric_keys_from_requested_base_metric() -> None:
    path, fixture = _deposit_fixture()
    fixture.expected = {"npv": 1.0, "dv01": 1.0, "bucketed_dv01::USD-OIS::1y": 1.0}
    fixture.tolerances = {metric: ToleranceEntry(abs=1e-9) for metric in fixture.expected}

    validate_fixture(path, fixture)


def test_manual_screenshot_paths_must_stay_under_screenshots_directory() -> None:
    path = DATA_ROOTS["pricing"] / "pricing/cap_floor/usd_cap_5y_atm_black.json"
    fixture = GoldenFixture.from_path(path)
    fixture.metadata.screenshots[0].path = "../cap_floor/usd_cap_5y_atm_black.json"

    with pytest.raises(AssertionError, match="screenshots/"):
        validate_fixture(path, fixture)


def test_pricing_validation_rejects_inconsistent_swaption_underlying_tenor() -> None:
    path = DATA_ROOTS["pricing"] / "pricing/swaption/usd_swaption_normal_vol_self_test.json"
    fixture = GoldenFixture.from_path(path)
    fixture.body = deepcopy(fixture.body)
    spec = fixture.body["instrument"]["instrument"]["spec"]
    spec["swap_end"] = "2029-05-08"
    spec["underlying_fixed_leg"]["end"] = "2032-05-05"
    spec["underlying_float_leg"]["end"] = "2032-05-05"

    with pytest.raises(AssertionError, match="swaption"):
        validate_fixture(path, fixture)


def test_sabr_strike_keys_must_match_expected() -> None:
    path = DATA_ROOTS["market_data"] / "market_data/sabr/beta_half_smile.json"
    fixture = GoldenFixture.from_path(path)
    fixture.body = deepcopy(fixture.body)
    fixture.body["strikes"].pop()

    with pytest.raises(AssertionError, match="strike keys"):
        validate_fixture(path, fixture)


def _deposit_fixture() -> tuple[object, GoldenFixture]:
    path = DATA_ROOTS["pricing"] / "pricing/deposit/usd_deposit_3m.json"
    fixture = GoldenFixture.from_path(path)
    fixture.body = deepcopy(fixture.body)
    return path, fixture
