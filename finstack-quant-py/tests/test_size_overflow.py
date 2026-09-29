"""Sizes whose products overflow are validation errors, not panics.

Rust computes every flat-matrix and path-buffer size with checked arithmetic,
so an ``n`` whose square overflows ``usize`` raises ``ValueError`` here and a
``validation`` FinstackError in WASM
(``finstack-quant-wasm/tests/facade/wasm32_safety.test.mjs``). Before, the
product wrapped to a small number, the length check passed, and the kernel
indexed out of bounds.
"""

from __future__ import annotations

import math

import pytest

from finstack_quant.core.math import linalg, special_functions
from finstack_quant.models import correlation
from finstack_quant.models.credit import MertonModel

WRAPS = 2**32  # 2**32 * 2**32 == 2**64 wraps to 0 in a 64-bit usize


def test_correlation_checks_reject_a_wrapping_dimension() -> None:
    with pytest.raises(ValueError, match="Invalid matrix size"):
        correlation.validate_correlation_matrix([], WRAPS)
    with pytest.raises(ValueError, match="Invalid matrix size"):
        correlation.nearest_correlation([], WRAPS)


def test_cholesky_solve_names_both_lengths() -> None:
    assert linalg.cholesky_solve([[2.0, 0.0], [1.0, 1.0]], [2.0, 1.0]) == [0.5, 0.0]
    with pytest.raises(ValueError, match="4 entries but a right-hand side of length 3 needs 9"):
        linalg.cholesky_solve([[2.0, 0.0], [1.0, 1.0]], [2.0, 1.0, 0.0])


def test_merton_path_simulation_rejects_overflowing_sizes() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    with pytest.raises(ValueError, match="num_steps"):
        model.simulate_paths(1, 2**64 - 1, 1.0, 7)
    with pytest.raises(ValueError, match="overflow"):
        model.simulate_paths(2**63, 4, 1.0, 7)
    assert len(model.simulate_paths(2, 4, 1.0, 7).asset_values) == 2 * 5


def test_student_t_propagates_nan_instead_of_panicking() -> None:
    assert math.isnan(special_functions.student_t_cdf(math.nan, 5.0))
    copula = correlation.CopulaSpec.student_t(5.0).build()
    assert math.isnan(copula.tail_dependence(math.nan))
    assert copula.tail_dependence(0.5) > 0.0
