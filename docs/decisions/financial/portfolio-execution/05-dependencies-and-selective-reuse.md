# PORTFOLIO-EXECUTION-005 — Dependency invalidation and selective valuation reuse

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

Selective repricing uses declared market dependencies and request-local portfolio identity, with conservative handling of unknown dependencies and FX triangulation. It is an optimization required to preserve the full-pricing result under [accepted ADR 0022](../../architecture/0022-portfolio-evaluation-state.md), not permission to reuse a result solely because its position IDs match.

## Dependency algorithm

The index flattens rate/credit curve roles, credit-index IDs, scalar IDs, unique volatility IDs, FX pairs and fixing-series IDs into typed keys and maps them to ordered position indices. An instrument whose dependency extraction fails is stored as unresolved and included in every `affected_positions` query, even when the supplied change list is empty.

Credit-index dependencies are resolved on the request market: collect the index hazard curve, optional issuer hazard curves, and base-correlation curve. Direct index changes or changes to any constituent key invalidate positions using the index. Resolution errors propagate; the query iterates indexed credit indices, so an unresolved index can fail a query even when a different factor changed. Any changed FX quote invalidates every position declaring any FX dependency, because the dependent pair may be triangulated through that quote. The returned dirty indices are sorted and deduplicated.

`revalue_affected` also refreshes base-currency conversion for **all** reused rows after any FX-key change, and uses `portfolio.as_of`. It relies on the caller's complete change manifest. It does not compare two market snapshots or discover omitted mutations.

## Reuse admission and execution

A prior valuation is reusable only when its effective date matches the new state, nonserialized provenance has the same exact metric/risk profile and base currency, and portfolio state identity matches (unless an internal authoritative instrument-change manifest permits it). The position count and ordered IDs must match. For every clean row, check entity ID, exact bits of the scale factor, base currency, and live instrument ID against the stored valuation result. Dirty indices must be in range. Failure of these checks selects full evaluation rather than an error.

Public position replacement and append assign fresh state IDs; clones keep the old ID until mutation. The checks do not hash instrument economics, market contents or `FinstackConfig`. A deserialized result has no private provenance and therefore cannot enter this fast path. A valid selective job copies clean `PositionValue` rows and reprices dirty rows, then always reassembles entity/portfolio totals in the ordinary reducer. Base-FX refresh updates base value, rate and triangulation flag while preserving native value.

## Limitations and alternatives

This establishes a structural admission mechanism, not proof that every supplied dirty set is complete. Changed config across calls is not independently fingerprinted in provenance; public changes to portfolio metadata are not all financial inputs, while instrument interior mutation violates the immutability precondition. No stale-price example was numerically reproduced in this audit.

Full repricing is the implemented fallback and is the correctness oracle for selective regression tests. A complete FX path manifest could narrow conservative invalidation but is not supplied here. [Accepted ADR 0046](../../architecture/0046-portfolio-execution-operational-contract.md) permits an explicitly supplied immutable prior result only with complete financial compatibility and an authoritative change set; otherwise it requires fresh valuation. Executor-retained cross-request valuation caches remain prohibited. The separate immutable [artifact decode cache](07-artifact-cache-and-load-reporting.md) stores instruments, not valuations. Adoption of this policy does not close the configuration and originating-market provenance gaps above.

## Evidence

[dependencies.rs](../../../../finstack-quant/portfolio/src/dependencies.rs), [plan.rs](../../../../finstack-quant/portfolio/src/evaluation/plan.rs), [executor.rs](../../../../finstack-quant/portfolio/src/evaluation/executor.rs), [portfolio.rs](../../../../finstack-quant/portfolio/src/portfolio.rs) and [valuation.rs](../../../../finstack-quant/portfolio/src/valuation.rs) own the implementation. Inspected [selective repricing tests](../../../../finstack-quant/portfolio/tests/selective_repricing.rs) cover profile changes, same-ID instrument replacement, position shape, FX-dependent native pricing and conversion-only rows. Inline dependency tests locate dynamic credit-index routing and FX triangulation. These compare with the same production full-pricing path; independent product accuracy and exhaustive dependency declarations remain outside their proof.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
