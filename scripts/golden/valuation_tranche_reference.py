"""Independent offline references for the frozen B4/P5 tranche diagnostics.

Run after producing and reviewing the Rust diagnostic export:
    uv run --no-project --with scipy==1.18.1 --with numpy==2.5.1 python \
        scripts/golden/valuation_tranche_reference.py --input export.json --output pins.json

The output contains regression pins; pins.report.json retains the complete
baseline acceptance report beside it. Both are written after all checks pass.

It uses no finstack pricing, probability or integration functions.
Equal-exposure estimates use a NumPy DFT;
directed-rounding Decimal polynomial products independently enclose their
conditional arithmetic error. Small pools use subsets; the concentrated pool
uses one Bernoulli times a binomial cohort. Genuine recovery-dispersion grids
retain their baseline bits without pretending to have an exact reference.

Gaussian integration uses SciPy QUADPACK and independently refined composite
GL32, with the existing clipped marginals/effective correlation. Its integration
uncertainty is a numerical estimate, NOT a rigorous quadrature certificate.
Every evaluated conditional expectation is checked against Decimal enclosures;
this deliberately expensive offline calculation is shared across four caps.
The first two GL32 grids alone require 7,680 conditional evaluations per dated
pool, each with quadratic Decimal polynomial work. Reference-generation time
is separate from valuation latency and is retained in the execution ledger.

Student-t references use the exported fixed nodes, actual conditional
probabilities and weights. They measure conditional arithmetic under the frozen
rule, not the rule's quadrature bias or the accuracy of its CDF/quantile inputs.
The strict candidate gates are Gaussian 2e-10 and Student-t 1e-12 pool fraction.
The 5e-13 conditional-node gate applies only to Student-t's fixed-rule
arithmetic; Gaussian probe errors remain diagnostics beneath its integrated
raw-cap, period and PV gates.
Old grid deviations are recorded, never used to reject the before baseline.
"""

from __future__ import annotations

import argparse
from decimal import ROUND_CEILING, ROUND_FLOOR, Decimal, localcontext
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
import time
from typing import Any
import warnings

import numpy as np
import scipy
from scipy.integrate import IntegrationWarning, quad
from scipy.special import ndtr, ndtri, roots_legendre
from scipy.stats import binom
from tranche_reference_storage import _report_path, regression_pins, write_json

PRECISION = 80
NODE_UNCERTAINTY_LIMIT = 1e-13
STUDENT_NODE_GATE = 5e-13
STUDENT_CAP_GATE = 1e-12
GAUSSIAN_CAP_GATE = 2e-10
GAUSSIAN_REFERENCE_LIMIT = 1e-13
NORMAL_TAIL_MASS = math.erfc(10.0 / math.sqrt(2.0))
LABELS = ("loss_attach", "recovery_attach", "loss_detach", "recovery_detach")
ZERO = Decimal(0)
ONE = Decimal(1)


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _progress(message: str) -> None:
    sys.stderr.write(f"tranche-reference: {message}\n")
    sys.stderr.flush()


def _json_hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    ).hexdigest()


def _bits(value: float) -> str:
    return struct.pack(">d", float(value)).hex()


def _decimal(value: float) -> Decimal:
    _require(math.isfinite(value), "non-finite exported input")
    return Decimal.from_float(float(value))


def _outward_float(value: Decimal) -> float:
    """Round a nonnegative Decimal upper bound outward to binary64."""
    _require(value >= ZERO, "negative error bound")
    result = float(value)
    if Decimal.from_float(result) < value:
        result = math.nextafter(result, math.inf)
    return result


def _polynomial_product(polynomials: list[list[Decimal]], rounding: str) -> list[Decimal]:
    """Balanced nonnegative products, with directed rounding at every operation."""
    with localcontext() as context:
        context.prec = PRECISION
        context.rounding = rounding
        level = polynomials
        while len(level) > 1:
            following = []
            for index in range(0, len(level), 2):
                if index + 1 == len(level):
                    following.append(level[index])
                    continue
                left, right = level[index], level[index + 1]
                product = [ZERO] * (len(left) + len(right) - 1)
                for i, a in enumerate(left):
                    for j, b in enumerate(right):
                        product[i + j] = context.add(product[i + j], context.multiply(a, b))
                following.append(product)
            level = following
        return level[0] if level else [ONE]


def _count_bounds(probabilities: list[float]) -> tuple[list[Decimal], list[Decimal]]:
    bounds = []
    for rounding in (ROUND_FLOOR, ROUND_CEILING):
        with localcontext() as context:
            context.prec = PRECISION
            context.rounding = rounding
            leaves = [[context.subtract(ONE, _decimal(p)), _decimal(p)] for p in probabilities]
            bounds.append(_polynomial_product(leaves, rounding))
    return bounds[0], bounds[1]


def _dft_pmf(probabilities: list[float]) -> np.ndarray:
    """Recover degree-N coefficients without aliasing or probability repairs."""
    count = len(probabilities)
    _require(count > 0, "empty count distribution")
    size = count + 1
    roots = np.exp(2j * np.pi * np.arange(size) / size)
    samples = np.ones(size, dtype=np.complex128)
    for p in probabilities:
        samples *= (1.0 - p) + p * roots
    coefficients = np.fft.fft(samples) / size
    _require(float(np.max(np.abs(coefficients.imag))) < 1e-13, "DFT imaginary residue")
    pmf = coefficients.real
    _require(float(np.min(pmf)) > -1e-13, "material negative DFT probability")
    _require(abs(math.fsum(map(float, pmf)) - 1.0) < 1e-12, "DFT mass discrepancy")
    positions = [k / count for k in range(size)]
    mean = math.fsum(float(mass) * position for mass, position in zip(pmf, positions, strict=True))
    expected_mean = math.fsum(probabilities) / count
    variance = math.fsum(float(mass) * (position - mean) ** 2 for mass, position in zip(pmf, positions, strict=True))
    expected_variance = math.fsum(p * (1.0 - p) for p in probabilities) / (count * count)
    _require(abs(mean - expected_mean) < 1e-12, "DFT normalized mean discrepancy")
    _require(abs(variance - expected_variance) < 1e-12, "DFT normalized variance discrepancy")
    # Small negative roundoff remains visible; never clip or renormalize it.
    return pmf


def _validate_dft_construction() -> None:
    """Check this independent oracle's transform convention, not production PMFs."""
    for count in (1, 10, 16, 17, 18, 50, 125):
        point_masses = (
            [0.0] * count,
            [1.0] * count,
            [1.0 if index % 3 == 0 else 0.0 for index in range(count)],
        )
        for probabilities in point_masses:
            actual = _dft_pmf(probabilities)
            expected = np.zeros(count + 1)
            expected[int(sum(probabilities))] = 1.0
            _require(float(np.max(np.abs(actual - expected))) < 1e-12, "DFT point-mass placement failed")
    # A separate binomial-product construction checks a non-degenerate pool.
    probabilities = [0.07] * 20 + [0.31] * 30
    actual = _dft_pmf(probabilities)
    expected = np.convolve(binom.pmf(np.arange(21), 20, 0.07), binom.pmf(np.arange(31), 30, 0.31))
    _require(float(np.max(np.abs(actual - expected))) < 1e-12, "DFT two-cohort cross-check failed")


def _subset_pmf(probabilities: list[float]) -> np.ndarray:
    pmf = np.array([1.0])
    for p in probabilities:
        pmf = np.concatenate((pmf * (1.0 - p), pmf * p))
    return pmf


class _ConditionalOracle:
    """Independent conditional expectations for one dated pool and its four caps."""

    def __init__(self, period: dict[str, Any]) -> None:
        self.pool = period["pool"]
        self.rows = period["raw_caps"]
        self.weights = list(map(float, self.pool["weights"]))
        self.recoveries = list(map(float, self.pool["recoveries"]))
        self.count = len(self.weights)
        _require(self.count > 0, "empty pool")
        _require(len(self.recoveries) == self.count, "recovery shape mismatch")
        _require(len(self.pool["pds"]) == self.count, "PD shape mismatch")
        _require(len(self.pool["ids"]) == self.count, "issuer shape mismatch")
        _require(all(w > 0.0 and math.isfinite(w) for w in self.weights), "export only positive names")
        _require(abs(math.fsum(self.weights) - 1.0) <= 1e-9, "invalid total contractual weight")
        _require(all(0.0 <= r <= 1.0 for r in self.recoveries), "invalid recovery")
        _require(tuple(row["label"] for row in self.rows) == LABELS, "cap order mismatch")
        _require(all(0.0 <= row["cap"] <= 1.0 for row in self.rows), "invalid effective cap")
        _require(
            tuple(row["exposure"] for row in self.rows) == ("loss", "recovery", "loss", "recovery"),
            "cap exposure order mismatch",
        )
        _require(
            all(row["correlation_effective"] == self.rows[0]["correlation_effective"] for row in self.rows),
            "draft requires equal effective correlations",
        )
        self.equal = all(w == self.weights[0] for w in self.weights) and all(
            r == self.recoveries[0] for r in self.recoveries
        )
        _require(self.equal == period["exact_equal_exposure"], "exact exposure predicate mismatch")
        if self.equal:
            self.kind = "count_dft"
            self.states = [[(0, k)] for k in range(self.count + 1)]
        elif self.count <= 17:
            self.kind = "explicit_subsets"
            self.states = [[(i, 1) for i in range(self.count) if mask & (1 << i)] for mask in range(1 << self.count)]
        elif (
            self.count == 50
            and all(w == self.weights[1] for w in self.weights[1:])
            and all(r == self.recoveries[1] for r in self.recoveries[1:])
            and all(p == self.pool["pds"][1] for p in self.pool["pds"][1:])
        ):
            self.kind = "bernoulli_times_binomial"
            self.states = [[(0, first), (1, k)] for first in (0, 1) for k in range(50)]
        else:
            self.kind = "unreferenced_grid"
            self.states = []
        self.amount_bounds = [
            [self._amount(state, row, rounding) for state in self.states]
            for row in self.rows
            for rounding in (ROUND_FLOOR, ROUND_CEILING)
        ]
        self.amounts = [[self._float_amount(state, row) for state in self.states] for row in self.rows]

    def _amount(self, state: list[tuple[int, int]], row: dict[str, Any], rounding: str) -> Decimal:
        with localcontext() as context:
            context.prec = PRECISION
            context.rounding = rounding
            total = ZERO
            for i, multiplicity in state:
                recovery = _decimal(self.recoveries[i])
                exposure = context.subtract(ONE, recovery) if row["exposure"] == "loss" else recovery
                amount = context.multiply(context.multiply(Decimal(multiplicity), _decimal(self.weights[i])), exposure)
                total = context.add(total, amount)
            return min(total, _decimal(row["cap"]))

    def _float_amount(self, state: list[tuple[int, int]], row: dict[str, Any]) -> float:
        return min(
            math.fsum(
                count
                * self.weights[i]
                * (1.0 - self.recoveries[i] if row["exposure"] == "loss" else self.recoveries[i])
                for i, count in state
            ),
            float(row["cap"]),
        )

    def _distribution(self, probabilities: list[float]) -> tuple[np.ndarray, list[Decimal], list[Decimal]]:
        _require(len(probabilities) == self.count, "conditional PD shape mismatch")
        _require(all(math.isfinite(p) and 0.0 <= p <= 1.0 for p in probabilities), "invalid conditional PD")
        if self.kind == "count_dft":
            lower, upper = _count_bounds(probabilities)
            return _dft_pmf(probabilities), lower, upper
        if self.kind == "bernoulli_times_binomial":
            _require(all(p == probabilities[1] for p in probabilities[1:]), "cohort conditional PD mismatch")
            cohort_lower, cohort_upper = _count_bounds(probabilities[1:])
            cohort = binom.pmf(np.arange(50), 49, probabilities[1])
            floating = np.concatenate(((1.0 - probabilities[0]) * cohort, probabilities[0] * cohort))
            bounds = []
            for rounding, cohort_bound in ((ROUND_FLOOR, cohort_lower), (ROUND_CEILING, cohort_upper)):
                with localcontext() as context:
                    context.prec = PRECISION
                    context.rounding = rounding
                    p = _decimal(probabilities[0])
                    factors = (context.subtract(ONE, p), p)
                    bounds.append([context.multiply(factor, mass) for factor in factors for mass in cohort_bound])
            return floating, bounds[0], bounds[1]
        _require(self.kind == "explicit_subsets", "exact conditional reference unavailable")
        bounds = []
        for rounding in (ROUND_FLOOR, ROUND_CEILING):
            with localcontext() as context:
                context.prec = PRECISION
                context.rounding = rounding
                pmf = []
                for mask in range(1 << self.count):
                    mass = ONE
                    for i, p in enumerate(probabilities):
                        probability = _decimal(p)
                        factor = probability if mask & (1 << i) else context.subtract(ONE, probability)
                        mass = context.multiply(mass, factor)
                    pmf.append(mass)
                bounds.append(pmf)
        return _subset_pmf(probabilities), bounds[0], bounds[1]

    def evaluate(self, probabilities: list[float], crosscheck: bool = False) -> tuple[list[float], list[float]]:
        floating, lower, upper = self._distribution(probabilities)
        estimates = [
            math.fsum(float(p) * a for p, a in zip(floating, amounts, strict=True)) for amounts in self.amounts
        ]
        uncertainty = []
        for index, estimate in enumerate(estimates):
            endpoints = []
            for rounding, pmf, amounts in (
                (ROUND_FLOOR, lower, self.amount_bounds[2 * index]),
                (ROUND_CEILING, upper, self.amount_bounds[2 * index + 1]),
            ):
                with localcontext() as context:
                    context.prec = PRECISION
                    context.rounding = rounding
                    total = ZERO
                    for p, amount in zip(pmf, amounts, strict=True):
                        total = context.add(total, context.multiply(p, amount))
                    endpoints.append(total)
            with localcontext() as context:
                context.prec = PRECISION
                context.rounding = ROUND_CEILING
                value = _decimal(estimate)
                error = max(context.subtract(value, endpoints[0]), context.subtract(endpoints[1], value), ZERO)
            bound = _outward_float(error)
            _require(bound <= NODE_UNCERTAINTY_LIMIT, f"conditional oracle uncertainty {bound} exceeds 1e-13")
            uncertainty.append(bound)
        if crosscheck and self.equal and self.count <= 18:
            subset = _subset_pmf(probabilities)
            counts = np.array([int(mask).bit_count() for mask in range(len(subset))])
            count_mass = np.bincount(counts, weights=subset, minlength=self.count + 1)
            for estimate, amounts in zip(estimates, self.amounts, strict=True):
                alternate = math.fsum(float(p) * amount for p, amount in zip(count_mass, amounts, strict=True))
                _require(abs(estimate - alternate) < 2e-13, "DFT/subset cross-check failed")
        if crosscheck and self.equal and all(p == probabilities[0] for p in probabilities):
            pmf = binom.pmf(np.arange(self.count + 1), self.count, probabilities[0])
            for estimate, amounts in zip(estimates, self.amounts, strict=True):
                alternate = math.fsum(float(p) * amount for p, amount in zip(pmf, amounts, strict=True))
                _require(abs(estimate - alternate) < 2e-13, "DFT/binomial cross-check failed")
        return estimates, uncertainty


def _gaussian_caps(oracle: _ConditionalOracle) -> list[dict[str, Any]]:
    correlations = [float(row["correlation_effective"]) for row in oracle.rows]
    _require(all(rho == correlations[0] for rho in correlations), "draft requires equal effective correlations")
    rho = correlations[0]
    _require(0.0 < rho < 1.0, "invalid effective Gaussian correlation")
    thresholds = ndtri(np.clip(np.asarray(oracle.pool["pds"], dtype=float), 1e-12, 1.0 - 1e-12))
    cache: dict[float, tuple[list[float], list[float]]] = {}

    def conditional(z: float) -> tuple[list[float], list[float]]:
        if z not in cache:
            probabilities = ndtr((thresholds - math.sqrt(rho) * z) / math.sqrt(1.0 - rho))
            cache[z] = oracle.evaluate(list(map(float, probabilities)))
        return cache[z]

    results = []
    nodes, weights = roots_legendre(32)
    for cap_index, row in enumerate(oracle.rows):
        _progress(f"Gaussian {row['label']}: QUADPACK")

        def integrand(z: float, index: int = cap_index) -> float:
            return conditional(float(z))[0][index] * math.exp(-0.5 * z * z) / math.sqrt(2.0 * math.pi)

        value, quad_error = quad(
            integrand, -10.0, 10.0, epsabs=2e-14, epsrel=0.0, points=list(range(-9, 10)), limit=500
        )
        initial_conditional_bound = max(entry[1][cap_index] for entry in cache.values())
        tail_bound = min(float(row["cap"]), 1.0) * NORMAL_TAIL_MASS
        _require(
            float(quad_error) + initial_conditional_bound + tail_bound < GAUSSIAN_REFERENCE_LIMIT,
            f"Gaussian QUADPACK reference budget exhausted for {row['label']}: "
            f"quadrature={quad_error:.3e}, conditional={initial_conditional_bound:.3e}, "
            f"tail={tail_bound:.3e}, limit={GAUSSIAN_REFERENCE_LIMIT:.3e}; "
            "GL32 refinement cannot repair the fixed QUADPACK error estimate",
        )
        previous = None
        accepted = None
        for refinement in range(7):
            width = 0.25 / (2**refinement)
            panels = round(20.0 / width)
            _progress(f"Gaussian {row['label']}: GL32 width={width:g}, panels={panels}")
            terms = []
            for panel in range(panels):
                left = -10.0 + panel * width
                for node, weight in zip(nodes, weights, strict=True):
                    z = left + 0.5 * width * (float(node) + 1.0)
                    terms.append(0.5 * width * float(weight) * integrand(z))
            check = math.fsum(terms)
            conditional_bound = max(entry[1][cap_index] for entry in cache.values())
            discrepancy = abs(value - check)
            refinement_delta = math.inf if previous is None else abs(check - previous)
            # Estimates, not a rigorous guarantee of the continuous integral.
            estimated_uncertainty = (
                max(float(quad_error), discrepancy, refinement_delta) + conditional_bound + tail_bound
            )
            if previous is not None and estimated_uncertainty <= GAUSSIAN_REFERENCE_LIMIT:
                accepted = {
                    "expected": value,
                    "uncertainty": estimated_uncertainty,
                    "uncertainty_kind": "quadrature_estimate_plus_bounded_conditional_arithmetic",
                    "quadpack_error_estimate": float(quad_error),
                    "composite_gl32": check,
                    "gl32_panel_width": width,
                    "gl32_refinement_delta": refinement_delta,
                    "conditional_arithmetic_bound": conditional_bound,
                    "omitted_normal_tail_bound": tail_bound,
                }
                break
            previous = check
        _require(accepted is not None, f"Gaussian oracle not converged for {row['label']}")
        results.append(accepted)
    return results


def _student_caps(conditional: list[dict[str, Any]], nodes: list[dict[str, Any]]) -> list[dict[str, Any]]:
    weights = [float(node["weight"]) for node in nodes]
    _require(all(math.isfinite(weight) and weight >= 0.0 for weight in weights), "invalid Student-t weight")
    _require(abs(math.fsum(weights) - 1.0) < 1e-8, "Student-t factor mass changed")
    result = []
    for index in range(4):
        value = math.fsum(
            weight * pin["expected_caps"][index] for weight, pin in zip(weights, conditional, strict=True)
        )
        bounds = []
        for rounding in (ROUND_FLOOR, ROUND_CEILING):
            with localcontext() as context:
                context.prec = PRECISION
                context.rounding = rounding
                total = ZERO
                for weight, pin in zip(weights, conditional, strict=True):
                    estimate = _decimal(pin["expected_caps"][index])
                    error = _decimal(pin["uncertainties"][index])
                    endpoint = (
                        context.subtract(estimate, error) if rounding == ROUND_FLOOR else context.add(estimate, error)
                    )
                    total = context.add(total, context.multiply(_decimal(weight), endpoint))
                bounds.append(total)
        with localcontext() as context:
            context.prec = PRECISION
            context.rounding = ROUND_CEILING
            uncertainty = _outward_float(
                max(context.subtract(_decimal(value), bounds[0]), context.subtract(bounds[1], _decimal(value)), ZERO)
            )
        _require(uncertainty < STUDENT_CAP_GATE, "Student-t weighted oracle uncertainty exceeds 1e-12")
        result.append({
            "expected": value,
            "uncertainty": uncertainty,
            "uncertainty_kind": "bounded_arithmetic_under_frozen_product_weights",
            "factor_mass": math.fsum(weights),
        })
    return result


def _oracle_reference(period: dict[str, Any], student: bool, cache_dir: Path, script_hash: str) -> dict[str, Any]:
    """Resume only complete, hash-verified oracle work for identical inputs."""
    oracle = _ConditionalOracle(period)
    identity = {
        "script_sha256": script_hash,
        "scipy_version": scipy.__version__,
        "numpy_version": np.__version__,
        "student": student,
        "pool": period["pool"],
        "caps": [
            {key: row[key] for key in ("label", "cap", "exposure", "correlation_effective")}
            for row in period["raw_caps"]
        ],
        "nodes": [
            {key: node[key] for key in ("factors", "weight", "conditional_probabilities", "conditioning_hash")}
            for node in period["nodes"]
        ],
    }
    key = _json_hash(identity)
    path = cache_dir / f"{key}.json"
    if path.exists():
        checkpoint = json.loads(path.read_text())
        _require(checkpoint["identity"] == identity, f"oracle checkpoint identity mismatch: {path}")
        reference = checkpoint["reference"]
        _require(_json_hash(reference) == checkpoint["reference_sha256"], f"oracle checkpoint content mismatch: {path}")
        _progress(f"resuming completed oracle {key[:12]}")
        return {**reference, "cache_status": "resumed", "cache_key": key}
    reference = {
        "reference_method": oracle.kind,
        "exact_reference_available": oracle.kind != "unreferenced_grid",
        "raw_caps": [{} for _ in period["raw_caps"]],
        "conditional": [],
    }
    for node in period["nodes"]:
        pin = {"conditioning_hash": node["conditioning_hash"]}
        if reference["exact_reference_available"]:
            expected, uncertainty = oracle.evaluate(
                list(map(float, node["conditional_probabilities"])), crosscheck=True
            )
            pin.update(expected_caps=expected, uncertainties=uncertainty)
            if student:
                pin["candidate_node_tolerance"] = STUDENT_NODE_GATE
        reference["conditional"].append(pin)
    if reference["exact_reference_available"]:
        reference["raw_caps"] = (
            _student_caps(reference["conditional"], period["nodes"]) if student else _gaussian_caps(oracle)
        )
    write_json(path, {"identity": identity, "reference": reference, "reference_sha256": _json_hash(reference)})
    _progress(f"saved completed oracle {key[:12]}")
    return {**reference, "cache_status": "computed", "cache_key": key}


def _period_reference(period: dict[str, Any], student: bool, cache_dir: Path, script_hash: str) -> dict[str, Any]:
    reference = _oracle_reference(period, student, cache_dir, script_hash)
    equal_exposure = period["exact_equal_exposure"]
    strict = equal_exposure or period["branch"] == "heterogeneous_exact_subsets"
    unchanged = not equal_exposure or period["branch"] in ("homogeneous_index_binomial", "issuer_homogeneous_binomial")
    candidate_policy = (
        "same_build_bits_and_exact_reference"
        if strict and unchanged
        else "same_build_baseline_bits"
        if unchanged
        else "strict_exact_reference"
    )
    output = {
        "accrual_start": period["accrual_start"],
        "accrual_end": period["accrual_end"],
        "payment_date": period["payment_date"],
        "known": period["known"],
        "branch": period["branch"],
        "baseline_el_bits": _bits(period["el"]),
        "baseline_wd_bits": _bits(period["wd"]),
        "reference_method": reference["reference_method"],
        "oracle_cache_status": reference["cache_status"],
        "oracle_cache_key": reference["cache_key"],
        "exact_reference_available": reference["exact_reference_available"],
        "candidate_policy": candidate_policy,
        "candidate_raw_cap_tolerance": STUDENT_CAP_GATE if student else GAUSSIAN_CAP_GATE,
        "conditional_node_policy": "student_fixed_rule_arithmetic" if student else "gaussian_diagnostic_probes",
        "raw_caps": [
            {"baseline_bits": _bits(row["actual"]), "expected": None, "uncertainty": None} for row in period["raw_caps"]
        ],
        "conditional": [],
        "baseline_grid_deviations": [],
    }
    for node, oracle_pin in zip(period["nodes"], reference["conditional"], strict=True):
        _require(len(node["actual_caps"]) == 4, "conditional cap shape mismatch")
        pin = {
            "conditioning_hash": node["conditioning_hash"],
            "baseline_caps_bits": [_bits(value) for value in node["actual_caps"]],
        }
        pin.update(oracle_pin)
        if output["exact_reference_available"]:
            pin["baseline_errors_with_uncertainty"] = [
                abs(float(actual) - expected) + uncertainty
                for actual, expected, uncertainty in zip(
                    node["actual_caps"], pin["expected_caps"], pin["uncertainties"], strict=True
                )
            ]
        if output["exact_reference_available"] and "grid" in period["branch"]:
            pin["baseline_grid_deviations"] = [
                float(actual) - expected
                for actual, expected in zip(node["actual_caps"], pin["expected_caps"], strict=True)
            ]
        output["conditional"].append(pin)
    if output["exact_reference_available"]:
        for pin, cap_reference in zip(output["raw_caps"], reference["raw_caps"], strict=True):
            pin.update(cap_reference)
        if "grid" in period["branch"]:
            output["baseline_grid_deviations"] = [
                float(row["actual"]) - pin["expected"]
                for row, pin in zip(period["raw_caps"], output["raw_caps"], strict=True)
            ]
    return output


def _scenario_reference(scenario: dict[str, Any], student: bool, cache_dir: Path, script_hash: str) -> dict[str, Any]:
    keys = (
        "name",
        "actual_pv",
        "period_error_bound",
        "pv_error_bound",
        "pv_roundoff_allowance",
        "prior_el",
        "prior_wd",
        "notional",
        "coupon_decimal",
        "pool_factor",
        "original_tranche_width",
        "premium_sign",
        "protection_sign",
    )
    output = {key: scenario[key] for key in keys}
    output["baseline_pv_bits"] = _bits(scenario["actual_pv"])
    for key in ("student_accumulation_allowance", "student_accumulation_operation_budget"):
        if key in scenario:
            output[key] = scenario[key]
    output["periods"] = []
    for index, period in enumerate(scenario["periods"]):
        start = time.perf_counter()
        _progress(
            f"{scenario['name']} period {index + 1}/{len(scenario['periods'])}, accrual_end={period['accrual_end']}"
        )
        output["periods"].append(_period_reference(period, student, cache_dir, script_hash))
        _progress(f"completed period in {time.perf_counter() - start:.3f}s")
    output["exact_reference_available"] = all(period["exact_reference_available"] for period in output["periods"])
    output["expected_pv"] = None
    if not output["exact_reference_available"]:
        return output
    factor = float(scenario["pool_factor"]) / float(scenario["original_tranche_width"])
    previous_el = float(scenario["prior_el"])
    previous_wd = float(scenario["prior_wd"])
    notional = float(scenario["notional"])
    coupon = float(scenario["coupon_decimal"])
    contributions = []
    max_component_uncertainty = 0.0
    pv_reference_bound = 0.0
    for source, pin in zip(scenario["periods"], output["periods"], strict=True):
        if source["known"]:
            el, wd = float(scenario["prior_el"]), float(scenario["prior_wd"])
        else:
            caps = [row["expected"] for row in pin["raw_caps"]]
            el = min(1.0, max(0.0, max(0.0, caps[2] - caps[0]) * factor + scenario["prior_el"]))
            wd = min(1.0 - el, max(0.0, max(0.0, caps[1] - caps[3]) * factor + scenario["prior_wd"]))
            el = max(previous_el, el)
            wd = min(1.0 - el, max(previous_wd, wd))
            max_component_uncertainty = max(
                max_component_uncertainty, 2.0 * factor * max(row["uncertainty"] for row in pin["raw_caps"])
            )
        delta_el = max(0.0, el - previous_el)
        delta_wd = max(0.0, wd - previous_wd)
        outstanding = notional * max(0.0, 1.0 - previous_el - previous_wd)
        aod = (1.0 - source["default_fraction"]) * source["remaining_accrual"] * notional * (delta_el + delta_wd)
        premium = coupon * max(0.0, source["accrual"] * outstanding - aod)
        contributions.extend((
            scenario["premium_sign"] * premium * source["discount_payment"],
            scenario["protection_sign"] * notional * delta_el * source["discount_default"],
        ))
        coefficient = notional * (
            2.0 * source["discount_default"]
            + abs(coupon)
            * source["discount_payment"]
            * (2.0 * source["accrual"] + 4.0 * (1.0 - source["default_fraction"]) * source["remaining_accrual"])
        )
        pv_reference_bound += coefficient * max_component_uncertainty
        pin.update(expected_el=el, expected_wd=wd, component_reference_uncertainty=max_component_uncertainty)
        previous_el, previous_wd = el, wd
    output["expected_pv"] = math.fsum(contributions)
    # The exported acceptance bound already includes a ledger covering BOTH
    # production and reference reconstruction roundoff. Count that allowance
    # once; this field is only the propagated raw-reference uncertainty.
    output["reference_pv_uncertainty"] = pv_reference_bound
    output["baseline_pv_deviation"] = float(scenario["actual_pv"]) - output["expected_pv"]
    return output


def _check_exact_baseline(case: dict[str, Any], reference: dict[str, Any]) -> None:
    """Gate existing exact models; old grids remain explicitly report-only."""
    student = case["copula"]["type"] == "student_t"
    exact_branches = {
        "homogeneous_index_binomial",
        "issuer_homogeneous_binomial",
        "heterogeneous_exact_subsets",
    }
    exact_period_count = 0
    grid_period_count = 0
    exact_scenarios: dict[str, bool] = {}
    for scenario, scenario_pin in zip(case["scenarios"], reference["scenarios"], strict=True):
        all_exact = True
        for period, pin in zip(scenario["periods"], scenario_pin["periods"], strict=True):
            context = f"B0 {case['id']} / {scenario['name']} / {period['accrual_end']}"
            exact = period["branch"] in exact_branches
            all_exact &= exact
            if not exact:
                _require(period["branch"] == "heterogeneous_fractional_grid", f"{context}: unknown baseline branch")
                pin["baseline_acceptance"] = {
                    "policy": "grid_report_only",
                    "passed": None,
                    "reason": "old conditional grid approximation is not an exact-count accuracy baseline",
                }
                grid_period_count += 1
                continue
            _require(pin["exact_reference_available"], f"{context}: exact baseline lacks a reference")
            raw_errors = []
            for actual, cap in zip(period["raw_caps"], pin["raw_caps"], strict=True):
                error = abs(float(actual["actual"]) - cap["expected"]) + cap["uncertainty"]
                _require(
                    error < pin["candidate_raw_cap_tolerance"],
                    f"{context}: exact baseline {actual['label']} error {error:.3e} exceeds {pin['candidate_raw_cap_tolerance']:.3e}",
                )
                raw_errors.append(error)
            node_errors = []
            for node, node_pin in zip(period["nodes"], pin["conditional"], strict=True):
                _require(
                    node["conditioning_hash"] == node_pin["conditioning_hash"],
                    f"{context}: conditional input hash mismatch",
                )
                for actual, expected, uncertainty in zip(
                    node["actual_caps"], node_pin["expected_caps"], node_pin["uncertainties"], strict=True
                ):
                    _require(
                        uncertainty <= NODE_UNCERTAINTY_LIMIT,
                        f"{context}: conditional oracle uncertainty exceeds its sealed limit",
                    )
                    error = abs(float(actual) - expected) + uncertainty
                    if student:
                        _require(
                            node_pin["candidate_node_tolerance"] == STUDENT_NODE_GATE,
                            f"{context}: Student-t conditional gate changed",
                        )
                        _require(
                            error < node_pin["candidate_node_tolerance"],
                            f"{context}: exact baseline conditional error {error:.3e} exceeds {node_pin['candidate_node_tolerance']:.3e}",
                        )
                    else:
                        _require(
                            "candidate_node_tolerance" not in node_pin,
                            f"{context}: Gaussian probe incorrectly carries a Student-t conditional gate",
                        )
                    node_errors.append(error)
            period_error = (
                max(
                    abs(float(period["el"]) - pin["expected_el"]),
                    abs(float(period["wd"]) - pin["expected_wd"]),
                )
                + pin["component_reference_uncertainty"]
            )
            _require(
                period_error <= scenario_pin["period_error_bound"],
                f"{context}: exact baseline EL/WD error {period_error:.3e} exceeds {scenario_pin['period_error_bound']:.3e}",
            )
            pin["baseline_acceptance"] = {
                "policy": "exact_reference_checked",
                "passed": True,
                "maximum_raw_cap_error_with_uncertainty": max(raw_errors),
                "maximum_node_error_with_uncertainty": max(node_errors, default=0.0),
                "component_error_with_uncertainty": period_error,
            }
            exact_period_count += 1
        exact_scenarios[scenario["name"]] = all_exact
        if all_exact:
            error = (
                abs(float(scenario["actual_pv"]) - scenario_pin["expected_pv"])
                + scenario_pin["reference_pv_uncertainty"]
            )
            _require(
                error <= scenario_pin["pv_error_bound"],
                f"B0 {case['id']} / {scenario['name']}: exact baseline PV error {error:.3e} exceeds {scenario_pin['pv_error_bound']:.3e}",
            )
            scenario_pin["baseline_acceptance"] = {
                "policy": "exact_reference_checked",
                "passed": True,
                "pv_error_with_uncertainty": error,
                "pv_error_bound": scenario_pin["pv_error_bound"],
            }
        else:
            scenario_pin["baseline_acceptance"] = {"policy": "grid_report_only", "passed": None}
    report: dict[str, Any] = {
        "exact_periods_checked": exact_period_count,
        "grid_periods_report_only": grid_period_count,
        "exact_models_passed": True if exact_period_count else None,
    }
    delta = _check_baseline_delta(case, reference, exact_scenarios)
    if delta is not None:
        report["correlation_delta"] = delta
    reference["baseline_acceptance"] = report


def _check_baseline_delta(
    case: dict[str, Any], reference: dict[str, Any], exact_scenarios: dict[str, bool]
) -> dict[str, Any] | None:
    if not ({"correlation_up", "correlation_down"} & exact_scenarios.keys()):
        return None
    _require(
        {"correlation_up", "correlation_down"} <= exact_scenarios.keys(),
        "correlation baseline requires both bumped scenarios",
    )
    if not (exact_scenarios["correlation_up"] and exact_scenarios["correlation_down"]):
        return {"policy": "grid_report_only", "passed": None}
    sources = {scenario["name"]: scenario for scenario in case["scenarios"]}
    pins = {scenario["name"]: scenario for scenario in reference["scenarios"]}
    up, down = pins["correlation_up"], pins["correlation_down"]
    actual = (sources["correlation_up"]["actual_pv"] - sources["correlation_down"]["actual_pv"]) / 2.0
    expected = (up["expected_pv"] - down["expected_pv"]) / 2.0
    uncertainty = (up["reference_pv_uncertainty"] + down["reference_pv_uncertainty"]) / 2.0
    bound = (up["pv_error_bound"] + down["pv_error_bound"]) / 2.0
    error = abs(actual - expected) + uncertainty
    _require(
        error <= bound,
        f"B0 {case['id']}: exact baseline correlation delta error {error:.3e} exceeds {bound:.3e}",
    )
    return {
        "policy": "exact_reference_checked",
        "passed": True,
        "error_with_uncertainty": error,
        "error_bound": bound,
    }


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--cache-dir", type=Path, help="completed period-oracle checkpoints; defaults beside the input")
    parser.add_argument(
        "--probe-case", help="diagnostic case ID for one-period cost measurement; produces partial output"
    )
    parser.add_argument("--probe-scenario", choices=("base", "correlation_up", "correlation_down"))
    parser.add_argument("--probe-period", type=int, help="zero-based period index, required with --probe-case")
    args = parser.parse_args()
    _require(
        (args.probe_case is None) == (args.probe_period is None),
        "--probe-case and --probe-period must be supplied together",
    )
    _require(
        args.probe_scenario is None or args.probe_case is not None,
        "--probe-scenario requires --probe-case and --probe-period",
    )
    return args


def main() -> None:
    """Generate complete frozen pins or one explicitly partial cost probe."""
    args = _parse_args()
    _require(scipy.__version__ == "1.18.1", "run with the documented scipy==1.18.1 pin")
    warnings.simplefilter("error", IntegrationWarning)
    _validate_dft_construction()
    payload = json.loads(
        args.input.read_text(),
        parse_constant=lambda value: (_ for _ in ()).throw(ValueError(f"invalid JSON constant {value}")),
    )
    _require(payload["schema_version"] == 1, "unsupported export schema")
    _require(isinstance(payload["input_hash"], str) and bool(payload["input_hash"]), "missing input hash")
    _require(
        isinstance(payload.get("target"), str) and bool(payload["target"]),
        "missing Rust diagnostic target",
    )
    case_ids = [case["id"] for case in payload["cases"]]
    _require(len(case_ids) == len(set(case_ids)), "duplicate reference case IDs")
    script_hash = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    cache_dir = args.cache_dir or args.input.parent / "tranche-reference-cache"
    if args.probe_case is not None:
        probe_scenario = args.probe_scenario or "base"
        _require(
            args.output.name != "valuation_b4_tranche_reference.json",
            "one-period probes cannot overwrite the sealed pin fixture",
        )
        _require(args.probe_case in case_ids, f"unknown probe case {args.probe_case}")
        case = next(case for case in payload["cases"] if case["id"] == args.probe_case)
        scenarios = {scenario["name"]: scenario for scenario in case["scenarios"]}
        _require(probe_scenario in scenarios, f"probe scenario {probe_scenario} is unavailable")
        periods = scenarios[probe_scenario]["periods"]
        _require(0 <= args.probe_period < len(periods), "probe period index out of range")
        kind = case["copula"]["type"]
        _require(kind in ("gaussian", "student_t"), f"unsupported copula {kind}")
        _progress(f"one-period oracle probe: {case['id']} / {probe_scenario} / {args.probe_period}")
        started = time.perf_counter()
        period = _period_reference(periods[args.probe_period], kind == "student_t", cache_dir, script_hash)
        elapsed = time.perf_counter() - started
        write_json(
            args.output,
            {
                "schema_version": 1,
                "artifact_kind": "partial_period_oracle_probe_not_sealable_pins",
                "input_hash": payload["input_hash"],
                "script_sha256": script_hash,
                "case": case["id"],
                "scenario": probe_scenario,
                "period_index": args.probe_period,
                "elapsed_seconds": elapsed,
                "oracle_cache_status": period["oracle_cache_status"],
                "elapsed_scope": "fresh_oracle_calculation"
                if period["oracle_cache_status"] == "computed"
                else "checkpoint_read_and_reconstruction",
                "baseline_acceptance": "not_run_requires_complete_scenario",
                "period": period,
            },
        )
        _progress(f"completed probe in {elapsed:.3f}s; saved {args.output}")
        return
    report_path = _report_path(args.input, args.output)
    output = {
        "schema_version": 1,
        "input_hash": payload["input_hash"],
        "provenance": {
            "scipy_version": scipy.__version__,
            "numpy_version": np.__version__,
            "script_sha256": script_hash,
            "exact_target": payload["target"],
            "decimal_precision": PRECISION,
            "decimal_rounding": "outward interval bounds at every nonnegative operation",
            "gaussian_quadrature": "QUADPACK plus independently refined composite GL32; estimated uncertainty",
            "student_t_scope": "arithmetic under exported binary64 product-rule weights and conditional probabilities; no quadrature-bias claim",
            "conditional_node_gates": "5e-13 applies only to Student-t; Gaussian probes retain diagnostic errors and unchanged-kernel bit checks",
            "candidate_checks_are_not_before_grid_acceptance": True,
        },
        "cases": [],
    }
    for case in payload["cases"]:
        _progress(f"case {case['id']}")
        kind = case["copula"]["type"]
        _require(kind in ("gaussian", "student_t"), f"unsupported copula {kind}")
        scenario_names = [scenario["name"] for scenario in case["scenarios"]]
        _require(
            "base" in scenario_names and len(set(scenario_names)) == len(scenario_names),
            "missing base or duplicate scenarios",
        )
        _require(set(scenario_names) <= {"base", "correlation_up", "correlation_down"}, "unknown scenario")
        result = {key: case[key] for key in ("id", "aliases", "fixture_hash", "copula")}
        result["scenarios"] = [
            _scenario_reference(scenario, kind == "student_t", cache_dir, script_hash) for scenario in case["scenarios"]
        ]
        result["exact_reference_available"] = all(
            scenario["exact_reference_available"] for scenario in result["scenarios"]
        )
        _check_exact_baseline(case, result)
        output["cases"].append(result)
        case_identity = {"input_hash": payload["input_hash"], "script_sha256": script_hash, "case_id": case["id"]}
        write_json(
            cache_dir / "cases" / f"{_json_hash(case_identity)}.json", {"identity": case_identity, "case": result}
        )
        _progress(f"completed case {case['id']}; saved case report")
    # No partial pin file survives an oracle failure. Retain the full report
    # before publishing the smaller regression-only projection.
    write_json(report_path, output)
    write_json(args.output, regression_pins(output))
    _progress(f"regression pins saved to {args.output}; complete B0 acceptance report saved to {report_path}")


if __name__ == "__main__":
    main()
