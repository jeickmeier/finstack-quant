"""Unit tests for golden tolerance comparison."""

from __future__ import annotations

import pytest

from .schema import ToleranceEntry
from .tolerance import compare


def abs_only(abs_tolerance: float) -> ToleranceEntry:
    return ToleranceEntry(abs=abs_tolerance)


def rel_only(rel_tolerance: float) -> ToleranceEntry:
    return ToleranceEntry(rel=rel_tolerance)


def both(abs_tolerance: float, rel_tolerance: float) -> ToleranceEntry:
    return ToleranceEntry(abs=abs_tolerance, rel=rel_tolerance)


def both_or(abs_tolerance: float, rel_tolerance: float) -> ToleranceEntry:
    return ToleranceEntry(
        abs=abs_tolerance,
        rel=rel_tolerance,
        tolerance_reason="abs-or-rel tolerance reflects vendor screen rounding",
    )


def test_abs_only_pass() -> None:
    result = compare("x", 1.005, 1.0, abs_only(0.01))
    assert result.passed


def test_abs_only_fail() -> None:
    result = compare("x", 1.5, 1.0, abs_only(0.01))
    assert not result.passed


def test_rel_only_pass() -> None:
    result = compare("x", 1.0001, 1.0, rel_only(1e-3))
    assert result.passed


def test_rel_handles_zero_expected() -> None:
    result = compare("x", 1e-15, 0.0, rel_only(1e-3))
    assert result.passed


def test_either_abs_or_rel_passes_by_default() -> None:
    result = compare("x", 1_000_000.5, 1_000_000.0, both(0.01, 1e-6))
    assert result.passed


def test_tolerance_reason_is_not_needed_for_or_semantics() -> None:
    result = compare("x", 1_000_000.5, 1_000_000.0, both_or(0.01, 1e-6))
    assert result.passed


def test_neither_passes() -> None:
    result = compare("x", 100.0, 1.0, both(0.01, 1e-6))
    assert not result.passed


def test_empty_tolerance_raises() -> None:
    with pytest.raises(AssertionError, match="neither abs nor rel"):
        compare("x", 1.0, 1.0, ToleranceEntry())
