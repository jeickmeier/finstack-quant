"""Tests for error handling: invalid inputs should raise clear exceptions."""

from datetime import date

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import FxConversionPolicy, FxMatrix
from finstack_quant.core.money import Money


class TestCurrencyErrors:
    """Invalid currency codes should raise."""

    def test_unknown_code(self) -> None:
        """Completely bogus code raises ValueError-like exception."""
        with pytest.raises(Exception, match=r"[Uu]nknown|[Ii]nvalid|Currency"):
            Currency("XYZ123")

    def test_empty_code(self) -> None:
        """Empty string is not a valid currency code."""
        with pytest.raises(ValueError, match=r"(?i)currency|invalid|empty"):
            Currency("")

    def test_numeric_code(self) -> None:
        """Pure numeric string is not a valid ISO alpha code."""
        with pytest.raises(ValueError, match=r"(?i)currency|invalid|unknown"):
            Currency("123")


class TestMoneyCurrencyMismatch:
    """Cross-currency arithmetic should raise."""

    def test_addition_mismatch(self) -> None:
        """Adding USD + EUR raises."""
        with pytest.raises(Exception, match=r"[Cc]urrency|[Mm]ismatch"):
            Money(100.0, Currency("USD")) + Money(50.0, Currency("EUR"))

    def test_subtraction_mismatch(self) -> None:
        """Subtracting USD - GBP raises."""
        with pytest.raises(Exception, match=r"[Cc]urrency|[Mm]ismatch"):
            Money(100.0, Currency("USD")) - Money(50.0, Currency("GBP"))


class TestFxErrors:
    """FxMatrix rejects invalid inputs."""

    def test_zero_rate(self) -> None:
        """Zero FX rate should raise."""
        fx = FxMatrix()
        with pytest.raises(Exception, match=r"(?i)(positive|rate|invalid input parameter)"):
            fx.set_quote(Currency("EUR"), Currency("USD"), 0.0)

    def test_negative_rate(self) -> None:
        """Negative FX rate should raise."""
        fx = FxMatrix()
        with pytest.raises(Exception, match=r"(?i)(positive|rate|invalid input parameter|negative)"):
            fx.set_quote(Currency("EUR"), Currency("USD"), -1.0)

    def test_missing_pair(self) -> None:
        """Looking up an unset pair should raise."""
        fx = FxMatrix()
        # Missing-pair lookups now raise KeyError (InputError::NotFound).
        # NOTE: expectation updated alongside the binding change; not yet
        # executed against a rebuilt wheel.
        with pytest.raises(KeyError, match=r"(?i)rate|pair|not found|missing"):
            fx.rate(
                Currency("EUR"),
                Currency("GBP"),
                date(2024, 1, 1),
                FxConversionPolicy.CASHFLOW_DATE,
            )


class TestDayCountErrors:
    """Invalid day-count inputs should raise."""

    def test_invalid_daycount_name(self) -> None:
        """Unrecognised day-count name should raise."""
        from finstack_quant.core.dates import DayCount

        with pytest.raises(ValueError, match=r"(?i)day.?count|invalid|unknown"):
            DayCount.from_name("invalid_convention")
