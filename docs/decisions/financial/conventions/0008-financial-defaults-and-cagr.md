# ADR 0008: Financial defaults and dated-window CAGR

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Financial conventions

## Decision

Omitting an optional financial input has a documented meaning, implemented in
Rust and shared across interfaces. Changing a default requires explicit
agreement. Adding an alternative method does not change the existing default.
Defaults are settled individually; this decision does not approve every
default present in the codebase.

The default for CAGR over an explicit dated window is **Actual/365 Fixed**:

```text
CAGR = (1 + cumulative_return)^(365 / actual_calendar_days) - 1
```

The denominator counts the actual calendar days in the window; the year basis
stays 365 across leap years. Other conventions remain available when explicitly
selected. This decision does not redefine frequency-based annualization, the
treatment of invalid return domains, or the XIRR cash-flow calculation.

## Rationale

The meaning of an omitted argument affects results as directly as an explicit
input. A stable, named default supports reproducibility across interfaces and
spreadsheet reconciliation.

## Examples

| Situation | Required treatment |
|---|---|
| A 10% cumulative gain over 365 actual days | Default CAGR is `1.10^(365 / 365) - 1`, or 10%. |
| A 10% cumulative gain over 366 actual days | Default CAGR is `1.10^(365 / 366) - 1`. |
| A caller explicitly selects another supported convention | Use that convention. |
| A refactor changes the default annualization convention | Require explicit agreement. |
| Python supplies a different default before calling Rust | Violates the shared-default contract. |
| A default selects a method based on available market data | Require the selection rule itself to be agreed and documented. |

## Enforcement

- Compare omitted arguments with the explicitly selected default.
- Pin independent dated numerical examples, including a leap-year window.
- Verify identical default behavior through Rust, Python, and WASM.
- Update dependent calculations, tests, and documentation in the same change
  when an authorized default changes.

## Implementation notes and references

Acceptance establishes the default; it does not certify implementation compliance.
Implementation reconciliation is tracked in
[the CAGR follow-up](../../FOLLOW_UPS.md#cagr-default).

- [CAGR day-count type and calculation](../../../../finstack-quant/analytics/src/risk_metrics/return_based.rs).
- [Performance CAGR and default-dependent calculations](../../../../finstack-quant/analytics/src/performance/scalar.rs).
- [Existing CAGR default regression](../../../../finstack-quant/analytics/tests/correctness_regressions.rs).
- [Microsoft XIRR documentation](https://support.microsoft.com/en-us/excel/functions/xirr-function) specifies a fixed 365-day discounting year. This supports the chosen year basis; XIRR and CAGR remain distinct calculations.
