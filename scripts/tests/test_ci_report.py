"""Unit tests for `scripts/ci_report.py`."""

from __future__ import annotations

from typing import TYPE_CHECKING

from scripts import ci_report

if TYPE_CHECKING:
    from pathlib import Path

    import pytest


def test_build_rust_report_with_junit_and_lcov(tmp_path: Path) -> None:
    """Rust report summarizes JUnit test counts, slow tests, failures, and LCOV coverage."""
    junit_path = tmp_path / "junit.xml"
    junit_path.write_text(
        """<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="1" errors="0" time="25.5">
  <testsuite name="finstack-quant-core" tests="2" disabled="0" errors="0" failures="0">
    <testcase name="currency::fast_case" classname="finstack-quant-core" time="0.05"/>
    <testcase name="rates::slow_case" classname="finstack-quant-core" time="22.40"/>
  </testsuite>
  <testsuite name="finstack-quant-valuations::rates" tests="1" disabled="0" errors="0" failures="1">
    <testcase name="swaption::broken_case" classname="finstack-quant-valuations::rates" time="3.05">
      <failure message="assertion failed: left == right">panicked at line 42</failure>
    </testcase>
  </testsuite>
</testsuites>
""",
        encoding="utf-8",
    )

    lcov_path = tmp_path / "lcov.info"
    lcov_path.write_text(
        """SF:/workspace/finstack-quant/core/src/currency.rs
FNF:10
FNH:8
LF:100
LH:90
end_of_record
SF:/workspace/finstack-quant/valuations/rates/src/swaption.rs
FNF:5
FNH:4
LF:50
LH:40
end_of_record
""",
        encoding="utf-8",
    )

    report = ci_report.build_rust_report(junit_path, lcov_path, slow_seconds=20.0)

    assert "## Rust Test & Coverage Summary" in report
    assert "| **Failed** | 3 | 2 | 1 | 0 | 1 | 25.5s | 86.7% (130 / 150) | 80.0% (12 / 15) |" in report
    assert "### Failed Tests (1)" in report
    assert "`finstack-quant-valuations::rates`" in report
    assert "`swaption::broken_case`" in report
    assert "panicked at line 42" in report
    assert "Slow tests (&ge; 20s): 1" in report
    assert "`rates::slow_case`" in report
    assert "`core`" in report
    assert "`valuations/rates`" in report


def test_build_python_report_with_junit_and_cobertura(tmp_path: Path) -> None:
    """Python report summarizes pytest JUnit counts and Cobertura package coverage."""
    junit_path = tmp_path / "junit.xml"
    junit_path.write_text(
        """<?xml version="1.0" encoding="utf-8"?>
<testsuites>
  <testsuite name="pytest" errors="0" failures="0" skipped="1" tests="4" time="12.3">
    <testcase classname="tests.test_core" name="test_money" time="0.1"/>
    <testcase classname="tests.test_core" name="test_dates" time="0.2"/>
    <testcase classname="tests.test_core" name="test_curves" time="0.3"/>
    <testcase classname="tests.test_core" name="test_skipped" time="0.0">
      <skipped message="optional"/>
    </testcase>
  </testsuite>
</testsuites>
""",
        encoding="utf-8",
    )

    cobertura_path = tmp_path / "coverage.xml"
    cobertura_path.write_text(
        """<?xml version="1.0" ?>
<coverage lines-valid="200" lines-covered="190" line-rate="0.95">
  <packages>
    <package name="finstack_quant.core" line-rate="0.95">
      <classes>
        <class name="__init__.py" filename="finstack_quant/core/__init__.py">
          <lines>
            <line number="1" hits="1"/>
            <line number="2" hits="1"/>
            <line number="3" hits="0"/>
          </lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>
""",
        encoding="utf-8",
    )

    report = ci_report.build_python_report(junit_path, cobertura_path)

    assert "## Python Test & Coverage Summary" in report
    assert "| **Passed** | 4 | 3 | 0 | 0 | 1 | 12.3s | 95.0% (190 / 200) |" in report
    assert "`finstack_quant.core` | 2 / 3 | 66.7%" in report


def test_build_python_report_without_cobertura(tmp_path: Path) -> None:
    """Python report without a Cobertura file shows coverage as unavailable."""
    junit_path = tmp_path / "junit.xml"
    junit_path.write_text(
        """<?xml version="1.0" encoding="utf-8"?>
<testsuites>
  <testsuite name="pytest" errors="0" failures="0" skipped="0" tests="1" time="0.5">
    <testcase classname="tests.test_core" name="test_money" time="0.5"/>
  </testsuite>
</testsuites>
""",
        encoding="utf-8",
    )

    report = ci_report.build_python_report(junit_path, tmp_path / "missing.xml")

    assert "| **Passed** | 1 | 1 | 0 | 0 | 0 | 0.5s | — |" in report
    assert "Coverage by package" not in report


def test_build_wasm_report_parses_wasm_pack_and_node_test(tmp_path: Path) -> None:
    """WASM report aggregates wasm-bindgen and Node.js facade test results."""
    log_path = tmp_path / "wasm-test.log"
    log_path.write_text(
        "Running unittests src/lib.rs\n"
        "running 4 tests\n"
        "test result: ok. 4 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.04s\n"
        "Running tests/wasm.rs\n"
        "running 234 tests\n"
        "test result: ok. 234 passed; 0 failed; 0 ignored; 0 filtered out; finished in 3.36s\n"
        "\u2139 tests 950\n"
        "\u2139 suites 0\n"
        "\u2139 pass 950\n"
        "\u2139 fail 0\n"
        "\u2139 cancelled 0\n"
        "\u2139 skipped 0\n"
        "\u2139 todo 0\n"
        "\u2139 duration_ms 139800.0\n"
        "> finstack-quant-wasm@0.11.0 docs:check-sync\n"
        "> node scripts/sync-facade-jsdoc.mjs --check\n",
        encoding="utf-8",
    )

    report = ci_report.build_wasm_report(log_path)

    assert "## WASM Test Summary" in report
    assert "| `wasm-bindgen` (`wasm-pack test --node`) | 238 | 0 | 0 | 3.4s | Passed |" in report
    assert "| Node.js facade (`node --test`) | 950 | 0 | 0 | 139.8s | Passed |" in report
    assert "| JSDoc sync (`docs:check-sync`) | — | — | — | — | Executed |" in report
    assert "| **Total** | **1,188** | **0** | **0** | **143.2s** | **Passed** |" in report


def test_render_sonarqube_markdown_formats_conditions_and_measures() -> None:
    """SonarQube renderer produces Quality Gate conditions and project measures tables."""
    qg_payload = {
        "projectStatus": {
            "status": "OK",
            "conditions": [
                {
                    "status": "OK",
                    "metricKey": "new_reliability_rating",
                    "comparator": "GT",
                    "errorThreshold": "1",
                    "actualValue": "1",
                },
                {
                    "status": "OK",
                    "metricKey": "new_coverage",
                    "comparator": "LT",
                    "errorThreshold": "80",
                    "actualValue": "85.25",
                },
            ],
        }
    }
    measures_payload = {
        "component": {
            "measures": [
                {"metric": "coverage", "value": "83.9"},
                {"metric": "bugs", "value": "1"},
                {"metric": "reliability_rating", "value": "3.0"},
                {"metric": "vulnerabilities", "value": "0"},
                {"metric": "security_rating", "value": "1.0"},
                {"metric": "code_smells", "value": "5"},
                {"metric": "sqale_rating", "value": "1.0"},
                {"metric": "duplicated_lines_density", "value": "0.8"},
                {"metric": "ncloc", "value": "2020043"},
            ]
        }
    }

    markdown, status = ci_report.render_sonarqube_markdown(
        dashboard_url="https://sonarcloud.io/dashboard?id=jeickmeier_finstack-quant",
        quality_gate_payload=qg_payload,
        measures_payload=measures_payload,
    )

    assert status == "OK"
    assert "**Quality Gate:** **PASSED** (`OK`)" in markdown
    assert "| Reliability Rating on New Code | **OK** | A | > A |" in markdown
    assert "| Coverage on New Code | **OK** | 85.2% | < 80.0% |" in markdown
    assert "| Coverage | 83.9% | — |" in markdown
    assert "| Bugs | 1 | C |" in markdown
    assert "| Lines of Code | 2,020,043 | — |" in markdown


def test_cli_writes_to_github_step_summary(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """CLI appends the generated Markdown report to `$GITHUB_STEP_SUMMARY`."""
    summary_file = tmp_path / "step_summary.md"
    monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary_file))

    log_path = tmp_path / "wasm.log"
    log_path.write_text(
        "test result: ok. 4 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.1s\n"
        "\u2139 tests 10\n\u2139 pass 10\n\u2139 fail 0\n\u2139 skipped 0\n\u2139 duration_ms 500\n",
        encoding="utf-8",
    )

    rc = ci_report.main(["wasm", "--log", str(log_path)])
    assert rc == 0
    written = summary_file.read_text(encoding="utf-8")
    assert "## WASM Test Summary" in written
    assert "| **Total** | **14** | **0** | **0** | **0.6s** | **Passed** |" in written
