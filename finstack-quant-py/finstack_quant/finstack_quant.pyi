"""
Type stubs for the compiled ``finstack_quant.finstack_quant`` extension module.

These stubs allow static type checkers to resolve the extension namespace in
environments where the PyO3 module has not been built yet, such as the CI lint
job.

Examples
--------
>>> import finstack_quant.finstack_quant as finstack_quant
>>> finstack_quant.__name__
'finstack_quant.finstack_quant'
"""

from __future__ import annotations

from typing import Any

analytics: Any
attribution: Any
cashflows: Any
core: Any
covenants: Any
factor_model: Any
features: Any
margin: Any
monte_carlo: Any
portfolio: Any
reporting: Any
scenarios: Any
statements: Any
statements_analytics: Any
valuations: Any

__all__: list[str]
