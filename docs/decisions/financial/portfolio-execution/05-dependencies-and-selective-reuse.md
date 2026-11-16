# PORTFOLIO-EXECUTION-005 — Dependency invalidation and selective valuation reuse

- **Decision status:** Accepted 2026-11-15 — complete typed dependency coverage, authoritative reuse proof, FX refresh and fresh valuation package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent typed routing, credit-index replacement, missing change proof and FX refresh examples verified. Production equivalence and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio dependencies and selective repricing

1. **Track exact dependencies for selected PV and metrics.** Use
   declarations to plan work. Reuse requires coverage of sources
   actually used and inputs that control source selection. Preserve
   namespace, family, ID and applicable role or coordinate. Bind
   the dependency index to the admitted holding order and return
   unique affected holdings in that order.
   [Active valuation bindings](../valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle) ·
   [Typed market identity](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

2. **Resolve credit-index dependencies in both states.** Track
   the exact index and its qualified aggregate hazard, applicable
   issuer hazards and base correlation in opening and target
   states. A replacement can invalidate a holding through either
   the old or new constituent. Reprice holdings with unresolved
   dependency coverage, including for an empty change list. A
   qualified empty dependency set can support reuse. Canonical
   pricing reports missing required data in holding order.

3. **Require immutable prior rows and authoritative change evidence.**
   Bind rows to the opening state and verify the exact target,
   ordered holdings, effective date, profile, model, registry,
   configuration, provider, overrides and reporting basis.
   Require a complete change set covering insertions, removals and
   changes in source eligibility or precedence. An admitted
   replacement in the same slot can invalidate that slot.
   Changes in date, profile or holding order require fresh work.
   [Selective reuse contract](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope) ·
   [Scenario change manifests](../scenarios/17-reports-and-replay.md#accepted-decision-for-checked-scenario-reports-and-replay).

4. **Run fresh work when proof is incomplete.** A bare changed-key
   list or restored result without verified provenance cannot
   authorize reuse. Reprice affected holdings and rows with
   incomplete requested risk. Malformed required inputs and
   failures in required pricing, sizing, FX or NAV remain errors.
   [Accepted valuation profiles](03-valuation-profiles-and-failures.md#accepted-decision-for-checked-portfolio-valuation-profiles-and-failures).

5. **Separate native repricing from reporting FX refresh.** An FX
   change invalidates native PV and metrics that may consume it
   unless authoritative evidence proves equivalent opening and
   target ordered queries. Include applicable dates, policies,
   authorities and route selection, including newly eligible
   quotes. For a reporting-only change retain qualified native
   Money and refresh the target base amount and FX provenance.
   [FX resolution](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks).

6. **Preserve actual provenance and reassemble the result.** Retain
   reused rows' original pricing provenance and update reporting
   conversion provenance where refreshed. Rebuild entity subtotals
   and NAV under record 004. Reuse raw base endpoints only with
   proof of both native pricing and FX compatibility. Otherwise
   recompute through the shared raw valuation path.
   [Execution and reduction](04-execution-order-and-reduction.md#accepted-decision-for-checked-portfolio-execution-and-nav-reduction).

7. **Keep direct selective repricing Rust-only and qualify against
   fresh execution.** Existing host workflows use the same Rust
   admission and results. Compare ordered native and base amounts,
   FX provenance, metric coverage, errors and totals. Keep
   financial caches request-local.
   [Host parity ledger](../../../../finstack-quant-py/parity_contract.toml).

## Independent calculation and routing checks

- A commodity source and forward source named `SHARED` route
  separately. Rebinding an index constituent `OLD` to `NEW`
  invalidates the index-linked holding on either dependency.
  Unresolved holdings remain eligible for fresh work when the
  change list is empty.
- A `100` payoff changes from `95` to `90` when its
  discount factor moves from `.95` to `.90`. A bare empty
  changed-key list does not establish that the prior mark of
  `95` is reusable.
- A qualified reporting-only change retains EUR 100 while USD
  per EUR moves from `1.10` to `1.20`, refreshing USD 110
  to USD 120. A fixed EUR/USD of 1.20 and USD/JPY moving from
  150 to 155 change the triangulated EUR/JPY from 180 to 186.

## Observed choice and scope at the audit revision

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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
