"""
Numerical helpers: linear algebra, statistics, special functions, summation.

Provides pure-function submodules for numerical computation backed by
``finstack-quant-core`` Rust implementations.

Examples
--------
>>> from finstack_quant.core.math import stats
>>> stats.mean([1.0, 2.0, 3.0])
2.0

"""

from __future__ import annotations

from finstack_quant.core.math import linalg as linalg
from finstack_quant.core.math import special_functions as special_functions
from finstack_quant.core.math import stats as stats
from finstack_quant.core.math import summation as summation

__all__ = ["linalg", "special_functions", "stats", "summation"]
