"""Regression checks for core Python input conversion and value protocols."""

from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pytest

from finstack_quant.core.market_data import FxMatrix
from finstack_quant.core.math import linalg, stats
from finstack_quant.core.types import CreditRating


def matrix_layout(values: list[list[float]], layout: str) -> npt.NDArray[np.float64]:
    """Keep logical values identical while changing physical strides."""
    array = np.asarray(values, dtype=np.float64)
    if layout == "fortran":
        return np.asfortranarray(array)
    if layout == "transpose":
        return array.T.copy().T
    if layout == "strided":
        storage = np.zeros((array.shape[0] * 2, array.shape[1] * 2))
        storage[::2, ::2] = array
        return storage[::2, ::2]
    if layout == "reversed":
        return array[::-1, ::-1].copy()[::-1, ::-1]
    return array


@pytest.mark.parametrize("layout", ["c", "fortran", "transpose", "strided", "reversed"])
def test_triangular_operations_preserve_logical_matrix_order(layout: str) -> None:
    lower = matrix_layout([[1.0, 0.0], [2.0, 3.0]], layout)
    assert linalg.apply_lower_triangular(lower, [4.0, 5.0]) == pytest.approx([4.0, 23.0])
    assert linalg.cholesky_solve(lower, [1.0, 1.0]) == pytest.approx([11.0 / 9.0, -1.0 / 9.0])


@pytest.mark.parametrize("layout", ["c", "fortran", "transpose", "strided", "reversed"])
def test_shrinkage_preserves_observation_rows_and_variable_columns(layout: str) -> None:
    values = [[1.0, 10.0], [2.0, 20.0], [4.0, 30.0], [8.0, 40.0]]
    covariance, shrinkage = linalg.ledoit_wolf_shrinkage(matrix_layout(values, layout))
    expected_covariance, expected_shrinkage = linalg.ledoit_wolf_shrinkage(values)
    np.testing.assert_allclose(covariance, expected_covariance, rtol=0.0, atol=0.0)
    assert shrinkage == expected_shrinkage


@pytest.mark.parametrize("pair", ["ééé", "€USD", "US😀D", "USD/é", "USD/EUR/USD", "US1USD", "USD/12A"])
def test_fx_pair_parser_rejects_malformed_unicode_and_ascii(pair: str) -> None:
    with pytest.raises(ValueError, match="invalid FX pair"):
        FxMatrix.from_dict({pair: 1.0})


def test_fx_pair_parser_accepts_compact_and_slash_separated_codes() -> None:
    compact = FxMatrix.from_dict({"eurusd": 1.1})
    separated = FxMatrix.from_dict({"EUR/USD": 1.1})
    assert compact.rate("EUR", "USD", "2025-01-01").rate == separated.rate("EUR", "USD", "2025-01-01").rate


def test_rating_equality_and_hashing_use_typed_values() -> None:
    rating = CreditRating("AAA")
    alias = CreditRating("Aaa")
    assert rating == alias
    assert hash(rating) == hash(alias)
    assert {rating, alias} == {CreditRating.AAA}
    assert {rating: "highest quality"}[alias] == "highest quality"
    assert rating != "AAA"
    label = "AAA"
    assert label != rating
    assert rating != "Aaa"
    assert len({rating, "AAA", "Aaa"}) == 3
    assert "AAA" not in {rating: "highest quality"}
    with pytest.raises(TypeError):
        _ = rating < "AAA"


@pytest.mark.parametrize(
    "function",
    [
        "mean",
        "variance",
        "population_variance",
        "mean_var",
        "mean_or_nan",
        "sample_variance_or_nan",
        "sample_std_or_nan",
        "median_or_nan",
        "finite_min_or_nan",
        "finite_max_or_nan",
        "finite_count",
        "log_returns",
        "realized_variance",
    ],
)
@pytest.mark.parametrize("layout", ["contiguous", "strided", "reversed"])
def test_statistic_array_inputs_match_list_semantics(function: str, layout: str) -> None:
    source = np.array([1.0, 2.0, 4.0, 8.0], dtype=np.float64)
    if layout == "strided":
        storage = np.zeros(source.size * 2)
        storage[::2] = source
        source = storage[::2]
    elif layout == "reversed":
        source = source[::-1]
    statistic = getattr(stats, function)
    before = source.copy()
    assert statistic(source) == pytest.approx(statistic(source.tolist()), nan_ok=True)
    np.testing.assert_array_equal(source, before)


@pytest.mark.parametrize("function", ["quantile", "quantile_linear_or_nan"])
def test_quantile_array_inputs_preserve_caller_storage(function: str) -> None:
    source = np.array([8.0, 4.0, 2.0, 1.0], dtype=np.float64)[::-1]
    before = source.copy()
    statistic = getattr(stats, function)
    assert statistic(source, 0.3) == pytest.approx(statistic(source.tolist(), 0.3))
    np.testing.assert_array_equal(source, before)


@pytest.mark.parametrize("function", ["covariance", "correlation"])
def test_pairwise_statistic_array_inputs_preserve_strides(function: str) -> None:
    x = np.array([1.0, 9.0, 2.0, 9.0, 4.0, 9.0])[::2]
    y = np.array([8.0, 4.0, 2.0])[::-1]
    statistic = getattr(stats, function)
    assert statistic(x, y) == pytest.approx(statistic(x.tolist(), y.tolist()))


def test_ohlc_array_inputs_match_list_semantics() -> None:
    opening = np.array([100.0, 101.0, 103.0, 102.0])[::-1]
    high, low, close = opening + 2.0, opening - 2.0, opening + 1.0
    actual = stats.realized_variance_ohlc(opening, high, low, close)
    expected = stats.realized_variance_ohlc(opening.tolist(), high.tolist(), low.tolist(), close.tolist())
    assert actual == pytest.approx(expected)


@pytest.mark.parametrize("data", [np.ones((2, 2)), np.array(1.0), [[1.0]], ["invalid"]])
def test_statistic_rejects_non_vector_or_non_numeric_inputs(data: object) -> None:
    with pytest.raises(TypeError):
        stats.mean(data)


def test_statistics_keep_empty_and_non_finite_policies_in_rust() -> None:
    empty = np.array([], dtype=np.float64)
    assert stats.mean(empty) == 0.0
    assert stats.variance(empty) == 0.0
    assert stats.finite_count(np.array([1.0, np.nan, np.inf])) == 1
    assert np.isnan(stats.mean_or_nan(empty))
    with pytest.raises(ValueError, match="finite"):
        stats.realized_variance(np.array([1.0, np.nan]))
