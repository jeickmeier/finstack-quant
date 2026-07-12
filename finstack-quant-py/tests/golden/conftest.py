"""pytest helpers for golden tests that consume Rust crate JSON fixtures."""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import contextmanager
import csv
from dataclasses import dataclass
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
import warnings

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
PRICING_GOLDEN_TYPES = ("regression_goldens", "quantlib", "bloomberg")
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


class ExpectedUnresolvedWarning(UserWarning):
    """A metric-specific external benchmark gap matched its allowlist entry."""


@dataclass(frozen=True)
class UnresolvedMetric:
    """Strictly validated unresolved metric metadata."""

    metric: str
    reason: str
    evidence: str


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
    "exotics.asian_option": "pricing_common",
    "exotics.barrier_option": "pricing_common",
    "exotics.lookback_option": "pricing_common",
    "fixed_income.bond": "pricing_common",
    "fixed_income.bond_future": "pricing_common",
    "fixed_income.convertible": "pricing_common",
    "fixed_income.inflation_linked_bond": "pricing_common",
    "fixed_income.term_loan": "pricing_common",
    "fixed_income.structured_credit": "pricing_common",
    "fx.fx_option": "pricing_common",
    "fx.fx_digital_option": "pricing_common",
    "fx.fx_barrier_option": "pricing_common",
    "fx.quanto_option": "pricing_common",
    "fx.fx_forward": "pricing_common",
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
    parts = Path(relative_dir).parts
    if len(parts) == 2 and parts[0] == "pricing" and parts[1] not in PRICING_GOLDEN_TYPES:
        roots = [data_root / "pricing" / golden_type / parts[1] for golden_type in PRICING_GOLDEN_TYPES]
    else:
        roots = [data_root / relative_dir]
    return sorted(str(path.relative_to(data_root)) for root in roots if root.exists() for path in root.rglob("*.json"))


@cache
def _known_unresolved_metrics() -> dict[str, dict[str, UnresolvedMetric]]:
    """Load and validate metric-specific unresolved golden comparisons."""
    if _strict_golden_mode_enabled():
        return {}

    raw = json.loads(KNOWN_NON_EXECUTABLE_PATH.read_text(encoding="utf-8"))
    unresolved = _parse_unresolved_allowlist(raw)
    for relative_path, metrics in unresolved.items():
        path = fixture_path(relative_path)
        if not path.exists():
            msg = f"stale unresolved fixture entry {relative_path!r}: fixture does not exist"
            raise AssertionError(msg)
        fixture = GoldenFixture.from_path(path)
        validate_fixture(path, fixture)

        for metric in metrics:
            if metric not in fixture.expected:
                msg = f"unresolved metric {metric!r} is not expected by fixture {relative_path!r}"
                raise AssertionError(msg)
    return unresolved


def _strict_golden_mode_enabled() -> bool:
    value = os.environ.get("GOLDEN_IGNORE_NON_EXECUTABLE")
    return value is not None and value.strip().lower() in {"1", "true", "yes", "on"}


def _parse_unresolved_allowlist(raw: object) -> dict[str, dict[str, UnresolvedMetric]]:
    root = _require_object(raw, "root")
    _reject_unknown_fields(root, {"description", "fixtures"}, "root")
    _required_non_empty_string(root, "description")
    fixtures = root.get("fixtures")
    if not isinstance(fixtures, list):
        msg = "unresolved allowlist field 'fixtures' must be a list"
        raise TypeError(msg)

    unresolved: dict[str, dict[str, UnresolvedMetric]] = {}
    for raw_fixture in fixtures:
        fixture = _require_object(raw_fixture, "fixture")
        _reject_unknown_fields(fixture, {"path", "description", "metrics"}, "fixture")
        relative_path = _required_non_empty_string(fixture, "path")
        _required_non_empty_string(fixture, "description")
        if relative_path in unresolved:
            msg = f"duplicate unresolved fixture entry {relative_path!r}"
            raise AssertionError(msg)

        raw_metrics = fixture.get("metrics")
        if not isinstance(raw_metrics, list):
            msg = f"unresolved fixture entry {relative_path!r} field 'metrics' must be a list"
            raise TypeError(msg)
        if not raw_metrics:
            msg = f"unresolved fixture entry {relative_path!r} must contain a non-empty metrics list"
            raise AssertionError(msg)

        metrics: dict[str, UnresolvedMetric] = {}
        for raw_metric in raw_metrics:
            metric_entry = _require_object(raw_metric, "metric")
            _reject_unknown_fields(metric_entry, {"metric", "reason", "evidence"}, "metric")
            metric = _required_non_empty_string(metric_entry, "metric")
            reason = _required_non_empty_string(metric_entry, "reason")
            evidence = _required_non_empty_string(metric_entry, "evidence")
            if metric in metrics:
                msg = f"duplicate unresolved metric {metric!r} for fixture {relative_path!r}"
                raise AssertionError(msg)
            metrics[metric] = UnresolvedMetric(metric=metric, reason=reason, evidence=evidence)
        unresolved[relative_path] = metrics
    return unresolved


def _require_object(value: object, scope: str) -> dict[str, object]:
    if not isinstance(value, dict):
        msg = f"unresolved allowlist {scope} must be an object, got {type(value).__name__}"
        raise TypeError(msg)
    return value


def _reject_unknown_fields(entry: dict[str, object], allowed: set[str], scope: str) -> None:
    unknown = sorted(set(entry) - allowed)
    if unknown:
        msg = f"unknown {scope} field {unknown[0]!r}"
        raise AssertionError(msg)


def _required_non_empty_string(entry: dict[str, object], key: str) -> str:
    value = entry.get(key)
    if not isinstance(value, str):
        msg = f"unresolved allowlist field {key!r} must be a string"
        raise TypeError(msg)
    if not value.strip():
        msg = f"unresolved allowlist field {key!r} must be a non-empty string"
        raise AssertionError(msg)
    return value


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
    unresolved = _known_unresolved_metrics().get(relative_path, {})
    invalid_metrics = set(unresolved) - set(fixture.expected)
    if invalid_metrics:
        metric = sorted(invalid_metrics)[0]
        msg = f"unresolved metric {metric!r} is not expected by fixture {relative_path!r}"
        raise AssertionError(msg)

    runner = _load_runner(fixture.metadata.domain)
    actuals = runner.run(fixture)

    failures = []
    results = []
    for metric in sorted(fixture.expected):
        expected = fixture.expected[metric]
        if metric not in actuals:
            failures.append(f"{path}: runner did not produce metric '{metric}'")
            continue
        tolerance = fixture.tolerances[metric]
        result = compare(metric, actuals[metric], expected, tolerance)
        results.append(result)
        unresolved_entry = unresolved.get(metric)
        if result.passed and unresolved_entry is not None:
            failures.append(
                f"stale unresolved metric '{metric}' for {relative_path}: comparison now passes; "
                "remove the allowlist entry"
            )
        elif not result.passed and unresolved_entry is not None:
            warnings.warn(
                f"expected unresolved metric {relative_path}::{metric}: "
                f"{unresolved_entry.reason} Evidence: {unresolved_entry.evidence}\n"
                f"{result.failure_message(str(path))}",
                ExpectedUnresolvedWarning,
                stacklevel=2,
            )
        elif not result.passed:
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

    if fixture.kind == "pricing":
        relative = path.relative_to(DATA_ROOTS["pricing"] / "pricing")
        actual = relative.parts[0]
        expected = _pricing_golden_type(metadata.source)
        assert actual == expected, (
            f"metadata.source {metadata.source!r} requires pricing/{expected}/, found pricing/{actual}/"
        )

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


def _pricing_golden_type(source: str) -> str:
    if source == "quantlib":
        return "quantlib"
    if source in {"bloomberg-api", "bloomberg-screen"}:
        return "bloomberg"
    return "regression_goldens"


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
    if _git_tracks(git, relative_path):
        return True
    pricing_root = Path("finstack-quant/valuations/tests/golden/data/pricing")
    try:
        relative_pricing_path = relative_path.relative_to(pricing_root)
    except ValueError:
        return False
    if relative_pricing_path.parts[:1] not in {(golden_type,) for golden_type in PRICING_GOLDEN_TYPES}:
        return False
    return _git_tracks(git, pricing_root / Path(*relative_pricing_path.parts[1:]))


def _git_tracks(git: str, relative_path: Path) -> bool:
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
