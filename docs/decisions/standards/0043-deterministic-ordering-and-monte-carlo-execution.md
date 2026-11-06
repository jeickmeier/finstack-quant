# ADR 0043: Deterministic ordering and Monte Carlo execution

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision and scope

Ordering, explicit seeds, and controlled reduction execution support the
reproducibility guarantees in
[ADR 0021](0021-reproducibility-guarantees.md). That decision defines the
promised tiers and cross-host agreement; this decision defines their concrete
execution requirements without extending target coverage.

The precise clauses remain in [INVARIANTS.md](../../../INVARIANTS.md), with
correspondence recorded in the [invariant decision map](../INVARIANT_MAP.md).
Acceptance does not approve every current engine or certify implementation
and validation coverage.

| Catalogue clause | Decision |
| --- | --- |
| [§2.1: target-specific baselines](../../../INVARIANTS.md#21-reproducibility-tiers) | Recapture a baseline on its originating build target and record the relevant build, runtime, and execution conditions needed to interpret and reproduce it. A target name alone does not establish equivalent conditions. Cross-target numerical comparisons use the justified tolerances required by ADR 0021 unless bit identity is demonstrated. |
| [§2.1: structural and error ordering](../../../INVARIANTS.md#21-reproducibility-tiers) | Preserve result structure, keys, ordering, error kinds, and the identity of the failing input across targets. Parallel maps return the lowest-index failing item's error for every supported target and thread count. |
| [§2.1: hash iteration](../../../INVARIANTS.md#21-reproducibility-tiers) | Hash-map iteration order must not determine results, floating-point reductions, or error selection. |
| [§2.2: seeds](../../../INVARIANTS.md#22-random-number-generation) | Library code uses explicit seeds, without `thread_rng()` or `rand::random()`. |
| [§2.2: primary Monte Carlo engine](../../../INVARIANTS.md#22-random-number-generation) | Its serial/Rayon bit-identity guarantee requires a splittable stream such as `PhiloxRng::split(path_id)` or `substream(path_id)`, deterministic work partitioning independent of thread count, a fixed merge order independent of Rayon thread count, and disabled adaptive confidence-interval stopping. The guarantee compares the same declared inputs and configuration. |
| [§2.3: visible map ordering](../../../INVARIANTS.md#23-map-ordering) | Use `IndexMap` for insertion order or `BTreeMap` for sorted order wherever map order is visible through serialization, files, public iterators, snapshots, or golden tests. `IndexMap` insertion must follow a deterministic logical order; inserting in hash-iteration or worker-completion order does not establish determinism. |
| [§2.3: internal maps](../../../INVARIANTS.md#23-map-ordering) | Hash maps may serve internal caches, intern tables, and lookups whose iteration order cannot escape. Neither the standard map's randomized iteration nor Fx hashing supplies a public ordering contract. |
| [§2.4: bit-reproducible reductions](../../../INVARIANTS.md#24-reductions) | Reductions promising bit identity across execution modes or thread counts use deterministic partitions and a fixed merge tree for the same declared inputs and configuration. `OnlineStats::merge`, Welford/Chan statistics, and compensated sums remain order-sensitive and belong inside that tree when supporting this guarantee. |

The primary engine's default partition is a pure function of path count.
An explicitly selected chunk size may be part of the declared configuration;
hold it fixed when comparing executions. Neither default nor explicit
partitioning may vary with thread count or worker scheduling. This does not
promise bit identity between different chunk-size configurations.

The Monte Carlo conditions, including disabled adaptive confidence-interval
stopping, are scoped to the primary engine's declared bit-identity tier. They
do not forbid adaptive Monte Carlo or another engine under a separately
documented, justified guarantee. Fixed partitions and merge trees are not
imposed merely because an API declares statistical reproducibility. Preserve
each API's actual promised tier under ADR 0021. Selecting Philox alone is
insufficient.
Accuracy and comparison semantics remain in
[accepted ADR 0042](0042-numerical-accumulation-and-comparisons.md).

## Rationale and alternatives

An explicit seed fixes only one source of variation. Work assignment, draw
assignment, reduction order, and stopping can still depend on scheduling.
The rules make the promised result independent of thread count
within its stated conditions. Fixing one thread count instead would narrow
the guarantee and require an explicit decision under ADR 0021.

Deterministic hashing is insufficient for a visible ordering contract.
Explicitly ordered maps express whether insertion or sorted order is intended.
An insertion-ordered container cannot correct a nondeterministic insertion
sequence; the producer must establish the logical order first.
Internal hash maps retain their existing role where ordering cannot affect
observable behavior. Choosing whichever error completes first would expose
scheduling to callers; lowest-index selection preserves a logical input order.

An originating target identifies only part of a baseline's environment. The
relevant compiler/build choices, numerical backends, runtime conditions, and
execution settings must be recorded where they affect the declared guarantee.
This preserves the distinction between repeatability within named conditions
and an unsupported promise across all builds on the same target.

## Source observations and limitations

The [Philox implementation](../../../finstack-quant/models/src/monte_carlo/rng/philox.rs)
provides substreams, and the
[primary engine](../../../finstack-quant/models/src/monte_carlo/engine/pricing.rs)
contains partition and merge logic. Its
[tests](../../../finstack-quant/models/src/monte_carlo/engine/tests.rs)
include same-target serial/parallel and thread-count cases and rejection of
quasi-random streams, including serial use in the generic engine. These
locations do not establish a cross-target bit-identity guarantee.

The catalogue's current Sobol implementation is non-splittable and does not
satisfy the stated parallel tier. This is a scope limitation, not a conclusion
that all Sobol or quasi-Monte Carlo implementations are unsupported.
[Halton multi-start calibration](../../../finstack-quant/calibration/src/solver/multi_start.rs)
is deterministic by construction; that observation does not establish the
same target and execution guarantees for every subsequent solver operation.

[Ordered parallel mapping](../../../finstack-quant/core/src/parallel.rs)
implements logical result and lowest-index error ordering. The
[core exports](../../../finstack-quant/core/src/lib.rs) expose `FxHashMap` and
`FxHashSet`; their availability does not establish safe ordering in callers.
No runtime tests were run for this documentation change.

## Enforcement and implementation coverage

- Compare repeated serial and parallel runs across the promised thread-count
  classes with fixed inputs, seeds, path counts, and declared execution
  settings, including an explicit chunk size where selected. Use the bitwise
  comparisons specified by ADR 0042 when checking bit identity.
- Test multiple failing inputs to verify lowest-index error selection, and
  inspect public ordering and reductions for escaping hash iteration or
  worker-completion order, including insertion into `IndexMap`.
- Trace draw assignment, partitioning, merge order, and stopping together;
  do not infer the guarantee from the generator name alone.
- Verify the originating target and the relevant recorded build, runtime,
  and execution conditions when regenerating baselines. Test cross-host
  structure and numerical agreement separately.

Complete caller coverage, RNG support, and supported-target evidence remain open in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
Acceptance records the policy; it does not mark those checks complete or
change a different engine's declared tier.
