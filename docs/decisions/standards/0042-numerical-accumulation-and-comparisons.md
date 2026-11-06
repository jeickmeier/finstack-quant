# ADR 0042: Numerical accumulation and comparison rules

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision and scope

Floating-point accumulation uses numerical methods appropriate to the result
being calculated. Comparisons state the numerical or bit-level property they
assert and use justified tolerances where approximation is intended.

The precise operational clauses remain in [INVARIANTS.md](../../../INVARIANTS.md);
the [invariant decision map](../INVARIANT_MAP.md) records their correspondence.
Monetary construction, Decimal/Money arithmetic, and rounding boundaries remain
governed by [ADR 0020](../financial/conventions/0020-numerical-representation-and-rounding.md).
This decision applies to floating-point calculations; it does not replace
required Decimal arithmetic with compensated `f64` arithmetic.

| Catalogue clause | Decision |
| --- | --- |
| [§1.2: compensated aggregation](../../../INVARIANTS.md#12-arithmetic-and-aggregation) | For floating-point totals, use compensated summation for mixed-sign cash flows, large dynamic ranges, or totals whose floating-point error can affect a reported result. Use `neumaier_sum` or `NeumaierAccumulator` for these totals. |
| [§1.2: online statistics](../../../INVARIANTS.md#12-arithmetic-and-aggregation) | Use `OnlineStats` for online mean and variance calculations. Its Welford/Chan statistics are distinct from compensated summation and are not a substitute for a compensated total. |
| [§1.2: finite inputs](../../../INVARIANTS.md#12-arithmetic-and-aggregation) | Validate finiteness before Kahan/Neumaier accumulation; compensation does not make `NaN` or infinity safe. |
| [§2.4: accuracy and materiality](../../../INVARIANTS.md#24-reductions) | Compensated summation improves accuracy but does not independently establish bit reproducibility. Assess cancellation, dynamic range, and materiality instead of substituting an arbitrary term-count threshold. |
| [§2.5: approximate comparisons](../../../INVARIANTS.md#25-comparisons) | Document the tolerance's semantics: absolute, relative, notional-scaled, probability, rate, percentage, money half-unit, or another named domain tolerance. |
| [§2.5: exact numerical comparisons](../../../INVARIANTS.md#25-comparisons) | Use exact `f64` numerical equality only when that exact numerical relation is intended, including exact-zero sentinels and values known to come from the same deterministic operation. |
| [§2.5: bit-identity comparisons](../../../INVARIANTS.md#25-comparisons) | Compare `f64::to_bits()` representations when the promised property is bit identity. Ordinary `==` does not establish bit identity: it equates positive and negative zero and does not equate NaNs with themselves. |
| [§2.5: documented helpers](../../../INVARIANTS.md#25-comparisons) | Cite a shared comparison helper only when it actually exists in the public API; this rule does not require introducing a helper. |

Independently justified references and protected tolerances remain governed by
[ADR 0014](../project/0014-financial-reference-evidence.md). Acceptance of this
decision does not authorize changing a protected expected value or tolerance.

The accepted reproducibility guarantees remain in
[ADR 0021](0021-reproducibility-guarantees.md). The separate execution
requirements in [ADR 0043](0043-deterministic-ordering-and-monte-carlo-execution.md)
are also accepted: fixed partitions and merge order apply to the declared
bit-identity scope. Numerical accuracy and execution reproducibility require
their own evidence.

## Rationale and alternatives

Cancellation and dynamic range can make a short sum inaccurate; a long sum
does not alone establish material error. The criteria connect accumulation
to the numerical problem and the result's use. A single term-count cutoff
would not capture those conditions.

Reusing the shared Neumaier primitives reduces summation error without a new
abstraction. Online mean and variance are different calculations: their
Welford/Chan accumulator does not supply a compensated cash-flow total.

One global epsilon would erase the distinction between money, rates,
probabilities, and notional-scaled outputs. Exact comparison everywhere would
also conflict with the scoped cross-target guarantees in ADR 0021. Named,
justified tolerances preserve those distinctions. Exact numerical equality
and bit-representation equality remain separate assertions, each appropriate
only when it expresses the property promised to callers.

## Enforcement

- Exercise floating-point cancellation, mixed signs, and large dynamic ranges
  against independently justified results; confirm non-finite inputs are
  handled before compensated accumulation.
- Review accumulator choice against the requested quantity: compensated
  total versus online mean/variance. Preserve the separate Decimal monetary
  boundaries under ADR 0020.
- Review each approximate comparison's units, scale, domain, and tolerance
  rationale. Preserve protected expectations under ADR 0014.
- Check that exact comparisons name the property being asserted. Bit-identity
  checks compare representations; exact numerical checks use the intended
  numerical semantics, including any relevant signed-zero or NaN behavior.
- Verify that documented helper names resolve to the published API.
- Verify accuracy separately from execution-order and cross-target behavior;
  success in one does not establish the other.

## Implementation notes and references

The shared [summation implementation](../../../finstack-quant/core/src/math/summation.rs)
provides `neumaier_sum` and `NeumaierAccumulator`; the
[statistics implementation](../../../finstack-quant/core/src/math/stats.rs)
provides the Welford/Chan `OnlineStats` accumulator. These source locations
support the named capabilities, not complete caller compliance.

Existing accumulation, finiteness, comparison, and evidence gaps remain in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
Acceptance records the policy; it does not establish that every calculation
or check complies. No numerical tests were run for this documentation change.
