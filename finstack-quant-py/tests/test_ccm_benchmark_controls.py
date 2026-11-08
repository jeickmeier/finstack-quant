"""Checks for private CCM benchmark sampling and validity controls, without bindings."""

from __future__ import annotations

from collections.abc import Callable
import importlib.util
import json
import math
from pathlib import Path
from types import ModuleType, SimpleNamespace

import pytest


@pytest.fixture(scope="module")
def controls() -> ModuleType:
    path = Path(__file__).resolve().parents[1] / "benchmarks" / "_ccm.py"
    spec = importlib.util.spec_from_file_location("ccm_benchmark_controls", path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class _BenchmarkProbe:
    fullname = "finstack-quant-py/benchmarks/bench_bindings.py::TestCoreBenchmarks::test_currency_creation"
    disabled = False
    has_error = False

    def __init__(self) -> None:
        self.extra_info: dict[str, object] = {}
        self.settings: dict[str, int] | None = None

    def pedantic(self, operation: Callable[[], float], **settings: int) -> float:
        self.settings = settings
        if self.disabled:
            return operation()
        for _ in range(settings["warmup_rounds"] + settings["rounds"]):
            for _ in range(settings["iterations"]):
                result = operation()
        return operation() if settings["iterations"] > 1 else result


@pytest.mark.parametrize("raw", ["", "1.5", "NaN", "-1", "0"])
def test_invalid_round_counts_fail_before_measurement(
    controls: ModuleType, monkeypatch: pytest.MonkeyPatch, raw: str
) -> None:
    monkeypatch.setenv("FQ_CCM_PY_ROUNDS", raw)
    benchmark = _BenchmarkProbe()
    with pytest.raises(ValueError, match="FQ_CCM_PY_ROUNDS"):
        controls._run_checked(
            benchmark,
            lambda: 7.0,
            controls._scalar(7.0),
            parameters={},
            boundary="probe",
            identity={},
        )
    assert benchmark.settings is None
    assert benchmark.extra_info == {}


def test_fixed_batches_and_explicit_warmup_are_forwarded(controls: ModuleType, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("FQ_CCM_PY_ROUNDS", "10")
    monkeypatch.setenv("FQ_CCM_PY_WARMUP_ROUNDS", "1")
    benchmark = _BenchmarkProbe()
    calls = 0

    def operation() -> float:
        nonlocal calls
        calls += 1
        return 7.0

    result = controls._run_checked(
        benchmark,
        operation,
        controls._scalar(7.0),
        parameters={"fixed_input": 3},
        boundary="probe boundary",
        identity={"release_profile_verified_by_harness": False},
    )
    assert result == 7.0
    assert benchmark.settings == {"rounds": 10, "warmup_rounds": 1, "iterations": 512}
    assert calls == 1 + (10 + 1) * 512 + 1
    assert benchmark.extra_info["sampling"]["normalized_unit"] == "seconds per operation"
    assert benchmark.extra_info["parameters"] == {"fixed_input": 3}
    assert benchmark.extra_info["preflight_output"] == benchmark.extra_info["observed_output"] == 7.0
    assert benchmark.extra_info["extra_result_call"] is True
    assert benchmark.extra_info["timings_collected"] is True
    json.dumps(benchmark.extra_info, allow_nan=False)


def test_invalid_warmup_counts_are_rejected(controls: ModuleType, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("FQ_CCM_PY_WARMUP_ROUNDS", "-1")
    with pytest.raises(ValueError, match="FQ_CCM_PY_WARMUP_ROUNDS"):
        controls._settings()


def test_disabled_measurement_is_explicitly_labelled(controls: ModuleType, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("FQ_CCM_PY_ROUNDS", raising=False)
    monkeypatch.delenv("FQ_CCM_PY_WARMUP_ROUNDS", raising=False)
    benchmark = _BenchmarkProbe()
    benchmark.disabled = True
    controls._run_checked(
        benchmark,
        lambda: 7.0,
        controls._scalar(7.0),
        parameters={},
        boundary="probe",
        identity={},
    )
    assert benchmark.extra_info["timings_collected"] is False
    assert benchmark.extra_info["extra_result_call"] is False


def test_invalid_preflight_prevents_a_timing_record(controls: ModuleType) -> None:
    benchmark = _BenchmarkProbe()
    with pytest.raises(AssertionError):
        controls._run_checked(
            benchmark,
            lambda: float("nan"),
            controls._scalar(7.0),
            parameters={},
            boundary="probe",
            identity={},
        )
    assert benchmark.settings is None
    assert benchmark.extra_info == {}
    assert benchmark.has_error is True


def test_changed_final_output_cannot_be_reported_as_success(
    controls: ModuleType, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("FQ_CCM_PY_ROUNDS", "1")
    monkeypatch.setenv("FQ_CCM_PY_WARMUP_ROUNDS", "0")
    benchmark = _BenchmarkProbe()
    calls = 0

    def operation() -> float:
        nonlocal calls
        calls += 1
        return 7.0 if calls == 1 else 8.0

    with pytest.raises(AssertionError):
        controls._run_checked(
            benchmark,
            operation,
            controls._scalar(7.0),
            parameters={},
            boundary="probe",
            identity={},
        )
    assert benchmark.extra_info == {}
    assert benchmark.has_error is True


def test_operation_errors_propagate_without_a_success_record(controls: ModuleType) -> None:
    benchmark = _BenchmarkProbe()

    def operation() -> float:
        raise ValueError("controlled operation failure")

    with pytest.raises(ValueError, match="controlled operation failure"):
        controls._run_checked(
            benchmark,
            operation,
            controls._scalar(7.0),
            parameters={},
            boundary="probe",
            identity={},
        )
    assert benchmark.settings is None
    assert benchmark.extra_info == {}
    assert benchmark.has_error is True


def test_cholesky_check_observes_all_entries(controls: ModuleType) -> None:
    factor = [[2.0, 0.0], [1.0, math.sqrt(2.0)]]
    matrix = [[4.0, 2.0], [2.0, 3.0]]
    assert controls._cholesky(factor, matrix) == factor
    with pytest.raises(AssertionError):
        controls._cholesky([[2.0, 0.0], [1.0, 1.0]], matrix)
    with pytest.raises(AssertionError):
        controls._cholesky([[2.0, 0.1], [1.0, math.sqrt(2.0)]], matrix)


def test_bernoulli_check_rejects_wrong_marginals_despite_unit_sum(controls: ModuleType) -> None:
    with pytest.raises(AssertionError):
        controls._bernoulli([0.25, 0.25, 0.25, 0.25])


def test_estimate_checks_independent_and_physical_counts_separately(controls: ModuleType) -> None:
    mean = SimpleNamespace(currency="USD", amount=10.0)
    lower = SimpleNamespace(currency="USD", amount=9.8)
    upper = SimpleNamespace(currency="USD", amount=10.2)
    payload = {"mean": {"amount": "10", "currency": "USD"}, "num_paths": 5_000, "num_simulated_paths": 10_000}
    estimate = SimpleNamespace(
        mean=mean,
        ci_lower=lower,
        ci_upper=upper,
        stderr=0.1,
        std_dev=None,
        num_paths=5_000,
        num_simulated_paths=10_000,
        to_json=lambda: json.dumps(payload),
    )
    assert controls._estimate(estimate, independent=5_000, physical=10_000) == payload
    with pytest.raises(AssertionError):
        controls._estimate(estimate, independent=10_000, physical=10_000)
    with pytest.raises(AssertionError):
        controls._estimate(estimate, independent=5_000, physical=5_000)


def test_source_and_receipt_payloads_are_hashed_without_release_claims(
    controls: ModuleType, monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    record = tmp_path / "receipt.json"
    record.write_text('{"profile":"release","exit":0}', encoding="utf-8")
    monkeypatch.setenv("FQ_CCM_BUILD_RECEIPT", str(record))
    captured = controls._external_record("FQ_CCM_BUILD_RECEIPT")
    assert captured["payload"] == {"profile": "release", "exit": 0}
    assert len(captured["sha256"]) == 64
    assert "release_profile_verified_by_harness" not in captured
    record.write_text("invalid JSON", encoding="utf-8")
    with pytest.raises(json.JSONDecodeError):
        controls._external_record("FQ_CCM_BUILD_RECEIPT")


def test_required_case_ledger_has_all_domains_and_fixed_mc_single_iterations(controls: ModuleType) -> None:
    assert len(controls._ITERATIONS) == 22
    assert sum(name.startswith("TestCoreBenchmarks::") for name in controls._ITERATIONS) == 10
    assert sum(name.startswith("TestCorrelationBenchmarks::") for name in controls._ITERATIONS) == 7
    assert sum(name.startswith("TestMonteCarloBenchmarks::") for name in controls._ITERATIONS) == 5
    for name in ("test_lsmc_american_put", "test_european_pricer", "test_path_dependent_asian"):
        assert controls._ITERATIONS[f"TestMonteCarloBenchmarks::{name}"] == 1
