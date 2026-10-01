"""Regression coverage for core input contracts at the Python boundary."""

from datetime import date
from decimal import Decimal

import numpy as np
import pytest

from finstack_quant.core.market_data import MarketContext, ScalarTimeSeries, VolSurface
from finstack_quant.models.correlation import validate_correlation_matrix


def test_decimal_precision_cannot_bypass_exact_float_validation() -> None:
    value = Decimal("0.50000000000000000000000000001")
    with pytest.raises(ValueError, match="exactly representable"):
        MarketContext().insert_price("PX", value)
    with pytest.raises(ValueError, match="exactly representable"):
        ScalarTimeSeries("PX", [(date(2025, 1, 1), value)])


@pytest.mark.parametrize(
    "values",
    [
        [[0.1], [0.2, 0.3, 0.4]],
        [[0.1, 0.2, 0.3, 0.4]],
        np.array([[0.1, 0.2, 0.3, 0.4]]),
    ],
)
def test_surface_rejects_wrong_row_shape_even_when_element_count_matches(values: object) -> None:
    with pytest.raises(ValueError, match="vol_rows"):
        VolSurface("VOL", [1.0, 2.0], [90.0, 100.0], values)  # type: ignore[arg-type]


@pytest.mark.parametrize(
    "values",
    [
        [[0.1, 0.2], [0.3, 0.4]],
        np.array([[0.1, 0.2], [0.3, 0.4]]),
        [0.1, 0.2, 0.3, 0.4],
    ],
)
def test_surface_preserves_valid_shape_and_quote_conventions(values: object) -> None:
    surface = VolSurface(
        "VOL",
        [1.0, 2.0],
        [1.0, 2.0],
        values,
        secondary_axis="tenor",
        interpolation_mode="total_variance",
        quote_type="normal",
    )
    assert surface.vol(1.0, 2.0) == 0.2
    assert surface.secondary_axis == "tenor"
    assert surface.interpolation_mode == "total_variance"
    assert surface.quote_type == "normal"


def test_correlation_validator_rejects_zero_pivot_indefinite_residual() -> None:
    with pytest.raises(ValueError, match="positive"):
        validate_correlation_matrix([1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 1.0, -1.0, 1.0], 3)
