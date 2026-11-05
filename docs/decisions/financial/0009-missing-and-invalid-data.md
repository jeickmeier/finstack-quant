# ADR 0009: Invalid inputs, missing observations, and undefined results

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Financial conventions

## Decision

Each operation specifies which inputs are invalid, which missing observations
it permits, and how it represents an unestimable result. Implementations and
bindings preserve these distinctions. Filling, dropping, or substituting
values requires an explicitly selected, documented policy.

For performance analytics, the concrete behaviors below are accepted. Other
operations require their own explicitly agreed policies. This is not a global
rule that every `NaN` must be rejected or that all invalid requests should
produce an undefined numeric result.

## Rationale

A zero return, a missing observation, and a statistic that cannot be estimated
mean different things. Substituting one for another can produce plausible but
misleading portfolio results, while failing an entire panel can discard valid
results for unaffected tickers.

## Examples

| Situation | Required treatment |
|---|---|
| A rolling window has length zero | Reject with a validation error. |
| A series begins later or ends earlier than others in a panel | Permit leading/trailing gaps and use its valid observation span. |
| An observation is missing inside that span | Reject; do not silently forward-fill or insert a zero return. |
| Portfolio and benchmark have no overlapping observations for beta | Report a documented undefined result, not zero beta. |
| One ticker's metric is undefined in an otherwise valid panel | Preserve other tickers' results where the operation supports per-ticker outcomes. |

## Enforcement

- Test invalid requests separately from permitted missing observations.
- Test insufficient observations and non-overlapping windows explicitly.
- Include panels containing both valid and undefined per-ticker outcomes.
- Verify that bindings preserve the meaning of each result with documented
  host representations, including serialization of unavailable values.

## Implementation notes and references

The performance panel documents leading/trailing gaps as `NaN` in numeric
series and `null` on the wire, with a contiguous observed span. Existing
regressions cover the accepted distinctions. Their presence is not evidence
that every operation or binding already has complete contract coverage.

- [Performance panel and its wire representation](../../../finstack-quant/analytics/src/performance/panel.rs).
- [Performance correctness regressions](../../../finstack-quant/analytics/tests/correctness_regressions.rs): edge-ragged panels, interior missing values, zero windows, and empty-overlap outputs.
- [Financial defaults](0008-financial-defaults-and-cagr.md).
