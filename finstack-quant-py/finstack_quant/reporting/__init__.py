"""Publication-quality tear sheets, tables, and charts (pure-Python presentation layer)."""

from .attribution import attribution_tearsheet
from .instrument import instrument_tearsheet, recommended_metrics
from .performance import performance_tearsheet
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "INSTITUTIONAL",
    "Theme",
    "attribution_tearsheet",
    "instrument_tearsheet",
    "performance_tearsheet",
    "recommended_metrics",
]
