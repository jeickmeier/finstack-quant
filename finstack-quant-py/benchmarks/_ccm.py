"""Private checked controls for the core, correlation and model benchmark rows."""

from __future__ import annotations

from datetime import UTC, datetime
from functools import lru_cache
from hashlib import sha256
from importlib.metadata import version
import json
import math
import os
from pathlib import Path
import platform
import sys

_PROTOCOL = "ccm-host-v1"
_SCALAR_ABS_TOL = 1e-12
_SCALAR_REL_TOL = 1e-12
_CHOLESKY_ABS_TOL = 1e-11
_STATS_ABS_TOL = 1e-10
_BS_ABS_TOL = 1e-10
# The fixed Gaussian conditional-PD fixture uses statrs' central erfc branch.
# Match the owning CDF precision gate in core/math/special_functions.rs rather
# than imposing the general scalar arithmetic bound on that approximation.
_COPULA_ABS_TOL = 1e-11
_MC_SANITY_STDERRS = 6.0
_NUMERICAL_BOUNDS = {
    "scalar": {"absolute": _SCALAR_ABS_TOL, "relative": _SCALAR_REL_TOL},
    "cholesky_reconstruction": {"absolute": _CHOLESKY_ABS_TOL, "relative": _SCALAR_REL_TOL},
    "stats": {"absolute": _STATS_ABS_TOL, "relative": _SCALAR_REL_TOL},
    "black_scholes": {"absolute": _BS_ABS_TOL, "relative": _SCALAR_REL_TOL},
    "gaussian_conditional_pd": {"absolute": _COPULA_ABS_TOL, "relative": _SCALAR_REL_TOL},
    "discount_df": {"exclusive_lower": 0.75, "exclusive_upper": 0.95, "pillar_absolute": _SCALAR_ABS_TOL},
    "mc": {
        "exclusive_mean_lower": 0.0,
        "exclusive_mean_upper": 100.0,
        "sanity_stderrs": _MC_SANITY_STDERRS,
        "sanity_absolute": _BS_ABS_TOL,
    },
}

# Frozen before the first CCM-B2 host execution. Pytest-benchmark divides each
# measured batch by this iteration count and persists every normalized sample.
_ITERATIONS = {
    "TestCoreBenchmarks::test_currency_creation": 512,
    "TestCoreBenchmarks::test_money_add_sub": 256,
    "TestCoreBenchmarks::test_daycount_year_fraction": 512,
    "TestCoreBenchmarks::test_discount_curve_df": 512,
    "TestCoreBenchmarks::test_cholesky_5x5": 16,
    "TestCoreBenchmarks::test_forward_curve_rate": 512,
    "TestCoreBenchmarks::test_fx_matrix_rate": 256,
    "TestCoreBenchmarks::test_stats_mean_variance": 8,
    "TestCoreBenchmarks::test_tenor_parsing": 256,
    "TestCoreBenchmarks::test_rate_conversions": 256,
    "TestCorrelationBenchmarks::test_copula_build_and_conditional": 128,
    "TestCorrelationBenchmarks::test_correlated_bernoulli": 128,
    "TestCorrelationBenchmarks::test_recovery_model": 128,
    "TestCorrelationBenchmarks::test_factor_model": 128,
    "TestCorrelationBenchmarks::test_single_factor_model_construction": 128,
    "TestCorrelationBenchmarks::test_correlation_bounds": 512,
    "TestCorrelationBenchmarks::test_validate_correlation_matrix": 64,
    "TestMonteCarloBenchmarks::test_black_scholes_call": 512,
    "TestMonteCarloBenchmarks::test_lsmc_american_put": 1,
    "TestMonteCarloBenchmarks::test_european_pricer": 1,
    "TestMonteCarloBenchmarks::test_path_dependent_asian": 1,
    "TestMonteCarloBenchmarks::test_black_scholes_put": 512,
}


def _integer_setting(name, default, minimum):
    raw = os.environ.get(name, str(default))
    try:
        value = int(raw)
    except ValueError as error:
        raise ValueError(f"{name} must be an integer of at least {minimum}, got {raw!r}") from error
    if value < minimum:
        raise ValueError(f"{name} must be an integer of at least {minimum}, got {raw!r}")
    return value


def _settings():
    return {
        "rounds": _integer_setting("FQ_CCM_PY_ROUNDS", 10, 1),
        "warmup_rounds": _integer_setting("FQ_CCM_PY_WARMUP_ROUNDS", 1, 0),
    }


def _file_identity(path):
    resolved = Path(path).resolve(strict=True)
    return {"path": str(resolved), "sha256": sha256(resolved.read_bytes()).hexdigest()}


def _external_record(name):
    path = os.environ.get(name)
    if not path:
        return None
    identity = _file_identity(path)
    identity["payload"] = json.loads(Path(identity["path"]).read_text(encoding="utf-8"))
    return identity


@lru_cache(maxsize=1)
def _runtime_identity():
    # The existing benchmark imports have already loaded this one extension.
    # Its identity does not, by itself, establish its optimization profile.
    extension = sys.modules["finstack_quant.finstack_quant"]
    return {
        "python": sys.version,
        "executable": sys.executable,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "pytest_benchmark": version("pytest-benchmark"),
        "numpy": version("numpy"),
        "extension": _file_identity(extension.__file__),
        "harness": _file_identity(Path(__file__).with_name("bench_bindings.py")),
        "helper": _file_identity(__file__),
        "source_identity": _external_record("FQ_CCM_SOURCE_IDENTITY"),
        "build_receipt": _external_record("FQ_CCM_BUILD_RECEIPT"),
        "release_profile_verified_by_harness": False,
        "release_evidence": "The external build receipt and campaign review establish the release profile.",
        "rayon_num_threads": os.environ.get("RAYON_NUM_THREADS"),
    }


def _run_checked(benchmark, operation, validate, *, parameters, boundary, identity):
    case_id = "::".join(benchmark.fullname.split("::")[-2:])
    iterations = _ITERATIONS[case_id]
    settings = _settings()
    started = datetime.now(UTC).isoformat()
    try:
        preflight = validate(operation())
        result = benchmark.pedantic(operation, iterations=iterations, **settings)
        observed = validate(result)
        assert observed == preflight, f"{case_id}: deterministic preflight and final output differ"
    except Exception:
        # Pytest-benchmark records errors inside pedantic itself, but validation
        # deliberately runs outside timing. Failed postchecks must also exclude
        # the row from its success JSON rather than leave valid-looking stats.
        benchmark.has_error = True
        raise
    benchmark.extra_info.update({
        "ccm_protocol": _PROTOCOL,
        "case_id": case_id,
        "parameters": parameters,
        "parameters_sha256": sha256(json.dumps(parameters, sort_keys=True, allow_nan=False).encode()).hexdigest(),
        "sampling": {**settings, "iterations": iterations, "normalized_unit": "seconds per operation"},
        "numerical_bounds": _NUMERICAL_BOUNDS,
        "timing_boundary": boundary,
        "validation_boundary": "One untimed preflight and the complete returned result after measurement.",
        "extra_result_call": iterations > 1 and not benchmark.disabled,
        "timings_collected": not benchmark.disabled,
        "preflight_output": preflight,
        "observed_output": observed,
        "started_utc": started,
        "completed_utc": datetime.now(UTC).isoformat(),
        "identity": identity,
    })
    return result


def _finite(value):
    assert math.isfinite(value), f"non-finite benchmark output: {value!r}"
    return float(value)


def _close(value, expected, *, abs_tol=_SCALAR_ABS_TOL, rel_tol=_SCALAR_REL_TOL):
    actual = _finite(value)
    assert math.isclose(actual, expected, abs_tol=abs_tol, rel_tol=rel_tol), (actual, expected)
    return actual


def _scalar(expected, *, abs_tol=_SCALAR_ABS_TOL):
    return lambda value: _close(value, expected, abs_tol=abs_tol)


def _currency(value):
    assert str(value) == "USD"
    assert value.numeric == 840
    assert value.decimals == 2
    return {"code": str(value), "numeric": value.numeric, "decimals": value.decimals}


def _money(value, expected):
    assert str(value.currency) == "USD"
    assert _finite(value.amount) == expected
    return json.loads(value.to_json())


def _discount(value):
    actual = _finite(value)
    assert 0.75 < actual < 0.95
    return actual


def _cholesky(value, matrix):
    n = len(matrix)
    assert len(value) == n
    for i, row in enumerate(value):
        assert len(row) == n
        assert _finite(row[i]) > 0.0
        for j, item in enumerate(row):
            _finite(item)
            if j > i:
                assert item == 0.0
    for i in range(n):
        for j in range(n):
            reconstructed = math.fsum(value[i][k] * value[j][k] for k in range(n))
            _close(reconstructed, matrix[i][j], abs_tol=_CHOLESKY_ABS_TOL)
    return [list(row) for row in value]


def _fx(value):
    _close(value.rate, 0.92)
    assert value.triangulated is False
    return {"rate": value.rate, "triangulated": value.triangulated}


def _mean_variance(value):
    assert len(value) == 2
    n = 10_000
    _close(value[0], 0.01 * (n - 1) / 2.0, abs_tol=_STATS_ABS_TOL)
    _close(value[1], 0.01**2 * n * (n + 1) / 12.0, abs_tol=_STATS_ABS_TOL)
    return list(value)


def _tenor(value):
    assert value.count == 3
    assert value.months == 3
    assert value.to_years() == 0.25
    return {"count": value.count, "months": value.months, "years": value.to_years()}


def _rates(value):
    assert len(value) == 2
    for rate in value:
        _close(rate.as_decimal, 0.05)
        assert rate.as_percent == 5.0
        assert rate.as_bp == 500
    return [{"decimal": rate.as_decimal, "percent": rate.as_percent, "bp": rate.as_bp} for rate in value]


def _normal_cdf(value):
    return 0.5 * math.erfc(-value / math.sqrt(2.0))


def _bernoulli(value):
    assert len(value) == 4
    p1, p2, rho = 0.3, 0.5, 0.2
    p11 = p1 * p2 + rho * math.sqrt(p1 * (1 - p1) * p2 * (1 - p2))
    expected = (p11, p1 - p11, p2 - p11, 1.0 - p1 - p2 + p11)
    for actual, target in zip(value, expected, strict=True):
        assert 0.0 <= _close(actual, target) <= 1.0
    _close(math.fsum(value), 1.0)
    _close(value[0] + value[1], p1)
    _close(value[0] + value[2], p2)
    return list(value)


def _factor(value):
    assert value.num_factors == 1
    assert value.volatility == 0.15
    assert value.mean_reversion == 0.5
    return {"factors": value.num_factors, "volatility": value.volatility, "mean_reversion": value.mean_reversion}


def _bounds(value):
    assert len(value) == 2
    p1, p2 = 0.3, 0.5
    denominator = math.sqrt(p1 * (1 - p1) * p2 * (1 - p2))
    lower = (max(0.0, p1 + p2 - 1.0) - p1 * p2) / denominator
    upper = (min(p1, p2) - p1 * p2) / denominator
    _close(value[0], lower)
    _close(value[1], upper)
    assert -1.0 <= value[0] <= value[1] <= 1.0
    return list(value)


def _none(value):
    assert value is None


def _bs_reference(*, volatility=0.2, is_call=True):
    # Independent host arithmetic, with no Finstack function used as an oracle.
    spot, strike, rate, maturity = 100.0, 100.0, 0.05, 1.0
    sigma_t = volatility * math.sqrt(maturity)
    d1 = (math.log(spot / strike) + (rate + volatility**2 / 2.0) * maturity) / sigma_t
    d2 = d1 - sigma_t
    discount = math.exp(-rate * maturity)
    if is_call:
        return spot * _normal_cdf(d1) - strike * discount * _normal_cdf(d2)
    return strike * discount * _normal_cdf(-d2) - spot * _normal_cdf(-d1)


def _estimate(value, *, independent, physical, reference=None, lower_reference=None):
    mean = value.mean
    lower = value.ci_lower
    upper = value.ci_upper
    for money in (mean, lower, upper):
        assert str(money.currency) == "USD"
        _finite(money.amount)
    assert 0.0 < mean.amount < 100.0
    stderr = _finite(value.stderr)
    assert stderr > 0.0
    assert lower.amount <= mean.amount <= upper.amount
    assert value.num_paths == independent
    assert value.num_simulated_paths == physical
    if value.std_dev is not None:
        assert _finite(value.std_dev) > 0.0
    if reference is not None:
        assert abs(mean.amount - reference) <= _MC_SANITY_STDERRS * stderr + _BS_ABS_TOL
    if lower_reference is not None:
        assert mean.amount + _MC_SANITY_STDERRS * stderr + _BS_ABS_TOL >= lower_reference
    return json.loads(value.to_json())
