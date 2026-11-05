# ADR 0022: Shared portfolio execution with isolated state

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Specific code decisions

## Decision

Ordinary portfolio pricing across valuation, attribution, scenarios, replay,
and factor stress uses the shared Rust executor. Evaluation inputs are
immutable, and valuation/recalibration caches stay within their request and
applicable market state. Reuse preserves the complete financial meaning of
the request.

Changed markets, effective valuation dates, or portfolio/instrument state
must not reuse results as if their inputs were unchanged. Metric sets,
strict-risk requirements, and currency policies also constrain reuse. A
best-effort result cannot satisfy a strict-risk request.

Introducing process-wide or cross-request valuation caches requires an
explicit architectural decision. This rule concerns financial evaluation
state and caches; static immutable registries and UI presentation caches are
separate concerns.

## Rationale

Duplicated pricing loops and caches can apply different financial policies or
return stale results while individual calculations remain correct. Shared
execution ownership and explicit state boundaries preserve consistent
evaluation across workflows and make selective repricing verifiable.

## Examples

| Change or reuse | Treatment |
| --- | --- |
| Reuse a result because its instrument ID matches despite changed curves | Prohibited. |
| Change the effective valuation date | Treat it as a distinct evaluation state. |
| Return a cached best-effort result for a strict-risk request | Prohibited. |
| Reuse an unaffected native-currency PV after an FX change | Requires justified dependency checks and refreshed base-currency conversion. |
| Add a separate position-pricing loop inside the scenario engine | Violates shared execution ownership. |
| Introduce a process-wide valuation cache | Requires an explicit architectural decision. |

## Enforcement

- Compare selective repricing with fresh evaluation after changing markets,
  dates, positions, metric sets, and evaluation policies.
- Verify that requests do not mutate their input snapshots or reuse another
  request's evaluation state.
- Exercise FX changes that affect conversion separately from native pricing,
  including dependencies reached through triangulation.
- Preserve the shared executor boundary across adapters and workflows;
  ordinary portfolio pricing must not acquire a second execution owner.
- Apply the scoped numerical and ordering guarantees in
  [ADR 0021](../standards/0021-reproducibility-guarantees.md).

## Implementation notes and references

[INVARIANTS.md](../../../INVARIANTS.md), section 2.6, specifies the shared
executor, immutable state identities, request-local caches, evaluation
profiles, and selective repricing constraints. Request-owned state is defined
in [state.rs](../../../finstack-quant/portfolio/src/evaluation/state.rs), and
evaluation keys and invalidation inputs are defined in
[plan.rs](../../../finstack-quant/portfolio/src/evaluation/plan.rs).

Those mechanisms are existing implementation evidence, not proof that every
workflow and invalidation case is covered. Audit callers and regression
coverage against this decision, and record any missing cases before claiming
complete enforcement. This documentation change does not alter the executor
or its caches.
