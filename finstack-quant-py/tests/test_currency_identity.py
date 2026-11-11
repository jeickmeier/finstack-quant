"""Currency identity is the alphabetic code (record CORE-CONVENTIONS-012 #12)."""

from __future__ import annotations

import operator

import pytest

from finstack_quant.core.currency import Currency


def test_ordering_follows_the_alphabetic_code_not_the_numeric_code() -> None:
    # AUD is numeric 036 and USD 840; CHF is 756 and JPY 392.
    assert Currency("AUD") < Currency("USD")
    assert Currency("CHF") < Currency("JPY")
    codes = ["USD", "JPY", "AUD", "EUR", "CHF"]
    assert [str(c) for c in sorted(Currency(code) for code in codes)] == sorted(codes)


def test_hash_and_equality_follow_the_alphabetic_code() -> None:
    assert hash(Currency("USD")) == hash(Currency("usd"))
    assert hash(Currency("USD")) != hash(Currency("EUR"))
    assert len({Currency("USD"), Currency("usd"), Currency("EUR")}) == 2


def test_currency_is_not_an_integer() -> None:
    with pytest.raises(TypeError):
        operator.index(Currency("USD"))  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        int(Currency("USD"))  # type: ignore[call-overload]


def test_numeric_lookup_requires_a_unique_supported_match() -> None:
    assert str(Currency.from_numeric(840)) == "USD"
    assert Currency("USD").numeric == 840
    with pytest.raises(ValueError, match="999"):
        Currency.from_numeric(999)
