"""Publication-quality tear sheets, tables, and charts (pure-Python presentation layer)."""

from .performance import performance_tearsheet
from .theme import INSTITUTIONAL, Theme

__all__ = ["INSTITUTIONAL", "Theme", "performance_tearsheet"]
