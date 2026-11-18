# COVENANTS-03: Numeric compliance, denominator validity and headroom

**Decision status:** Accepted 2026-11-17 for numeric bounds, ratio validity and explicit headroom measures.

**Implementation status:** Inclusive comparisons are implemented for finite scalar inputs. Explicit denominators for all built-in ratios, typed contractual comparisons, indeterminate outcomes and separate headroom measures remain pending.

**Validation status:** The 115-test Covenants Rust baseline and all nine focused headroom tests passed on 2026-11-17 against current behavior. The accepted changes still need implementation and qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Require an explicit contract-defined denominator for each built-in leverage and coverage ratio. The producer supplies the ratio and denominator for the same reporting period. A nonpositive denominator or negative gross leverage yields an indeterminate assessment. Negative net leverage with positive earnings is valid. Missing observations and supplied nonfinite values retain their distinct input outcomes.
2. Report an indeterminate result with its reason and omit numeric headroom and breach probability. Record a contractual breach from an indeterminate result only when the agreement explicitly elects that treatment. The contractual election leaves modeled probability unavailable.
3. Compare valid values at contractual precision. A maximum passes at `value <= limit` and a minimum passes at `value >= floor`. Apply only rounding specified by the agreement and retain the [accepted monetary representation](../conventions/0020-numerical-representation-and-rounding.md). Basket tests compare supplied utilization with the limit.
4. Report absolute and relative headroom separately. The signed absolute distance is `threshold - value` for a maximum and `value - threshold` for a minimum, in the metric's units. Relative headroom divides that distance by `abs(threshold)` and is undefined at zero. Report arithmetic overflow distinctly from indeterminate assessment and retain any representable headroom measure. At a maximum of 4.0× and an actual of 3.6×, the absolute cushion is 0.4× and the relative cushion is 10%.

## Observed computation

After activation and effective-term selection, read the selected raw metric and optional denominator. Missing inputs error; supplied NaN or infinity errors. For built-in gross maximum leverage (`MaxDebtToEbitda`, `MaxTotalLeverage`, `MaxSeniorLeverage`), a negative ratio is internally converted to NaN. Any supplied earnings denominator at or below zero also produces this internal sentinel. A net-cash negative net-debt/EBITDA value is allowed when its required denominator is positive. The engine never recomputes a ratio or verifies numerator/denominator consistency.

An indeterminate internal value is a breach with absent actual/projected value and absent headroom, plus a point-in-time explanatory detail. It is not confused with supplied nonfinite data, which fails the call. Gross leverage's denominator is optional; minimum coverage ratios have no denominator field permitted by `CovenantSpec::validate`, so their denominator sign is delegated upstream. Custom maxima and baskets do not inherit the gross-leverage negative-value restriction.

Finite comparisons are exact, without tolerance: maxima pass at `v <= t`, minima at `v >= t`. Thus equality passes; maximum breach uses `v > t` and minimum breach `v < t`. `Basket` is an upper-bound test of the supplied utilization amount, not a usage ledger, draw engine or capacity decrementer. No threshold rounding is applied before comparison.

For a maximum, headroom is `(t-v)/d`; for a minimum, `(v-t)/d`, where `d=1` if `abs(t) < f64::EPSILON`, otherwise `d=abs(t)`. This condition includes tiny nonzero thresholds, not only zero. Positive means cushion and negative means deficit even for negative thresholds. Near zero the result has the metric's absolute-distance scale instead of a relative fraction. Nonfinite input or nonfinite resulting arithmetic yields no public headroom. A very large finite subtraction can therefore produce a valid pass/fail with absent headroom.

## Rationale, alternatives and adoption boundary

The documented ratio convention prevents a negative gross leverage result from appearing comfortably below a positive maximum and distinguishes net cash from nonpositive earnings. Absolute-threshold normalization preserves sign for negative thresholds. These are observed policies, not a universal legal definition of financial-covenant compliance. Alternative policies include an explicit not-meaningful status, required denominators for every ratio, contractual precision before comparison, and separate absolute/relative headroom fields. Currency/LTM inputs and threshold rounding require agreement under [ADR 0007](../conventions/0007-units-and-sign-conventions.md) and [ADR 0020](../conventions/0020-numerical-representation-and-rounding.md); generic epsilon comparison is not silently introduced.

## Evidence

Owning [helpers](../../../../finstack-quant/covenants/src/engine/helpers.rs), [point-in-time evaluation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), [spec validation](../../../../finstack-quant/covenants/src/engine/types.rs), and [forecast reuse](../../../../finstack-quant/covenants/src/forward.rs) share this convention. Inspected [engine convention tests](../../../../finstack-quant/covenants/tests/engine_conventions.rs) cover negative gross/custom values and negative-threshold headroom; [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) cover net cash versus zero/negative earnings. Inline engine tests cover exact equality and neighboring values.

The Rust baseline covered exact bounds, negative gross and custom ratios, and net cash versus nonpositive earnings. The focused headroom tests exercised the existing shared helper. Independent boundary fixtures for the accepted denominator, outcome and headroom rules remain pending.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
