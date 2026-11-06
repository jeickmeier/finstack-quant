# ADR 0046: Portfolio execution scheduling, reuse, and adapter boundaries

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Specific code decisions

## Decision and scope

The operational requirements in
[INVARIANTS.md section 2.6](../../../INVARIANTS.md#26-portfolio-evaluation-execution)
form the detailed execution contract supporting accepted
[ADR 0022](0022-portfolio-evaluation-state.md). The scope is ordinary position
valuation within portfolio valuation, metrics-based attribution, scenario
P&L, scenario batches, replay, and factor-stress endpoint P&L.

The catalogue remains the canonical home of the operational clauses. This
decision establishes their scope and rationale; acceptance does not certify
implementation or validation coverage.
The [invariant map](../INVARIANT_MAP.md) records the correspondence.

| Covered clause | Accepted contract |
| --- | --- |
| Shared execution ownership | Public entry points and host bindings adapt into the request-scoped executor. They do not own another pricing loop, valuation assembler, calibration cache, or scheduling policy. |
| Immutable identities | Register immutable market states separately from immutable portfolio/instrument states. Changed markets, effective dates, or instrument sets receive distinct state identities. |
| Recalibration caches | Create fresh rate and hazard recalibration caches per market state. Cloned options may share them only within that immutable state; never across states or requests. |
| Evaluation profiles | Distinguish the exact requested metric menu, strict-risk policy, and base-currency policy. Each position receives its structurally applicable subset; report inapplicable metrics explicitly rather than treating them as successful zero results. Strict mode returns an error when an applicable requested calculation fails. An explicitly empty menu stays PV-only. A degraded best-effort result cannot satisfy a strict request. Workflows rejecting cross-currency positions check native currency before FX lookup. |
| Selective reuse | An explicitly supplied immutable prior result may seed a new request only when the relevant pricing configuration, parent and updated markets, evaluation profile, and portfolio identity/shape are compatible and an authoritative change set identifies all affected work. If compatibility cannot be established, perform fresh valuation. Separate instrument repricing from base-currency conversion refresh. |
| FX dependencies | Without an authoritative FX path manifest, a changed FX quote invalidates every position with any FX dependency, including possible triangulation. When conversion inputs change, refresh base-currency conversion for every reused position; unchanged conversion may be reused only when its inputs are proven unchanged. |
| Scheduling | Select at most one portfolio Rayon axis per execution wave: states, positions, or factors. Do not nest state-by-position or factor-by-position fan-out. |
| Result order | Collect indexed parallel results before a serial logical-order scan. Preserve position/state order, first-error selection, and Neumaier aggregation order under [ADR 0021](../standards/0021-reproducibility-guarantees.md). |
| Cache lifetime | Keep evaluation state and financial caches request-local; prohibit executor-retained global, thread-local, process-wide, persistent, and cross-request valuation caches. An explicit immutable prior input is governed by the selective-reuse checks above. Static immutable registries, immutable decoded-input caches, and UI presentation caches remain outside this financial-cache scope; they do not authorize caching valuation results. |
| Python adapter boundary | Extract Python objects and construct Python results while attached to Python. Typed borrow guards may remain outside native closures while adapters extract plain Rust references; native evaluation closures capture only Rust state. Detach for native evaluation, JSON parsing, portfolio materialization, and large result serialization after copying required Python string input. This boundary does not require cloning entire portfolios or markets. |
| Replacement and reducers | Delete a migrated workflow's superseded compute path. Do not retain a legacy/new switch, dormant fallback engine, or duplicate scheduler. Financially distinct reducers may remain, with ordinary portfolio valuation entering through the shared executor. |

## Rationale and alternatives

Exact profiles, complete input compatibility, and state identities prevent
reuse from changing the meaning of a request. Caller-supplied prior results
make the reuse input explicit; they do not permit the executor to retain a
hidden cache between requests. A complete change set must account for relevant
pricing configuration as well as market and portfolio changes.

The metric menu serves heterogeneous portfolios without silently dropping
unsupported requests or confusing inapplicability with calculation failure.
This decision does not adopt arbitrary metric defaults or qualify every
instrument's applicability declaration.

Conservative FX invalidation trades some repricing work for
correctness when a complete dependency path is unavailable. One parallel
axis and ordered collection keep scheduling and first-error behavior
reviewable without relying on worker completion order.

Nested portfolio parallelism, workflow-owned schedulers, and long-lived financial
caches could offer different throughput or latency tradeoffs, but introduce
additional scheduling, invalidation, and reproducibility contracts. They are
not exceptions under this decision. An alternative design would require its own
authorized change and evidence under ADRs 0021, 0022, and
[0040](../standards/0040-measured-performance-decisions.md).

## Verification criteria and evidence

The [executor](../../../finstack-quant/portfolio/src/evaluation/executor.rs),
[state model](../../../finstack-quant/portfolio/src/evaluation/state.rs),
[evaluation plan](../../../finstack-quant/portfolio/src/evaluation/plan.rs),
and [Python valuation adapter](../../../finstack-quant-py/src/bindings/portfolio/valuation.rs)
are evidence locations. Their existence is not proof of complete compliance.

Enforcement evidence must compare selective reuse against fresh evaluation
after each relevant configuration, market, date, portfolio, and profile change,
including an unverifiable parent or change set that must trigger fresh valuation.
It must cover FX
triangulation and reused native PV with refreshed base PV when conversion
inputs change, and unchanged conversion reuse only under established
compatibility, applicable/inapplicable metrics, exact PV-only and strict-risk
requests, error and aggregation order across promised thread counts, and each
workflow's executor, recalibration caches, and Python attachment boundary.

Acceptance does not establish that this evidence is complete. Implementation
and coverage work is tracked in the
[portfolio execution follow-up](../FOLLOW_UPS.md#portfolio-execution).
