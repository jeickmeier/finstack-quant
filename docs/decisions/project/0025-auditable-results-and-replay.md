# ADR 0025: Auditable results and explicit replay limits

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

Valuation and risk workflows support audit/replay records. Results saved or
shared for that purpose carry or reference their inputs, calculation choices,
and producing build. Callers control archival storage; replay must not
silently substitute current data for missing historical inputs.

The record identifies, where applicable:

- Instrument or portfolio definitions and immutable market snapshots.
- Requested and effective valuation dates, scenarios, and measures.
- Selected models, effective settings, overrides, rounding, and FX policies.
- Library/build identity and stochastic seed or stream specification.
- The reproducibility guarantee established under
  [ADR 0021](../standards/0021-reproducibility-guarantees.md).

Immutable references may point to caller-retained inputs; raw inputs do not
need to accompany every result. A reference must identify the historical
content, not only a mutable object name. If the required inputs are
unavailable, report the missing information and the resulting replay limit.

Low-level scalar APIs can continue returning plain numbers. Detailed
calculation traces remain optional; provenance does not require a complete
decomposition for every model or hidden library-managed archival storage.

## Rationale

A number alone cannot establish which data, model choices, configuration, or
build produced it. Retained evidence makes a result explainable and supports
reproduction under an explicit guarantee. Caller-owned storage keeps this
capability compatible with independent library use under
[ADR 0017](0017-library-first-product.md).

## Examples

| Result or replay behavior | Treatment |
| --- | --- |
| Reference an archived market snapshot by an immutable identifier | Appropriate when the caller retains the referenced data. |
| Record only `USD-OIS` while that curve's contents change daily | Insufficient to identify the original market inputs. |
| Required replay inputs are unavailable | Report missing information; do not substitute today's curves. |
| Return a plain number from a low-level formula | Allowed; scalar APIs need not become audit envelopes. |
| Offer an optional detailed explanation trace | Allowed; trace detail is separate from provenance completeness. |

## Enforcement

- Restore representative archived inputs and effective settings, then
  reproduce results under the declared build and reproducibility guarantee.
- Verify that required metadata survives host bindings and relevant exports.
- Verify missing or unresolved replay inputs produce explicit diagnostics.
- Distinguish replay evidence from [ADR 0022](../architecture/0022-portfolio-evaluation-state.md)'s
  request/cache isolation and [ADR 0019](0019-final-release-qualification.md)'s
  release qualification. Each protects a different contract.

## Implementation notes and references

[ValuationProvenance](../../../finstack-quant/valuations/core/src/results/provenance.rs)
records model, requested date, declared market dependencies, scenario
adjustment, and bump sizes. The
[valuation result](../../../finstack-quant/valuations/core/src/results/valuation_result.rs)
permits absent provenance on directly constructed results and optional
explanation traces. Those mechanisms do not establish complete replay
information for every workflow.

[Market state snapshots](../../../finstack-quant/core/src/market_data/context/state_serde.rs)
and [portfolio replay](../../../finstack-quant/portfolio/src/replay.rs) provide
building blocks. This ADR does not add an archive service or certify universal
replay completeness. Evidence and workflow-coverage work is tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
