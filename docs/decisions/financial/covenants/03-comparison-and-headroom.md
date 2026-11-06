# COVENANTS-03: Numeric compliance, denominator validity and headroom

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Computation

After activation and effective-term selection, read the selected raw metric and optional denominator. Missing inputs error; supplied NaN or infinity errors. For built-in gross maximum leverage (`MaxDebtToEbitda`, `MaxTotalLeverage`, `MaxSeniorLeverage`), a negative ratio is internally converted to NaN. Any supplied earnings denominator at or below zero also produces this internal sentinel. A net-cash negative net-debt/EBITDA value is allowed when its required denominator is positive. The engine never recomputes a ratio or verifies numerator/denominator consistency.

An indeterminate internal value is a breach with absent actual/projected value and absent headroom, plus a point-in-time explanatory detail. It is not confused with supplied nonfinite data, which fails the call. Gross leverage's denominator is optional; minimum coverage ratios have no denominator field permitted by `CovenantSpec::validate`, so their denominator sign is delegated upstream. Custom maxima and baskets do not inherit the gross-leverage negative-value restriction.

Finite comparisons are exact, without tolerance: maxima pass at `v <= t`, minima at `v >= t`. Thus equality passes; maximum breach uses `v > t` and minimum breach `v < t`. `Basket` is an upper-bound test of the supplied utilization amount, not a usage ledger, draw engine or capacity decrementer. No threshold rounding is applied before comparison.

For a maximum, headroom is `(t-v)/d`; for a minimum, `(v-t)/d`, where `d=1` if `abs(t) < f64::EPSILON`, otherwise `d=abs(t)`. This condition includes tiny nonzero thresholds, not only zero. Positive means cushion and negative means deficit even for negative thresholds. Near zero the result has the metric's absolute-distance scale instead of a relative fraction. Nonfinite input or nonfinite resulting arithmetic yields no public headroom. A very large finite subtraction can therefore produce a valid pass/fail with absent headroom.

## Rationale, alternatives and adoption boundary

The documented ratio convention prevents a negative gross leverage result from appearing comfortably below a positive maximum and distinguishes net cash from nonpositive earnings. Absolute-threshold normalization preserves sign for negative thresholds. These are observed policies, not a universal legal definition of financial-covenant compliance. Alternative policies include an explicit not-meaningful status, required denominators for every ratio, contractual precision before comparison, and separate absolute/relative headroom fields. Currency/LTM inputs and threshold rounding require agreement under [ADR 0007](../conventions/0007-units-and-sign-conventions.md) and [ADR 0020](../conventions/0020-numerical-representation-and-rounding.md); generic epsilon comparison is not silently introduced.

## Evidence

Owning [helpers](../../../../finstack-quant/covenants/src/engine/helpers.rs), [point-in-time evaluation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), [spec validation](../../../../finstack-quant/covenants/src/engine/types.rs), and [forecast reuse](../../../../finstack-quant/covenants/src/forward.rs) share this convention. Inspected [engine convention tests](../../../../finstack-quant/covenants/tests/engine_conventions.rs) cover negative gross/custom values and negative-threshold headroom; [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) cover net cash versus zero/negative earnings. Inline engine tests cover exact equality and neighboring values.

No numerical cases were executed.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
