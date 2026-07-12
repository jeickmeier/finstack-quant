"""Smoke tests for golden pytest helpers."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
from types import SimpleNamespace

import pytest

from . import conftest as golden_conftest
from .conftest import (
    DATA_ROOTS,
    discover_fixtures,
    fixture_path,
    validate_fixture,
)
from .schema import GoldenFixture, ToleranceEntry
from .tolerance import compare


def test_fixture_path_pricing() -> None:
    path = fixture_path("pricing/quantlib/irs/foo.json")
    assert path.parts[-4:] == ("pricing", "quantlib", "irs", "foo.json")
    assert "valuations" in str(path)


def test_fixture_path_analytics() -> None:
    path = fixture_path("analytics/returns/foo.json")
    assert "analytics" in str(path)


def test_fixture_path_unknown_domain_raises() -> None:
    with pytest.raises(ValueError, match="known top-level domain"):
        fixture_path("bogus/foo.json")


def test_discover_fixtures_empty_dir() -> None:
    assert discover_fixtures("pricing/nonexistent") == []


def test_discover_pricing_product_across_golden_types() -> None:
    fixtures = discover_fixtures("pricing/fra")

    assert "pricing/bloomberg/fra/usd_fra_3x6.json" in fixtures
    assert "pricing/quantlib/fra/usd_fra_3x6_quantlib.json" in fixtures


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
        "schema": "finstack_quant.instrument/1",
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
    path = DATA_ROOTS["pricing"] / "pricing/bloomberg/cap_floor/usd_cap_5y_atm_black.json"
    fixture = GoldenFixture.from_path(path)
    fixture.metadata.screenshots[0].path = "../cap_floor/usd_cap_5y_atm_black.json"

    with pytest.raises(AssertionError, match="screenshots/"):
        validate_fixture(path, fixture)


def test_pricing_validation_rejects_inconsistent_swaption_underlying_tenor() -> None:
    path = DATA_ROOTS["pricing"] / "pricing/regression_goldens/swaption/usd_swaption_normal_vol_self_test.json"
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


def test_allowlisted_metric_mismatch_is_reported_not_fatal(monkeypatch: pytest.MonkeyPatch) -> None:
    relative, _fixture, actuals = _deposit_actuals()
    actuals["npv"] += 100.0
    _patch_golden_run(monkeypatch, relative, actuals, {"npv": _unresolved_entry("npv")})

    with pytest.warns(golden_conftest.ExpectedUnresolvedWarning, match="expected unresolved"):
        golden_conftest.run_golden(relative)


def test_allowlisted_metric_pass_is_stale_and_fails(monkeypatch: pytest.MonkeyPatch) -> None:
    relative, _fixture, actuals = _deposit_actuals()
    _patch_golden_run(monkeypatch, relative, actuals, {"npv": _unresolved_entry("npv")})

    with pytest.raises(AssertionError, match=r"stale.*npv"):
        golden_conftest.run_golden(relative)


def test_unrelated_metric_mismatch_remains_fatal(monkeypatch: pytest.MonkeyPatch) -> None:
    relative, _fixture, actuals = _deposit_actuals()
    actuals["dv01"] += 100.0
    _patch_golden_run(monkeypatch, relative, actuals, {"npv": _unresolved_entry("npv")})

    with pytest.raises(AssertionError, match="dv01"):
        golden_conftest.run_golden(relative)


def test_invalid_allowlisted_metric_fails(monkeypatch: pytest.MonkeyPatch) -> None:
    relative, _fixture, actuals = _deposit_actuals()
    _patch_golden_run(monkeypatch, relative, actuals, {"not_a_metric": _unresolved_entry("not_a_metric")})

    with pytest.raises(AssertionError, match=r"not_a_metric.*not expected"):
        golden_conftest.run_golden(relative)


def test_allowlist_never_suppresses_execution_errors(monkeypatch: pytest.MonkeyPatch) -> None:
    relative, _fixture, _actuals = _deposit_actuals()
    monkeypatch.setattr(
        golden_conftest,
        "_known_unresolved_metrics",
        lambda: {relative: {"npv": _unresolved_entry("npv")}},
        raising=False,
    )
    monkeypatch.setattr(
        golden_conftest,
        "_load_runner",
        lambda _domain: SimpleNamespace(run=lambda _fixture: (_ for _ in ()).throw(RuntimeError("boom"))),
    )

    with pytest.raises(RuntimeError, match="boom"):
        golden_conftest.run_golden(relative)


def test_missing_allowlisted_fixture_fails_stale(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    allowlist_path = tmp_path / "known_non_executable.json"
    allowlist_path.write_text(
        json.dumps({
            "description": "test",
            "fixtures": [
                {
                    "path": "pricing/deposit/does_not_exist.json",
                    "description": "missing fixture",
                    "metrics": [
                        {
                            "metric": "npv",
                            "reason": "known independent benchmark gap",
                            "evidence": "expected 1, actual 2",
                        }
                    ],
                }
            ],
        }),
        encoding="utf-8",
    )
    golden_conftest._known_unresolved_metrics.cache_clear()
    monkeypatch.setattr(golden_conftest, "KNOWN_NON_EXECUTABLE_PATH", allowlist_path)

    with pytest.raises(AssertionError, match="fixture does not exist"):
        golden_conftest._known_unresolved_metrics()

    golden_conftest._known_unresolved_metrics.cache_clear()


@pytest.mark.parametrize(
    ("scope", "payload"),
    [
        (
            "root",
            {
                "description": "test",
                "fixtures": [],
                "unknown": True,
            },
        ),
        (
            "fixture",
            {
                "description": "test",
                "fixtures": [
                    {
                        "path": "pricing/deposit/usd_deposit_3m.json",
                        "description": "test fixture",
                        "metrics": [],
                        "unknown": True,
                    }
                ],
            },
        ),
        (
            "metric",
            {
                "description": "test",
                "fixtures": [
                    {
                        "path": "pricing/deposit/usd_deposit_3m.json",
                        "description": "test fixture",
                        "metrics": [
                            {
                                "metric": "npv",
                                "reason": "known independent benchmark gap",
                                "evidence": "expected 1, actual 2",
                                "unknown": True,
                            }
                        ],
                    }
                ],
            },
        ),
    ],
)
def test_unresolved_parser_rejects_unknown_fields(scope: str, payload: object) -> None:
    with pytest.raises(AssertionError, match=rf"unknown {scope} field"):
        golden_conftest._parse_unresolved_allowlist(payload)


def test_unresolved_parser_rejects_blank_required_strings() -> None:
    mutations = [
        ("root description", lambda payload: payload.__setitem__("description", " ")),
        ("fixture path", lambda payload: payload["fixtures"][0].__setitem__("path", "")),
        (
            "fixture description",
            lambda payload: payload["fixtures"][0].__setitem__("description", " "),
        ),
        (
            "metric",
            lambda payload: payload["fixtures"][0]["metrics"][0].__setitem__("metric", ""),
        ),
        (
            "reason",
            lambda payload: payload["fixtures"][0]["metrics"][0].__setitem__("reason", " "),
        ),
        (
            "evidence",
            lambda payload: payload["fixtures"][0]["metrics"][0].__setitem__("evidence", ""),
        ),
    ]
    for _label, mutate in mutations:
        payload = _valid_allowlist_payload()
        mutate(payload)
        with pytest.raises(AssertionError, match="non-empty string"):
            golden_conftest._parse_unresolved_allowlist(payload)


def test_unresolved_parser_rejects_invalid_types() -> None:
    payloads: list[object] = [
        [],
        {"description": "test", "fixtures": {}},
        {"description": "test", "fixtures": [1]},
        {
            "description": "test",
            "fixtures": [
                {
                    "path": "pricing/deposit/usd_deposit_3m.json",
                    "description": "test fixture",
                    "metrics": {},
                }
            ],
        },
        {
            "description": "test",
            "fixtures": [
                {
                    "path": "pricing/deposit/usd_deposit_3m.json",
                    "description": "test fixture",
                    "metrics": [1],
                }
            ],
        },
    ]
    for payload in payloads:
        with pytest.raises(TypeError):
            golden_conftest._parse_unresolved_allowlist(payload)


def test_unresolved_parser_rejects_duplicate_fixture_and_metric_entries() -> None:
    duplicate_fixture = _valid_allowlist_payload()
    duplicate_fixture["fixtures"].append(deepcopy(duplicate_fixture["fixtures"][0]))
    with pytest.raises(AssertionError, match="duplicate unresolved fixture"):
        golden_conftest._parse_unresolved_allowlist(duplicate_fixture)

    duplicate_metric = _valid_allowlist_payload()
    duplicate_metric["fixtures"][0]["metrics"].append(deepcopy(duplicate_metric["fixtures"][0]["metrics"][0]))
    with pytest.raises(AssertionError, match="duplicate unresolved metric"):
        golden_conftest._parse_unresolved_allowlist(duplicate_metric)


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        (None, False),
        ("", False),
        ("0", False),
        ("false", False),
        ("FALSE", False),
        ("1", True),
        ("true", True),
        ("YES", True),
        ("on", True),
    ],
)
def test_strict_mode_env_truthiness(monkeypatch: pytest.MonkeyPatch, value: str | None, expected: bool) -> None:
    if value is None:
        monkeypatch.delenv("GOLDEN_IGNORE_NON_EXECUTABLE", raising=False)
    else:
        monkeypatch.setenv("GOLDEN_IGNORE_NON_EXECUTABLE", value)
    assert golden_conftest._strict_golden_mode_enabled() is expected


def test_multiple_unresolved_warnings_are_sorted_by_metric(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    relative, _fixture, actuals = _deposit_actuals()
    actuals["npv"] += 100.0
    actuals["dv01"] += 100.0
    _patch_golden_run(
        monkeypatch,
        relative,
        actuals,
        {
            "npv": _unresolved_entry("npv"),
            "dv01": _unresolved_entry("dv01"),
        },
    )

    with pytest.warns(golden_conftest.ExpectedUnresolvedWarning) as captured:
        golden_conftest.run_golden(relative)

    messages = [str(warning.message) for warning in captured]
    assert ["::dv01:" in messages[0], "::npv:" in messages[1]] == [True, True]


def _deposit_fixture() -> tuple[object, GoldenFixture]:
    path = DATA_ROOTS["pricing"] / "pricing/quantlib/deposit/usd_deposit_3m.json"
    fixture = GoldenFixture.from_path(path)
    fixture.body = deepcopy(fixture.body)
    return path, fixture


def _valid_allowlist_payload() -> dict:
    return {
        "description": "test allowlist",
        "fixtures": [
            {
                "path": "pricing/quantlib/deposit/usd_deposit_3m.json",
                "description": "test fixture",
                "metrics": [
                    {
                        "metric": "npv",
                        "reason": "known benchmark gap",
                        "evidence": "expected 1, actual 2",
                    }
                ],
            }
        ],
    }


def _deposit_actuals() -> tuple[str, GoldenFixture, dict[str, float]]:
    relative = "pricing/quantlib/deposit/usd_deposit_3m.json"
    fixture = GoldenFixture.from_path(fixture_path(relative))
    return relative, fixture, dict(fixture.expected)


def _unresolved_entry(metric: str) -> golden_conftest.UnresolvedMetric:
    return golden_conftest.UnresolvedMetric(
        metric=metric,
        reason="known independent benchmark gap",
        evidence="expected 1, actual 2",
    )


def _patch_golden_run(
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
    actuals: dict[str, float],
    metrics: dict[str, golden_conftest.UnresolvedMetric],
) -> None:
    monkeypatch.setattr(
        golden_conftest,
        "_known_unresolved_metrics",
        lambda: {relative: metrics},
        raising=False,
    )
    monkeypatch.setattr(
        golden_conftest,
        "_load_runner",
        lambda _domain: SimpleNamespace(run=lambda _fixture: actuals),
    )
