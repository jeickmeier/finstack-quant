"""Publication-quality tear sheets, tables, and charts (pure-Python presentation layer).

Examples:
--------
>>> from finstack_quant.reporting import INSTITUTIONAL
>>> INSTITUTIONAL.name
'institutional'
"""

from .attribution import attribution_tearsheet
from .benchmark import benchmark_tearsheet
from .credit import credit_tearsheet
from .dcf import dcf_tearsheet
from .document import KPI, Section, TearSheet
from .instrument import instrument_tearsheet
from .performance import performance_tearsheet
from .portfolio import portfolio_tearsheet
from .portfolio_risk import portfolio_risk_tearsheet
from .scenario import scenario_tearsheet
from .statement import statement_tearsheet
from .tables import data_table
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "INSTITUTIONAL",
    "KPI",
    "Section",
    "TearSheet",
    "Theme",
    "attribution_tearsheet",
    "benchmark_tearsheet",
    "credit_tearsheet",
    "data_table",
    "dcf_tearsheet",
    "instrument_tearsheet",
    "performance_tearsheet",
    "portfolio_risk_tearsheet",
    "portfolio_tearsheet",
    "scenario_tearsheet",
    "statement_tearsheet",
]
