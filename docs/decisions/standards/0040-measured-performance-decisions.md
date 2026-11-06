# ADR 0040: Performance decisions backed by measurement

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision

Performance-driven complexity must be justified by representative measurements
while preserving the library's correctness and public contracts.

Identify the bottleneck, relevant input sizes, and intended usage. Compare
before and after under comparable hardware, runtime, build settings, and cache
conditions. Repeat measurements and account for variability. Distinguish
small-kernel improvements from end-to-end gains.

Optimizations retain financial semantics, numerical accuracy, error behavior,
and declared reproducibility. Existing contract protection applies, including
[ADR 0014](../project/0014-financial-reference-evidence.md) for reference results
and tolerances and [ADR 0021](0021-reproducibility-guarantees.md) for
reproducibility guarantees.

Report material tradeoffs, including relevant effects on memory, allocations,
startup, or package size alongside speed. Retain the commands, revisions,
inputs, and environment needed to repeat material performance claims.

Define performance budgets explicitly for agreed workloads and conditions.
Regression thresholds apply within that scope; benchmark availability alone
does not create a universal CI gate. Keep performance timing assertions in
benchmarks under [ADR 0039](0039-behavior-and-invariant-tests.md).

## Rationale

An optimization can improve a small kernel while leaving a caller's workflow
unchanged or making another resource constraint worse. Comparable, repeated
measurements establish what improved and under which conditions. Explicit
workloads and budgets make regression checks meaningful without treating
machine noise or exploratory measurements as universal acceptance criteria.

## Examples

| Claim or change | Treatment |
| --- | --- |
| Compare the same portfolio workload before and after under matching release settings | Appropriate comparative evidence. |
| Compare a debug baseline with an optimized release build | Insufficient evidence for attributing the gain to the code change. |
| Report a faster interpolation kernel as an equal improvement in total portfolio latency | Requires end-to-end evidence for that broader claim. |
| Compare a warm cache with a cold baseline without identifying the difference | Not a comparable measurement of the claimed change. |
| Improve latency while materially increasing retained memory | Report both effects. |
| Reduce accuracy or change the promised random stream to pass a performance budget | Requires the relevant contract decision; speed does not authorize it. |
| Apply a threshold agreed for materialization to every benchmark | Outside the agreed workload scope. |

## Enforcement

- Review performance-driven complexity alongside its representative workload,
  baseline, changed revision, measurement conditions, and repeated results.
- Check that claims match what was measured, including the timing boundary,
  input sizes, cache state, and distinction between kernel and end-to-end work.
- Run relevant correctness and contract checks alongside performance
  measurements; protect financial expectations and reproducibility guarantees.
- Review material resource tradeoffs and retain enough evidence to repeat
  material claims.
- Apply regression gates only to their agreed workloads and conditions. Changes
  to protected budgets, baselines, or checks follow
  [ADR 0016](../project/0016-independent-contract-protection.md).
- Use canonical benchmark and verification tasks under
  [ADR 0034](0034-code-standards-authority.md).

## Implementation notes and references

- [Benchmark documentation](../../../benchmarks/README.md) distinguishes the
  recorded materialization acceptance process from exploratory benchmarks.
- The [materialization benchmark procedure](../../../benchmarks/MATERIALIZATION_BENCHMARKS.md)
  documents a specific workload, timing boundaries, provenance, and regression
  budgets. Those thresholds are not adopted as library-wide limits by this ADR.
- [Canonical tasks](../../../mise.toml) provide scoped and broader benchmark
  execution.
- [ADR 0037](0037-simple-modular-implementation-and-reuse.md) governs the
  complexity and ownership of the resulting implementation.

The evidence requirement applies to material performance claims; it does not
prescribe a single storage format for all exploratory measurements. Existing
claims and enforcement coverage remain subject to the review tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
