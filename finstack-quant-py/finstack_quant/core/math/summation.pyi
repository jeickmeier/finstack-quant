"""
Numerically stable summation: Kahan and Neumaier compensated sums.

Examples
--------
>>> from finstack_quant.core.math import summation
>>> summation.kahan_sum([1.0, 2.0, 3.0])
6.0

"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl

__all__ = ["kahan_sum", "neumaier_sum"]

def kahan_sum(values: Sequence[float] | npt.NDArray[Any] | pd.Series | pl.Series) -> float:
    """
    Kahan compensated summation -- reduces floating-point rounding errors.

    Best for sequences where all values have the same sign. For
    mixed-sign values, prefer :func:`neumaier_sum`.

    Parameters
    ----------
    values : Sequence[float] | numpy.ndarray | pandas.Series | polars.Series
        Values to sum.

    Returns
    -------
    float
        Compensated sum (``0.0`` for an empty sequence).

    Raises
    ------
    ValueError
        If any element of *values* is ``nan`` or infinite.
    RuntimeError
        If running accumulation overflows to a non-finite value.

    Examples
    --------
    >>> from finstack_quant.core.math import summation
    >>> summation.kahan_sum([1.0, 2.0, 3.0])
    6.0
    """
    ...

def neumaier_sum(values: Sequence[float] | npt.NDArray[Any] | pd.Series | pl.Series) -> float:
    """
    Neumaier compensated summation -- handles mixed-sign values
    better than Kahan.

    Recommended for financial calculations with mixed-sign cashflows.

    Parameters
    ----------
    values : Sequence[float] | numpy.ndarray | pandas.Series | polars.Series
        Values to sum.

    Returns
    -------
    float
        Compensated sum (``0.0`` for an empty sequence).

    Raises
    ------
    ValueError
        If any element of *values* is ``nan`` or infinite.
    RuntimeError
        If running accumulation overflows to a non-finite value.

    Examples
    --------
    >>> from finstack_quant.core.math import summation
    >>> summation.neumaier_sum([1.0, -2.0, 3.0])
    2.0
    """
    ...
