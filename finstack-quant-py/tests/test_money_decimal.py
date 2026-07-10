"""Regression tests for Money's Decimal-aware constructor.

Covers the cases the second-pass audit flagged:
  * type-name-based dispatch is fragile: subclasses must still be detected
    via Python's ``isinstance``, not by string-comparing the type name.
  * Infinity / NaN must be rejected, not silently corrupted.
  * Float / int inputs continue to work (backwards compatibility).
  * Both the polymorphic constructor and the explicit ``from_decimal``
    classmethod must give identical results.
"""

from __future__ import annotations

from decimal import Decimal

import pytest

from finstack_quant.core.money import Money


def test_decimal_preserves_19_digit_precision() -> None:
    # The whole point of the Decimal path is to avoid IEEE 754 rounding.
    raw = "1234567890.0123456789"
    m = Money.from_decimal(Decimal(raw), "USD")
    # format() at 10 dp should round-trip the original literal exactly.
    assert m.format(decimals=10, show_currency=False) == raw


def test_decimal_via_polymorphic_constructor_matches_classmethod() -> None:
    raw = "987654321.123456789"
    m1 = Money(Decimal(raw), "USD")
    m2 = Money.from_decimal(Decimal(raw), "USD")
    assert m1.format(decimals=9, show_currency=False) == m2.format(decimals=9, show_currency=False)


def test_decimal_subclass_uses_decimal_path_not_float() -> None:
    """A user-defined Decimal subclass must NOT be silently routed through f64.

    Earlier, dispatch was done by ``type(obj).__name__ == 'Decimal'``, which
    fails for subclasses. The fix uses ``isinstance``.
    """

    class HighPrecisionDecimal(Decimal):
        pass

    raw = "1234567890.0123456789"
    m = Money(HighPrecisionDecimal(raw), "USD")
    # If the subclass had been routed through f64, the trailing digits would
    # be lost.
    assert m.format(decimals=10, show_currency=False) == raw


def test_decimal_infinity_rejected() -> None:
    with pytest.raises(ValueError, match='Invalid Decimal value "Infinity"'):
        Money(Decimal("Infinity"), "USD")


def test_decimal_negative_infinity_rejected() -> None:
    with pytest.raises(ValueError, match='Invalid Decimal value "-Infinity"'):
        Money(Decimal("-Infinity"), "USD")


def test_decimal_nan_rejected() -> None:
    with pytest.raises(ValueError, match='Invalid Decimal value "NaN"'):
        Money(Decimal("NaN"), "USD")


def test_float_input_still_works() -> None:
    m = Money(100.5, "USD")
    # IEEE 754: 100.5 is exact, so format() must return the exact literal.
    assert m.format(decimals=2, show_currency=False) == "100.50"


def test_int_input_still_works() -> None:
    m = Money(100, "USD")
    assert m.format(decimals=2, show_currency=False) == "100.00"


def test_invalid_string_raises_type_error() -> None:
    with pytest.raises((TypeError, ValueError)):
        Money("not_a_number", "USD")  # type: ignore[arg-type]


def test_decimal_ordering_and_repr_do_not_round_through_float() -> None:
    lower = Money(Decimal("10000000000000000.1"), "USD")
    higher = Money(Decimal("10000000000000000.2"), "USD")

    assert lower < higher
    assert repr(lower) == "Money(10000000000000000.1, 'USD')"
