"""pytest helpers for golden tests that consume Rust crate JSON fixtures."""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import contextmanager
import csv
from datetime import date
from functools import cache
import importlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from types import ModuleType

import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import validate_calibration_json

from .pricing_validation import requested_metrics, validate_requested_metrics, validated_instrument_json
from .schema import SCHEMA_VERSION, GoldenFixture
from .tolerance import compare

WORKSPACE_ROOT = Path(__file__).resolve().parents[3]
REPORT_HEADER = [
    "runner",
    "fixture",
    "metric",
    "actual",
    "expected",
    "abs_diff",
    "rel_diff",
    "abs_tolerance",
    "rel_tolerance",
    "passed",
    "tolerance_reason",
]
REPORT_LOCK_TIMEOUT_SECONDS = 30.0
REPORT_LOCK_POLL_SECONDS = 0.01

DATA_ROOTS = {
    "pricing": WORKSPACE_ROOT / "finstack-quant/valuations/tests/golden/data",
    "market_data": WORKSPACE_ROOT / "finstack-quant/valuations/tests/golden/data",
    "analytics": WORKSPACE_ROOT / "finstack-quant/analytics/tests/golden/data",
}
KNOWN_NON_EXECUTABLE_PATH = WORKSPACE_ROOT / "finstack-quant/valuations/tests/golden/known_non_executable.json"
VALID_SOURCES = {
    "quantlib",
    "bloomberg-api",
    "bloomberg-screen",
    "intex",
    "formula",
    "textbook",
}
MANUAL_SCREENSHOT_SOURCES = {"bloomberg-screen", "intex"}
ZERO_RISK_EPSILON = 2.220446049250313e-16
ZERO_RISK_METRICS_REQUIRING_REASON = {
    "bucketed_dv01",
    "convexity",
    "cs01",
    "delta",
    "duration_mod",
    "dv01",
    "foreign_rho",
    "gamma",
    "inflation01",
    "recovery_01",
    "rho",
    "spread_dv01",
    "vega",
}
_DOMAIN_RUNNERS = {
    "analytics.benchmark": "analytics_common",
    "analytics.drawdown": "analytics_common",
    "analytics.performance": "analytics_common",
    "analytics.returns": "analytics_common",
    "analytics.risk": "analytics_common",
    "analytics.vol": "analytics_common",
    "credit.cds": "pricing_common",
    "credit.cds_option": "pricing_common",
    "credit.cds_tranche": "pricing_common",
    "equity.equity_option": "pricing_common",
    "equity.equity_index_future": "pricing_common",
    "exotics.barrier_option": "pricing_common",
    "fixed_income.bond": "pricing_common",
    "fixed_income.bond_future": "pricing_common",
    "fixed_income.convertible": "pricing_common",
    "fixed_income.inflation_linked_bond": "pricing_common",
    "fixed_income.term_loan": "pricing_common",
    "fixed_income.structured_credit": "pricing_common",
    "fx.fx_option": "pricing_common",
    "fx.fx_swap": "pricing_common",
    "rates.cap_floor": "pricing_common",
    "rates.cms_option": "pricing_common",
    "rates.deposit": "pricing_common",
    "rates.fra": "pricing_common",
    "rates.inflation_swap": "pricing_common",
    "rates.irs": "pricing_common",
    "rates.ir_future": "pricing_common",
    "rates.swaption": "pricing_common",
    "volatility.sabr": "sabr_smile",
}


def _data_root_for(relative_path: str) -> Path:
    top = relative_path.split("/", 1)[0]
    if top not in DATA_ROOTS:
        known = ", ".join(sorted(DATA_ROOTS))
        msg = f"path '{relative_path}' does not start with a known top-level domain ({known})"
        raise ValueError(msg)
    return DATA_ROOTS[top]


def fixture_path(relative_path: str) -> Path:
    """Resolve a fixture path relative to its owning Rust crate's data root."""
    return _data_root_for(relative_path) / relative_path


def discover_fixtures(relative_dir: str) -> list[str]:
    """Return JSON fixtures under a relative data directory."""
    data_root = _data_root_for(relative_dir)
    root = data_root / relative_dir
    if not root.exists():
        return []
    return sorted(str(path.relative_to(data_root)) for path in root.rglob("*.json"))


@cache
def _known_non_executable() -> dict[str, str]:
    """Load the shared {fixture path: reason} allowlist of non-executable goldens.

    The same `known_non_executable.json` drives the Rust runner's non-fatal
    handling, so the allowlist lives in one place across both languages.
    """
    raw = json.loads(KNOWN_NON_EXECUTABLE_PATH.read_text(encoding="utf-8"))
    return {entry["path"]: entry["reason"] for entry in raw["fixtures"]}


def discover_fixtures_with_marks(relative_dir: str) -> list:
    """Discover fixtures, marking known non-executable ones as xfail(strict=False).

    Setting `GOLDEN_IGNORE_NON_EXECUTABLE` skips the marks so every fixture runs
    strictly and all failures surface (see `mise goldens-test-strict`).
    """
    allowlist = {} if os.environ.get("GOLDEN_IGNORE_NON_EXECUTABLE") else _known_non_executable()
    params = []
    for fixture in discover_fixtures(relative_dir):
        reason = allowlist.get(fixture)
        if reason is None:
            params.append(fixture)
        else:
            params.append(pytest.param(fixture, marks=pytest.mark.xfail(reason=reason, strict=False)))
    return params


def _load_runner(domain: str) -> ModuleType:
    if domain not in _DOMAIN_RUNNERS:
        msg = f"no Python runner registered for domain '{domain}'"
        raise ValueError(msg)
    module_name = _DOMAIN_RUNNERS[domain]
    return importlib.import_module(f".runners.{module_name}", package=__package__)


def run_golden(relative_path: str) -> None:
    """Load, dispatch, compare, and assert one golden fixture."""
    path = fixture_path(relative_path)
    fixture = GoldenFixture.from_path(path)
    validate_fixture(path, fixture)
    runner = _load_runner(fixture.metadata.domain)
    actuals = runner.run(fixture)

    failures = []
    results = []
    for metric, expected in fixture.expected.items():
        if metric not in actuals:
            failures.append(f"{path}: runner did not produce metric '{metric}'")
            continue
        tolerance = fixture.tolerances[metric]
        result = compare(metric, actuals[metric], expected, tolerance)
        results.append(result)
        if not result.passed:
            failures.append(result.failure_message(str(path)))

    _write_comparison_csv(relative_path, results)

    if failures:
        msg = f"{len(failures)} metric(s) failed:\n" + "\n\n".join(failures)
        raise AssertionError(msg)


def validate_fixture(path: Path, fixture: GoldenFixture) -> None:
    """Validate one golden fixture before runner dispatch."""
    metadata = fixture.metadata
    assert fixture.schema_version == SCHEMA_VERSION, (
        f"schema_version is {fixture.schema_version!r}, expected {SCHEMA_VERSION!r}"
    )
    assert metadata.name.strip(), "metadata.name is empty"
    assert metadata.domain.strip(), "metadata.domain is empty"
    assert metadata.description.strip(), "metadata.description is empty"
    assert metadata.valuation_date.strip(), "metadata.valuation_date is empty"
    assert metadata.source in VALID_SOURCES, f"unknown metadata.source {metadata.source!r}"
    assert metadata.source_detail.strip(), "metadata.source_detail is empty"
    assert metadata.captured_by.strip(), "metadata.captured_by is empty"
    assert metadata.captured_on.strip(), "metadata.captured_on is empty"
    assert metadata.last_reviewed_by.strip(), "metadata.last_reviewed_by is empty"
    assert metadata.last_reviewed_on.strip(), "metadata.last_reviewed_on is empty"

    extra_tolerances = set(fixture.tolerances) - set(fixture.expected)
    missing_tolerances = set(fixture.expected) - set(fixture.tolerances)
    assert not extra_tolerances, f"tolerances has extra keys: {extra_tolerances}"
    assert not missing_tolerances, f"tolerances missing keys: {missing_tolerances}"

    for metric, tolerance in fixture.tolerances.items():
        assert tolerance.abs is not None or tolerance.rel is not None, (
            f"tolerance for {metric!r} has neither abs nor rel"
        )

    for metric, expected in fixture.expected.items():
        if abs(expected) <= ZERO_RISK_EPSILON and _metric_base(metric) in ZERO_RISK_METRICS_REQUIRING_REASON:
            assert _has_zero_metric_reason(fixture, metric), (
                f"zero risk metric {metric!r} requires a tolerances[metric].tolerance_reason"
            )

    _validate_screenshots(path, fixture)
    if fixture.kind == "pricing":
        _validate_pricing_body(fixture)
    else:
        _validate_sabr_body(fixture)


def _validate_pricing_body(fixture: GoldenFixture) -> None:
    body = fixture.body
    assert str(body.get("model", "")).strip(), "pricing fixture model is empty"

    market = body["market"]
    assert isinstance(market, dict), "pricing fixture market must be an object"
    kind = market.get("kind")
    if kind == "snapshot":
        try:
            MarketContext.from_json(json.dumps(market["data"]))
        except Exception as exc:
            raise AssertionError(f"market.data is not a valid MarketContext: {exc}") from exc
    elif kind == "envelope":
        try:
            validate_calibration_json(json.dumps(market["envelope"]))
        except Exception as exc:
            raise AssertionError(f"market.envelope is not a valid CalibrationEnvelope: {exc}") from exc
    else:
        msg = f"pricing fixture market.kind must be 'snapshot' or 'envelope', got {kind!r}"
        raise AssertionError(msg)

    try:
        validated_instrument_json(body["instrument"])
    except Exception as exc:
        raise AssertionError(f"pricing fixture instrument is not valid: {exc}") from exc
    _validate_swaption_underlying_tenor(body["instrument"])
    validate_requested_metrics(requested_metrics(fixture.expected))
    _validate_required_pricing_risk_metrics(fixture)


def _validate_sabr_body(fixture: GoldenFixture) -> None:
    strikes = fixture.body["strikes"]
    assert strikes, "sabr_smile fixture must define at least one strike"
    strike_keys = {entry["key"] for entry in strikes}
    assert len(strike_keys) == len(strikes), "sabr_smile strike keys must be unique"
    assert strike_keys == set(fixture.expected), "sabr_smile strike keys must match the expected metric keys exactly"


def _validate_swaption_underlying_tenor(instrument_json: dict) -> None:
    instrument = instrument_json.get("instrument", instrument_json)
    if instrument.get("type") != "swaption":
        return
    spec = instrument.get("spec", {})
    assert isinstance(spec, dict), "swaption instrument.spec must be an object"
    top_tenor = _tenor_days(spec, "swap_start", "swap_end")
    fixed_tenor = _tenor_days(spec["underlying_fixed_leg"], "start", "end")
    float_tenor = _tenor_days(spec["underlying_float_leg"], "start", "end")
    assert fixed_tenor == float_tenor, (
        f"swaption underlying fixed/float leg tenors differ: fixed={fixed_tenor}d, float={float_tenor}d"
    )
    assert abs(top_tenor - fixed_tenor) <= 7, (
        f"swaption top-level tenor ({top_tenor}d) does not match underlying leg tenor ({fixed_tenor}d)"
    )


def _tenor_days(obj: dict, start_key: str, end_key: str) -> int:
    return (date.fromisoformat(obj[end_key]) - date.fromisoformat(obj[start_key])).days


def _validate_required_pricing_risk_metrics(fixture: GoldenFixture) -> None:
    domain = fixture.metadata.domain
    if domain.startswith("rates."):
        assert _has_expected_metric(fixture, "dv01"), "rates pricing fixtures must assert dv01"

    if domain.startswith("fixed_income."):
        assert _has_expected_metric(fixture, "dv01"), "fixed-income pricing fixtures must assert dv01"

    if domain.startswith("credit."):
        assert _has_expected_metric(fixture, "dv01"), "credit pricing fixtures must assert dv01"
        assert _has_expected_metric(fixture, "cs01"), "credit pricing fixtures must assert cs01"


def _has_expected_metric(fixture: GoldenFixture, base_metric: str) -> bool:
    return any(_metric_base(metric) == base_metric for metric in fixture.expected)


def _validate_screenshots(path: Path, fixture: GoldenFixture) -> None:
    metadata = fixture.metadata
    if metadata.source in MANUAL_SCREENSHOT_SOURCES:
        assert metadata.screenshots, f"source {metadata.source!r} requires at least one screenshot"

    for screenshot in metadata.screenshots:
        screenshot_path = Path(screenshot.path)
        assert _is_valid_screenshot_path(screenshot_path), (
            f"screenshot {screenshot.path!r} must be a relative path under screenshots/"
        )
        assert screenshot_path.suffix.lower() in {".png", ".jpg", ".jpeg", ".webp"}, (
            f"screenshot {screenshot.path!r} must use an image extension"
        )
        screenshot_path = path.parent / screenshot_path
        assert screenshot_path.exists(), (
            f"screenshot {screenshot.path!r} does not exist (resolved to {screenshot_path})"
        )
        assert is_git_tracked(screenshot_path), f"screenshot {screenshot.path!r} exists but is not tracked by git"


def _is_valid_screenshot_path(path: Path) -> bool:
    return not path.is_absolute() and ".." not in path.parts and path.parts[:1] == ("screenshots",)


def _has_zero_metric_reason(fixture: GoldenFixture, metric: str) -> bool:
    tolerance = fixture.tolerances.get(metric)
    return bool(tolerance and tolerance.tolerance_reason and tolerance.tolerance_reason.strip())


def _metric_base(metric: str) -> str:
    return metric.split("::", 1)[0]


def is_git_tracked(path: Path) -> bool:
    try:
        relative_path = path.relative_to(WORKSPACE_ROOT)
    except ValueError:
        return False
    git = shutil.which("git")
    if git is None:
        return False
    result = subprocess.run(  # noqa: S603 - fixed executable, no shell, path constrained to repo.
        [git, "ls-files", "--error-unmatch", "--", str(relative_path)],
        cwd=WORKSPACE_ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def _write_comparison_csv(relative_path: str, results: list) -> None:
    """Write a dataframe-shaped comparison report for analyst review."""
    report_path = WORKSPACE_ROOT / "target/golden-reports/golden-comparisons.csv"
    report_path.parent.mkdir(parents=True, exist_ok=True)
    with _report_lock(report_path):
        existing_rows = _existing_comparison_rows(report_path, "python", relative_path)
        rows = [REPORT_HEADER, *existing_rows]
        rows.extend(
            [
                "python",
                relative_path,
                result.metric,
                f"{result.actual:.12f}",
                f"{result.expected:.12f}",
                f"{result.abs_diff:.12e}",
                f"{result.rel_diff:.12e}",
                "" if result.used_tolerance.abs is None else f"{result.used_tolerance.abs:.12f}",
                "" if result.used_tolerance.rel is None else f"{result.used_tolerance.rel:.12f}",
                str(result.passed).lower(),
                result.used_tolerance.tolerance_reason or "",
            ]
            for result in results
        )
        _write_report_atomically(report_path, rows)


@contextmanager
def _report_lock(report_path: Path) -> Iterator[None]:
    """Acquire a process-wide lock for read/modify/write report updates."""
    lock_path = report_path.with_suffix(".csv.lock")
    deadline = time.monotonic() + REPORT_LOCK_TIMEOUT_SECONDS
    fd: int | None = None
    while fd is None:
        try:
            fd = os.open(lock_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
        except FileExistsError:
            if time.monotonic() >= deadline:
                msg = f"timed out waiting for report lock {lock_path}"
                raise TimeoutError(msg) from None
            time.sleep(REPORT_LOCK_POLL_SECONDS)

    try:
        yield
    finally:
        os.close(fd)
        lock_path.unlink(missing_ok=True)


def _write_report_atomically(report_path: Path, rows: list[list[str]]) -> None:
    temp_path = report_path.with_suffix(f".csv.{os.getpid()}.tmp")
    try:
        with temp_path.open("w", encoding="utf-8", newline="") as handle:
            writer = csv.writer(handle)
            writer.writerows(rows)
        temp_path.replace(report_path)
    finally:
        temp_path.unlink(missing_ok=True)


def _existing_comparison_rows(report_path: Path, runner: str, relative_path: str) -> list[list[str]]:
    """Read existing aggregate rows, dropping stale rows for this runner/fixture."""
    if not report_path.exists():
        return []

    with report_path.open("r", encoding="utf-8", newline="") as handle:
        rows = list(csv.reader(handle))
    return [row for row in rows[1:] if len(row) >= 2 and row[:2] != [runner, relative_path]]
