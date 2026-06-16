"""Tolerance comparator mirroring the Rust golden comparator."""

from __future__ import annotations

from dataclasses import dataclass

from .schema import ToleranceEntry

REL_DENOM_MIN = 1e-12


@dataclass
class ComparisonResult:
    """Result of comparing one actual metric against its reference value."""

    metric: str
    actual: float
    expected: float
    abs_diff: float
    rel_diff: float
    passed: bool
    used_tolerance: ToleranceEntry

    def failure_message(self, fixture_path: str) -> str:
        """Format a diagnostic message for a failed golden comparison."""
        return (
            f"Golden mismatch in {fixture_path}\n"
            f"  metric: {self.metric}\n"
            f"  actual: {self.actual:.12g}\n"
            f"  expected: {self.expected:.12g}\n"
            f"  abs_diff: {self.abs_diff:.6e}\n"
            f"  rel_diff: {self.rel_diff:.6e}\n"
            f"  tolerance: abs={self.used_tolerance.abs!r}, rel={self.used_tolerance.rel!r}"
        )


def compare(metric: str, actual: float, expected: float, tol: ToleranceEntry) -> ComparisonResult:
    """Compare one metric using abs-or-rel tolerance semantics."""
    abs_diff = abs(actual - expected)
    rel_diff = abs_diff / max(abs(expected), REL_DENOM_MIN)

    abs_pass = tol.abs is not None and abs_diff <= tol.abs
    rel_pass = tol.rel is not None and rel_diff <= tol.rel
    if tol.abs is None and tol.rel is None:
        msg = f"ToleranceEntry for metric '{metric}' has neither abs nor rel; malformed fixture"
        raise AssertionError(msg)

    return ComparisonResult(
        metric=metric,
        actual=actual,
        expected=expected,
        abs_diff=abs_diff,
        rel_diff=rel_diff,
        passed=_passed(abs_pass, rel_pass, tol),
        used_tolerance=tol,
    )


def _passed(abs_pass: bool, rel_pass: bool, tol: ToleranceEntry) -> bool:
    _ = tol
    return abs_pass or rel_pass
