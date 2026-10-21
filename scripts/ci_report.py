"""Generate GitHub Actions job summaries and annotations for Nightly CI jobs.

Reads structured outputs from the Rust (`nextest` JUnit XML + `lcov.info`),
Python (`pytest` JUnit XML + Cobertura `coverage.xml`), WASM (`wasm-test` log),
and SonarQube (`.scannerwork/report-task.txt` + SonarQube Cloud API) steps,
writes Markdown tables to `$GITHUB_STEP_SUMMARY`, and emits workflow
annotations for test failures and Quality Gate status.
"""

from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass, field
import json
import os
from pathlib import Path
import re
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen
import xml.etree.ElementTree as ET

_MAX_ANNOTATIONS = 20
_MAX_FAILURE_SNIPPET_LINES = 30
_RATING_LETTERS = {
    "1": "A",
    "1.0": "A",
    "2": "B",
    "2.0": "B",
    "3": "C",
    "3.0": "C",
    "4": "D",
    "4.0": "D",
    "5": "E",
    "5.0": "E",
}
_METRIC_LABELS = {
    "new_reliability_rating": "Reliability Rating on New Code",
    "new_security_rating": "Security Rating on New Code",
    "new_maintainability_rating": "Maintainability Rating on New Code",
    "new_coverage": "Coverage on New Code",
    "new_duplicated_lines_density": "Duplicated Lines (%) on New Code",
    "new_security_hotspots_reviewed": "Security Hotspots Reviewed on New Code",
    "new_bugs": "New Bugs",
    "new_vulnerabilities": "New Vulnerabilities",
    "new_code_smells": "New Code Smells",
    "coverage": "Coverage",
    "bugs": "Bugs",
    "vulnerabilities": "Vulnerabilities",
    "security_hotspots": "Security Hotspots",
    "code_smells": "Code Smells",
    "duplicated_lines_density": "Duplicated Lines",
    "ncloc": "Lines of Code",
}
_COMPARATOR_SYMBOLS = {
    "GT": ">",
    "LT": "<",
    "EQ": "=",
    "NE": "!=",
}


@dataclass(frozen=True)
class TestCaseRecord:
    """Single test case execution record parsed from JUnit XML."""

    suite: str
    name: str
    duration_seconds: float
    failure_message: str | None = None
    failure_detail: str | None = None


@dataclass
class JUnitSummary:
    """Aggregated test execution statistics from a JUnit XML report."""

    tests: int = 0
    passed: int = 0
    failed: int = 0
    errors: int = 0
    skipped: int = 0
    duration_seconds: float = 0.0
    failures: list[TestCaseRecord] = field(default_factory=list)
    slow_cases: list[TestCaseRecord] = field(default_factory=list)


@dataclass
class CoverageBucket:
    """Line and function coverage counters for a crate or package."""

    lines_hit: int = 0
    lines_found: int = 0
    functions_hit: int = 0
    functions_found: int = 0

    @property
    def line_percent(self) -> float:
        """Return line coverage percentage in [0, 100]."""
        if self.lines_found == 0:
            return 100.0
        return (self.lines_hit / self.lines_found) * 100.0

    @property
    def function_percent(self) -> float:
        """Return function coverage percentage in [0, 100]."""
        if self.functions_found == 0:
            return 100.0
        return (self.functions_hit / self.functions_found) * 100.0


def _write_summary(markdown: str) -> None:
    """Print markdown to stdout and append it to `$GITHUB_STEP_SUMMARY` if set."""
    print(markdown)
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with Path(summary_path).open("a", encoding="utf-8") as handle:
            handle.write(markdown)
            if not markdown.endswith("\n"):
                handle.write("\n")


def _escape_annotation(value: str) -> str:
    """Escape a string for GitHub Actions workflow command output."""
    return value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def _emit_annotation(level: str, title: str, message: str) -> None:
    """Emit a GitHub Actions workflow annotation (`notice`, `warning`, or `error`)."""
    safe_title = _escape_annotation(title).replace(":", "%3A").replace(",", "%2C")
    safe_message = _escape_annotation(message)
    print(f"::{level} title={safe_title}::{safe_message}")


def _truncate_snippet(text: str, max_lines: int = _MAX_FAILURE_SNIPPET_LINES) -> str:
    """Trim a failure output block to at most `max_lines` lines."""
    lines = text.strip().splitlines()
    if len(lines) <= max_lines:
        return "\n".join(lines)
    kept = lines[:max_lines]
    kept.append(f"... ({len(lines) - max_lines} more lines)")
    return "\n".join(kept)


def parse_junit_xml(path: Path, slow_seconds: float = 20.0) -> JUnitSummary:
    """Parse a JUnit XML file from `cargo nextest` or `pytest`."""
    tree = ET.parse(path)  # noqa: S314 -- trusted local CI test report artifact
    root = tree.getroot()
    suites = [root] if root.tag == "testsuite" else list(root.findall("testsuite"))

    summary = JUnitSummary()
    root_time = root.attrib.get("time")
    if root_time is not None:
        summary.duration_seconds = float(root_time)

    suite_time_sum = 0.0
    for suite in suites:
        suite_name = suite.attrib.get("name", "suite")
        suite_time_sum += float(suite.attrib.get("time", "0") or 0.0)
        for case in suite.findall("testcase"):
            summary.tests += 1
            case_name = case.attrib.get("name", "unknown")
            class_name = case.attrib.get("classname", suite_name)
            duration = float(case.attrib.get("time", "0") or 0.0)

            failure_el = case.find("failure")
            error_el = case.find("error")
            skipped_el = case.find("skipped")

            if failure_el is not None:
                summary.failed += 1
                msg = failure_el.attrib.get("message") or "Test failed"
                detail = (failure_el.text or "").strip() or msg
                summary.failures.append(
                    TestCaseRecord(
                        suite=class_name,
                        name=case_name,
                        duration_seconds=duration,
                        failure_message=msg,
                        failure_detail=detail,
                    )
                )
            elif error_el is not None:
                summary.errors += 1
                msg = error_el.attrib.get("message") or "Test error"
                detail = (error_el.text or "").strip() or msg
                summary.failures.append(
                    TestCaseRecord(
                        suite=class_name,
                        name=case_name,
                        duration_seconds=duration,
                        failure_message=msg,
                        failure_detail=detail,
                    )
                )
            elif skipped_el is not None:
                summary.skipped += 1
            else:
                summary.passed += 1
                if duration >= slow_seconds:
                    summary.slow_cases.append(
                        TestCaseRecord(
                            suite=class_name,
                            name=case_name,
                            duration_seconds=duration,
                        )
                    )

    if root_time is None:
        summary.duration_seconds = suite_time_sum
    summary.slow_cases.sort(key=lambda item: item.duration_seconds, reverse=True)
    return summary


def _crate_name_from_source_path(source_path: str) -> str:
    """Extract a readable workspace crate key from an LCOV `SF:` path."""
    normalized = source_path.replace("\\", "/")
    marker = "finstack-quant/"
    idx = normalized.rfind(marker)
    rel = normalized[idx + len(marker) :] if idx != -1 else normalized.lstrip("/")
    parts = [part for part in rel.split("/") if part]
    if not parts:
        return "workspace"
    if parts[0] == "valuations" and len(parts) >= 2 and parts[1] != "src":
        return f"valuations/{parts[1]}"
    if parts[0] == "src":
        return "finstack-quant"
    return parts[0]


def parse_lcov(path: Path) -> tuple[CoverageBucket, dict[str, CoverageBucket]]:
    """Parse an LCOV report into total and per-crate coverage buckets."""
    total = CoverageBucket()
    by_crate: dict[str, CoverageBucket] = {}
    current_crate = "workspace"
    lf = lh = fnf = fnh = 0

    for raw_line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw_line.strip()
        if line.startswith("SF:"):
            current_crate = _crate_name_from_source_path(line[3:])
            lf = lh = fnf = fnh = 0
        elif line.startswith("LF:"):
            lf = int(line[3:] or 0)
        elif line.startswith("LH:"):
            lh = int(line[3:] or 0)
        elif line.startswith("FNF:"):
            fnf = int(line[4:] or 0)
        elif line.startswith("FNH:"):
            fnh = int(line[4:] or 0)
        elif line == "end_of_record":
            total.lines_found += lf
            total.lines_hit += lh
            total.functions_found += fnf
            total.functions_hit += fnh
            bucket = by_crate.setdefault(current_crate, CoverageBucket())
            bucket.lines_found += lf
            bucket.lines_hit += lh
            bucket.functions_found += fnf
            bucket.functions_hit += fnh

    return total, dict(sorted(by_crate.items()))


def parse_cobertura_xml(path: Path) -> tuple[CoverageBucket, dict[str, CoverageBucket]]:
    """Parse a Cobertura `coverage.xml` report into total and per-package buckets."""
    tree = ET.parse(path)  # noqa: S314 -- trusted local CI coverage artifact
    root = tree.getroot()
    total = CoverageBucket(
        lines_hit=int(root.attrib.get("lines-covered", "0") or 0),
        lines_found=int(root.attrib.get("lines-valid", "0") or 0),
    )
    by_package: dict[str, CoverageBucket] = {}
    for pkg in root.findall(".//package"):
        pkg_name = pkg.attrib.get("name", ".") or "."
        bucket = by_package.setdefault(pkg_name, CoverageBucket())
        for cls in pkg.findall(".//class"):
            for line_el in cls.findall("./lines/line"):
                bucket.lines_found += 1
                if int(line_el.attrib.get("hits", "0") or 0) > 0:
                    bucket.lines_hit += 1

    if total.lines_found == 0 and by_package:
        total.lines_found = sum(b.lines_found for b in by_package.values())
        total.lines_hit = sum(b.lines_hit for b in by_package.values())

    return total, dict(sorted(by_package.items()))


def _format_failures_section(failures: list[TestCaseRecord], label: str) -> list[str]:
    """Render failed tests as a Markdown table and expandable details."""
    if not failures:
        return []
    for record in failures[:_MAX_ANNOTATIONS]:
        first_line = (record.failure_message or "Test failed").splitlines()[0]
        _emit_annotation(
            "error",
            f"{label} test failed: {record.suite}::{record.name}",
            first_line,
        )

    lines = [
        "",
        f"### Failed Tests ({len(failures)})",
        "",
        "| Suite | Test | Duration |",
        "| --- | --- | ---: |",
    ]
    for record in failures[:50]:
        lines.append(f"| `{record.suite}` | `{record.name}` | {record.duration_seconds:.2f}s |")
    lines.append("")
    for record in failures[:20]:
        if record.failure_detail:
            snippet = _truncate_snippet(record.failure_detail)
            lines.extend([
                f"<details><summary><code>{record.suite}::{record.name}</code></summary>",
                "",
                "```text",
                snippet,
                "```",
                "",
                "</details>",
                "",
            ])
    return lines


def build_rust_report(
    junit_path: Path | None,
    lcov_path: Path | None,
    slow_seconds: float = 20.0,
) -> str:
    """Build the Markdown summary for the `Test Rust` job."""
    lines = ["## Rust Test & Coverage Summary", ""]
    if junit_path is None or not junit_path.is_file():
        lines.append("_No nextest JUnit report found._")
        return "\n".join(lines) + "\n"

    junit = parse_junit_xml(junit_path, slow_seconds=slow_seconds)
    total_failed = junit.failed + junit.errors
    status = "Passed" if total_failed == 0 else "Failed"

    line_cov_str = "—"
    fn_cov_str = "—"
    by_crate: dict[str, CoverageBucket] = {}
    if lcov_path is not None and lcov_path.is_file():
        total_cov, by_crate = parse_lcov(lcov_path)
        if total_cov.lines_found > 0:
            line_cov_str = f"{total_cov.line_percent:.1f}% ({total_cov.lines_hit:,} / {total_cov.lines_found:,})"
        if total_cov.functions_found > 0:
            fn_cov_str = (
                f"{total_cov.function_percent:.1f}% ({total_cov.functions_hit:,} / {total_cov.functions_found:,})"
            )

    lines.extend([
        "| Status | Tests Run | Passed | Failed | Skipped | Slow | Duration | Line Coverage | Function Coverage |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
        f"| **{status}** | {junit.tests:,} | {junit.passed:,} | {total_failed:,} | "
        f"{junit.skipped:,} | {len(junit.slow_cases):,} | {junit.duration_seconds:.1f}s | "
        f"{line_cov_str} | {fn_cov_str} |",
    ])

    lines.extend(_format_failures_section(junit.failures, "Rust"))

    if junit.slow_cases:
        lines.extend([
            "",
            f"<details><summary>Slow tests (&ge; {slow_seconds:.0f}s): {len(junit.slow_cases)}</summary>",
            "",
            "| Crate / Binary | Test | Duration |",
            "| --- | --- | ---: |",
        ])
        for case in junit.slow_cases[:30]:
            lines.append(f"| `{case.suite}` | `{case.name}` | {case.duration_seconds:.1f}s |")
        lines.extend(["", "</details>"])

    if by_crate:
        lines.extend([
            "",
            f"<details><summary>Coverage by crate ({len(by_crate)})</summary>",
            "",
            "| Crate | Lines | Line % | Functions | Function % |",
            "| --- | ---: | ---: | ---: | ---: |",
        ])
        for crate, cov in by_crate.items():
            lines.append(
                f"| `{crate}` | {cov.lines_hit:,} / {cov.lines_found:,} | "
                f"{cov.line_percent:.1f}% | {cov.functions_hit:,} / {cov.functions_found:,} | "
                f"{cov.function_percent:.1f}% |"
            )
        lines.extend(["", "</details>"])

    return "\n".join(lines) + "\n"


def build_python_report(
    junit_path: Path | None,
    cobertura_path: Path | None,
) -> str:
    """Build the Markdown summary for the `Test Python` job."""
    lines = ["## Python Test & Coverage Summary", ""]
    if junit_path is None or not junit_path.is_file():
        lines.append("_No pytest JUnit report found._")
        return "\n".join(lines) + "\n"

    junit = parse_junit_xml(junit_path)
    total_failed = junit.failed + junit.errors
    status = "Passed" if total_failed == 0 else "Failed"

    line_cov_str = "—"
    by_pkg: dict[str, CoverageBucket] = {}
    if cobertura_path is not None and cobertura_path.is_file():
        total_cov, by_pkg = parse_cobertura_xml(cobertura_path)
        if total_cov.lines_found > 0:
            line_cov_str = f"{total_cov.line_percent:.1f}% ({total_cov.lines_hit:,} / {total_cov.lines_found:,})"

    lines.extend([
        "| Status | Tests Run | Passed | Failed | Errors | Skipped | Duration | Line Coverage |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
        f"| **{status}** | {junit.tests:,} | {junit.passed:,} | {junit.failed:,} | "
        f"{junit.errors:,} | {junit.skipped:,} | {junit.duration_seconds:.1f}s | {line_cov_str} |",
    ])

    lines.extend(_format_failures_section(junit.failures, "Python"))

    if by_pkg:
        lines.extend([
            "",
            f"<details><summary>Coverage by package ({len(by_pkg)})</summary>",
            "",
            "| Package | Lines | Line % |",
            "| --- | ---: | ---: |",
        ])
        for pkg_name, cov in by_pkg.items():
            lines.append(f"| `{pkg_name}` | {cov.lines_hit:,} / {cov.lines_found:,} | {cov.line_percent:.1f}% |")
        lines.extend(["", "</details>"])

    return "\n".join(lines) + "\n"


_WASM_RUST_RESULT_RE = re.compile(
    r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
    r"\d+ (?:measured; \d+ )?filtered out; finished in ([0-9.]+)s"
)
_WASM_RUST_FAIL_RE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.MULTILINE)
_NODE_METRIC_RE = re.compile(
    r"^\u2139 (tests|pass|fail|skipped|cancelled|todo|duration_ms) ([0-9.]+)$",
    re.MULTILINE,
)
_NODE_FAIL_RE = re.compile(r"^\u2716 (.+?)(?: \([0-9.]+ms\))?$", re.MULTILINE)


def build_wasm_report(log_path: Path | None) -> str:
    """Build the Markdown summary for the `Test WASM` job from its log output."""
    lines = ["## WASM Test Summary", ""]
    if log_path is None or not log_path.is_file():
        lines.append("_No WASM test log found._")
        return "\n".join(lines) + "\n"

    text = log_path.read_text(encoding="utf-8", errors="replace")

    wb_passed = wb_failed = wb_ignored = 0
    wb_duration = 0.0
    wb_found = False
    for match in _WASM_RUST_RESULT_RE.finditer(text):
        wb_found = True
        wb_passed += int(match.group(2))
        wb_failed += int(match.group(3))
        wb_ignored += int(match.group(4))
        wb_duration += float(match.group(5))

    node_metrics: dict[str, float] = {}
    for match in _NODE_METRIC_RE.finditer(text):
        node_metrics[match.group(1)] = float(match.group(2))

    node_found = "tests" in node_metrics
    node_passed = int(node_metrics.get("pass", 0))
    node_failed = int(node_metrics.get("fail", 0)) + int(node_metrics.get("cancelled", 0))
    node_skipped = int(node_metrics.get("skipped", 0)) + int(node_metrics.get("todo", 0))
    node_duration = node_metrics.get("duration_ms", 0.0) / 1000.0

    jsdoc_ran = "docs:check-sync" in text or "sync-facade-jsdoc.mjs --check" in text

    wb_status = "Passed" if (wb_found and wb_failed == 0) else ("Failed" if wb_failed > 0 else "—")
    node_status = "Passed" if (node_found and node_failed == 0) else ("Failed" if node_failed > 0 else "—")
    jsdoc_status = "Executed" if jsdoc_ran else "—"

    total_passed = wb_passed + node_passed
    total_failed = wb_failed + node_failed
    total_skipped = wb_ignored + node_skipped
    total_duration = wb_duration + node_duration
    overall_status = (
        "Passed"
        if (wb_found and node_found and total_failed == 0)
        else ("Failed" if total_failed > 0 else "Incomplete")
    )

    lines.extend([
        "| Suite | Passed | Failed | Skipped | Duration | Status |",
        "| --- | ---: | ---: | ---: | ---: | --- |",
        f"| `wasm-bindgen` (`wasm-pack test --node`) | {wb_passed:,} | {wb_failed:,} | "
        f"{wb_ignored:,} | {wb_duration:.1f}s | {wb_status} |",
        f"| Node.js facade (`node --test`) | {node_passed:,} | {node_failed:,} | "
        f"{node_skipped:,} | {node_duration:.1f}s | {node_status} |",
        f"| JSDoc sync (`docs:check-sync`) | — | — | — | — | {jsdoc_status} |",
        f"| **Total** | **{total_passed:,}** | **{total_failed:,}** | "
        f"**{total_skipped:,}** | **{total_duration:.1f}s** | **{overall_status}** |",
    ])

    failed_names = [f"wasm-bindgen::{m.group(1)}" for m in _WASM_RUST_FAIL_RE.finditer(text)] + [
        f"facade::{m.group(1)}" for m in _NODE_FAIL_RE.finditer(text) if not m.group(1).startswith("failing tests:")
    ]

    if failed_names:
        for name in failed_names[:_MAX_ANNOTATIONS]:
            _emit_annotation("error", "WASM test failed", name)
        lines.extend(["", f"### Failed Tests ({len(failed_names)})", ""])
        for name in failed_names[:50]:
            lines.append(f"- `{name}`")

    return "\n".join(lines) + "\n"


def parse_sonar_report_task(path: Path) -> dict[str, str]:
    """Parse key=value pairs from `.scannerwork/report-task.txt`."""
    props: dict[str, str] = {}
    for raw_line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        props[key.strip()] = value.strip()
    return props


def _sonar_get_json(url: str, token: str | None) -> dict:
    """Perform an authenticated GET against the SonarQube API and decode JSON."""
    if not url.startswith(("https://", "http://")):
        msg = f"Unsupported URL scheme: {url}"
        raise ValueError(msg)
    headers = {"Accept": "application/json"}
    if token:
        encoded = base64.b64encode(f"{token}:".encode()).decode("ascii")
        headers["Authorization"] = f"Basic {encoded}"
    req = Request(url, headers=headers)  # noqa: S310 -- validated http(s) SonarQube API URL
    with urlopen(req, timeout=30) as response:  # noqa: S310
        return json.loads(response.read().decode("utf-8"))


def _format_sonar_metric_value(metric_key: str, raw_value: str) -> str:
    """Format a SonarQube metric value with ratings (A-E), percentages, or commas."""
    if "rating" in metric_key:
        letter = _RATING_LETTERS.get(raw_value, raw_value)
        return f"{letter}"
    if "coverage" in metric_key or "density" in metric_key or "hotspots_reviewed" in metric_key:
        try:
            return f"{float(raw_value):.1f}%"
        except ValueError:
            return f"{raw_value}%"
    try:
        if "." not in raw_value:
            return f"{int(raw_value):,}"
    except ValueError:
        pass
    return raw_value


def render_sonarqube_markdown(
    dashboard_url: str,
    quality_gate_payload: dict | None,
    measures_payload: dict | None,
) -> tuple[str, str]:
    """Render SonarQube Quality Gate and measures payloads as Markdown.

    Returns:
        Tuple of `(markdown, gate_status)` where `gate_status` is `OK`, `WARN`,
        `ERROR`, or `UNKNOWN`.
    """
    project_status = (quality_gate_payload or {}).get("projectStatus") or {}
    gate_status = str(project_status.get("status", "UNKNOWN"))
    gate_label = {
        "OK": "PASSED",
        "WARN": "WARNING",
        "ERROR": "FAILED",
    }.get(gate_status, gate_status)

    lines = [
        "## SonarQube Analysis Summary",
        "",
        f"- **Quality Gate:** **{gate_label}** (`{gate_status}`)",
        f"- **Dashboard:** [{dashboard_url}]({dashboard_url})",
    ]

    conditions = project_status.get("conditions") or []
    if conditions:
        lines.extend([
            "",
            "### Quality Gate Conditions",
            "",
            "| Condition | Status | Actual | Threshold |",
            "| --- | --- | ---: | ---: |",
        ])
        for cond in conditions:
            metric_key = str(cond.get("metricKey", ""))
            label = _METRIC_LABELS.get(metric_key, f"`{metric_key}`")
            cond_status = str(cond.get("status", "—"))
            actual = _format_sonar_metric_value(metric_key, str(cond.get("actualValue", "—")))
            comparator = _COMPARATOR_SYMBOLS.get(str(cond.get("comparator", "")), str(cond.get("comparator", "")))
            threshold_raw = str(cond.get("errorThreshold", "—"))
            threshold = (
                f"{comparator} {_format_sonar_metric_value(metric_key, threshold_raw)}".strip()
                if threshold_raw != "—"
                else "—"
            )
            lines.append(f"| {label} | **{cond_status}** | {actual} | {threshold} |")

    component = (measures_payload or {}).get("component") or {}
    measures_list = component.get("measures") or []
    measures = {str(item.get("metric")): str(item.get("value", "")) for item in measures_list if item.get("metric")}
    if measures:
        lines.extend([
            "",
            "### Project Measures",
            "",
            "| Metric | Value | Rating |",
            "| --- | ---: | :---: |",
        ])
        rows = [
            ("coverage", None),
            ("bugs", "reliability_rating"),
            ("vulnerabilities", "security_rating"),
            ("security_hotspots", None),
            ("code_smells", "sqale_rating"),
            ("duplicated_lines_density", None),
            ("ncloc", None),
        ]
        for metric_key, rating_key in rows:
            if metric_key not in measures:
                continue
            label = _METRIC_LABELS.get(metric_key, metric_key)
            val = _format_sonar_metric_value(metric_key, measures[metric_key])
            rating = (
                _format_sonar_metric_value(rating_key, measures[rating_key])
                if rating_key and rating_key in measures
                else "—"
            )
            lines.append(f"| {label} | {val} | {rating} |")

    return "\n".join(lines) + "\n", gate_status


def build_sonarqube_report(
    report_task_path: Path,
    token: str | None,
    timeout_seconds: int = 180,
    poll_interval_seconds: int = 5,
) -> str:
    """Poll SonarQube compute engine task and build the Markdown job summary."""
    if not report_task_path.is_file():
        return f"## SonarQube Analysis Summary\n\n_Scanner metadata `{report_task_path}` not found._\n"

    props = parse_sonar_report_task(report_task_path)
    server_url = props.get("serverUrl", "https://sonarcloud.io").rstrip("/")
    project_key = props.get("projectKey", "jeickmeier_finstack-quant")
    dashboard_url = props.get("dashboardUrl", f"{server_url}/dashboard?id={project_key}")
    ce_task_url = props.get("ceTaskUrl")

    analysis_id: str | None = None
    if ce_task_url:
        deadline = time.monotonic() + timeout_seconds
        while True:
            try:
                task_payload = _sonar_get_json(ce_task_url, token)
            except (HTTPError, URLError, TimeoutError, ValueError, json.JSONDecodeError) as exc:
                print(f"SonarQube CE task query failed: {exc}", file=sys.stderr)
                break
            task = task_payload.get("task") or {}
            status = str(task.get("status", ""))
            if status in {"PENDING", "IN_PROGRESS"} and time.monotonic() < deadline:
                time.sleep(poll_interval_seconds)
                continue
            analysis_id = task.get("analysisId")
            break

    quality_gate_payload: dict | None = None
    measures_payload: dict | None = None
    try:
        qg_query = urlencode({"analysisId": analysis_id}) if analysis_id else urlencode({"projectKey": project_key})
        quality_gate_payload = _sonar_get_json(
            f"{server_url}/api/qualitygates/project_status?{qg_query}",
            token,
        )
    except (HTTPError, URLError, TimeoutError, ValueError, json.JSONDecodeError) as exc:
        print(f"SonarQube quality gate query failed: {exc}", file=sys.stderr)

    try:
        metric_keys = ",".join([
            "bugs",
            "vulnerabilities",
            "security_hotspots",
            "code_smells",
            "coverage",
            "duplicated_lines_density",
            "ncloc",
            "reliability_rating",
            "security_rating",
            "sqale_rating",
        ])
        measures_query = urlencode({"component": project_key, "metricKeys": metric_keys})
        measures_payload = _sonar_get_json(
            f"{server_url}/api/measures/component?{measures_query}",
            token,
        )
    except (HTTPError, URLError, TimeoutError, ValueError, json.JSONDecodeError) as exc:
        print(f"SonarQube measures query failed: {exc}", file=sys.stderr)

    markdown, gate_status = render_sonarqube_markdown(
        dashboard_url=dashboard_url,
        quality_gate_payload=quality_gate_payload,
        measures_payload=measures_payload,
    )
    annotation_level = "notice" if gate_status == "OK" else "warning"
    _emit_annotation(
        annotation_level,
        f"SonarQube Quality Gate: {gate_status}",
        f"Results: {dashboard_url}",
    )
    return markdown


def _resolve_default_rust_junit() -> Path | None:
    """Locate the most recent nextest `junit.xml` across standard target paths."""
    target_root = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    candidates = [
        target_root / "llvm-cov-target" / "nextest" / "default" / "junit.xml",
        target_root / "nextest" / "default" / "junit.xml",
    ]
    existing = [candidate for candidate in candidates if candidate.is_file()]
    if not existing:
        return None
    existing.sort(key=lambda item: item.stat().st_mtime, reverse=True)
    return existing[0]


def main(argv: list[str] | None = None) -> int:
    """CLI entry point for generating CI job summaries."""
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    rust_parser = subparsers.add_parser("rust", help="Summarize Rust nextest and LCOV results")
    rust_parser.add_argument("--junit", type=Path, default=None)
    rust_parser.add_argument("--lcov", type=Path, default=Path("coverage/rust/lcov.info"))
    rust_parser.add_argument("--slow-seconds", type=float, default=20.0)

    py_parser = subparsers.add_parser("python", help="Summarize pytest and Cobertura results")
    py_parser.add_argument("--junit", type=Path, default=Path("coverage/python/junit.xml"))
    py_parser.add_argument("--cobertura", type=Path, default=Path("coverage/python/coverage.xml"))

    wasm_parser = subparsers.add_parser("wasm", help="Summarize wasm-test output")
    wasm_parser.add_argument("--log", type=Path, default=Path("coverage/wasm/wasm-test.log"))

    sonar_parser = subparsers.add_parser("sonarqube", help="Summarize SonarQube scan results")
    sonar_parser.add_argument(
        "--report-task",
        type=Path,
        default=Path(".scannerwork/report-task.txt"),
    )
    sonar_parser.add_argument("--timeout-seconds", type=int, default=180)
    sonar_parser.add_argument("--poll-interval-seconds", type=int, default=5)

    args = parser.parse_args(argv)

    if args.command == "rust":
        junit_path = args.junit or _resolve_default_rust_junit()
        _write_summary(
            build_rust_report(
                junit_path=junit_path,
                lcov_path=args.lcov,
                slow_seconds=args.slow_seconds,
            )
        )
    elif args.command == "python":
        _write_summary(
            build_python_report(
                junit_path=args.junit,
                cobertura_path=args.cobertura,
            )
        )
    elif args.command == "wasm":
        _write_summary(build_wasm_report(log_path=args.log))
    elif args.command == "sonarqube":
        _write_summary(
            build_sonarqube_report(
                report_task_path=args.report_task,
                token=os.environ.get("SONAR_TOKEN"),
                timeout_seconds=args.timeout_seconds,
                poll_interval_seconds=args.poll_interval_seconds,
            )
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
