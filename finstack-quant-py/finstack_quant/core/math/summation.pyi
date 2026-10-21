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

import numpy as np
import numpy.typing as npt

__all__ = ["kahan_sum", "neumaier_sum"]

def kahan_sum(values: Sequence[float] | npt.NDArray[np.float64]) -> float:
    """
    Kahan compensated summation -- reduces floating-point rounding errors.

    Best for sequences where all values have the same sign. For
    mixed-sign values, prefer :func:`neumaier_sum`.

    Parameters
    ----------
    values : list[float]
        Values to sum.

    Returns
    -------
    float
        Compensated sum.

    Notes
    -----
    This method does not raise; it returns the compensated sum of ``values``.

    Examples
    --------
    >>> from finstack_quant.core.math import summation
    >>> summation.kahan_sum([1.0, 2.0, 3.0])
    6.0
    """
    ...

def neumaier_sum(values: Sequence[float] | npt.NDArray[np.float64]) -> float:
    """
    Neumaier compensated summation -- handles mixed-sign values
    better than Kahan.

    Recommended for financial calculations with mixed-sign cashflows.

    Parameters
    ----------
    values : list[float]
        Values to sum.

    Returns
    -------
    float
        Compensated sum.

    Notes
    -----
    This method does not raise; it returns the compensated sum of ``values``.

    Examples
    --------
    >>> from finstack_quant.core.math import summation
    >>> summation.neumaier_sum([1.0, -2.0, 3.0])
    2.0
    """
    ...
