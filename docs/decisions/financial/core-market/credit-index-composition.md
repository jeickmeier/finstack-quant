# CORE-MARKET-19: Credit-index constituent coverage, weights, and fallback inputs

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`CreditIndexData` combines index hazard, base correlation, constituent count, recovery and optional issuer-level data. It supports a homogeneous pool when issuer curves are absent and complete heterogeneous coverage when present. It does not compute a pool loss distribution or tranche price.

## Algorithm, domain, and selection

Construction requires positive u16 constituent count, explicit index recovery in [0,1], an index hazard curve and a base-correlation curve. An issuer-curve map, when supplied, must be nonempty and have exactly the constituent count. Maps are keyed by issuer name in BTreeMap order; this is a complete list, not a sparse overlay.

Issuer recovery overrides require issuer curves, may cover a subset, must reference known issuer keys and lie in [0,1]. Missing override selects the index recovery. Issuer weights also require issuer curves and must cover every key exactly once, be finite/nonnegative, and sum to one within absolute tolerance 1e-9. No normalization is performed. With no weight map the getter returns `1/num_constituents`.

`get_issuer_curve` returns a named issuer curve when found and otherwise the index curve. The recovery and weight getters also return their aggregate/equal-weight defaults for an unknown issuer ID even when a full issuer map exists. Consequently builder completeness does not imply lookup is strict. Index recovery, each hazard curve's stored recovery and issuer recovery overrides are separate values and are not cross-validated here.

When inserted into `MarketContext`, all composite curve references are rebound by their curve IDs to canonical context entries. A supplied detached curve cannot override the same-ID context curve through this composite. Insertion fails for missing or wrong-type references. Later context updates/removals trigger rebinding and can remove invalidated composites; see [context lifecycle](context-lifecycle.md).

## Limitations, alternatives

No check equates the index hazard to a weighted issuer aggregate, and zero constituent weights are valid. Recovery endpoints are allowed at the data layer, even when a downstream spread-to-hazard transformation needs strictly positive loss given default. An unknown issuer silently falling back can conceal a misspelled identifier. Strict issuer lookup, normalized weights, or consistency constraints between recovery sources would be new choices and require agreement.

Supported homogeneous/equal-weight and heterogeneous/explicit-weight data modes are economically different inputs; the consuming pricing model must name which it uses. Full coverage rejects partially specified issuer-curve maps rather than filling their missing members.

## Evidence

Owning code and inline test: [credit_index.rs](../../../../finstack-quant/core/src/market_data/term_structures/credit_index.rs), [canonical reference insertion](../../../../finstack-quant/core/src/market_data/context/insert.rs). Inspected `issuer_curves_cover_every_constituent` rejects a one-name map for a two-name pool; [integration tests](../../../../finstack-quant/core/tests/market_data/credit_index.rs) locate weight-sum, equal-weight fallback and recovery-override tests. These are input-contract/invariant evidence, not an independent portfolio-loss benchmark. [Laurent–Gregory](../../../REFERENCES.md#laurent-gregory-2005-factor-copulas) and [ADR 0031](../models-and-valuations-credit/0031-tranche-loss-model-selection.md) provide downstream model context.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
