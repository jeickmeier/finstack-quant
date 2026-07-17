"""Publication-quality tear sheets, tables, and charts (pure-Python presentation layer).

Examples:
--------
>>> import finstack_quant.reporting as reporting
>>> reporting.__name__
'finstack_quant.reporting'
"""

from .attribution import attribution_tearsheet
from .benchmark import benchmark_tearsheet
from .credit import credit_tearsheet
from .dcf import dcf_tearsheet
from .instrument import instrument_tearsheet, recommended_metrics
from .performance import performance_tearsheet
from .portfolio import portfolio_tearsheet
from .portfolio_risk import portfolio_risk_tearsheet
from .scenario import scenario_tearsheet
from .statement import statement_tearsheet
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "INSTITUTIONAL",
    "Theme",
    "attribution_tearsheet",
    "benchmark_tearsheet",
    "credit_tearsheet",
    "dcf_tearsheet",
    "instrument_tearsheet",
    "performance_tearsheet",
    "portfolio_risk_tearsheet",
    "portfolio_tearsheet",
    "recommended_metrics",
    "scenario_tearsheet",
    "statement_tearsheet",
]
