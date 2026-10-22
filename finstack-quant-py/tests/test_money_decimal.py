"""Regression tests for Money's exact-decimal entry point.

``Money.from_decimal`` is the single exact-decimal constructor; the regular
``Money(amount, currency)`` constructor takes ``float``/``int`` only.

Covers the cases the second-pass audit flagged:
  * subclasses of ``decimal.Decimal`` must still be detected via Python's
    ``isinstance``, not by string-comparing the type name.
  * Infinity / NaN must be rejected, not silently corrupted.
  * Float / int inputs continue to work through the constructor.
"""

from __future__ import annotations

from decimal import Decimal

import pytest

from finstack_quant.core.money import Money


def test_decimal_preserves_19_digit_precision() -> None:
    # The whole point of the Decimal path is to avoid IEEE 754 rounding.
    raw = "1234567890.0123456789"
    m = Money.from_decimal(Decimal(raw), "USD")
    # format_with() at 10 dp should round-trip the original literal exactly.
    assert m.format_with(decimals=10, show_currency=False) == raw


def test_constructor_rejects_decimal_and_text_amounts() -> None:
    with pytest.raises(TypeError, match="from_decimal"):
        Money(Decimal("1.25"), "USD")  # type: ignore[arg-type]
    with pytest.raises(TypeError, match="from_decimal"):
        Money("1.25", "USD")  # type: ignore[arg-type]


def test_decimal_subclass_uses_decimal_path_not_float() -> None:
    """A user-defined Decimal subclass must NOT be silently routed through f64.

    Earlier, dispatch was done by ``type(obj).__name__ == 'Decimal'``, which
    fails for subclasses. The fix uses ``isinstance``.
    """

    class HighPrecisionDecimal(Decimal):
        pass

    raw = "1234567890.0123456789"
    m = Money.from_decimal(HighPrecisionDecimal(raw), "USD")
    # If the subclass had been routed through f64, the trailing digits would
    # be lost.
    assert m.format_with(decimals=10, show_currency=False) == raw


@pytest.mark.parametrize("raw", ["Infinity", "-Infinity", "NaN"])
def test_decimal_non_finite_rejected(raw: str) -> None:
    with pytest.raises(ValueError, match="exactly representable"):
        Money.from_decimal(Decimal(raw), "USD")


def test_float_input_still_works() -> None:
    m = Money(100.5, "USD")
    # IEEE 754: 100.5 is exact, so format_with() must return the exact literal.
    assert m.format_with(decimals=2, show_currency=False) == "100.50"


def test_int_input_still_works() -> None:
    m = Money(100, "USD")
    assert m.format_with(decimals=2, show_currency=False) == "100.00"


def test_invalid_string_raises_type_error() -> None:
    with pytest.raises(TypeError):
        Money("not_a_number", "USD")  # type: ignore[arg-type]


def test_decimal_ordering_and_repr_do_not_round_through_float() -> None:
    lower = Money.from_decimal(Decimal("10000000000000000.1"), "USD")
    higher = Money.from_decimal(Decimal("10000000000000000.2"), "USD")

    assert lower < higher
    assert repr(lower) == "Money(10000000000000000.1, 'USD')"


def test_convert_at_rate_preserves_decimal_amount_and_retags_currency() -> None:
    converted = Money.from_decimal(Decimal("10000000000000000.1"), "USD").convert_at_rate("EUR", 1.25)
    assert converted.amount_decimal == Decimal("12500000000000000.125")
    assert str(converted.currency) == "EUR"

    with pytest.raises(ValueError, match=r"rate|positive"):
        Money(1, "USD").convert_at_rate("EUR", 0.0)


def test_from_decimal_accepts_exact_amounts() -> None:
    wide = Money.from_decimal(Decimal("12345678901234567890.12345"), "USD")
    assert wide.amount_decimal == Decimal("12345678901234567890.12345")
    assert str(wide.currency) == "USD"
    scientific = Money.from_decimal(Decimal("1.2345e3"), "EUR")
    assert scientific.amount_decimal == Decimal("1234.5")
    for amount in [
        "0.1e29",
        "1.00e-27",
        "2000e-31",
        "-7.9228162514264337593543950335e28",
    ]:
        assert Money.from_decimal(Decimal(amount), "USD").amount_decimal == Decimal(amount)


@pytest.mark.parametrize(
    "amount",
    [
        "1.24500000000000000000000000001",
        "1.23450000000000000000000000001e3",
        "1.123456789012345678901234567891",
        "0.50000000000000000000000000001",
        "1e-29",
        "1e29",
        "1.23e-28",
    ],
)
def test_from_decimal_rejects_inexact_amounts(amount: str) -> None:
    with pytest.raises(ValueError, match="exactly representable"):
        Money.from_decimal(Decimal(amount), "USD")


def test_from_decimal_rejects_bad_currency_and_wrong_types() -> None:
    with pytest.raises(ValueError, match="Invalid currency code"):
        Money.from_decimal(Decimal("1.0"), "NOT-A-CCY")
    with pytest.raises(TypeError):
        Money.from_decimal(1.0, "USD")  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        Money.from_decimal("1.0", "USD")  # type: ignore[arg-type]


def test_format_with_rejects_precision_above_native_bound() -> None:
    with pytest.raises(ValueError, match="formatting precision"):
        Money(1.0, "USD").format_with(decimals=1_000_001)


def test_from_decimal_applies_scientific_exponent_exactly() -> None:
    assert Money.from_decimal(Decimal("2000e-31"), "USD").amount_decimal == Decimal("2e-28")


def test_comparison_currency_errors_and_python_protocol() -> None:
    usd = Money(1, "USD")
    eur = Money(1, "EUR")
    assert usd != eur
    assert usd.__eq__(eur) is False
    for compare in [lambda: usd < eur, lambda: usd <= eur, lambda: usd > eur, lambda: usd >= eur]:
        with pytest.raises(ValueError, match=r"currency|Currency"):
            compare()
    assert usd.__eq__(object()) is NotImplemented
    assert usd.__lt__(object()) is NotImplemented
    with pytest.raises(TypeError):
        _ = usd < object()
